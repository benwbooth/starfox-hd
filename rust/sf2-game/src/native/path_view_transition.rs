//! Paired scripted-view commands ($7F:B295/$7F:B320). The fixed primary
//! view is independent of both the live player pointer and path selection.

use super::super::actor_auxiliary::AuxiliaryKind;
use super::super::view_transition::{disable_projectiles, restore_view, save_view};
use super::*;

const BEGIN_CUE: u8 = 248;
const END_CUE: u8 = 247;

impl PathRuntime {
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
