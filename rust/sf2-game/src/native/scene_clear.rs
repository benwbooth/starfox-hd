//! Scene-transition actor selection and proxy teardown (`$03:AC81..ACDB`).
//! Actors remain live until normal retirement; this service does not release
//! contacts, program resources, attachments, or actor slots.

use super::scene_proxy::{SceneProxyError, SceneProxyStore};
use super::ObjectStore;

pub fn clear(
    objects: &mut ObjectStore,
    proxies: &mut SceneProxyStore,
) -> Result<(), SceneProxyError> {
    let mut current = objects.active_ids().first().copied();
    while let Some(owner) = current {
        let actor = objects
            .get(owner)
            .ok_or(SceneProxyError::MissingActor(owner))?;
        current = actor.base.next;
        if actor.base.flags.general_search_eligible {
            // Detachment precedes both release and the removal mark. A missing
            // proxy therefore leaves the handle cleared, but no removal mark.
            proxies.release_actor_proxy(objects, owner)?;
            objects
                .get_mut(owner)
                .ok_or(SceneProxyError::MissingActor(owner))?
                .base
                .flags
                .remove_after_tick = true;
        }
    }

    // Retired actors and repeated captures leave proxies without a current
    // actor handle. Release from the head to preserve subsequent slot reuse.
    // An excluded actor's handle is deliberately not repaired by this pass.
    while let Some(id) = proxies.active_ids().first().copied() {
        proxies.release(id)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "scene_clear_tests.rs"]
mod tests;
