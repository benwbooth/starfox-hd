//! Primary-player palette service (`$07:EA67..EACA`) with its complete
//! flash, red pulse and restoration branches. Path-owned transition delay
//! and consumable blockers are the existing shared owners, not copies.

use super::player_consumable::TriggeredUseBlockers;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore};

const FLASH_ACTIVE: u8 = 0x08;
const RESTORATION_ACTIVE: u8 = 0x10;
const PROGRESS_PULSE: u8 = 0x20;
const PROGRESS_RESTORATION: u8 = 0x40;
const PROGRESS_REQUESTED: u8 = 0x80;
const PROGRESS_FLAGS: u8 = PROGRESS_PULSE | PROGRESS_RESTORATION | PROGRESS_REQUESTED;
const PULSE_CLOCK_MASK: u16 = 0x18;
const PALETTE_BANK_COLORS: usize = 16;
const FIRST_FLASH_COLOR: usize = 64;
const FIRST_PULSE_COLOR: usize = 16;
const COMPONENT_MASK: u16 = 0x1F;
const GREEN_SHIFT: u32 = 5;
const BLUE_SHIFT: u32 = 10;
const FLASH_RED_STEP: u16 = 3;
const FLASH_OTHER_STEP: u16 = 1;
const FLASH_OTHER_LIMIT: u16 = 28;
const PULSE_GREEN_STEP: u16 = 1;
const PULSE_BLUE_STEP: u16 = 2;

/// Mission-progress flags in 6BE9. The same byte's bits 08 and 10 remain
/// owned by PlayerConsumableControl::projectile_blockers. Unused low bits
/// have no native behavior; no second copy of the blockers is retained.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerPaletteControl(u8);

impl PlayerPaletteControl {
    pub const fn from_control(control: u8) -> Self {
        Self(control & PROGRESS_FLAGS)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    /// `$06:9E19..9E24` requests a pulse only once. A previously requested
    /// but partially completed transition is deliberately not reactivated.
    pub fn request_progress_pulse(&mut self) {
        if self.0 & PROGRESS_REQUESTED == 0 {
            self.0 |= PROGRESS_FLAGS;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteError {
    World(WorldInputError),
    MissingPrimaryPlayer,
    MissingTargetControl(ObjectId),
    MissingConsumableControl(ObjectId),
    MissingPaletteControl(ObjectId),
    MissingPalette,
}

impl From<WorldInputError> for PaletteError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn components(color: u16) -> [u16; 3] {
    [
        color & COMPONENT_MASK,
        (color >> GREEN_SHIFT) & COMPONENT_MASK,
        (color >> BLUE_SHIFT) & COMPONENT_MASK,
    ]
}

fn pack([red, green, blue]: [u16; 3]) -> u16 {
    red | (green << GREEN_SHIFT) | (blue << BLUE_SHIFT)
}

/// Brightening skips the transparent first color of each of four banks.
/// The green/blue ceiling can LOWER a component already above that ceiling;
/// changed colors lose the unused high bit, while skipped colors retain it.
pub fn flash(world: &mut ScenePathWorld) -> Result<(), PaletteError> {
    world.palette_refresh_requested = Some(true);
    let palette = world.palette.as_mut().ok_or(PaletteError::MissingPalette)?;
    for (index, color) in palette
        .colors
        .iter_mut()
        .enumerate()
        .skip(FIRST_FLASH_COLOR)
    {
        if index % PALETTE_BANK_COLORS == 0 {
            continue;
        }
        let [red, green, blue] = components(*color);
        *color = pack([
            (red + FLASH_RED_STEP).min(COMPONENT_MASK),
            (green + FLASH_OTHER_STEP).min(FLASH_OTHER_LIMIT),
            (blue + FLASH_OTHER_STEP).min(FLASH_OTHER_LIMIT),
        ]);
    }
    Ok(())
}

/// Red progress pulse changes colors 16..127, including bank-first colors.
/// Red is unchanged; green and blue decrease at distinct source rates.
pub fn pulse(world: &mut ScenePathWorld) -> Result<(), PaletteError> {
    world.palette_refresh_requested = Some(true);
    let palette = world.palette.as_mut().ok_or(PaletteError::MissingPalette)?;
    for color in &mut palette.colors[FIRST_PULSE_COLOR..] {
        let [red, green, blue] = components(*color);
        *color = pack([
            red,
            green.saturating_sub(PULSE_GREEN_STEP),
            blue.saturating_sub(PULSE_BLUE_STEP),
        ]);
    }
    Ok(())
}

/// Restore four banks, including transparent colors. Completion tests the
/// ENTERING full color words, not the values after this visit's one-step
/// approach. A saved high bit can consequently keep restoration active.
pub fn restore(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), PaletteError> {
    world.palette_refresh_requested = Some(true);
    let palette = world.palette.as_mut().ok_or(PaletteError::MissingPalette)?;
    let mut changed = false;
    for (color, &saved) in palette.colors[FIRST_FLASH_COLOR..]
        .iter_mut()
        .zip(&palette.saved_colors[FIRST_FLASH_COLOR..])
    {
        if *color == saved {
            continue;
        }
        let current = components(*color);
        let target = components(saved);
        *color = pack(std::array::from_fn(|axis| {
            match current[axis].cmp(&target[axis]) {
                std::cmp::Ordering::Less => current[axis] + 1,
                std::cmp::Ordering::Greater => current[axis] - 1,
                std::cmp::Ordering::Equal => current[axis],
            }
        }));
        changed = true;
    }
    if !changed {
        let records = world.player_mut(objects, owner)?;
        let blockers = &mut records
            .consumable
            .as_mut()
            .ok_or(PaletteError::MissingConsumableControl(owner))?
            .projectile_blockers;
        *blockers = TriggeredUseBlockers::from_control(blockers.bits() & !RESTORATION_ACTIVE);
        let control = records
            .palette_effects
            .as_mut()
            .ok_or(PaletteError::MissingPaletteControl(owner))?;
        if control.0 & PROGRESS_PULSE == 0 {
            control.0 &= !PROGRESS_RESTORATION;
        }
    }
    Ok(())
}

/// The original explicitly selects the primary player, independent of the
/// actor invoking scene entry. Delay updates precede branch selection, so
/// the last delay visit can restore immediately. No palette is required on
/// an idle visit, and no primary record is reset on secondary dispatch.
pub fn advance_primary(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
) -> Result<(), PaletteError> {
    let owner = world
        .primary_player
        .ok_or(PaletteError::MissingPrimaryPlayer)?;
    let records = world.player_mut(objects, owner)?;
    let delay = &mut records
        .target_control
        .as_mut()
        .ok_or(PaletteError::MissingTargetControl(owner))?
        .transition_delay;
    let delay_was_active = *delay != 0;
    if delay_was_active {
        *delay -= 1;
    }
    let flash_remains_active = *delay != 0;
    let blockers = &mut records
        .consumable
        .as_mut()
        .ok_or(PaletteError::MissingConsumableControl(owner))?
        .projectile_blockers;
    if delay_was_active {
        *blockers = TriggeredUseBlockers::from_control(if flash_remains_active {
            blockers.bits() | FLASH_ACTIVE | RESTORATION_ACTIVE
        } else {
            blockers.bits() & !FLASH_ACTIVE
        });
    }
    let blockers = blockers.bits();
    if blockers & FLASH_ACTIVE != 0 {
        return flash(world);
    }
    if blockers & RESTORATION_ACTIVE != 0 {
        return restore(objects, world, owner);
    }
    let control = records
        .palette_effects
        .ok_or(PaletteError::MissingPaletteControl(owner))?;
    if control.0 & PROGRESS_PULSE != 0 {
        if world.strategy_clock & PULSE_CLOCK_MASK == 0 {
            pulse(world)
        } else {
            restore(objects, world, owner)
        }
    } else if control.0 & PROGRESS_RESTORATION != 0 {
        restore(objects, world, owner)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "player_palette_tests.rs"]
mod tests;
