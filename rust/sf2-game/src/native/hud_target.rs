//! The frame's display services for the primary player's target: the
//! target service (`$07:AA8C`) with its compass scan (`$07:B117`) and marker
//! classification (`$07:ACD8`), and the non-drawing parts of the HUD service
//! (`$07:A326`): target-mode tracking (`$07:A3F8..A417`), hit feedback
//! (`$07:B548`) and the candidate reset after drawing (`$07:A843..A86F`).
//! Reticle positioning and target retention (`$07:A418..A66B`) are the
//! existing reticle/lock services, run between those.
//!
//! The HUD gauges (6BA5..6BA7, 6BAA, 6BAC), the marker sprite selections
//! (6BDA..6BE1) and the drawing itself (`$07:A66C..A842`) feed only the
//! renderer and are not modeled here; nothing else reads them.

use super::path_target::{self, TargetAnchor, TargetSelection};
use super::player_reticle::{self, ReticleError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::view_transition::FixedViewAngles;
use super::{Angle, ObjectId, ObjectStore, Vector3};

/// `$07:B12D`: the compass rescans once every 32 frames.
const COMPASS_PERIOD_MASK: u16 = 0x1F;
/// `$07:B177`: records nearer than this are not compass targets.
const COMPASS_MINIMUM: u16 = 2000;
const NO_COMPASS: u16 = 0xFFFF;
/// Map record byte 12: compass target, and spawned.
const COMPASS_RECORD: u8 = 0x20;
const SPAWNED_RECORD: u8 = 0x01;
/// Display status bit 80 (6BB6): no target this frame.
const NO_TARGET: u8 = 0x80;
/// Control flag 20 (6BC2): the marker is off screen.
const MARKER_OFF_SCREEN: u8 = 0x20;
/// `$07:B4A5`: marker classification by yaw/pitch class.
const MARKER_CLASSES: [u8; 24] = [
    0x00, 0x01, 0x01, 0x00, 0x00, 0x02, 0x02, 0x00, 0x03, 0x05, 0x05, 0x00, 0x03, 0x06, 0x06, 0x00,
    0x04, 0x07, 0x07, 0x00, 0x04, 0x08, 0x08, 0x00,
];
const FORCED_MARKER: u8 = 0x0A;
const SCRIPTED_MARKER: u8 = 0x09;
const TRACKING_MODE: u16 = 8;
const NO_TARGET_YAW: u16 = 0x8000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudTargetError {
    World(WorldInputError),
    Reticle(ReticleError),
    MissingPrimaryPlayer,
    MissingFixedView,
    MissingProxy,
    MissingObjectiveState,
    MissingActionGate,
    MissingPlayerServices,
    MissingViewMode,
    MissingSceneInhibition,
    MissingReticleEnable,
    MissingMapRecords,
    MissingCompletionCode,
    MissingSelection(ObjectId),
    MissingTargetLock(ObjectId),
    MissingTargetControl(ObjectId),
    MissingSecondaryPlayer,
    /// `$03:D9F4` dispatches only modes 0, 2, 4, 6 and 8.
    UnknownScreenEffect(u16),
}

impl From<WorldInputError> for HudTargetError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

struct Actors {
    primary: ObjectId,
    view: ObjectId,
    proxy: ObjectId,
}

fn actors(world: &ScenePathWorld) -> Result<Actors, HudTargetError> {
    Ok(Actors {
        primary: world.primary_player.ok_or(HudTargetError::MissingPrimaryPlayer)?,
        view: world.fixed_players[0].ok_or(HudTargetError::MissingFixedView)?,
        proxy: world
            .weapons
            .and_then(|weapons| weapons.fallback)
            .ok_or(HudTargetError::MissingProxy)?,
    })
}

fn selection<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut TargetSelection, HudTargetError> {
    world
        .player_mut(objects, owner)?
        .target_selection
        .as_mut()
        .ok_or(HudTargetError::MissingSelection(owner))
}

fn position(objects: &ObjectStore, id: ObjectId) -> Result<Vector3, HudTargetError> {
    Ok(objects.get(id).ok_or(WorldInputError::MissingActor(id))?.base.position)
}

fn set_position(objects: &mut ObjectStore, id: ObjectId, position: Vector3) -> Result<(), HudTargetError> {
    objects.get_mut(id).ok_or(WorldInputError::MissingActor(id))?.base.position = position;
    Ok(())
}

/// `$7F:249F`: the X/Z distance estimate between two actors (12DE).
fn estimated_distance(from: Vector3, to: Vector3) -> u16 {
    let half = |delta: i16| ((delta.wrapping_abs() as u16) as i16) >> 1;
    let x = half(to.x.wrapping_sub(from.x));
    let z = half(to.z.wrapping_sub(from.z));
    let sum = (z.wrapping_add(x) as u16).wrapping_shl(1);
    let larger = if z.wrapping_sub(x) < 0 { x } else { z };
    let total = (larger as u16).wrapping_add(sum) as i16;
    let step = (total >> 1).wrapping_add(total);
    ((step >> 1) >> 1) as u16
}

/// `$07:AE51`: the clamped Manhattan X/Z distance (12DE).
fn clamped_distance(from: Vector3, to: Vector3) -> u16 {
    const LIMIT: u16 = 0x0FFF;
    let axis = |delta: i16| {
        let value = delta.wrapping_abs() as u16;
        if value & 0xF000 != 0 { LIMIT } else { value }
    };
    let sum = axis(from.x.wrapping_sub(to.x)).wrapping_add(axis(from.z.wrapping_sub(to.z)));
    if sum & 0xF000 != 0 { LIMIT } else { sum }
}

/// `$03:D87D..D9EC` without its colour-math and HDMA output: for the
/// primary player (and the secondary in two-player mode) count the hit
/// feedback down, then step the screen-effect machine (`$03:D9ED`).
pub fn advance_screen_effects(objects: &ObjectStore, world: &mut ScenePathWorld) -> Result<(), HudTargetError> {
    const TWO_PLAYERS: u16 = 0x0001;
    let primary = world.primary_player.ok_or(HudTargetError::MissingPrimaryPlayer)?;
    step_screen_effects(objects, world, primary)?;
    if world
        .view_transition_mode
        .ok_or(HudTargetError::MissingViewMode)?
        .flags
        & TWO_PLAYERS
        != 0
    {
        let secondary = world
            .secondary_player
            .ok_or(HudTargetError::MissingSecondaryPlayer)?;
        step_screen_effects(objects, world, secondary)?;
    }
    Ok(())
}

fn step_screen_effects(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), HudTargetError> {
    const IDLE: u16 = 0;
    const START: u16 = 2;
    const EXPAND: u16 = 4;
    const FADE: u16 = 6;
    const END: u16 = 8;
    let records = world.player_mut(objects, owner)?;
    let hit = &mut records
        .contact
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit;
    // `$03:D8EC`: the countdown; the tint itself is colour math.
    hit.feedback_duration = hit.feedback_duration.saturating_sub(1);
    let control = records
        .target_control
        .as_mut()
        .ok_or(HudTargetError::MissingTargetControl(owner))?;
    match control.mode & 0x00FF {
        IDLE => {}
        START => {
            control.progress = 0;
            control.mode = (control.mode & 0xFF00) | EXPAND;
        }
        EXPAND => {
            if control.progress < control.limit {
                control.progress = control.progress.wrapping_add(control.positive_range);
            } else {
                control.mode = FADE;
            }
        }
        FADE => {
            for limit in &mut control.axis_limits {
                if *limit != 0 {
                    let next = limit.wrapping_sub(control.control);
                    *limit = if (next as i8) < 0 { 0 } else { next };
                }
            }
            if control.axis_limits == [0; 3] {
                control.mode = (control.mode & 0xFF00) | IDLE;
            }
        }
        END => control.mode = (control.mode & 0xFF00) | IDLE,
        _ => return Err(HudTargetError::UnknownScreenEffect(control.mode)),
    }
    Ok(())
}

/// `$07:AA8C`: the per-frame target service.
pub fn advance_target_service(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
) -> Result<(), HudTargetError> {
    let actors = actors(world)?;
    let blocked = !world
        .contacts_enabled()
        .ok_or(HudTargetError::MissingObjectiveState)?
        || world
            .action_gate
            .ok_or(HudTargetError::MissingActionGate)?
            .code
            != 0
        || world
            .player_service_flags
            .ok_or(HudTargetError::MissingPlayerServices)?
            .minimum_protection()
        || world
            .scripted_view_active()
            .ok_or(HudTargetError::MissingViewMode)?
        || world
            .reticle_inhibited
            .ok_or(HudTargetError::MissingSceneInhibition)?;
    if !blocked {
        advance_compass(objects, world, &actors)?;
        let candidate = selection(objects, world, actors.primary)?.candidate;
        if candidate.is_some()
            && world
                .reticle_enabled
                .ok_or(HudTargetError::MissingReticleEnable)?
        {
            return classify_marker(objects, world, &actors);
        }
    }
    selection(objects, world, actors.primary)?.display_status |= NO_TARGET;
    Ok(())
}

/// `$07:B117..B1E9`. With no candidate, rescan the map records every 32
/// frames for the nearest compass target, then offer it to the selection.
fn advance_compass(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    actors: &Actors,
) -> Result<(), HudTargetError> {
    if selection(objects, world, actors.primary)?.candidate.is_some() {
        return Ok(());
    }
    if world.strategy_clock & COMPASS_PERIOD_MASK == 0 {
        let view = position(objects, actors.view)?;
        let mut best = NO_COMPASS;
        let mut best_position = Vector3::default();
        let records = world.map_records.as_ref().ok_or(HudTargetError::MissingMapRecords)?;
        for &id in records.active_ids() {
            let record = records.get(id).expect("listed map record");
            if record.flags.0 & COMPASS_RECORD == 0 || record.flags.0 & SPAWNED_RECORD != 0 {
                continue;
            }
            // The scan poses the proxy at each candidate record.
            objects
                .get_mut(actors.proxy)
                .ok_or(WorldInputError::MissingActor(actors.proxy))?
                .base
                .position = record.position;
            let distance = estimated_distance(view, record.position);
            if distance >= COMPASS_MINIMUM && distance < best {
                best = distance;
                best_position = record.position;
            }
        }
        let selection = selection(objects, world, actors.primary)?;
        selection.compass_distance = best;
        selection.compass_position = best_position;
    }
    let compass = *selection(objects, world, actors.primary)?;
    if compass.compass_distance != NO_COMPASS {
        set_position(objects, actors.proxy, compass.compass_position)?;
        if compass.compass_distance >= COMPASS_MINIMUM {
            let anchor = TargetAnchor::from_view(
                objects.get(actors.view).ok_or(WorldInputError::MissingActor(actors.view))?,
            );
            path_target::consider(
                selection(objects, world, actors.primary)?,
                actors.proxy,
                compass.compass_position,
                anchor,
            );
        }
    }
    Ok(())
}

/// `$07:AAF8..AB8E` with `$07:ACD8..AE50`: classify the marker, update the
/// off-screen flag and on-screen point, then the display status and style.
fn classify_marker(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    actors: &Actors,
) -> Result<(), HudTargetError> {
    let code = scan_marker(objects, world, actors)?;
    let current = *selection(objects, world, actors.primary)?;
    let lock = world
        .player_mut(objects, actors.primary)?
        .target_lock
        .as_mut()
        .ok_or(HudTargetError::MissingTargetLock(actors.primary))?;
    let mut style = lock.marker_style;
    let mut status = current.display_status;
    if code & 0x80 != 0 {
        if status & 0x80 == 0 {
            style = style.wrapping_add(0x10) & 0x1F;
        }
    } else {
        style &= 0x0F;
        status &= 0x7F;
    }
    status = (status & 0xE0) | (code & 0x1F);
    if current.forced_owner.is_some() {
        status = (status & 0xF0) | FORCED_MARKER;
        style = style.wrapping_add(1);
        if style & 0x0F >= 5 {
            style = (style & 0xF0) | 3;
        }
    } else {
        style = style.wrapping_add(1);
        if style & 0x0F >= 6 {
            style &= 0xF0;
        }
    }
    lock.marker_style = style;
    selection(objects, world, actors.primary)?.display_status = status;
    Ok(())
}

/// `$07:ACD8..AE50`: the marker class (1DB0) from the clipped angles, then,
/// for an on-screen candidate, its projected screen point.
fn scan_marker(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    actors: &Actors,
) -> Result<u8, HudTargetError> {
    let current = *selection(objects, world, actors.primary)?;
    let clipped = current.clipped_yaw & 0x0F;
    let mut code = clipped;
    if clipped != 0 {
        let mut class = 0u8;
        let mut yaw = (current.yaw >> 8) as u8;
        if (yaw as i8) < 0 {
            class = 4;
            yaw = yaw.wrapping_neg();
        }
        if yaw >= 0x0A {
            class += 1;
        }
        if yaw >= 0x40 {
            class += 1;
        }
        let pitch = (current.pitch >> 8) as u8;
        if (pitch as i8) < 0 {
            if pitch < 0xF8 {
                class |= 0x08;
            }
        } else if pitch >= 0x08 {
            class |= 0x10;
        }
        code = MARKER_CLASSES[usize::from(class)];
    }
    if code & 0x0F != 0 {
        selection(objects, world, actors.primary)?.control_flags |= MARKER_OFF_SCREEN;
    } else {
        selection(objects, world, actors.primary)?.control_flags &= !MARKER_OFF_SCREEN;
        let scripted = world
            .view_transition_mode
            .ok_or(HudTargetError::MissingViewMode)?
            .active();
        let completion = world
            .node_exit
            .completion_code
            .ok_or(HudTargetError::MissingCompletionCode)?;
        if completion != 0 || scripted {
            code = (code & 0xF0) | SCRIPTED_MARKER;
        }
    }
    if clipped == 0 && current.candidate.is_some() {
        let view = objects
            .get(actors.view)
            .ok_or(WorldInputError::MissingActor(actors.view))?;
        let origin = view.base.position;
        let view_angles = FixedViewAngles::capture(view);
        let mut delta = [
            current.position.x.wrapping_sub(origin.x),
            current.position.y.wrapping_sub(origin.y),
            current.position.z.wrapping_sub(origin.z),
        ];
        let mut largest = delta.iter().map(|value| value.wrapping_abs()).max().expect("three axes");
        // Halve every axis until the largest magnitude fits 12 bits.
        while largest as u16 & 0xF000 != 0 {
            largest = ((largest as u16) >> 1) as i16;
            for value in &mut delta {
                *value >>= 1;
            }
        }
        let point = Vector3 {
            x: origin.x.wrapping_add(delta[0]),
            y: origin.y.wrapping_add(delta[1]),
            z: origin.z.wrapping_add(delta[2]),
        };
        set_position(objects, actors.proxy, point)?;
        // `$07:AFFA`: projected from the view's position, with the proxy's
        // own pitch and yaw for the marker offset.
        world.marker_projection.published_view_angles = Some(view_angles);
        let proxy = objects
            .get(actors.proxy)
            .ok_or(WorldInputError::MissingActor(actors.proxy))?;
        let projected = player_reticle::project_marker(
            world,
            [proxy.base.pitch, proxy.base.yaw],
            point,
            origin,
        )
        .map_err(HudTargetError::Reticle)?;
        let screen = projected.map(|value| value.max(0) as u8);
        selection(objects, world, actors.primary)?.screen = screen;
    }
    Ok(code)
}

/// `$07:A3F8..A417`: while the target control's mode tracks a visible origin
/// and any axis limit remains, keep tracking; otherwise fall back to mode 8
/// with the configuration unlocked.
pub fn advance_target_mode(objects: &mut ObjectStore, world: &mut ScenePathWorld) -> Result<(), HudTargetError> {
    let actors = actors(world)?;
    let keep = tracks_origin(objects, world, &actors)? && {
        let control = world
            .player_mut(objects, actors.primary)?
            .target_control
            .as_mut()
            .ok_or(HudTargetError::MissingTargetControl(actors.primary))?;
        // `$07:B4FD`: decay each nonzero limit by its rate, floored at zero.
        for (limit, rate) in control.axis_limits.iter_mut().zip(control.axis_rates) {
            if *limit != 0 {
                let next = limit.wrapping_sub(rate);
                *limit = if (next as i8) < 0 { 0 } else { next };
            }
        }
        control.axis_limits.iter().any(|&limit| limit != 0)
    };
    if !keep {
        let control = world
            .player_mut(objects, actors.primary)?
            .target_control
            .as_mut()
            .ok_or(HudTargetError::MissingTargetControl(actors.primary))?;
        control.mode = TRACKING_MODE;
        control.progress = 0;
        control.configuration_locked = false;
        control.offset_enabled = false;
    }
    Ok(())
}

/// `$07:B3AF..B455`. A zero mode or an origin in front of the view stops
/// tracking. Authored offsets keep it. Otherwise the positive range follows
/// the origin's distance; the projected offsets it also writes (6C1E/6C20)
/// are rewritten before the next read by `$07:B80C` and are not modeled.
fn tracks_origin(objects: &mut ObjectStore, world: &mut ScenePathWorld, actors: &Actors) -> Result<bool, HudTargetError> {
    let control = world
        .player(objects, actors.primary)?
        .target_control
        .ok_or(HudTargetError::MissingTargetControl(actors.primary))?;
    if control.mode == 0 {
        return Ok(false);
    }
    if control.offset_enabled {
        return Ok(true);
    }
    // `$07:B8CA`: pose the proxy at the origin, turned from the view.
    set_position(objects, actors.proxy, control.origin)?;
    let view = objects
        .get(actors.view)
        .ok_or(WorldInputError::MissingActor(actors.view))?;
    let view_position = view.base.position;
    let view_yaw = (FixedViewAngles::capture(view).yaw >> 8) as u8;
    let bearing = (sf_core::aim_angle::sf2_atan16(
        view_position.x.wrapping_sub(control.origin.x),
        view_position.z.wrapping_sub(control.origin.z),
    ) >> 8) as u8;
    let proxy_yaw = bearing.wrapping_neg();
    objects
        .get_mut(actors.proxy)
        .ok_or(WorldInputError::MissingActor(actors.proxy))?
        .base
        .yaw = Angle::from_units(proxy_yaw);
    let facing = view_yaw.wrapping_neg().wrapping_sub(proxy_yaw).wrapping_add(0x40);
    if (facing as i8) >= 0 {
        return Ok(false);
    }
    // `$07:AFFA` publishes the view angles before projecting the proxy.
    world.marker_projection.published_view_angles = Some(FixedViewAngles::capture(
        objects.get(actors.view).expect("validated view"),
    ));
    // `$07:B77B`: nearer origins widen the positive range.
    let distance = clamped_distance(control.origin, view_position);
    let high = distance >> 8;
    let high = if high & 0xFFF0 != 0 { 0x0F } else { high };
    let widening = match !high & 0x0F {
        0 => 1,
        value => value,
    };
    let range = control.range.wrapping_add(widening as i16);
    world
        .player_mut(objects, actors.primary)?
        .target_control
        .as_mut()
        .expect("validated target control")
        .positive_range = if range < 0 { 1 } else { range as u16 };
    Ok(true)
}

/// `$07:B548..B64A`: start, hold and decay the hit-flash tint; the feedback
/// duration ends with the tint.
pub fn advance_feedback(objects: &ObjectStore, world: &mut ScenePathWorld) -> Result<(), HudTargetError> {
    let actors = actors(world)?;
    let hit = &mut world
        .player_mut(objects, actors.primary)?
        .contact
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerContact(actors.primary))?
        .hit;
    let mut decay = true;
    if hit.feedback_duration == 0 {
        hit.tint = [0; 3];
        hit.feedback_flags = 0;
    } else if hit.feedback_flags & 0x20 != 0 {
        let flags = hit.feedback_flags;
        if flags & 0x02 != 0 {
            let add = if flags & 0x01 != 0 { 4 } else { 2 };
            hit.feedback_flags = flags & 0xDC;
            let level = hit.tint[0].wrapping_add(add);
            let level = if level & 0xE0 != 0 { 0x1F } else { level };
            hit.tint = [level; 3];
            hit.tint_step = [4; 3];
            decay = false;
        } else if flags & 0x40 != 0 || flags & 0x80 == 0 {
            hit.tint = if flags & 0x08 != 0 {
                [0, 0, 0x0C]
            } else if flags & 0x10 != 0 {
                [0x1F, 0, 0x04]
            } else {
                [0x16, 0x12, 0x12]
            };
            hit.tint_step = [4; 3];
            hit.feedback_flags = (flags & 0xDF) | if flags & 0x40 != 0 { 0x80 } else { 0 };
        }
    }
    if decay {
        for (level, step) in hit.tint.iter_mut().zip(hit.tint_step) {
            if *level != 0 {
                let next = level.wrapping_sub(step);
                *level = if (next as i8) < 0 { 0 } else { next };
            }
        }
    }
    if hit.tint == [0; 3] {
        hit.feedback_duration = 0;
    }
    Ok(())
}

/// `$07:A843..A86F`: after drawing, the candidate is released for the next
/// epoch's paths to compete for.
pub fn reset_candidate(objects: &ObjectStore, world: &mut ScenePathWorld) -> Result<(), HudTargetError> {
    let actors = actors(world)?;
    let selection = selection(objects, world, actors.primary)?;
    selection.candidate = None;
    selection.auxiliary_distance = 0xFFFF;
    selection.distance = 0xFFFF;
    selection.yaw = NO_TARGET_YAW;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compass_distance_keeps_the_source_shifts() {
        let origin = Vector3::default();
        // |dx| = 3000, |dz| = 4000: halves 1500/2000, sum*2 = 7000, plus the
        // larger half 2000 = 9000, then (9000 / 2 + 9000) / 4 = 3375.
        assert_eq!(estimated_distance(origin, Vector3 { x: 3000, y: 0, z: -4000 }), 3375);
        assert_eq!(clamped_distance(origin, Vector3 { x: 0x7000, y: 0, z: 5 }), 0x0FFF);
    }
}
