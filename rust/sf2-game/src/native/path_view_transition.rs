//! Paired scripted-view commands ($7F:B295/$7F:B320). The fixed primary
//! view is independent of both the live player pointer and path selection.

use super::super::actor_auxiliary::AuxiliaryKind;
use super::super::view_transition::{
    disable_projectiles, restore_view, save_view, FixedViewAngles,
};
use super::*;

const BEGIN_CUE: u8 = 248;
const END_CUE: u8 = 247;

/// The view's three word angles share their high bytes with ordinary actor
/// fields. Keep these named aliases live, not a second orientation snapshot.
fn view_angles(view: &Object) -> [u16; 3] {
    let angles = FixedViewAngles::capture(view);
    [angles.pitch, angles.yaw, angles.roll]
}

fn set_view_angles(view: &mut Object, angles: [u16; 3]) {
    FixedViewAngles {
        pitch: angles[0],
        yaw: angles[1],
        roll: angles[2],
    }
    .write_to(view);
}

impl PathRuntime {
    /// $7F:BFF6/C005 and BE38/BE8C. These write the live view aliases, so
    /// path repetition and wait state share the angle high bytes as before.
    pub(super) fn execute_fixed_view_command(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        world: &PathWorld<'_>,
        command: super::super::view_transition::FixedViewCommand,
        next: PathCursor,
    ) -> Result<ControlStep, ProgramError> {
        use super::super::player_pose::quarter_word;
        use super::super::view_transition::FixedViewCommand;
        use sf_core::aim_angle::{sf2_atan16, sf2_xz_angle_distance};
        const FINE_ANGLE_SHIFT: u32 = 8;
        const PITCH_SHIFT_MASK: u8 = 7;
        if matches!(command, FixedViewCommand::AimTracking { .. }) {
            self.steering.unchanged_axes = 0;
        }
        let view = world.fixed_players[0].ok_or(ProgramError::MissingFixedView)?;
        objects
            .get(view)
            .ok_or(PathRuntimeError::MissingActor(view))?;
        match command {
            FixedViewCommand::CopyPosition | FixedViewCommand::ChasePosition => {
                let position = objects
                    .get(owner)
                    .expect("validated view source")
                    .base
                    .position;
                let destination = &mut objects.get_mut(view).expect("validated view").base.position;
                if command == FixedViewCommand::CopyPosition {
                    *destination = position;
                } else {
                    use super::super::path_fields::chase_word;
                    destination.x = chase_word(destination.x as u16, position.x as u16) as i16;
                    destination.y = chase_word(destination.y as u16, position.y as u16) as i16;
                    destination.z = chase_word(destination.z as u16, position.z as u16) as i16;
                }
            }
            FixedViewCommand::CopyRotation => {
                let source = objects.get(owner).expect("validated view source");
                let angles = [source.base.pitch, source.base.yaw, source.base.roll]
                    .map(|angle| u16::from(angle.units()) << FINE_ANGLE_SHIFT);
                set_view_angles(objects.get_mut(view).expect("validated view"), angles);
            }
            FixedViewCommand::CopyRotationFromView => {
                let angles = FixedViewAngles::capture(objects.get(view).expect("validated view"));
                let actor = objects.get_mut(owner).expect("validated view reader");
                actor.base.pitch = super::super::Angle::from_units((angles.pitch >> FINE_ANGLE_SHIFT) as u8);
                actor.base.yaw = super::super::Angle::from_units((angles.yaw >> FINE_ANGLE_SHIFT) as u8);
                actor.base.roll = super::super::Angle::from_units((angles.roll >> FINE_ANGLE_SHIFT) as u8);
            }
            FixedViewCommand::SetYawWord(word) => {
                let camera = objects.get_mut(view).expect("validated view");
                let mut angles = FixedViewAngles::capture(camera);
                angles.yaw = word;
                angles.write_to(camera);
            }
            FixedViewCommand::ChaseRotation => {
                use super::super::path_fields::chase_word;
                let source = objects.get(owner).expect("validated view source");
                let target = [source.base.pitch, source.base.yaw, source.base.roll]
                    .map(|angle| u16::from(angle.units()) << FINE_ANGLE_SHIFT);
                let camera = objects.get_mut(view).expect("validated view");
                let angles = FixedViewAngles::capture(camera);
                FixedViewAngles {
                    pitch: chase_word(angles.pitch, target[0]),
                    yaw: chase_word(angles.yaw, target[1]),
                    roll: chase_word(angles.roll, target[2]),
                }
                .write_to(camera);
            }
            FixedViewCommand::EaseYawTowardThreeQuarterTurn => {
                const THREE_QUARTER_TURN: u16 = 0xC000;
                let camera = objects.get_mut(view).expect("validated view");
                let mut angles = FixedViewAngles::capture(camera);
                let half = ((angles.yaw.wrapping_sub(THREE_QUARTER_TURN)) as i16) >> 1;
                let quarter = half >> 1;
                angles.yaw = (quarter as u16)
                    .wrapping_add(half as u16)
                    .wrapping_add(THREE_QUARTER_TURN);
                angles.write_to(camera);
            }
            FixedViewCommand::AimTracking { pitch_shift, chase } => {
                let tracking = world
                    .camera_tracking
                    .as_ref()
                    .ok_or(ProgramError::MissingCameraTrackingTarget)?
                    .actor
                    .ok_or(ProgramError::MissingCameraTrackingTarget)?;
                // The tracking handle (1DFF) can name an actor retired since it
                // was selected; the freed record keeps its last position until
                // the slot is reused.
                let (center, _) = objects
                    .attachment_parent_pose(tracking)
                    .ok_or(PathRuntimeError::MissingActor(tracking))?;
                let camera = objects.get(view).expect("validated view");
                let delta = super::super::Vector3 {
                    x: center.x.wrapping_sub(camera.base.position.x),
                    y: center.y.wrapping_sub(camera.base.position.y),
                    z: center.z.wrapping_sub(camera.base.position.z),
                };
                let current = view_angles(camera);
                let pitch = (sf2_atan16(delta.y, sf2_xz_angle_distance(delta.x, delta.z))
                    .wrapping_neg() as i16)
                    >> (pitch_shift & PITCH_SHIFT_MASK);
                let yaw = sf2_atan16(delta.x, delta.z);
                let angles = [
                    if chase {
                        quarter_word(current[0], pitch as u16)
                    } else {
                        pitch as u16
                    },
                    if chase {
                        quarter_word(current[1], yaw)
                    } else {
                        yaw
                    },
                    quarter_word(current[2], 0),
                ];
                set_view_angles(objects.get_mut(view).expect("validated view"), angles);
            }
        }
        objects
            .get_mut(owner)
            .expect("validated view source")
            .base
            .path = Some(next);
        Ok(ControlStep::Continue)
    }

    /// Source $7F:B376/$7F:B43B: fixed primary view, not selected/live player.
    pub(super) fn execute_fixed_view_motion(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        world: &PathWorld<'_>,
        snap: bool,
        next: PathCursor,
    ) -> Result<ControlStep, ProgramError> {
        use super::super::path_fields::chase_word;
        let source = objects
            .get(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?;
        let target_position = source.base.position;
        // All targets are calculated before angle stores, including owner=view.
        let target_angles = [source.base.pitch, source.base.yaw, source.base.roll]
            .map(|angle| u16::from_le_bytes([0, angle.units()]).wrapping_neg());
        let view_id = world.fixed_players[0].ok_or(ProgramError::MissingFixedView)?;
        let view = objects
            .get_mut(view_id)
            .ok_or(PathRuntimeError::MissingActor(view_id))?;
        let approach = |current: i16, target: i16| {
            if snap {
                target
            } else {
                chase_word(current as u16, target as u16) as i16
            }
        };
        view.base.position.x = approach(view.base.position.x, target_position.x);
        view.base.position.y = approach(view.base.position.y, target_position.y);
        view.base.position.z = approach(view.base.position.z, target_position.z);
        // The original deliberately performs the Z chase twice per invocation.
        view.base.position.z = approach(view.base.position.z, target_position.z);
        view.base.view_rear_distance = approach(view.base.view_rear_distance, 0);
        let current_angles = view_angles(view);
        set_view_angles(
            view,
            std::array::from_fn(|axis| {
                if snap {
                    target_angles[axis]
                } else {
                    chase_word(current_angles[axis], target_angles[axis])
                }
            }),
        );
        objects
            .get_mut(owner)
            .expect("validated view-motion owner")
            .base
            .path = Some(next);
        Ok(ControlStep::Continue)
    }

    pub(super) fn execute_view_transition(
        &mut self,
        catalog: &PathCatalog,
        objects: &mut ObjectStore,
        owner: ObjectId,
        world: &mut PathWorld<'_>,
        enabled: bool,
        next: PathCursor,
    ) -> Result<ControlStep, ProgramError> {
        // Missing host observations are input errors, before state changes.
        world
            .view_transition_mode
            .as_ref()
            .ok_or(ProgramError::MissingViewTransitionMode)?;
        world.audio.as_ref().ok_or(ProgramError::MissingAudio)?;
        let auxiliary = objects
            .get(owner)
            .ok_or(PathRuntimeError::MissingActor(owner))?
            .extension
            .auxiliary
            .clone();
        let needs_view = enabled
            || auxiliary
                .find(&self.resources, owner, AuxiliaryKind::SavedView)
                .map_err(ProgramError::Auxiliary)?
                .is_some();
        let view = if needs_view {
            let view = world.fixed_players[0].ok_or(ProgramError::MissingFixedView)?;
            objects
                .get(view)
                .ok_or(PathRuntimeError::MissingActor(view))?;
            Some(view)
        } else {
            None
        };

        world
            .view_transition_mode
            .as_deref_mut()
            .expect("validated view mode")
            .set_active(enabled);
        objects
            .get_mut(owner)
            .expect("validated transition owner")
            .base
            .contacts
            .run_when_paused = enabled;
        let mut continuation = next;
        if enabled {
            disable_projectiles(objects);
            let saved = objects
                .get(view.expect("save requires fixed view"))
                .expect("validated view")
                .clone();
            save_view(
                &mut self.resources,
                &mut objects
                    .get_mut(owner)
                    .expect("validated transition owner")
                    .extension
                    .auxiliary,
                owner,
                &saved,
            )
            .map_err(ProgramError::ViewSave)?;
        } else if let Some(view) = view {
            let restored = restore_view(
                &mut self.resources,
                &auxiliary,
                owner,
                objects.get_mut(view).expect("validated fixed view"),
            )
            .map_err(ProgramError::ViewSave)?;
            if restored && view == owner {
                // The source's ordinary advance reads the now-restored path
                // field. If owner and view alias, this is the saved C2's
                // continuation, not the C3 instruction's continuation.
                let saved_path = objects.get(owner).expect("restored view owner").base.path;
                let Some(path) = saved_path else {
                    return Err(ProgramError::InvalidSavedViewPath(None));
                };
                let Statement::ViewTransition {
                    enabled: true,
                    next,
                } = catalog.statement(path)?
                else {
                    return Err(ProgramError::InvalidSavedViewPath(saved_path));
                };
                continuation = next;
            }
        }
        // $7F:A3B8 passes the primary pointer directly to A439. Neither
        // selected-player routing nor distance/marker observations apply.
        world
            .audio
            .as_mut()
            .expect("validated transition audio")
            .events
            .queue(super::super::SoundEvent::Authored(
                super::super::path_sound::AuthoredCue::new(
                    if enabled { BEGIN_CUE } else { END_CUE },
                    0,
                    super::super::path_control::PlayerTarget::Primary,
                ),
            ));
        objects
            .get_mut(owner)
            .expect("validated transition owner")
            .base
            .path = Some(continuation);
        Ok(ControlStep::Continue)
    }
}
