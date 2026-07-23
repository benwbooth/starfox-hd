//! Executable, proof-gated Star Fox 2 object-path interpreter.
//!
//! The instruction stream and handler identities come from `sf2-data`'s
//! closed retail-ROM graph.  Unreviewed handlers are hard errors: this crate
//! never substitutes an SF1 opcode merely because its machine-code shape is
//! similar.

pub use sf2_data::path::PathAddress;
use sf2_data::path::{
    PathCommand, PathHandler, PathSemantic, PATH_COMMANDS, PATH_HANDLERS, PATH_ROOTS,
};

pub const VAR_WAIT_COUNTER: u8 = 0x17;
pub const VAR_LOOP_COUNTER: u8 = 0x15;
pub const VAR_OBJECT_FLAGS_09: u8 = 0x09;
pub const VAR_PATH_FLAGS: u8 = 0x21;
pub const VAR_OBJECT_FLAGS_23: u8 = 0x23;
pub const VAR_VELOCITY: u8 = 0x18;
pub const VAR_OBJECT_FLAGS_20: u8 = 0x20;
pub const VAR_OBJECT_FLAGS_22: u8 = 0x22;
pub const VAR_OBJECT_FLAGS_24: u8 = 0x24;
pub const VAR_OBJECT_FLAGS_25: u8 = 0x25;
pub const VAR_OBJECT_FLAGS_26: u8 = 0x26;
pub const VAR_OBJECT_FLAGS_31: u8 = 0x31;
pub const VAR_WORLD_X: u8 = 0x0C;
pub const VAR_WORLD_Y: u8 = 0x0E;
pub const VAR_WORLD_Z: u8 = 0x10;
pub const VAR_ROTATION_X: u8 = 0x12;
pub const VAR_ROTATION_Y: u8 = 0x14;
pub const VAR_ROTATION_Z: u8 = 0x16;
pub const PATH_FLAG_RELATIVE_TO_PLAYER: u8 = 0x10;
pub const PATH_FLAG_HELICOPTER: u8 = 0x40;
pub const PATH_FLAG_INVISIBLE: u8 = 0x01;
pub const OBJECT_FLAG_INVISIBLE: u8 = 0x02;
pub const INDEXED_VARIABLE_TABLE: u16 = 0xD75C;
pub const INLINE_DISPATCH_INDEX: u16 = 0xD77F;
const INLINE_DISPATCH_PATHS: [u16; 10] = [
    0xAA3A, 0xA8DC, 0xA964, 0xA8CA, 0xA915, 0xA8CA, 0xA95C, 0xA8D5, 0xA8C6, 0x8D81,
];
const LATE_INLINE_DISPATCH_PATHS: [u16; 10] = [
    0x040B, 0x07FE, 0x7A2E, 0x8D81, 0x96A1, 0xA26B, 0xC190, 0xF72C, 0xF7B1, 0xF7C9,
];
const INLINE_GLOBAL_CONTROL_PHASE: u16 = 0x1CD9;
const INLINE_GLOBAL_CONTROL_MODE: u16 = 0x1CDA;
const INLINE_GLOBAL_EVENT_FLAGS: u16 = 0x1D74;
const INLINE_COLOR_PHASE_EXTENSION: u16 = 0x1CCA;
const INLINE_PLAYER_RELATIVE_PHASE_EXTENSION: u16 = 0x1CE2;
const INLINE_OBJECT_FLAG_24_BIT_04: u8 = 0x04;
const INLINE_OBJECT_FLAG_25_BIT_02: u8 = 0x02;
const INLINE_INITIALIZE_LAUNCHED_EXTERNAL_OBJECT: u16 = 0xAC64;
const INLINE_EASE_FIXED_PLAYER_YAW: u16 = 0xB91A;
const INLINE_CONFIGURE_RANDOMIZED_OBJECT_MOTION: u16 = 0xD024;
const INLINE_INITIALIZE_SPAWNED_OBJECT_MOTION: u16 = 0xD0DE;
const INLINE_CHASE_YAW_OPPOSITE_FIXED_PLAYER: u16 = 0xDCBD;
const INLINE_ACCUMULATE_PLAYER_AUXILIARY_MOTION: u16 = 0xE78A;
const INLINE_INITIALIZE_PLAYER_AUXILIARY_CHARGE: u16 = 0xE839;
const INLINE_UPDATE_CONDITIONAL_OBJECT_PHASE: u16 = 0xE939;
const INLINE_SPAWN_PLAYER_LINKED_OBJECT: u16 = 0xE967;
const INLINE_LINK_SELECTED_OBJECT_TRANSFORM: u16 = 0xF078;
const INLINE_REFRESH_PLAYER_AUXILIARY_MODE: u16 = 0xF2E4;
const INLINE_ENABLE_PLAYER_AUXILIARY_CONTROL: u16 = 0xF391;
const INLINE_INITIALIZE_PLAYER_RELATIVE_MOTION: u16 = 0xF39E;
const INLINE_CHASE_CURRENT_RELATIVE_OFFSETS: u16 = 0xF3F0;
const INLINE_ADVANCE_CURRENT_RELATIVE_OFFSETS: u16 = 0xF45B;
const INLINE_CHASE_CURRENT_RELATIVE_POSE: u16 = 0xF46E;
const INLINE_RESET_PLAYER_AUXILIARY_TARGET: u16 = 0xF500;
const INLINE_APPLY_CURRENT_HEALTH_DECAY: u16 = 0xF659;
const INLINE_SEPARATE_YAW_TARGETS: u16 = 0xF693;
const INLINE_ADVANCE_VERTICAL_OSCILLATION: u16 = 0xF7C9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTransition {
    Unbecome,
    Become,
    BecomeMother,
    BecomeChild(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildSpawn {
    pub shape: u16,
    pub path: PathAddress,
    pub rotation: [u8; 3],
    pub hit_points: u8,
    pub attack_points: u8,
    pub offset: [i16; 3],
    pub child_number: u8,
}

/// Full-size object spawn used by retail path opcode `$031`.
///
/// Unlike the SF1 payload, SF2 stores the three local offsets as signed
/// 16-bit values.  Keeping that distinction here prevents the engine host
/// from accidentally truncating the all-range formation offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectSpawn {
    pub shape: u16,
    pub path: PathAddress,
    pub rotation: [u8; 3],
    pub hit_points: u8,
    pub attack_points: u8,
    pub offset: [i16; 3],
}

/// Reviewed object/transform services whose implementation needs the retail
/// object pool rather than only path variables.  Every recovered handler has
/// a distinct typed operation; hosts must implement it and cannot silently
/// accept an unknown opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2PathOperation {
    FaceSelectedSmooth,
    FaceLinkedSmooth,
    ExplodeObject,
    SpawnObject(ObjectSpawn),
    FlagLinkedObject,
    UnlinkChild(u8),
    AccumulateObject1cde(u8),
    SaturatingAddSelectedAuxWord(u16),
    RefreshSelectedRelativeTransform,
    SelectSelfAndClearRelativeTransform,
    InitializePlayerAuxWord(u16),
    SetPlayerAuxMode(bool),
    RefreshLinkedRotationDeltas,
    UpdatePilotAuxState,
    QueueFixedMarker1400(u8),
    QueueFixedMarker0320(u8),
    QueueSelectedMarkerPair { first: u8, second: u8 },
    FaceSelectedImmediate,
    ChasePlayerTowardObject,
    SnapPlayerToObject,
    RotateAroundLinkedPitch(i8),
    RotateLocalOffsetYaw(i8),
    RotateLocalOffsetPitch(i8),
    UnlinkSelf,
    PositionRelativeToLinked(i8),
    CopySelectedSlotWorldPosition,
    PositionExternalObjectAndFaceSelected,
    CallExternalStrategy(u8),
    CopySelectedAuxRotation,
    ConfigurePilotAuxModeA(u16),
    ConfigurePilotAuxModeB(u16),
    ApplyFormationOffset,
    FreeObjectAuxiliaryAndResetD742,
    ResetSelectedAuxiliaryMotion,
    IncrementLinkedAuxiliaryCounter,
    DecrementLinkedAuxiliaryCounter,
    SelectCurrentAsRotationTarget,
    ClearSelectedAuxiliaryFlag01,
    SetSelectedSlotLowNibble1,
    ChaseObjectPositionTowardCurrent(u16),
    CopySelectedRotation,
    CopyPositionToObject(u16),
    CopyRotationToObjectFixed(u16),
    PopPathStackPair,
    ConfigurePlayerAuxiliary(u16),
    SetObjectRotationTowardTarget { object: u16, shift: u8 },
    ChaseObjectRotationTowardTarget { object: u16, shift: u8 },
    RefreshOwnedPlayerAuxiliaryOrigin,
    InstallStrategyAndStop { strategy: u16, state: u8 },
    CaptureSelectedAuxiliaryMotion,
    LinkSpawnedObjectToCurrent,
    ClearObjectRelativeReference,
    PreserveCurrentPathContinuation,
    IncrementSelectedAuxiliaryStage,
    PreserveCurrentObjectForParent,
    ScaleHorizontalMotion,
    InitializeLaunchedExternalObject,
    EaseFixedPlayerYaw,
    ConfigureRandomizedObjectMotion,
    InitializeSpawnedObjectMotion,
    ChaseYawOppositeFixedPlayer,
    AccumulatePlayerAuxiliaryMotion,
    InitializePlayerAuxiliaryCharge,
    UpdateConditionalObjectPhase,
    SpawnPlayerLinkedObject,
    LinkSelectedObjectTransform,
    RefreshPlayerAuxiliaryMode,
    EnablePlayerAuxiliaryControl,
    InitializePlayerRelativeMotion,
    ChaseCurrentRelativeOffsets,
    AdvanceCurrentRelativeOffsets,
    ChaseCurrentRelativePose,
    ResetPlayerAuxiliaryTarget,
    ApplyCurrentHealthDecay,
    SeparateYawTargets,
    AdvanceVerticalOscillation,
}

/// Reviewed branch predicates that need selected/linked object state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2PathCondition {
    HitGround { offset: u16 },
    ProjectedSelectedPointNegative,
    SelectedLeftOfObject,
    ProjectedSelectedForwardPointNegative,
    SelectedBelowObject,
    SelectedOrCurrentAuxState,
    SelectedAuxiliaryMapCellOccupied,
    SelectedAuxiliaryFlag04Clear,
    SelectedAuxiliaryStateMatchesGlobal,
    PlayerOneFlag25Bit20,
}

/// Collision/contact categories selected by the reviewed three-way path
/// branch. `None` from [`Sf2PathHost::classify_path_contact`] means that no
/// contact occurred and execution falls through to the next command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathContactClass {
    /// Contact was reported without an object target.
    NoObject,
    /// The contacted object uses the ordinary auxiliary-contact class.
    AuxiliaryType0b,
    /// Player, inactive, or alternate-class object contact.
    OtherObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathTrigger {
    pub path: PathAddress,
    pub delay: u8,
    pub trigger: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedMarkerClass {
    Direct,
    Class1,
    Class2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerTargetUpdate {
    FlagLinked,
    Flag08,
}

/// Engine services used by every currently reviewed handler.
///
/// Variable IDs are the exact one-byte identifiers consumed by `$7F:CB47`.
/// They are deliberately not treated as offsets into a guessed Rust struct:
/// values at or above `$80` address SF2's parallel object arrays.
pub trait Sf2PathHost {
    type Error;

    fn read_variable_byte(&self, id: u8) -> Result<u8, Self::Error>;
    fn write_variable_byte(&mut self, id: u8, value: u8) -> Result<(), Self::Error>;
    fn read_variable_word(&self, id: u8) -> Result<u16, Self::Error>;
    fn write_variable_word(&mut self, id: u8, value: u16) -> Result<(), Self::Error>;

    fn read_external_byte(&self, address: u16) -> Result<u8, Self::Error>;
    fn write_external_byte(&mut self, address: u16, value: u8) -> Result<(), Self::Error>;
    fn read_external_word(&self, address: u16) -> Result<u16, Self::Error>;
    fn write_external_word(&mut self, address: u16, value: u16) -> Result<(), Self::Error>;
    fn read_external_long_byte(&self, address: u32) -> Result<u8, Self::Error>;
    fn read_external_long_word(&self, address: u32) -> Result<u16, Self::Error>;

    /// Read/write the SF2-only per-object extension arrays addressed as
    /// absolute WRAM offsets plus the current retail object index X.
    fn read_object_extension_byte(&self, offset: u16) -> Result<u8, Self::Error>;
    fn write_object_extension_byte(&mut self, offset: u16, value: u8) -> Result<(), Self::Error>;
    fn read_object_extension_word(&self, offset: u16) -> Result<u16, Self::Error>;
    fn write_object_extension_word(&mut self, offset: u16, value: u16) -> Result<(), Self::Error>;

    fn find_shape(&mut self, shape: u16) -> Result<(), Self::Error>;
    fn pointed_shape_is_dead(&self) -> Result<bool, Self::Error>;
    /// Resolve the retail mother/child links and report whether the selected
    /// child is absent or dead.
    fn child_is_dead(&mut self, child_number: u8) -> Result<bool, Self::Error>;
    /// Resolve the retail mother/child links and set bit `$08` in the selected
    /// child's object flags, if that child exists.
    fn flag_child(&mut self, child_number: u8) -> Result<(), Self::Error>;
    /// Spawn the current object's selected weapon and apply the retail
    /// projectile flag update performed by path opcode `$035`.
    fn fire_weapon(&mut self) -> Result<(), Self::Error>;
    /// Point the object fully at the active player (pitch and yaw) and
    /// regenerate its velocity vectors.
    fn face_player(&mut self) -> Result<(), Self::Error>;
    /// Execute the SF2-specific yaw-only face-player helper, including its
    /// linked-object bookkeeping.
    fn face_player_yaw(&mut self) -> Result<(), Self::Error>;
    /// Face the current object's mother in pitch and yaw when it exists,
    /// regenerating only vectors whose angle changed.
    fn face_mother(&mut self) -> Result<(), Self::Error>;
    /// Copy world X/Y/Z from the object selected in retail `$CF1F`.
    fn copy_selected_world_position(&mut self) -> Result<(), Self::Error>;
    fn enter_path_hold(&mut self) -> Result<(), Self::Error>;
    /// Return the distance produced by retail `$7F:8C25` for the object in
    /// `$CF1F`, or for the current object's mother when one exists.
    fn selected_distance(&mut self) -> Result<u16, Self::Error>;
    fn mother_distance(&mut self) -> Result<Option<u16>, Self::Error>;
    fn selected_within_range(&mut self, range: u16) -> Result<bool, Self::Error>;
    /// Calculate `angle(current - selected) + selected yaw` using retail's
    /// byte-angle convention.
    fn selected_relative_yaw(&mut self) -> Result<u8, Self::Error>;
    /// Calculate `angle(selected - current) + current yaw` using retail's
    /// byte-angle convention.
    fn selected_bearing_plus_yaw(&mut self) -> Result<u8, Self::Error>;
    fn rotate_around_selected_yaw(&mut self, angle: i8) -> Result<(), Self::Error>;
    fn rotate_around_selected_pitch(&mut self, angle: i8) -> Result<(), Self::Error>;
    /// Try a context transition whose target may not exist.  On success the
    /// host preserves the caller context for a later `Unbecome`.
    fn try_transition_context(
        &mut self,
        transition: ContextTransition,
        resume_at: PathAddress,
    ) -> Result<bool, Self::Error>;
    /// SF2's selected-object auxiliary records are indexed through the
    /// selected object's `$2B` slot field.
    fn selected_slot_class(&self) -> Result<u8, Self::Error>;
    fn selected_aux_flags(&self) -> Result<u8, Self::Error>;
    fn or_selected_aux_flags(&mut self, bits: u8) -> Result<(), Self::Error>;
    /// Preserve the selected auxiliary slot's high nibble and set its low
    /// nibble to `$4`, exactly as retail path opcode `$16F` does.
    fn set_selected_slot_low_nibble_4(&mut self) -> Result<(), Self::Error>;
    /// Allocate a retail auxiliary record of type `$0B` and store the path
    /// operand in its `$6A62` byte.
    fn allocate_auxiliary_type_0b(&mut self, value: u8) -> Result<(), Self::Error>;
    /// Allocate the companion retail auxiliary record type `$0D`.
    fn allocate_auxiliary_type_0d(&mut self, value: u8) -> Result<(), Self::Error>;
    /// Advance the selected auxiliary progress toward its retail terminal
    /// value and report whether both progress and the current phase were
    /// already settled before this command.
    fn advance_selected_auxiliary_progress(&mut self, step: u8) -> Result<bool, Self::Error>;
    /// Execute a reviewed operation which requires concrete object-pool or
    /// transform state.
    fn perform_path_operation(&mut self, operation: Sf2PathOperation) -> Result<(), Self::Error>;
    /// Evaluate a reviewed object-pool branch predicate.
    fn evaluate_path_condition(&mut self, condition: Sf2PathCondition)
        -> Result<bool, Self::Error>;
    /// Classify the current collision result for the reviewed three-way
    /// contact branch. `None` is the retail carry-clear fallthrough case.
    fn classify_path_contact(&mut self) -> Result<Option<PathContactClass>, Self::Error>;
    /// Execute retail `$0D:AF3A`, which refreshes the current object's
    /// collision target/projection fields `$1CE8`, `$1CEA`, and `$1CEB`.
    fn refresh_collision_target(&mut self) -> Result<(), Self::Error>;
    fn cancel_trigger(&mut self, path: PathAddress) -> Result<(), Self::Error>;
    /// Replace the active trigger continuation, including retail's strategy,
    /// counter, and rotation reset.  This is a no-op outside a trigger.
    fn force_trigger_path(&mut self, path: PathAddress) -> Result<(), Self::Error>;
    fn update_player_target(&mut self, update: PlayerTargetUpdate) -> Result<(), Self::Error>;
    fn queue_selected_marker(
        &mut self,
        value: u8,
        class: SelectedMarkerClass,
    ) -> Result<(), Self::Error>;
    fn spawn_linked_object_effects(&mut self) -> Result<(), Self::Error>;
    /// One byte from retail `random_l`, preserving the shared RNG stream.
    fn random_byte(&mut self) -> Result<u8, Self::Error>;
    fn do_queue(&mut self, queue: u8) -> Result<(), Self::Error>;
    fn set_trail(&mut self, trail: u8) -> Result<(), Self::Error>;
    /// Regenerate movement vectors after a velocity write.  Retail skips
    /// this while relative-to-player mode is active.
    fn regenerate_velocity_vectors(&mut self) -> Result<(), Self::Error>;
    fn set_sprite(&mut self, x: u8, y: u8) -> Result<(), Self::Error>;
    fn quick_spawn(
        &mut self,
        shape: u16,
        path: PathAddress,
        hit_points: u8,
        attack_points: u8,
    ) -> Result<(), Self::Error>;
    fn spawn_child(&mut self, spawn: ChildSpawn) -> Result<(), Self::Error>;
    fn remove_child(&mut self, child_number: u8) -> Result<(), Self::Error>;
    fn start_message(&mut self, message: u8) -> Result<(), Self::Error>;
    fn schedule_trigger(&mut self, trigger: PathTrigger) -> Result<(), Self::Error>;

    fn push_path_value(&mut self, value: u16) -> Result<(), Self::Error>;
    fn pop_path_value(&mut self) -> Result<Option<u16>, Self::Error>;

    /// Change the current object exactly as the retail become stack does.
    /// `resume_at` is the caller path after its one-byte opcode.
    fn transition_context(
        &mut self,
        transition: ContextTransition,
        resume_at: PathAddress,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YieldReason {
    Wait,
    WaitOne,
    Hold,
    Goto,
    Next,
    /// The retail path call stack was empty (or contained its `$FFFF`
    /// sentinel), so RETURN leaves this path invocation.
    Return,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStop {
    Yielded(YieldReason),
    BudgetExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunReport {
    pub commands_executed: usize,
    pub stop: RunStop,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PathVmError<E> {
    InvalidRoot(usize),
    CommandNotRecovered(PathAddress),
    HandlerNotRecovered(u16),
    UnsupportedOpcode { address: PathAddress, opcode: u16 },
    InvalidBitIndex { address: PathAddress, index: u8 },
    InvalidInlineDispatchIndex { address: PathAddress, index: u16 },
    InvalidInlineAddress(PathAddress),
    Host(E),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathVm {
    cursor: PathAddress,
    invert_next_condition: bool,
}

impl PathVm {
    pub fn new(cursor: PathAddress) -> Self {
        Self {
            cursor,
            invert_next_condition: false,
        }
    }

    pub fn from_root(root_index: usize) -> Result<Self, PathVmError<std::convert::Infallible>> {
        let root = PATH_ROOTS
            .get(root_index)
            .ok_or(PathVmError::InvalidRoot(root_index))?;
        Ok(Self::new(*root))
    }

    pub fn cursor(&self) -> PathAddress {
        self.cursor
    }

    /// Synchronize the VM with a host-owned object path while preserving
    /// dispatcher-local state such as a pending `IFNOT`.
    pub fn set_cursor(&mut self, cursor: PathAddress) {
        self.cursor = cursor;
    }

    pub fn invert_pending(&self) -> bool {
        self.invert_next_condition
    }

    pub fn run<H: Sf2PathHost>(
        &mut self,
        host: &mut H,
        command_budget: usize,
    ) -> Result<RunReport, PathVmError<H::Error>> {
        let mut executed = 0usize;
        while executed < command_budget {
            let command =
                command_at(self.cursor).ok_or(PathVmError::CommandNotRecovered(self.cursor))?;
            let handler = handler_for(command.opcode)
                .ok_or(PathVmError::HandlerNotRecovered(command.opcode))?;
            let semantic = handler.semantic.ok_or(PathVmError::UnsupportedOpcode {
                address: command.address,
                opcode: command.opcode,
            })?;
            executed += 1;

            let stop = self.execute(command, semantic, host)?;
            if let Some(reason) = stop {
                return Ok(RunReport {
                    commands_executed: executed,
                    stop: RunStop::Yielded(reason),
                });
            }
        }
        Ok(RunReport {
            commands_executed: executed,
            stop: RunStop::BudgetExhausted,
        })
    }

    fn execute<H: Sf2PathHost>(
        &mut self,
        command: &PathCommand,
        semantic: PathSemantic,
        host: &mut H,
    ) -> Result<Option<YieldReason>, PathVmError<H::Error>> {
        use PathSemantic::*;
        match semantic {
            ClearFlag21Bit08 => {
                set_byte_bits(host, VAR_PATH_FLAGS, 0x08, false)?;
                self.advance(command);
            }
            Wait => {
                let target = operand_byte(command, 1);
                let counter = host
                    .read_variable_byte(VAR_WAIT_COUNTER)
                    .map_err(PathVmError::Host)?;
                if counter != target {
                    host.write_variable_byte(VAR_WAIT_COUNTER, counter.wrapping_add(1))
                        .map_err(PathVmError::Host)?;
                    return Ok(Some(YieldReason::Wait));
                }
                host.write_variable_byte(VAR_WAIT_COUNTER, 0)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            RelativeToPlayerOn | RelativeToPlayerOff => {
                let flags = host
                    .read_variable_byte(VAR_PATH_FLAGS)
                    .map_err(PathVmError::Host)?;
                let flags = if semantic == RelativeToPlayerOn {
                    flags | PATH_FLAG_RELATIVE_TO_PLAYER
                } else {
                    flags & !PATH_FLAG_RELATIVE_TO_PLAYER
                };
                host.write_variable_byte(VAR_PATH_FLAGS, flags)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetVelocity => {
                host.write_variable_byte(VAR_VELOCITY, operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                let flags = host
                    .read_variable_byte(VAR_PATH_FLAGS)
                    .map_err(PathVmError::Host)?;
                if flags & PATH_FLAG_RELATIVE_TO_PLAYER == 0 {
                    host.regenerate_velocity_vectors()
                        .map_err(PathVmError::Host)?;
                }
                self.advance(command);
            }
            AddByte => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(operand_byte(command, 2));
                host.write_variable_byte(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddWord => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_word(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(operand_word(command, 2));
                host.write_variable_word(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            FacePlayerYaw => {
                host.face_player_yaw().map_err(PathVmError::Host)?;
                self.advance(command);
            }
            FacePlayer => {
                host.face_player().map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetByte => {
                host.write_variable_byte(operand_byte(command, 2), operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetWord => {
                host.write_variable_word(operand_byte(command, 3), operand_word(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetWeapon => {
                host.write_variable_byte(0x2F, operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            FindShape => {
                host.find_shape(operand_word(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            End => {
                set_byte_bits(host, VAR_OBJECT_FLAGS_25, 0x08, true)?;
                set_byte_bits(host, VAR_OBJECT_FLAGS_22, 0x02, false)?;
                set_byte_bits(host, VAR_PATH_FLAGS, 0x80, false)?;
                set_byte_bits(host, VAR_OBJECT_FLAGS_26, 0x02, false)?;
                set_byte_bits(host, VAR_OBJECT_FLAGS_26, 0x04, false)?;
                return Ok(Some(YieldReason::End));
            }
            HelicopterOn => {
                let flags = host
                    .read_variable_byte(VAR_PATH_FLAGS)
                    .map_err(PathVmError::Host)?;
                host.write_variable_byte(VAR_PATH_FLAGS, flags | PATH_FLAG_HELICOPTER)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfSelectedDistanceLess | IfMotherDistanceLess => {
                let distance = if semantic == IfSelectedDistanceLess {
                    Some(host.selected_distance().map_err(PathVmError::Host)?)
                } else {
                    host.mother_distance().map_err(PathVmError::Host)?
                };
                let Some(distance) = distance else {
                    // Retail exits before inspecting or clearing IFNOT when
                    // there is no mother object.
                    self.advance(command);
                    return Ok(None);
                };
                if self.take_condition(distance < operand_word(command, 1)) {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 3),
                    };
                } else {
                    self.advance(command);
                }
            }
            Goto => {
                self.cursor = PathAddress {
                    offset: operand_word(command, 1),
                };
                return Ok(Some(YieldReason::Goto));
            }
            GotoImmediate => {
                self.cursor = PathAddress {
                    offset: operand_word(command, 1),
                };
            }
            SetObjectBytes0a0b => {
                host.write_variable_byte(0x0A, operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                host.write_variable_byte(0x0B, operand_byte(command, 2))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            PathHold => {
                set_byte_bits(host, VAR_OBJECT_FLAGS_09, 0x08, true)?;
                host.enter_path_hold().map_err(PathVmError::Host)?;
                return Ok(Some(YieldReason::Hold));
            }
            InitAnimation | InitColorAnimation => {
                let offset = if semantic == InitAnimation {
                    0x1CCB
                } else {
                    0x1CCA
                };
                host.write_object_extension_byte(offset, operand_byte(command, 1) | 0x80)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddAnimation | AddColorAnimation => {
                let amount = operand_byte(command, 1);
                let max_frames = operand_byte(command, 2);
                let offset = if semantic == AddAnimation {
                    0x1CCB
                } else {
                    0x1CCA
                };
                let mut frame = host
                    .read_object_extension_byte(offset)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(amount);
                if frame as i8 >= 0 {
                    frame = frame.wrapping_add(max_frames);
                }
                frame &= 0x7F;
                if frame >= max_frames {
                    frame = frame.wrapping_sub(max_frames);
                }
                host.write_object_extension_byte(offset, frame | 0x80)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ChildDead => {
                let dead = host
                    .child_is_dead(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                if dead {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 2),
                    };
                } else {
                    self.advance(command);
                }
            }
            FlagChild => {
                host.flag_child(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ShapeDead => {
                let dead = host.pointed_shape_is_dead().map_err(PathVmError::Host)?;
                if self.take_condition(dead) {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            MessageLiteral => {
                host.start_message(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            RandomGoto => {
                let take = host.random_byte().map_err(PathVmError::Host)? < 0x7F;
                if take {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            IfSameByte => {
                let equal = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?
                    == operand_byte(command, 2);
                if self.take_condition(equal) {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 3),
                    };
                } else {
                    self.advance(command);
                }
            }
            IfSameWord => {
                let equal = host
                    .read_variable_word(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?
                    == operand_word(command, 2);
                if self.take_condition(equal) {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 4),
                    };
                } else {
                    self.advance(command);
                }
            }
            IfBetweenByte => {
                let value = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                let lower_delta = operand_byte(command, 2).wrapping_sub(value) as i8;
                let upper_delta = operand_byte(command, 3).wrapping_sub(value) as i8;
                let between = lower_delta < 0 && upper_delta >= 0;
                if self.take_condition(between) {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 4),
                    };
                } else {
                    self.advance(command);
                }
            }
            IfBetweenWord => {
                let value = host
                    .read_variable_word(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                let lower_delta = operand_word(command, 2).wrapping_sub(value) as i16;
                let upper_delta = operand_word(command, 4).wrapping_sub(value) as i16;
                let between = lower_delta < 0 && upper_delta >= 0;
                if self.take_condition(between) {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 6),
                    };
                } else {
                    self.advance(command);
                }
            }
            SetFlag22Bit08 | ClearFlag22Bit08 => {
                set_byte_bits(host, VAR_OBJECT_FLAGS_22, 0x08, semantic == SetFlag22Bit08)?;
                self.advance(command);
            }
            SpawnChildAlias | SpawnChild => {
                // The shared `$7F:9042` handler has two formats selected by
                // the logical opcode saved in `$1911`. `$0F5` is the compact
                // SF2 record with three signed word offsets; `$033` retains
                // the 17-byte form with three rotation bytes followed by its
                // signed word offsets.
                let (rotation, hit_points, attack_points, offset, child_number) =
                    if semantic == SpawnChild {
                        (
                            [0; 3],
                            operand_byte(command, 5),
                            operand_byte(command, 6),
                            [
                                operand_word(command, 7) as i16,
                                operand_word(command, 9) as i16,
                                operand_word(command, 11) as i16,
                            ],
                            operand_byte(command, 13),
                        )
                    } else {
                        (
                            [
                                operand_byte(command, 5),
                                operand_byte(command, 6),
                                operand_byte(command, 7),
                            ],
                            operand_byte(command, 8),
                            operand_byte(command, 9),
                            [
                                operand_word(command, 10) as i16,
                                operand_word(command, 12) as i16,
                                operand_word(command, 14) as i16,
                            ],
                            operand_byte(command, 16),
                        )
                    };
                host.spawn_child(ChildSpawn {
                    shape: operand_word(command, 1),
                    path: PathAddress {
                        offset: operand_word(command, 3),
                    },
                    rotation,
                    hit_points,
                    attack_points,
                    offset,
                    child_number,
                })
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            FireWeapon => {
                host.fire_weapon().map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfFlag23Bit08 => {
                let flags = host
                    .read_variable_byte(VAR_OBJECT_FLAGS_23)
                    .map_err(PathVmError::Host)?;
                if flags & 0x08 != 0 {
                    host.write_variable_byte(VAR_OBJECT_FLAGS_23, flags & !0x08)
                        .map_err(PathVmError::Host)?;
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            Gosub => {
                host.push_path_value(command.address.offset)
                    .map_err(PathVmError::Host)?;
                self.cursor = PathAddress {
                    offset: operand_word(command, 1),
                };
            }
            Return => {
                let caller = host.pop_path_value().map_err(PathVmError::Host)?;
                let Some(caller) = caller.filter(|caller| *caller != u16::MAX) else {
                    return Ok(Some(YieldReason::Return));
                };
                // GOSUB is one opcode byte followed by a two-byte target.
                self.cursor = PathAddress {
                    offset: caller.wrapping_add(3),
                };
            }
            Next | ImmediateNext => {
                let count = host
                    .pop_path_value()
                    .map_err(PathVmError::Host)?
                    .unwrap_or(0)
                    .wrapping_sub(1);
                if count == 0 {
                    // Discard the saved body address and continue after NEXT.
                    let _ = host.pop_path_value().map_err(PathVmError::Host)?;
                    self.advance(command);
                } else {
                    let body = host
                        .pop_path_value()
                        .map_err(PathVmError::Host)?
                        .unwrap_or(0);
                    host.push_path_value(body).map_err(PathVmError::Host)?;
                    host.push_path_value(count).map_err(PathVmError::Host)?;
                    self.cursor = PathAddress { offset: body };
                    if semantic == Next {
                        return Ok(Some(YieldReason::Next));
                    }
                }
            }
            Break => {
                let _ = host.pop_path_value().map_err(PathVmError::Host)?;
                let _ = host.pop_path_value().map_err(PathVmError::Host)?;
                self.cursor = PathAddress {
                    offset: operand_word(command, 1),
                };
            }
            InvisibleOn | InvisibleOff => {
                let object_flags = host
                    .read_variable_byte(VAR_OBJECT_FLAGS_23)
                    .map_err(PathVmError::Host)?;
                let path_flags = host
                    .read_variable_byte(VAR_PATH_FLAGS)
                    .map_err(PathVmError::Host)?;
                let (object_flags, path_flags) = if semantic == InvisibleOn {
                    (
                        object_flags | OBJECT_FLAG_INVISIBLE,
                        path_flags | PATH_FLAG_INVISIBLE,
                    )
                } else {
                    (
                        object_flags & !OBJECT_FLAG_INVISIBLE,
                        path_flags & !PATH_FLAG_INVISIBLE,
                    )
                };
                host.write_variable_byte(VAR_OBJECT_FLAGS_23, object_flags)
                    .map_err(PathVmError::Host)?;
                host.write_variable_byte(VAR_PATH_FLAGS, path_flags)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ScheduleTrigger | ScheduleAlways | ScheduleRelative | ScheduleTriggered => {
                let prefix = command.prefix_size as usize;
                let (path, delay, trigger) = match semantic {
                    ScheduleAlways => (
                        PathAddress {
                            offset: operand_word(command, 1),
                        },
                        0,
                        0,
                    ),
                    ScheduleRelative => (
                        PathAddress {
                            offset: command
                                .address
                                .offset
                                .wrapping_add(command.raw[prefix + 1] as u16),
                        },
                        operand_byte(command, 2),
                        0,
                    ),
                    ScheduleTrigger => (
                        PathAddress {
                            offset: operand_word(command, 1),
                        },
                        operand_byte(command, 3),
                        0,
                    ),
                    ScheduleTriggered => (
                        PathAddress {
                            offset: operand_word(command, 1),
                        },
                        operand_byte(command, 3),
                        operand_byte(command, 4).wrapping_add(1),
                    ),
                    _ => unreachable!(),
                };
                host.schedule_trigger(PathTrigger {
                    path,
                    delay,
                    trigger,
                })
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            CancelTrigger => {
                host.cancel_trigger(PathAddress {
                    offset: operand_word(command, 1),
                })
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ForceTriggerPath => {
                host.force_trigger_path(PathAddress {
                    offset: operand_word(command, 1),
                })
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            Sprite => {
                host.set_sprite(operand_byte(command, 1), operand_byte(command, 2))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetVariableByteFromByte => {
                let destination = operand_byte(command, 1);
                let source = operand_byte(command, 2);
                let value = host.read_variable_byte(source).map_err(PathVmError::Host)?;
                host.write_variable_byte(destination, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetVariableWordFromWord => {
                let destination = operand_byte(command, 1);
                let source = operand_byte(command, 2);
                let value = host.read_variable_word(source).map_err(PathVmError::Host)?;
                host.write_variable_word(destination, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddVariableByteFromByte | AddVariableByteFromByteAlias => {
                let destination = operand_byte(command, 1);
                let source = operand_byte(command, 2);
                let value = host
                    .read_variable_byte(destination)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(host.read_variable_byte(source).map_err(PathVmError::Host)?);
                host.write_variable_byte(destination, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddVariableWordFromWord => {
                let destination = operand_byte(command, 1);
                let source = operand_byte(command, 2);
                let value = host
                    .read_variable_word(destination)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(host.read_variable_word(source).map_err(PathVmError::Host)?);
                host.write_variable_word(destination, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            NegateByte => {
                let variable = operand_byte(command, 1);
                let value = 0u8.wrapping_sub(
                    host.read_variable_byte(variable)
                        .map_err(PathVmError::Host)?,
                );
                host.write_variable_byte(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddVariableWordFromByte => {
                let destination = operand_byte(command, 1);
                let source = operand_byte(command, 2);
                let delta =
                    host.read_variable_byte(source).map_err(PathVmError::Host)? as i8 as i16 as u16;
                let value = host
                    .read_variable_word(destination)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(delta);
                host.write_variable_word(destination, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            NegateWord => {
                let variable = operand_byte(command, 1);
                let value = 0u16.wrapping_sub(
                    host.read_variable_word(variable)
                        .map_err(PathVmError::Host)?,
                );
                host.write_variable_word(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetRandomByte => {
                let variable = operand_byte(command, 1);
                let value =
                    host.random_byte().map_err(PathVmError::Host)? & operand_byte(command, 2);
                host.write_variable_byte(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetFlag21Bit01 => {
                set_byte_bits(host, VAR_PATH_FLAGS, 0x01, true)?;
                self.advance(command);
            }
            QuickSpawn => {
                host.quick_spawn(
                    operand_word(command, 1),
                    PathAddress {
                        offset: operand_word(command, 3),
                    },
                    operand_byte(command, 5),
                    operand_byte(command, 6),
                )
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            DoQueue => {
                // `$7F:9641` pushes the address after the two-byte DOQUEUE
                // command, followed by the one-byte iteration count.  NEXT
                // and IMMEDIATE_NEXT consume this pair from the object's
                // heap-backed path stack.
                host.push_path_value(fallthrough(command).offset)
                    .map_err(PathVmError::Host)?;
                host.push_path_value(u16::from(operand_byte(command, 1)))
                    .map_err(PathVmError::Host)?;
                host.do_queue(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            DoVariableByte => {
                host.push_path_value(fallthrough(command).offset)
                    .map_err(PathVmError::Host)?;
                let value = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                host.push_path_value(value as u16)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            RemoveChild => {
                host.remove_child(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfZeroByte => {
                let is_zero = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?
                    == 0;
                if is_zero {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 2),
                    };
                } else {
                    self.advance(command);
                }
            }
            IfZeroWord | IfNotZeroByte | IfNotZeroWord => {
                let variable = operand_byte(command, 1);
                let is_zero = if semantic == IfNotZeroByte {
                    host.read_variable_byte(variable)
                        .map_err(PathVmError::Host)?
                        == 0
                } else {
                    host.read_variable_word(variable)
                        .map_err(PathVmError::Host)?
                        == 0
                };
                let take = if semantic == IfZeroWord {
                    is_zero
                } else {
                    !is_zero
                };
                if take {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 2),
                    };
                } else {
                    self.advance(command);
                }
            }
            SetZeroByte => {
                host.write_variable_byte(operand_byte(command, 1), 0)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetZeroWord => {
                host.write_variable_word(operand_byte(command, 1), 0)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IncrementByte => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(1);
                host.write_variable_byte(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IncrementWord => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_word(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(1);
                host.write_variable_word(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            DecrementByte => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_sub(1);
                host.write_variable_byte(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddWorldX | AddWorldY => {
                let variable = if semantic == AddWorldX { 0x0C } else { 0x0E };
                let delta = operand_byte(command, 1) as i8 as i16 as u16;
                let value = host
                    .read_variable_word(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(delta);
                host.write_variable_word(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddSignedByteToWord => {
                let variable = operand_byte(command, 1);
                let delta = operand_byte(command, 2) as i8 as i16 as u16;
                let value = host
                    .read_variable_word(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(delta);
                host.write_variable_word(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            WaitOne => {
                self.advance(command);
                return Ok(Some(YieldReason::WaitOne));
            }
            ImportByteAbsolute => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_external_byte(operand_word(command, 2))
                    .map_err(PathVmError::Host)?;
                host.write_variable_byte(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ImportByteIndexed => {
                let variable = operand_byte(command, 1);
                let address = INDEXED_VARIABLE_TABLE.wrapping_add(operand_byte(command, 2) as u16);
                let value = host
                    .read_external_byte(address)
                    .map_err(PathVmError::Host)?;
                host.write_variable_byte(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ImportWordIndexed => {
                let variable = operand_byte(command, 1);
                let address = INDEXED_VARIABLE_TABLE.wrapping_add(operand_byte(command, 2) as u16);
                let value = host
                    .read_external_word(address)
                    .map_err(PathVmError::Host)?;
                host.write_variable_word(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ImportWordAbsolute => {
                let value = host
                    .read_external_word(operand_word(command, 2))
                    .map_err(PathVmError::Host)?;
                host.write_variable_word(operand_byte(command, 1), value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ExportByteAbsolute => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)?;
                host.write_external_byte(operand_word(command, 2), value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ExportWordAbsolute => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_word(variable)
                    .map_err(PathVmError::Host)?;
                host.write_external_word(operand_word(command, 2), value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ExportByteIndexed => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)?;
                let address = INDEXED_VARIABLE_TABLE.wrapping_add(operand_byte(command, 2) as u16);
                host.write_external_byte(address, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ExportWordIndexed => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_word(variable)
                    .map_err(PathVmError::Host)?;
                let address = INDEXED_VARIABLE_TABLE.wrapping_add(operand_byte(command, 2) as u16);
                host.write_external_word(address, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddRotationX | AddRotationY | AddRotationZ => {
                let variable = match semantic {
                    AddRotationX => 0x12,
                    AddRotationY => 0x14,
                    AddRotationZ => 0x16,
                    _ => unreachable!(),
                };
                let value = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(operand_byte(command, 1));
                host.write_variable_byte(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AchaseByte => {
                let target = operand_byte(command, 1);
                let variable = operand_byte(command, 2);
                let current = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)?;
                host.write_variable_byte(variable, achase_byte(current, target))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ChaseVariableByte => {
                let destination = operand_byte(command, 1);
                let target = host
                    .read_variable_byte(operand_byte(command, 2))
                    .map_err(PathVmError::Host)?;
                let current = host
                    .read_variable_byte(destination)
                    .map_err(PathVmError::Host)?;
                host.write_variable_byte(destination, chase_variable_byte(current, target))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ChaseVariableWord => {
                let destination = operand_byte(command, 1);
                let target = host
                    .read_variable_word(operand_byte(command, 2))
                    .map_err(PathVmError::Host)?;
                let current = host
                    .read_variable_word(destination)
                    .map_err(PathVmError::Host)?;
                host.write_variable_word(destination, chase_variable_word(current, target))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfNot => {
                self.invert_next_condition = true;
                self.advance(command);
            }
            Inline65816 => match command.address.offset {
                0x2059 | 0x9122 | 0x919F | 0x91DA | 0xE690 | 0xF9A1 => {
                    host.perform_path_operation(Sf2PathOperation::LinkSpawnedObjectToCurrent)
                        .map_err(PathVmError::Host)?;
                    self.cursor = command.successors[0];
                }
                0x8D54 => {
                    set_byte_bits(host, VAR_OBJECT_FLAGS_24, 0x04, false)?;
                    self.cursor = PathAddress { offset: 0x8D61 };
                }
                0x8D62 => {
                    set_byte_bits(
                        host,
                        VAR_OBJECT_FLAGS_24,
                        INLINE_OBJECT_FLAG_24_BIT_04,
                        true,
                    )?;
                    self.cursor = command.successors[0];
                }
                0x4B4D => {
                    set_byte_bits(
                        host,
                        VAR_OBJECT_FLAGS_25,
                        INLINE_OBJECT_FLAG_25_BIT_02,
                        true,
                    )?;
                    self.cursor = command.successors[0];
                }
                0x9E71 => {
                    host.perform_path_operation(Sf2PathOperation::PreserveCurrentObjectForParent)
                        .map_err(PathVmError::Host)?;
                    self.cursor = command.successors[0];
                }
                0x9808 => {
                    let phase = host
                        .read_object_extension_byte(INLINE_COLOR_PHASE_EXTENSION)
                        .map_err(PathVmError::Host)?;
                    host.write_object_extension_byte(INLINE_COLOR_PHASE_EXTENSION, phase ^ 1)
                        .map_err(PathVmError::Host)?;
                    self.cursor = command.successors[0];
                }
                0xAB2A => {
                    let index = host
                        .read_external_word(INLINE_DISPATCH_INDEX)
                        .map_err(PathVmError::Host)?;
                    if index & 1 != 0 || index as usize / 2 >= INLINE_DISPATCH_PATHS.len() {
                        return Err(PathVmError::InvalidInlineDispatchIndex {
                            address: command.address,
                            index,
                        });
                    }
                    self.cursor = PathAddress {
                        offset: INLINE_DISPATCH_PATHS[index as usize / 2],
                    };
                }
                0xB8C5 => {
                    host.write_external_byte(INLINE_GLOBAL_CONTROL_MODE, 2)
                        .map_err(PathVmError::Host)?;
                    host.write_external_byte(INLINE_GLOBAL_CONTROL_PHASE, 0)
                        .map_err(PathVmError::Host)?;
                    self.cursor = command.successors[0];
                }
                0xB8E4 => {
                    for variable in [VAR_WORLD_X, VAR_WORLD_Y, VAR_WORLD_Z] {
                        let value = host
                            .read_variable_word(variable)
                            .map_err(PathVmError::Host)?;
                        host.write_variable_word(variable, value.wrapping_add(i16::MIN as u16))
                            .map_err(PathVmError::Host)?;
                    }
                    for (variable, value) in [
                        (VAR_ROTATION_X, 26),
                        (VAR_ROTATION_Y, 64),
                        (VAR_ROTATION_Z, 0),
                    ] {
                        host.write_variable_byte(variable, value)
                            .map_err(PathVmError::Host)?;
                    }
                    self.cursor = command.successors[0];
                }
                0xB0CB => {
                    host.copy_selected_world_position()
                        .map_err(PathVmError::Host)?;
                    self.cursor = PathAddress { offset: 0xB0E6 };
                }
                0xB116 => {
                    host.refresh_collision_target().map_err(PathVmError::Host)?;
                    self.cursor = PathAddress { offset: 0xB121 };
                }
                0xB129 => {
                    set_byte_bits(host, VAR_OBJECT_FLAGS_25, 0x08, true)?;
                    self.cursor = PathAddress { offset: 0xB136 };
                }
                0xCFF8 | 0xD098 | 0xD253 | 0xE845 => {
                    let mask = match command.address.offset {
                        0xCFF8 => 0x04,
                        0xE845 => 0x01,
                        _ => 0x08,
                    };
                    let value = host
                        .read_external_byte(INLINE_GLOBAL_EVENT_FLAGS)
                        .map_err(PathVmError::Host)?
                        | mask;
                    host.write_external_byte(INLINE_GLOBAL_EVENT_FLAGS, value)
                        .map_err(PathVmError::Host)?;
                    self.cursor = command.successors[0];
                }
                0xF313 => {
                    set_byte_bits(
                        host,
                        VAR_OBJECT_FLAGS_25,
                        INLINE_OBJECT_FLAG_25_BIT_02,
                        true,
                    )?;
                    self.cursor = command.successors[0];
                }
                0xF6E6 => {
                    host.perform_path_operation(Sf2PathOperation::ScaleHorizontalMotion)
                        .map_err(PathVmError::Host)?;
                    self.cursor = command.successors[0];
                }
                0xF348 => {
                    let locked = host
                        .evaluate_path_condition(Sf2PathCondition::PlayerOneFlag25Bit20)
                        .map_err(PathVmError::Host)?;
                    if !locked {
                        host.write_object_extension_byte(INLINE_PLAYER_RELATIVE_PHASE_EXTENSION, 1)
                            .map_err(PathVmError::Host)?;
                    }
                    self.cursor = command.successors[0];
                }
                0xF668 => {
                    let index = host
                        .read_external_word(INLINE_DISPATCH_INDEX)
                        .map_err(PathVmError::Host)?;
                    if index & 1 != 0 || index as usize / 2 >= LATE_INLINE_DISPATCH_PATHS.len() {
                        return Err(PathVmError::InvalidInlineDispatchIndex {
                            address: command.address,
                            index,
                        });
                    }
                    self.cursor = PathAddress {
                        offset: LATE_INLINE_DISPATCH_PATHS[index as usize / 2],
                    };
                }
                site @ (INLINE_INITIALIZE_LAUNCHED_EXTERNAL_OBJECT
                | INLINE_EASE_FIXED_PLAYER_YAW
                | INLINE_CONFIGURE_RANDOMIZED_OBJECT_MOTION
                | INLINE_INITIALIZE_SPAWNED_OBJECT_MOTION
                | INLINE_CHASE_YAW_OPPOSITE_FIXED_PLAYER
                | INLINE_ACCUMULATE_PLAYER_AUXILIARY_MOTION
                | INLINE_INITIALIZE_PLAYER_AUXILIARY_CHARGE
                | INLINE_UPDATE_CONDITIONAL_OBJECT_PHASE
                | INLINE_SPAWN_PLAYER_LINKED_OBJECT
                | INLINE_LINK_SELECTED_OBJECT_TRANSFORM
                | INLINE_REFRESH_PLAYER_AUXILIARY_MODE
                | INLINE_ENABLE_PLAYER_AUXILIARY_CONTROL
                | INLINE_INITIALIZE_PLAYER_RELATIVE_MOTION
                | INLINE_CHASE_CURRENT_RELATIVE_OFFSETS
                | INLINE_ADVANCE_CURRENT_RELATIVE_OFFSETS
                | INLINE_CHASE_CURRENT_RELATIVE_POSE
                | INLINE_RESET_PLAYER_AUXILIARY_TARGET
                | INLINE_APPLY_CURRENT_HEALTH_DECAY
                | INLINE_SEPARATE_YAW_TARGETS
                | INLINE_ADVANCE_VERTICAL_OSCILLATION) => {
                    let operation = match site {
                        INLINE_INITIALIZE_LAUNCHED_EXTERNAL_OBJECT => {
                            Sf2PathOperation::InitializeLaunchedExternalObject
                        }
                        INLINE_EASE_FIXED_PLAYER_YAW => Sf2PathOperation::EaseFixedPlayerYaw,
                        INLINE_CONFIGURE_RANDOMIZED_OBJECT_MOTION => {
                            Sf2PathOperation::ConfigureRandomizedObjectMotion
                        }
                        INLINE_INITIALIZE_SPAWNED_OBJECT_MOTION => {
                            Sf2PathOperation::InitializeSpawnedObjectMotion
                        }
                        INLINE_CHASE_YAW_OPPOSITE_FIXED_PLAYER => {
                            Sf2PathOperation::ChaseYawOppositeFixedPlayer
                        }
                        INLINE_ACCUMULATE_PLAYER_AUXILIARY_MOTION => {
                            Sf2PathOperation::AccumulatePlayerAuxiliaryMotion
                        }
                        INLINE_INITIALIZE_PLAYER_AUXILIARY_CHARGE => {
                            Sf2PathOperation::InitializePlayerAuxiliaryCharge
                        }
                        INLINE_UPDATE_CONDITIONAL_OBJECT_PHASE => {
                            Sf2PathOperation::UpdateConditionalObjectPhase
                        }
                        INLINE_SPAWN_PLAYER_LINKED_OBJECT => {
                            Sf2PathOperation::SpawnPlayerLinkedObject
                        }
                        INLINE_LINK_SELECTED_OBJECT_TRANSFORM => {
                            Sf2PathOperation::LinkSelectedObjectTransform
                        }
                        INLINE_REFRESH_PLAYER_AUXILIARY_MODE => {
                            Sf2PathOperation::RefreshPlayerAuxiliaryMode
                        }
                        INLINE_ENABLE_PLAYER_AUXILIARY_CONTROL => {
                            Sf2PathOperation::EnablePlayerAuxiliaryControl
                        }
                        INLINE_INITIALIZE_PLAYER_RELATIVE_MOTION => {
                            Sf2PathOperation::InitializePlayerRelativeMotion
                        }
                        INLINE_CHASE_CURRENT_RELATIVE_OFFSETS => {
                            Sf2PathOperation::ChaseCurrentRelativeOffsets
                        }
                        INLINE_ADVANCE_CURRENT_RELATIVE_OFFSETS => {
                            Sf2PathOperation::AdvanceCurrentRelativeOffsets
                        }
                        INLINE_CHASE_CURRENT_RELATIVE_POSE => {
                            Sf2PathOperation::ChaseCurrentRelativePose
                        }
                        INLINE_RESET_PLAYER_AUXILIARY_TARGET => {
                            Sf2PathOperation::ResetPlayerAuxiliaryTarget
                        }
                        INLINE_APPLY_CURRENT_HEALTH_DECAY => {
                            Sf2PathOperation::ApplyCurrentHealthDecay
                        }
                        INLINE_SEPARATE_YAW_TARGETS => Sf2PathOperation::SeparateYawTargets,
                        INLINE_ADVANCE_VERTICAL_OSCILLATION => {
                            Sf2PathOperation::AdvanceVerticalOscillation
                        }
                        _ => unreachable!(),
                    };
                    host.perform_path_operation(operation)
                        .map_err(PathVmError::Host)?;
                    if site == INLINE_REFRESH_PLAYER_AUXILIARY_MODE {
                        let mode = host
                            .read_object_extension_byte(INLINE_PLAYER_RELATIVE_PHASE_EXTENSION)
                            .map_err(PathVmError::Host)?;
                        self.cursor = if mode & 0xFE == 0 {
                            command.successors[1]
                        } else {
                            command.successors[0]
                        };
                    } else {
                        self.cursor = command.successors[0];
                    }
                }
                _ => {
                    return Err(PathVmError::InvalidInlineAddress(command.address));
                }
            },
            SetFlag20Bit08 | ClearFlag20Bit08 => {
                set_byte_bits(host, VAR_OBJECT_FLAGS_20, 0x08, semantic == SetFlag20Bit08)?;
                self.advance(command);
            }
            DivideByteByTwo => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)? as i8;
                host.write_variable_byte(variable, (value / 2) as u8)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IndexByteBanked | IndexWordBanked => {
                let base =
                    operand_word(command, 1) as u32 | (operand_byte(command, 3) as u32) << 16;
                let index = host
                    .read_variable_byte(operand_byte(command, 4))
                    .map_err(PathVmError::Host)? as u32;
                let destination = operand_byte(command, 5);
                if semantic == IndexByteBanked {
                    let value = host
                        .read_external_long_byte(base.wrapping_add(index))
                        .map_err(PathVmError::Host)?;
                    host.write_variable_byte(destination, value)
                        .map_err(PathVmError::Host)?;
                } else {
                    let value = host
                        .read_external_long_word(base.wrapping_add(index.wrapping_mul(2)))
                        .map_err(PathVmError::Host)?;
                    host.write_variable_word(destination, value)
                        .map_err(PathVmError::Host)?;
                }
                self.advance(command);
            }
            PushByte => {
                let value = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                host.push_path_value(value as u16)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            PushWord => {
                let value = host
                    .read_variable_word(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                host.push_path_value(value).map_err(PathVmError::Host)?;
                self.advance(command);
            }
            PullByte => {
                let value = host
                    .pop_path_value()
                    .map_err(PathVmError::Host)?
                    .unwrap_or(0);
                host.write_variable_byte(operand_byte(command, 1), value as u8)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            PullWord => {
                let value = host
                    .pop_path_value()
                    .map_err(PathVmError::Host)?
                    .unwrap_or(0);
                host.write_variable_word(operand_byte(command, 1), value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfSelectedWithinRange => {
                if host
                    .selected_within_range(operand_word(command, 1))
                    .map_err(PathVmError::Host)?
                {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 3),
                    };
                } else {
                    self.advance(command);
                }
            }
            IfSelectedWithinYawArc => {
                let radius = operand_byte(command, 1);
                let bearing = host
                    .selected_bearing_plus_yaw()
                    .map_err(PathVmError::Host)?;
                if radius.wrapping_add(bearing) < radius.wrapping_mul(2) {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 2),
                    };
                } else {
                    self.advance(command);
                }
            }
            RotateAroundSelectedYaw => {
                host.rotate_around_selected_yaw(operand_byte(command, 1) as i8)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            RotateAroundSelectedPitch => {
                host.rotate_around_selected_pitch(operand_byte(command, 1) as i8)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            BecomeChildLiteralOrGoto | BecomeChildVariableOrGoto | BecomeMotherOrGoto => {
                let (transition, target_offset) = match semantic {
                    BecomeChildLiteralOrGoto => {
                        (ContextTransition::BecomeChild(operand_byte(command, 1)), 2)
                    }
                    BecomeChildVariableOrGoto => (
                        ContextTransition::BecomeChild(
                            host.read_variable_byte(operand_byte(command, 1))
                                .map_err(PathVmError::Host)?,
                        ),
                        2,
                    ),
                    BecomeMotherOrGoto => (ContextTransition::BecomeMother, 1),
                    _ => unreachable!(),
                };
                let resume_at = fallthrough(command);
                if host
                    .try_transition_context(transition, resume_at)
                    .map_err(PathVmError::Host)?
                {
                    self.cursor = resume_at;
                } else {
                    self.cursor = PathAddress {
                        offset: operand_word(command, target_offset),
                    };
                }
            }
            BecomeLinked => {
                let resume_at = fallthrough(command);
                host.transition_context(ContextTransition::BecomeMother, resume_at)
                    .map_err(PathVmError::Host)?;
                self.cursor = resume_at;
            }
            Unbecome | Become => {
                let resume_at = fallthrough(command);
                let transition = match semantic {
                    Unbecome => ContextTransition::Unbecome,
                    Become => ContextTransition::Become,
                    _ => unreachable!(),
                };
                host.transition_context(transition, resume_at)
                    .map_err(PathVmError::Host)?;
                self.cursor = resume_at;
            }
            SelectPlayerAndClearFlag24Bit80 => {
                let player = host.read_external_word(0x12C3).map_err(PathVmError::Host)?;
                host.write_external_word(0xCF1F, player)
                    .map_err(PathVmError::Host)?;
                set_byte_bits(host, VAR_OBJECT_FLAGS_24, 0x80, false)?;
                self.advance(command);
            }
            IfSelectedSlotClass1 | IfSelectedSlotClass2 | IfSelectedSlotClass3 => {
                let wanted = match semantic {
                    IfSelectedSlotClass1 => 0x10,
                    IfSelectedSlotClass2 => 0x20,
                    IfSelectedSlotClass3 => 0x30,
                    _ => unreachable!(),
                };
                if host.selected_slot_class().map_err(PathVmError::Host)? & 0xF0 == wanted {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            NoOp0B8 => {
                self.advance(command);
            }
            SetFlag26Bit08 => {
                set_byte_bits(host, VAR_OBJECT_FLAGS_26, 0x08, true)?;
                self.advance(command);
            }
            UpdatePlayerTargetAndFlagLinked | UpdatePlayerTargetFlag08 => {
                let update = if semantic == UpdatePlayerTargetAndFlagLinked {
                    PlayerTargetUpdate::FlagLinked
                } else {
                    PlayerTargetUpdate::Flag08
                };
                host.update_player_target(update)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetObject1cef => {
                host.write_object_extension_byte(0x1CEF, 1)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ClearFlag26Bit10 | SetFlag26Bit10 => {
                set_byte_bits(host, VAR_OBJECT_FLAGS_26, 0x10, semantic == SetFlag26Bit10)?;
                self.advance(command);
            }
            SetVariableBit | ClearVariableBit => {
                let index = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                let mask = variable_bit_mask(command.address, index)?;
                let destination = operand_byte(command, 2);
                let value = host
                    .read_variable_word(destination)
                    .map_err(PathVmError::Host)?;
                let value = if semantic == SetVariableBit {
                    value | mask
                } else {
                    value & !mask
                };
                host.write_variable_word(destination, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfVariableBitSet => {
                let index = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                let mask = variable_bit_mask(command.address, index)?;
                let value = host
                    .read_variable_word(operand_byte(command, 2))
                    .map_err(PathVmError::Host)?;
                if value & mask != 0 {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 3),
                    };
                } else {
                    self.advance(command);
                }
            }
            MessageVariable => {
                let message = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                host.start_message(message).map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfExternalD77dBitsSet | IfExternalD77dBitsClear => {
                let present = host.read_external_word(0xD77D).map_err(PathVmError::Host)?
                    & operand_word(command, 1)
                    != 0;
                let take = if semantic == IfExternalD77dBitsSet {
                    present
                } else {
                    !present
                };
                if take {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 3),
                    };
                } else {
                    self.advance(command);
                }
            }
            OrExternalD77d => {
                let value = host.read_external_word(0xD77D).map_err(PathVmError::Host)?
                    | operand_word(command, 1);
                host.write_external_word(0xD77D, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ClearExternalD77dBits => {
                let value = host.read_external_word(0xD77D).map_err(PathVmError::Host)?
                    & !operand_word(command, 1);
                host.write_external_word(0xD77D, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ClearExternalD77d => {
                host.write_external_word(0xD77D, 0)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IncrementExternalByte => {
                let address = operand_word(command, 1);
                let value = host
                    .read_external_byte(address)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(1);
                host.write_external_byte(address, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IncrementExternalWord => {
                let address = operand_word(command, 1);
                let value = host
                    .read_external_word(address)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(1);
                host.write_external_word(address, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            DecrementExternalByte => {
                let address = operand_word(command, 1);
                let value = host
                    .read_external_byte(address)
                    .map_err(PathVmError::Host)?
                    .wrapping_sub(1);
                host.write_external_byte(address, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddExternalByteToVariableByte => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(
                        host.read_external_byte(operand_word(command, 2))
                            .map_err(PathVmError::Host)?,
                    );
                host.write_variable_byte(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddVariableByteToExternalByte => {
                let address = operand_word(command, 1);
                let value = host
                    .read_external_byte(address)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(
                        host.read_variable_byte(operand_byte(command, 3))
                            .map_err(PathVmError::Host)?,
                    );
                host.write_external_byte(address, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            CopySelectedWorldPosition => {
                host.copy_selected_world_position()
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfVariableBytesLess | IfVariableWordsLess | IfVariableBytesSame => {
                let first = operand_byte(command, 1);
                let second = operand_byte(command, 2);
                let take = match semantic {
                    IfVariableBytesLess => {
                        let first = host.read_variable_byte(first).map_err(PathVmError::Host)?;
                        let second = host.read_variable_byte(second).map_err(PathVmError::Host)?;
                        (second.wrapping_sub(first) as i8) < 0
                    }
                    IfVariableWordsLess => {
                        let first = host.read_variable_word(first).map_err(PathVmError::Host)?;
                        let second = host.read_variable_word(second).map_err(PathVmError::Host)?;
                        (second.wrapping_sub(first) as i16) < 0
                    }
                    IfVariableBytesSame => {
                        let equal = host.read_variable_byte(first).map_err(PathVmError::Host)?
                            == host.read_variable_byte(second).map_err(PathVmError::Host)?;
                        self.take_condition(equal)
                    }
                    _ => unreachable!(),
                };
                if take {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 3),
                    };
                } else {
                    self.advance(command);
                }
            }
            AddExternalD77fByte2 => {
                let value = host
                    .read_external_byte(INLINE_DISPATCH_INDEX)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(2);
                host.write_external_byte(INLINE_DISPATCH_INDEX, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            Trail => {
                host.set_trail(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            MaskFlag31 => {
                let value = host
                    .read_variable_byte(VAR_OBJECT_FLAGS_31)
                    .map_err(PathVmError::Host)?
                    & operand_byte(command, 1);
                host.write_variable_byte(VAR_OBJECT_FLAGS_31, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            WriteObject1ccb80 => {
                host.write_object_extension_byte(0x1CCB, 0x80)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            StoreExternalByte => {
                host.write_external_byte(operand_word(command, 1), operand_byte(command, 3))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            StoreExternalWord => {
                host.write_external_word(operand_word(command, 1), operand_word(command, 3))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            QueueSelectedMarkerDirect | QueueSelectedMarkerClass1 | QueueSelectedMarkerClass2 => {
                let class = match semantic {
                    QueueSelectedMarkerDirect => SelectedMarkerClass::Direct,
                    QueueSelectedMarkerClass1 => SelectedMarkerClass::Class1,
                    QueueSelectedMarkerClass2 => SelectedMarkerClass::Class2,
                    _ => unreachable!(),
                };
                host.queue_selected_marker(operand_byte(command, 1), class)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddIndexedSignedByteAndAdvanceFrame => {
                add_indexed_byte_and_advance_frame(host, command, true)?;
                self.advance(command);
            }
            WriteObject1ccc => {
                host.write_object_extension_byte(0x1CCC, operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            FaceMother => {
                host.face_mother().map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetFlag21Bit20AndClearObject1cc1 => {
                set_byte_bits(host, VAR_PATH_FLAGS, 0x20, true)?;
                host.write_object_extension_word(0x1CC1, 0)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetFlag26Bit40AndHold => {
                set_byte_bits(host, VAR_OBJECT_FLAGS_26, 0x40, true)?;
                // `$7F:BC80` leaves the cursor on this opcode and enters the
                // common movement yield.  It does not install the `$9DDE`
                // strategy used by the unrelated `$019` path hold.
                return Ok(Some(YieldReason::Hold));
            }
            IfSelectedRelativeYawBetween => {
                let lower = operand_byte(command, 1);
                let width = operand_byte(command, 2).wrapping_sub(lower);
                let angle = host.selected_relative_yaw().map_err(PathVmError::Host)?;
                if angle.wrapping_sub(lower) < width {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 3),
                    };
                } else {
                    self.advance(command);
                }
            }
            SetFlag24Bit08 => {
                set_byte_bits(host, VAR_OBJECT_FLAGS_24, 0x08, true)?;
                self.advance(command);
            }
            SetFlag09Bit01 => {
                set_byte_bits(host, VAR_OBJECT_FLAGS_09, 0x01, true)?;
                self.advance(command);
            }
            IfSelectedAuxBit40 => {
                if host.selected_aux_flags().map_err(PathVmError::Host)? & 0x40 != 0 {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            OrSelectedAuxFlags => {
                host.or_selected_aux_flags(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetSelectedSlotLowNibble4 => {
                host.set_selected_slot_low_nibble_4()
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AllocateAuxiliaryType0b => {
                host.allocate_auxiliary_type_0b(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            StoreExternalD77fByte => {
                host.write_external_byte(INLINE_DISPATCH_INDEX, operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SpawnLinkedObjectEffects => {
                host.spawn_linked_object_effects()
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfExternalC4BitsSet => {
                if host.read_external_byte(0x00C4).map_err(PathVmError::Host)?
                    & operand_byte(command, 1)
                    != 0
                {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 2),
                    };
                } else {
                    self.advance(command);
                }
            }
            SetExternal1d74Bit40 => {
                let value = host.read_external_byte(0x1D74).map_err(PathVmError::Host)? | 0x40;
                host.write_external_byte(0x1D74, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetExternal1dddBit80 => {
                let value = host.read_external_byte(0x1DDD).map_err(PathVmError::Host)? | 0x80;
                host.write_external_byte(0x1DDD, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetFlag21Bit08 => {
                set_byte_bits(host, VAR_PATH_FLAGS, 0x08, true)?;
                self.advance(command);
            }
            FaceSelectedSmooth
            | FaceLinkedSmooth
            | ExplodeObject
            | FlagLinkedObject
            | FlagMother
            | UnlinkSelf
            | RefreshSelectedRelativeTransform
            | SelectSelfAndClearRelativeTransform
            | RefreshLinkedRotationDeltas
            | UpdatePilotAuxState
            | FaceSelectedImmediate
            | ChasePlayerTowardObject
            | SnapPlayerToObject
            | CopySelectedSlotWorldPosition
            | PositionExternalObjectAndFaceSelected
            | CopySelectedAuxRotation
            | ApplyFormationOffset
            | FreeObjectAuxiliaryAndResetD742
            | ResetSelectedAuxiliaryMotion
            | IncrementLinkedAuxiliaryCounter
            | DecrementLinkedAuxiliaryCounter
            | SelectCurrentAsRotationTarget
            | ClearSelectedAuxiliaryFlag01
            | SetSelectedSlotLowNibble1
            | ChaseObjectPositionTowardCurrent
            | CopySelectedRotation
            | CopyPositionToObject
            | CopyRotationToObjectFixed
            | PopPathStackPair
            | ConfigurePlayerAuxiliary
            | SetObjectRotationTowardTarget
            | ChaseObjectRotationTowardTarget
            | RefreshOwnedPlayerAuxiliaryOrigin
            | CaptureSelectedAuxiliaryMotion
            | ClearObjectRelativeReference
            | PreserveCurrentPathContinuation
            | IncrementSelectedAuxiliaryStage => {
                let operation = match semantic {
                    FaceSelectedSmooth => Sf2PathOperation::FaceSelectedSmooth,
                    FaceLinkedSmooth => Sf2PathOperation::FaceLinkedSmooth,
                    ExplodeObject => Sf2PathOperation::ExplodeObject,
                    FlagLinkedObject => Sf2PathOperation::FlagLinkedObject,
                    FlagMother => Sf2PathOperation::FlagLinkedObject,
                    UnlinkSelf => Sf2PathOperation::UnlinkSelf,
                    RefreshSelectedRelativeTransform => {
                        Sf2PathOperation::RefreshSelectedRelativeTransform
                    }
                    SelectSelfAndClearRelativeTransform => {
                        Sf2PathOperation::SelectSelfAndClearRelativeTransform
                    }
                    RefreshLinkedRotationDeltas => Sf2PathOperation::RefreshLinkedRotationDeltas,
                    UpdatePilotAuxState => Sf2PathOperation::UpdatePilotAuxState,
                    FaceSelectedImmediate => Sf2PathOperation::FaceSelectedImmediate,
                    ChasePlayerTowardObject => Sf2PathOperation::ChasePlayerTowardObject,
                    SnapPlayerToObject => Sf2PathOperation::SnapPlayerToObject,
                    CopySelectedSlotWorldPosition => {
                        Sf2PathOperation::CopySelectedSlotWorldPosition
                    }
                    PositionExternalObjectAndFaceSelected => {
                        Sf2PathOperation::PositionExternalObjectAndFaceSelected
                    }
                    CopySelectedAuxRotation => Sf2PathOperation::CopySelectedAuxRotation,
                    ApplyFormationOffset => Sf2PathOperation::ApplyFormationOffset,
                    FreeObjectAuxiliaryAndResetD742 => {
                        Sf2PathOperation::FreeObjectAuxiliaryAndResetD742
                    }
                    ResetSelectedAuxiliaryMotion => Sf2PathOperation::ResetSelectedAuxiliaryMotion,
                    IncrementLinkedAuxiliaryCounter => {
                        Sf2PathOperation::IncrementLinkedAuxiliaryCounter
                    }
                    DecrementLinkedAuxiliaryCounter => {
                        Sf2PathOperation::DecrementLinkedAuxiliaryCounter
                    }
                    SelectCurrentAsRotationTarget => {
                        Sf2PathOperation::SelectCurrentAsRotationTarget
                    }
                    ClearSelectedAuxiliaryFlag01 => Sf2PathOperation::ClearSelectedAuxiliaryFlag01,
                    SetSelectedSlotLowNibble1 => Sf2PathOperation::SetSelectedSlotLowNibble1,
                    ChaseObjectPositionTowardCurrent => {
                        Sf2PathOperation::ChaseObjectPositionTowardCurrent(operand_word(command, 1))
                    }
                    CopySelectedRotation => Sf2PathOperation::CopySelectedRotation,
                    CopyPositionToObject => {
                        Sf2PathOperation::CopyPositionToObject(operand_word(command, 1))
                    }
                    CopyRotationToObjectFixed => {
                        Sf2PathOperation::CopyRotationToObjectFixed(operand_word(command, 1))
                    }
                    PopPathStackPair => Sf2PathOperation::PopPathStackPair,
                    ConfigurePlayerAuxiliary => {
                        Sf2PathOperation::ConfigurePlayerAuxiliary(operand_word(command, 1))
                    }
                    SetObjectRotationTowardTarget => {
                        Sf2PathOperation::SetObjectRotationTowardTarget {
                            object: operand_word(command, 1),
                            shift: operand_byte(command, 3) & 7,
                        }
                    }
                    ChaseObjectRotationTowardTarget => {
                        Sf2PathOperation::ChaseObjectRotationTowardTarget {
                            object: operand_word(command, 1),
                            shift: operand_byte(command, 3) & 7,
                        }
                    }
                    RefreshOwnedPlayerAuxiliaryOrigin => {
                        Sf2PathOperation::RefreshOwnedPlayerAuxiliaryOrigin
                    }
                    CaptureSelectedAuxiliaryMotion => {
                        Sf2PathOperation::CaptureSelectedAuxiliaryMotion
                    }
                    ClearObjectRelativeReference => Sf2PathOperation::ClearObjectRelativeReference,
                    PreserveCurrentPathContinuation => {
                        Sf2PathOperation::PreserveCurrentPathContinuation
                    }
                    IncrementSelectedAuxiliaryStage => {
                        Sf2PathOperation::IncrementSelectedAuxiliaryStage
                    }
                    _ => unreachable!(),
                };
                host.perform_path_operation(operation)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AdvanceSelectedAuxiliaryOrGotoWhenSettled => {
                let settled = host
                    .advance_selected_auxiliary_progress(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                if settled {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 2),
                    };
                } else {
                    self.advance(command);
                }
            }
            InstallStrategyAndStop => {
                host.perform_path_operation(Sf2PathOperation::InstallStrategyAndStop {
                    strategy: operand_word(command, 1),
                    state: operand_byte(command, 3),
                })
                .map_err(PathVmError::Host)?;
                self.cursor = PathAddress { offset: 0 };
                return Ok(Some(YieldReason::End));
            }
            BranchOnContactClass => {
                self.cursor = match host.classify_path_contact().map_err(PathVmError::Host)? {
                    None => fallthrough(command),
                    Some(PathContactClass::NoObject) => PathAddress {
                        offset: operand_word(command, 1),
                    },
                    Some(PathContactClass::AuxiliaryType0b) => PathAddress {
                        offset: operand_word(command, 3),
                    },
                    Some(PathContactClass::OtherObject) => PathAddress {
                        offset: operand_word(command, 5),
                    },
                };
            }
            IfSelectedAuxiliaryMapCellOccupied
            | IfSelectedAuxiliaryFlag04Clear
            | IfSelectedAuxiliaryStateMatchesGlobal => {
                let condition = match semantic {
                    IfSelectedAuxiliaryMapCellOccupied => {
                        Sf2PathCondition::SelectedAuxiliaryMapCellOccupied
                    }
                    IfSelectedAuxiliaryFlag04Clear => {
                        Sf2PathCondition::SelectedAuxiliaryFlag04Clear
                    }
                    IfSelectedAuxiliaryStateMatchesGlobal => {
                        Sf2PathCondition::SelectedAuxiliaryStateMatchesGlobal
                    }
                    _ => unreachable!(),
                };
                let matched = host
                    .evaluate_path_condition(condition)
                    .map_err(PathVmError::Host)?;
                if self.take_condition(matched) {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            HelicopterOff => {
                set_byte_bits(host, VAR_PATH_FLAGS, PATH_FLAG_HELICOPTER, false)?;
                self.advance(command);
            }
            IfHitGround => {
                let condition = host
                    .evaluate_path_condition(Sf2PathCondition::HitGround {
                        offset: operand_word(command, 1),
                    })
                    .map_err(PathVmError::Host)?;
                if self.take_condition(condition) {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 3),
                    };
                } else {
                    self.advance(command);
                }
            }
            IfProjectedSelectedPointNegative
            | IfSelectedLeftOfObject
            | IfProjectedSelectedForwardPointNegative
            | IfSelectedBelowObject
            | IfSelectedOrCurrentAuxState => {
                let condition = match semantic {
                    IfProjectedSelectedPointNegative => {
                        Sf2PathCondition::ProjectedSelectedPointNegative
                    }
                    IfSelectedLeftOfObject => Sf2PathCondition::SelectedLeftOfObject,
                    IfProjectedSelectedForwardPointNegative => {
                        Sf2PathCondition::ProjectedSelectedForwardPointNegative
                    }
                    IfSelectedBelowObject => Sf2PathCondition::SelectedBelowObject,
                    IfSelectedOrCurrentAuxState => Sf2PathCondition::SelectedOrCurrentAuxState,
                    _ => unreachable!(),
                };
                if host
                    .evaluate_path_condition(condition)
                    .map_err(PathVmError::Host)?
                {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            IfExternal1e0dBit01 => {
                if host.read_external_byte(0x1E0D).map_err(PathVmError::Host)? & 0x01 != 0 {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            SpawnObject => {
                host.perform_path_operation(Sf2PathOperation::SpawnObject(ObjectSpawn {
                    shape: operand_word(command, 1),
                    path: PathAddress {
                        offset: operand_word(command, 3),
                    },
                    rotation: [
                        operand_byte(command, 5),
                        operand_byte(command, 6),
                        operand_byte(command, 7),
                    ],
                    hit_points: operand_byte(command, 8),
                    attack_points: operand_byte(command, 9),
                    offset: [
                        operand_word(command, 10) as i16,
                        operand_word(command, 12) as i16,
                        operand_word(command, 14) as i16,
                    ],
                }))
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetVariableByteFromWord => {
                let value = host
                    .read_variable_word(operand_byte(command, 2))
                    .map_err(PathVmError::Host)? as u8;
                host.write_variable_byte(operand_byte(command, 1), value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetVariableWordFromByte => {
                let value = host
                    .read_variable_byte(operand_byte(command, 2))
                    .map_err(PathVmError::Host)? as i8 as i16 as u16;
                host.write_variable_word(operand_byte(command, 1), value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetRandomWord => {
                let low = host.random_byte().map_err(PathVmError::Host)? as u16;
                let high = host.random_byte().map_err(PathVmError::Host)? as u16;
                let value = (low | high << 8) & operand_word(command, 1);
                host.write_variable_word(operand_byte(command, 3), value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfHitFlag => {
                let index = operand_byte(command, 3);
                let mask = variable_bit_mask(command.address, index)? as u8;
                let flags = host.read_variable_byte(0x38).map_err(PathVmError::Host)?;
                if flags & mask != 0 {
                    host.write_variable_byte(0x38, flags & !mask)
                        .map_err(PathVmError::Host)?;
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            ClearFlag21Bit01 => {
                set_byte_bits(host, VAR_PATH_FLAGS, 0x01, false)?;
                self.advance(command);
            }
            UnlinkChild => {
                host.perform_path_operation(Sf2PathOperation::UnlinkChild(operand_byte(
                    command, 1,
                )))
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AccumulateObject1cde => {
                host.perform_path_operation(Sf2PathOperation::AccumulateObject1cde(operand_byte(
                    command, 1,
                )))
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AchaseWord => {
                let target = operand_word(command, 1);
                let variable = operand_byte(command, 3);
                let current = host
                    .read_variable_word(variable)
                    .map_err(PathVmError::Host)?;
                host.write_variable_word(variable, achase_word(current, target))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            WaitAchaseByte => {
                let target = operand_byte(command, 1);
                let variable = operand_byte(command, 2);
                let current = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)?;
                if current == target {
                    self.advance(command);
                } else {
                    host.write_variable_byte(variable, achase_byte(current, target))
                        .map_err(PathVmError::Host)?;
                    return Ok(Some(YieldReason::Wait));
                }
            }
            DivideWordByTwo => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_word(variable)
                    .map_err(PathVmError::Host)? as i16;
                host.write_variable_word(variable, (value / 2) as u16)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddIndexedByteAndAdvanceFrame => {
                add_indexed_byte_and_advance_frame(host, command, false)?;
                self.advance(command);
            }
            SaturatingAddSelectedAuxWord => {
                host.perform_path_operation(Sf2PathOperation::SaturatingAddSelectedAuxWord(
                    operand_word(command, 1),
                ))
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            RotateAroundSelectedYawVariable => {
                let angle = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)? as i8;
                host.rotate_around_selected_yaw(angle)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            InitializePlayerAuxWord => {
                host.perform_path_operation(Sf2PathOperation::InitializePlayerAuxWord(
                    operand_word(command, 1),
                ))
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetFlag20Bit02 => {
                set_byte_bits(host, VAR_OBJECT_FLAGS_20, 0x02, true)?;
                self.advance(command);
            }
            EnablePlayerAuxMode | DisablePlayerAuxMode => {
                host.perform_path_operation(Sf2PathOperation::SetPlayerAuxMode(
                    semantic == EnablePlayerAuxMode,
                ))
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ClearExternalCf33VariableBit => {
                let index = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                let mask = variable_bit_mask(command.address, index)?;
                let value = host.read_external_word(0xCF33).map_err(PathVmError::Host)? & !mask;
                host.write_external_word(0xCF33, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddExternalWordToVariableWord => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_word(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(
                        host.read_external_word(operand_word(command, 2))
                            .map_err(PathVmError::Host)?,
                    );
                host.write_variable_word(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddVariableWordToExternalWord => {
                let address = operand_word(command, 1);
                let value = host
                    .read_external_word(address)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(
                        host.read_variable_word(operand_byte(command, 3))
                            .map_err(PathVmError::Host)?,
                    );
                host.write_external_word(address, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfVariableWordsSame => {
                let equal = host
                    .read_variable_word(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?
                    == host
                        .read_variable_word(operand_byte(command, 2))
                        .map_err(PathVmError::Host)?;
                if self.take_condition(equal) {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 3),
                    };
                } else {
                    self.advance(command);
                }
            }
            Loop => {
                let target = operand_byte(command, 1);
                let counter = host
                    .read_variable_byte(VAR_LOOP_COUNTER)
                    .map_err(PathVmError::Host)?;
                if counter != target {
                    host.write_variable_byte(VAR_LOOP_COUNTER, counter.wrapping_add(1))
                        .map_err(PathVmError::Host)?;
                    self.cursor = PathAddress {
                        offset: operand_word(command, 2),
                    };
                    return Ok(Some(YieldReason::Goto));
                }
                host.write_variable_byte(VAR_LOOP_COUNTER, 0)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            QueueFixedMarker1400 => {
                host.perform_path_operation(Sf2PathOperation::QueueFixedMarker1400(operand_byte(
                    command, 1,
                )))
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            QueueFixedMarker0320 => {
                host.perform_path_operation(Sf2PathOperation::QueueFixedMarker0320(operand_byte(
                    command, 1,
                )))
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            QueueSelectedMarkerPair => {
                host.perform_path_operation(Sf2PathOperation::QueueSelectedMarkerPair {
                    first: operand_byte(command, 1),
                    second: operand_byte(command, 2),
                })
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            RotateAroundLinkedPitch => {
                let angle = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)? as i8;
                host.perform_path_operation(Sf2PathOperation::RotateAroundLinkedPitch(angle))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            WaitVariable => {
                let target = host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                let counter = host
                    .read_variable_byte(VAR_WAIT_COUNTER)
                    .map_err(PathVmError::Host)?;
                if counter == target {
                    host.write_variable_byte(VAR_WAIT_COUNTER, 0)
                        .map_err(PathVmError::Host)?;
                    self.advance(command);
                } else {
                    host.write_variable_byte(VAR_WAIT_COUNTER, counter.wrapping_add(1))
                        .map_err(PathVmError::Host)?;
                    return Ok(Some(YieldReason::Wait));
                }
            }
            ClearFlag09Bit01 => {
                set_byte_bits(host, VAR_OBJECT_FLAGS_09, 0x01, false)?;
                self.advance(command);
            }
            IfVariableEqualsExternal1dd4 => {
                if host
                    .read_variable_byte(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?
                    == host.read_external_byte(0x1DD4).map_err(PathVmError::Host)?
                {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 2),
                    };
                } else {
                    self.advance(command);
                }
            }
            CallExternalStrategy1e14 => {
                let strategy = host.read_external_byte(0x1E14).map_err(PathVmError::Host)?;
                host.perform_path_operation(Sf2PathOperation::CallExternalStrategy(strategy))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ShiftByteRight => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)?
                    >> 1;
                host.write_variable_byte(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            ConfigurePilotAuxModeA | ConfigurePilotAuxModeB => {
                let value = operand_word(command, 1);
                let operation = if semantic == ConfigurePilotAuxModeA {
                    Sf2PathOperation::ConfigurePilotAuxModeA(value)
                } else {
                    Sf2PathOperation::ConfigurePilotAuxModeB(value)
                };
                host.perform_path_operation(operation)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetFlag26Bit80 | SetFlag26Bit20 => {
                let mask = if semantic == SetFlag26Bit80 {
                    0x80
                } else {
                    0x20
                };
                set_byte_bits(host, VAR_OBJECT_FLAGS_26, mask, true)?;
                self.advance(command);
            }
            AllocateAuxiliaryType0d => {
                host.allocate_auxiliary_type_0d(operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            OrFlag31 => {
                let value = host
                    .read_variable_byte(VAR_OBJECT_FLAGS_31)
                    .map_err(PathVmError::Host)?
                    | operand_byte(command, 1);
                host.write_variable_byte(VAR_OBJECT_FLAGS_31, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfExternal1dddBit80 => {
                if host.read_external_byte(0x1DDD).map_err(PathVmError::Host)? & 0x80 != 0 {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            DecrementWord => {
                let variable = operand_byte(command, 1);
                let value = host
                    .read_variable_word(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_sub(1);
                host.write_variable_word(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddWorldZ => {
                let value = host
                    .read_variable_word(0x10)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(operand_byte(command, 1) as i8 as i16 as u16);
                host.write_variable_word(0x10, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            RotateLocalOffsetYaw | RotateLocalOffsetPitch => {
                let angle = operand_byte(command, 1) as i8;
                let operation = if semantic == RotateLocalOffsetYaw {
                    Sf2PathOperation::RotateLocalOffsetYaw(angle)
                } else {
                    Sf2PathOperation::RotateLocalOffsetPitch(angle)
                };
                host.perform_path_operation(operation)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SetObject1ce3 => {
                host.write_object_extension_byte(0x1CE3, operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            PositionRelativeToLinkedLiteral | PositionRelativeToLinkedVariable => {
                let value = if semantic == PositionRelativeToLinkedLiteral {
                    operand_byte(command, 1)
                } else {
                    host.read_variable_byte(operand_byte(command, 1))
                        .map_err(PathVmError::Host)?
                };
                host.perform_path_operation(Sf2PathOperation::PositionRelativeToLinked(
                    value as i8,
                ))
                .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            CopyWorldPositionTo1e01 => {
                for (variable, address) in [(0x0C, 0x1E01), (0x0E, 0x1E03), (0x10, 0x1E05)] {
                    let value = host
                        .read_variable_word(variable)
                        .map_err(PathVmError::Host)?;
                    host.write_external_word(address, value)
                        .map_err(PathVmError::Host)?;
                }
                self.advance(command);
            }
            SetExternal1d72 => {
                host.write_external_byte(0x1D72, operand_byte(command, 1))
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            RestoreMapCursorPair => {
                let bank = host.read_external_byte(0x1D77).map_err(PathVmError::Host)?;
                let offset = host.read_external_word(0x1D78).map_err(PathVmError::Host)?;
                host.write_external_byte(0x192E, bank)
                    .map_err(PathVmError::Host)?;
                host.write_external_word(0x1657, offset)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            IfExternal1d72NotEqual => {
                if operand_byte(command, 1)
                    != host.read_external_byte(0x1D72).map_err(PathVmError::Host)?
                {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 2),
                    };
                } else {
                    self.advance(command);
                }
            }
            IfExternal1d72Equal => {
                if operand_byte(command, 1)
                    == host.read_external_byte(0x1D72).map_err(PathVmError::Host)?
                {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 2),
                    };
                } else {
                    self.advance(command);
                }
            }
            IfCurrentAtOrAboveCollisionTarget => {
                host.refresh_collision_target().map_err(PathVmError::Host)?;
                let current_y = host.read_variable_word(0x0E).map_err(PathVmError::Host)?;
                let target_y = host.read_external_word(0x0008).map_err(PathVmError::Host)?;
                if current_y.wrapping_sub(target_y) as i16 >= 0 {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            IfExternalD743EqualsOne => {
                if host
                    .read_external_byte(0xD743)
                    .map_err(PathVmError::Host)?
                    .wrapping_sub(1)
                    == 0
                {
                    self.cursor = PathAddress {
                        offset: operand_word(command, 1),
                    };
                } else {
                    self.advance(command);
                }
            }
            AddCenteredRandomByte => {
                let variable = operand_byte(command, 1);
                let mask = operand_byte(command, 2);
                let delta =
                    (host.random_byte().map_err(PathVmError::Host)? & mask).wrapping_sub(mask >> 1);
                let value = host
                    .read_variable_byte(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(delta);
                host.write_variable_byte(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            AddCenteredRandomWord => {
                let variable = operand_byte(command, 1);
                let mask = operand_word(command, 2);
                let high = host.random_byte().map_err(PathVmError::Host)? as u16;
                let low = host.random_byte().map_err(PathVmError::Host)? as u16;
                let delta = ((high << 8 | low) & mask).wrapping_sub(mask >> 1);
                let value = host
                    .read_variable_word(variable)
                    .map_err(PathVmError::Host)?
                    .wrapping_add(delta);
                host.write_variable_word(variable, value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
            SwapVariableWords => {
                let first = operand_byte(command, 1);
                let second = operand_byte(command, 2);
                let first_value = host.read_variable_word(first).map_err(PathVmError::Host)?;
                let second_value = host.read_variable_word(second).map_err(PathVmError::Host)?;
                host.write_variable_word(first, second_value)
                    .map_err(PathVmError::Host)?;
                host.write_variable_word(second, first_value)
                    .map_err(PathVmError::Host)?;
                self.advance(command);
            }
        }
        Ok(None)
    }

    fn take_condition(&mut self, value: bool) -> bool {
        if self.invert_next_condition {
            self.invert_next_condition = false;
            !value
        } else {
            value
        }
    }

    fn advance(&mut self, command: &PathCommand) {
        self.cursor = fallthrough(command);
    }
}

fn set_byte_bits<H: Sf2PathHost>(
    host: &mut H,
    variable: u8,
    mask: u8,
    enabled: bool,
) -> Result<(), PathVmError<H::Error>> {
    let value = host
        .read_variable_byte(variable)
        .map_err(PathVmError::Host)?;
    let value = if enabled { value | mask } else { value & !mask };
    host.write_variable_byte(variable, value)
        .map_err(PathVmError::Host)
}

fn variable_bit_mask<E>(address: PathAddress, index: u8) -> Result<u16, PathVmError<E>> {
    if !(1..=16).contains(&index) {
        return Err(PathVmError::InvalidBitIndex { address, index });
    }
    Ok(1u16 << (index - 1))
}

fn chase_variable_byte(current: u8, target: u8) -> u8 {
    if current == target {
        return current;
    }
    let mut delta = target.wrapping_sub(current) as i8;
    if (0..8).contains(&delta) {
        delta = 8;
    } else if (-8..0).contains(&delta) {
        delta = -8;
    }
    current.wrapping_add((delta / 8) as u8)
}

fn chase_variable_word(current: u16, target: u16) -> u16 {
    if current == target {
        return current;
    }
    let mut delta = target.wrapping_sub(current) as i16;
    if (0..8).contains(&delta) {
        delta = 8;
    } else if (-8..0).contains(&delta) {
        delta = -8;
    }
    current.wrapping_add((delta / 8) as u16)
}

fn achase_byte(current: u8, target: u8) -> u8 {
    if current == target {
        return current;
    }
    let mut delta = target.wrapping_sub(current) as i8 as i16;
    if (0..8).contains(&delta) {
        delta = 8;
    } else if (-7..0).contains(&delta) {
        delta = -8;
    }
    for _ in 0..3 {
        delta /= 2;
    }
    current.wrapping_add(delta as u8)
}

fn achase_word(current: u16, target: u16) -> u16 {
    if current == target {
        return current;
    }
    let mut delta = target.wrapping_sub(current) as i16;
    if (0..8).contains(&delta) {
        delta = 8;
    } else if (-8..0).contains(&delta) {
        delta = -8;
    }
    for _ in 0..3 {
        delta /= 2;
    }
    current.wrapping_add(delta as u16)
}

fn add_indexed_byte_and_advance_frame<H: Sf2PathHost>(
    host: &mut H,
    command: &PathCommand,
    signed: bool,
) -> Result<(), PathVmError<H::Error>> {
    let base = operand_word(command, 1) as u32 | (operand_byte(command, 3) as u32) << 16;
    let frame_variable = operand_byte(command, 4);
    let frame = host
        .read_variable_byte(frame_variable)
        .map_err(PathVmError::Host)?;
    let byte = host
        .read_external_long_byte(base.wrapping_add(frame as u32))
        .map_err(PathVmError::Host)?;
    let delta = if signed {
        byte as i8 as i16 as u16
    } else {
        byte as u16
    };
    let destination = operand_byte(command, 5);
    let value = host
        .read_variable_word(destination)
        .map_err(PathVmError::Host)?
        .wrapping_add(delta);
    host.write_variable_word(destination, value)
        .map_err(PathVmError::Host)?;

    let mut next_frame = frame.wrapping_add(1);
    if operand_byte(command, 6).wrapping_sub(1) < next_frame {
        next_frame = 0;
    }
    host.write_variable_byte(frame_variable, next_frame)
        .map_err(PathVmError::Host)
}

pub fn command_at(address: PathAddress) -> Option<&'static PathCommand> {
    PATH_COMMANDS
        .binary_search_by_key(&address, |command| command.address)
        .ok()
        .map(|index| &PATH_COMMANDS[index])
}

pub fn handler_for(opcode: u16) -> Option<&'static PathHandler> {
    PATH_HANDLERS
        .binary_search_by_key(&opcode, |handler| handler.opcode)
        .ok()
        .map(|index| &PATH_HANDLERS[index])
}

fn operand_byte(command: &PathCommand, operand: usize) -> u8 {
    command.raw[command.prefix_size as usize + operand]
}

fn operand_word(command: &PathCommand, operand: usize) -> u16 {
    u16::from_le_bytes([
        operand_byte(command, operand),
        operand_byte(command, operand + 1),
    ])
}

fn fallthrough(command: &PathCommand) -> PathAddress {
    PathAddress {
        offset: command.address.offset.wrapping_add(command.raw_len as u16),
    }
}
