//! Fixed-view continuity after camera-mode dispatch ($07:97FB..9AAA).
//! A mode supplies the current pose; this service captures requested offsets,
//! decays them, applies retained rotation/position and saves the final pose.
//! All aliases use the same actor fields as path motion and view save/restore.

use super::player_steering::half_word;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::view_transition::FixedViewAngles;
use super::{Angle, Object, ObjectStore, Rotation, Vector3};
use sf_core::snes_trig::rotate_16xz;

#[cfg(test)]
#[path = "view_blend_tests.rs"]
mod tests;

const SLOW_POSITION_DIVISOR: i16 = 16;
const FINE_ANGLE_SHIFT: u32 = 8;
const FINE_FRACTION_MASK: u16 = 0x00FF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewBlendError {
    World(WorldInputError),
    MissingFixedView,
    MissingViewMode,
    MissingProxy,
}

impl From<WorldInputError> for ViewBlendError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

/// Camera meanings of the shared base motion flags. Do not retain a second
/// copy: authored paths and base-view saves must observe the same bits.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ViewBlendControl {
    pub capture_position: bool,
    pub capture_rotation: bool,
    pub discard_capture: bool,
    pub position_active: bool,
    pub rotation_active: bool,
    pub fast_position_recovery: bool,
}

impl ViewBlendControl {
    pub fn capture(view: &Object) -> Self {
        let motion = view.extension.path_state.motion;
        Self {
            capture_position: motion.follow_player_displacement,
            capture_rotation: motion.generate_velocity_each_step,
            discard_capture: motion.carry_selected_player,
            position_active: motion.bank_turn,
            rotation_active: motion.view_rotation_blend,
            fast_position_recovery: motion.suppress_child_refresh,
        }
    }

    pub fn write_to(self, view: &mut Object) {
        let motion = &mut view.extension.path_state.motion;
        motion.follow_player_displacement = self.capture_position;
        motion.generate_velocity_each_step = self.capture_rotation;
        motion.carry_selected_player = self.discard_capture;
        motion.bank_turn = self.position_active;
        motion.view_rotation_blend = self.rotation_active;
        motion.suppress_child_refresh = self.fast_position_recovery;
    }
}

fn high(angle: u16) -> u8 {
    (angle >> FINE_ANGLE_SHIFT) as u8
}

fn add_high(angle: u16, delta: Angle) -> u16 {
    (angle & FINE_FRACTION_MASK)
        | (u16::from(high(angle).wrapping_add(delta.units())) << FINE_ANGLE_SHIFT)
}

fn decay_angle(angle: Angle) -> Angle {
    let signed = angle.units() as i8;
    Angle::from_units(match signed {
        1.. => (signed - 1) as u8,
        ..=-1 => (signed + 1) as u8,
        0 => 0,
    })
}

fn decay_position(current: i16, fast: bool) -> i16 {
    if fast {
        return half_word(current as u16, 0) as i16;
    }
    // $7F:25E8 uses a signed, wrapped delta and minimum progress, rounding
    // each negative divide toward zero. The half-word tie stays negative.
    let delta = current.wrapping_neg();
    let step = if delta == 0 {
        0
    } else if delta < 0 {
        delta.min(-SLOW_POSITION_DIVISOR) / SLOW_POSITION_DIVISOR
    } else {
        delta.max(SLOW_POSITION_DIVISOR) / SLOW_POSITION_DIVISOR
    };
    current.wrapping_add(step)
}

pub fn advance(objects: &mut ObjectStore, world: &ScenePathWorld) -> Result<(), ViewBlendError> {
    let view_id = world.fixed_players[0].ok_or(ViewBlendError::MissingFixedView)?;
    let scripted = world
        .view_transition_mode
        .ok_or(ViewBlendError::MissingViewMode)?
        .active();
    let view = objects
        .get_mut(view_id)
        .ok_or(WorldInputError::MissingActor(view_id))?;
    if scripted {
        view.extension.relative_position = Vector3::default();
    }
    let mut control = ViewBlendControl::capture(view);
    if control.capture_position || control.capture_rotation {
        if control.discard_capture {
            control.discard_capture = false;
            control.capture_position = false;
            control.capture_rotation = false;
        } else {
            if control.capture_position {
                control.capture_position = false;
                control.position_active = true;
                let previous = view.extension.path_state.platform_carry.saved_position;
                view.extension.relative_position = Vector3 {
                    x: previous.x.wrapping_sub(view.base.position.x),
                    y: previous.y.wrapping_sub(view.base.position.y),
                    z: previous.z.wrapping_sub(view.base.position.z),
                };
            }
            if control.capture_rotation {
                control.capture_rotation = false;
                control.rotation_active = true;
                let current = FixedViewAngles::capture(view);
                let previous = view.extension.path_state.motion_delta;
                view.extension.relative_rotation = Rotation {
                    pitch: Angle::from_units(
                        high(previous.x as u16).wrapping_sub(high(current.pitch)),
                    ),
                    yaw: Angle::from_units(high(previous.y as u16).wrapping_sub(high(current.yaw))),
                    roll: Angle::from_units(
                        high(previous.z as u16).wrapping_sub(high(current.roll)),
                    ),
                };
                view.base.target_speed = high(current.yaw);
            }
        }
    }
    let offset = &mut view.extension.relative_position;
    offset.x = decay_position(offset.x, control.fast_position_recovery);
    offset.y = decay_position(offset.y, control.fast_position_recovery);
    offset.z = decay_position(offset.z, control.fast_position_recovery);
    if *offset == Vector3::default() {
        control.position_active = false;
    }
    let rotation = &mut view.extension.relative_rotation;
    rotation.pitch = decay_angle(rotation.pitch);
    rotation.yaw = decay_angle(rotation.yaw);
    rotation.roll = decay_angle(rotation.roll);
    if *rotation == Rotation::default() {
        control.rotation_active = false;
    }
    control.write_to(view);
    if control.rotation_active {
        let current = FixedViewAngles::capture(view);
        let rotation = view.extension.relative_rotation;
        FixedViewAngles {
            pitch: add_high(current.pitch, rotation.pitch),
            yaw: add_high(current.yaw, rotation.yaw),
            roll: add_high(current.roll, rotation.roll),
        }
        .write_to(view);
    }
    if control.position_active {
        let current = FixedViewAngles::capture(view);
        let offset = view.extension.relative_position;
        let yaw_delta = view.base.target_speed.wrapping_sub(high(current.yaw));
        let proxy_id = world
            .weapons
            .and_then(|state| state.fallback)
            .ok_or(ViewBlendError::MissingProxy)?;
        let proxy = objects
            .get_mut(proxy_id)
            .ok_or(WorldInputError::MissingActor(proxy_id))?;
        proxy.base.pitch = Angle::from_units(high(current.pitch));
        proxy.base.roll = Angle::from_units(high(current.roll));
        proxy.base.yaw = Angle::from_units(yaw_delta);
        let (x, z) = rotate_16xz(yaw_delta, offset.x, offset.z);
        let view = objects.get_mut(view_id).expect("validated fixed view");
        view.base.position.x = view.base.position.x.wrapping_add(x);
        view.base.position.z = view.base.position.z.wrapping_add(z);
        view.base.position.y = view.base.position.y.wrapping_add(offset.y);
    }
    let view = objects.get_mut(view_id).expect("validated fixed view");
    view.extension.path_state.platform_carry.saved_position = view.base.position;
    let current = FixedViewAngles::capture(view);
    view.extension.path_state.motion_delta = Vector3 {
        x: current.pitch as i16,
        y: current.yaw as i16,
        z: current.roll as i16,
    };
    Ok(())
}
