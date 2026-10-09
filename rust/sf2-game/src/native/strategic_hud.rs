//! The map's HUD layout service (`$04:A3ED`, layout 5 = `$04:A429`), run in
//! each service frame after the sprite pass: the planet's pending damage
//! drains into its health one point every other frame (and a destroyed
//! planet ends the campaign), the map's palette cycles advance, and the
//! panel's numbers are redrawn.
//!
//! The palette cycles copy 32-byte rows inside the palette shadow
//! (EFE5..F4E5); the numbers are published as `HudNumber`s for the
//! renderer.

use super::strategic_director::MapDirector;
use super::strategic_screen::{MapScreen, ScreenError, ScreenLinks};
use super::strategic_sim::StrategicMap;

/// EFE5..F4E5: the palette shadow (EFE5..F1E5) and the cycle sources.
pub const PALETTE_BYTES: usize = 0x500;
const PALETTE_BASE: u16 = 0xEFE5;
/// `$04:AC08`: the planet's colours by health (three words per row).
const PLANET_COLOURS: [u16; 6] = [0x2EFF, 0x2F9F, 0x57FF, 0x1CF6, 0x1CFF, 0x4A5F];
const ROW: usize = 0x20;

/// The service's own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapHud {
    /// F4E5..F4E7: the markers' palette cycle (countdown, reload, row
    /// offset word); F4F3..F4F5 the stations'.
    pub marker_cycle: [u8; 4],
    pub station_cycle: [u8; 4],
    /// EFE5..F4E5.
    pub palette: Vec<u8>,
}

impl Default for MapHud {
    fn default() -> Self {
        Self { marker_cycle: [0; 4], station_cycle: [0; 4], palette: vec![0; PALETTE_BYTES] }
    }
}

/// A number the panel shows, at a text cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HudNumber {
    pub column: u8,
    pub row: u8,
    pub digits: u8,
    pub value: u16,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HudOutput {
    pub numbers: Vec<HudNumber>,
}

/// Words the service reads that other owners keep.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HudInputs {
    /// 1AA7 bit 10 turns the service off.
    pub service_flags: u8,
    /// 1BA2: the layout (5 on the map).
    pub layout: u8,
    /// D816: the score; 1DD2 the lives.
    pub score: u16,
    pub lives: u8,
}

/// `$04:A3ED`.
#[allow(clippy::too_many_arguments)]
pub fn service(
    hud: &mut MapHud,
    screen: &mut MapScreen,
    director: &mut MapDirector,
    map: &mut StrategicMap,
    links: &mut ScreenLinks,
    phase: u16,
    inputs: HudInputs,
    output: &mut HudOutput,
) -> Result<(), ScreenError> {
    if inputs.service_flags & 0x10 != 0 {
        return Ok(());
    }
    match inputs.layout {
        0 | 1 | 6 | 7 => return Ok(()),
        5 => {}
        other => return Err(ScreenError::Unported(0x04A416 + u32::from(other) * 2)),
    }
    // $04:A443.
    if drain(screen, director, map, links, phase) {
        map.globals.speed_flags |= 0x2000;
    }
    if map.globals.speed_flags & 0x2000 != 0 {
        // $04:A457.
        let percent = links.planet_damage_percent;
        if percent != 0x64 {
            output.numbers.push(HudNumber { column: 2, row: 0x17, digits: 2, value: percent });
        } else {
            output.numbers.push(HudNumber { column: 2, row: 0x17, digits: 3, value: percent });
        }
    }
    // $04:A495/$04:A4BE/$04:A4DC.
    if map.globals.speed_flags & 0x0002 != 0 {
        output.numbers.push(HudNumber { column: 0x19, row: 0x18, digits: 3, value: screen.elapsed_steps });
    }
    output.numbers.push(HudNumber { column: 0x18, row: 0x1A, digits: 5, value: inputs.score });
    if links.scene.stage_results & 0x0080 == 0 {
        output.numbers.push(HudNumber { column: 0x16, row: 0x19, digits: 1, value: u16::from(inputs.lives) });
    }
    cycles(hud, screen, map, links)?;
    map.globals.speed_flags &= !0x2000;
    Ok(())
}

/// `$7F:5FA0`: one point of the planet's pending damage, every other
/// frame; returns whether one was taken.
fn drain(
    screen: &mut MapScreen,
    director: &mut MapDirector,
    map: &mut StrategicMap,
    links: &mut ScreenLinks,
    phase: u16,
) -> bool {
    if links.scene.campaign_events & 0x0200 != 0 || phase != 0 || links.scene.planet_damage == 0 {
        return false;
    }
    links.scene.planet_damage -= 1;
    links.planet_health = links.planet_health.wrapping_sub(1);
    links.scene.scene_events |= 0x1000;
    links.planet_damage_percent = 0x0064u16.wrapping_sub(links.planet_health);
    if links.planet_damage_percent == 0x0064 {
        // The planet falls: the map stops and asks to leave.
        director.exit_state = 2;
        links.scene.scene_events |= 0x0100;
        map.globals.hold |= 0x003F;
        screen.interface = 0;
        screen.hover = 0;
        screen.ship.travel = 0;
        screen.wingmate.flags = 0;
        director.choosing = 0;
        links.mode |= 0x0030;
        links.scene.campaign_events |= 0x0200;
    }
    true
}

fn copy_row(hud: &mut MapHud, source: u16, destination: u16) -> Result<(), ScreenError> {
    let from = usize::from(source.wrapping_sub(PALETTE_BASE));
    let to = usize::from(destination.wrapping_sub(PALETTE_BASE));
    if from + ROW > PALETTE_BYTES || to + ROW > PALETTE_BYTES {
        return Err(ScreenError::TableOverrun(0x7E0000 | u32::from(source)));
    }
    let row: Vec<u8> = hud.palette[from..from + ROW].to_vec();
    hud.palette[to..to + ROW].copy_from_slice(&row);
    Ok(())
}

/// One palette cycle: count down, then step the row offset and copy.
fn cycle(counter: &mut [u8; 4], step: u16) -> Option<u16> {
    counter[0] = counter[0].wrapping_sub(1);
    let fired = if step == 0x20 { (counter[0] as i8) < 0 } else { counter[0] == 0 };
    if !fired {
        return None;
    }
    counter[0] = counter[1];
    let offset = u16::from_le_bytes([counter[2], counter[3]]).wrapping_add(step) & 0x00FF;
    counter[2..4].copy_from_slice(&offset.to_le_bytes());
    Some(offset)
}

/// `$04:ABE3`: the planet's three colours (EFE7+0x80) from `row`.
pub(super) fn planet_colours(hud: &mut MapHud, row: usize) {
    for k in 0..3 {
        let at = usize::from(0xEFE7u16 + 0x80 - PALETTE_BASE) + 2 * k;
        hud.palette[at..at + 2].copy_from_slice(&PLANET_COLOURS[row + k].to_le_bytes());
    }
}

/// `$04:AB07`.
fn cycles(hud: &mut MapHud, screen: &mut MapScreen, map: &StrategicMap, links: &ScreenLinks) -> Result<(), ScreenError> {
    if links.mode & 0x0040 != 0 {
        return Ok(());
    }
    if links.display_flags & 0x0200 != 0 {
        if let Some(offset) = cycle(&mut hud.marker_cycle, 0x20) {
            copy_row(hud, 0xF1E5 + offset, 0xF025)?;
        }
    }
    if links.display_flags & 0x0400 != 0 {
        let flash = &mut screen.warning_flash;
        if links.scene.scene_events & 0x0100 != 0 {
            flash[2..4].copy_from_slice(&0x0080u16.to_le_bytes());
            copy_row(hud, 0xF2E5 + 0x80, 0xF045)?;
        } else if map.globals.speed_flags & 0x1000 != 0 || (links.planet_damage_percent.wrapping_sub(0x32) as i16) >= 0 {
            flash[0] = flash[0].wrapping_sub(1);
            if flash[0] == 0 {
                flash[0] = flash[1];
                let offset = u16::from_le_bytes([flash[2], flash[3]]).wrapping_add(0x20) & 0x00FF;
                flash[2..4].copy_from_slice(&offset.to_le_bytes());
                copy_row(hud, 0xF2E5 + offset, 0xF045)?;
            }
        }
    }
    // $04:AC14.
    if links.display_flags & 0x0080 != 0 {
        let counter = &mut hud.station_cycle;
        counter[0] = counter[0].wrapping_sub(1);
        if counter[0] == 0 {
            counter[0] = counter[1];
            let offset = u16::from_le_bytes([counter[2], counter[3]]).wrapping_sub(0x20) & 0x00FF;
            counter[2..4].copy_from_slice(&offset.to_le_bytes());
            copy_row(hud, 0xF3E5 + offset, 0xF1C5)?;
        }
    }
    if map.globals.speed_flags & 0x2000 != 0 {
        let percent = links.planet_damage_percent;
        let row = if (percent.wrapping_sub(0x50) as i16) >= 0 {
            Some(3)
        } else if (percent.wrapping_sub(0x32) as i16) >= 0 {
            Some(0)
        } else {
            None
        };
        if let Some(row) = row {
            planet_colours(hud, row);
        }
    }
    Ok(())
}
