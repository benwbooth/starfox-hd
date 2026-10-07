//! Fixed-view save/restore support (`$7F:B295..B373`) and its projectile
//! cleanup (`$03:A6A5`). The saved source record ends at base field 3E;
//! separately indexed extension state is deliberately not rolled back.

#[cfg(test)]
#[path = "scene_view_tests.rs"]
mod scene_tests;

use super::actor_auxiliary::{ActorAuxiliary, AuxiliaryError, AuxiliaryKind, AuxiliaryRecord};
use super::collision_pass::ExclusionGroups;
use super::object::ObjectBase;
use super::path_motion::MotionSettings;
use super::path_runtime::ActorPathState;
use super::path_trigger_conditions::TriggerActorState;
use super::program_resources::{AllocationFailure, ProgramResources};
use super::program_state::ProgramData;
use super::ObjectId;
use super::{Object, ObjectStore, Vector3};

const HOSTILE_PROJECTILE_CLASSES: ExclusionGroups = ExclusionGroups::from_authored_class(0x50);
const VIEW_BASE_COST: u16 = 63;
const SCRIPTED_VIEW_MODE: u16 = 0x0002;

/// Authored operations on the fixed primary view, not on the selected player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedViewCommand {
    CopyPosition,
    ChasePosition,
    CopyRotation,
    /// Reverse of `CopyRotation` (`$7F:C0BF`): the owner's rotation bytes take
    /// the view's three angle HIGH bytes (0x13/0x15/0x17), dropping fine bits.
    CopyRotationFromView,
    /// `$7F:C069`: chase the view's three angle words toward the owner's
    /// rotation bytes widened into their high bytes (`CopyRotation`'s target).
    ChaseRotation,
    /// `$07:F52B`: ease the view's FULL yaw word a quarter of the way to
    /// 0xC000 (three-quarter turn), using two arithmetic halvings.
    EaseYawTowardThreeQuarterTurn,
    /// Aim at the shared tracking actor, with signed fine-pitch attenuation.
    AimTracking {
        pitch_shift: u8,
        chase: bool,
    },
}

/// Full fixed-view angles share base storage with ordinary actor fields.
/// Read and write those aliases directly so camera motion, view save/restore,
/// target projection and warning bearings always observe the same state.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FixedViewAngles {
    pub pitch: u16,
    pub yaw: u16,
    pub roll: u16,
}

impl FixedViewAngles {
    pub fn capture(view: &Object) -> Self {
        Self {
            pitch: u16::from_le_bytes([view.base.pitch.units(), view.base.child_number]),
            yaw: u16::from_le_bytes([
                view.base.yaw.units(),
                view.extension.path_state.repeat_counter,
            ]),
            roll: u16::from_le_bytes([view.base.roll.units(), view.base.wait_timer]),
        }
    }

    pub fn write_to(self, view: &mut Object) {
        let [pitch, child_number] = self.pitch.to_le_bytes();
        let [yaw, repeat_counter] = self.yaw.to_le_bytes();
        let [roll, wait_timer] = self.roll.to_le_bytes();
        view.base.pitch = super::Angle::from_units(pitch);
        view.base.child_number = child_number;
        view.base.yaw = super::Angle::from_units(yaw);
        view.extension.path_state.repeat_counter = repeat_counter;
        view.base.roll = super::Angle::from_units(roll);
        view.base.wait_timer = wait_timer;
    }

    pub fn heading(self) -> super::Angle {
        const FINE_ANGLE_FRACTION_BITS: u32 = 8;
        super::Angle::from_units((self.yaw >> FINE_ANGLE_FRACTION_BITS) as u8)
    }
}

/// Shared execution-mode word ($1B84). Only the scripted-view bit is changed
/// by C2/C3; the other mode bits and companion byte remain live. This mode
/// determines fresh actors' pause exemption and suppresses nearby warnings.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ViewTransitionMode {
    pub flags: u16,
}

impl ViewTransitionMode {
    pub const fn active(self) -> bool {
        self.flags & SCRIPTED_VIEW_MODE != 0
    }

    pub fn set_active(&mut self, active: bool) {
        if active {
            self.flags |= SCRIPTED_VIEW_MODE;
        } else {
            self.flags &= !SCRIPTED_VIEW_MODE;
        }
    }

    pub fn spawn_defaults(
        self,
        mut defaults: super::ObjectSpawnDefaults,
    ) -> super::ObjectSpawnDefaults {
        defaults.run_when_paused = self.active();
        defaults
    }

    pub fn warning_control(
        self,
        mut control: super::proximity_warning::WarningControl,
    ) -> super::proximity_warning::WarningControl {
        control.globally_disabled = self.active();
        control
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewSaveError {
    Auxiliary(AuxiliaryError),
    Allocation(AllocationFailure),
    MissingSavedView,
}

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

/// Allocate a fresh actor-owned save, then publish it in auxiliary type 8.
/// Repeated saves deliberately retain the previous owned payload. If table
/// growth fails, the newly allocated payload also remains owned for later
/// actor cleanup, matching the source's allocation-before-publication order.
pub fn save_view(
    resources: &mut ProgramResources<ProgramData>,
    auxiliary: &mut ActorAuxiliary,
    owner: ObjectId,
    view: &Object,
) -> Result<(), ViewSaveError> {
    let value = ProgramData::SavedView(Box::new(ViewBaseSnapshot::capture(view)));
    let id = resources
        .allocate_owned(owner, VIEW_BASE_COST, value)
        .map_err(|error| ViewSaveError::Allocation(error.reason))?;
    auxiliary
        .set(resources, owner, AuxiliaryRecord::SavedView(id))
        .map_err(ViewSaveError::Auxiliary)
}

/// Restore a present save and free only its payload. The auxiliary table
/// retains its reference, so repeating restore without another save is a
/// stale-reference error, not an absent-record success. No saved entry is a
/// valid no-copy case; the outer command still performs its mode/audio work.
pub fn restore_view(
    resources: &mut ProgramResources<ProgramData>,
    auxiliary: &ActorAuxiliary,
    owner: ObjectId,
    view: &mut Object,
) -> Result<bool, ViewSaveError> {
    let Some(AuxiliaryRecord::SavedView(id)) = auxiliary
        .find(resources, owner, AuxiliaryKind::SavedView)
        .map_err(ViewSaveError::Auxiliary)?
    else {
        return Ok(false);
    };
    let Some(ProgramData::SavedView(snapshot)) = resources.get_owned(owner, id) else {
        return Err(ViewSaveError::MissingSavedView);
    };
    snapshot.restore(view);
    resources
        .release_owned(owner, id)
        .expect("validated saved-view owner");
    Ok(true)
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
