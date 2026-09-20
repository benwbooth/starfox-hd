//! Fixed-view save/restore support (`$7F:B295..B373`) and its projectile
//! cleanup (`$03:A6A5`). The saved source record ends at base field 3E;
//! separately indexed extension state is deliberately not rolled back.

use super::collision_pass::ExclusionGroups;
use super::object::ObjectBase;
use super::path_motion::MotionSettings;
use super::path_runtime::ActorPathState;
use super::path_trigger_conditions::TriggerActorState;
use super::{Object, ObjectStore, Vector3};

const HOSTILE_PROJECTILE_CLASSES: ExclusionGroups = ExclusionGroups::from_authored_class(0x50);

/// Typed contents of the fixed view actor's source base record. A source
/// copy includes the base links themselves, but never visits their targets
/// or repairs the active list. This is not a general-purpose actor clone or
/// a pool restore operation; callers retain ownership of the fixed view.
///
/// Some base fields currently live in ActorPathState for domain locality.
/// Enumerate those explicitly instead of cloning ObjectExtension, which
/// would also roll back callbacks, allocation handles, motion working words,
/// animation, relative pose, scene proxies, and other unsaved state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewBaseSnapshot {
    base: ObjectBase,
    needs_path_initialization: bool,
    script_parameter: u8,
    weapon_selection: u8,
    friend_health_slot: u8,
    hold_latched: bool,
    motion: MotionSettings,
    saved_position: Vector3,
    clear_on_path_exit_latch: bool,
    conditions: TriggerActorState,
    repeat_counter: u8,
}

impl ViewBaseSnapshot {
    pub fn capture(view: &Object) -> Self {
        // Exhaustive on purpose: any new actor-path field needs a decision
        // about whether it belongs to the saved base or live extension.
        let ActorPathState {
            needs_path_initialization,
            animation: _,
            motion_phase: _,
            motion_delta: _,
            script_parameter,
            weapon_selection,
            friend_health_slot,
            script_value: _,
            hold_latched,
            motion,
            platform_carry,
            clear_on_path_exit_latch,
            stack: _,
            triggers: _,
            conditions,
            repeat_counter,
            part: _,
        } = &view.extension.path_state;
        Self {
            base: view.base.clone(),
            needs_path_initialization: *needs_path_initialization,
            script_parameter: *script_parameter,
            weapon_selection: *weapon_selection,
            friend_health_slot: *friend_health_slot,
            hold_latched: *hold_latched,
            motion: *motion,
            saved_position: platform_carry.saved_position,
            clear_on_path_exit_latch: *clear_on_path_exit_latch,
            conditions: *conditions,
            repeat_counter: *repeat_counter,
        }
    }

    pub fn restore(&self, view: &mut Object) {
        view.base = self.base.clone();
        let path = &mut view.extension.path_state;
        path.needs_path_initialization = self.needs_path_initialization;
        path.script_parameter = self.script_parameter;
        path.weapon_selection = self.weapon_selection;
        path.friend_health_slot = self.friend_health_slot;
        path.hold_latched = self.hold_latched;
        path.motion = self.motion;
        path.platform_carry.saved_position = self.saved_position;
        path.clear_on_path_exit_latch = self.clear_on_path_exit_latch;
        path.conditions = self.conditions;
        path.repeat_counter = self.repeat_counter;
    }
}

/// Mark the two source projectile classes before capturing the view. Both
/// hostile-class bits must be present, or the hit-side class alone suffices.
/// High-level object/weapon kind, visibility, health, collision eligibility,
/// first visit, and search eligibility are not additional filters.
///
/// This does not retire any slot or run a callback. Existing links, contact
/// records, allocation ownership, and all other actor state remain live.
pub fn disable_projectiles(objects: &mut ObjectStore) {
    // No callbacks or link writes occur during this source traversal.
    for id in objects.active_ids().to_vec() {
        let actor = objects.get_mut(id).expect("active transition candidate");
        let classes = actor.base.contacts.exclusion_groups;
        if classes.intersection(HOSTILE_PROJECTILE_CLASSES) == HOSTILE_PROJECTILE_CLASSES
            || classes.excludes(ExclusionGroups::HIT_SIDE_CLASS)
        {
            actor.base.contacts.run_when_paused = true;
            actor.base.flags.collision_disabled = true;
            actor.base.hit_points = 0;
        }
    }
}

#[cfg(test)]
#[path = "view_transition_tests.rs"]
mod tests;
