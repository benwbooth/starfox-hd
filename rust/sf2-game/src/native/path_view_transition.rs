//! Paired scripted-view commands ($7F:B295/$7F:B320). The fixed primary
//! view is independent of both the live player pointer and path selection.

use super::super::actor_auxiliary::AuxiliaryKind;
use super::super::view_transition::{disable_projectiles, restore_view, save_view};
use super::*;

const BEGIN_CUE: u8 = 248;
const END_CUE: u8 = 247;

/// The view's three word angles share their high bytes with ordinary actor
/// fields. Keep these named aliases live, not a second orientation snapshot.
fn view_angles(view: &Object) -> [u16; 3] {
    [
        u16::from_le_bytes([view.base.pitch.units(), view.base.child_number]),
        u16::from_le_bytes([
            view.base.yaw.units(),
            view.extension.path_state.repeat_counter,
        ]),
        u16::from_le_bytes([view.base.roll.units(), view.base.wait_timer]),
    ]
}

fn set_view_angles(view: &mut Object, angles: [u16; 3]) {
    use super::super::Angle;
    let [pitch, child_number] = angles[0].to_le_bytes();
    let [yaw, repeat_counter] = angles[1].to_le_bytes();
    let [roll, wait_timer] = angles[2].to_le_bytes();
    view.base.pitch = Angle::from_units(pitch);
    view.base.child_number = child_number;
    view.base.yaw = Angle::from_units(yaw);
    view.extension.path_state.repeat_counter = repeat_counter;
    view.base.roll = Angle::from_units(roll);
    view.base.wait_timer = wait_timer;
}

impl PathRuntime {
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
