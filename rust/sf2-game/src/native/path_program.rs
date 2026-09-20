//! Execution of decoded native path catalogs. Source addresses and operand
//! encodings belong to offline lowering, never to this dispatcher. A catalog
//! contains only implemented statements; missing entries are errors, not NOPs.
//!
//! Common entry (`$7F:7E53`) selects the actor's player once. Immediate advances
//! (`$7F:7E75`) retain selection. Movement and callback services remain explicit
//! scheduler boundaries; dispatch does not advance time or tick other actors.

use super::path_commands::{BranchCommand, ControlCommand, ControlStep, MotionCommand};
use super::path_conditions::{Predicate, SpatialCondition};
use super::path_fields::{ByteOperand, Mutation, WordOperand};
use super::path_runtime::{PathRuntime, PathRuntimeError};
use super::{Object, ObjectId, ObjectStore, PathCursor, RandomState};

/// Shared world inputs, borrowed rather than duplicated per actor or path.
/// The caller owns clock advancement and random state across every service.
pub struct PathWorld<'a> {
    pub scene: ScenePathInputs,
    pub scenery_distance: Option<&'a mut SceneryDistanceState>,
    pub targeting_upgrade: Option<&'a mut super::path_target::TargetingUpgradeState>,
    pub shield_recovery: Option<&'a mut super::player_hit_control::ShieldRecoveryRequest>,
    /// Whole shared action-gate byte (1D72), not a narrowed protection flag.
    pub action_gate: Option<u8>,
    /// Shared environmental reference plane (1E0F), in world-Y coordinates.
    pub environment_plane_height: Option<i16>,
    pub projectile_trigger: Option<&'a mut ProjectileTrigger>,
    pub primary_pitch_recoil: Option<&'a mut super::path_player_control::PitchRecoil>,
    pub linked_effect_activity: Option<&'a mut super::path_protection::LinkedEffectActivity>,
    pub protection: Option<super::path_protection::PathProtection<'a>>,
    pub audio: Option<super::path_sound::PathAudio<'a>>,
    pub radio: Option<super::path_radio::PathRadio<'a>>,
    pub campaign: Option<CampaignPathInputs>,
    pub guidance: Option<&'a mut GuidanceHistory>,
    pub pickup_history: Option<&'a mut PickupHistory>,
    pub control_style: Option<super::FlightControlStyle>,
    /// Fresh selected-player exemption (auxiliary map flag bit 80).
    pub selected_occupancy_exempt: Option<bool>,
    pub occupancy: Option<&'a super::world_occupancy::WorldOccupancy>,
    pub surface_mode: Option<super::collision_surface::SurfaceMode>,
    /// Primary player identity, independent of the current selected slot.
    pub primary_player: Option<ObjectId>,
    pub selected: Option<ObjectId>,
    /// Fixed player actors, distinct from the live selected/primary pointers.
    pub fixed_players: [Option<ObjectId>; 2],
    /// Fresh primary auxiliary mode and retained displacement, required only
    /// by the one-time primary-motion inheritance action.
    pub primary_motion: Option<PrimaryMotionInput>,
    /// Published player-service snapshot, distinct from fresh primary inputs.
    pub published_motion: Option<super::path_motion::PublishedPlayerMotion>,
    /// Shared active-pilot threshold; it can change independently of selection.
    pub active_charge_threshold: Option<u8>,
    pub selected_charge: Option<super::path_charge::SelectedChargeInput>,
    pub primary_control: Option<super::path_player_control::PrimaryControl<'a>>,
    pub primary_target: Option<super::path_target::PrimaryTarget<'a>>,
    /// Live active campaign-node flags. Source node loading updates only the
    /// low byte, while path imports and node writeback retain the whole word.
    pub active_node_flags: Option<u16>,
    pub countdown: Option<&'a mut super::path_countdown::PathCountdown>,
    /// Shared selected-player auxiliary state. Commands and branches borrow
    /// the same live record; a missing record faults only when needed.
    pub selected_auxiliary: Option<&'a mut SelectedAuxiliaryState>,
    /// Fresh path-selected equipment, not the published active-pilot snapshot.
    pub selected_equipment: Option<&'a mut super::path_equipment::SelectedEquipment>,
    pub selected_score: Option<&'a mut super::path_score::PlayerScore>,
    /// Fresh initializer-mode observations. Missing inputs fault only if
    /// this invocation reaches a spawn; they are not guessed from pause state.
    pub spawn_defaults: Option<super::ObjectSpawnDefaults>,
    pub random: &'a mut RandomState,
    /// Shared strategy/animation clock (C4), also read by authored clock gates.
    pub animation_clock: u8,
}

/// Shared scene selectors. The player configuration also selects the player
/// shape record (`$06:85E7`); encounter location comes from the campaign node
/// (`$04:B1FC`). Keep full bytes, not the special-case predicates they drive.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScenePathInputs {
    pub player_configuration: Option<u8>,
    pub encounter_location: Option<u8>,
    /// Active player's published weapon level ($1DD4), copied from its
    /// per-player weapon record at $06:9CE1. This is not a fresh lookup of
    /// the path-selected actor; pilot exchange updates the published byte.
    pub active_weapon_level: Option<u8>,
}

/// Shared scenery proximity mask ($D78C). Authored paths select which bits
/// to replace using their script parameter; importing/exporting preserves
/// the full byte. This record belongs to the scene, not to any one actor.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SceneryDistanceState {
    pub near_mask: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneryDistanceCommand {
    CopyTo(super::path_fields::ByteField),
    Assign(ByteOperand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneByte {
    PlayerConfiguration,
    EncounterLocation,
    ActiveWeaponLevel,
}

impl SceneByte {
    fn read(self, input: ScenePathInputs) -> Option<u8> {
        match self {
            Self::PlayerConfiguration => input.player_configuration,
            Self::EncounterLocation => input.encounter_location,
            Self::ActiveWeaponLevel => input.active_weapon_level,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedAuxiliaryState {
    pub mode: u8,
    pub action_flags: u8,
}

/// Shared authored guidance history ($D792). Paths copy and replace the
/// complete word; individual paths decide which already-shown bits to test.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuidanceHistory {
    pub flags: u16,
}

/// Shared collected-pickup mask ($D78E). The pickup family tests its authored
/// identity before showing itself, then sets that bit after collection.
/// Source initialization clears the word; path import/export replaces the
/// complete word and is not an implicit atomic bit-update service.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PickupHistory {
    pub collected_mask: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupHistoryCommand {
    CopyTo(super::path_fields::WordField),
    Assign(WordOperand),
}

/// Shared trigger for the F48B/F4CA projectile family (1E59). Launch clears
/// it; either the input service or a projectile contact can set it. Preserve
/// the full byte when an authored path imports it into its attack variable.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProjectileTrigger {
    pub activation: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileTriggerCommand {
    CopyTo(super::path_fields::ByteField),
    Assign(ByteOperand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidanceCommand {
    CopyTo(super::path_fields::WordField),
    Assign(WordOperand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedAuxiliaryCommand {
    SetModeLowNibbleOne,
    SetModeLowNibbleFour,
    ClearActionBit01,
}

impl SelectedAuxiliaryCommand {
    /// Selected-slot handlers at $7F:B081, $7F:B04D, and $7F:B77A.
    /// The mode class (high nibble) is independent of the low-nibble state.
    fn apply(self, state: &mut SelectedAuxiliaryState) {
        const MODE_CLASS_MASK: u8 = 0xF0;
        const LOW_MODE_ONE: u8 = 1;
        const LOW_MODE_FOUR: u8 = 4;
        const ACTION_BIT_01: u8 = 0x01;
        match self {
            Self::SetModeLowNibbleOne => state.mode = (state.mode & MODE_CLASS_MASK) | LOW_MODE_ONE,
            Self::SetModeLowNibbleFour => {
                state.mode = (state.mode & MODE_CLASS_MASK) | LOW_MODE_FOUR
            }
            Self::ClearActionBit01 => state.action_flags &= !ACTION_BIT_01,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimaryMotionInput {
    pub auxiliary_mode: u8,
    pub displacement: super::Vector3,
}

/// Fresh campaign observations for authored paths. Encounter variant is
/// selected by `$04:C331`; the later map entry can replace it with four
/// (`$05:FC5D`) before the delayed announcement samples it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignPathInputs {
    pub difficulty: super::Difficulty,
    pub encounter_variant: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampaignByte {
    Difficulty,
    EncounterVariant,
}

impl CampaignByte {
    fn read(self, input: CampaignPathInputs) -> u8 {
        match self {
            Self::EncounterVariant => input.encounter_variant,
            Self::Difficulty => match input.difficulty {
                super::Difficulty::Normal => 0,
                super::Difficulty::Hard => 1,
                super::Difficulty::Expert => 2,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedAuxiliaryCondition {
    Continuation,
    ActionBit40,
    ActionBit04Clear,
    ModeClass(super::path_conditions::AuxiliaryModeClass),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbitCenter {
    Selected,
    LocalOrigin,
}

impl SelectedAuxiliaryCondition {
    fn sample(self, input: SelectedAuxiliaryState) -> Predicate {
        match self {
            Self::Continuation => Predicate::SelectedAuxiliaryContinuation {
                mode: input.mode,
                action_flags: input.action_flags,
            },
            Self::ActionBit40 => Predicate::AnyByteBitsSet {
                value: input.action_flags,
                mask: 0x40,
            },
            Self::ActionBit04Clear => Predicate::ZeroByte(input.action_flags & 0x04),
            Self::ModeClass(class) => Predicate::SelectedAuxiliaryModeClass {
                mode: input.mode,
                class,
            },
        }
    }
}

/// Retained expressions are sampled from the live owner on every execution.
/// In particular, an immediate loop must not retain the first iteration's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorCondition {
    EqualShape(super::ShapeId),
    EqualByte(ByteOperand, ByteOperand),
    EqualWord(WordOperand, WordOperand),
    BetweenByte {
        value: ByteOperand,
        lower: ByteOperand,
        upper: ByteOperand,
    },
    BetweenWord {
        value: WordOperand,
        lower: WordOperand,
        upper: WordOperand,
    },
    NonzeroByte(ByteOperand),
    NonzeroWord(WordOperand),
    ZeroByte(ByteOperand),
    ZeroWord(WordOperand),
    SecondByteLess(ByteOperand, ByteOperand),
    SecondWordLess(WordOperand, WordOperand),
    AnyByteBitsSet(ByteOperand, ByteOperand),
    AnyWordBitsSet(WordOperand, WordOperand),
}

impl ActorCondition {
    fn sample(self, actor: &Object) -> Predicate {
        match self {
            Self::EqualShape(shape) => Predicate::EqualWord {
                value: actor.base.shape.catalog_index() as u16,
                expected: shape.catalog_index() as u16,
            },
            Self::EqualByte(a, b) => Predicate::EqualByte {
                value: a.read(actor),
                expected: b.read(actor),
            },
            Self::EqualWord(a, b) => Predicate::EqualWord {
                value: a.read(actor),
                expected: b.read(actor),
            },
            Self::BetweenByte {
                value,
                lower,
                upper,
            } => Predicate::BetweenByte {
                value: value.read(actor),
                lower: lower.read(actor),
                upper: upper.read(actor),
            },
            Self::BetweenWord {
                value,
                lower,
                upper,
            } => Predicate::BetweenWord {
                value: value.read(actor),
                lower: lower.read(actor),
                upper: upper.read(actor),
            },
            Self::NonzeroByte(value) => Predicate::NonzeroByte(value.read(actor)),
            Self::NonzeroWord(value) => Predicate::NonzeroWord(value.read(actor)),
            Self::ZeroByte(value) => Predicate::ZeroByte(value.read(actor)),
            Self::ZeroWord(value) => Predicate::ZeroWord(value.read(actor)),
            Self::SecondByteLess(a, b) => Predicate::SecondByteLess {
                first: a.read(actor),
                second: b.read(actor),
            },
            Self::SecondWordLess(a, b) => Predicate::SecondWordLess {
                first: a.read(actor),
                second: b.read(actor),
            },
            Self::AnyByteBitsSet(value, mask) => Predicate::AnyByteBitsSet {
                value: value.read(actor),
                mask: mask.read(actor),
            },
            Self::AnyWordBitsSet(value, mask) => Predicate::AnyWordBitsSet {
                value: value.read(actor),
                mask: mask.read(actor),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Statement {
    /// The operand is a literal byte and the source branch bypasses IFNOT.
    /// Keep all eight bits even though normal weapon levels are one to three.
    ActiveWeaponLevelEquals {
        expected: u8,
        taken: PathCursor,
        next: PathCursor,
    },
    /// Source ShapeDead tests only attachment absence, not health or slot
    /// liveness. Like the source direct branch, this leaves IFNOT intact.
    AttachmentAbsent {
        taken: PathCursor,
        next: PathCursor,
    },
    TargetingUpgradeOwned {
        taken: PathCursor,
        next: PathCursor,
    },
    AcquireTargetingUpgrade {
        next: PathCursor,
    },
    SceneryDistance {
        command: SceneryDistanceCommand,
        next: PathCursor,
    },
    ClockBitsSet {
        mask: u8,
        taken: PathCursor,
        next: PathCursor,
    },
    RequestShieldRecovery {
        amount: ByteOperand,
        next: PathCursor,
    },
    AccumulateShieldRecovery {
        amount: ByteOperand,
        next: PathCursor,
    },
    ImportActionGate {
        destination: super::path_fields::ByteField,
        next: PathCursor,
    },
    ImportSceneByte {
        source: SceneByte,
        destination: super::path_fields::ByteField,
        next: PathCursor,
    },
    ImportEnvironmentPlaneHeight {
        destination: super::path_fields::WordField,
        next: PathCursor,
    },
    ProjectileTrigger {
        command: ProjectileTriggerCommand,
        next: PathCursor,
    },
    InitializePrimaryPitchRecoil {
        amount: i16,
        next: PathCursor,
    },
    LinkPrimaryCollisionExclusion {
        next: PathCursor,
    },
    LinkedEffectActivity {
        command: super::path_protection::ActivityCommand,
        next: PathCursor,
    },
    UpdateProtectionEffect {
        ordinary_return: PathCursor,
        flicker: PathCursor,
    },
    StackValue {
        command: super::path_commands::StackValueCommand,
        next: PathCursor,
    },
    Appearance {
        command: super::path_appearance::AppearanceCommand,
        next: PathCursor,
    },
    RunWhenPaused {
        enabled: bool,
        next: PathCursor,
    },
    /// Named source inline action: latch phase low byte to one if the primary
    /// player's view-side filter is enabled; otherwise leave it unchanged.
    LatchPrimaryViewFilter {
        next: PathCursor,
    },
    InheritPrimaryHorizontalMotion {
        next: PathCursor,
    },
    ImportPlayerMotion {
        axis: super::path_fields::Axis,
        destination: super::path_fields::WordField,
        next: PathCursor,
    },
    ImportPlayerPosition {
        axis: super::path_fields::Axis,
        destination: super::path_fields::WordField,
        next: PathCursor,
    },
    ImportPlayerMotionByte {
        axis: super::path_fields::Axis,
        part: super::path_fields::BytePart,
        destination: super::path_fields::ByteField,
        next: PathCursor,
    },
    ImportChargeThreshold {
        destination: super::path_fields::ByteField,
        next: PathCursor,
    },
    ImportActiveNodeFlags {
        destination: super::path_fields::WordField,
        next: PathCursor,
    },
    RefreshSelectedChargeAttachment {
        next: PathCursor,
    },
    AttachedEffectMotion {
        command: super::path_steering::AttachedEffectMotion,
        next: PathCursor,
    },
    PlayerControl {
        command: super::path_player_control::PlayerControlCommand,
        next: PathCursor,
    },
    Sound {
        cue: super::path_sound::AuthoredCue,
        next: PathCursor,
    },
    SpatialLoop {
        sound: Option<super::SpatialLoop>,
        next: PathCursor,
    },
    Countdown {
        command: super::path_countdown::CountdownCommand,
        next: PathCursor,
    },
    MarkerSound {
        id: u8,
        mode: super::path_sound::MarkerCueMode,
        next: PathCursor,
    },
    SpawnChild {
        kind: super::ObjectKind,
        parameters: super::path_spawn::ChildSpawn,
        next: PathCursor,
    },
    SpawnIndependent {
        kind: super::ObjectKind,
        parameters: super::path_spawn::IndependentSpawn,
        next: PathCursor,
    },
    Relationship {
        command: super::path_relationships::RelationshipCommand,
        next: PathCursor,
    },
    ConsiderPrimaryTarget {
        next: PathCursor,
    },
    ChildMissing {
        number: u8,
        taken: PathCursor,
        next: PathCursor,
    },
    CopySelectedTransform {
        command: super::path_relationships::SelectedTransformCommand,
        next: PathCursor,
    },
    SelectedAuxiliaryBranch {
        condition: SelectedAuxiliaryCondition,
        taken: PathCursor,
        next: PathCursor,
    },
    SelectedAuxiliary {
        command: SelectedAuxiliaryCommand,
        next: PathCursor,
    },
    CollectSelectedConsumables {
        amount: u8,
        already_full: PathCursor,
        next: PathCursor,
    },
    UpgradeSelectedWeapon {
        next: PathCursor,
    },
    AwardSelectedScore {
        points: u16,
        next: PathCursor,
    },
    Message {
        number: ByteOperand,
        next: PathCursor,
    },
    ImportCampaignByte {
        source: CampaignByte,
        destination: super::path_fields::ByteField,
        next: PathCursor,
    },
    Guidance {
        command: GuidanceCommand,
        next: PathCursor,
    },
    PickupHistory {
        command: PickupHistoryCommand,
        next: PathCursor,
    },
    ImportControlStyle {
        destination: super::path_fields::ByteField,
        next: PathCursor,
    },
    OccupiedCell {
        taken: PathCursor,
        next: PathCursor,
    },
    AtOrAboveSurface {
        taken: PathCursor,
        next: PathCursor,
    },
    ImportSurfaceMode {
        destination: super::path_fields::ByteField,
        next: PathCursor,
    },
    Random {
        mutation: super::path_random::RandomMutation,
        next: PathCursor,
    },
    RandomBranch {
        taken: PathCursor,
        next: PathCursor,
    },
    DisableCollision {
        next: PathCursor,
    },
    Animation {
        command: super::path_appearance::AnimationCommand,
        next: PathCursor,
    },
    Sprite {
        color: u8,
        size: u8,
        next: PathCursor,
    },
    /// Control statements whose operands are literal or semantic cursors.
    Control(ControlCommand),
    Branch(BranchCommand),
    Motion {
        command: MotionCommand,
        next: PathCursor,
    },
    Facing {
        command: super::path_steering::FacingCommand,
        next: PathCursor,
    },
    FaceSelectedOffset {
        offset: super::path_steering::AimOffset,
        next: PathCursor,
    },
    YawOrbit {
        center: OrbitCenter,
        angle: ByteOperand,
        next: PathCursor,
    },
    Radius {
        command: super::path_steering::RadiusCommand,
        next: PathCursor,
    },
    Contact {
        command: super::path_contact::ContactCommand,
        next: PathCursor,
    },
    Mutate {
        mutation: Mutation,
        next: PathCursor,
    },
    Wait {
        duration: ByteOperand,
        next: PathCursor,
    },
    WaitChase {
        field: super::path_fields::ByteField,
        target: ByteOperand,
        next: PathCursor,
    },
    Repeat {
        count: ByteOperand,
        target: PathCursor,
        next: PathCursor,
    },
    BeginLoop {
        iterations: WordOperand,
        next: PathCursor,
    },
    Compare {
        condition: ActorCondition,
        taken: PathCursor,
        next: PathCursor,
    },
    Spatial {
        condition: SpatialCondition,
        taken: PathCursor,
        next: PathCursor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    MissingTargetingUpgrade,
    MissingSceneryDistance,
    MissingSceneByte(SceneByte),
    MissingShieldRecovery,
    MissingActionGate,
    MissingEnvironmentPlaneHeight,
    MissingProjectileTrigger,
    MissingPrimaryPitchRecoil,
    MissingLinkedEffectActivity,
    MissingProtection,
    Protection(super::path_protection::ProtectionError),
    MissingPrimaryPlayer,
    MissingPrimaryMotion,
    MissingPublishedMotion,
    MissingChargeThreshold,
    MissingSelectedCharge,
    MissingPrimaryControl,
    MissingPrimaryTarget,
    MissingActiveNodeFlags,
    MissingAudio,
    MissingRadio,
    MissingCampaign,
    MissingGuidance,
    MissingPickupHistory,
    MissingControlStyle,
    MissingOccupancyExemption,
    MissingOccupancy,
    MissingSurfaceMode,
    SurfaceQuery(super::collision_surface::SurfaceQueryError),
    MissingSoundMarkers,
    MissingCountdown,
    Spawn(super::path_spawn::SpawnError),
    MissingSpawnDefaults,
    Relationship(super::path_relationships::RelationshipError),
    MissingSelectedAuxiliary,
    MissingSelectedEquipment,
    MissingSelectedScore,
    Runtime(PathRuntimeError),
    MissingStatement(PathCursor),
    TooManyPaths,
    TooManyStatements {
        path_index: usize,
    },
    /// Diagnostic guard only, not a source WAIT or successful tick completion.
    BudgetExceeded {
        cursor: PathCursor,
        executed: usize,
    },
}

impl From<PathRuntimeError> for ProgramError {
    fn from(error: PathRuntimeError) -> Self {
        Self::Runtime(error)
    }
}

/// Dense, immutable semantic indices. There is no fallback to encoded scripts
/// or an original-program executor when a path has not been lowered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCatalog {
    paths: Vec<Vec<Statement>>,
}

impl PathCatalog {
    pub fn new(paths: Vec<Vec<Statement>>) -> Result<Self, ProgramError> {
        const INDEX_CAPACITY: usize = u16::MAX as usize + 1;
        if paths.len() > INDEX_CAPACITY {
            return Err(ProgramError::TooManyPaths);
        }
        for (path_index, path) in paths.iter().enumerate() {
            if path.len() > INDEX_CAPACITY {
                return Err(ProgramError::TooManyStatements { path_index });
            }
        }
        Ok(Self { paths })
    }

    pub fn statement(&self, cursor: PathCursor) -> Result<Statement, ProgramError> {
        self.paths
            .get(usize::from(cursor.path.catalog_index()))
            .and_then(|path| path.get(usize::from(cursor.command_index)))
            .copied()
            .ok_or(ProgramError::MissingStatement(cursor))
    }
}

impl PathRuntime {
    /// Execute a new common-entry invocation. A callback already entered by
    /// step_callbacks uses resume_program, preserving its selected-player state.
    pub fn enter_program(
        &mut self,
        catalog: &PathCatalog,
        objects: &mut ObjectStore,
        owner: ObjectId,
        world: &mut PathWorld<'_>,
        budget: usize,
    ) -> Result<ControlStep, ProgramError> {
        self.check_execution_owner(owner)?;
        self.initialize_path_strategy(objects, owner)?;
        self.enter(objects, owner)?;
        self.resume_program(catalog, objects, owner, world, budget)
    }

    /// Run only immediate statements. Budget exhaustion retains the next live
    /// cursor and returns an error; it must never manufacture a movement tick.
    pub fn resume_program(
        &mut self,
        catalog: &PathCatalog,
        objects: &mut ObjectStore,
        owner: ObjectId,
        world: &mut PathWorld<'_>,
        budget: usize,
    ) -> Result<ControlStep, ProgramError> {
        self.check_execution_owner(owner)?;
        for executed in 0..=budget {
            let actor = objects
                .get(owner)
                .ok_or(PathRuntimeError::MissingActor(owner))?;
            let cursor = actor
                .base
                .path
                .ok_or(PathRuntimeError::MissingPath(owner))?;
            if executed == budget {
                return Err(ProgramError::BudgetExceeded { cursor, executed });
            }
            let statement = catalog.statement(cursor)?;
            let outcome = match statement {
                Statement::ActiveWeaponLevelEquals { expected, taken, next } => {
                    let actual = world.scene.active_weapon_level
                        .ok_or(ProgramError::MissingSceneByte(SceneByte::ActiveWeaponLevel))?;
                    objects.get_mut(owner).expect("validated weapon-level observer").base.path =
                        Some(if actual == expected { taken } else { next });
                    Ok(ControlStep::Continue)
                }
                Statement::AttachmentAbsent { taken, next } => {
                    let destination = if actor.base.attachment.is_none() { taken } else { next };
                    objects.get_mut(owner).expect("validated attachment observer").base.path = Some(destination);
                    Ok(ControlStep::Continue)
                }
                Statement::TargetingUpgradeOwned { taken, next } => {
                    let upgrade = world.targeting_upgrade.as_deref()
                        .ok_or(ProgramError::MissingTargetingUpgrade)?;
                    // $7F:C488 branches directly, without consuming IFNOT.
                    objects.get_mut(owner).expect("validated upgrade observer").base.path =
                        Some(if upgrade.active_pilot_has_upgrade() { taken } else { next });
                    Ok(ControlStep::Continue)
                }
                Statement::AcquireTargetingUpgrade { next } => {
                    world.targeting_upgrade.as_deref_mut()
                        .ok_or(ProgramError::MissingTargetingUpgrade)?
                        .acquire_for_active_pilot();
                    objects.get_mut(owner).expect("validated upgrade collector").base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ClockBitsSet { mask, taken, next } => {
                    // $7F:BD06 takes direct branches: IFNOT is untouched.
                    objects
                        .get_mut(owner)
                        .expect("validated clock-gate owner")
                        .base
                        .path = Some(if world.animation_clock & mask != 0 {
                        taken
                    } else {
                        next
                    });
                    Ok(ControlStep::Continue)
                }
                Statement::AccumulateShieldRecovery { amount, next } => {
                    let amount = amount.read(actor);
                    let request = world.shield_recovery.as_deref_mut()
                        .ok_or(ProgramError::MissingShieldRecovery)?;
                    request.amount = request.amount.wrapping_add(amount);
                    objects.get_mut(owner).expect("validated recovery owner").base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::RequestShieldRecovery { amount, next } => {
                    let request = world
                        .shield_recovery
                        .as_mut()
                        .ok_or(ProgramError::MissingShieldRecovery)?;
                    request.amount = amount.read(actor);
                    objects
                        .get_mut(owner)
                        .expect("validated recovery requester")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ImportActionGate { destination, next } => {
                    let value = world.action_gate.ok_or(ProgramError::MissingActionGate)?;
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated action-gate reader");
                    destination.write(actor, value);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ImportSceneByte { source, destination, next } => {
                    let value = source.read(world.scene).ok_or(ProgramError::MissingSceneByte(source))?;
                    let actor = objects.get_mut(owner).expect("validated scene-selector reader");
                    destination.write(actor, value);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::SceneryDistance { command, next } => {
                    let scenery = world.scenery_distance.as_deref_mut()
                        .ok_or(ProgramError::MissingSceneryDistance)?;
                    let actor = objects.get_mut(owner).expect("validated scenery owner");
                    match command {
                        SceneryDistanceCommand::CopyTo(field) => field.write(actor, scenery.near_mask),
                        SceneryDistanceCommand::Assign(value) => scenery.near_mask = value.read(actor),
                    }
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ImportEnvironmentPlaneHeight { destination, next } => {
                    let value = world
                        .environment_plane_height
                        .ok_or(ProgramError::MissingEnvironmentPlaneHeight)?;
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated environmental-plane reader");
                    destination.write(actor, value as u16);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ProjectileTrigger { command, next } => {
                    let trigger = world
                        .projectile_trigger
                        .as_mut()
                        .ok_or(ProgramError::MissingProjectileTrigger)?;
                    let actor = objects.get_mut(owner).expect("validated projectile owner");
                    match command {
                        ProjectileTriggerCommand::CopyTo(field) => {
                            field.write(actor, trigger.activation)
                        }
                        ProjectileTriggerCommand::Assign(value) => {
                            trigger.activation = value.read(actor)
                        }
                    }
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::InitializePrimaryPitchRecoil { amount, next } => {
                    let primary = world
                        .primary_player
                        .ok_or(ProgramError::MissingPrimaryPlayer)?;
                    objects
                        .get(primary)
                        .ok_or(PathRuntimeError::MissingActor(primary))?;
                    world
                        .primary_pitch_recoil
                        .as_mut()
                        .ok_or(ProgramError::MissingPrimaryPitchRecoil)?
                        .initialize_if_idle(amount);
                    objects
                        .get_mut(owner)
                        .expect("validated recoil owner")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::LinkPrimaryCollisionExclusion { next } => {
                    let primary = world
                        .primary_player
                        .ok_or(ProgramError::MissingPrimaryPlayer)?;
                    objects
                        .get(primary)
                        .ok_or(PathRuntimeError::MissingActor(primary))?;
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated collision-link owner");
                    actor.base.linked_object = Some(primary);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::LinkedEffectActivity { command, next } => {
                    let activity = world
                        .linked_effect_activity
                        .as_deref_mut()
                        .ok_or(ProgramError::MissingLinkedEffectActivity)?;
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated effect-activity owner");
                    activity.apply(actor, command);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::UpdateProtectionEffect {
                    ordinary_return,
                    flicker,
                } => {
                    let input = world
                        .protection
                        .as_mut()
                        .ok_or(ProgramError::MissingProtection)?;
                    let ordinary = super::path_protection::update_effect(
                        objects,
                        owner,
                        input,
                        world.surface_mode,
                    )
                    .map_err(ProgramError::Protection)?;
                    objects
                        .get_mut(owner)
                        .expect("validated protection effect")
                        .base
                        .path = Some(if ordinary { ordinary_return } else { flicker });
                    Ok(ControlStep::Continue)
                }
                Statement::StackValue { command, next } => {
                    self.execute_stack_value(objects, owner, command, next)
                }
                Statement::Appearance { command, next } => {
                    let actor = objects.get_mut(owner).expect("validated appearance owner");
                    command.apply(actor);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::RunWhenPaused { enabled, next } => {
                    let actor = objects.get_mut(owner).expect("validated pause-mode owner");
                    actor.base.contacts.run_when_paused = enabled;
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::LatchPrimaryViewFilter { next } => {
                    let primary = world
                        .primary_player
                        .ok_or(ProgramError::MissingPrimaryPlayer)?;
                    let filtered = objects
                        .get(primary)
                        .ok_or(PathRuntimeError::MissingActor(primary))?
                        .base
                        .flags
                        .view_side_filter;
                    let actor = objects.get_mut(owner).expect("validated phase-latch owner");
                    if filtered {
                        let phase = &mut actor.extension.path_state.motion_phase;
                        *phase = (*phase & 0xFF00) | 1;
                    }
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ConsiderPrimaryTarget { next } => {
                    let primary = world
                        .primary_player
                        .ok_or(ProgramError::MissingPrimaryPlayer)?;
                    objects
                        .get(primary)
                        .ok_or(PathRuntimeError::MissingActor(primary))?;
                    let input = world
                        .primary_target
                        .as_mut()
                        .ok_or(ProgramError::MissingPrimaryTarget)?;
                    super::path_target::consider(
                        input.selection,
                        owner,
                        actor.base.position,
                        input.anchor,
                    );
                    objects
                        .get_mut(owner)
                        .expect("validated target candidate")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::CopySelectedTransform { command, next } => {
                    super::path_relationships::copy_selected_transform(
                        objects,
                        owner,
                        world.selected,
                        command,
                    )
                    .map_err(ProgramError::Relationship)?;
                    objects
                        .get_mut(owner)
                        .expect("validated transform-copy owner")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::InheritPrimaryHorizontalMotion { next } => {
                    let primary = world
                        .primary_player
                        .ok_or(ProgramError::MissingPrimaryPlayer)?;
                    let input = world
                        .primary_motion
                        .ok_or(ProgramError::MissingPrimaryMotion)?;
                    let velocity = objects
                        .get(primary)
                        .ok_or(PathRuntimeError::MissingActor(primary))?
                        .base
                        .velocity;
                    let actor = objects.get_mut(owner).expect("validated inheritance owner");
                    super::path_motion::inherit_horizontal_motion(
                        actor,
                        velocity,
                        input.displacement,
                        input.auxiliary_mode,
                    );
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ImportPlayerPosition { axis, destination, next } => {
                    use super::path_fields::Axis;
                    let position = world.published_motion
                        .ok_or(ProgramError::MissingPublishedMotion)?.position;
                    let value = match axis {
                        Axis::X => position.x,
                        Axis::Y => position.y,
                        Axis::Z => position.z,
                    };
                    let actor = objects.get_mut(owner).expect("validated position-import owner");
                    destination.write(actor, value as u16);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ImportPlayerMotion {
                    axis,
                    destination,
                    next,
                } => {
                    use super::path_fields::Axis;
                    let delta = world
                        .published_motion
                        .ok_or(ProgramError::MissingPublishedMotion)?
                        .delta;
                    let value = match axis {
                        Axis::X => delta.x,
                        Axis::Y => delta.y,
                        Axis::Z => delta.z,
                    };
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated motion-import owner");
                    destination.write(actor, value as u16);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ImportPlayerMotionByte { axis, part, destination, next } => {
                    use super::path_fields::{Axis, BytePart};
                    let delta = world.published_motion
                        .ok_or(ProgramError::MissingPublishedMotion)?.delta;
                    let word = match axis {
                        Axis::X => delta.x,
                        Axis::Y => delta.y,
                        Axis::Z => delta.z,
                    } as u16;
                    let value = match part {
                        BytePart::Low => word as u8,
                        BytePart::High => (word >> u8::BITS) as u8,
                    };
                    let actor = objects.get_mut(owner).expect("validated motion-byte import owner");
                    destination.write(actor, value);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ImportChargeThreshold { destination, next } => {
                    let value = world
                        .active_charge_threshold
                        .ok_or(ProgramError::MissingChargeThreshold)?;
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated charge-import owner");
                    destination.write(actor, value);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ImportActiveNodeFlags { destination, next } => {
                    let value = world
                        .active_node_flags
                        .ok_or(ProgramError::MissingActiveNodeFlags)?;
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated node-flags import owner");
                    destination.write(actor, value);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::RefreshSelectedChargeAttachment { next } => {
                    use super::path_relationships::RelationshipError;
                    let selected = world.selected.ok_or(ProgramError::Relationship(
                        RelationshipError::MissingSelected,
                    ))?;
                    objects.get(selected).ok_or(ProgramError::Relationship(
                        RelationshipError::MissingActor(selected),
                    ))?;
                    let input = world
                        .selected_charge
                        .ok_or(ProgramError::MissingSelectedCharge)?;
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated charge-callback owner");
                    super::path_charge::refresh_attachment(actor, selected, input);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::PlayerControl { command, next } => {
                    use super::path_player_control::{primary_position, PlayerControlCommand};
                    let primary = world
                        .primary_player
                        .ok_or(ProgramError::MissingPrimaryPlayer)?;
                    let player = objects
                        .get(primary)
                        .ok_or(PathRuntimeError::MissingActor(primary))?;
                    let pose = (player.base.position, player.base.pitch, player.base.yaw);
                    let input = world
                        .primary_control
                        .as_mut()
                        .ok_or(ProgramError::MissingPrimaryControl)?;
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated player-control owner");
                    match command {
                        PlayerControlCommand::LockToProjectile => {
                            input.target.lock_to_projectile(owner, actor.base.position)
                        }
                        PlayerControlCommand::Configure(range) => {
                            input.target.configure(owner, actor.base.position, range)
                        }
                        PlayerControlCommand::ConfigureDoubledLowByte(range) => input
                            .target
                            .configure_doubled_low_byte(owner, actor.base.position, range),
                        PlayerControlCommand::ConfigureAlternateAxes(range) => input
                            .target
                            .configure_alternate_axes(owner, actor.base.position, range),
                        PlayerControlCommand::LockForLinkedMode => {
                            input.target.lock_for_linked_mode(input.linked_mode)
                        }
                        PlayerControlCommand::FollowPrimaryPosition => {
                            actor.base.position =
                                primary_position(pose.0, pose.1, pose.2, input.linked_mode)
                        }
                        PlayerControlCommand::RefreshOwnedOrigin => input
                            .target
                            .refresh_owned_origin(owner, actor.base.position),
                    }
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Sound { cue, next } => {
                    world
                        .audio
                        .as_mut()
                        .ok_or(ProgramError::MissingAudio)?
                        .queue(cue, self.selected_player());
                    objects
                        .get_mut(owner)
                        .expect("validated sound owner")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::SpatialLoop { sound, next } => {
                    let actor = objects.get_mut(owner).expect("validated sound owner");
                    actor.extension.spatial_loop = sound;
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Countdown { command, next } => {
                    let countdown = world
                        .countdown
                        .as_mut()
                        .ok_or(ProgramError::MissingCountdown)?;
                    let actor = objects.get_mut(owner).expect("validated countdown owner");
                    countdown.apply(actor, command);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::MarkerSound { id, mode, next } => {
                    let source = objects
                        .get(owner)
                        .expect("validated sound owner")
                        .base
                        .position;
                    world
                        .audio
                        .as_mut()
                        .ok_or(ProgramError::MissingAudio)?
                        .queue_marker(id, mode, source, self.selected_player())
                        .map_err(|_| ProgramError::MissingSoundMarkers)?;
                    objects
                        .get_mut(owner)
                        .expect("validated sound owner")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::SpawnChild {
                    kind,
                    parameters,
                    next,
                } => {
                    let defaults = world
                        .spawn_defaults
                        .ok_or(ProgramError::MissingSpawnDefaults)?;
                    // Validate the independent child's native entry before
                    // allocating; absent catalog coverage is never a no-op.
                    if let Some(path) = parameters.path {
                        catalog.statement(path)?;
                    }
                    self.spawns
                        .child(objects, owner, kind, parameters, defaults)
                        .map_err(ProgramError::Spawn)?;
                    objects
                        .get_mut(owner)
                        .expect("validated spawn caller")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::SpawnIndependent {
                    kind,
                    parameters,
                    next,
                } => {
                    let defaults = world
                        .spawn_defaults
                        .ok_or(ProgramError::MissingSpawnDefaults)?;
                    if let Some(path) = parameters.path {
                        catalog.statement(path)?;
                    }
                    self.spawns
                        .independent(objects, owner, kind, parameters, defaults)
                        .map_err(ProgramError::Spawn)?;
                    objects
                        .get_mut(owner)
                        .expect("validated independent spawn caller")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Relationship { command, next } => {
                    super::path_relationships::apply(objects, owner, command)
                        .map_err(ProgramError::Relationship)?;
                    objects
                        .get_mut(owner)
                        .expect("validated relationship owner")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ChildMissing {
                    number,
                    taken,
                    next,
                } => {
                    let missing = super::path_relationships::child_missing(objects, owner, number)
                        .map_err(ProgramError::Relationship)?;
                    // This source branch bypasses the IFNOT machinery.
                    objects
                        .get_mut(owner)
                        .expect("validated child-search owner")
                        .base
                        .path = Some(if missing { taken } else { next });
                    Ok(ControlStep::Continue)
                }
                Statement::SelectedAuxiliaryBranch {
                    condition,
                    taken,
                    next,
                } => {
                    let input = world
                        .selected_auxiliary
                        .as_deref()
                        .ok_or(ProgramError::MissingSelectedAuxiliary)?;
                    self.execute_branch(
                        objects,
                        owner,
                        BranchCommand::Test {
                            predicate: condition.sample(*input),
                            taken,
                            next,
                        },
                    )
                }
                Statement::FaceSelectedOffset { offset, next } => {
                    self.execute_facing_offset(objects, owner, world.selected, offset, next)
                }
                Statement::OccupiedCell { taken, next } => {
                    let exempt = world
                        .selected_occupancy_exempt
                        .ok_or(ProgramError::MissingOccupancyExemption)?;
                    // The source exemption returns before the map lookup.
                    // Neither edge enters or consumes the IFNOT machinery.
                    let occupied = !exempt
                        && world
                            .occupancy
                            .ok_or(ProgramError::MissingOccupancy)?
                            .contains(actor.base.position);
                    objects
                        .get_mut(owner)
                        .expect("validated occupancy owner")
                        .base
                        .path = Some(if occupied { taken } else { next });
                    Ok(ControlStep::Continue)
                }
                Statement::AtOrAboveSurface { taken, next } => {
                    let search = world
                        .surface_mode
                        .ok_or(ProgramError::MissingSurfaceMode)?
                        .search();
                    let result = super::collision_surface::query_object_surface(
                        objects,
                        owner,
                        world.animation_clock,
                        search,
                    )
                    .map_err(ProgramError::SurfaceQuery)?;
                    let actor = objects.get_mut(owner).expect("validated surface owner");
                    // $7F:BF86 restores the old group byte, but retains both
                    // the supporting-object link and contact flags, even
                    // when no surface was found. Both edges bypass IFNOT.
                    let group = actor.extension.surface_contact.group;
                    actor.extension.surface_contact = result.contact;
                    actor.extension.surface_contact.group = group;
                    actor.base.path =
                        Some(if actor.base.position.y.wrapping_sub(result.height) >= 0 {
                            taken
                        } else {
                            next
                        });
                    Ok(ControlStep::Continue)
                }
                Statement::ImportSurfaceMode { destination, next } => {
                    let mode = world.surface_mode.ok_or(ProgramError::MissingSurfaceMode)?;
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated surface mode owner");
                    destination.write(actor, mode.flags);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Guidance { command, next } => {
                    let history = world
                        .guidance
                        .as_deref_mut()
                        .ok_or(ProgramError::MissingGuidance)?;
                    let actor = objects.get_mut(owner).expect("validated guidance owner");
                    match command {
                        GuidanceCommand::CopyTo(field) => field.write(actor, history.flags),
                        GuidanceCommand::Assign(value) => history.flags = value.read(actor),
                    }
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::PickupHistory { command, next } => {
                    let history = world.pickup_history.as_deref_mut()
                        .ok_or(ProgramError::MissingPickupHistory)?;
                    let actor = objects.get_mut(owner).expect("validated pickup-history owner");
                    match command {
                        PickupHistoryCommand::CopyTo(field) => field.write(actor, history.collected_mask),
                        PickupHistoryCommand::Assign(value) => history.collected_mask = value.read(actor),
                    }
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ImportControlStyle { destination, next } => {
                    let style = world
                        .control_style
                        .ok_or(ProgramError::MissingControlStyle)?;
                    let value = match style {
                        super::FlightControlStyle::TypeA => 0,
                        super::FlightControlStyle::TypeB => 1,
                    };
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated control style owner");
                    destination.write(actor, value);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::ImportCampaignByte {
                    source,
                    destination,
                    next,
                } => {
                    let input = world.campaign.ok_or(ProgramError::MissingCampaign)?;
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated campaign import owner");
                    destination.write(actor, source.read(input));
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Message { number, next } => {
                    let number = number.read(actor);
                    let radio = world.radio.as_mut().ok_or(ProgramError::MissingRadio)?;
                    radio.request_message(number);
                    objects
                        .get_mut(owner)
                        .expect("validated radio owner")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::CollectSelectedConsumables { amount, already_full, next } => {
                    let kind = (actor.extension.path_state.motion_phase >> 8) as u8;
                    let state = world.selected_equipment.as_deref_mut()
                        .ok_or(ProgramError::MissingSelectedEquipment)?;
                    let full = state.collect_consumables(amount, kind);
                    // This branch bypasses IFNOT and does not reset WAIT.
                    objects.get_mut(owner).expect("validated equipment owner").base.path =
                        Some(if full { already_full } else { next });
                    Ok(ControlStep::Continue)
                }
                Statement::AwardSelectedScore { points, next } => {
                    let score = world.selected_score.as_deref_mut()
                        .ok_or(ProgramError::MissingSelectedScore)?;
                    score.award_path_points(points);
                    objects.get_mut(owner).expect("validated score owner").base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::UpgradeSelectedWeapon { next } => {
                    let state = world.selected_equipment.as_deref_mut()
                        .ok_or(ProgramError::MissingSelectedEquipment)?;
                    state.upgrade_weapon();
                    objects.get_mut(owner).expect("validated equipment owner").base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::SelectedAuxiliary { command, next } => {
                    let state = world
                        .selected_auxiliary
                        .as_deref_mut()
                        .ok_or(ProgramError::MissingSelectedAuxiliary)?;
                    command.apply(state);
                    objects
                        .get_mut(owner)
                        .expect("validated auxiliary owner")
                        .base
                        .path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Random { mutation, next } => {
                    self.execute_random(objects, owner, world.random, mutation, next)
                }
                Statement::YawOrbit {
                    center,
                    angle,
                    next,
                } => {
                    use super::path_steering::{SteeringError, YawOrbitTarget};
                    let angle = super::Angle::from_units(angle.read(actor));
                    let target =
                        match center {
                            OrbitCenter::Selected => YawOrbitTarget::Object(world.selected.ok_or(
                                PathRuntimeError::Steering(SteeringError::MissingSelected),
                            )?),
                            OrbitCenter::LocalOrigin => YawOrbitTarget::LocalOrigin,
                        };
                    self.execute_yaw_orbit(objects, owner, target, angle, next)
                }
                Statement::Radius { command, next } => {
                    self.execute_radius(objects, owner, command, world.selected, next)
                }
                Statement::RandomBranch { taken, next } => {
                    let take = super::path_random::take_branch(world.random);
                    objects
                        .get_mut(owner)
                        .expect("validated random branch owner")
                        .base
                        .path = Some(if take { taken } else { next });
                    Ok(ControlStep::Continue)
                }
                Statement::DisableCollision { next } => {
                    self.execute_disable_collision(objects, owner, next)
                }
                Statement::Animation { command, next } => {
                    self.execute_animation(objects, owner, command, next)
                }
                Statement::Sprite { color, size, next } => {
                    self.execute_sprite(objects, owner, color, size, next)
                }
                Statement::Control(command) => self.execute_control(objects, owner, command),
                Statement::Branch(command) => self.execute_branch(objects, owner, command),
                Statement::Motion { command, next } => {
                    self.execute_motion(objects, owner, command, next)
                }
                Statement::Contact { command, next } => {
                    let actor = objects.get_mut(owner).expect("validated contact owner");
                    command.apply(actor);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Facing { command, next } => self.execute_facing(
                    objects,
                    owner,
                    command,
                    super::path_steering::FacingTargets {
                        selected: world.selected,
                        primary: world.primary_player,
                        fixed_players: world.fixed_players,
                    },
                    next,
                ),
                Statement::Mutate { mutation, next } => {
                    self.execute_mutation(objects, owner, mutation, next)
                }
                Statement::AttachedEffectMotion { command, next } => {
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated attached effect owner");
                    command.apply(actor);
                    actor.base.path = Some(next);
                    Ok(ControlStep::Continue)
                }
                Statement::Wait { duration, next } => {
                    let duration = duration.read(actor);
                    self.execute_control(objects, owner, ControlCommand::Wait { duration, next })
                }
                Statement::WaitChase {
                    field,
                    target,
                    next,
                } => {
                    let current = field.read(actor);
                    let target = target.read(actor);
                    let actor = objects
                        .get_mut(owner)
                        .expect("validated waiting chase owner");
                    field.write(actor, super::path_fields::chase_byte(current, target));
                    if current == target {
                        actor.base.path = Some(next);
                        Ok(ControlStep::Continue)
                    } else {
                        Ok(ControlStep::Movement)
                    }
                }
                Statement::Repeat {
                    count,
                    target,
                    next,
                } => {
                    let count = count.read(actor);
                    self.execute_control(
                        objects,
                        owner,
                        ControlCommand::Repeat {
                            count,
                            target,
                            next,
                        },
                    )
                }
                Statement::BeginLoop { iterations, next } => {
                    let iterations = iterations.read(actor);
                    self.execute_control(
                        objects,
                        owner,
                        ControlCommand::BeginLoop { iterations, next },
                    )
                }
                Statement::Compare {
                    condition,
                    taken,
                    next,
                } => {
                    let predicate = condition.sample(actor);
                    self.execute_branch(
                        objects,
                        owner,
                        BranchCommand::Test {
                            predicate,
                            taken,
                            next,
                        },
                    )
                }
                Statement::Spatial {
                    condition,
                    taken,
                    next,
                } => self.execute_spatial_branch(
                    objects,
                    owner,
                    world.selected,
                    condition,
                    taken,
                    next,
                ),
            }?;
            if outcome != ControlStep::Continue {
                super::path_appearance::publish_animation(
                    objects.get_mut(owner).expect("executed actor remains live"),
                    world.animation_clock,
                );
                return Ok(outcome);
            }
        }
        unreachable!("inclusive budget iteration always returns")
    }
}

#[cfg(test)]
mod tests {
    use super::super::path_control::PlayerTarget;
    use super::super::path_fields::{ByteField, ByteOperation, WordField};
    use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
    use super::super::path_triggers::{Trigger, TriggerKind};
    use super::super::{Behavior, ObjectKind, PathId, ShapeId};
    use super::*;

    fn cursor(path: u16, command_index: u16) -> PathCursor {
        PathCursor {
            path: PathId::from_catalog_index(path),
            command_index,
        }
    }

    fn world(random: &mut RandomState) -> PathWorld<'_> {
        PathWorld {
            scene: ScenePathInputs::default(),
            scenery_distance: None,
            targeting_upgrade: None,
            shield_recovery: None,
            action_gate: None,
            environment_plane_height: None,
            projectile_trigger: None,
            primary_pitch_recoil: None,
            linked_effect_activity: None,
            protection: None,
            audio: None,
            radio: None,
            campaign: None,
            guidance: None,
            pickup_history: None,
            control_style: None,
            selected_occupancy_exempt: None,
            occupancy: None,
            surface_mode: None,
            primary_player: None,
            selected: None,
            fixed_players: [None; 2],
            primary_motion: None,
            published_motion: None,
            active_charge_threshold: None,
            selected_charge: None,
            primary_control: None,
            primary_target: None,
            active_node_flags: None,
            countdown: None,
            selected_auxiliary: None,
            selected_equipment: None,
            selected_score: None,
            spawn_defaults: None,
            random,
            animation_clock: 0,
        }
    }

    #[test]
    fn recovery_and_environment_statements_require_live_inputs_and_keep_full_field_widths() {
        use super::super::player_hit_control::ShieldRecoveryRequest;
        let statements = [
            (
                Statement::RequestShieldRecovery {
                    amount: ByteOperand::Actor(ByteField::AttackPower),
                    next: cursor(0, 1),
                },
                ProgramError::MissingShieldRecovery,
            ),
            (
                Statement::ImportActionGate {
                    destination: ByteField::AttackPower,
                    next: cursor(0, 1),
                },
                ProgramError::MissingActionGate,
            ),
            (
                Statement::ImportEnvironmentPlaneHeight {
                    destination: WordField::ScriptValue,
                    next: cursor(0, 1),
                },
                ProgramError::MissingEnvironmentPlaneHeight,
            ),
        ];
        for (statement, error) in statements {
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            let initial_random = random;
            objects.get_mut(owner).unwrap().base.wait_timer = 91;
            let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
            let before = objects.clone();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(error)
            );
            assert_eq!(objects, before);
            for value in 0..=u16::MAX {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(cursor(0, 0));
                actor.base.attack_power = if matches!(statement, Statement::ImportActionGate { .. })
                {
                    !(value as u8)
                } else {
                    value as u8
                };
                actor.extension.path_state.script_value = !value;
                let mut expected = objects.clone();
                let actor = expected.get_mut(owner).unwrap();
                actor.base.path = Some(cursor(0, 1));
                if matches!(statement, Statement::ImportActionGate { .. }) {
                    actor.base.attack_power = value as u8;
                }
                if matches!(statement, Statement::ImportEnvironmentPlaneHeight { .. }) {
                    actor.extension.path_state.script_value = value;
                }
                let mut request = ShieldRecoveryRequest {
                    amount: !(value as u8),
                };
                let mut inputs = world(&mut random);
                inputs.shield_recovery = Some(&mut request);
                inputs.action_gate = Some(value as u8);
                inputs.environment_plane_height = Some(value as i16);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 0),
                        executed: 0
                    })
                );
                assert_eq!(
                    inputs.shield_recovery.as_ref().unwrap().amount,
                    !(value as u8)
                );
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 1),
                        executed: 1
                    })
                );
                assert_eq!(
                    request.amount,
                    if matches!(statement, Statement::RequestShieldRecovery { .. }) {
                        value as u8
                    } else {
                        !(value as u8)
                    }
                );
                assert_eq!(objects, expected);
                assert!(runtime.branch.invert_next);
            }
            assert_eq!(random, initial_random);
        }
    }

    #[test]
    fn clock_mask_branch_samples_live_shared_clock_and_preserves_ifnot() {
        for inverted in [false, true] {
            for mask in 0..=u8::MAX {
                let catalog = PathCatalog::new(vec![vec![Statement::ClockBitsSet {
                    mask,
                    taken: cursor(0, 2),
                    next: cursor(0, 1),
                }]])
                .unwrap();
                let (mut runtime, mut objects, owner, mut random) = setup();
                runtime.branch.invert_next = inverted;
                let original_random = random;
                for clock in 0..=u8::MAX {
                    objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
                    let mut expected = objects.clone();
                    let next = cursor(0, if clock & mask != 0 { 2 } else { 1 });
                    expected.get_mut(owner).unwrap().base.path = Some(next);
                    let mut inputs = world(&mut random);
                    inputs.animation_clock = clock;
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: next,
                            executed: 1
                        })
                    );
                    assert_eq!(objects, expected);
                    assert_eq!(runtime.branch.invert_next, inverted);
                }
                assert_eq!(random, original_random);
            }
        }
    }

    #[test]
    fn authored_recovery_parent_spawns_both_panels_and_flattens_pose_for_forty_five_visits() {
        use super::super::path_sound::{AuthoredCue, CueListener, PathAudio};
        use super::super::{authored_paths, Angle, AudioState, ObjectSpawnDefaults, SoundEvent};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::ATTACHED_RECOVERY_EFFECT);
        actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
        let original_random = random;
        let mut audio = AudioState::default();
        let catalog = authored_paths::catalog();
        for visit in 0..45 {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.pitch = Angle::from_units(57);
            actor.base.roll = Angle::from_units(99);
            actor.base.yaw = Angle::from_units(193);
            let mut inputs = world(&mut random);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.audio = Some(PathAudio {
                events: &mut audio,
                listeners: [CueListener::Other; 2],
                markers: None,
            });
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16),
                Ok(if visit == 44 {
                    ControlStep::Ended
                } else {
                    ControlStep::Movement
                })
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                (actor.base.pitch, actor.base.roll, actor.base.yaw.units()),
                (Angle::ZERO, Angle::ZERO, 193)
            );
            assert_eq!(objects.len(), 3);
        }
        for (number, shape, x) in [(41, 112, 5), (42, 113, -5)] {
            let child = objects
                .active_ids()
                .iter()
                .copied()
                .find(|id| objects.get(*id).unwrap().base.child_number == number)
                .unwrap();
            let actor = objects.get(child).unwrap();
            assert_eq!(actor.base.kind, ObjectKind::Effect);
            assert_eq!(actor.base.shape, ShapeId::from_catalog_index(shape));
            assert_eq!(
                actor.extension.relative_position,
                super::super::Vector3 {
                    x,
                    y: -100,
                    z: -200
                }
            );
            assert_eq!(actor.base.attachment, Some(owner));
            assert_eq!(actor.extension.parent, Some(owner));
            assert_eq!((actor.base.hit_points, actor.base.attack_power), (1, 1));
            assert_eq!(
                actor.extension.path_state.conditions.selected_player,
                PlayerTarget::Secondary
            );
        }
        assert_eq!(
            audio
                .take_events()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            [SoundEvent::Authored(AuthoredCue::new(
                55,
                0,
                PlayerTarget::Secondary
            ))]
        );
        assert_eq!(random, original_random);
    }

    #[test]
    fn authored_recovery_children_preserve_timed_callbacks_requests_and_both_terminal_routes() {
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_sound::{CueListener, PathAudio};
        use super::super::path_steering::AttachedEffectMotion;
        use super::super::player_hit_control::{PlayerHitControl, ShieldRecoveryRequest};
        use super::super::{authored_paths, Angle, AudioState, ObjectSpawnDefaults, Vector3};
        let catalog = authored_paths::catalog();
        // Exercise pure authored dispatch and callbacks; world pose is an
        // explicit observation here, not an implicit attachment-service tick.
        for number in [41, 42] {
            for gate_at in [None, Some(0), Some(9), Some(34), Some(35)] {
                for route in 0..7 {
                    let (mut runtime, mut objects, parent, mut random) = setup();
                    objects.get_mut(parent).unwrap().base.path =
                        Some(authored_paths::ATTACHED_RECOVERY_EFFECT);
                    let mut audio = AudioState::default();
                    let mut inputs = world(&mut random);
                    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                    inputs.audio = Some(PathAudio {
                        events: &mut audio,
                        listeners: [CueListener::Other; 2],
                        markers: None,
                    });
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, parent, &mut inputs, 16),
                        Ok(ControlStep::Movement)
                    );
                    let child = objects
                        .active_ids()
                        .iter()
                        .copied()
                        .find(|id| objects.get(*id).unwrap().base.child_number == number)
                        .unwrap();
                    for id in objects.active_ids().to_vec() {
                        objects
                            .get_mut(id)
                            .unwrap()
                            .base
                            .flags
                            .exclude_from_shape_footprint_search = true;
                    }
                    let actor = objects.get_mut(child).unwrap();
                    actor.base.position = Vector3 {
                        x: 31,
                        y: -500,
                        z: 77,
                    };
                    actor.base.roll = Angle::from_units(19);
                    actor.extension.path_state.motion_phase = 0xA5E6;
                    let mut expected = actor.clone();
                    expected.extension.path_state.script_value =
                        if number == 41 { 40 } else { (-40i16) as u16 };
                    let mode = [1, 2, 2, 130, 1, 2, 2][route];
                    let plane = match route {
                        1 => -500,
                        5 => 32767,
                        6 => -32768,
                        _ => -499,
                    };
                    // The world-Y comparison wraps its subtraction. In
                    // particular, -500 versus 32767 takes the self-frame edge.
                    let self_relative = matches!(route, 1 | 5 | 6);
                    let release_visit = if number == 41 { 40 } else { 45 };
                    let early_end = gate_at.filter(|at| *at < 35).map(|at| at + 1);
                    let final_visit =
                        early_end.unwrap_or(release_visit + if route == 4 { 3 } else { 9 });
                    let mut request = ShieldRecoveryRequest::default();
                    let mut player = PlayerHitControl {
                        reserve_shield: 1,
                        ..Default::default()
                    };
                    let mut recovered_at = Vec::new();
                    let mut expected_random = random;
                    let mut roll_target = 0;
                    for visit in 0..=final_visit {
                        if route == 4 && visit == release_visit + 3 {
                            objects.get_mut(child).unwrap().base.position.y = 0;
                            expected.base.position.y = 0;
                        }
                        let mut inputs = world(&mut random);
                        inputs.shield_recovery = Some(&mut request);
                        inputs.action_gate = Some(if gate_at == Some(visit) { 255 } else { 0 });
                        inputs.surface_mode = Some(SurfaceMode { flags: mode });
                        if mode == 2 {
                            inputs.environment_plane_height = Some(plane);
                        }
                        inputs.animation_clock = (visit + 1) as u8;
                        let outcome = runtime
                            .enter_program(&catalog, &mut objects, child, &mut inputs, 48)
                            .unwrap();
                        if early_end == Some(visit) {
                            assert_eq!(outcome, ControlStep::Ended);
                            assert_eq!(objects.get(child).unwrap().base.hit_points, 1);
                            assert_eq!(
                                objects
                                    .get(child)
                                    .unwrap()
                                    .extension
                                    .path_state
                                    .motion_phase,
                                0xA5E6
                            );
                            break;
                        }
                        assert_eq!(outcome, ControlStep::Movement);
                        if visit <= 4 {
                            AttachedEffectMotion::Settle.apply(&mut expected);
                        }
                        if visit == release_visit {
                            if self_relative {
                                roll_target = expected.extension.relative_position.x as u8;
                                expected.extension.relative_position = Vector3::default();
                                expected.extension.relative_rotation = Default::default();
                                expected.extension.path_state.motion_phase =
                                    (u16::from(mode) << 8) | u16::from(roll_target);
                            } else {
                                let turn = (expected_random.next_byte() & 31).wrapping_add(240);
                                expected.extension.path_state.motion_phase =
                                    (u16::from(mode) << 8) | u16::from(turn);
                            }
                        }
                        if visit >= release_visit {
                            if self_relative {
                                expected.base.roll =
                                    Angle::from_units(super::super::path_fields::chase_byte(
                                        expected.base.roll.units(),
                                        roll_target,
                                    ));
                                expected.base.position.y -= 4;
                            } else {
                                AttachedEffectMotion::Tumble.apply(&mut expected);
                            }
                        }
                        let active_callbacks = runtime.begin_callbacks(&objects, child).unwrap();
                        let mut callback_count = 0;
                        while active_callbacks {
                            match runtime
                                .step_callbacks(&mut objects, child, TriggerWorldInputs::default())
                                .unwrap()
                            {
                                CallbackStep::Complete => break,
                                CallbackStep::Run(_) => {
                                    callback_count += 1;
                                    assert_eq!(
                                        runtime.resume_program(
                                            &catalog,
                                            &mut objects,
                                            child,
                                            &mut inputs,
                                            12
                                        ),
                                        Ok(ControlStep::ResumeCallbacks)
                                    );
                                }
                                CallbackStep::Skipped | CallbackStep::Expired => {}
                            }
                        }
                        assert_eq!(
                            callback_count,
                            usize::from(visit < 35) + usize::from((4..29).contains(&visit)),
                            "visit {visit}"
                        );
                        if (4..29).contains(&visit) {
                            AttachedEffectMotion::Center.apply(&mut expected);
                        }
                        let actor = objects.get(child).unwrap();
                        assert_eq!(
                            actor.extension.relative_position, expected.extension.relative_position,
                            "number {number}, route {route}, visit {visit}"
                        );
                        assert_eq!(
                            actor.extension.relative_rotation,
                            expected.extension.relative_rotation
                        );
                        assert_eq!(actor.base.position, expected.base.position);
                        assert_eq!(actor.base.roll, expected.base.roll);
                        assert_eq!(
                            actor.extension.path_state.motion_phase,
                            expected.extension.path_state.motion_phase
                        );
                        assert_eq!(
                            actor.extension.parent,
                            Some(if self_relative && visit >= release_visit {
                                child
                            } else {
                                parent
                            })
                        );
                        assert_eq!(actor.base.attachment, Some(parent));
                        assert!(actor.base.flags.collision_disabled);
                        if visit >= 29 {
                            assert_eq!(
                                actor.extension.depth_offset,
                                if visit <= 37 && (visit + 1) & 1 == 0 {
                                    1
                                } else {
                                    3
                                }
                            );
                        }
                        // Source allocation already sets the hold flag;
                        // PATHHOLD also switches the strategy to movement.
                        assert!(actor.extension.path_state.hold_latched);
                        assert_eq!(
                            actor.base.behavior,
                            if visit == final_visit && early_end.is_none() {
                                Behavior::PathMovement
                            } else {
                                Behavior::FollowPath
                            }
                        );
                        assert_eq!(
                            actor.base.hit_points,
                            if visit == final_visit && early_end.is_none() {
                                0
                            } else {
                                1
                            }
                        );
                        assert!(!actor.base.flags.remove_after_tick);
                        if request.consume(&mut player, 127) {
                            recovered_at.push(visit);
                        }
                    }
                    let expected_recovery: Vec<_> = [9, 19, 29]
                        .into_iter()
                        .filter(|at| *at < early_end.unwrap_or(100))
                        .collect();
                    assert_eq!(recovered_at, expected_recovery);
                    assert_eq!(player.reserve_shield, 1 + 40 * recovered_at.len() as u8);
                    assert_eq!(random, expected_random);
                }
            }
        }
    }

    #[test]
    fn authored_triggered_projectile_parent_keeps_aiming_through_loop_break_and_final_wait() {
        use super::super::path_steering::{face_selected_offset, AimOffset};
        use super::super::{authored_paths, Angle, ObjectSpawnDefaults, Vector3};
        let catalog = authored_paths::catalog();
        for trigger_visit in [None, Some(0), Some(1), Some(10), Some(11), Some(12)] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let primary = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            let selected = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            objects.get_mut(owner).unwrap().base.path =
                Some(authored_paths::TRIGGERED_LINKED_PROJECTILE);
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .conditions
                .selected_player = PlayerTarget::Secondary;
            let mut trigger = ProjectileTrigger::default();
            let original_random = random;
            let wait_start = trigger_visit.unwrap_or(11).min(11);
            let last_visit = wait_start + 20;
            let mut child = None;
            for visit in 0..=last_visit {
                let selected_actor = objects.get_mut(selected).unwrap();
                selected_actor.base.position = Vector3 {
                    x: visit as i16 * 13,
                    y: -500,
                    z: 900,
                };
                selected_actor.base.pitch = Angle::from_units(visit as u8 * 7);
                selected_actor.base.yaw = Angle::from_units(visit as u8 * 3);
                let copied = selected_actor.base.position;
                if trigger_visit == Some(visit) {
                    trigger.activation = 255;
                }
                let expected_attack = if trigger_visit.is_some_and(|at| at <= visit && at <= 11) {
                    255
                } else {
                    0
                };
                let mut inputs = world(&mut random);
                inputs.primary_player = Some(primary);
                inputs.selected = Some(selected);
                inputs.projectile_trigger = Some(&mut trigger);
                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 30),
                    Ok(if visit == last_visit {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }),
                    "trigger={trigger_visit:?} visit={visit}"
                );
                if visit == 0 {
                    child = runtime.spawns.last_spawn;
                    let spawned = objects.get(child.unwrap()).unwrap();
                    assert_eq!(spawned.base.kind, ObjectKind::Projectile);
                    assert_eq!(spawned.base.shape, ShapeId::from_catalog_index(7));
                    assert_eq!(
                        spawned.extension.relative_position,
                        Vector3 { x: 0, y: -10, z: 0 }
                    );
                    assert_eq!(spawned.extension.relative_rotation.pitch.units(), 231);
                    assert_eq!((spawned.base.hit_points, spawned.base.attack_power), (1, 1));
                    assert_eq!(spawned.base.attachment, Some(owner));
                    assert_eq!(spawned.extension.parent, Some(owner));
                    assert_eq!(spawned.base.linked_object, None);
                    assert_eq!(
                        spawned.extension.path_state.conditions.selected_player,
                        PlayerTarget::Secondary
                    );
                }
                if visit != last_visit {
                    let mut expected = objects.clone();
                    expected.get_mut(owner).unwrap().base.position = copied;
                    face_selected_offset(
                        &mut expected,
                        owner,
                        Some(selected),
                        AimOffset { x: 0, y: 0, z: 127 },
                        &mut Default::default(),
                    )
                    .unwrap();
                    let expected = &expected.get(owner).unwrap().base;
                    let pitch = (i16::from(expected.pitch.units() as i8) / 2) as u8;
                    assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                    assert!(matches!(
                        runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                        Ok(CallbackStep::Run(_))
                    ));
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 8),
                        Ok(ControlStep::ResumeCallbacks)
                    );
                    assert_eq!(
                        runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                        Ok(CallbackStep::Complete)
                    );
                    let actual = &objects.get(owner).unwrap().base;
                    assert_eq!(actual.position, copied);
                    assert_eq!(
                        (actual.pitch.units(), actual.yaw, actual.roll),
                        (pitch, expected.yaw, Angle::ZERO)
                    );
                }
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.linked_object, Some(primary));
                assert!(actor.base.flags.collision_disabled);
                assert_eq!(actor.base.attack_power, expected_attack);
                assert_eq!(
                    actor.base.wait_timer,
                    if visit < wait_start || visit == last_visit {
                        0
                    } else {
                        (visit - wait_start + 1) as u8
                    }
                );
                assert_eq!(runtime.spawns.last_spawn, child);
                assert_eq!(objects.len(), 4);
            }
            assert_eq!(random, original_random);
            // END is not actor retirement: the child and callback allocation
            // remain until the surrounding strategy owner retires them.
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .triggers
                    .entries(&runtime.resources, owner)
                    .unwrap()
                    .len(),
                1
            );
        }
    }

    #[test]
    fn projectile_shared_import_link_and_recoil_require_live_inputs_without_partial_statement_writes(
    ) {
        use super::super::path_player_control::PitchRecoil;
        let (mut runtime, mut objects, owner, mut random) = setup();
        let primary = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let removed = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        objects.remove(removed).unwrap();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.wait_timer = 17;
        actor.extension.path_state.repeat_counter = 31;
        actor.base.attachment = Some(primary);
        runtime.branch.invert_next = true;
        let original = objects.clone();
        let initial_random = random;
        for (statement, needs_recoil) in [
            (
                Statement::LinkPrimaryCollisionExclusion { next: cursor(0, 1) },
                false,
            ),
            (
                Statement::InitializePrimaryPitchRecoil {
                    amount: 128,
                    next: cursor(0, 1),
                },
                true,
            ),
        ] {
            objects = original.clone();
            let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
            let mut recoil = PitchRecoil::default();
            let mut missing_player_recoil = PitchRecoil::default();
            let mut inputs = world(&mut random);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::MissingPrimaryPlayer)
            );
            assert_eq!(objects, original);
            inputs.primary_player = Some(removed);
            inputs.primary_pitch_recoil = Some(&mut missing_player_recoil);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::Runtime(PathRuntimeError::MissingActor(
                    removed
                )))
            );
            assert_eq!(objects, original);
            assert_eq!(inputs.primary_pitch_recoil.as_ref().unwrap().amount, 0);
            inputs.primary_player = Some(primary);
            inputs.primary_pitch_recoil = None;
            if needs_recoil {
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::MissingPrimaryPitchRecoil)
                );
                assert_eq!(objects, original);
            }
            inputs.primary_pitch_recoil = Some(&mut recoil);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 0),
                    executed: 0
                })
            );
            assert_eq!(objects, original);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            let mut expected = original.clone();
            expected.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
            if !needs_recoil {
                expected.get_mut(owner).unwrap().base.linked_object = Some(primary);
            }
            assert_eq!(objects, expected);
            assert_eq!(recoil.amount, if needs_recoil { 128 } else { 0 });
        }
        for writing in [false, true] {
            let command = if writing {
                ProjectileTriggerCommand::Assign(ByteOperand::Actor(ByteField::AttackPower))
            } else {
                ProjectileTriggerCommand::CopyTo(ByteField::AttackPower)
            };
            let catalog = PathCatalog::new(vec![vec![Statement::ProjectileTrigger {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            objects = original.clone();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingProjectileTrigger)
            );
            assert_eq!(objects, original);
            for byte in 0..=u8::MAX {
                objects = original.clone();
                let mut trigger = ProjectileTrigger {
                    activation: if writing { 0 } else { byte },
                };
                if writing {
                    objects.get_mut(owner).unwrap().base.attack_power = byte;
                }
                let mut expected = objects.clone();
                expected.get_mut(owner).unwrap().base.attack_power = byte;
                expected.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
                let mut inputs = world(&mut random);
                inputs.projectile_trigger = Some(&mut trigger);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 1),
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert_eq!(trigger.activation, byte);
            }
        }
        assert_eq!(random, initial_random);
        assert!(runtime.branch.invert_next);
    }

    #[test]
    fn authored_triggered_child_enters_locked_target_on_request_contact_occupancy_or_surface() {
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_player_control::{PitchRecoil, PlayerTargetControl, PrimaryControl};
        use super::super::path_sound::AuthoredCue;
        use super::super::path_sound::{CueListener, PathAudio};
        use super::super::world_occupancy::{
            MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle,
        };
        use super::super::{authored_paths, Angle, ObjectSpawnDefaults, Vector3};
        use super::super::{AudioState, SoundEvent};
        let catalog = authored_paths::catalog();
        for cause in 0..6 {
            for initial_recoil in [0, 64, -32768] {
                let (mut runtime, mut objects, parent, mut random) = setup();
                let primary = objects
                    .allocate(Object::new(
                        ObjectKind::Player,
                        ShapeId::EMPTY,
                        Behavior::PlayerFlight,
                    ))
                    .unwrap();
                let selected = objects
                    .allocate(Object::new(
                        ObjectKind::Player,
                        ShapeId::EMPTY,
                        Behavior::PlayerFlight,
                    ))
                    .unwrap();
                objects.get_mut(parent).unwrap().base.path =
                    Some(authored_paths::TRIGGERED_LINKED_PROJECTILE);
                objects
                    .get_mut(parent)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .selected_player = PlayerTarget::Secondary;
                let mut trigger = ProjectileTrigger::default();
                let mut recoil = PitchRecoil {
                    amount: initial_recoil,
                };
                let mut control = PlayerTargetControl {
                    configuration_locked: true,
                    owner: Some(parent),
                    transition_delay: 59,
                    ..Default::default()
                };
                let mut audio = AudioState::default();
                let mut occupancy = WorldOccupancy::default();
                occupancy.apply(
                    &MarkerCoverage::from_rectangle(WorldRectangle {
                        x: 0,
                        z: 0,
                        width: 512,
                        depth: 512,
                    })
                    .unwrap(),
                    OccupancyChange::Mark,
                );
                let mut inputs = world(&mut random);
                inputs.primary_player = Some(primary);
                inputs.projectile_trigger = Some(&mut trigger);
                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, parent, &mut inputs, 30),
                    Ok(ControlStep::Movement)
                );
                let child = runtime.spawns.last_spawn.unwrap();
                let original_random = *inputs.random;
                objects.get_mut(child).unwrap().base.pitch = Angle::from_units(20);
                objects.get_mut(child).unwrap().base.position = Vector3 {
                    x: 0,
                    y: -500,
                    z: 0,
                };
                let activated_at = if cause == 0 {
                    0
                } else if cause == 5 {
                    99
                } else {
                    4
                };
                let mut expected_relative_y: i16 = -10;
                for visit in 0..8 {
                    if cause == 0 && visit == 0 || cause == 1 && visit == 4 {
                        trigger.activation = 255;
                    }
                    if cause == 4 && visit == 4 {
                        objects.get_mut(child).unwrap().base.position.y = 0;
                    }
                    if visit > activated_at {
                        // Explicit world service changes are observed live;
                        // this path loop does not secretly tick player state.
                        objects.get_mut(child).unwrap().base.position.x = 1200 + visit;
                        control.transition_delay = 3;
                    }
                    let mut inputs = world(&mut random);
                    inputs.primary_player = Some(primary);
                    inputs.selected = Some(selected);
                    inputs.projectile_trigger = Some(&mut trigger);
                    inputs.primary_pitch_recoil = Some(&mut recoil);
                    inputs.primary_control = Some(PrimaryControl {
                        target: &mut control,
                        linked_mode: true,
                    });
                    inputs.selected_occupancy_exempt = Some(!(cause == 3 && visit == 4));
                    if cause == 3 && visit == 4 {
                        inputs.occupancy = Some(&occupancy);
                    }
                    inputs.surface_mode = Some(SurfaceMode { flags: 1 });
                    inputs.audio = Some(PathAudio {
                        events: &mut audio,
                        listeners: [CueListener::Other; 2],
                        markers: None,
                    });
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, child, &mut inputs, 30),
                        Ok(ControlStep::Movement),
                        "cause={cause} visit={visit}"
                    );
                    let active = visit >= activated_at;
                    assert!(runtime
                        .begin_movement(&mut objects, child, Default::default())
                        .unwrap());
                    let actor = objects.get(child).unwrap();
                    let velocity_y = actor.base.velocity.y;
                    // Authored callback explicitly adds vertical velocity,
                    // then ordinary attached movement adds it a second time.
                    expected_relative_y =
                        expected_relative_y.wrapping_add(velocity_y.wrapping_mul(2));
                    if !active && visit == 0 {
                        assert_ne!(velocity_y, 0);
                    } else if active {
                        assert_eq!(velocity_y, 0);
                    }
                    objects
                        .get_mut(child)
                        .unwrap()
                        .base
                        .contacts
                        .new_contact_latched = cause == 2 && visit == 3;
                    let mut runs = 0;
                    loop {
                        match runtime
                            .step_callbacks(&mut objects, child, TriggerWorldInputs::default())
                            .unwrap()
                        {
                            CallbackStep::Run(_) => {
                                runs += 1;
                                assert_eq!(
                                    runtime.resume_program(
                                        &catalog,
                                        &mut objects,
                                        child,
                                        &mut inputs,
                                        8
                                    ),
                                    Ok(ControlStep::ResumeCallbacks)
                                );
                            }
                            CallbackStep::Skipped => {}
                            CallbackStep::Complete => break,
                            other => panic!("unexpected {other:?}"),
                        }
                    }
                    runtime
                        .finish_movement(&mut objects, &mut [None; 2])
                        .unwrap();
                    assert_eq!(runs, if cause == 2 && visit == 3 { 2 } else { 1 });
                    let actor = objects.get(child).unwrap();
                    assert_eq!(actor.base.linked_object, Some(primary));
                    assert_eq!(actor.base.attachment, Some(parent));
                    assert_eq!(actor.base.speed, if active { 0 } else { 25 });
                    assert_eq!(
                        actor.base.shape,
                        ShapeId::from_catalog_index(if active { 0 } else { 7 })
                    );
                    assert_eq!(actor.extension.relative_position.y, expected_relative_y);
                    assert_eq!(
                        actor.extension.relative_rotation.pitch.units(),
                        231u8.wrapping_add((visit as u8 + 1) * 4)
                    );
                    assert_eq!(
                        actor
                            .extension
                            .path_state
                            .triggers
                            .entries(&runtime.resources, child)
                            .unwrap()
                            .len(),
                        if active { 1 } else { 2 }
                    );
                    if active {
                        assert_eq!(control.owner, Some(child));
                        assert_eq!(control.origin, actor.base.position);
                        assert!(control.configuration_locked);
                        assert!(!control.offset_enabled);
                        assert_eq!(control.axis_rates, [3, 3, 2]);
                        assert_eq!(control.axis_limits, [25, 25, 31]);
                        assert_eq!(
                            control.transition_delay,
                            if visit == activated_at { 10 } else { 3 }
                        );
                        assert_eq!(
                            recoil.amount,
                            if initial_recoil == 0 {
                                128
                            } else {
                                initial_recoil
                            }
                        );
                        assert_eq!(trigger.activation, 1);
                    } else {
                        assert_eq!(control.owner, Some(parent));
                        assert_eq!(control.transition_delay, 59);
                        assert_eq!(recoil.amount, initial_recoil);
                    }
                }
                let events = audio
                    .take_events()
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                let expected = if cause == 5 { vec![42] } else { vec![42, 43] };
                assert_eq!(
                    events,
                    expected
                        .into_iter()
                        .map(|cue| SoundEvent::Authored(AuthoredCue::new(
                            cue,
                            0,
                            PlayerTarget::Secondary
                        )))
                        .collect::<Vec<_>>()
                );
                assert_eq!(random, original_random);
                assert_eq!(objects.len(), 4);
            }
        }
    }

    fn setup() -> (PathRuntime, ObjectStore, ObjectId, RandomState) {
        let mut objects = ObjectStore::new();
        let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        actor.base.path = Some(cursor(0, 0));
        let owner = objects.allocate(actor).unwrap();
        (
            PathRuntime::default(),
            objects,
            owner,
            RandomState::default(),
        )
    }

    #[test]
    fn saved_variables_survive_movement_callbacks_calls_and_counted_loops() {
        use super::super::path_commands::StackValueCommand;
        use super::super::path_fields::{BytePart, WordOperation};
        let phase_low = ByteField::WordPart {
            field: WordField::MotionPhase,
            part: BytePart::Low,
        };
        let callback = cursor(0, 9);
        let catalog = PathCatalog::new(vec![vec![
            Statement::Control(ControlCommand::BeginLoop {
                iterations: 3,
                next: cursor(0, 1),
            }),
            Statement::StackValue {
                command: StackValueCommand::SaveByte(phase_low),
                next: cursor(0, 2),
            },
            Statement::Mutate {
                mutation: Mutation::Byte {
                    field: phase_low,
                    operation: ByteOperation::Assign(ByteOperand::Literal(17)),
                },
                next: cursor(0, 3),
            },
            Statement::Control(ControlCommand::Register {
                trigger: Trigger {
                    path: callback,
                    kind: TriggerKind::Always,
                    timer: 0,
                },
                next: cursor(0, 4),
            }),
            Statement::Control(ControlCommand::WaitOne { next: cursor(0, 5) }),
            Statement::StackValue {
                command: StackValueCommand::RestoreByte(phase_low),
                next: cursor(0, 6),
            },
            Statement::Control(ControlCommand::Cancel {
                path: callback,
                next: cursor(0, 7),
            }),
            Statement::Control(ControlCommand::Next {
                immediate: false,
                next: cursor(0, 8),
            }),
            Statement::Control(ControlCommand::End),
            Statement::StackValue {
                command: StackValueCommand::SaveWord(WordField::ScriptValue),
                next: cursor(0, 10),
            },
            Statement::Mutate {
                mutation: Mutation::Word {
                    field: WordField::ScriptValue,
                    operation: WordOperation::Assign(WordOperand::Literal(512)),
                },
                next: cursor(0, 11),
            },
            Statement::Control(ControlCommand::Call {
                target: cursor(0, 14),
                next: cursor(0, 12),
            }),
            Statement::StackValue {
                command: StackValueCommand::RestoreWord(WordField::ScriptValue),
                next: cursor(0, 13),
            },
            Statement::Control(ControlCommand::Return),
            Statement::StackValue {
                command: StackValueCommand::SaveByte(ByteField::Health),
                next: cursor(0, 15),
            },
            Statement::Mutate {
                mutation: Mutation::Byte {
                    field: ByteField::Health,
                    operation: ByteOperation::Assign(ByteOperand::Literal(0)),
                },
                next: cursor(0, 16),
            },
            Statement::StackValue {
                command: StackValueCommand::RestoreByte(ByteField::Health),
                next: cursor(0, 17),
            },
            Statement::Control(ControlCommand::Return),
        ]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.hit_points = 73;
        actor.extension.path_state.motion_phase = 0x1234;
        actor.extension.path_state.script_value = 0xABCD;
        for visit in 0..6 {
            let mut inputs = world(&mut random);
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 12),
                Ok(if visit == 5 {
                    ControlStep::Ended
                } else {
                    ControlStep::Movement
                })
            );
            if visit % 2 == 0 {
                assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                assert_eq!(
                    runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                    Ok(CallbackStep::Run(callback))
                );
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 12),
                    Ok(ControlStep::ResumeCallbacks)
                );
                assert_eq!(
                    runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                    Ok(CallbackStep::Complete)
                );
            } else if visit != 5 {
                assert!(!runtime.begin_callbacks(&objects, owner).unwrap());
            }
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                actor.extension.path_state.motion_phase,
                if visit % 2 == 0 { 0x1211 } else { 0x1234 }
            );
            assert_eq!(actor.extension.path_state.script_value, 0xABCD);
            assert_eq!(actor.base.hit_points, 73);
            assert_eq!(actor.base.position, Default::default());
        }
        assert_eq!(runtime.resources.owner_count(owner), 1); // Empty stack remains; canceling the final trigger releases its list.
        runtime.release_actor_programs(&mut objects, owner).unwrap();
        assert_eq!(
            runtime.resources.available_capacity(),
            super::super::program_resources::PROGRAM_CAPACITY
        );
    }

    #[test]
    fn stack_value_statement_budget_and_failed_restore_leave_cursor_and_actor_unchanged() {
        use super::super::path_commands::StackValueCommand;
        use super::super::program_state::PathStackError;
        let catalog = PathCatalog::new(vec![vec![
            Statement::StackValue {
                command: StackValueCommand::SaveByte(ByteField::Health),
                next: cursor(0, 1),
            },
            Statement::StackValue {
                command: StackValueCommand::RestoreWord(WordField::ScriptValue),
                next: cursor(0, 2),
            },
            Statement::Control(ControlCommand::End),
        ]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.hit_points = 251;
        objects.get_mut(owner).unwrap().base.wait_timer = 19;
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .repeat_counter = 37;
        runtime.branch.invert_next = true;
        let mut inputs = world(&mut random);
        let before = (runtime.clone(), objects.clone());
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 0),
                executed: 0
            })
        );
        assert_eq!((&runtime, &objects), (&before.0, &before.1));
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 1),
                executed: 1
            })
        );
        let before = (runtime.clone(), objects.clone());
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::Runtime(PathRuntimeError::Stack(
                PathStackError::IncompatibleSavedValue
            )))
        );
        assert_eq!((&runtime, &objects), (&before.0, &before.1));
        assert_eq!(
            runtime.execute_stack_value(
                &mut objects,
                owner,
                StackValueCommand::RestoreByte(ByteField::AttackPower),
                cursor(0, 2)
            ),
            Ok(ControlStep::Continue)
        );
        assert_eq!(objects.get(owner).unwrap().base.attack_power, 251);
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, 19);
        assert_eq!(
            objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .repeat_counter,
            37
        );
        assert!(runtime.branch.invert_next);
        let other = objects
            .allocate(Object::new(
                ObjectKind::Enemy,
                ShapeId::EMPTY,
                Behavior::FollowPath,
            ))
            .unwrap();
        let before = (runtime.clone(), objects.clone());
        assert_eq!(
            runtime.execute_stack_value(
                &mut objects,
                other,
                StackValueCommand::SaveByte(ByteField::Health),
                cursor(0, 0)
            ),
            Err(PathRuntimeError::MissingPath(other))
        );
        assert_eq!((&runtime, &objects), (&before.0, &before.1));
    }

    #[test]
    fn offset_facing_statement_preserves_wait_and_inversion_and_faults_before_mutation() {
        use super::super::path_steering::{face_selected_offset, AimOffset, SteeringError};
        let offset = AimOffset {
            x: -128,
            y: 127,
            z: 64,
        };
        let catalog = PathCatalog::new(vec![vec![Statement::FaceSelectedOffset {
            offset,
            next: cursor(0, 1),
        }]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let target = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        objects.get_mut(owner).unwrap().base.wait_timer = 173;
        runtime.branch.invert_next = true;
        runtime.steering.unchanged_axes = 199;
        let before = objects.clone();
        let initial_random = random;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::Runtime(PathRuntimeError::Steering(
                SteeringError::MissingSelected
            )))
        );
        assert_eq!(objects, before);
        assert_eq!(runtime.steering.unchanged_axes, 199);
        assert!(runtime.branch.invert_next);
        let mut inputs = world(&mut random);
        inputs.selected = Some(target);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 0),
                executed: 0
            })
        );
        assert_eq!(objects, before);
        assert_eq!(runtime.steering.unchanged_axes, 199);
        let mut expected = before.clone();
        let mut expected_steering = runtime.steering;
        face_selected_offset(
            &mut expected,
            owner,
            Some(target),
            offset,
            &mut expected_steering,
        )
        .unwrap();
        expected.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 1),
                executed: 1
            })
        );
        assert_eq!(objects, expected);
        assert_eq!(runtime.steering, expected_steering);
        assert!(runtime.branch.invert_next);
        assert_eq!(random, initial_random);
    }

    #[test]
    fn node_flag_import_preserves_every_word_and_faults_before_mutation() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let original_random = random;
        runtime.branch.invert_next = true;
        objects.get_mut(owner).unwrap().base.wait_timer = 173;
        let catalog = PathCatalog::new(vec![vec![Statement::ImportActiveNodeFlags {
            destination: WordField::ScriptValue,
            next: cursor(0, 1),
        }]])
        .unwrap();
        let before = objects.clone();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingActiveNodeFlags)
        );
        assert_eq!(objects, before);
        for flags in 0..=u16::MAX {
            objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
            let mut expected = objects.get(owner).unwrap().clone();
            expected.extension.path_state.script_value = flags;
            expected.base.path = Some(cursor(0, 1));
            let mut inputs = world(&mut random);
            inputs.active_node_flags = Some(flags);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1,
                })
            );
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn authored_node_target_gate_uses_all_health_selectors_and_skips_target_inputs_on_exit() {
        use super::super::authored_paths;
        use super::super::path_target::{PrimaryTarget, TargetAnchor, TargetSelection};
        use super::super::Vector3;
        let catalog = authored_paths::catalog();
        let entry = authored_paths::NODE_GATED_TARGET_SERVICE;
        let Statement::Appearance { next: import, .. } = catalog.statement(entry).unwrap() else {
            panic!("target service must begin with visibility");
        };
        let Statement::ImportActiveNodeFlags {
            next: comparison, ..
        } = catalog.statement(import).unwrap()
        else {
            panic!("target service requires a fresh node-flags import");
        };
        let Statement::Compare {
            condition:
                ActorCondition::AnyWordBitsSet(
                    WordOperand::Actor(WordField::ScriptValue),
                    WordOperand::IndexedBitMask {
                        selector: ByteOperand::Actor(ByteField::Health),
                        masks,
                    },
                ),
            taken: terminal,
            ..
        } = catalog.statement(comparison).unwrap()
        else {
            panic!("target service requires live health, not a literal bit selector");
        };
        for health in 0..=u8::MAX {
            // Independent table indexing: decrement/double wraps as a byte,
            // making health 0 alias 128, and health 129 alias 1.
            let mask = masks[(usize::from(health) + 127) % 128];
            for flags in [0, !mask, mask, u16::MAX] {
                let ended = flags & mask != 0;
                let (mut runtime, mut objects, owner, mut random) = setup();
                let original_random = random;
                runtime.branch.invert_next = true;
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(entry);
                actor.base.hit_points = health;
                actor.base.wait_timer = 193;
                actor.base.position.z = 100;
                actor.base.contacts.new_contact_latched = true;
                actor.base.contacts.hit_by_primary = true;
                actor.base.contacts.hit_by_secondary = true;
                actor.extension.path_state.clear_on_path_exit_latch = true;
                let mut expected = objects.clone();
                let actor = expected.get_mut(owner).unwrap();
                actor.base.flags.visible = false;
                actor.base.flags.collision_disabled = true;
                actor.extension.path_state.script_value = flags;
                actor.base.path = Some(if ended { terminal } else { import });
                if ended {
                    actor.base.flags.remove_after_tick = true;
                    actor.base.contacts.new_contact_latched = false;
                    actor.base.contacts.hit_by_primary = false;
                    actor.base.contacts.hit_by_secondary = false;
                    actor.extension.path_state.clear_on_path_exit_latch = false;
                }
                let mut selection = TargetSelection {
                    distance: u16::MAX,
                    ..TargetSelection::default()
                };
                let mut expected_selection = selection;
                if !ended {
                    expected_selection.control_flags = 0x08;
                    expected_selection.candidate = Some(owner);
                    expected_selection.distance = 56;
                    expected_selection.auxiliary_distance = 100;
                    expected_selection.position.z = 100;
                    expected_selection.screen = [112, 96];
                }
                let mut inputs = world(&mut random);
                inputs.active_node_flags = Some(flags);
                // The taken exit must not ask for either target input.
                if !ended {
                    inputs.primary_player = Some(owner);
                    inputs.primary_target = Some(PrimaryTarget {
                        anchor: TargetAnchor {
                            position: Vector3::default(),
                            pitch: 0,
                            yaw: 0,
                        },
                        selection: &mut selection,
                    });
                }
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 6),
                    Ok(if ended {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }),
                    "health={health} flags={flags}"
                );
                assert_eq!(objects, expected);
                assert_eq!(selection, expected_selection);
                // The bit-test branch bypasses, rather than consumes, IFNOT.
                assert!(runtime.branch.invert_next);
                assert_eq!(random, original_random);
            }
        }
    }

    #[test]
    fn authored_node_target_loop_resamples_flags_and_health_without_repeating_visibility() {
        use super::super::authored_paths;
        use super::super::path_target::{PrimaryTarget, TargetAnchor, TargetSelection};
        use super::super::Vector3;
        let catalog = authored_paths::catalog();
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::NODE_GATED_TARGET_SERVICE);
        let mut selection = TargetSelection::default();
        for (visit, (health, flags)) in [(16, 1), (1, 2), (16, 0x8000)].into_iter().enumerate() {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.hit_points = health;
            actor.base.position.z = 100 + visit as i16;
            // Visibility is run only on entry, not on the yielding backedge.
            actor.base.flags.visible = true;
            actor.base.flags.collision_disabled = false;
            selection.distance = u16::MAX;
            let previous_selection = selection;
            let mut inputs = world(&mut random);
            inputs.active_node_flags = Some(flags);
            inputs.primary_player = Some(owner);
            inputs.primary_target = Some(PrimaryTarget {
                anchor: TargetAnchor {
                    position: Vector3::default(),
                    pitch: 0,
                    yaw: 0,
                },
                selection: &mut selection,
            });
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 6),
                Ok(if visit == 2 {
                    ControlStep::Ended
                } else {
                    ControlStep::Movement
                })
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.extension.path_state.script_value, flags);
            assert_eq!(actor.base.flags.visible, visit != 0);
            assert_eq!(actor.base.flags.collision_disabled, visit == 0);
            if visit == 2 {
                assert_eq!(selection, previous_selection);
            } else {
                assert_eq!(selection.position, actor.base.position);
                assert_eq!(selection.candidate, Some(owner));
            }
        }
    }

    #[test]
    fn primary_target_dispatch_uses_live_world_selection_and_preserves_actors_ifnot_and_rng() {
        use super::super::path_target::{PrimaryTarget, TargetAnchor, TargetSelection};
        use super::super::Vector3;
        let (mut runtime, mut objects, owner, mut random) = setup();
        let primary = objects
            .allocate(objects.get(owner).unwrap().clone())
            .unwrap();
        let removed = objects
            .allocate(objects.get(owner).unwrap().clone())
            .unwrap();
        objects.remove(removed).unwrap();
        let original_random = random;
        runtime.branch.invert_next = true;
        let mut selection = TargetSelection {
            distance: u16::MAX,
            display_status: 255,
            control_flags: 0x40,
            ..TargetSelection::default()
        };
        let mut anchor = TargetAnchor {
            position: Vector3::default(),
            pitch: 0,
            yaw: 0,
        };
        let catalog = PathCatalog::new(vec![vec![Statement::ConsiderPrimaryTarget {
            next: cursor(0, 1),
        }]])
        .unwrap();
        for (player, missing_input, error) in [
            (None, false, ProgramError::MissingPrimaryPlayer),
            (
                Some(removed),
                false,
                ProgramError::Runtime(PathRuntimeError::MissingActor(removed)),
            ),
            (Some(primary), true, ProgramError::MissingPrimaryTarget),
        ] {
            let before = objects.clone();
            let previous_selection = selection;
            let mut inputs = world(&mut random);
            inputs.primary_player = player;
            if !missing_input {
                inputs.primary_target = Some(PrimaryTarget {
                    anchor,
                    selection: &mut selection,
                });
            }
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(error)
            );
            assert_eq!(objects, before);
            assert_eq!(selection, previous_selection);
        }
        // An ordinary acceptance, tie rejection, forced acceptance, then
        // locked rejection. The actual primary identity may alias the caller;
        // selected-player pose is unrelated to the fixed view anchor.
        for visit in 0..4 {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(cursor(0, 0));
            actor.base.wait_timer = 77;
            actor.base.position = Vector3 {
                x: 0,
                y: 0,
                z: if visit < 2 { 100 } else { 1000 },
            };
            let mut expected_objects = objects.clone();
            expected_objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
            selection.display_status = 255;
            if visit == 2 {
                selection.forced_owner = Some(owner);
                anchor.position.z = 200;
            }
            let mut expected = selection;
            if visit == 0 {
                expected.display_status = 127;
                expected.control_flags = 0x48;
                expected.candidate = Some(owner);
                expected.distance = 56;
                expected.auxiliary_distance = 100;
                expected.position.z = 100;
                expected.screen = [112, 96];
            } else if visit == 1 {
                expected.display_status = 127;
            } else if visit == 2 {
                expected.display_status = 127;
                expected.control_flags = 0x58;
                expected.distance = 450;
                expected.auxiliary_distance = 288;
                expected.position.z = 1000;
            }
            let mut inputs = world(&mut random);
            inputs.primary_player = Some(if visit & 1 == 0 { primary } else { owner });
            inputs.selected = Some(removed); // This service does not read selection.
            inputs.primary_target = Some(PrimaryTarget {
                anchor,
                selection: &mut selection,
            });
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(objects, expected_objects);
            assert_eq!(selection, expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn yaw_orbit_dispatch_samples_aliasing_angle_before_position_and_preserves_other_state() {
        use super::super::path_fields::{Axis, BytePart};
        use super::super::path_steering::yaw_orbit_position;
        use super::super::{Angle, Vector3};
        for center in [OrbitCenter::Selected, OrbitCenter::LocalOrigin] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let target = objects
                .allocate(objects.get(owner).unwrap().clone())
                .unwrap();
            runtime.branch.invert_next = true;
            let original_random = random;
            let field = if center == OrbitCenter::LocalOrigin {
                WordField::RelativePosition(Axis::X)
            } else {
                WordField::Position(Axis::X)
            };
            let catalog = PathCatalog::new(vec![vec![Statement::YawOrbit {
                center,
                angle: ByteOperand::Actor(ByteField::WordPart {
                    field,
                    part: BytePart::Low,
                }),
                next: cursor(0, 1),
            }]])
            .unwrap();
            for angle in 0..=u8::MAX {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(cursor(0, 0));
                actor.base.wait_timer = 79;
                actor.base.position = Vector3 {
                    x: 31000,
                    y: -17003,
                    z: -32000,
                };
                actor.extension.relative_position = Vector3 {
                    x: -1001,
                    y: 30007,
                    z: 2003,
                };
                let value = field.read(actor);
                field.write(actor, (value & 0xFF00) | u16::from(angle));
                let selected_position = Vector3 {
                    x: -31000,
                    y: 803,
                    z: i16::from(angle),
                };
                objects.get_mut(target).unwrap().base.position = selected_position;
                let selected = if angle % 2 == 0 { target } else { owner };
                let actor = objects.get(owner).unwrap();
                let position = if center == OrbitCenter::LocalOrigin {
                    actor.extension.relative_position
                } else {
                    actor.base.position
                };
                let origin = if center == OrbitCenter::LocalOrigin {
                    Vector3::default()
                } else {
                    objects.get(selected).unwrap().base.position
                };
                let result = yaw_orbit_position(position, origin, Angle::from_units(angle));
                let mut expected = objects.clone();
                let actor = expected.get_mut(owner).unwrap();
                if center == OrbitCenter::LocalOrigin {
                    actor.extension.relative_position = result;
                } else {
                    actor.base.position = result;
                }
                actor.base.path = Some(cursor(0, 1));
                let mut inputs = world(&mut random);
                inputs.selected = (center == OrbitCenter::Selected).then_some(selected);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 1),
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert_eq!(random, original_random);
                assert!(runtime.branch.invert_next);
            }
        }
    }

    #[test]
    fn radius_dispatch_preserves_center_choice_signed_amount_and_unrelated_fields() {
        use super::super::path_steering::{RadiusCenter, RadiusCommand};
        use super::super::Vector3;
        for center in [
            RadiusCenter::Selected,
            RadiusCenter::Linked,
            RadiusCenter::LocalOrigin,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let target = objects
                .allocate(objects.get(owner).unwrap().clone())
                .unwrap();
            let linked = objects
                .allocate(objects.get(owner).unwrap().clone())
                .unwrap();
            let original_random = random;
            runtime.branch.invert_next = true;
            for amount in i8::MIN..=i8::MAX {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(cursor(0, 0));
                actor.base.wait_timer = 59;
                actor.base.position = Vector3 {
                    x: 100,
                    y: -300,
                    z: 513,
                };
                actor.extension.relative_position = Vector3 {
                    x: -30001,
                    y: 1023,
                    z: 29900,
                };
                actor.base.attachment = Some(if amount & 1 == 0 { linked } else { owner });
                let selected = if amount & 2 == 0 { target } else { owner };
                objects.get_mut(target).unwrap().base.position = Vector3 {
                    x: i16::from(amount),
                    y: 151,
                    z: 10,
                };
                objects.get_mut(linked).unwrap().base.position = Vector3 {
                    x: 88,
                    y: -151,
                    z: i16::from(amount),
                };
                let actor = objects.get(owner).unwrap();
                let position = if center == RadiusCenter::LocalOrigin {
                    actor.extension.relative_position
                } else {
                    actor.base.position
                };
                let origin = match center {
                    RadiusCenter::Selected => objects.get(selected).unwrap().base.position,
                    RadiusCenter::Linked => {
                        objects
                            .get(actor.base.attachment.unwrap())
                            .unwrap()
                            .base
                            .position
                    }
                    RadiusCenter::LocalOrigin => Vector3::default(),
                };
                let result =
                    super::super::path_math::change_radius(position, origin, i16::from(amount));
                let mut expected = objects.clone();
                let actor = expected.get_mut(owner).unwrap();
                if center == RadiusCenter::LocalOrigin {
                    actor.extension.relative_position = result;
                } else {
                    actor.base.position = result;
                }
                actor.base.path = Some(cursor(0, 1));
                let catalog = PathCatalog::new(vec![vec![Statement::Radius {
                    command: RadiusCommand {
                        center,
                        amount: i16::from(amount),
                    },
                    next: cursor(0, 1),
                }]])
                .unwrap();
                let mut inputs = world(&mut random);
                inputs.selected = (center == RadiusCenter::Selected).then_some(selected);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 1),
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert_eq!(random, original_random);
                assert!(runtime.branch.invert_next);
            }
        }
    }

    #[test]
    fn geometry_dispatch_missing_or_dangling_centers_fail_before_mutation() {
        use super::super::path_steering::{RadiusCenter, RadiusCommand, SteeringError};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let absent = objects
            .allocate(objects.get(owner).unwrap().clone())
            .unwrap();
        objects.remove(absent).unwrap();
        let original_random = random;
        runtime.branch.invert_next = true;
        for statement in [
            Statement::YawOrbit {
                center: OrbitCenter::Selected,
                angle: ByteOperand::Literal(64),
                next: cursor(0, 1),
            },
            Statement::Radius {
                command: RadiusCommand {
                    center: RadiusCenter::Selected,
                    amount: -128,
                },
                next: cursor(0, 1),
            },
            Statement::Radius {
                command: RadiusCommand {
                    center: RadiusCenter::Linked,
                    amount: 127,
                },
                next: cursor(0, 1),
            },
        ] {
            for center in [None, Some(absent)] {
                objects.get_mut(owner).unwrap().base.attachment = center;
                let before = objects.clone();
                let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
                let mut inputs = world(&mut random);
                inputs.selected = center;
                let error = match center {
                    Some(id) => SteeringError::MissingActor(id),
                    None if matches!(
                        statement,
                        Statement::Radius {
                            command: RadiusCommand {
                                center: RadiusCenter::Linked,
                                ..
                            },
                            ..
                        }
                    ) =>
                    {
                        SteeringError::MissingLinked(owner)
                    }
                    None => SteeringError::MissingSelected,
                };
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::Runtime(PathRuntimeError::Steering(error)))
                );
                assert_eq!(objects, before);
                assert_eq!(random, original_random);
                assert!(runtime.branch.invert_next);
            }
        }
    }

    #[test]
    fn random_branch_dispatch_is_immediate_and_keeps_inversion_and_wait_state() {
        for same_destination in [false, true] {
            let catalog = PathCatalog::new(vec![vec![Statement::RandomBranch {
                taken: cursor(0, 2),
                next: cursor(0, if same_destination { 2 } else { 1 }),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, _) = setup();
            objects.get_mut(owner).unwrap().base.wait_timer = 43;
            let mut random = RandomState::default();
            let original = random;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 0),
                    executed: 0
                })
            );
            assert_eq!(random, original);
            for inverted in [false, true] {
                for first in [0, 1] {
                    for last in 0..=u8::MAX {
                        runtime.branch.invert_next = inverted;
                        objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
                        let mut random = RandomState::new([first, 0, 0, last]);
                        let mut expected_random = random;
                        let draw = expected_random.next_byte();
                        let destination = cursor(
                            0,
                            if draw <= 126 || same_destination {
                                2
                            } else {
                                1
                            },
                        );
                        let mut expected = objects.clone();
                        expected.get_mut(owner).unwrap().base.path = Some(destination);
                        assert_eq!(
                            runtime.resume_program(
                                &catalog,
                                &mut objects,
                                owner,
                                &mut world(&mut random),
                                1
                            ),
                            Err(ProgramError::BudgetExceeded {
                                cursor: destination,
                                executed: 1
                            })
                        );
                        assert_eq!(objects, expected);
                        assert_eq!(runtime.branch.invert_next, inverted);
                        assert_eq!(random, expected_random);
                    }
                }
            }
        }
    }

    #[test]
    fn attached_motion_dispatch_is_atomic_immediate_and_keeps_wait_repeat_ifnot_and_random() {
        use super::super::path_steering::AttachedEffectMotion;
        for command in [
            AttachedEffectMotion::Settle,
            AttachedEffectMotion::Center,
            AttachedEffectMotion::Tumble,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            let original_random = random;
            let actor = objects.get_mut(owner).unwrap();
            actor.base.wait_timer = 181;
            actor.extension.path_state.repeat_counter = 43;
            actor.extension.relative_position = super::super::Vector3 {
                x: -32768,
                y: 32767,
                z: -199,
            };
            actor.extension.path_state.script_value = 40;
            actor.extension.path_state.motion_phase = 0xA7F0;
            let before = objects.clone();
            let catalog = PathCatalog::new(vec![vec![Statement::AttachedEffectMotion {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 0),
                    executed: 0
                })
            );
            assert_eq!(objects, before);
            let mut expected = before;
            let actor = expected.get_mut(owner).unwrap();
            command.apply(actor);
            actor.base.path = Some(cursor(0, 1));
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(objects, expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn word_swap_dispatch_reads_live_fields_and_advances_without_yielding_or_consuming_ifnot() {
        for second in [WordField::MotionPhase, WordField::ScriptValue] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            let original_random = random;
            objects.get_mut(owner).unwrap().base.wait_timer = 181;
            let catalog = PathCatalog::new(vec![vec![Statement::Mutate {
                mutation: Mutation::SwapWords {
                    first: WordField::ScriptValue,
                    second,
                },
                next: cursor(0, 1),
            }]])
            .unwrap();
            for value in [0, 32768, 65535] {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(cursor(0, 0));
                actor.extension.path_state.script_value = value;
                actor.extension.path_state.motion_phase = !value;
                let mut expected = objects.clone();
                let actor = expected.get_mut(owner).unwrap();
                if second != WordField::ScriptValue {
                    actor.extension.path_state.script_value = !value;
                    actor.extension.path_state.motion_phase = value;
                }
                actor.base.path = Some(cursor(0, 1));
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 1),
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, original_random);
            }
        }
    }

    #[test]
    fn indexed_add_and_advance_is_immediate_and_preserves_branch_rng_and_wait_state() {
        use super::super::path_fields::{BytePart, IndexedAddField};
        static VALUES: [u8; 256] = [255; 256];
        let selector = ByteField::WordPart {
            field: WordField::MotionPhase,
            part: BytePart::High,
        };
        for field in [
            IndexedAddField::Byte(selector),
            IndexedAddField::SignedWord(WordField::MotionPhase),
        ] {
            for period in [0, 1, 3, 255] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                objects.get_mut(owner).unwrap().base.wait_timer = 183;
                runtime.branch.invert_next = true;
                let initial_random = random;
                let catalog = PathCatalog::new(vec![vec![Statement::Mutate {
                    mutation: Mutation::IndexedAddAndAdvance {
                        field,
                        selector,
                        values: &VALUES,
                        period,
                    },
                    next: cursor(0, 0),
                }]])
                .unwrap();
                for phase in [0, 1, 255, 256, 32767, 32768, 65535] {
                    objects
                        .get_mut(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .motion_phase = phase;
                    let mut expected = objects.get(owner).unwrap().clone();
                    let added = match field {
                        IndexedAddField::Byte(_) => {
                            ((phase >> 8).wrapping_sub(1) << 8) | (phase & 255)
                        }
                        IndexedAddField::SignedWord(_) => phase.wrapping_sub(1),
                    };
                    let incremented = (((added >> 8) + 1) & 255) as u8;
                    let bounded = if period != 0 && incremented >= period {
                        0
                    } else {
                        incremented
                    };
                    expected.extension.path_state.motion_phase =
                        (u16::from(bounded) << 8) | (added & 255);
                    assert_eq!(
                        runtime.resume_program(
                            &catalog,
                            &mut objects,
                            owner,
                            &mut world(&mut random),
                            1
                        ),
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(0, 0),
                            executed: 1
                        })
                    );
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, initial_random);
                }
            }
        }
    }

    #[test]
    fn charge_inputs_are_live_and_missing_observations_fault_before_mutation() {
        use super::super::path_charge::SelectedChargeInput;
        use super::super::path_fields::BytePart;
        use super::super::path_relationships::RelationshipError;
        let (mut runtime, mut objects, owner, mut random) = setup();
        let other = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let removed = objects
            .allocate(Object::new(
                ObjectKind::Enemy,
                ShapeId::EMPTY,
                Behavior::FollowPath,
            ))
            .unwrap();
        objects.remove(removed).unwrap();
        let original_random = random;
        runtime.branch.invert_next = true;
        let import = PathCatalog::new(vec![vec![Statement::ImportChargeThreshold {
            destination: ByteField::WordPart {
                field: WordField::MotionPhase,
                part: BytePart::High,
            },
            next: cursor(0, 0),
        }]])
        .unwrap();
        let callback = PathCatalog::new(vec![vec![Statement::RefreshSelectedChargeAttachment {
            next: cursor(0, 0),
        }]])
        .unwrap();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0xF37D;
        let before = objects.get(owner).unwrap().clone();
        assert_eq!(
            runtime.resume_program(&import, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingChargeThreshold)
        );
        assert_eq!(objects.get(owner).unwrap(), &before);
        for (selected, expected) in [
            (
                None,
                ProgramError::Relationship(RelationshipError::MissingSelected),
            ),
            (
                Some(removed),
                ProgramError::Relationship(RelationshipError::MissingActor(removed)),
            ),
            (Some(other), ProgramError::MissingSelectedCharge),
        ] {
            let mut inputs = world(&mut random);
            inputs.selected = selected;
            assert_eq!(
                runtime.resume_program(&callback, &mut objects, owner, &mut inputs, 1),
                Err(expected)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
        }
        let expected_outcome = Err(ProgramError::BudgetExceeded {
            cursor: cursor(0, 0),
            executed: 1,
        });
        for value in 0..=u8::MAX {
            let mut inputs = world(&mut random);
            inputs.active_charge_threshold = Some(value);
            let mut expected = objects.get(owner).unwrap().clone();
            expected.extension.path_state.motion_phase =
                (u16::from(value) << 8) | (expected.extension.path_state.motion_phase & 255);
            assert_eq!(
                runtime.resume_program(&import, &mut objects, owner, &mut inputs, 1),
                expected_outcome
            );
            assert_eq!(objects.get(owner).unwrap(), &expected);
            for selected in [owner, other] {
                for linked_mode in [false, true] {
                    inputs.selected = Some(selected);
                    inputs.selected_charge = Some(SelectedChargeInput {
                        linked_mode,
                        level: !value,
                    });
                    expected.extension.parent = Some(selected);
                    expected.extension.relative_position.y = 0;
                    expected.extension.relative_position.z =
                        expected.extension.path_state.script_value as i16;
                    expected.extension.path_state.motion_phase =
                        (u16::from(value) << 8) | u16::from(!value);
                    if linked_mode {
                        expected.extension.texture_scroll_x = 255;
                    }
                    assert_eq!(
                        runtime.resume_program(&callback, &mut objects, owner, &mut inputs, 1),
                        expected_outcome
                    );
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(inputs.random, &original_random);
                }
            }
        }
    }

    #[test]
    fn complete_charge_orb_waits_grows_and_holds_while_callback_remains_live() {
        use super::super::path_charge::SelectedChargeInput;
        use super::super::{authored_paths, path_motion, Vector3};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let other = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let other_before = objects.get(other).unwrap().clone();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::PLAYER_CHARGE_ORB);
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .relative_position
            .x = -193;
        let original_random = random;
        let catalog = authored_paths::catalog();
        let mut size = 0_u8;
        for visit in 0..15 {
            let mut inputs = world(&mut random);
            // Readiness changes after the first comparison. The next visit
            // compares the complete prior callback byte, including bit 80.
            inputs.active_charge_threshold = Some(if visit < 10 { 25 } else { 165 });
            let selected = if visit % 2 == 0 { other } else { owner };
            inputs.selected = Some(selected);
            let linked_mode = visit == 4 || visit == 12;
            let level = if visit == 9 { 165 } else { 5 };
            inputs.selected_charge = Some(SelectedChargeInput { linked_mode, level });
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .script_value = 32765 + visit;
            if !objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .hold_latched
            {
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 32),
                    Ok(ControlStep::Movement)
                );
            }
            let actor = objects.get(owner).unwrap();
            assert!(actor.base.flags.collision_disabled);
            assert_eq!(
                actor.base.wait_timer,
                if visit < 2 { visit as u8 + 1 } else { 0 }
            );
            if (2..=9).contains(&visit) {
                size = size.wrapping_add(1);
            }
            if visit == 10 {
                size = 8;
            }
            if visit >= 2 {
                assert_eq!(actor.extension.texture_scroll_x, size);
            }
            assert_eq!(actor.extension.path_state.hold_latched, visit >= 10);
            if visit >= 2 {
                assert_eq!(
                    actor.base.shape,
                    ShapeId::from_catalog_index(if visit < 10 { 15 } else { 17 })
                );
            }
            assert!(!actor.base.flags.remove_after_tick);
            let callbacks = runtime
                .begin_movement(
                    &mut objects,
                    owner,
                    path_motion::PlayerDisplacement::default(),
                )
                .unwrap();
            assert_eq!(callbacks, visit >= 2);
            if callbacks {
                assert!(matches!(
                    runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                    Ok(CallbackStep::Run(_))
                ));
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 2),
                    Ok(ControlStep::ResumeCallbacks)
                );
                assert_eq!(
                    runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                    Ok(CallbackStep::Complete)
                );
                if linked_mode {
                    size = 255;
                }
            }
            runtime
                .finish_movement(&mut objects, &mut [None, None])
                .unwrap();
            let actor = objects.get(owner).unwrap();
            if callbacks {
                assert_eq!(actor.extension.parent, Some(selected));
                assert_eq!(
                    actor.extension.relative_position,
                    Vector3 {
                        x: -193,
                        y: 0,
                        z: (32765 + visit) as i16
                    }
                );
                assert_eq!(actor.extension.texture_scroll_x, size);
                assert_eq!(actor.extension.path_state.motion_phase as u8, level);
                assert_eq!(
                    actor.extension.path_state.motion_phase >> 8,
                    if visit < 9 {
                        0
                    } else if visit == 9 {
                        25
                    } else {
                        165
                    }
                );
            }
            assert_eq!(actor.base.position, Vector3::default());
            assert_eq!(actor.base.attachment, None);
            assert!(!actor.extension.path_state.motion.attached_coordinates);
            assert_eq!(objects.get(other).unwrap(), &other_before);
            assert_eq!(inputs.random, &original_random);
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }

    #[test]
    fn pickup_history_import_export_preserves_full_words_and_unrelated_actor_state() {
        assert_eq!(PickupHistory::default().collected_mask, 0);
        for importing in [false, true] {
            let command = if importing { PickupHistoryCommand::CopyTo(WordField::ScriptValue) }
                else { PickupHistoryCommand::Assign(WordOperand::Actor(WordField::ScriptValue)) };
            let catalog = PathCatalog::new(vec![vec![Statement::PickupHistory {
                command, next: cursor(0, 0),
            }]]).unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.wait_timer = 193;
            objects.get_mut(owner).unwrap().extension.path_state.motion_phase = 0xABCD;
            let initial_random = random;
            for inverted in [false, true] {
                runtime.branch.invert_next = inverted;
                let before_missing = objects.clone();
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
                    Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                    Err(ProgramError::MissingPickupHistory));
                assert_eq!(objects, before_missing);
                for value in 0..=u16::MAX {
                    let opposite = value ^ u16::MAX;
                    objects.get_mut(owner).unwrap().extension.path_state.script_value = if importing { opposite } else { value };
                    let mut expected = objects.clone();
                    let mut history = PickupHistory { collected_mask: if importing { value } else { opposite } };
                    let mut inputs = world(&mut random);
                    inputs.pickup_history = Some(&mut history);
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 1 }));
                    if importing { expected.get_mut(owner).unwrap().extension.path_state.script_value = value; }
                    assert_eq!(objects, expected);
                    assert_eq!(history.collected_mask, value);
                    assert_eq!(runtime.branch.invert_next, inverted);
                    assert_eq!(random, initial_random);
                }
            }
        }
    }

    #[test]
    fn pickup_history_export_resumes_from_actor_copy_and_replaces_intervening_scene_writes() {
        let catalog = PathCatalog::new(vec![vec![
            Statement::PickupHistory { command: PickupHistoryCommand::CopyTo(WordField::ScriptValue), next: cursor(0, 1) },
            Statement::PickupHistory { command: PickupHistoryCommand::Assign(WordOperand::Actor(WordField::ScriptValue)), next: cursor(0, 2) },
        ]]).unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let mut history = PickupHistory { collected_mask: 0xF0F0 };
        let mut inputs = world(&mut random);
        inputs.pickup_history = Some(&mut history);
        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::BudgetExceeded { cursor: cursor(0, 1), executed: 1 }));
        assert_eq!(objects.get(owner).unwrap().extension.path_state.script_value, 0xF0F0);
        let paused_history = inputs.pickup_history.take().unwrap();
        let before = objects.clone();
        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1), Err(ProgramError::MissingPickupHistory));
        assert_eq!(objects, before);
        paused_history.collected_mask = 0x0F0F;
        inputs.pickup_history = Some(paused_history);
        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::BudgetExceeded { cursor: cursor(0, 2), executed: 1 }));
        assert_eq!(history.collected_mask, 0xF0F0); // replacement, not OR or a fresh import
        assert_eq!(objects.get(owner).unwrap().extension.path_state.script_value, 0xF0F0);
    }

    #[test]
    fn published_motion_byte_imports_preserve_other_halves_and_require_a_live_snapshot() {
        use super::super::path_fields::{Axis, BytePart};
        use super::super::path_motion::PublishedPlayerMotion;
        use super::super::Vector3;
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            for part in [BytePart::Low, BytePart::High] {
                for destination_part in [BytePart::Low, BytePart::High] {
                    let catalog = PathCatalog::new(vec![vec![Statement::ImportPlayerMotionByte {
                        axis, part, destination: ByteField::WordPart {
                            field: WordField::MotionPhase, part: destination_part,
                        }, next: cursor(0, 0),
                    }]]).unwrap();
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.extension.path_state.motion_phase = 0xABCD;
                    actor.base.wait_timer = 193;
                    actor.base.velocity = Vector3 { x: -31, y: 1234, z: -32768 };
                    let initial = actor.clone();
                    let initial_random = random;
                    for inverted in [false, true] {
                        runtime.branch.invert_next = inverted;
                        let before_missing = objects.clone();
                        // Budget exhaustion is reported before consulting a
                        // missing snapshot; neither fault changes the actor.
                        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
                            Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                            Err(ProgramError::MissingPublishedMotion));
                        assert_eq!(objects, before_missing);
                        for bits in 0..=u16::MAX {
                            let mut delta = Vector3 { x: 13, y: 29, z: 43 };
                            match axis {
                                Axis::X => delta.x = bits as i16,
                                Axis::Y => delta.y = bits as i16,
                                Axis::Z => delta.z = bits as i16,
                            }
                            let snapshot = PublishedPlayerMotion {
                                position: Vector3 { x: -123, y: -456, z: -789 }, delta,
                            };
                            let mut inputs = world(&mut random);
                            inputs.published_motion = Some(snapshot);
                            let value = match part {
                                BytePart::Low => bits % 256,
                                BytePart::High => bits / 256,
                            };
                            let mut expected = initial.clone();
                            expected.extension.path_state.motion_phase = match destination_part {
                                BytePart::Low => 0xAB00 | value,
                                BytePart::High => value * 256 | 0xCD,
                            };
                            assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                                Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 1 }));
                            assert_eq!(objects.get(owner).unwrap(), &expected);
                            assert_eq!(inputs.published_motion, Some(snapshot));
                            assert_eq!(runtime.branch.invert_next, inverted);
                            assert_eq!(inputs.random, &initial_random);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn published_position_imports_do_not_substitute_live_poses_or_displacement() {
        use super::super::path_fields::Axis;
        use super::super::path_motion::PublishedPlayerMotion;
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            for inverted in [false, true] {
                let catalog = PathCatalog::new(vec![vec![Statement::ImportPlayerPosition {
                    axis, destination: WordField::ScriptValue, next: cursor(0, 0),
                }]]).unwrap();
                let (mut runtime, mut objects, owner, mut random) = setup();
                runtime.branch.invert_next = inverted;
                let actor = objects.get_mut(owner).unwrap();
                actor.base.position = super::super::Vector3 { x: 123, y: -321, z: 791 };
                actor.base.wait_timer = 179;
                let initial = actor.clone();
                let initial_random = random;
                let mut inputs = world(&mut random);
                inputs.selected = Some(owner);
                inputs.primary_player = Some(owner);
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::MissingPublishedMotion));
                assert_eq!(objects.get(owner).unwrap(), &initial);
                for bits in 0..=u16::MAX {
                    let mut position = super::super::Vector3 { x: 13, y: 29, z: 43 };
                    match axis { Axis::X => position.x = bits as i16, Axis::Y => position.y = bits as i16, Axis::Z => position.z = bits as i16 }
                    let snapshot = PublishedPlayerMotion { position, delta: super::super::Vector3 { x: -19, y: -41, z: -83 } };
                    inputs.published_motion = Some(snapshot);
                    let before = objects.get(owner).unwrap().clone();
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                        Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                    assert_eq!(objects.get(owner).unwrap(), &before);
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 1 }));
                    let mut expected = initial.clone();
                    expected.extension.path_state.script_value = bits;
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert_eq!(inputs.published_motion, Some(snapshot));
                    assert_eq!(inputs.random, &initial_random);
                    assert_eq!(runtime.branch.invert_next, inverted);
                }
            }
        }
    }

    #[test]
    fn published_motion_imports_preserve_words_and_require_the_snapshot_not_a_live_player() {
        use super::super::path_fields::Axis;
        use super::super::path_motion::PublishedPlayerMotion;
        use super::super::Vector3;
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            let catalog = PathCatalog::new(vec![vec![Statement::ImportPlayerMotion {
                axis,
                destination: WordField::ScriptValue,
                next: cursor(0, 0),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            let before = objects.get(owner).unwrap().clone();
            let original_random = random;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingPublishedMotion)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            for bits in 0..=u16::MAX {
                let mut delta = Vector3 {
                    x: 13,
                    y: 29,
                    z: 43,
                };
                match axis {
                    Axis::X => delta.x = bits as i16,
                    Axis::Y => delta.y = bits as i16,
                    Axis::Z => delta.z = bits as i16,
                }
                let snapshot = PublishedPlayerMotion {
                    position: Vector3::default(),
                    delta,
                };
                let mut inputs = world(&mut random);
                inputs.published_motion = Some(snapshot);
                let mut expected = before.clone();
                expected.extension.path_state.script_value = bits;
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 0),
                        executed: 1
                    })
                );
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert_eq!(inputs.published_motion, Some(snapshot));
                assert!(runtime.branch.invert_next);
                assert_eq!(inputs.random, &original_random);
            }
        }
    }

    #[test]
    fn complete_counter_motion_root_resamples_snapshot_and_selected_rotation_each_yield() {
        use super::super::path_motion::PublishedPlayerMotion;
        use super::super::{authored_paths, Angle, Vector3};
        let catalog = authored_paths::catalog();
        for selected_is_owner in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            let selected = if selected_is_owner {
                owner
            } else {
                objects
                    .allocate(Object::new(
                        ObjectKind::Player,
                        ShapeId::EMPTY,
                        Behavior::FollowPath,
                    ))
                    .unwrap()
            };
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::COUNTER_MOTION_EFFECT);
            actor.base.position = Vector3 {
                x: i16::MIN,
                y: 437,
                z: i16::MAX,
            };
            actor.base.velocity.y = -371;
            actor.base.wait_timer = 97;
            let original_random = random;
            for visit in 0..=u8::MAX {
                let target = objects.get_mut(selected).unwrap();
                target.base.pitch = Angle::from_units(visit);
                target.base.yaw = Angle::from_units(visit.wrapping_mul(3));
                target.base.roll = Angle::from_units(visit.wrapping_neg());
                let selected_before = target.clone();
                let delta = Vector3 {
                    x: (u16::from(visit) * 257) as i16,
                    y: 809,
                    z: (u16::from(visit) * 257).wrapping_add(32768) as i16,
                };
                let snapshot = PublishedPlayerMotion {
                    position: Vector3 { x: 3, y: 5, z: 7 },
                    delta,
                };
                let mut expected = objects.get(owner).unwrap().clone();
                expected.base.flags.collision_disabled = true;
                expected.base.flags.far_sort_bias = true;
                expected.base.velocity.x = delta.x.wrapping_neg();
                expected.base.velocity.z = delta.z.wrapping_neg();
                expected.base.pitch = Angle::from_units(64);
                expected.base.yaw = selected_before.base.yaw;
                expected.base.roll = Angle::from_units(128);
                expected.base.path = Some(PathCursor {
                    command_index: authored_paths::COUNTER_MOTION_EFFECT.command_index + 2,
                    ..authored_paths::COUNTER_MOTION_EFFECT
                });
                let mut inputs = world(&mut random);
                inputs.published_motion = Some(snapshot);
                inputs.selected = Some(selected);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 10),
                    Ok(ControlStep::Movement)
                );
                assert_eq!(objects.get(owner).unwrap(), &expected);
                if !selected_is_owner {
                    assert_eq!(objects.get(selected).unwrap(), &selected_before);
                }
                assert_eq!(inputs.published_motion, Some(snapshot));
                assert_eq!(inputs.random, &original_random);
                assert!(runtime.branch.invert_next);
            }
        }
    }

    #[test]
    fn positional_loop_writes_are_retained_not_queued_and_zero_disables_them() {
        use super::super::SpatialLoop;
        for initial in [
            None,
            Some(SpatialLoop::CapitalEngine),
            SpatialLoop::from_authored_control(255),
        ] {
            for value in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                runtime.branch.invert_next = true;
                let actor = objects.get_mut(owner).unwrap();
                actor.base.wait_timer = 59;
                actor.extension.spatial_loop = initial;
                actor.extension.animation_frame = 123;
                actor.extension.texture_scroll_x = 87;
                let initial_random = random.clone();
                let sound = SpatialLoop::from_authored_control(value);
                assert_eq!(sound.map_or(0, SpatialLoop::authored_control), value);
                assert_eq!(sound.is_none(), value == 0);
                let mut expected = actor.clone();
                expected.extension.spatial_loop = sound;
                expected.base.path = Some(cursor(0, 1));
                let catalog = PathCatalog::new(vec![vec![Statement::SpatialLoop {
                    sound,
                    next: cursor(0, 1),
                }]])
                .unwrap();
                // No cue queue or listener is required for a retained write.
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 1),
                        executed: 1
                    })
                );
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert_eq!(random, initial_random);
                assert!(runtime.branch.invert_next);
            }
        }
    }

    #[test]
    fn countdown_statements_require_shared_state_before_mutating_and_preserve_ifnot() {
        use super::super::path_countdown::{CountdownCommand, PathCountdown};
        use super::super::path_fields::ByteField;
        for command in [
            CountdownCommand::CopyTo(ByteField::Part),
            CountdownCommand::Assign(ByteOperand::Actor(ByteField::Part)),
            CountdownCommand::Increment,
            CountdownCommand::Decrement,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            objects.get_mut(owner).unwrap().extension.path_state.part = 211;
            let before = objects.get(owner).unwrap().clone();
            let original_random = random.clone();
            let catalog = PathCatalog::new(vec![vec![Statement::Countdown {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingCountdown)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
            let mut countdown = PathCountdown { remaining: 143 };
            let mut expected = before;
            let expected_value = match command {
                CountdownCommand::CopyTo(_) => {
                    expected.extension.path_state.part = 143;
                    143
                }
                CountdownCommand::Assign(_) => 211,
                CountdownCommand::Increment => 144,
                CountdownCommand::Decrement => 142,
            };
            expected.base.path = Some(cursor(0, 1));
            let mut inputs = world(&mut random);
            inputs.countdown = Some(&mut countdown);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert_eq!(countdown.remaining, expected_value);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn complete_shared_countdown_root_samples_live_state_and_stops_at_zero() {
        use super::super::path_countdown::PathCountdown;
        use super::super::{authored_paths, collision_pass::ExclusionGroups};
        let catalog = authored_paths::catalog();
        for initial in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let original_random = random.clone();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::SHARED_COUNTDOWN_SERVICE);
            actor.base.wait_timer = 63;
            actor.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(0xF8);
            actor.base.contacts.credits_hit_side = true;
            actor.base.contacts.mutually_non_damaging = true;
            actor.base.contacts.first_strategy_visit = true;
            actor.base.contacts.suppress_attack_damage = true;
            actor.base.velocity = super::super::Vector3 {
                x: 12,
                y: -31,
                z: 7,
            };
            let mut expected = actor.clone();
            expected.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(0xE8);
            expected.base.contacts.run_when_paused = true;
            expected.base.flags.visible = false;
            expected.base.flags.collision_disabled = true;
            expected.base.path = Some(PathCursor {
                command_index: authored_paths::SHARED_COUNTDOWN_SERVICE.command_index + 4,
                ..authored_paths::SHARED_COUNTDOWN_SERVICE
            });
            let mut countdown = PathCountdown { remaining: initial };
            for visit in 0..(u16::from(initial) + 3) {
                let before = countdown.remaining;
                expected.extension.path_state.part = before;
                let mut inputs = world(&mut random);
                inputs.countdown = Some(&mut countdown);
                assert_eq!(
                    runtime
                        .resume_program(&catalog, &mut objects, owner, &mut inputs, 8)
                        .unwrap(),
                    ControlStep::Movement
                );
                assert_eq!(countdown.remaining, before.saturating_sub(1));
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert_eq!(random, original_random);
                // Reassign the shared record after it has already reached
                // zero: the next loop must not reuse its old actor snapshot.
                if visit == u16::from(initial) + 1 {
                    countdown.remaining = 177;
                }
            }
        }
    }

    #[test]
    fn facing_statements_sample_live_targets_without_advancing_motion_or_other_state() {
        use super::super::path_steering::{face, FacingCommand, FacingTargets};
        use super::super::{Angle, Vector3};
        for command in [
            FacingCommand::SelectedImmediate,
            FacingCommand::SelectedSmooth,
            FacingCommand::SelectedYaw,
            FacingCommand::FixedPlayerImmediate,
            FacingCommand::LinkedSmooth,
            FacingCommand::LinkedImmediate,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let targets = [(100, 50, 0), (-100, -75, 10), (7, 200, -200)].map(|(x, y, z)| {
                let mut actor =
                    Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
                actor.base.position = Vector3 { x, y, z };
                objects.allocate(actor).unwrap()
            });
            let actor = objects.get_mut(owner).unwrap();
            actor.base.attachment = Some(targets[2]);
            actor.base.pitch = Angle::from_units(50);
            actor.base.yaw = Angle::from_units(70);
            actor.base.roll = Angle::from_units(90);
            actor.base.velocity = Vector3 {
                x: 11,
                y: -22,
                z: 33,
            };
            actor.base.wait_timer = 19;
            actor.base.hit_flags = 0xA5;
            runtime.branch.invert_next = true;
            runtime.steering.unchanged_axes = 7;
            let initial_random = random.clone();
            let catalog = PathCatalog::new(vec![vec![Statement::Facing {
                command,
                next: cursor(0, 0),
            }]])
            .unwrap();
            // Switch live selection AND move all candidate targets between
            // resumes. Fixed players remain distinct from the selected ID.
            for selected in [targets[0], targets[1], targets[2]] {
                let observations = FacingTargets {
                    selected: Some(selected),
                    primary: Some(targets[0]),
                    fixed_players: [Some(targets[1]), Some(targets[2])],
                };
                let mut expected_objects = objects.clone();
                let mut expected_steering = runtime.steering;
                face(
                    &mut expected_objects,
                    owner,
                    command,
                    observations,
                    &mut expected_steering,
                )
                .unwrap();
                let mut inputs = world(&mut random);
                inputs.selected = observations.selected;
                inputs.primary_player = observations.primary;
                inputs.fixed_players = observations.fixed_players;
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 0),
                        executed: 1
                    })
                );
                for id in [owner, targets[0], targets[1], targets[2]] {
                    assert_eq!(objects.get(id), expected_objects.get(id));
                }
                assert_eq!(runtime.steering, expected_steering);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, initial_random);
                for id in targets {
                    let position = &mut objects.get_mut(id).unwrap().base.position;
                    position.x = position.x.wrapping_add(200);
                    position.y = position.y.wrapping_sub(100);
                }
            }
        }
    }

    #[test]
    fn facing_missing_inputs_fault_but_absent_link_is_a_source_defined_advance() {
        use super::super::path_steering::{FacingCommand, SteeringError};
        for (command, error) in [
            (
                FacingCommand::SelectedImmediate,
                Some(SteeringError::MissingSelected),
            ),
            (
                FacingCommand::FixedPlayerImmediate,
                Some(SteeringError::MissingFixedPlayer),
            ),
            (FacingCommand::LinkedImmediate, None),
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.steering.unchanged_axes = 11;
            runtime.branch.invert_next = true;
            let mut expected = objects.get(owner).unwrap().clone();
            let catalog = PathCatalog::new(vec![vec![Statement::Facing {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            let result =
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1);
            if let Some(error) = error {
                assert_eq!(
                    result,
                    Err(ProgramError::Runtime(PathRuntimeError::Steering(error)))
                );
            } else {
                expected.base.path = Some(cursor(0, 1));
                assert_eq!(
                    result,
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 1),
                        executed: 1
                    })
                );
            }
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert_eq!(runtime.steering.unchanged_axes, 11);
            assert!(runtime.branch.invert_next);
        }
    }

    #[test]
    fn relationship_signals_feed_the_consuming_hit_branch_once_and_preserve_ifnot() {
        use super::super::path_relationships::RelationshipCommand;
        for command in [
            RelationshipCommand::SignalLinked,
            RelationshipCommand::SignalChild { number: 255 },
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let target = objects
                .allocate(objects.get(owner).unwrap().clone())
                .unwrap();
            objects.get_mut(owner).unwrap().base.attachment = Some(target);
            objects.get_mut(target).unwrap().base.first_child = Some(target);
            objects.get_mut(target).unwrap().base.child_number = 255;
            objects.get_mut(owner).unwrap().extension.parent = Some(owner);
            objects.get_mut(owner).unwrap().base.wait_timer = 99;
            let original_random = random;
            runtime.branch.invert_next = true;
            let catalog = PathCatalog::new(vec![vec![
                Statement::Relationship {
                    command,
                    next: cursor(0, 1),
                },
                Statement::Branch(BranchCommand::HitEvent {
                    taken: cursor(0, 2),
                    next: cursor(0, 3),
                }),
            ]])
            .unwrap();
            let mut expected = objects.clone();
            expected.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
            expected
                .get_mut(target)
                .unwrap()
                .extension
                .path_state
                .conditions
                .hit_event_pending = true;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(objects, expected);
            for destination in [cursor(0, 2), cursor(0, 3)] {
                objects.get_mut(target).unwrap().base.path = Some(cursor(0, 1));
                let mut expected = objects.clone();
                expected
                    .get_mut(target)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .hit_event_pending = false;
                expected.get_mut(target).unwrap().base.path = Some(destination);
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        target,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::BudgetExceeded {
                        cursor: destination,
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, original_random);
            }
        }
    }

    #[test]
    fn child_missing_dispatch_preserves_ifnot_and_faults_before_cursor_mutation() {
        use super::super::path_relationships::RelationshipError;
        let (mut runtime, mut objects, owner, mut random) = setup();
        let parent = objects
            .allocate(objects.get(owner).unwrap().clone())
            .unwrap();
        let child = objects
            .allocate(objects.get(owner).unwrap().clone())
            .unwrap();
        objects.get_mut(child).unwrap().base.child_number = 128;
        objects.get_mut(parent).unwrap().base.first_child = Some(child);
        runtime.branch.invert_next = true;
        let original_random = random;
        for refresh in [false, true] {
            for mother in [None, Some(parent)] {
                for number in [0, 127, 128, 255] {
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(cursor(0, 0));
                    actor.base.attachment = mother;
                    actor.base.wait_timer = 99;
                    actor.extension.path_state.motion.refresh_child_chain = refresh;
                    actor.extension.path_state.conditions.hit_event_pending = true;
                    // Self-owned search is empty even though mother's is not.
                    let missing = refresh || (mother.is_some() && number != 128);
                    let destination = cursor(0, if missing { 2 } else { 1 });
                    let catalog = PathCatalog::new(vec![vec![Statement::ChildMissing {
                        number,
                        taken: cursor(0, 2),
                        next: cursor(0, 1),
                    }]])
                    .unwrap();
                    let mut expected = objects.clone();
                    expected.get_mut(owner).unwrap().base.path = Some(destination);
                    assert_eq!(
                        runtime.resume_program(
                            &catalog,
                            &mut objects,
                            owner,
                            &mut world(&mut random),
                            1
                        ),
                        Err(ProgramError::BudgetExceeded {
                            cursor: destination,
                            executed: 1
                        })
                    );
                    assert_eq!(objects, expected);
                    assert!(runtime.branch.invert_next);
                }
            }
        }
        objects.remove(parent).unwrap();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(cursor(0, 0));
        actor.base.attachment = Some(parent);
        actor.extension.path_state.motion.refresh_child_chain = false;
        let before = objects.clone();
        for statement in [
            Statement::ChildMissing {
                number: 128,
                taken: cursor(0, 2),
                next: cursor(0, 1),
            },
            Statement::Relationship {
                command: super::super::path_relationships::RelationshipCommand::SignalLinked,
                next: cursor(0, 1),
            },
            Statement::Relationship {
                command: super::super::path_relationships::RelationshipCommand::SignalChild {
                    number: 128,
                },
                next: cursor(0, 1),
            },
        ] {
            let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::Relationship(RelationshipError::MissingActor(
                    parent
                )))
            );
            assert_eq!(objects, before);
            assert!(runtime.branch.invert_next);
        }
        assert_eq!(random, original_random);
    }

    #[test]
    fn linked_rotation_refresh_advances_without_selection_and_faults_on_dangling_link() {
        use super::super::path_relationships::{RelationshipCommand, RelationshipError};
        use super::super::{Angle, Rotation};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let linked = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let original_random = random;
        runtime.branch.invert_next = true;
        let catalog = PathCatalog::new(vec![vec![Statement::Relationship {
            command: RelationshipCommand::RefreshLinkedRotation,
            next: cursor(0, 1),
        }]])
        .unwrap();
        for link in [Some(linked), None, Some(owner), Some(linked)] {
            let target = objects.get_mut(linked).unwrap();
            target.base.pitch = Angle::from_units(90);
            target.base.yaw = Angle::from_units(160);
            target.base.roll = Angle::from_units(230);
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(cursor(0, 0));
            actor.base.attachment = link;
            actor.base.pitch = Angle::from_units(10);
            actor.base.yaw = Angle::from_units(20);
            actor.base.roll = Angle::from_units(30);
            actor.base.wait_timer = 99;
            actor.extension.relative_rotation = Rotation {
                pitch: Angle::from_units(1),
                yaw: Angle::from_units(2),
                roll: Angle::from_units(3),
            };
            let mut expected = actor.clone();
            if link == Some(linked) {
                expected.extension.relative_rotation = Rotation {
                    pitch: Angle::from_units(176),
                    yaw: Angle::from_units(116),
                    roll: Angle::from_units(56),
                };
            } else if link == Some(owner) {
                expected.extension.relative_rotation = Rotation::default();
            }
            expected.base.path = Some(cursor(0, 1));
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert!(runtime.branch.invert_next);
        }
        objects.remove(linked).unwrap();
        // Normal removal clears inbound links. Deliberately construct an
        // invalid retained link to exercise the diagnostic boundary.
        objects.get_mut(owner).unwrap().base.attachment = Some(linked);
        objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
        let before = objects.get(owner).unwrap().clone();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::Relationship(RelationshipError::MissingActor(
                linked
            )))
        );
        assert_eq!(objects.get(owner).unwrap(), &before);
        assert_eq!(random, original_random);
    }

    #[test]
    fn relative_reference_controls_advance_without_reselecting_or_changing_motion_flags() {
        use super::super::path_relationships::RelationshipCommand;
        use super::super::{Angle, Rotation, Vector3};
        for command in [
            RelationshipCommand::ClearRelativeReference,
            RelationshipCommand::UseSelfRelativeFrame,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.extension.parent = Some(owner);
            actor.base.attachment = Some(owner);
            actor.extension.path_state.motion.relative_coordinates = true;
            actor.extension.path_state.motion.attached_coordinates = true;
            actor.extension.relative_position = Vector3 {
                x: 107,
                y: -939,
                z: i16::MIN,
            };
            actor.extension.relative_rotation = Rotation {
                pitch: Angle::from_units(50),
                yaw: Angle::from_units(75),
                roll: Angle::from_units(100),
            };
            let mut expected = actor.clone();
            if command == RelationshipCommand::ClearRelativeReference {
                expected.extension.parent = None;
            } else {
                expected.extension.relative_position = Vector3::default();
                expected.extension.relative_rotation = Rotation::default();
            }
            expected.base.path = Some(cursor(0, 1));
            runtime.branch.invert_next = true;
            let initial_random = random;
            let catalog = PathCatalog::new(vec![vec![Statement::Relationship {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, initial_random);
            assert_eq!(runtime.selected_player(), PlayerTarget::Primary);
        }
    }

    #[test]
    fn selected_transform_copies_read_live_selection_and_preserve_neighboring_fields() {
        use super::super::path_relationships::{RelationshipError, SelectedTransformCommand};
        use super::super::{Angle, Vector3};
        for command in [
            SelectedTransformCommand::WorldPosition,
            SelectedTransformCommand::WorldRotation,
            SelectedTransformCommand::RelativeFrame,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let other = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.child_number = 73;
            actor.base.wait_timer = 57;
            actor.base.position = Vector3 {
                x: 100,
                y: -200,
                z: 300,
            };
            actor.base.velocity = Vector3 { x: 1, y: -2, z: 3 };
            actor.extension.relative_position = Vector3 {
                x: 40,
                y: -50,
                z: 60,
            };
            actor.extension.path_state.motion.relative_coordinates = true;
            actor.extension.parent = Some(other);
            runtime.branch.invert_next = true;
            let original_random = random;
            let catalog = PathCatalog::new(vec![vec![Statement::CopySelectedTransform {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            let before = objects.get(owner).unwrap().clone();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::Relationship(
                    RelationshipError::MissingSelected
                ))
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            let removed = objects.allocate(before.clone()).unwrap();
            objects.remove(removed).unwrap();
            let mut inputs = world(&mut random);
            inputs.selected = Some(removed);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::Relationship(RelationshipError::MissingActor(
                    removed
                )))
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            for angle in 0..=u8::MAX {
                let target = objects.get_mut(other).unwrap();
                target.base.position = Vector3 {
                    x: (u16::from(angle) * 257) as i16,
                    y: i16::MIN,
                    z: i16::MAX,
                };
                target.base.pitch = Angle::from_units(angle);
                target.base.yaw = Angle::from_units(angle.wrapping_add(100));
                target.base.roll = Angle::from_units(angle.wrapping_sub(50));
                // Child number and elapsed wait sit between source angle
                // bytes, so a bulk rotation-word copy would be incorrect.
                target.base.child_number = angle;
                target.base.wait_timer = angle;
                for selected in [other, owner] {
                    objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
                    let mut expected = objects.get(owner).unwrap().clone();
                    let target = objects.get(selected).unwrap();
                    match command {
                        SelectedTransformCommand::WorldPosition => {
                            expected.base.position = target.base.position
                        }
                        SelectedTransformCommand::WorldRotation => {
                            expected.base.pitch = target.base.pitch;
                            expected.base.yaw = target.base.yaw;
                            expected.base.roll = target.base.roll;
                        }
                        SelectedTransformCommand::RelativeFrame => {
                            let (x, y, z) = sf_core::snes_trig::matrix_rotate_q15(
                                sf_core::snes_trig::zxy_matrix_q15(
                                    target.base.pitch.units().wrapping_neg(),
                                    target.base.yaw.units().wrapping_neg(),
                                    target.base.roll.units().wrapping_neg(),
                                ),
                                expected
                                    .base
                                    .position
                                    .x
                                    .wrapping_sub(target.base.position.x),
                                expected
                                    .base
                                    .position
                                    .y
                                    .wrapping_sub(target.base.position.y),
                                expected
                                    .base
                                    .position
                                    .z
                                    .wrapping_sub(target.base.position.z),
                            );
                            expected.extension.parent = Some(selected);
                            expected.extension.relative_position = Vector3 { x, y, z };
                            expected.extension.relative_rotation = super::super::Rotation {
                                pitch: Angle::from_units(
                                    expected
                                        .base
                                        .pitch
                                        .units()
                                        .wrapping_sub(target.base.pitch.units()),
                                ),
                                yaw: Angle::from_units(
                                    expected
                                        .base
                                        .yaw
                                        .units()
                                        .wrapping_sub(target.base.yaw.units()),
                                ),
                                roll: Angle::from_units(
                                    expected
                                        .base
                                        .roll
                                        .units()
                                        .wrapping_sub(target.base.roll.units()),
                                ),
                            };
                            expected.extension.path_state.motion.relative_coordinates = true;
                        }
                    }
                    expected.base.path = Some(cursor(0, 1));
                    let other_before = objects.get(other).unwrap().clone();
                    let mut inputs = world(&mut random);
                    inputs.selected = Some(selected);
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(0, 1),
                            executed: 1
                        })
                    );
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert_eq!(objects.get(other).unwrap(), &other_before);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, original_random);
                }
            }
        }
    }

    #[test]
    fn waiting_chase_checks_equality_before_update_and_does_not_reset_elapsed_wait() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![Statement::WaitChase {
            field: ByteField::ScriptParameter,
            target: ByteOperand::Actor(ByteField::Health),
            next: cursor(0, 1),
        }]])
        .unwrap();
        let original_random = random;
        runtime.branch.invert_next = true;
        for current in 0..=u8::MAX {
            for target in 0..=u8::MAX {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(cursor(0, 0));
                actor.extension.path_state.script_parameter = current;
                actor.base.hit_points = target;
                actor.base.wait_timer = 57;
                actor.base.velocity.x = 123;
                let mut expected = actor.clone();
                expected.extension.path_state.script_parameter =
                    super::super::path_fields::chase_byte(current, target);
                let result = runtime.resume_program(
                    &catalog,
                    &mut objects,
                    owner,
                    &mut world(&mut random),
                    1,
                );
                if current == target {
                    expected.base.path = Some(cursor(0, 1));
                    assert_eq!(
                        result,
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(0, 1),
                            executed: 1
                        })
                    );
                } else {
                    // Even a final one-unit step yields at the same cursor.
                    assert_eq!(result, Ok(ControlStep::Movement));
                }
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, original_random);
            }
        }
    }

    #[test]
    fn primary_horizontal_inheritance_samples_mode_and_motion_without_movement() {
        use super::super::Vector3;
        for own_primary in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let other = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            let primary = if own_primary { owner } else { other };
            objects.get_mut(other).unwrap().base.velocity = Vector3 {
                x: 2345,
                y: -32,
                z: -6789,
            };
            let actor = objects.get_mut(owner).unwrap();
            actor.base.velocity = Vector3 {
                x: i16::MAX,
                y: 42,
                z: i16::MIN,
            };
            actor.base.position = Vector3 {
                x: 101,
                y: 202,
                z: 303,
            };
            actor.base.wait_timer = 57;
            runtime.branch.invert_next = true;
            let initial_random = random.clone();
            let catalog = PathCatalog::new(vec![vec![Statement::InheritPrimaryHorizontalMotion {
                next: cursor(0, 0),
            }]])
            .unwrap();
            for mode in 0..=u8::MAX {
                let displacement = Vector3 {
                    x: i16::MIN + i16::from(mode),
                    y: 999,
                    z: i16::MAX - i16::from(mode),
                };
                let mut expected = objects.get(owner).unwrap().clone();
                let other_before = objects.get(other).unwrap().clone();
                let addition = if (16..32).contains(&mode) {
                    objects.get(primary).unwrap().base.velocity
                } else {
                    displacement
                };
                expected.base.velocity.x = expected.base.velocity.x.wrapping_add(addition.x);
                expected.base.velocity.z = expected.base.velocity.z.wrapping_add(addition.z);
                let mut inputs = world(&mut random);
                inputs.primary_player = Some(primary);
                // Deliberately select a different actor: this action must
                // always use primary_player, including owner/primary aliasing.
                inputs.selected = Some(if own_primary { other } else { owner });
                inputs.primary_motion = Some(PrimaryMotionInput {
                    auxiliary_mode: mode,
                    displacement,
                });
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 0),
                        executed: 1
                    })
                );
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert_eq!(objects.get(other).unwrap(), &other_before);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, initial_random);
            }
        }
    }

    #[test]
    fn inheritance_missing_world_inputs_fault_before_mutating_actor() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![Statement::InheritPrimaryHorizontalMotion {
            next: cursor(0, 1),
        }]])
        .unwrap();
        let before = objects.get(owner).unwrap().clone();
        for (primary, error) in [
            (None, ProgramError::MissingPrimaryPlayer),
            (Some(owner), ProgramError::MissingPrimaryMotion),
        ] {
            let mut inputs = world(&mut random);
            inputs.primary_player = primary;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(error)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
        }
    }

    fn spawn_statement() -> Statement {
        Statement::SpawnChild {
            kind: ObjectKind::Effect,
            parameters: super::super::path_spawn::ChildSpawn {
                shape: ShapeId::from_catalog_index(9),
                path: Some(cursor(1, 0)),
                position: super::super::Vector3 {
                    x: -123,
                    y: 456,
                    z: -789,
                },
                rotation: super::super::Rotation::default(),
                hit_points: 1,
                attack_power: 2,
                number: 3,
            },
            next: cursor(0, 1),
        }
    }

    #[test]
    fn shape_assignment_and_equality_use_semantic_ids_without_reinitializing_actor() {
        use super::super::path_appearance::AppearanceCommand;
        let (mut runtime, mut objects, owner, mut random) = setup();
        let original_random = random;
        for index in 0..577 {
            for equal in [false, true] {
                for inverted in [false, true] {
                    let shape = ShapeId::from_catalog_index(index);
                    let other =
                        ShapeId::from_catalog_index(if equal { index } else { (index + 1) % 577 });
                    let catalog = PathCatalog::new(vec![vec![
                        Statement::Appearance {
                            command: AppearanceCommand::Shape(shape),
                            next: cursor(0, 1),
                        },
                        Statement::Compare {
                            condition: ActorCondition::EqualShape(other),
                            taken: cursor(0, 2),
                            next: cursor(0, 3),
                        },
                        Statement::Control(ControlCommand::End),
                        Statement::Control(ControlCommand::End),
                    ]])
                    .unwrap();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(cursor(0, 0));
                    actor.base.flags.visible = false;
                    actor.base.flags.scaled_sprite = true;
                    actor.base.flags.casts_shadow = true;
                    actor.base.flags.collision_disabled = true;
                    actor.extension.texture_scroll_x = 67;
                    actor.extension.animation_frame = 12;
                    actor.extension.path_state.motion_phase = 0xABCD;
                    let mut expected = actor.clone();
                    expected.base.shape = shape;
                    let next = cursor(0, if equal != inverted { 2 } else { 3 });
                    expected.base.path = Some(next);
                    runtime.branch.invert_next = inverted;
                    assert_eq!(
                        runtime.resume_program(
                            &catalog,
                            &mut objects,
                            owner,
                            &mut world(&mut random),
                            2
                        ),
                        Err(ProgramError::BudgetExceeded {
                            cursor: next,
                            executed: 2
                        })
                    );
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert!(!runtime.branch.invert_next);
                    assert_eq!(random, original_random);
                }
            }
        }
    }

    #[test]
    fn appearance_controls_advance_immediately_without_consuming_contact_or_inversion() {
        use super::super::path_appearance::AppearanceCommand;
        let commands = [
            AppearanceCommand::Visibility(false),
            AppearanceCommand::Collision(true),
            AppearanceCommand::Shadow(true),
            AppearanceCommand::MaximumDrawDistance(true),
            AppearanceCommand::Visibility(true),
            AppearanceCommand::Shadow(false),
            AppearanceCommand::MaximumDrawDistance(false),
            AppearanceCommand::FarSortBias(true),
            AppearanceCommand::FarSortBias(false),
        ];
        let mut statements = commands
            .iter()
            .enumerate()
            .map(|(index, command)| Statement::Appearance {
                command: *command,
                next: cursor(0, index as u16 + 1),
            })
            .collect::<Vec<_>>();
        statements.push(Statement::Control(ControlCommand::End));
        let catalog = PathCatalog::new(vec![statements]).unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        runtime.branch.invert_next_condition();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.flags.draw_list_admitted = true;
        actor.base.flags.collided = true;
        actor.base.hit_flags = 0xA5;
        actor.base.wait_timer = 19;
        actor.extension.path_state.conditions.hit_event_pending = true;
        let original_random = random;
        for (index, expected_flags) in [
            (false, true, false, false),
            (false, false, false, false),
            (false, false, true, false),
            (false, false, true, true),
            (true, false, true, true),
            (true, false, false, true),
            (true, false, false, false),
            (true, false, false, false),
            (true, false, false, false),
        ]
        .into_iter()
        .enumerate()
        {
            let mut expected = objects.get(owner).unwrap().clone();
            let flags = &mut expected.base.flags;
            (
                flags.visible,
                flags.collision_disabled,
                flags.casts_shadow,
                flags.maximum_draw_distance,
            ) = expected_flags;
            flags.far_sort_bias = index == 7;
            let next = cursor(0, index as u16 + 1);
            expected.base.path = Some(next);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: next,
                    executed: 1
                })
            );
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn clipping_plane_selection_is_immediate_and_preserves_all_other_actor_state() {
        use super::super::path_appearance::AppearanceCommand;
        use super::super::render::ClippingPlaneSelection;
        let catalog = PathCatalog::new(vec![vec![Statement::Appearance {
            command: AppearanceCommand::ClippingPlane(ClippingPlaneSelection::FIRST),
            next: cursor(0, 1),
        }]]).unwrap();
        for previous in 0..=u8::MAX {
            for inverted in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                runtime.branch.invert_next = inverted;
                let original_random = random;
                let actor = objects.get_mut(owner).unwrap();
                actor.extension.clipping_plane = ClippingPlaneSelection::from_selector_byte(previous);
                actor.extension.depth_offset = 0xABCD;
                actor.base.flags.visible = previous & 1 != 0;
                actor.base.flags.collision_disabled = previous & 2 != 0;
                actor.base.flags.maximum_draw_distance = previous & 4 != 0;
                actor.base.wait_timer = previous;
                actor.base.hit_flags = previous;
                actor.extension.path_state.conditions.hit_event_pending = true;
                let original = objects.clone();
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
                    Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                assert_eq!(objects, original);
                let mut expected = objects.get(owner).unwrap().clone();
                expected.extension.clipping_plane = ClippingPlaneSelection::FIRST;
                expected.base.path = Some(cursor(0, 1));
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                    Err(ProgramError::BudgetExceeded { cursor: cursor(0, 1), executed: 1 }));
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert_eq!(runtime.branch.invert_next, inverted);
                assert_eq!(random, original_random);
            }
        }
    }

    #[test]
    fn authored_weapon_selection_assignment_preserves_classification_wait_and_all_other_state() {
        use super::super::WeaponKind;
        for value in 0..=u8::MAX {
            for inverted in [false, true] {
                for kind in [WeaponKind::None, WeaponKind::Laser, WeaponKind::ChargedLaser,
                             WeaponKind::NovaBomb, WeaponKind::EnemyLaser, WeaponKind::Missile] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.weapon = kind;
                    actor.base.wait_timer = value;
                    actor.base.hit_points = value ^ 255;
                    actor.base.attack_power = value;
                    actor.extension.path_state.weapon_selection = value ^ 255;
                    actor.extension.path_state.motion_phase = 0xABCD;
                    let mut expected = actor.clone();
                    runtime.branch.invert_next = inverted;
                    let initial_random = random;
                    let catalog = PathCatalog::new(vec![vec![Statement::Mutate {
                        mutation: Mutation::Byte { field: ByteField::WeaponSelection,
                            operation: ByteOperation::Assign(ByteOperand::Literal(value)) },
                        next: cursor(0, 1),
                    }]]).unwrap();
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
                        Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    expected.extension.path_state.weapon_selection = value;
                    expected.base.path = Some(cursor(0, 1));
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                        Err(ProgramError::BudgetExceeded { cursor: cursor(0, 1), executed: 1 }));
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert_eq!(ByteField::WeaponSelection.read(objects.get(owner).unwrap()), value);
                    assert_eq!(runtime.branch.invert_next, inverted);
                    assert_eq!(random, initial_random);
                }
            }
        }
    }

    #[test]
    fn clipping_operand_and_fixed_command_alias_one_full_byte() {
        use super::super::path_appearance::AppearanceCommand;
        use super::super::render::ClippingPlaneSelection;
        for value in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            let original_random = random;
            let catalog = PathCatalog::new(vec![vec![
                Statement::Mutate { mutation: Mutation::Byte {
                    field: ByteField::ClippingPlane,
                    operation: ByteOperation::Assign(ByteOperand::Literal(value)),
                }, next: cursor(0, 1) },
                Statement::Appearance {
                    command: AppearanceCommand::ClippingPlane(ClippingPlaneSelection::FIRST),
                    next: cursor(0, 2),
                },
            ]]).unwrap();
            for (index, expected_byte) in [value, 1].into_iter().enumerate() {
                let mut expected = objects.get(owner).unwrap().clone();
                expected.extension.clipping_plane = ClippingPlaneSelection::from_selector_byte(expected_byte);
                let next = cursor(0, index as u16 + 1);
                expected.base.path = Some(next);
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                    Err(ProgramError::BudgetExceeded { cursor: next, executed: 1 }));
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert_eq!(ByteField::ClippingPlane.read(objects.get(owner).unwrap()), expected_byte);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, original_random);
            }
        }
    }

    #[test]
    fn scenery_transfers_preserve_full_bytes_and_fault_before_mutation() {
        use super::super::path_fields::BytePart;
        let field = ByteField::WordPart { field: WordField::MotionPhase, part: BytePart::Low };
        for importing in [false, true] {
            let command = if importing { SceneryDistanceCommand::CopyTo(field) }
                else { SceneryDistanceCommand::Assign(ByteOperand::Actor(field)) };
            let catalog = PathCatalog::new(vec![vec![Statement::SceneryDistance { command, next: cursor(0, 1) }]]).unwrap();
            for value in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                runtime.branch.invert_next = true;
                let actor = objects.get_mut(owner).unwrap();
                actor.extension.path_state.motion_phase = 0xA500 | u16::from(value);
                actor.base.wait_timer = 251;
                let initial = objects.clone();
                let initial_random = random;
                let mut scenery = SceneryDistanceState { near_mask: value ^ 0xFF };
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                    Err(ProgramError::MissingSceneryDistance));
                assert_eq!(objects, initial);
                let mut inputs = world(&mut random);
                inputs.scenery_distance = Some(&mut scenery);
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                    Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                assert_eq!(objects, initial);
                assert_eq!(inputs.scenery_distance.as_ref().unwrap().near_mask, value ^ 0xFF);
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded { cursor: cursor(0, 1), executed: 1 }));
                let mut expected = initial;
                let actor = expected.get_mut(owner).unwrap();
                actor.base.path = Some(cursor(0, 1));
                if importing { actor.extension.path_state.motion_phase = 0xA500 | u16::from(value ^ 0xFF); }
                assert_eq!(objects, expected);
                assert_eq!(scenery.near_mask, if importing { value ^ 0xFF } else { value });
                assert!(runtime.branch.invert_next);
                assert_eq!(random, initial_random);
            }
        }
    }

    #[test]
    fn distance_scenery_keeps_hysteresis_all_selectors_and_word_sized_bit_mutation() {
        use super::super::{authored_paths, path_fields::WordOperation};
        use crate::{Angle, Vector3};
        let catalog = authored_paths::catalog();
        // Source table contents are independently verified by the static
        // extractor tests. Here check their end-to-end use by both graphs.
        let masks = catalog.paths.iter().flatten().find_map(|statement| match statement {
            Statement::Mutate { mutation: Mutation::Word { operation: WordOperation::SetBits(WordOperand::IndexedBitMask { masks, .. }), .. }, .. } => Some(*masks),
            _ => None,
        }).unwrap();
        for root in [authored_paths::DISTANCE_GATED_SCENERY, authored_paths::HEALTH_ROTATED_DISTANCE_SCENERY] {
            for selector in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let selected = objects.allocate(Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight)).unwrap();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(root);
                actor.base.position.y = (0xAB00 | u16::from(selector)) as i16;
                actor.base.hit_points = selector ^ 0xFF;
                actor.base.yaw = Angle::from_units(37);
                actor.extension.path_state.motion_phase = 0xA57E;
                actor.base.flags.exclude_from_shape_footprint_search = true;
                let initial_random = random;
                let mut scenery = SceneryDistanceState { near_mask: 0x5A };
                let mut expected_mask = scenery.near_mask;
                let mut phase = 0xA57E;
                let mask = masks[usize::from(selector.wrapping_sub(1) & 0x7F)];
                let mut inputs = world(&mut random);
                inputs.scene.player_configuration = Some(0);
                inputs.selected = Some(selected);
                if selector != 0 { inputs.scenery_distance = Some(&mut scenery); }
                // Far side includes 512. Once near, 512..599 retain near;
                // once far, 512..599 retain far. Equality at 600 exits near.
                for (visit, (distance, near)) in [
                    (600, false), (599, false), (512, false), (511, true),
                    (512, true), (599, true), (600, false), (511, true),
                ].into_iter().enumerate() {
                    objects.get_mut(selected).unwrap().base.position.x = distance;
                    let result = if visit == 0 {
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 60)
                    } else {
                        // The previous GOTO already selected the next loop.
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 15)
                    };
                    assert_eq!(result, Ok(ControlStep::Movement), "selector {selector}, visit {visit}");
                    if selector != 0 {
                        phase = (phase & 0xFF00) | u16::from(expected_mask);
                        phase = if near { phase | mask } else { phase & !mask };
                        expected_mask = phase as u8;
                        assert_eq!(inputs.scenery_distance.as_ref().unwrap().near_mask, expected_mask);
                    }
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.extension.path_state.motion_phase, phase);
                    assert_eq!(actor.base.flags.far_sort_bias, !near);
                    assert_eq!(actor.extension.path_state.script_parameter, selector);
                    assert_eq!(actor.base.position, Vector3::default());
                    assert_eq!(actor.base.yaw.units(), if root == authored_paths::HEALTH_ROTATED_DISTANCE_SCENERY { selector ^ 0xFF } else { 37 });
                    assert_eq!((actor.base.hit_points, actor.base.attack_power), (100, 4));
                    assert!(actor.base.flags.maximum_draw_distance);
                    assert!(actor.base.flags.collision_disabled);
                    assert!(!actor.base.flags.exclude_from_shape_footprint_search);
                    assert!(!actor.base.flags.casts_shadow);
                    assert!(actor.base.contacts.suppress_contacts_next_epoch);
                    assert!(!actor.base.flags.strategy_suspended);
                    assert!(!runtime.branch.invert_next);
                }
                assert_eq!(random, initial_random);
            }
        }
    }

    #[test]
    fn scenery_import_and_export_are_separate_resumable_commands_not_an_atomic_mask_update() {
        use super::super::authored_paths;
        let catalog = authored_paths::catalog();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let selected = objects.allocate(Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight)).unwrap();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::DISTANCE_GATED_SCENERY);
        actor.base.position.y = 9;
        actor.extension.path_state.motion_phase = 0xA47E;
        let mut inputs = world(&mut random);
        inputs.scene.player_configuration = Some(0);
        inputs.selected = Some(selected);
        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 60),
            Err(ProgramError::MissingSceneryDistance));
        let before = objects.clone();
        let failed = objects.get(owner).unwrap().base.path.unwrap();
        assert!(matches!(catalog.statement(failed), Ok(Statement::SceneryDistance { command: SceneryDistanceCommand::CopyTo(_), .. })));
        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::MissingSceneryDistance));
        assert_eq!(objects, before);
        let mut scenery = SceneryDistanceState { near_mask: 0x33 };
        inputs.scenery_distance = Some(&mut scenery);
        let result = runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 2);
        let exporting = objects.get(owner).unwrap().base.path.unwrap();
        assert_eq!(result, Err(ProgramError::BudgetExceeded { cursor: exporting, executed: 2 }));
        assert!(matches!(catalog.statement(exporting), Ok(Statement::SceneryDistance { command: SceneryDistanceCommand::Assign(_), .. })));
        assert_eq!(objects.get(owner).unwrap().extension.path_state.motion_phase, 0xA533);
        let paused_scene = inputs.scenery_distance.take().unwrap();
        let before = objects.clone();
        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::MissingSceneryDistance));
        assert_eq!(objects, before);
        // An intervening scene writer changes the shared byte, but the
        // pending export must use the actor's earlier imported value.
        paused_scene.near_mask = 0x88;
        inputs.scenery_distance = Some(paused_scene);
        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 2), Ok(ControlStep::Movement));
        assert_eq!(scenery.near_mask, 0x33);
    }

    #[test]
    fn weapon_level_branch_compares_all_literal_bytes_to_published_level_and_preserves_ifnot() {
        for expected in 0..=u8::MAX {
            let catalog = PathCatalog::new(vec![vec![Statement::ActiveWeaponLevelEquals {
                expected, taken: cursor(0, 1), next: cursor(0, 2),
            }]]).unwrap();
            for actual in 0..=u8::MAX {
                for invert in [false, true] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let selected = objects.allocate(Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight)).unwrap();
                    objects.get_mut(selected).unwrap().base.hit_points = actual ^ 255;
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.hit_points = expected ^ 255;
                    actor.base.attack_power = actual ^ 255;
                    actor.base.wait_timer = 193;
                    actor.extension.path_state.motion_phase = 0xABCD;
                    runtime.branch.invert_next = invert;
                    let mut expected_objects = objects.clone();
                    let destination = cursor(0, if actual == expected { 1 } else { 2 });
                    expected_objects.get_mut(owner).unwrap().base.path = Some(destination);
                    let initial_random = random;
                    let mut inputs = world(&mut random);
                    inputs.selected = Some(selected);
                    inputs.scene.active_weapon_level = Some(actual);
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded { cursor: destination, executed: 1 }));
                    assert_eq!(objects, expected_objects);
                    assert_eq!(runtime.branch.invert_next, invert);
                    assert_eq!(random, initial_random);
                }
            }
        }
    }

    #[test]
    fn weapon_level_branch_faults_before_mutation_and_reads_changed_publication_on_resume() {
        let catalog = PathCatalog::new(vec![vec![Statement::ActiveWeaponLevelEquals {
            expected: 3, taken: cursor(0, 0), next: cursor(0, 1),
        }]]).unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        runtime.branch.invert_next = true;
        let initial = objects.clone();
        let initial_random = random;
        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
            Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
        for _ in 0..2 {
            assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingSceneByte(SceneByte::ActiveWeaponLevel)));
            assert_eq!(objects, initial);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, initial_random);
        }
        for (actual, destination) in [(3, cursor(0, 0)), (3, cursor(0, 0)), (255, cursor(0, 1))] {
            let mut inputs = world(&mut random);
            inputs.scene.active_weapon_level = Some(actual);
            assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded { cursor: destination, executed: 1 }));
            let mut expected = initial.clone();
            expected.get_mut(owner).unwrap().base.path = Some(destination);
            assert_eq!(objects, expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, initial_random);
        }
    }

    #[test]
    fn motion_fade_sprite_entries_preserve_mode_gating_wrapped_audio_and_saved_phase_callbacks() {
        use super::super::path_motion::PublishedPlayerMotion;
        use super::super::path_sound::{AuthoredCue, CueListener, CueMarker, MarkerInputs, PathAudio};
        use super::super::{authored_paths, Angle, AudioState, SoundEvent, Vector3};
        let catalog = authored_paths::catalog();
        let roots = [authored_paths::PHASE_INCREMENTED_MOTION_FADE_SPRITE,
            authored_paths::RANDOM_SIZE_MOTION_FADE_SPRITE, authored_paths::SMALL_RANDOM_MOTION_FADE_SPRITE];
        for (entry, root) in roots.into_iter().enumerate() {
            for initial in 0..=u8::MAX {
                for mode in [0x1Fu8, 0x2F, 0x3F, 0xFF] {
                    for inverted in [false, true] {
                        for skip_jitter in [false, true] {
                            for health in [1u8, 100] {
                                let (mut runtime, mut objects, owner, _) = setup();
                                let mut random = RandomState::new([initial, mode, 93, 253]);
                                let mut expected_random = random;
                                let power = match entry {
                                    0 => initial,
                                    1 => initial.wrapping_add(expected_random.next_byte() & 31),
                                    _ => expected_random.next_byte() & 15,
                                };
                                let size = power.wrapping_add(8);
                                // Mode-class branches bypass IFNOT; the later
                                // health equality consumes it before audio.
                                let cue = if (health == 1) != inverted { None }
                                    else { Some(if (17u8.wrapping_sub(size) as i8) < 0 { 112 } else { 139 }) };
                                let has_callback = !matches!(mode & 0xF0, 0x20 | 0x30);
                                let initial_high = if skip_jitter { if entry == 0 { 0 } else { 1 } } else { 255u8 };
                                let high = if entry == 0 { initial_high.wrapping_add(1) } else { initial_high };
                                let initial_position = Vector3 { x: i16::MAX, y: i16::MIN, z: -1234 };
                                let mut position = initial_position;
                                let mut low = 17;
                                if !skip_jitter {
                                    for coordinate in [&mut position.x, &mut position.y, &mut position.z] {
                                        low = (expected_random.next_byte() & 127).wrapping_add(192);
                                        *coordinate = coordinate.wrapping_add(3 * i16::from(low as i8));
                                    }
                                }
                                let actor = objects.get_mut(owner).unwrap();
                                actor.base.path = Some(root);
                                actor.base.position = initial_position;
                                actor.base.attack_power = initial;
                                actor.base.hit_points = health;
                                actor.base.wait_timer = initial;
                                actor.base.flags.casts_shadow = true;
                                actor.extension.depth_offset = 0xABCD;
                                actor.extension.path_state.part = 2;
                                actor.extension.path_state.motion_phase = u16::from_be_bytes([initial_high, initial]);
                                let shape_animation = actor.extension.path_state.animation.shape;
                                runtime.branch.invert_next = inverted;
                                let mut auxiliary = SelectedAuxiliaryState { mode, action_flags: initial };
                                let mut audio = AudioState::default();
                                for visit in 0..8u8 {
                                    let mut inputs = world(&mut random);
                                    if visit == 0 {
                                        inputs.selected_auxiliary = Some(&mut auxiliary);
                                        if cue.is_some() {
                                            inputs.audio = Some(PathAudio {
                                                events: &mut audio, listeners: [CueListener::PrimaryPlayer; 2],
                                                markers: Some(MarkerInputs {
                                                    selected_sides: [PlayerTarget::Primary; 2],
                                                    markers: [CueMarker { identity: CueListener::Other,
                                                        position: initial_position, bearing: Angle::ZERO }; 2],
                                                }),
                                            });
                                        }
                                    }
                                    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 64),
                                        Ok(if visit == 7 { ControlStep::Ended } else { ControlStep::Movement }),
                                        "entry={entry} initial={initial} mode={mode} inverted={inverted} skip_jitter={skip_jitter} health={health} visit={visit}");
                                    assert_eq!(audio.take_events().into_iter().flatten().collect::<Vec<_>>(),
                                        cue.filter(|_| visit == 0).into_iter()
                                            .map(|id| SoundEvent::Authored(AuthoredCue::new(id, 0, PlayerTarget::Secondary))).collect::<Vec<_>>());
                                    if visit != 7 {
                                        assert_eq!(runtime.begin_callbacks(&objects, owner).unwrap(), has_callback);
                                        if has_callback {
                                            assert!(matches!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Run(_))));
                                            let motion_x = initial.wrapping_add(visit.wrapping_mul(33));
                                            let motion_z = initial.wrapping_sub(visit.wrapping_mul(17));
                                            let mut inputs = world(&mut random);
                                            inputs.published_motion = Some(PublishedPlayerMotion {
                                                position: Vector3::default(),
                                                delta: Vector3 { x: (0xA500 | u16::from(motion_x)) as i16,
                                                    y: -32768, z: (0x5A00 | u16::from(motion_z)) as i16 },
                                            });
                                            assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 11), Ok(ControlStep::ResumeCallbacks));
                                            assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Complete));
                                            position.x = position.x.wrapping_add(i16::from(motion_x.wrapping_mul(2).wrapping_neg() as i8));
                                            position.z = position.z.wrapping_add(i16::from(motion_z.wrapping_mul(2).wrapping_neg() as i8));
                                        }
                                    }
                                    let actor = objects.get(owner).unwrap();
                                    assert_eq!(actor.base.position, position);
                                    assert_eq!(actor.extension.path_state.motion_phase, u16::from_be_bytes([high, low]));
                                    assert_eq!(actor.extension.path_state.part, if skip_jitter { 2 } else { 3 });
                                    assert_eq!(actor.extension.path_state.animation.color.fixed_frame(), Some(visit));
                                    assert_eq!(actor.extension.path_state.animation.shape, shape_animation);
                                    assert_eq!(actor.extension.texture_scroll_x, size);
                                    assert_eq!(actor.extension.depth_offset, 0xAB00);
                                    assert_eq!((actor.base.attack_power, actor.base.hit_points, actor.base.wait_timer), (power, health, initial));
                                    assert!(actor.base.flags.scaled_sprite);
                                    assert!(actor.base.flags.collision_disabled);
                                    assert!(actor.base.flags.casts_shadow);
                                    assert_eq!(actor.base.flags.remove_after_tick, visit == 7);
                                    assert!(!runtime.branch.invert_next);
                                    assert_eq!(random, expected_random);
                                }
                                assert_eq!(auxiliary, SelectedAuxiliaryState { mode, action_flags: initial });
                                runtime.release_actor_programs(&mut objects, owner).unwrap();
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn motion_fade_callback_resumes_saved_byte_stack_and_resamples_each_axis_import() {
        use super::super::path_motion::PublishedPlayerMotion;
        use super::super::{authored_paths, Vector3};
        let catalog = authored_paths::catalog();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::PHASE_INCREMENTED_MOTION_FADE_SPRITE);
        actor.base.hit_points = 1; // no cue dependency
        actor.extension.path_state.motion_phase = 0; // increment high skips jitter
        actor.base.position = Vector3 { x: 32767, y: -456, z: -32768 };
        let mut auxiliary = SelectedAuxiliaryState { mode: 0, action_flags: 0 };
        let mut inputs = world(&mut random);
        inputs.selected_auxiliary = Some(&mut auxiliary);
        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16), Ok(ControlStep::Movement));
        assert!(runtime.begin_callbacks(&objects, owner).unwrap());
        assert!(matches!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Run(_))));
        // Save the phase byte once, then stop immediately before importing X.
        let result = runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1);
        assert!(matches!(result, Err(ProgramError::BudgetExceeded { executed: 1, .. })));
        for delta in [None, Some(Vector3 { x: 100, y: 999, z: 7 })] {
            let before = objects.clone();
            let mut inputs = world(&mut random);
            inputs.published_motion = delta.map(|delta| PublishedPlayerMotion { position: Vector3::default(), delta });
            let result = runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 4);
            if delta.is_none() {
                assert_eq!(result, Err(ProgramError::MissingPublishedMotion));
                assert_eq!(objects, before);
            } else {
                assert!(matches!(result, Err(ProgramError::BudgetExceeded { executed: 4, .. })));
            }
        }
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.position, Vector3 { x: 32767i16.wrapping_add(56), y: -456, z: -32768 });
        assert_eq!(actor.extension.path_state.motion_phase, 0x0138);
        let before = objects.clone();
        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 6),
            Err(ProgramError::MissingPublishedMotion));
        assert_eq!(objects, before);
        let mut inputs = world(&mut random);
        // Z observes the changed publication, not the earlier snapshot's 7.
        inputs.published_motion = Some(PublishedPlayerMotion {
            position: Vector3::default(), delta: Vector3 { x: -3000, y: -999, z: 300 },
        });
        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 6), Ok(ControlStep::ResumeCallbacks));
        assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Complete));
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.position, Vector3 { x: 32767i16.wrapping_add(56), y: -456, z: (-32768i16).wrapping_sub(88) });
        assert_eq!(actor.extension.path_state.motion_phase, 0x0111);
        for visit in 1..=7 {
            assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 4),
                Ok(if visit == 7 { ControlStep::Ended } else { ControlStep::Movement }));
            assert_eq!(objects.get(owner).unwrap().extension.path_state.animation.color.fixed_frame(), Some(visit));
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }

    #[test]
    fn hit_cycled_shape_preserves_exact_loop_fallthrough_and_latched_hits_between_callbacks() {
        use super::super::{authored_paths, path_appearance::AnimationControl, Vector3};
        let catalog = authored_paths::catalog();
        // NEXT falls through on the final iteration. At the join between
        // decreasing and increasing loops, zero is overwritten by one in
        // the same visit. The last decreasing iteration of the return
        // phase similarly reaches seven, then reinitializes to zero.
        let opening: &[u8] = &[1, 2, 3, 4, 5, 6, 7, 6, 5, 4, 3, 2, 1, 1, 2, 3, 4, 5, 6, 7];
        let closing: &[u8] = &[6, 5, 4, 3, 2, 1, 0, 0];
        for retained in 0..=u8::MAX {
            for queue_during_animation in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::HIT_CYCLED_SHAPE);
                actor.base.position = Vector3 { x: 1234, y: i16::MIN, z: i16::MAX };
                actor.base.wait_timer = retained;
                actor.base.hit_points = retained;
                actor.base.attack_power = retained ^ 255;
                actor.base.flags.visible = retained & 1 != 0;
                actor.base.flags.collision_disabled = retained & 2 != 0;
                actor.base.flags.scaled_sprite = retained & 4 != 0;
                actor.base.flags.casts_shadow = retained & 8 != 0;
                actor.extension.path_state.animation.shape = AnimationControl::from_packed(retained);
                actor.extension.path_state.animation.color = AnimationControl::from_packed(retained);
                actor.extension.path_state.motion_phase = 0xABCD;
                actor.extension.depth_offset = 0xFEDC;
                actor.extension.texture_scroll_x = 231;
                actor.extension.path_state.repeat_counter = retained;
                let initial_random = random;
                runtime.branch.invert_next = true;
                for _ in 0..3 {
                    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 5),
                        Ok(ControlStep::Movement));
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.extension.path_state.animation.shape.fixed_frame(), Some(0));
                    assert!(actor.extension.path_state.hold_latched);
                    assert_eq!((actor.base.wait_timer, actor.extension.path_state.repeat_counter), (retained, retained));
                }
                for _cycle in 0..3 {
                    for frames in [opening, closing] {
                        let held = objects.get(owner).unwrap().base.path;
                        if !objects.get(owner).unwrap().extension.path_state.conditions.hit_event_pending {
                            assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                            assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Skipped));
                            assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Complete));
                            assert_eq!(objects.get(owner).unwrap().base.path, held);
                            objects.get_mut(owner).unwrap().extension.path_state.conditions.hit_event_pending = true;
                        }
                        assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                        assert!(matches!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Run(_))));
                        assert!(!objects.get(owner).unwrap().extension.path_state.conditions.hit_event_pending);
                        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 2),
                            Ok(ControlStep::ResumeCallbacks));
                        assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Complete));
                        assert_ne!(objects.get(owner).unwrap().base.path, held);
                        assert_eq!((objects.get(owner).unwrap().base.wait_timer,
                            objects.get(owner).unwrap().extension.path_state.repeat_counter), (0, 0));
                        for (visit, &frame) in frames.iter().enumerate() {
                            if visit == 2 && queue_during_animation {
                                objects.get_mut(owner).unwrap().extension.path_state.conditions.hit_event_pending = true;
                            }
                            assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8),
                                Ok(ControlStep::Movement));
                            let actor = objects.get(owner).unwrap();
                            let held = visit + 1 == frames.len();
                            assert_eq!(actor.extension.path_state.animation.shape.fixed_frame(), Some(frame));
                            // The source hold flag stays latched across a
                            // forced redirect; only the cursor leaves HOLD.
                            assert!(actor.extension.path_state.hold_latched);
                            assert_eq!(matches!(catalog.statement(actor.base.path.unwrap()),
                                Ok(Statement::Control(ControlCommand::Hold))), held);
                            let triggers = actor.extension.path_state.triggers.entries(&runtime.resources, owner).unwrap();
                            assert_eq!(triggers.len(), usize::from(held));
                            if held { assert_eq!(triggers[0].kind, super::super::path_triggers::TriggerKind::ConsumeHitEvent); }
                            assert_eq!(actor.extension.path_state.conditions.hit_event_pending, queue_during_animation && visit >= 2);
                            assert!(actor.base.contacts.run_when_paused);
                            assert!(actor.base.contacts.suppress_contacts_next_epoch);
                            assert_eq!(actor.base.flags.visible, retained & 1 != 0);
                            assert_eq!(actor.base.flags.collision_disabled, retained & 2 != 0);
                            assert_eq!(actor.base.flags.scaled_sprite, retained & 4 != 0);
                            assert_eq!(actor.base.flags.casts_shadow, retained & 8 != 0);
                            assert!(!actor.base.flags.remove_after_tick);
                            assert_eq!(actor.base.position, Vector3 { x: 1234, y: i16::MIN, z: i16::MAX });
                            assert_eq!((actor.base.wait_timer, actor.base.hit_points, actor.base.attack_power), (0, retained, retained ^ 255));
                            assert_eq!(actor.extension.path_state.motion_phase, 0xABCD);
                            assert_eq!(actor.extension.path_state.animation.color.packed(), retained);
                            assert_eq!(actor.extension.depth_offset, 0xFEDC);
                            assert_eq!(actor.extension.texture_scroll_x, 231);
                            assert!(runtime.branch.invert_next);
                            assert_eq!(random, initial_random);
                            if !held { assert!(!runtime.begin_callbacks(&objects, owner).unwrap()); }
                        }
                    }
                }
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }

    #[test]
    fn random_tumbling_mesh_sets_velocity_before_eighty_world_angle_steps() {
        use super::super::{authored_paths, path_motion, Angle, Vector3};
        let catalog = authored_paths::catalog();
        for seed in 0..=u8::MAX {
            for deferred in [false, true] {
                for inverted in [false, true] {
                    let (mut runtime, mut objects, owner, _) = setup();
                    let mut random = RandomState::new([seed, seed ^ 255, 93, 253]);
                    let mut expected_random = random;
                    let yaw = expected_random.next_byte();
                    let pitch = expected_random.next_byte();
                    let pitch_step = (expected_random.next_byte() & 31).wrapping_add(240);
                    let yaw_step = (expected_random.next_byte() & 31).wrapping_add(240);
                    let original_velocity = Vector3 { x: 123, y: -456, z: 789 };
                    let expected_velocity = if deferred { original_velocity } else {
                        path_motion::direction_velocity(Angle::from_units(pitch), Angle::from_units(yaw), 30, 1)
                    };
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(authored_paths::RANDOM_TUMBLING_MESH_EFFECT);
                    actor.base.position = Vector3 { x: i16::MIN, y: 456, z: i16::MAX };
                    actor.base.velocity = original_velocity;
                    actor.base.roll = Angle::from_units(seed);
                    actor.base.wait_timer = seed;
                    actor.base.hit_points = seed ^ 255;
                    actor.base.attack_power = seed;
                    actor.base.flags.casts_shadow = true;
                    actor.extension.path_state.motion.generate_velocity_each_step = deferred;
                    actor.extension.path_state.motion_phase = 0xABCD;
                    actor.extension.relative_position = Vector3 { x: 11, y: 22, z: 33 };
                    actor.extension.relative_rotation.pitch = Angle::from_units(71);
                    actor.extension.relative_rotation.yaw = Angle::from_units(95);
                    actor.extension.depth_offset = 0xABCD;
                    let animation = actor.extension.path_state.animation;
                    let local = (actor.extension.relative_position, actor.extension.relative_rotation);
                    runtime.branch.invert_next = inverted;
                    for visit in 1..=80u8 {
                        let result = runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 13);
                        assert_eq!(result, Ok(if visit == 80 { ControlStep::Ended } else { ControlStep::Movement }));
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.base.pitch.units(), pitch.wrapping_add(pitch_step.wrapping_mul(visit)));
                        assert_eq!(actor.base.yaw.units(), yaw.wrapping_add(yaw_step.wrapping_mul(visit)));
                        assert_eq!(actor.base.roll.units(), seed);
                        assert_eq!(actor.base.velocity, expected_velocity);
                        assert_eq!(actor.base.speed, 30);
                        assert_eq!(actor.base.position, Vector3 { x: i16::MIN, y: 456, z: i16::MAX });
                        assert_eq!(actor.extension.path_state.motion_phase, u16::from_le_bytes([pitch_step, yaw_step]));
                        assert_eq!((actor.extension.relative_position, actor.extension.relative_rotation), local);
                        assert_eq!(actor.extension.path_state.animation, animation);
                        assert_eq!(actor.extension.depth_offset, 0xABCD);
                        assert_eq!(actor.base.wait_timer, seed);
                        assert_eq!((actor.base.hit_points, actor.base.attack_power), (seed ^ 255, seed));
                        assert!(actor.base.flags.collision_disabled);
                        assert!(actor.base.flags.casts_shadow);
                        assert_eq!(actor.base.flags.remove_after_tick, visit == 80);
                        assert_eq!(runtime.branch.invert_next, inverted);
                        assert_eq!(random, expected_random);
                    }
                }
            }
        }
    }

    #[test]
    fn rolling_contact_shape_retains_wait_byte_and_rolls_only_on_movement_callbacks() {
        use super::super::{authored_paths, path_motion, Angle, Vector3};
        let catalog = authored_paths::catalog();
        for initial in 0..=u8::MAX {
            for deferred in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::ROLLING_CONTACT_SHAPE);
                actor.base.wait_timer = initial;
                actor.base.pitch = Angle::from_units(initial);
                actor.base.yaw = Angle::from_units(initial ^ 255);
                actor.base.roll = Angle::from_units(initial);
                actor.base.velocity = Vector3 { x: 123, y: -456, z: 789 };
                actor.base.flags.collision_disabled = true;
                actor.base.flags.casts_shadow = true;
                actor.extension.path_state.motion.generate_velocity_each_step = deferred;
                actor.extension.path_state.motion_phase = 0xABCD;
                let expected_velocity = if deferred { actor.base.velocity } else {
                    path_motion::direction_velocity(actor.base.pitch, actor.base.yaw, 80, 1)
                };
                let animation = actor.extension.path_state.animation;
                let initial_random = random;
                runtime.branch.invert_next = true;
                let terminal = usize::from(12u8.wrapping_sub(initial));
                for visit in 0..=terminal {
                    let ended = visit == terminal;
                    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 7),
                        Ok(if ended { ControlStep::Ended } else { ControlStep::Movement }));
                    if !ended {
                        assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                        assert!(matches!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Run(_))));
                        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 2),
                            Ok(ControlStep::ResumeCallbacks));
                        assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Complete));
                    }
                    let actor = objects.get(owner).unwrap();
                    let callbacks = (visit + usize::from(!ended)) as u8;
                    assert_eq!(actor.base.roll.units(), initial.wrapping_add(callbacks.wrapping_mul(16)));
                    assert_eq!((actor.base.pitch.units(), actor.base.yaw.units()), (initial, initial ^ 255));
                    assert_eq!(actor.base.shape, ShapeId::from_catalog_index(47));
                    assert_eq!(actor.base.speed, 80);
                    assert_eq!(actor.base.velocity, expected_velocity);
                    assert_eq!(actor.base.position, Vector3::default());
                    assert_eq!(actor.base.wait_timer, if ended { 0 } else { initial.wrapping_add((visit + 1) as u8) });
                    assert!(!actor.base.flags.collision_disabled);
                    assert!(actor.base.contacts.suppress_contacts_next_epoch);
                    assert!(actor.base.flags.casts_shadow);
                    assert_eq!(actor.base.flags.remove_after_tick, ended);
                    assert_eq!(actor.extension.path_state.motion_phase, 0xABCD);
                    assert_eq!(actor.extension.path_state.animation, animation);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, initial_random);
                }
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }

    #[test]
    fn held_mesh_roots_keep_distinct_class_animation_and_distance_controls() {
        use super::super::{authored_paths, collision_pass::ExclusionGroups, path_appearance::AnimationControl};
        let catalog = authored_paths::catalog();
        for reset in [false, true] {
            for initial in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(if reset { authored_paths::RESET_SHAPE_HOLD } else { authored_paths::DISTANT_SHAPE_HOLD });
                actor.base.hit_points = initial;
                actor.base.attack_power = initial;
                actor.base.wait_timer = initial;
                actor.base.flags.casts_shadow = true;
                actor.base.flags.maximum_draw_distance = initial & 1 != 0;
                actor.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(initial);
                actor.extension.path_state.animation.shape = AnimationControl::from_packed(initial);
                actor.extension.path_state.animation.color = AnimationControl::from_packed(initial ^ 255);
                let before = actor.clone();
                let initial_random = random;
                runtime.branch.invert_next = true;
                assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 6), Ok(ControlStep::Movement));
                let actor = objects.get(owner).unwrap();
                let mut expected = before;
                expected.base.path = actor.base.path;
                expected.base.behavior = Behavior::PathMovement;
                expected.base.hit_points = 100;
                expected.base.flags.collision_disabled = true;
                expected.base.flags.casts_shadow = false;
                expected.extension.path_state.hold_latched = true;
                expected.extension.animation_frame = if !reset && initial & 0x80 != 0 { initial & 0x7F } else { 0 };
                expected.extension.color_frame = if initial & 0x80 == 0 { (initial ^ 255) & 0x7F } else { 0 };
                if reset {
                    expected.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(initial & 0xEF);
                    expected.extension.path_state.animation.shape = AnimationControl::from_packed(0x80);
                } else {
                    expected.base.flags.maximum_draw_distance = true;
                }
                assert_eq!(actor, &expected);
                for _ in 0..4 {
                    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1), Ok(ControlStep::Movement));
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, initial_random);
                }
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }

    #[test]
    fn mesh_effect_relative_motion_wraps_without_changing_world_pose_or_render_channels() {
        use super::super::{authored_paths, Angle, Vector3};
        let catalog = authored_paths::catalog();
        for drift in [false, true] {
            for angle in 0..=u8::MAX {
                for position in [i16::MIN, -20, 0, i16::MAX] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(if drift { authored_paths::RELATIVE_DRIFT_ROLL_EFFECT }
                        else { authored_paths::RELATIVE_YAW_EFFECT });
                    actor.base.position = Vector3 { x: 32767, y: -32768, z: -999 };
                    actor.base.pitch = Angle::from_units(23);
                    actor.base.yaw = Angle::from_units(45);
                    actor.base.roll = Angle::from_units(67);
                    actor.base.wait_timer = angle;
                    actor.base.flags.casts_shadow = true;
                    actor.extension.relative_position = Vector3 { x: position, y: position, z: 789 };
                    actor.extension.relative_rotation.pitch = Angle::from_units(19);
                    actor.extension.relative_rotation.yaw = Angle::from_units(angle);
                    actor.extension.relative_rotation.roll = Angle::from_units(angle);
                    actor.extension.depth_offset = 0xABCD;
                    actor.extension.texture_scroll_x = angle;
                    let animation = actor.extension.path_state.animation;
                    let initial_random = random;
                    runtime.branch.invert_next = true;
                    for visit in 1..=70i16 {
                        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 6),
                            Ok(ControlStep::Movement));
                        let actor = objects.get(owner).unwrap();
                        let local = if drift { position.wrapping_sub(20 * visit) } else { position };
                        assert_eq!(actor.extension.relative_position, Vector3 { x: local, y: local, z: 789 });
                        assert_eq!(actor.extension.relative_rotation.pitch.units(), 19);
                        assert_eq!(actor.extension.relative_rotation.yaw.units(),
                            angle.wrapping_add(if drift { 0 } else { (visit * 8) as u8 }));
                        assert_eq!(actor.extension.relative_rotation.roll.units(),
                            angle.wrapping_add(if drift { (visit * 4) as u8 } else { 0 }));
                        assert_eq!(actor.base.position, Vector3 { x: 32767, y: -32768, z: -999 });
                        assert_eq!((actor.base.pitch.units(), actor.base.yaw.units(), actor.base.roll.units()), (23, 45, 67));
                        assert!(actor.base.flags.far_sort_bias);
                        assert!(actor.base.flags.collision_disabled);
                        assert!(actor.base.flags.casts_shadow);
                        assert!(!actor.base.flags.remove_after_tick);
                        assert_eq!(actor.base.wait_timer, angle);
                        assert_eq!(actor.extension.depth_offset, 0xABCD);
                        assert_eq!(actor.extension.texture_scroll_x, angle);
                        assert_eq!(actor.extension.path_state.animation, animation);
                        assert!(runtime.branch.invert_next);
                        assert_eq!(random, initial_random);
                    }
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn footprint_yaw_effects_keep_distinct_contact_flags_and_reset_shape_on_each_authored_loop() {
        use super::super::{authored_paths, path_appearance::AnimationControl, Angle, Vector3};
        let catalog = authored_paths::catalog();
        for reset_shape in [false, true] {
            for initial in 0..=u8::MAX {
                for suppression in [false, true] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(if reset_shape { authored_paths::RESET_ANIMATION_YAW_EFFECT }
                        else { authored_paths::FOOTPRINT_YAW_EFFECT });
                    actor.base.flags.exclude_from_shape_footprint_search = true;
                    actor.base.flags.casts_shadow = true;
                    actor.base.contacts.suppress_contacts_next_epoch = suppression;
                    actor.base.wait_timer = initial;
                    actor.extension.relative_rotation.yaw = Angle::from_units(initial);
                    actor.extension.relative_position = Vector3 { x: i16::MIN, y: 456, z: i16::MAX };
                    actor.extension.path_state.animation.color = AnimationControl::from_packed(initial);
                    actor.extension.path_state.motion_phase = 0xABCD;
                    let initial_position = actor.base.position;
                    let initial_random = random;
                    runtime.branch.invert_next = true;
                    for visit in 1..=130u16 {
                        let live_animation = initial.wrapping_add(visit as u8);
                        objects.get_mut(owner).unwrap().extension.path_state.animation.shape =
                            AnimationControl::from_packed(live_animation);
                        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8),
                            Ok(ControlStep::Movement));
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.extension.relative_rotation.yaw.units(), initial.wrapping_sub((visit * 2) as u8));
                        assert_eq!(actor.extension.path_state.animation.shape.packed(), if reset_shape { 128 } else { live_animation });
                        assert_eq!(actor.extension.path_state.animation.color.packed(), initial);
                        assert!(!actor.base.flags.exclude_from_shape_footprint_search);
                        assert_eq!(actor.base.contacts.suppress_contacts_next_epoch, !reset_shape || suppression);
                        assert!(!actor.base.flags.casts_shadow);
                        assert!(actor.base.flags.collision_disabled);
                        assert!(!actor.base.flags.remove_after_tick);
                        assert_eq!(actor.base.wait_timer, initial);
                        assert_eq!(actor.base.position, initial_position);
                        assert_eq!(actor.extension.relative_position, Vector3 { x: i16::MIN, y: 456, z: i16::MAX });
                        assert_eq!(actor.extension.path_state.motion_phase, 0xABCD);
                        assert!(runtime.branch.invert_next);
                        assert_eq!(random, initial_random);
                    }
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn timed_falling_yaw_effect_runs_thirty_world_y_callbacks_then_keeps_local_rotation() {
        use super::super::{authored_paths, Angle, Vector3};
        let catalog = authored_paths::catalog();
        for initial in 0..=u8::MAX {
            for y in [i16::MIN, -10, 0, i16::MAX] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::TIMED_FALLING_YAW_EFFECT);
                actor.base.position = Vector3 { x: 321, y, z: -987 };
                actor.base.pitch = Angle::from_units(initial);
                actor.base.wait_timer = initial;
                actor.base.flags.exclude_from_shape_footprint_search = true;
                actor.base.flags.casts_shadow = true;
                actor.extension.relative_rotation.yaw = Angle::from_units(initial);
                actor.extension.relative_position = Vector3 { x: -10, y: 100, z: -1000 };
                let animation = actor.extension.path_state.animation;
                let initial_random = random;
                runtime.branch.invert_next = true;
                for visit in 0..70u8 {
                    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8),
                        Ok(ControlStep::Movement));
                    if visit <= 30 {
                        assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                        let step = runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()).unwrap();
                        if visit < 30 {
                            assert!(matches!(step, CallbackStep::Run(_)));
                            assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 2),
                                Ok(ControlStep::ResumeCallbacks));
                        } else {
                            assert_eq!(step, CallbackStep::Expired);
                        }
                        assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Complete));
                    } else {
                        assert!(!runtime.begin_callbacks(&objects, owner).unwrap());
                    }
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.base.position, Vector3 { x: 321, y: y.wrapping_sub(10 * i16::from((visit + 1).min(30))), z: -987 });
                    assert_eq!(actor.base.pitch.units(), 128);
                    assert_eq!(actor.extension.relative_rotation.yaw.units(), initial.wrapping_sub((visit + 1).wrapping_mul(2)));
                    assert_eq!(actor.extension.relative_position, Vector3 { x: -10, y: 100, z: -1000 });
                    assert!(actor.base.flags.exclude_from_shape_footprint_search);
                    assert!(actor.base.contacts.suppress_contacts_next_epoch);
                    assert!(actor.base.flags.collision_disabled);
                    assert!(!actor.base.flags.casts_shadow);
                    assert!(!actor.base.flags.remove_after_tick);
                    assert_eq!(actor.base.wait_timer, initial);
                    assert_eq!(actor.extension.path_state.animation, animation);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, initial_random);
                }
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }

    #[test]
    fn finite_mesh_effects_preserve_packed_shape_arithmetic_and_retained_wait_byte() {
        use super::super::{authored_paths, path_appearance::AnimationControl, Vector3};
        let catalog = authored_paths::catalog();
        for wait in [false, true] {
            for initial in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(if wait { authored_paths::DEPTH_BIASED_WAIT_EFFECT }
                    else { authored_paths::SIX_STEP_SHAPE_EFFECT });
                actor.base.wait_timer = initial;
                actor.base.flags.casts_shadow = true;
                actor.base.flags.far_sort_bias = false;
                actor.base.position = Vector3 { x: -123, y: i16::MIN, z: i16::MAX };
                actor.extension.path_state.animation.shape = AnimationControl::from_packed(initial);
                actor.extension.path_state.animation.color = AnimationControl::from_packed(initial);
                actor.extension.depth_offset = 0xABCD;
                actor.extension.texture_scroll_x = initial;
                let initial_random = random;
                runtime.branch.invert_next = true;
                let terminal_visit = if wait { usize::from(20u8.wrapping_sub(initial)) } else { 5 };
                let mut packed = initial;
                for visit in 0..=terminal_visit {
                    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 5),
                        Ok(if visit == terminal_visit { ControlStep::Ended } else { ControlStep::Movement }));
                    if !wait {
                        // Wide source arithmetic, including the sign-conditioned
                        // correction and single subtraction; not frame modulo.
                        let sum = (u16::from(packed) + 1) % 256;
                        let corrected = (sum + if sum < 128 { 6 } else { 0 }) % 128;
                        packed = (if corrected >= 6 { corrected - 6 } else { corrected }) as u8 | 128;
                    }
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.extension.path_state.animation.shape.packed(), packed);
                    assert_eq!(actor.extension.path_state.animation.color.packed(), initial);
                    assert_eq!(actor.base.wait_timer, if !wait { initial }
                        else if visit == terminal_visit { 0 } else { initial.wrapping_add((visit + 1) as u8) });
                    assert!(actor.base.flags.collision_disabled);
                    assert!(actor.base.flags.casts_shadow);
                    assert_eq!(actor.base.flags.far_sort_bias, wait);
                    assert_eq!(actor.base.flags.remove_after_tick, visit == terminal_visit);
                    assert_eq!(actor.base.position, Vector3 { x: -123, y: i16::MIN, z: i16::MAX });
                    assert_eq!(actor.extension.depth_offset, 0xABCD);
                    assert_eq!(actor.extension.texture_scroll_x, initial);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, initial_random);
                }
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }

    #[test]
    fn held_sprite_and_depth_effects_preserve_distinct_full_word_and_sprite_contracts() {
        use super::super::{authored_paths, Vector3};
        let catalog = authored_paths::catalog();
        for solid in [false, true] {
            for sprite in [false, true] {
                for retained in 0..=u8::MAX {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(if solid { authored_paths::SOLID_SPRITE_HOLD } else { authored_paths::BIASED_DEPTH_HOLD });
                    actor.base.position = Vector3 { x: i16::MIN, y: i16::MAX, z: -1234 };
                    actor.base.flags.scaled_sprite = sprite;
                    actor.base.flags.far_sort_bias = sprite;
                    actor.base.wait_timer = retained;
                    actor.base.attack_power = retained;
                    actor.extension.texture_scroll_x = retained;
                    actor.extension.depth_offset = u16::from_be_bytes([retained, 173]);
                    let original_animation = actor.extension.path_state.animation;
                    runtime.branch.invert_next = true;
                    let initial_random = random;
                    for _ in 0..4 {
                        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 4), Ok(ControlStep::Movement));
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.extension.texture_scroll_x, if solid { 64 } else { retained });
                        assert_eq!(actor.extension.depth_offset, if solid { u16::from(retained) << 8 } else { 3 });
                        assert_eq!(actor.base.flags.far_sort_bias, !solid || sprite);
                        assert_eq!(actor.base.flags.scaled_sprite, solid || sprite);
                        assert!(actor.base.flags.collision_disabled);
                        assert!(!actor.base.flags.remove_after_tick);
                        assert!(!actor.base.flags.strategy_suspended);
                        assert!(actor.extension.path_state.hold_latched);
                        assert_eq!(actor.base.behavior, Behavior::PathMovement);
                        assert!(matches!(catalog.statement(actor.base.path.unwrap()), Ok(Statement::Control(ControlCommand::Hold))));
                        assert_eq!(actor.extension.path_state.animation, original_animation);
                        assert_eq!(actor.base.position, Vector3 { x: i16::MIN, y: i16::MAX, z: -1234 });
                        assert_eq!((actor.base.wait_timer, actor.base.attack_power), (retained, retained));
                        assert!(runtime.branch.invert_next);
                        assert_eq!(random, initial_random);
                    }
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn table_color_reveal_uses_every_initial_selector_and_keeps_callbacks_after_hold() {
        use super::super::{authored_paths, path_appearance::AnimationChannel, Vector3};
        let catalog = authored_paths::catalog();
        // The full 256-byte source table is independently pinned by static
        // source tests. Here prove live lookup and boundary-state behavior,
        // including initial selectors beyond the nominal seven-frame loop.
        let tables: Vec<_> = catalog.paths.iter().flatten().filter_map(|statement| match statement {
            Statement::Mutate { mutation: Mutation::Byte {
                field: ByteField::Animation(AnimationChannel::Color),
                operation: ByteOperation::Assign(ByteOperand::Lookup { values, .. }),
            }, .. } if values.len() == 256 && values[..7] == [128, 129, 130, 131, 130, 129, 128] => Some(*values),
            _ => None,
        }).collect();
        assert_eq!(tables.len(), 1);
        let table = tables[0];
        for initial_phase in 0..=u8::MAX {
            for retained_wait in [0u8, 3, 4, 255] {
                for initial_visible in [false, true] {
                    for initially_disabled in [false, true] {
                        let (mut runtime, mut objects, owner, mut random) = setup();
                        let actor = objects.get_mut(owner).unwrap();
                        actor.base.path = Some(authored_paths::TABLE_COLOR_REVEAL);
                        actor.base.flags.visible = initial_visible;
                        actor.base.flags.collision_disabled = initially_disabled;
                        actor.base.wait_timer = retained_wait;
                        actor.base.position = Vector3 { x: -1234, y: i16::MIN, z: i16::MAX };
                        actor.extension.depth_offset = 0xABCD;
                        actor.extension.texture_scroll_x = 241;
                        actor.extension.path_state.motion_phase = 0xAB00 | u16::from(initial_phase);
                        runtime.branch.invert_next = true;
                        let initial_random = random;
                        let reveal_visit = usize::from(3u8.wrapping_sub(retained_wait));
                        let mut phase = initial_phase;
                        for visit in 0..(reveal_visit + 270) {
                            assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 6), Ok(ControlStep::Movement));
                            if visit == 0 {
                                assert_eq!(objects.get(owner).unwrap().extension.path_state.animation.color.packed(), 129);
                            }
                            assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                            assert!(matches!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Run(_))));
                            assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 6), Ok(ControlStep::ResumeCallbacks));
                            assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Complete));
                            let color = table[usize::from(phase)];
                            phase = phase.wrapping_add(1);
                            if phase == 7 { phase = 0; }
                            let actor = objects.get(owner).unwrap();
                            assert_eq!(actor.extension.path_state.animation.color.packed(), color);
                            assert_eq!(actor.extension.path_state.motion_phase, 0xAB00 | u16::from(phase));
                            assert_eq!(actor.base.wait_timer, if visit >= reveal_visit { 0 }
                                else { retained_wait.wrapping_add((visit + 1) as u8) });
                            assert_eq!(actor.base.flags.visible, initial_visible || visit >= reveal_visit);
                            assert_eq!(actor.base.flags.collision_disabled, initially_disabled || visit >= reveal_visit);
                            assert_eq!(actor.extension.path_state.hold_latched, visit >= reveal_visit);
                            assert!(!actor.base.flags.remove_after_tick);
                            assert!(!actor.base.flags.strategy_suspended);
                            assert_eq!(actor.base.position, Vector3 { x: -1234, y: i16::MIN, z: i16::MAX });
                            assert_eq!(actor.extension.depth_offset, 0xABCD);
                            assert_eq!(actor.extension.texture_scroll_x, 241);
                            assert!(!runtime.branch.invert_next);
                            assert_eq!(random, initial_random);
                        }
                        runtime.release_actor_programs(&mut objects, owner).unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn fixed_count_sprite_effects_keep_exact_frames_and_offset_without_repeated_setup() {
        use super::super::{authored_paths, Vector3};
        let catalog = authored_paths::catalog();
        for shrinking in [false, true] {
            for retained in 0..=u8::MAX {
                for y in [i16::MIN, -600, 0, i16::MAX - 600, i16::MAX] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(if shrinking { authored_paths::OFFSET_SHRINK_SPRITE }
                        else { authored_paths::FIXED_SIZE_FADE_SPRITE });
                    actor.base.flags.casts_shadow = true;
                    actor.base.position = Vector3 { x: -1234, y, z: 32767 };
                    actor.base.wait_timer = retained;
                    actor.base.hit_points = retained;
                    actor.base.attack_power = retained;
                    actor.extension.depth_offset = 0xABCD;
                    actor.extension.path_state.animation.color = super::super::path_appearance::AnimationControl::from_packed(retained);
                    let original_animation = actor.extension.path_state.animation;
                    runtime.branch.invert_next = true;
                    let initial_random = random;
                    for visit in 0..8 {
                        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8),
                            Ok(if visit == 7 { ControlStep::Ended } else { ControlStep::Movement }));
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.base.position, Vector3 { x: -1234, y: if shrinking { y.wrapping_add(600) } else { y }, z: 32767 });
                        assert_eq!(actor.extension.texture_scroll_x, if shrinking { 32 - (visit + 1) * 3 } else { 24 });
                        assert_eq!(actor.extension.depth_offset, 0xAB00);
                        if shrinking { assert_eq!(actor.extension.path_state.animation, original_animation); }
                        else { assert_eq!(actor.extension.path_state.animation.color.fixed_frame(), Some((visit + 1) % 8)); }
                        assert_eq!(actor.base.flags.casts_shadow, !shrinking);
                        assert!(actor.base.flags.scaled_sprite);
                        assert!(actor.base.flags.collision_disabled);
                        assert_eq!(actor.base.flags.remove_after_tick, visit == 7);
                        assert_eq!((actor.base.wait_timer, actor.base.hit_points, actor.base.attack_power), (retained, retained, retained));
                        assert!(runtime.branch.invert_next);
                        assert_eq!(random, initial_random);
                    }
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn phase_growth_fade_reads_live_phase_each_step_and_preserves_arbitrary_initial_animation() {
        use super::super::{authored_paths, path_appearance::AnimationControl, Vector3};
        let catalog = authored_paths::catalog();
        for packed in 0..=u8::MAX {
            for power in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::PHASE_GROWTH_FADE_SPRITE);
                actor.base.attack_power = power;
                actor.base.wait_timer = packed;
                actor.base.position = Vector3 { x: -32768, y: 32767, z: 1234 };
                actor.extension.depth_offset = 0xABCD;
                actor.extension.path_state.motion_phase = u16::from_be_bytes([packed, 173]);
                actor.extension.path_state.animation.color = AnimationControl::from_packed(packed);
                let initial_shape_animation = actor.extension.path_state.animation.shape;
                runtime.branch.invert_next = true;
                let initial_random = random;
                let mut color = packed;
                let mut size = power;
                for visit in 0..8u8 {
                    let phase = if visit == 0 { 0 } else { power.wrapping_add(visit * 31) };
                    if visit != 0 {
                        objects.get_mut(owner).unwrap().extension.path_state.motion_phase = u16::from_be_bytes([packed, phase]);
                    }
                    size = size.wrapping_add(phase);
                    // Literal source arithmetic: packed byte increment,
                    // correction, seven-bit truncation, single subtraction.
                    // In particular an automatic control is not reset to zero.
                    color = color.wrapping_add(1);
                    if color & 0x80 == 0 { color = color.wrapping_add(8); }
                    color &= 0x7F;
                    if color >= 8 { color -= 8; }
                    color |= 0x80;
                    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 9),
                        Ok(if visit == 7 { ControlStep::Ended } else { ControlStep::Movement }));
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.extension.path_state.animation.color.packed(), color);
                    assert_eq!(actor.extension.path_state.animation.shape, initial_shape_animation);
                    assert_eq!(actor.extension.texture_scroll_x, size);
                    assert_eq!(actor.extension.depth_offset, 0xAB00);
                    assert_eq!(actor.extension.path_state.motion_phase, u16::from_be_bytes([packed, phase]));
                    assert_eq!(actor.base.position, Vector3 { x: -32768, y: 32767, z: 1234 });
                    assert_eq!(actor.base.attack_power, power);
                    assert_eq!(actor.base.wait_timer, packed);
                    assert!(actor.base.flags.scaled_sprite);
                    assert!(actor.base.flags.collision_disabled);
                    assert_eq!(actor.base.flags.remove_after_tick, visit == 7);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, initial_random);
                }
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }

    #[test]
    fn health_fade_sound_sprite_entries_queue_their_distinct_cue_once_and_count_full_health() {
        use super::super::path_sound::{AuthoredCue, CueListener, CueMarker, MarkerInputs, PathAudio};
        use super::super::{authored_paths, Angle, AudioState, SoundEvent, Vector3};
        let catalog = authored_paths::catalog();
        for (root, cue) in [(authored_paths::HEALTH_FADE_SOUND_SPRITE, 137),
            (authored_paths::ALTERNATE_HEALTH_FADE_SOUND_SPRITE, 136)] {
            for health in 0..=u8::MAX {
                let power = health ^ 255;
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(root);
                actor.base.hit_points = health;
                actor.base.attack_power = power;
                actor.base.wait_timer = health;
                actor.extension.depth_offset = 0xABCD;
                actor.extension.path_state.motion_phase = 0xFEDC;
                runtime.branch.invert_next = true;
                let initial_random = random;
                let mut audio = AudioState::default();
                let iterations = if health == 0 { 65_536 } else { usize::from(health) };
                for visit in 0..=iterations {
                    let mut inputs = world(&mut random);
                    if visit == 0 {
                        inputs.audio = Some(PathAudio {
                            events: &mut audio,
                            listeners: [CueListener::PrimaryPlayer; 2],
                            markers: Some(MarkerInputs {
                                selected_sides: [PlayerTarget::Primary; 2],
                                markers: [CueMarker { identity: CueListener::Other,
                                    position: Vector3::default(), bearing: Angle::ZERO }; 2],
                            }),
                        });
                    }
                    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 12),
                        Ok(if visit == iterations { ControlStep::Ended } else { ControlStep::Movement }));
                    assert_eq!(audio.take_events().into_iter().flatten().collect::<Vec<_>>(),
                        // Source atan(0, 0) takes its zero-denominator
                        // quarter-turn branch, so this cue is right-panned.
                        if visit == 0 { vec![SoundEvent::Authored(AuthoredCue::new(cue, 32, PlayerTarget::Secondary))] }
                        else { vec![] });
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.extension.path_state.animation.color.fixed_frame(), Some((visit % 8) as u8));
                    assert_eq!(actor.extension.texture_scroll_x, 8u8.wrapping_add(power));
                    assert_eq!(actor.extension.depth_offset, 0xAB00);
                    assert_eq!(actor.extension.path_state.motion_phase, 0xFEDC);
                    assert_eq!(actor.base.position, Vector3::default());
                    assert_eq!((actor.base.hit_points, actor.base.attack_power, actor.base.wait_timer), (health, power, health));
                    assert!(actor.base.flags.scaled_sprite);
                    assert!(actor.base.flags.collision_disabled);
                    assert_eq!(actor.base.flags.remove_after_tick, visit == iterations);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, initial_random);
                }
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }

    #[test]
    fn shrinking_rise_sprite_keeps_retained_wait_signed_phase_and_byte_wrapping() {
        use super::super::{authored_paths, Vector3};
        let catalog = authored_paths::catalog();
        for retained in 0..=u8::MAX {
            for initial_high in [retained, 0, 127, 128, 255] {
                let (mut runtime, mut objects, owner, _) = setup();
                let mut random = RandomState::new([retained, initial_high, 93, 253]);
                let mut expected_random = random;
                let mut expected_position = Vector3 { x: i16::MAX, y: i16::MIN, z: -1234 };
                let mut low = 0;
                for position in [&mut expected_position.x, &mut expected_position.y, &mut expected_position.z] {
                    low = (expected_random.next_byte() & 63).wrapping_add(224);
                    *position = position.wrapping_add(i16::from(low as i8));
                }
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::SHRINKING_RISE_SPRITE);
                actor.base.position = Vector3 { x: i16::MAX, y: i16::MIN, z: -1234 };
                actor.base.wait_timer = retained;
                actor.base.attack_power = retained;
                actor.extension.depth_offset = 0xABCD;
                actor.extension.path_state.motion_phase = u16::from_be_bytes([initial_high, 173]);
                let original_animation = actor.extension.path_state.animation;
                runtime.branch.invert_next = true;
                let movements = usize::from(13u8.wrapping_sub(retained));
                let mut high = initial_high;
                let mut size = 8u8.wrapping_add(retained);
                for visit in 0..=movements {
                    let ending = visit == movements;
                    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 16),
                        Ok(if ending { ControlStep::Ended } else { ControlStep::Movement }));
                    assert_eq!(objects.get(owner).unwrap().base.position, expected_position);
                    if !ending {
                        assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                        assert!(matches!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Run(_))));
                        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 4), Ok(ControlStep::ResumeCallbacks));
                        assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Complete));
                        high = high.wrapping_add(1);
                        expected_position.y = expected_position.y.wrapping_add(i16::from(high as i8));
                        size = size.wrapping_sub(1);
                    }
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.base.position, expected_position);
                    assert_eq!(actor.extension.texture_scroll_x, size);
                    assert_eq!(actor.extension.depth_offset, 0xAB00);
                    assert_eq!(actor.extension.path_state.motion_phase, u16::from_be_bytes([high, low]));
                    assert_eq!(actor.extension.path_state.animation, original_animation);
                    assert_eq!(actor.base.attack_power, retained);
                    assert_eq!(actor.base.wait_timer, if ending { 0 } else { retained.wrapping_add((visit + 1) as u8) });
                    assert!(actor.base.flags.scaled_sprite);
                    assert!(actor.base.flags.collision_disabled);
                    assert_eq!(actor.base.flags.remove_after_tick, ending);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, expected_random);
                }
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }

    #[test]
    fn part_sound_blink_sprite_emits_on_wrapped_phase_four_and_uses_incremented_part() {
        use super::super::path_sound::{AuthoredCue, CueListener, CueMarker, MarkerInputs, PathAudio};
        use super::super::{authored_paths, Angle, AudioState, SoundEvent, Vector3};
        let catalog = authored_paths::catalog();
        for initial_low in 0..=u8::MAX {
            for part in [0u8, 254, 255] {
                for health in [0u8, 1, 4, 255] {
                    if health == 0 && ![0, 3, 4, 255].contains(&initial_low) { continue; }
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(authored_paths::PART_SOUND_BLINK_SPRITE);
                    actor.base.hit_points = health;
                    actor.base.attack_power = initial_low;
                    actor.base.wait_timer = part;
                    actor.extension.depth_offset = 0xABCD;
                    actor.extension.path_state.motion_phase = 0xAB00 | u16::from(initial_low);
                    actor.extension.path_state.part = part;
                    runtime.branch.invert_next = true;
                    let initial_random = random;
                    let iterations = if health == 0 { 65_536 } else { usize::from(health) };
                    let effective_part = part.wrapping_add(1);
                    let mut audio = AudioState::default();
                    for visit in 0..=iterations {
                        let low = initial_low.wrapping_add(visit as u8);
                        let expected_cue = if visit == 0 { Some(150) }
                            else if low == 4 { Some(if effective_part == 0 { 131 } else { 150 }) }
                            else { None };
                        let mut inputs = world(&mut random);
                        // No audio dependency on iterations without a cue.
                        if expected_cue.is_some() {
                            inputs.audio = Some(PathAudio {
                                events: &mut audio,
                                listeners: [CueListener::PrimaryPlayer; 2],
                                markers: Some(MarkerInputs {
                                    selected_sides: [PlayerTarget::Primary; 2],
                                    markers: [CueMarker { identity: CueListener::Other,
                                        position: Vector3::default(), bearing: Angle::ZERO }; 2],
                                }),
                            });
                        }
                        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16),
                            Ok(if visit == iterations { ControlStep::Ended } else { ControlStep::Movement }));
                        assert_eq!(audio.take_events().into_iter().flatten().collect::<Vec<_>>(),
                            expected_cue.into_iter().map(|id| SoundEvent::Authored(AuthoredCue::new(id, 32, PlayerTarget::Secondary))).collect::<Vec<_>>());
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.extension.path_state.animation.color.fixed_frame(), Some((visit % 2) as u8));
                        assert_eq!(actor.extension.path_state.motion_phase, 0xAB00 | u16::from(low));
                        assert_eq!(actor.extension.path_state.part, effective_part);
                        assert_eq!(actor.extension.texture_scroll_x, 8u8.wrapping_add(initial_low));
                        assert_eq!(actor.extension.depth_offset, 0xAB00);
                        assert_eq!((actor.base.hit_points, actor.base.attack_power, actor.base.wait_timer), (health, initial_low, part));
                        assert_eq!(actor.base.position, Vector3::default());
                        assert!(actor.base.flags.collision_disabled);
                        assert!(actor.base.flags.scaled_sprite);
                        assert_eq!(actor.base.flags.remove_after_tick, visit == iterations);
                        assert_eq!(runtime.branch.invert_next, visit == 0);
                        assert_eq!(random, initial_random);
                    }
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn drifting_pulse_sprite_keeps_nested_loop_yields_and_signed_callback_drift() {
        use super::super::{authored_paths, Vector3};
        let catalog = authored_paths::catalog();
        // The fourth upward increment immediately falls into the first
        // downward increment. Color four is never a movement-boundary frame.
        let colors = [1, 2, 3, 3, 2, 1, 0].repeat(3);
        let colors: Vec<_> = colors.into_iter().chain(1..=7).collect();
        for power in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::DRIFTING_PULSE_SPRITE);
            actor.base.attack_power = power;
            actor.base.hit_points = 100;
            actor.base.wait_timer = power;
            actor.base.position = Vector3 { x: i16::MAX, y: i16::MIN, z: -1234 };
            actor.extension.depth_offset = 0xABCD;
            runtime.branch.invert_next = true;
            let initial_random = random;
            let mut expected_position = actor.base.position;
            for (visit, &color) in colors.iter().enumerate() {
                let ending = visit + 1 == colors.len();
                assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 16),
                    Ok(if ending { ControlStep::Ended } else { ControlStep::Movement }), "visit {visit}");
                assert_eq!(objects.get(owner).unwrap().base.position, expected_position);
                if !ending {
                    assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                    assert!(matches!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Run(_))));
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 5), Ok(ControlStep::ResumeCallbacks));
                    assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Complete));
                    expected_position.x = expected_position.x.wrapping_add(i16::from(power as i8));
                    expected_position.y = expected_position.y.wrapping_sub(20);
                }
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.position, expected_position);
                assert_eq!(actor.extension.path_state.animation.color.fixed_frame(), Some(color));
                assert_eq!(actor.extension.texture_scroll_x, 16 + 2 * (visit + 1).min(27) as u8);
                assert_eq!(actor.extension.depth_offset, 0xAB00);
                assert_eq!((actor.base.hit_points, actor.base.attack_power, actor.base.wait_timer), (100, power, power));
                assert!(actor.base.flags.scaled_sprite);
                assert!(actor.base.flags.collision_disabled);
                assert_eq!(actor.base.flags.remove_after_tick, ending);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, initial_random);
            }
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }

    #[test]
    fn part_jitter_sprite_consumes_ordered_draws_and_resumes_zero_count_word_loops() {
        use super::super::{authored_paths, Vector3};
        let catalog = authored_paths::catalog();
        for part in 0..=u8::MAX {
            for high in [0, 1, 2, 255] {
                for invert in [false, true] {
                    let (mut runtime, mut objects, owner, _) = setup();
                    let mut random = RandomState::new([part, high, 173, part ^ 255]);
                    let mut expected_random = random;
                    let power = expected_random.next_byte() & 31;
                    let skip = (high == 1) != invert;
                    let expected_part = if skip { part } else { part.wrapping_add(1) };
                    let iterations = if expected_part == 0 { 65_536 } else { usize::from(expected_part) };
                    let initial_position = Vector3 { x: i16::MAX, y: i16::MIN, z: -1234 };
                    let mut expected_position = initial_position;
                    let mut low = 173;
                    if !skip {
                        for position in [&mut expected_position.x, &mut expected_position.y, &mut expected_position.z] {
                            low = (expected_random.next_byte() & 127).wrapping_add(192);
                            *position = (i64::from(*position) + i64::from(low as i8) * iterations as i64) as i16;
                        }
                    }
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(authored_paths::PART_JITTER_FADE_SPRITE);
                    actor.base.position = initial_position;
                    actor.base.hit_points = 100;
                    actor.base.wait_timer = part;
                    actor.extension.depth_offset = 0xABCD;
                    actor.extension.path_state.part = part;
                    actor.extension.path_state.motion_phase = u16::from_be_bytes([high, 173]);
                    runtime.branch.invert_next = invert;
                    // Small slices prove that immediate loops resume without
                    // inventing movement, reinitializing, or drawing again.
                    let mut result = runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 127);
                    let mut slices = 0;
                    while let Err(ProgramError::BudgetExceeded { executed, cursor }) = result {
                        assert_eq!(executed, 127);
                        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor));
                        assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
                        slices += 1;
                        assert!(slices < 4000);
                        result = runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 127);
                    }
                    assert_eq!(result, Ok(ControlStep::Movement));
                    assert_eq!(slices, (if skip { 10 } else { 20 + 6 * iterations } - 1) / 127);
                    for visit in 0..8 {
                        if visit != 0 {
                            assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 4),
                                Ok(if visit == 7 { ControlStep::Ended } else { ControlStep::Movement }));
                        }
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.base.position, expected_position, "part={part} high={high} invert={invert}");
                        assert_eq!(actor.extension.path_state.part, expected_part);
                        assert_eq!(actor.extension.path_state.motion_phase, u16::from_be_bytes([high, low]));
                        assert_eq!(actor.extension.path_state.animation.color.fixed_frame(), Some(visit));
                        assert_eq!(actor.extension.texture_scroll_x, 8 + power);
                        assert_eq!(actor.extension.depth_offset, 0xAB00);
                        assert_eq!((actor.base.hit_points, actor.base.attack_power, actor.base.wait_timer), (100, power, part));
                        assert!(actor.base.flags.scaled_sprite);
                        assert!(actor.base.flags.collision_disabled);
                        assert_eq!(actor.base.flags.remove_after_tick, visit == 7);
                        assert!(!runtime.branch.invert_next);
                        assert_eq!(random, expected_random);
                    }
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn direct_fade_sprite_has_no_implicit_sprite_initialization_or_randomization() {
        use super::super::{authored_paths, Vector3};
        let catalog = authored_paths::catalog();
        for retained in 0..=u8::MAX {
            for sprite in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::FADE_SPRITE);
                actor.base.flags.scaled_sprite = sprite;
                actor.base.position = Vector3 { x: -32768, y: 1234, z: 32767 };
                actor.base.wait_timer = retained;
                actor.base.attack_power = retained;
                actor.extension.texture_scroll_x = retained;
                actor.extension.depth_offset = 0xABCD;
                actor.extension.path_state.part = retained;
                actor.extension.path_state.motion_phase = 0xFEDC;
                runtime.branch.invert_next = true;
                let initial_random = random;
                for visit in 0..8 {
                    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 4),
                        Ok(if visit == 7 { ControlStep::Ended } else { ControlStep::Movement }));
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.extension.path_state.animation.color.fixed_frame(), Some(visit));
                    assert_eq!(actor.base.position, Vector3 { x: -32768, y: 1234, z: 32767 });
                    assert_eq!(actor.base.wait_timer, retained);
                    assert_eq!(actor.base.attack_power, retained);
                    assert_eq!(actor.extension.texture_scroll_x, retained);
                    assert_eq!(actor.extension.depth_offset, 0xABCD);
                    assert_eq!(actor.extension.path_state.part, retained);
                    assert_eq!(actor.extension.path_state.motion_phase, 0xFEDC);
                    assert_eq!(actor.base.flags.scaled_sprite, sprite);
                    assert!(actor.base.flags.collision_disabled);
                    assert_eq!(actor.base.flags.remove_after_tick, visit == 7);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, initial_random);
                }
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }

    #[test]
    fn growing_sprite_holds_after_fifteen_growth_steps_and_keeps_sprite_mode_on_shape_change() {
        use super::super::{authored_paths, Vector3};
        let catalog = authored_paths::catalog();
        for retained in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::GROWING_SPRITE_HOLD);
            actor.base.shape = ShapeId::from_catalog_index(16);
            actor.base.wait_timer = retained;
            actor.base.position = Vector3 { x: -32768, y: 1234, z: 32767 };
            actor.base.hit_points = 10;
            actor.base.attack_power = 10;
            actor.extension.depth_offset = u16::from_be_bytes([retained, 255]);
            actor.extension.texture_scroll_x = retained;
            actor.extension.texture_scroll_y = retained;
            let original_animation = actor.extension.path_state.animation;
            runtime.branch.invert_next = true;
            let initial_random = random;
            for visit in 0..20 {
                assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8), Ok(ControlStep::Movement));
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.extension.texture_scroll_x, 16 + 2 * (visit + 1).min(15));
                assert_eq!(actor.extension.texture_scroll_y, retained);
                assert_eq!(actor.extension.depth_offset, u16::from(retained) << 8);
                assert_eq!(actor.base.shape, ShapeId::from_catalog_index(if visit < 15 { 16 } else { 18 }));
                assert!(actor.base.flags.scaled_sprite);
                assert!(actor.base.flags.collision_disabled);
                assert!(actor.base.flags.visible);
                assert!(!actor.base.flags.remove_after_tick);
                assert_eq!(actor.extension.path_state.animation, original_animation);
                assert_eq!(actor.base.position, Vector3 { x: -32768, y: 1234, z: 32767 });
                assert_eq!(actor.base.wait_timer, retained);
                assert_eq!((actor.base.hit_points, actor.base.attack_power), (10, 10));
                assert_eq!(actor.extension.path_state.hold_latched, visit >= 15);
                if visit >= 15 {
                    assert_eq!(actor.base.behavior, Behavior::PathMovement);
                    assert!(matches!(catalog.statement(actor.base.path.unwrap()), Ok(Statement::Control(ControlCommand::Hold))));
                }
                assert!(runtime.branch.invert_next);
                assert_eq!(random, initial_random);
            }
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }

    #[test]
    fn shape_filtered_scenery_adds_only_two_warning_shapes_and_enters_unsuspended_hold() {
        use super::super::{authored_paths, render::MaterialSetId, Vector3};
        let catalog = authored_paths::catalog();
        let original_material = MaterialSetId::from_catalog_token(33_534);
        for shape in 0..577 {
            for previously_member in [false, true] {
                for (configuration, location) in [(0, 2), (9, 2), (9, 5), (9, 255)] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(authored_paths::SHAPE_FILTERED_SCENERY);
                    actor.base.shape = ShapeId::from_catalog_index(shape);
                    actor.base.flags.proximity_warning_source = previously_member;
                    actor.base.flags.proximity_warning_latched = true;
                    actor.base.flags.exclude_from_shape_footprint_search = true;
                    actor.base.flags.casts_shadow = true;
                    actor.base.flags.maximum_draw_distance = true;
                    actor.base.position = Vector3 { x: -1234, y: 2345, z: 3456 };
                    actor.base.wait_timer = 193;
                    actor.extension.path_state.motion_phase = 0xABCD;
                    actor.extension.material_set = Some(original_material);
                    runtime.branch.invert_next = true;
                    let initial_random = random;
                    let mut inputs = world(&mut random);
                    inputs.scene.player_configuration = Some(configuration);
                    inputs.scene.encounter_location = (configuration == 9).then_some(location);
                    for visit in 0..3 {
                        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, if visit == 0 { 32 } else { 1 }), Ok(ControlStep::Movement));
                        let actor = objects.get(owner).unwrap();
                        let material = match (configuration, location) {
                            (9, 2) => MaterialSetId::from_catalog_token(33_796),
                            (9, 5) => MaterialSetId::from_catalog_token(33_944),
                            _ => original_material,
                        };
                        assert_eq!(actor.extension.material_set, Some(material));
                        assert_eq!(actor.base.shape, ShapeId::from_catalog_index(shape));
                        assert_eq!(actor.base.flags.proximity_warning_source, previously_member || [145, 201].contains(&shape));
                        assert!(actor.base.flags.proximity_warning_latched);
                        assert!(!actor.base.flags.exclude_from_shape_footprint_search);
                        assert!(!actor.base.flags.casts_shadow);
                        assert!(!actor.base.flags.maximum_draw_distance);
                        assert!(!actor.base.flags.strategy_suspended);
                        assert!(!actor.base.flags.remove_after_tick);
                        assert!(actor.base.flags.collision_disabled);
                        assert!(actor.base.contacts.suppress_contacts_next_epoch);
                        assert_eq!((actor.base.hit_points, actor.base.attack_power), (100, 4));
                        assert_eq!(actor.base.position, Vector3 { x: -1234, y: 2345, z: 3456 });
                        assert_eq!(actor.base.wait_timer, 193);
                        assert_eq!(actor.extension.path_state.motion_phase, 0xABCD);
                        assert!(actor.extension.path_state.hold_latched);
                        assert_eq!(actor.base.behavior, Behavior::PathMovement);
                        assert!(matches!(catalog.statement(actor.base.path.unwrap()), Ok(Statement::Control(ControlCommand::Hold))));
                        assert!(!runtime.branch.invert_next);
                    }
                    assert_eq!(random, initial_random);
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn hit_toggle_sprite_entries_preserve_wrapping_parameters_and_exact_counted_timing() {
        use super::super::authored_paths;
        let catalog = authored_paths::catalog();
        for (root, increments) in [(authored_paths::HIT_TOGGLE_SPRITE, false),
            (authored_paths::COUNTED_HIT_TOGGLE_SPRITE, true)] {
            for parameter in 0..=u8::MAX {
                for health in [0, 1, 2, 255] {
                    if health == 0 && ![0, 1, 255].contains(&parameter) { continue; }
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(root);
                    actor.base.hit_points = health;
                    actor.base.attack_power = parameter;
                    actor.extension.path_state.script_parameter = parameter;
                    actor.extension.depth_offset = 0xABCD;
                    runtime.branch.invert_next = true;
                    let initial_random = random;
                    let effective_parameter = parameter.wrapping_add(u8::from(increments));
                    // DOV widens the byte into the word-sized loop counter:
                    // zero takes 65,536 NEXT decrements, not 256.
                    let iterations = if health == 0 { 65_536 } else { usize::from(health) };
                    let visits = if effective_parameter == 0 { 32 } else { 3 + 2 * iterations };
                    for visit in 0..visits {
                        let ending = effective_parameter != 0 && visit + 1 == visits;
                        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 16),
                            Ok(if ending { ControlStep::Ended } else { ControlStep::Movement }),
                            "increments={increments} parameter={parameter} health={health} visit={visit}");
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.extension.path_state.animation.color.fixed_frame(),
                            Some(if visit < 3 { visit as u8 + 1 } else { ((visit - 3) % 2) as u8 }));
                        assert_eq!(actor.extension.path_state.script_parameter, effective_parameter);
                        assert_eq!(actor.extension.texture_scroll_x, 8u8.wrapping_add(parameter));
                        assert_eq!(actor.extension.depth_offset, 0xAB00);
                        assert_eq!(actor.base.hit_points, health);
                        assert!(actor.base.flags.scaled_sprite);
                        assert!(actor.base.flags.visible);
                        assert!(actor.base.flags.collision_disabled);
                        assert!(actor.base.flags.maximum_draw_distance);
                        assert!(actor.base.contacts.run_when_paused);
                        assert_eq!(actor.base.flags.remove_after_tick, ending);
                        assert_eq!(actor.base.wait_timer, 0);
                        assert!(runtime.branch.invert_next);
                        assert_eq!(random, initial_random);
                    }
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn hit_toggle_sprite_callbacks_switch_visibility_then_restart_fade_without_reinitializing_sprite() {
        use super::super::{authored_paths, path_triggers::TriggerKind};
        let catalog = authored_paths::catalog();
        for hit_visit in 0..8 {
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.path = Some(authored_paths::HIT_TOGGLE_SPRITE);
            objects.get_mut(owner).unwrap().base.attack_power = 37;
            runtime.branch.invert_next = true;
            let initial_random = random;
            for _ in 0..=hit_visit {
                assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 16),
                    Ok(ControlStep::Movement));
            }
            for visible in [false, true, false, true] {
                let saved_main = objects.get(owner).unwrap().base.path;
                // A pass without a new hit neither toggles nor redirects.
                assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Skipped));
                assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Complete));
                assert_eq!(objects.get(owner).unwrap().base.path, saved_main);
                objects.get_mut(owner).unwrap().extension.path_state.conditions.hit_event_pending = true;
                assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                assert!(matches!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Run(_))));
                assert!(!objects.get(owner).unwrap().extension.path_state.conditions.hit_event_pending);
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 4), Ok(ControlStep::ResumeCallbacks));
                assert_eq!(runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()), Ok(CallbackStep::Complete));
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.flags.visible, visible);
                assert!(actor.base.flags.collision_disabled);
                let redirected = actor.base.path;
                assert_ne!(redirected, saved_main);
                assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 12), Ok(ControlStep::Movement));
                let actor = objects.get(owner).unwrap();
                let triggers = actor.extension.path_state.triggers.entries(&runtime.resources, owner).unwrap();
                assert_eq!(triggers.len(), 1);
                assert_eq!(triggers[0].kind, TriggerKind::ConsumeHitEvent);
                assert_eq!(actor.extension.texture_scroll_x, 45);
                assert_eq!(actor.base.flags.visible, visible);
                if visible {
                    assert_eq!(actor.extension.path_state.animation.color.fixed_frame(), Some(1));
                } else {
                    let held = actor.base.path;
                    for _ in 0..3 {
                        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1), Ok(ControlStep::Movement));
                        assert_eq!(objects.get(owner).unwrap().base.path, held);
                    }
                }
                assert!(runtime.branch.invert_next);
                assert_eq!(random, initial_random);
            }
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }

    #[test]
    fn attachment_absence_branch_ignores_health_retirement_and_slot_liveness() {
        let catalog = PathCatalog::new(vec![vec![Statement::AttachmentAbsent {
            taken: cursor(0, 2), next: cursor(0, 1),
        }]]).unwrap();
        for inverted in [false, true] {
            for owner_health in [0, 1, u8::MAX] {
                for linked_health in [0, 1, u8::MAX] {
                    // Absent, allocated, queued for retirement, already
                    // released, and self-linked are distinct pointer cases.
                    for link_case in 0..5 {
                        let (mut runtime, mut objects, owner, mut random) = setup();
                        let mut linked = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
                        linked.base.hit_points = linked_health;
                        linked.base.flags.remove_after_tick = link_case == 2;
                        let target = objects.allocate(linked).unwrap();
                        if link_case == 3 { objects.remove(target).unwrap(); }
                        let actor = objects.get_mut(owner).unwrap();
                        actor.base.attachment = match link_case {
                            0 => None,
                            4 => Some(owner),
                            _ => Some(target),
                        };
                        actor.base.hit_points = owner_health;
                        actor.base.wait_timer = 193;
                        actor.base.flags.remove_after_tick = true;
                        runtime.branch.invert_next = inverted;
                        let before = objects.clone();
                        let initial_random = random;
                        let mut inputs = world(&mut random);
                        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                            Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                        assert_eq!(objects, before);
                        let destination = cursor(0, if link_case == 0 { 2 } else { 1 });
                        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                            Err(ProgramError::BudgetExceeded { cursor: destination, executed: 1 }));
                        let mut expected = before;
                        expected.get_mut(owner).unwrap().base.path = Some(destination);
                        assert_eq!(objects, expected);
                        assert_eq!(runtime.branch.invert_next, inverted);
                        assert_eq!(random, initial_random);
                    }
                }
            }
        }
    }

    #[test]
    fn targeting_upgrade_commands_preserve_other_pilot_bits_and_bypass_ifnot() {
        use super::super::path_target::TargetingUpgradeState;
        for flags in 0..=u8::MAX {
            for inverted in [false, true] {
                for acquiring in [false, true] {
                    let statement = if acquiring { Statement::AcquireTargetingUpgrade { next: cursor(0, 1) } }
                        else { Statement::TargetingUpgradeOwned { taken: cursor(0, 2), next: cursor(0, 1) } };
                    let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    runtime.branch.invert_next = inverted;
                    objects.get_mut(owner).unwrap().base.wait_timer = 193;
                    let before = objects.clone();
                    let initial_random = random;
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                        Err(ProgramError::MissingTargetingUpgrade));
                    assert_eq!(objects, before);
                    assert_eq!(runtime.branch.invert_next, inverted);
                    let mut upgrade = TargetingUpgradeState { pilot_flags: flags };
                    let mut inputs = world(&mut random);
                    inputs.targeting_upgrade = Some(&mut upgrade);
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                        Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                    assert_eq!(objects, before);
                    assert_eq!(inputs.targeting_upgrade.as_ref().unwrap().pilot_flags, flags);
                    let next = cursor(0, if !acquiring && flags & 0x80 != 0 { 2 } else { 1 });
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded { cursor: next, executed: 1 }));
                    let mut expected = before;
                    expected.get_mut(owner).unwrap().base.path = Some(next);
                    assert_eq!(objects, expected);
                    assert_eq!(upgrade.pilot_flags, if acquiring { flags | 0x80 } else { flags });
                    assert_eq!(runtime.branch.invert_next, inverted);
                    assert_eq!(random, initial_random);
                }
            }
        }
    }

    #[test]
    fn targeting_upgrade_glow_runs_independently_with_alternating_yielded_frames() {
        use super::super::authored_paths;
        let catalog = authored_paths::catalog();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::TARGETING_UPGRADE_GLOW);
        actor.base.shape = ShapeId::from_catalog_index(516);
        actor.base.hit_points = 10;
        runtime.branch.invert_next = true;
        let initial_random = random;
        for visit in 0..256 {
            assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 4), Ok(ControlStep::Movement));
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.extension.path_state.animation.color.fixed_frame(), Some(if visit % 2 == 0 { 2 } else { 3 }));
            assert_eq!(actor.base.hit_points, 10);
            assert!(actor.base.flags.collision_disabled);
            assert!(actor.base.flags.visible);
            assert!(!actor.base.flags.remove_after_tick);
            assert_eq!(actor.base.wait_timer, 0);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, initial_random);
        }
    }

    #[test]
    fn scene_imports_require_only_the_selected_input_and_preserve_full_byte_values() {
        use super::super::path_fields::BytePart;
        let destination = ByteField::WordPart { field: WordField::MotionPhase, part: BytePart::Low };
        for source in [SceneByte::PlayerConfiguration, SceneByte::EncounterLocation, SceneByte::ActiveWeaponLevel] {
            let catalog = PathCatalog::new(vec![vec![Statement::ImportSceneByte {
                source, destination, next: cursor(0, 1),
            }]]).unwrap();
            for value in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                runtime.branch.invert_next = true;
                objects.get_mut(owner).unwrap().extension.path_state.motion_phase = 0xA57E;
                let before = objects.clone();
                let initial_random = random;
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                    Err(ProgramError::MissingSceneByte(source)));
                assert_eq!(objects, before);
                let mut inputs = world(&mut random);
                match source {
                    SceneByte::PlayerConfiguration => inputs.scene.player_configuration = Some(value),
                    SceneByte::EncounterLocation => inputs.scene.encounter_location = Some(value),
                    SceneByte::ActiveWeaponLevel => inputs.scene.active_weapon_level = Some(value),
                }
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                    Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                assert_eq!(objects, before);
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded { cursor: cursor(0, 1), executed: 1 }));
                let mut expected = before;
                let actor = expected.get_mut(owner).unwrap();
                actor.extension.path_state.motion_phase = 0xA500 | u16::from(value);
                actor.base.path = Some(cursor(0, 1));
                assert_eq!(objects, expected);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, initial_random);
            }
        }
    }

    #[test]
    fn scene_material_scenery_covers_all_selector_bytes_and_restores_saved_phase() {
        use super::super::{authored_paths, render::MaterialSetId};
        let catalog = authored_paths::catalog();
        let original_material = MaterialSetId::from_catalog_token(33_534);
        for configuration in 0..=u8::MAX {
            for location in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let original_random = random;
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::SCENE_MATERIAL_SCENERY);
                actor.base.shape = ShapeId::from_catalog_index(429);
                actor.base.flags.casts_shadow = true;
                actor.base.flags.maximum_draw_distance = true;
                actor.extension.path_state.motion_phase = u16::from_be_bytes([location, configuration]);
                actor.extension.material_set = Some(original_material);
                let phase = actor.extension.path_state.motion_phase;
                let position = actor.base.position;
                let mut inputs = world(&mut random);
                inputs.scene.player_configuration = Some(configuration);
                // The early branch must never demand the skipped observation.
                if configuration == 9 {
                    inputs.scene.encounter_location = Some(location);
                }
                assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 30), Ok(ControlStep::Movement));
                let actor = objects.get(owner).unwrap();
                let expected = match (configuration, location) {
                    (9, 2) => MaterialSetId::from_catalog_token(33_796),
                    (9, 5) => MaterialSetId::from_catalog_token(33_944),
                    _ => original_material,
                };
                assert_eq!(actor.extension.material_set, Some(expected));
                assert_eq!(actor.extension.path_state.motion_phase, phase);
                assert_eq!(actor.base.position, position);
                assert_eq!(actor.base.hit_points, 100);
                assert!(actor.base.flags.collision_disabled);
                assert!(!actor.base.flags.casts_shadow);
                assert!(!actor.base.flags.maximum_draw_distance);
                assert!(actor.base.flags.strategy_suspended);
                assert!(!actor.extension.path_state.hold_latched);
                assert!(matches!(catalog.statement(actor.base.path.unwrap()), Ok(Statement::Control(ControlCommand::SuspendAndMove))));
                assert!(actor.base.flags.proximity_warning_source);
                assert!(!runtime.branch.invert_next);
                assert_eq!(random, original_random);
            }
        }
    }

    #[test]
    fn scene_scenery_warning_membership_only_adds_three_shapes_and_location_is_sampled_late() {
        use super::super::{authored_paths, render::MaterialSetId};
        let catalog = authored_paths::catalog();
        for shape in [0, 428, 429, 430, 431, 432] {
            for already_member in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::SCENE_MATERIAL_SCENERY);
                actor.base.shape = ShapeId::from_catalog_index(shape);
                actor.base.flags.proximity_warning_source = already_member;
                actor.base.flags.proximity_warning_latched = true;
                actor.extension.path_state.motion_phase = 0xA57E;
                let mut inputs = world(&mut random);
                inputs.scene.player_configuration = Some(9);
                assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 30),
                    Err(ProgramError::MissingSceneByte(SceneByte::EncounterLocation)));
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.extension.path_state.motion_phase, 0xA509);
                assert_eq!(actor.base.flags.proximity_warning_source, already_member);
                // The failed import retains the saved byte and return address;
                // the resumed command samples location, not configuration again.
                inputs.scene.player_configuration = None;
                inputs.scene.encounter_location = Some(5);
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 30), Ok(ControlStep::Movement));
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.extension.path_state.motion_phase, 0xA57E);
                assert_eq!(actor.extension.material_set, Some(MaterialSetId::from_catalog_token(33_944)));
                assert_eq!(actor.base.flags.proximity_warning_source, already_member || (429..=431).contains(&shape));
                assert!(actor.base.flags.proximity_warning_latched);
            }
        }
    }

    #[test]
    fn material_selection_preserves_other_actor_state_and_respects_command_budget() {
        use super::super::{path_appearance::AppearanceCommand, render::MaterialSetId};
        for token in [33_796, 33_944] {
            let material = MaterialSetId::from_catalog_token(token);
            let catalog = PathCatalog::new(vec![vec![Statement::Appearance {
                command: AppearanceCommand::MaterialSet(material),
                next: cursor(0, 1),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            let original_random = random;
            runtime.branch.invert_next = true;
            let actor = objects.get_mut(owner).unwrap();
            actor.base.wait_timer = 233;
            actor.base.flags.visible = false;
            actor.base.flags.collided = true;
            actor.base.flags.collision_disabled = true;
            actor.extension.animation_frame = 71;
            actor.extension.color_frame = 39;
            actor.extension.material_set = Some(MaterialSetId::from_catalog_token(33_534));
            let before = objects.clone();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
                Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 })
            );
            assert_eq!(objects, before);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded { cursor: cursor(0, 1), executed: 1 })
            );
            let mut expected = before;
            let actor = expected.get_mut(owner).unwrap();
            actor.extension.material_set = Some(material);
            actor.base.path = Some(cursor(0, 1));
            assert_eq!(objects, expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn proximity_warning_controls_preserve_latch_and_only_change_scan_membership() {
        for enabled in [false, true] {
            let catalog = PathCatalog::new(vec![vec![Statement::Appearance {
                command: super::super::path_appearance::AppearanceCommand::ProximityWarningSource(
                    enabled,
                ),
                next: cursor(0, 1),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            let original_random = random;
            runtime.branch.invert_next = true;
            let actor = objects.get_mut(owner).unwrap();
            actor.base.flags.proximity_warning_source = !enabled;
            actor.base.flags.proximity_warning_latched = true;
            actor.base.wait_timer = 233;
            let before = objects.clone();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 0),
                    executed: 0
                })
            );
            assert_eq!(objects, before);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            let mut expected = before;
            let actor = expected.get_mut(owner).unwrap();
            actor.base.flags.proximity_warning_source = enabled;
            actor.base.path = Some(cursor(0, 1));
            assert_eq!(objects, expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn suspension_runs_current_movement_and_callbacks_without_becoming_path_hold() {
        use super::super::path_motion::PlayerDisplacement;
        let catalog = PathCatalog::new(vec![vec![
            Statement::Control(ControlCommand::SuspendAndMove),
            Statement::Control(ControlCommand::Return),
        ]])
        .unwrap();
        for held in [false, true] {
            for relative in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let original_random = random;
                runtime.branch.invert_next = true;
                let actor = objects.get_mut(owner).unwrap();
                actor.extension.path_state.hold_latched = held;
                actor.extension.path_state.motion.relative_coordinates = relative;
                actor.base.position.x = 100;
                actor.base.velocity.x = 10;
                actor.base.wait_timer = 199;
                actor.base.contacts.new_contact_latched = true;
                runtime
                    .add_trigger(
                        &mut objects,
                        owner,
                        Trigger {
                            path: cursor(0, 1),
                            kind: TriggerKind::Always,
                            timer: 0,
                        },
                    )
                    .unwrap();
                let before = objects.clone();
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        0,
                    ),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 0),
                        executed: 0
                    })
                );
                assert_eq!(objects, before);
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1,
                    ),
                    Ok(ControlStep::Movement)
                );
                let mut expected = before;
                expected
                    .get_mut(owner)
                    .unwrap()
                    .base
                    .flags
                    .strategy_suspended = true;
                assert_eq!(objects, expected);
                assert!(runtime
                    .begin_movement(&mut objects, owner, PlayerDisplacement::default())
                    .unwrap());
                assert_eq!(
                    objects.get(owner).unwrap().base.position.x,
                    if relative { 100 } else { 110 }
                );
                assert_eq!(
                    runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                    Ok(CallbackStep::Run(cursor(0, 1)))
                );
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1,
                    ),
                    Ok(ControlStep::ResumeCallbacks)
                );
                assert_eq!(
                    runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                    Ok(CallbackStep::Complete)
                );
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
                let actor = objects.get(owner).unwrap();
                assert_eq!(
                    actor.extension.relative_position.x,
                    if relative { 10 } else { 0 }
                );
                assert_eq!(actor.base.behavior, Behavior::FollowPath);
                assert_eq!(actor.base.path, Some(cursor(0, 0)));
                assert_eq!(actor.base.wait_timer, 199);
                assert_eq!(actor.extension.path_state.hold_latched, held);
                assert!(actor.base.flags.strategy_suspended);
                assert!(!actor.base.flags.remove_after_tick);
                assert!(!actor.base.contacts.new_contact_latched);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, original_random);
            }
        }
    }

    #[test]
    fn radar_marker_assignment_preserves_other_actor_state_and_reaches_native_projection() {
        use super::super::path_appearance::AppearanceCommand;
        use super::super::radar::{RadarMarker, RadarView};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let original_random = random;
        runtime.branch.invert_next = true;
        objects.get_mut(owner).unwrap().base.wait_timer = 199;
        let view = RadarView {
            center: Default::default(),
            screen_origin: [100, 80],
            scale_shift: 0,
            clip_span: 8192,
            clip_bias: 4096,
        };
        for value in 0..=u8::MAX {
            let marker = RadarMarker::from_packed(value);
            let catalog = PathCatalog::new(vec![vec![Statement::Appearance {
                command: AppearanceCommand::RadarMarker(marker),
                next: cursor(0, 1),
            }]])
            .unwrap();
            objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
            let before = objects.clone();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 0),
                    executed: 0
                })
            );
            assert_eq!(objects, before);
            let mut expected = before;
            let actor = expected.get_mut(owner).unwrap();
            actor.extension.radar_marker = marker;
            actor.base.path = Some(cursor(0, 1));
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(objects, expected);
            assert_eq!(
                view.project_actor(objects.get(owner).unwrap()).is_some(),
                value != 0
            );
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn indexed_word_bit_branches_resample_live_fields_and_leave_ifnot_pending() {
        const MASKS: [u16; 128] = {
            let mut masks = [0; 128];
            let mut index = 0;
            while index < masks.len() {
                masks[index] = (index as u16 * 257) ^ 0x5AA5;
                index += 1;
            }
            masks
        };
        let catalog = PathCatalog::new(vec![vec![
            Statement::Compare {
                condition: ActorCondition::AnyWordBitsSet(
                    WordOperand::Actor(WordField::MotionPhase),
                    WordOperand::IndexedBitMask {
                        selector: ByteOperand::Actor(ByteField::Health),
                        masks: &MASKS,
                    },
                ),
                taken: cursor(0, 1),
                next: cursor(0, 2),
            },
            Statement::Control(ControlCommand::End),
            Statement::Control(ControlCommand::End),
        ]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let original_random = random;
        for selector in 0..=u8::MAX {
            let mask = MASKS[usize::from(selector.wrapping_sub(1).wrapping_mul(2)) / 2];
            for value in [0, mask, !mask, u16::MAX] {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(cursor(0, 0));
                actor.base.hit_points = selector;
                actor.extension.path_state.motion_phase = value;
                runtime.branch.invert_next_condition();
                let mut expected = actor.clone();
                let next = cursor(0, if value & mask != 0 { 1 } else { 2 });
                expected.base.path = Some(next);
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::BudgetExceeded {
                        cursor: next,
                        executed: 1
                    })
                );
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert!(runtime.branch.invert_next);
            }
        }
        assert_eq!(random, original_random);
    }

    #[test]
    fn marker_sound_faults_before_mutation_and_resamples_fixed_marker_inputs() {
        use super::super::path_sound::{
            marker_cue, CueListener, CueMarker, MarkerCueMode, MarkerInputs, MarkerRange,
            PathAudio, PathSoundClass,
        };
        use super::super::{Angle, AudioState, SoundEvent, Vector3};
        for mode in [
            MarkerCueMode::DistanceBands(PathSoundClass::DistanceOnly),
            MarkerCueMode::DistanceBands(PathSoundClass::Positioned),
            MarkerCueMode::RangeLimited(MarkerRange::Near),
            MarkerCueMode::RangeLimited(MarkerRange::Wide),
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let original_random = random;
            let catalog = PathCatalog::new(vec![vec![Statement::MarkerSound {
                id: 255,
                mode,
                next: cursor(0, 1),
            }]])
            .unwrap();
            let before = objects.get(owner).unwrap().clone();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingAudio)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            let mut audio = AudioState::default();
            audio.queue(SoundEvent::HostileLaser);
            let mut inputs = world(&mut random);
            inputs.audio = Some(PathAudio {
                events: &mut audio,
                listeners: [CueListener::Other, CueListener::PrimaryFallback],
                markers: None,
            });
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::MissingSoundMarkers)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            assert_eq!(
                inputs
                    .audio
                    .as_mut()
                    .unwrap()
                    .events
                    .take_events()
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>(),
                vec![SoundEvent::HostileLaser]
            );
            for selected in [PlayerTarget::Primary, PlayerTarget::Secondary] {
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .selected_player = selected;
                runtime.enter(&objects, owner).unwrap();
                // The retained runtime selection must survive a change in the
                // owner's selection flag. Marker side is independently live.
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .selected_player = if selected == PlayerTarget::Primary {
                    PlayerTarget::Secondary
                } else {
                    PlayerTarget::Primary
                };
                for side in [PlayerTarget::Primary, PlayerTarget::Secondary] {
                    for distance in [0_i16, 799, 800, 1299, 1300, 5119, 5120, 32767] {
                        let markers = [
                            CueMarker {
                                identity: CueListener::PrimaryFallback,
                                position: Vector3 {
                                    x: 100,
                                    y: i16::MAX,
                                    z: 200,
                                },
                                bearing: Angle::from_units(64),
                            },
                            CueMarker {
                                identity: CueListener::Other,
                                position: Vector3 {
                                    x: -300,
                                    y: i16::MIN,
                                    z: -400,
                                },
                                bearing: Angle::from_units(192),
                            },
                        ];
                        let opposite = if side == PlayerTarget::Primary {
                            PlayerTarget::Secondary
                        } else {
                            PlayerTarget::Primary
                        };
                        let selected_sides = if selected == PlayerTarget::Primary {
                            [side, opposite]
                        } else {
                            [opposite, side]
                        };
                        let marker = markers[usize::from(side == PlayerTarget::Secondary)];
                        inputs.audio.as_mut().unwrap().markers = Some(MarkerInputs {
                            selected_sides,
                            markers,
                        });
                        let actor = objects.get_mut(owner).unwrap();
                        actor.base.path = Some(cursor(0, 0));
                        actor.base.position = Vector3 {
                            x: marker.position.x,
                            y: 777,
                            z: marker.position.z.wrapping_add(distance),
                        };
                        actor.base.wait_timer = 57;
                        actor.base.yaw = Angle::from_units(17);
                        let mut expected = actor.clone();
                        expected.base.path = Some(cursor(0, 1));
                        let expected_cue = marker_cue(255, mode, actor.base.position, marker);
                        inputs
                            .audio
                            .as_mut()
                            .unwrap()
                            .events
                            .queue(SoundEvent::HostileLaser);
                        runtime.branch.invert_next = true;
                        assert_eq!(
                            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                            Err(ProgramError::BudgetExceeded {
                                cursor: cursor(0, 1),
                                executed: 1
                            })
                        );
                        assert_eq!(objects.get(owner).unwrap(), &expected);
                        assert!(runtime.branch.invert_next);
                        assert_eq!(runtime.selected_player(), selected);
                        let mut expected_events = vec![SoundEvent::HostileLaser];
                        expected_events.extend(expected_cue.map(SoundEvent::Authored));
                        assert_eq!(
                            inputs
                                .audio
                                .as_mut()
                                .unwrap()
                                .events
                                .take_events()
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>(),
                            expected_events
                        );
                    }
                }
            }
            assert_eq!(inputs.random, &original_random);
        }
    }

    #[test]
    fn sound_requires_live_shared_audio_and_routes_using_retained_selection() {
        use super::super::path_sound::{AuthoredCue, CueListener, PathAudio};
        use super::super::{AudioState, SoundEvent};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let cue = AuthoredCue::new(18, 127, PlayerTarget::Primary);
        let catalog = PathCatalog::new(vec![vec![
            Statement::Sound {
                cue,
                next: cursor(0, 1),
            },
            Statement::Control(ControlCommand::WaitOne { next: cursor(0, 0) }),
        ]])
        .unwrap();
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 4),
            Err(ProgramError::MissingAudio)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 0)));
        let before_random = random;
        let mut audio = AudioState::default();
        audio.queue(SoundEvent::HostileLaser);
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Secondary;
        let mut inputs = world(&mut random);
        inputs.audio = Some(PathAudio {
            events: &mut audio,
            listeners: [CueListener::Other, CueListener::PrimaryFallback],
            markers: None,
        });
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 4),
            Ok(ControlStep::Movement)
        );
        // Immediate/callback resume uses the retained selected slot, not the
        // owner's subsequently changed condition or spatial selected actor.
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Primary;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 4),
            Ok(ControlStep::Movement)
        );
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 4),
            Ok(ControlStep::Movement)
        );
        assert_eq!(inputs.random, &before_random);
        assert_eq!(
            inputs
                .audio
                .as_mut()
                .unwrap()
                .events
                .take_events()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            vec![
                SoundEvent::HostileLaser,
                SoundEvent::Authored(cue),
                SoundEvent::Authored(cue),
                SoundEvent::Authored(cue.for_listener(CueListener::Other))
            ]
        );
    }

    #[test]
    fn primary_view_latch_preserves_high_phase_and_does_not_use_selected_player() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let primary = objects
            .allocate(Object::new(
                ObjectKind::Enemy,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let catalog = PathCatalog::new(vec![vec![
            Statement::LatchPrimaryViewFilter { next: cursor(0, 1) },
            Statement::Control(ControlCommand::End),
        ]])
        .unwrap();
        let before_random = random;
        let mut inputs = world(&mut random);
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::MissingPrimaryPlayer)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 0)));
        inputs.primary_player = Some(primary);
        inputs.selected = Some(owner);
        runtime.branch.invert_next = true;
        for filtered in [false, true] {
            objects
                .get_mut(primary)
                .unwrap()
                .base
                .flags
                .view_side_filter = filtered;
            for high in [0, 0x7F00, 0x8000, 0xFF00] {
                for low in 0..=u8::MAX {
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(cursor(0, 0));
                    actor.base.flags.view_side_filter = !filtered;
                    actor.extension.path_state.motion_phase = high | u16::from(low);
                    actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
                    let mut expected = actor.clone();
                    expected.base.path = Some(cursor(0, 1));
                    if filtered {
                        expected.extension.path_state.motion_phase = high | 1;
                    }
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(0, 1),
                            executed: 1
                        })
                    );
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
                    assert!(runtime.branch.invert_next);
                }
            }
        }
        assert_eq!(inputs.random, &before_random);
        objects.remove(primary).unwrap();
        objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::Runtime(PathRuntimeError::MissingActor(
                primary
            )))
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 0)));
    }

    #[test]
    fn authored_callback_sprite_loops_then_primary_filter_or_aux_action_redirects_to_end() {
        use super::super::authored_paths;
        for primary_filtered in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let primary = objects
                .allocate(Object::new(
                    ObjectKind::Enemy,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            let catalog = authored_paths::catalog();
            let initial_random = random;
            {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::CALLBACK_GATED_SPRITE);
                actor.extension.path_state.motion_phase = 0xAB00;
                actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
            }
            for visit in 0..5 {
                let mut auxiliary = SelectedAuxiliaryState {
                    mode: 0,
                    action_flags: 0x40,
                };
                let mut inputs = world(&mut random);
                inputs.primary_player = Some(primary);
                inputs.selected = Some(owner);
                inputs.selected_auxiliary = Some(&mut auxiliary);
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16),
                    Ok(ControlStep::Movement)
                );
                let saved_main = objects.get(owner).unwrap().base.path;
                let actor = objects.get(owner).unwrap();
                assert!(actor.base.contacts.run_when_paused);
                assert!(actor.base.flags.collision_disabled);
                assert_eq!(actor.extension.path_state.motion_phase, 0xAB00);
                assert_eq!(
                    actor.extension.texture_scroll_x,
                    if visit < 3 { (visit + 1) * 4 } else { 0 }
                );
                assert_eq!(
                    actor
                        .extension
                        .path_state
                        .triggers
                        .entries(&runtime.resources, owner)
                        .unwrap()
                        .len(),
                    1
                );
                assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                assert!(matches!(
                    runtime
                        .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                        .unwrap(),
                    CallbackStep::Run(_)
                ));
                if visit == 4 {
                    objects
                        .get_mut(primary)
                        .unwrap()
                        .base
                        .flags
                        .view_side_filter = primary_filtered;
                    // A latched phase bypasses the auxiliary condition entirely.
                    if primary_filtered {
                        inputs.selected_auxiliary = None;
                    } else {
                        *inputs.selected_auxiliary.as_deref_mut().unwrap() =
                            SelectedAuxiliaryState {
                                mode: 0x40,
                                action_flags: 0,
                            };
                    }
                }
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 8),
                    Ok(ControlStep::ResumeCallbacks)
                );
                assert_eq!(
                    runtime
                        .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                        .unwrap(),
                    CallbackStep::Complete
                );
                assert_eq!(inputs.random, &initial_random);
                if visit < 4 {
                    assert_eq!(objects.get(owner).unwrap().base.path, saved_main);
                    assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
                } else {
                    let actor = objects.get(owner).unwrap();
                    assert_ne!(actor.base.path, saved_main);
                    assert_eq!(
                        actor.extension.path_state.motion_phase,
                        if primary_filtered { 0xAB01 } else { 0xAB00 }
                    );
                    assert_eq!(
                        catalog.statement(actor.base.path.unwrap()).unwrap(),
                        Statement::Control(ControlCommand::End)
                    );
                    assert!(!actor.base.flags.remove_after_tick);
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 2),
                        Ok(ControlStep::Ended)
                    );
                    assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
                }
            }
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }

    #[test]
    fn authored_repeated_children_run_independent_jitter_sound_and_color_paths() {
        use super::super::path_sound::{AuthoredCue, CueListener, PathAudio};
        use super::super::{
            authored_paths, path_appearance, AudioState, ObjectSpawnDefaults, SoundEvent,
        };
        let (mut runtime, mut objects, owner, mut random) = setup();
        {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::REPEATED_CHILD_SPRITE);
            actor.base.target_speed = 11;
            actor.extension.path_state.motion_phase = 0xAB00;
            actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
        }
        let before_random = random;
        let catalog = authored_paths::catalog();
        let mut audio = AudioState::default();
        let mut children = Vec::new();
        for invocation in 0..11 {
            let mut inputs = world(&mut random);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            // No audio is needed until the independently scheduled child runs.
            let outcome = runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 16)
                .unwrap();
            assert_eq!(
                outcome,
                if invocation < 10 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            assert_eq!(random, before_random);
            if invocation % 5 == 0 {
                let child = runtime.spawns.last_spawn.unwrap();
                assert!(!children.contains(&child));
                children.push(child);
                assert!(
                    objects
                        .get(child)
                        .unwrap()
                        .extension
                        .path_state
                        .needs_path_initialization
                );
            }
            assert_eq!(objects.len(), 1 + children.len());
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase,
                0xAB00 | (4 - invocation % 5)
            );
            // DO snapshots its count; this must not shorten its remaining loop.
            objects.get_mut(owner).unwrap().base.target_speed = 1;
        }
        assert_eq!(children.len(), 3);
        assert_eq!(
            objects.get(owner).unwrap().base.first_child,
            Some(children[0])
        );
        assert_eq!(
            objects.get(children[0]).unwrap().base.next_sibling,
            Some(children[1])
        );
        assert_eq!(
            objects.get(children[1]).unwrap().base.next_sibling,
            Some(children[2])
        );
        let mut expected_random = random;
        for child in children {
            let mut jitter = || {
                let high = expected_random.next_byte();
                let low = expected_random.next_byte();
                (u16::from_be_bytes([high, low]) & 31) as i16 - 15
            };
            let position = super::super::Vector3 {
                x: jitter(),
                y: jitter(),
                z: jitter(),
            };
            for color in 0..4 {
                let mut inputs = world(&mut random);
                inputs.audio = Some(PathAudio {
                    events: &mut audio,
                    listeners: [CueListener::PrimaryPlayer, CueListener::Other],
                    markers: None,
                });
                let outcome = runtime
                    .enter_program(&catalog, &mut objects, child, &mut inputs, 16)
                    .unwrap();
                assert_eq!(
                    outcome,
                    if color < 3 {
                        ControlStep::Movement
                    } else {
                        ControlStep::Ended
                    }
                );
                let actor = objects.get_mut(child).unwrap();
                path_appearance::publish_animation(actor, 29);
                assert_eq!(actor.extension.color_frame, color);
                assert_eq!(actor.extension.texture_scroll_x, 254);
                assert_eq!(actor.extension.relative_position, position);
                assert!(!actor.extension.path_state.needs_path_initialization);
                assert_eq!(random, expected_random);
                assert_eq!(actor.base.flags.remove_after_tick, color == 3);
                let events = audio
                    .take_events()
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                assert_eq!(
                    events,
                    if color == 0 {
                        vec![SoundEvent::Authored(AuthoredCue::new(
                            18,
                            0,
                            PlayerTarget::Secondary,
                        ))]
                    } else {
                        vec![]
                    }
                );
            }
        }
    }

    #[test]
    fn independent_spawning_continues_on_success_and_full_pool_without_running_the_new_path() {
        use super::super::{ObjectSpawnDefaults, OBJECT_CAPACITY};
        for full in [false, true] {
            let catalog = PathCatalog::new(vec![
                vec![
                    Statement::SpawnIndependent {
                        kind: ObjectKind::Effect,
                        parameters: super::super::path_spawn::IndependentSpawn {
                            shape: ShapeId::from_catalog_index(9),
                            path: Some(cursor(1, 0)),
                            hit_points: 255,
                            attack_power: 128,
                        },
                        next: cursor(0, 1),
                    },
                    Statement::Control(ControlCommand::WaitOne { next: cursor(0, 2) }),
                    Statement::Control(ControlCommand::End),
                ],
                vec![Statement::Control(ControlCommand::End)],
            ])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            let before_random = random;
            runtime.branch.invert_next = true;
            runtime.spawns.last_spawn = Some(owner);
            if full {
                for _ in 1..OBJECT_CAPACITY {
                    objects
                        .allocate(Object::new(
                            ObjectKind::Enemy,
                            ShapeId::EMPTY,
                            Behavior::FollowPath,
                        ))
                        .unwrap();
                }
            }
            let before = objects.clone();
            let mut inputs = world(&mut random);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 0),
                    executed: 0
                })
            );
            assert_eq!(objects, before);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 2),
                Ok(ControlStep::Movement)
            );
            assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 2)));
            assert!(runtime.branch.invert_next);
            if full {
                let mut expected = before;
                expected.get_mut(owner).unwrap().base.path = Some(cursor(0, 2));
                assert_eq!(objects, expected);
                assert_eq!(runtime.spawns.last_spawn, Some(owner));
            } else {
                let created = runtime.spawns.last_spawn.unwrap();
                assert_ne!(created, owner);
                assert_eq!(objects.active_ids(), &[owner, created]);
                let actor = objects.get(created).unwrap();
                assert_eq!(actor.base.path, Some(cursor(1, 0)));
                assert_eq!(actor.base.hit_points, 255);
                assert!(actor.extension.path_state.needs_path_initialization);
                assert!(!actor.base.flags.remove_after_tick);
                inputs.spawn_defaults = None;
                let mut spawned_runtime = PathRuntime::default();
                assert_eq!(
                    spawned_runtime.enter_program(&catalog, &mut objects, created, &mut inputs, 1),
                    Ok(ControlStep::Ended)
                );
                assert!(objects.get(created).unwrap().base.flags.remove_after_tick);
            }
            assert_eq!(random, before_random);
        }
    }

    #[test]
    fn independent_spawn_missing_inputs_or_catalog_fault_before_allocation() {
        let catalog = PathCatalog::new(vec![vec![Statement::SpawnIndependent {
            kind: ObjectKind::Effect,
            parameters: super::super::path_spawn::IndependentSpawn {
                shape: ShapeId::EMPTY,
                path: Some(cursor(1, 0)),
                hit_points: 1,
                attack_power: 2,
            },
            next: cursor(0, 1),
        }]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let before = objects.clone();
        runtime.spawns.last_spawn = Some(owner);
        let mut inputs = world(&mut random);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::MissingSpawnDefaults)
        );
        inputs.spawn_defaults = Some(super::super::ObjectSpawnDefaults::default());
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::MissingStatement(cursor(1, 0)))
        );
        assert_eq!(objects, before);
        assert_eq!(runtime.spawns.last_spawn, Some(owner));
    }

    #[test]
    fn spawning_continues_parent_immediately_and_child_runs_only_when_scheduled() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().extension.spawn_group = 45;
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Secondary;
        let catalog = PathCatalog::new(vec![
            vec![
                spawn_statement(),
                Statement::Control(ControlCommand::WaitOne { next: cursor(0, 2) }),
                Statement::Control(ControlCommand::End),
            ],
            vec![
                Statement::Sprite {
                    color: 9,
                    size: 12,
                    next: cursor(1, 1),
                },
                Statement::Control(ControlCommand::End),
            ],
        ])
        .unwrap();
        let before_random = random;
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(super::super::ObjectSpawnDefaults {
            run_when_paused: true,
            group: 99,
        });
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8),
            Ok(ControlStep::Movement)
        );
        let child = runtime.spawns.last_spawn.unwrap();
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 2)));
        assert_eq!(objects.get(owner).unwrap().base.first_child, Some(child));
        assert_eq!(objects.get(child).unwrap().base.path, Some(cursor(1, 0)));
        assert_eq!(objects.get(child).unwrap().extension.texture_scroll_x, 0);
        assert_eq!(objects.get(child).unwrap().extension.spawn_group, 45);
        assert!(objects.get(child).unwrap().base.contacts.run_when_paused);
        assert!(
            objects
                .get(child)
                .unwrap()
                .extension
                .path_state
                .needs_path_initialization
        );
        assert!(!objects.get(child).unwrap().base.flags.casts_shadow);
        assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
        // Neither dispatch nor spawning runs movement or recursively ticks
        // the child; those remain the source strategy scheduler's boundaries.
        assert_eq!(
            objects.get(child).unwrap().base.position,
            super::super::Vector3::default()
        );
        inputs.spawn_defaults = None; // this child's path needs no spawn inputs
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, child, &mut inputs, 8),
            Ok(ControlStep::Ended)
        );
        assert_eq!(objects.get(child).unwrap().extension.texture_scroll_x, 12);
        assert!(
            !objects
                .get(child)
                .unwrap()
                .extension
                .path_state
                .needs_path_initialization
        );
        assert!(objects.get(child).unwrap().base.flags.casts_shadow);
        assert!(
            objects
                .get(child)
                .unwrap()
                .base
                .flags
                .exclude_from_shape_footprint_search
        );
        assert!(objects.get(child).unwrap().base.flags.maximum_draw_distance);
        assert!(objects.get(child).unwrap().base.contacts.latch_new_contact);
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 2)));
        assert_eq!(random, before_random);
        assert_eq!(runtime.spawns.last_spawn, Some(child));
    }

    #[test]
    fn missing_spawn_inputs_or_child_catalog_entry_preserve_allocation_and_caller_cursor() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![spawn_statement()]]).unwrap();
        let before = objects.clone();
        let mut inputs = world(&mut random);
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8),
            Err(ProgramError::MissingSpawnDefaults)
        );
        assert_eq!(objects, before);
        inputs.spawn_defaults = Some(super::super::ObjectSpawnDefaults::default());
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8),
            Err(ProgramError::MissingStatement(cursor(1, 0)))
        );
        assert_eq!(objects, before);
        assert_eq!(runtime.spawns.last_spawn, None);
    }

    #[test]
    fn spawn_failure_does_not_advance_to_a_successful_parent_continuation() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        for _ in 1..super::super::OBJECT_CAPACITY {
            objects
                .allocate(Object::new(
                    ObjectKind::Scenery,
                    ShapeId::EMPTY,
                    Behavior::Effect,
                ))
                .unwrap();
        }
        let before = objects.clone();
        runtime.spawns.last_spawn = Some(owner);
        let catalog = PathCatalog::new(vec![
            vec![spawn_statement(), Statement::Control(ControlCommand::End)],
            vec![Statement::Control(ControlCommand::End)],
        ])
        .unwrap();
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(super::super::ObjectSpawnDefaults::default());
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8),
            Err(ProgramError::Spawn(
                super::super::path_spawn::SpawnError::PoolExhausted
            ))
        );
        assert_eq!(objects, before);
        assert_eq!(runtime.spawns.last_spawn, None);
    }

    fn health(operation: ByteOperation, next: PathCursor) -> Statement {
        Statement::Mutate {
            mutation: Mutation::Byte {
                field: ByteField::Health,
                operation,
            },
            next,
        }
    }

    #[test]
    fn authored_detaching_sprite_only_unlinks_after_last_yield_and_without_action_gate() {
        use super::super::authored_paths;
        for gate in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let mut child = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
            child.base.attachment = Some(owner);
            child.base.child_number = 1;
            child.base.flags.remove_with_parent = true;
            child.extension.path_state.motion.attached_coordinates = true;
            let child = objects.allocate(child).unwrap();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::CHILD_DETACHING_SPRITE);
            actor.base.first_child = Some(child);
            actor.extension.path_state.motion.refresh_child_chain = true;
            actor.extension.path_state.motion_phase = 0xABCD;
            let catalog = authored_paths::catalog();
            let before_random = random;
            let visits = if gate { 2 } else { 3 };
            for visit in 0..visits {
                let mut auxiliary = SelectedAuxiliaryState {
                    mode: 0x80, // mode does not participate in the action-bit gate
                    action_flags: if gate { 0x40 } else { 0 },
                };
                let mut inputs = world(&mut random);
                inputs.selected_auxiliary = Some(&mut auxiliary);
                let outcome = runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 16)
                    .unwrap();
                assert_eq!(
                    outcome,
                    if visit == visits - 1 {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                let linked = gate || visit != visits - 1;
                assert_eq!(
                    objects.get(child).unwrap().base.attachment,
                    linked.then_some(owner)
                );
                assert_eq!(
                    objects.get(owner).unwrap().base.first_child,
                    linked.then_some(child)
                );
                assert_eq!(
                    objects
                        .get(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .motion_phase,
                    0xAB00 | (visit + 1).min(2) as u16
                );
                assert_eq!(objects.get(owner).unwrap().extension.texture_scroll_x, 250);
            }
            assert_eq!(random, before_random);
            assert!(objects.get(child).is_some()); // detachment is not retirement
        }
    }

    #[test]
    fn auxiliary_action_gate_tests_only_action_bit_and_keeps_inversion() {
        for mode in 0..=u8::MAX {
            for action_flags in 0..=u8::MAX {
                let mut branch = super::super::path_conditions::BranchState { invert_next: true };
                assert_eq!(
                    branch.test(
                        SelectedAuxiliaryCondition::ActionBit40
                            .sample(SelectedAuxiliaryState { mode, action_flags })
                    ),
                    action_flags & 0x40 != 0
                );
                assert!(branch.invert_next);
                assert_eq!(
                    branch.test(
                        SelectedAuxiliaryCondition::ActionBit04Clear
                            .sample(SelectedAuxiliaryState { mode, action_flags })
                    ),
                    action_flags & 0x04 == 0
                );
                assert!(branch.invert_next);
            }
        }
    }

    #[test]
    fn auxiliary_clear_action_gate_resamples_and_requires_observation_before_mutation() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let original_random = random;
        runtime.branch.invert_next = true;
        objects.get_mut(owner).unwrap().base.wait_timer = 47;
        let catalog = PathCatalog::new(vec![vec![Statement::SelectedAuxiliaryBranch {
            condition: SelectedAuxiliaryCondition::ActionBit04Clear,
            taken: cursor(0, 2),
            next: cursor(0, 1),
        }]])
        .unwrap();
        let original_objects = objects.clone();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingSelectedAuxiliary)
        );
        assert_eq!(objects, original_objects);
        // One dispatcher repeatedly observes changing flags; no selected
        // pose, mode bit, inversion, or actor field stands in for this byte.
        for action_flags in 0..=u8::MAX {
            objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
            let destination = cursor(0, if action_flags & 0x04 == 0 { 2 } else { 1 });
            let mut expected = objects.clone();
            expected.get_mut(owner).unwrap().base.path = Some(destination);
            let mut auxiliary = SelectedAuxiliaryState {
                mode: !action_flags,
                action_flags,
            };
            let mut inputs = world(&mut random);
            inputs.selected_auxiliary = Some(&mut auxiliary);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: destination,
                    executed: 1
                })
            );
            assert_eq!(objects, expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn variable_loop_snapshots_unsigned_count_once_including_zero_wrap() {
        use super::super::path_fields::{WordField, WordOperation};
        for (initial, wide) in [
            (0, false),
            (1, false),
            (127, false),
            (128, false),
            (255, false),
            (257, true),
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.target_speed = initial as u8;
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .motion_phase = initial;
            let count = if wide {
                WordOperand::Actor(WordField::MotionPhase)
            } else {
                WordOperand::UnsignedByte(ByteOperand::Actor(ByteField::TargetSpeed))
            };
            let catalog = PathCatalog::new(vec![vec![
                Statement::BeginLoop {
                    iterations: count,
                    next: cursor(0, 1),
                },
                // The body overwrites BOTH possible count sources; the loop
                // must keep its saved count rather than sample either again.
                Statement::Mutate {
                    mutation: Mutation::Byte {
                        field: ByteField::TargetSpeed,
                        operation: ByteOperation::Assign(ByteOperand::Literal(0)),
                    },
                    next: cursor(0, 2),
                },
                Statement::Mutate {
                    mutation: Mutation::Word {
                        field: WordField::MotionPhase,
                        operation: WordOperation::Assign(WordOperand::Literal(0)),
                    },
                    next: cursor(0, 3),
                },
                Statement::Control(ControlCommand::Next {
                    immediate: false,
                    next: cursor(0, 4),
                }),
                Statement::Control(ControlCommand::End),
            ]])
            .unwrap();
            let expected = if initial == 0 {
                65536
            } else {
                usize::from(initial)
            };
            for visit in 1..=expected {
                let result = runtime
                    .enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 6)
                    .unwrap();
                assert_eq!(
                    result,
                    if visit == expected {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
            }
            assert_eq!(objects.get(owner).unwrap().base.target_speed, 0);
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase,
                0
            );
        }
    }

    #[test]
    fn authored_auxiliary_sprite_samples_new_input_and_preserves_pending_inversion() {
        use super::super::{authored_paths, path_motion};
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::AUXILIARY_GATED_SPRITE);
        objects.get_mut(owner).unwrap().base.velocity.x = 1;
        runtime.branch.invert_next = true;
        let catalog = authored_paths::catalog();
        for (visit, (size, color)) in [(1, 1), (3, 2), (5, 3), (0, 1), (0, 0)]
            .into_iter()
            .enumerate()
        {
            let mut auxiliary = SelectedAuxiliaryState {
                mode: if visit == 4 { 0x80 } else { 0 },
                action_flags: 0x20,
            };
            let mut inputs = PathWorld {
                scene: ScenePathInputs::default(),
                scenery_distance: None,
                targeting_upgrade: None,
                shield_recovery: None,
                action_gate: None,
                environment_plane_height: None,
                projectile_trigger: None,
                primary_pitch_recoil: None,
                linked_effect_activity: None,
                protection: None,
                audio: None,
                radio: None,
                campaign: None,
                guidance: None,
                pickup_history: None,
                control_style: None,
                selected_occupancy_exempt: None,
                occupancy: None,
                surface_mode: None,
                primary_player: None,
                selected: None,
                fixed_players: [None; 2],
                primary_motion: None,
                published_motion: None,
                active_charge_threshold: None,
                selected_charge: None,
                primary_control: None,
                primary_target: None,
                active_node_flags: None,
                spawn_defaults: None,
                countdown: None,
                // The initial four-count loop does not read this record.
                selected_auxiliary: (visit >= 3).then_some(&mut auxiliary),
                selected_equipment: None,
                selected_score: None,
                random: &mut random,
                animation_clock: 61,
            };
            let outcome = runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 16)
                .unwrap();
            assert_eq!(
                outcome,
                if visit < 4 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            assert!(runtime.branch.invert_next);
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.extension.texture_scroll_x, size);
            assert_eq!(actor.extension.color_frame, color);
            assert_eq!(actor.extension.animation_frame, 61);
            assert!(actor.base.flags.collision_disabled);
            if outcome == ControlStep::Movement {
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
            }
            assert_eq!(
                objects.get(owner).unwrap().base.position.x,
                (visit + 1).min(4) as i16
            );
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }

    #[test]
    fn auxiliary_class_branches_sample_high_mode_nibble_and_preserve_ifnot() {
        use super::super::path_conditions::AuxiliaryModeClass;
        for (class, expected_class) in [
            (AuxiliaryModeClass::One, 1),
            (AuxiliaryModeClass::Two, 2),
            (AuxiliaryModeClass::Three, 3),
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            let initial_random = random;
            let before = objects.get(owner).unwrap().clone();
            let catalog = PathCatalog::new(vec![vec![Statement::SelectedAuxiliaryBranch {
                condition: SelectedAuxiliaryCondition::ModeClass(class),
                taken: cursor(0, 2),
                next: cursor(0, 1),
            }]])
            .unwrap();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingSelectedAuxiliary)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            for mode in 0..=u8::MAX {
                for action_flags in 0..=u8::MAX {
                    *objects.get_mut(owner).unwrap() = before.clone();
                    let mut auxiliary = SelectedAuxiliaryState { mode, action_flags };
                    let mut inputs = world(&mut random);
                    inputs.selected_auxiliary = Some(&mut auxiliary);
                    let destination = cursor(0, if mode / 16 == expected_class { 2 } else { 1 });
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: destination,
                            executed: 1
                        })
                    );
                    let mut expected = before.clone();
                    expected.base.path = Some(destination);
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(inputs.random, &initial_random);
                }
            }
        }
    }

    #[test]
    fn shield_pickup_accumulation_wraps_live_request_and_defers_consumption() {
        use super::super::player_hit_control::{PlayerHitControl, ShieldRecoveryRequest};
        let catalog = PathCatalog::new(vec![vec![Statement::AccumulateShieldRecovery {
            amount: ByteOperand::Actor(ByteField::AttackPower), next: cursor(0, 1),
        }]]).unwrap();
        for pending in 0..=u8::MAX {
            for amount in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let inverted = pending % 2 != 0;
                runtime.branch.invert_next = inverted;
                let actor = objects.get_mut(owner).unwrap();
                actor.base.attack_power = amount;
                actor.base.wait_timer = pending;
                let before = objects.clone();
                let before_random = random;
                let mut request = ShieldRecoveryRequest { amount: pending };
                let mut inputs = world(&mut random);
                inputs.shield_recovery = Some(&mut request);
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                    Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                assert_eq!(inputs.shield_recovery.as_deref().unwrap().amount, pending);
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded { cursor: cursor(0, 1), executed: 1 }));
                let total = ((u16::from(pending) + u16::from(amount)) % 256) as u8;
                assert_eq!(request.amount, total);
                let mut expected = before;
                expected.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
                assert_eq!(objects, expected);
                assert_eq!(random, before_random);
                assert_eq!(runtime.branch.invert_next, inverted);
                // The separate player service consumes this exact shared
                // request. A wrapped-to-zero total requests no recovery.
                let mut player = PlayerHitControl::default();
                player.reserve_shield = 17;
                assert_eq!(request.consume(&mut player, 100), total != 0);
                assert_eq!(request.amount, 0);
                assert_eq!(player.reserve_shield, if total == 0 { 17 } else { ((u16::from(total) + 17) % 256).min(100) as u8 });
            }
        }
        let (mut runtime, mut objects, owner, mut random) = setup();
        let before = objects.clone();
        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingShieldRecovery));
        assert_eq!(objects, before);
    }

    #[test]
    fn selected_score_awards_preserve_high_byte_actor_and_control_state() {
        use super::super::path_score::PlayerScore;
        for points in [0, 1, 100, 255, 256, 32768, 65535] {
            let catalog = PathCatalog::new(vec![vec![Statement::AwardSelectedScore { points, next: cursor(0, 0) }]]).unwrap();
            for value in 0..=u16::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let invert = value % 2 != 0;
                runtime.branch.invert_next = invert;
                objects.get_mut(owner).unwrap().base.wait_timer = value as u8;
                let original_objects = objects.clone();
                let original_random = random;
                let high = (value >> 8) as u8;
                let mut score = PlayerScore::from_parts(value, high);
                let original_score = score;
                let mut inputs = world(&mut random);
                inputs.selected_score = Some(&mut score);
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                    Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                assert_eq!(inputs.selected_score.as_deref(), Some(&original_score));
                for visits in 1..=2 {
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 1 }));
                    let sum = u32::from(value) + u32::from(points) * visits;
                    let expected_low = if sum > 65535 { 65535 } else { sum };
                    assert_eq!(inputs.selected_score.as_deref().unwrap().points(), u32::from(high) * 65536 + expected_low);
                    assert_eq!(objects, original_objects);
                    assert_eq!(inputs.random, &original_random);
                    assert_eq!(runtime.branch.invert_next, invert);
                }
            }
        }
    }

    #[test]
    fn selected_score_missing_input_does_not_consume_any_state() {
        let catalog = PathCatalog::new(vec![vec![Statement::AwardSelectedScore { points: 100, next: cursor(0, 1) }]]).unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        runtime.branch.invert_next = true;
        let original_objects = objects.clone();
        let original_random = random;
        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingSelectedScore));
        assert_eq!(objects, original_objects);
        assert_eq!(random, original_random);
        assert!(runtime.branch.invert_next);
    }

    #[test]
    fn selected_equipment_statements_fault_only_when_needed_without_side_effects() {
        for statement in [
            Statement::CollectSelectedConsumables { amount: 1, already_full: cursor(0, 2), next: cursor(0, 1) },
            Statement::UpgradeSelectedWeapon { next: cursor(0, 1) },
        ] {
            let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            let original_objects = objects.clone();
            let original_random = random;
            let mut inputs = world(&mut random);
            assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
            assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::MissingSelectedEquipment));
            assert_eq!(objects, original_objects);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, original_random);
        }
    }

    #[test]
    fn selected_consumable_branch_preserves_ifnot_wait_actor_and_published_snapshot() {
        use super::super::path_equipment::SelectedEquipment;
        for amount in [0, 1, 8, 9, 128, 247, 248, 255] {
            let catalog = PathCatalog::new(vec![vec![Statement::CollectSelectedConsumables {
                amount, already_full: cursor(0, 2), next: cursor(0, 1),
            }]]).unwrap();
            for kind in 0..=u8::MAX {
                for same_type in [false, true] {
                    for inverted in [false, true] {
                        let (mut runtime, mut objects, owner, mut random) = setup();
                        runtime.branch.invert_next = inverted;
                        let actor = objects.get_mut(owner).unwrap();
                        actor.base.wait_timer = 137;
                        actor.extension.path_state.motion_phase = u16::from(kind) * 256 + u16::from(!kind);
                        actor.extension.path_state.weapon_selection = !kind;
                        let before = objects.clone();
                        let initial_random = random;
                        let initial = SelectedEquipment {
                            packed_consumables: kind,
                            consumable_type: if same_type { kind } else { kind.wrapping_add(1) },
                            weapon_level: !kind,
                        };
                        let mut equipment = initial;
                        let mut auxiliary = SelectedAuxiliaryState { mode: kind, action_flags: !kind };
                        let mut inputs = world(&mut random);
                        inputs.scene.active_weapon_level = Some(217);
                        inputs.selected_equipment = Some(&mut equipment);
                        inputs.selected_auxiliary = Some(&mut auxiliary);
                        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                            Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                        assert_eq!(inputs.selected_equipment.as_deref(), Some(&initial));
                        assert_eq!(objects, before);
                        let destination = cursor(0, if kind % 16 >= 9 && same_type { 2 } else { 1 });
                        let sum = (u16::from(kind % 16) + u16::from(amount)) % 256;
                        let count = if kind % 16 >= 9 { kind % 16 } else if sum >= 9 { 9 } else { sum as u8 };
                        let expected_equipment = SelectedEquipment {
                            packed_consumables: kind / 16 * 16 + count,
                            consumable_type: kind,
                            weapon_level: !kind,
                        };
                        assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                            Err(ProgramError::BudgetExceeded { cursor: destination, executed: 1 }));
                        assert_eq!(inputs.selected_equipment.as_deref(), Some(&expected_equipment));
                        assert_eq!(inputs.scene.active_weapon_level, Some(217));
                        assert_eq!(inputs.selected_auxiliary.as_deref(), Some(&SelectedAuxiliaryState { mode: kind, action_flags: !kind }));
                        let mut expected = before;
                        expected.get_mut(owner).unwrap().base.path = Some(destination);
                        assert_eq!(objects, expected);
                        assert_eq!(runtime.branch.invert_next, inverted);
                        assert_eq!(inputs.random, &initial_random);
                        assert_eq!(equipment, expected_equipment);
                    }
                }
            }
        }
    }

    #[test]
    fn selected_weapon_upgrade_mutates_live_equipment_not_active_snapshot() {
        use super::super::path_equipment::SelectedEquipment;
        let catalog = PathCatalog::new(vec![vec![Statement::UpgradeSelectedWeapon { next: cursor(0, 0) }]]).unwrap();
        for level in 0..=u8::MAX {
            for inverted in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                runtime.branch.invert_next = inverted;
                objects.get_mut(owner).unwrap().base.wait_timer = 213;
                let original_objects = objects.clone();
                let original_random = random;
                let original = SelectedEquipment { packed_consumables: !level, consumable_type: level, weapon_level: level };
                let mut equipment = original;
                let mut inputs = world(&mut random);
                inputs.selected_equipment = Some(&mut equipment);
                inputs.scene.active_weapon_level = Some(!level);
                assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                    Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 0 }));
                assert_eq!(inputs.selected_equipment.as_deref(), Some(&original));
                for visits in 1..=4 {
                    assert_eq!(runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded { cursor: cursor(0, 0), executed: 1 }));
                    assert_eq!(inputs.selected_equipment.as_deref(), Some(&SelectedEquipment {
                        weapon_level: if level >= 3 { level } else { (level + visits).min(3) },
                        ..original
                    }));
                    assert_eq!(inputs.scene.active_weapon_level, Some(!level));
                    assert_eq!(objects, original_objects);
                    assert_eq!(inputs.random, &original_random);
                    assert_eq!(runtime.branch.invert_next, inverted);
                }
            }
        }
    }

    #[test]
    fn selected_auxiliary_updates_preserve_every_unrelated_bit() {
        use SelectedAuxiliaryCommand::*;
        for mode in 0..=u8::MAX {
            for action_flags in 0..=u8::MAX {
                let original = SelectedAuxiliaryState { mode, action_flags };
                for (command, expected) in [
                    (
                        SetModeLowNibbleOne,
                        SelectedAuxiliaryState {
                            mode: mode / 16 * 16 + 1,
                            action_flags,
                        },
                    ),
                    (
                        SetModeLowNibbleFour,
                        SelectedAuxiliaryState {
                            mode: mode / 16 * 16 + 4,
                            action_flags,
                        },
                    ),
                    (
                        ClearActionBit01,
                        SelectedAuxiliaryState {
                            mode,
                            action_flags: action_flags / 2 * 2,
                        },
                    ),
                ] {
                    let mut state = original;
                    command.apply(&mut state);
                    assert_eq!(state, expected);
                    command.apply(&mut state);
                    assert_eq!(state, expected, "updates are idempotent");
                }
            }
        }
    }

    #[test]
    fn selected_auxiliary_commands_share_live_state_without_actor_or_control_side_effects() {
        use super::super::path_conditions::AuxiliaryModeClass;
        use SelectedAuxiliaryCommand::*;
        let catalog = PathCatalog::new(vec![vec![
            Statement::SelectedAuxiliary {
                command: SetModeLowNibbleFour,
                next: cursor(0, 1),
            },
            Statement::SelectedAuxiliary {
                command: ClearActionBit01,
                next: cursor(0, 2),
            },
            Statement::SelectedAuxiliary {
                command: SetModeLowNibbleOne,
                next: cursor(0, 3),
            },
            Statement::SelectedAuxiliaryBranch {
                condition: SelectedAuxiliaryCondition::ModeClass(AuxiliaryModeClass::Two),
                taken: cursor(0, 5),
                next: cursor(0, 4),
            },
        ]])
        .unwrap();
        for invert in [false, true] {
            for mode in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                objects.get_mut(owner).unwrap().base.wait_timer = 53;
                runtime.branch.invert_next = invert;
                let initial_random = random;
                let initial_objects = objects.clone();
                // No selected pose is needed: the caller supplies exactly the
                // selected auxiliary record, independently of actor transforms.
                let mut auxiliary = SelectedAuxiliaryState {
                    mode,
                    action_flags: !mode,
                };
                let mut inputs = world(&mut random);
                inputs.selected_auxiliary = Some(&mut auxiliary);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 0),
                        executed: 0
                    })
                );
                assert_eq!(
                    inputs.selected_auxiliary.as_deref(),
                    Some(&SelectedAuxiliaryState {
                        mode,
                        action_flags: !mode
                    })
                );
                for (index, expected_mode, expected_flags) in [
                    (1, mode / 16 * 16 + 4, !mode),
                    (2, mode / 16 * 16 + 4, !mode / 2 * 2),
                    (3, mode / 16 * 16 + 1, !mode / 2 * 2),
                ] {
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(0, index),
                            executed: 1
                        })
                    );
                    assert_eq!(
                        inputs.selected_auxiliary.as_deref(),
                        Some(&SelectedAuxiliaryState {
                            mode: expected_mode,
                            action_flags: expected_flags
                        })
                    );
                    let mut expected_objects = initial_objects.clone();
                    expected_objects.get_mut(owner).unwrap().base.path = Some(cursor(0, index));
                    assert_eq!(objects, expected_objects);
                    assert_eq!(runtime.branch.invert_next, invert);
                    assert_eq!(inputs.random, &initial_random);
                }
                let destination = cursor(0, if mode / 16 == 2 { 5 } else { 4 });
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: destination,
                        executed: 1
                    })
                );
                let mut expected_objects = initial_objects;
                expected_objects.get_mut(owner).unwrap().base.path = Some(destination);
                assert_eq!(objects, expected_objects);
                assert_eq!(runtime.branch.invert_next, invert);
                assert_eq!(inputs.random, &initial_random);
                // Releasing the borrow publishes the changes to the caller's
                // actual shared record, not a copy retained by the dispatcher.
                assert_eq!(
                    auxiliary,
                    SelectedAuxiliaryState {
                        mode: mode / 16 * 16 + 1,
                        action_flags: !mode / 2 * 2
                    }
                );
            }
        }
        for index in 0..3 {
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            objects.get_mut(owner).unwrap().base.path = Some(cursor(0, index));
            let before = objects.clone();
            let before_random = random;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingSelectedAuxiliary)
            );
            assert_eq!(objects, before);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, before_random);
        }
    }

    #[test]
    fn radio_requests_sample_live_byte_operands_and_replace_shared_state_immediately() {
        use super::super::path_fields::{ByteField, BytePart, WordField};
        use super::super::path_radio::{MessageIndex, PathRadio, RadioLayout, RadioRequest};
        let script_low = ByteField::WordPart {
            field: WordField::ScriptValue,
            part: BytePart::Low,
        };
        for literal in [false, true] {
            for invert in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let initial_random = random;
                runtime.branch.invert_next = invert;
                objects.get_mut(owner).unwrap().base.wait_timer = 61;
                let mut request = RadioRequest {
                    panel_y: 0xABCD,
                    ..RadioRequest::default()
                };
                let mut inputs = world(&mut random);
                inputs.radio = Some(PathRadio {
                    request: &mut request,
                    layout: RadioLayout {
                        compact_panel: false,
                        tracked_screen_y: 100,
                    },
                });
                for number in 0..=u8::MAX {
                    for (compact_panel, tracked_screen_y) in [
                        (false, 0),
                        (true, 17),
                        (false, 18),
                        (true, 100),
                        (false, 145),
                        (true, 146),
                        (false, 255),
                    ] {
                        objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
                        // Literal and actor values deliberately disagree.
                        script_low.write(
                            objects.get_mut(owner).unwrap(),
                            if literal { !number } else { number },
                        );
                        let mut expected_objects = objects.clone();
                        expected_objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
                        let catalog = PathCatalog::new(vec![vec![Statement::Message {
                            number: if literal {
                                ByteOperand::Literal(number)
                            } else {
                                ByteOperand::Actor(script_low)
                            },
                            next: cursor(0, 1),
                        }]])
                        .unwrap();
                        inputs.radio.as_mut().unwrap().layout = RadioLayout {
                            compact_panel,
                            tracked_screen_y,
                        };
                        assert_eq!(
                            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                            Err(ProgramError::BudgetExceeded {
                                cursor: cursor(0, 1),
                                executed: 1
                            })
                        );
                        let top = !(18..146).contains(&tracked_screen_y);
                        assert_eq!(
                            *inputs.radio.as_ref().unwrap().request,
                            RadioRequest {
                                message: MessageIndex::from_authored_number(number),
                                pending: true,
                                panel_y: if top {
                                    35
                                } else if compact_panel {
                                    139
                                } else {
                                    151
                                },
                                top_placement: top,
                            }
                        );
                        assert_eq!(objects, expected_objects);
                        assert_eq!(runtime.branch.invert_next, invert);
                        assert_eq!(inputs.random, &initial_random);
                    }
                }
                assert_eq!(request.message.index(), 254);
                assert!(request.pending);
            }
        }
    }

    #[test]
    fn radio_missing_input_and_zero_budget_leave_request_and_actor_untouched() {
        use super::super::path_radio::{PathRadio, RadioLayout, RadioRequest};
        let (mut runtime, mut objects, owner, mut random) = setup();
        runtime.branch.invert_next = true;
        let original_random = random;
        let original_objects = objects.clone();
        let catalog = PathCatalog::new(vec![vec![Statement::Message {
            number: ByteOperand::Literal(0),
            next: cursor(0, 1),
        }]])
        .unwrap();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingRadio)
        );
        let mut request = RadioRequest {
            panel_y: 0xBEEF,
            ..RadioRequest::default()
        };
        let original_request = request;
        let mut inputs = world(&mut random);
        inputs.radio = Some(PathRadio {
            request: &mut request,
            layout: RadioLayout {
                compact_panel: true,
                tracked_screen_y: 100,
            },
        });
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 0),
                executed: 0
            })
        );
        assert_eq!(objects, original_objects);
        assert_eq!(request, original_request);
        assert_eq!(random, original_random);
        assert!(runtime.branch.invert_next);
    }

    #[test]
    fn surface_mode_import_resamples_all_byte_bits_and_search_uses_only_low_three() {
        use super::super::collision_surface::{SurfaceMode, SurfaceSearch};
        let catalog = PathCatalog::new(vec![vec![Statement::ImportSurfaceMode {
            destination: ByteField::WordPart {
                field: WordField::MotionPhase,
                part: super::super::path_fields::BytePart::Low,
            },
            next: cursor(0, 1),
        }]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0xABCD;
        objects.get_mut(owner).unwrap().base.wait_timer = 17;
        runtime.branch.invert_next = true;
        let before = objects.clone();
        let before_random = random;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingSurfaceMode)
        );
        assert_eq!(objects, before);
        for flags in 0..=u8::MAX {
            objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
            let mode = SurfaceMode { flags };
            assert_eq!(
                mode.search(),
                if flags % 8 == 0 {
                    SurfaceSearch::Full
                } else {
                    SurfaceSearch::Reduced
                }
            );
            let mut inputs = world(&mut random);
            inputs.surface_mode = Some(mode);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            let mut expected = before.clone();
            expected
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .motion_phase = 0xAB00 | u16::from(flags);
            expected.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
            assert_eq!(objects, expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, before_random);
        }
    }

    #[test]
    fn authored_linked_protection_runs_intro_live_owner_refresh_flicker_and_shutdown() {
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_protection::{
            DeflectionProtection, LinkedEffectActivity, LinkedProtection, PathProtection,
            ProtectionRules,
        };
        use super::super::path_sound::{AuthoredCue, CueListener, PathAudio};
        use super::super::{authored_paths, Angle, AudioState, SoundEvent, Vector3};
        let catalog = authored_paths::catalog();
        for initial_activity in [0, 1, 2, 255] {
            for exit in 0..3 {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let original_random = random;
                let linked = objects
                    .allocate(Object::new(
                        ObjectKind::Player,
                        ShapeId::EMPTY,
                        Behavior::PlayerFlight,
                    ))
                    .unwrap();
                let mut selected =
                    Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
                selected.base.position = Vector3 {
                    x: 400,
                    y: -700,
                    z: 900,
                };
                let copied_position = selected.base.position;
                let selected = objects.allocate(selected).unwrap();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::LINKED_PROTECTION_EFFECT);
                actor.base.attachment = Some(linked);
                actor.base.shape = ShapeId::from_catalog_index(31);
                actor.extension.relative_rotation.pitch = Angle::from_units(250);
                actor.extension.relative_rotation.yaw = Angle::from_units(88);
                actor.extension.relative_rotation.roll = Angle::from_units(253);
                actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
                let mut activity = LinkedEffectActivity {
                    recent_spawn: initial_activity,
                };
                let mut protection = DeflectionProtection::from_control(0x6A);
                let mut audio = AudioState::default();
                let first_callback = if initial_activity == 0 { 8 } else { 0 };
                let last_visit = first_callback + 7;
                let first_flicker = first_callback + if exit == 2 { 0 } else { 3 };
                for visit in 0..=last_visit {
                    if visit == 1 {
                        objects.get_mut(selected).unwrap().base.position.x = 1234;
                    }
                    if visit == first_callback + 3 {
                        protection = DeflectionProtection::from_control(1);
                    }
                    let blocked = exit == 1 && visit >= first_callback + 6;
                    if visit == first_callback + 6 && !blocked {
                        protection = DeflectionProtection::from_control(0xE0);
                    }
                    if visit == first_callback + 6 && exit == 2 {
                        protection = DeflectionProtection::from_control(0);
                    }
                    if visit == first_callback + 1 {
                        activity.recent_spawn = 9;
                    }
                    let mut inputs = world(&mut random);
                    if visit == 0 {
                        inputs.selected = Some(selected);
                        inputs.audio = Some(PathAudio {
                            events: &mut audio,
                            listeners: [CueListener::Other; 2],
                            markers: None,
                        });
                    }
                    inputs.linked_effect_activity = Some(&mut activity);
                    if !blocked {
                        inputs.surface_mode = Some(SurfaceMode { flags: 8 });
                    }
                    inputs.protection = Some(PathProtection {
                        rules: ProtectionRules {
                            special_character: blocked,
                            blocked,
                            minimum_override: false,
                            contacts_enabled: exit != 2,
                        },
                        linked: if blocked {
                            None
                        } else {
                            Some(LinkedProtection {
                                owner: linked,
                                state: &mut protection,
                            })
                        },
                    });
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 30),
                        Ok(if visit == last_visit {
                            ControlStep::Ended
                        } else {
                            ControlStep::Movement
                        })
                    );
                    if visit >= first_callback && visit < last_visit {
                        assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                        assert!(matches!(
                            runtime.step_callbacks(
                                &mut objects,
                                owner,
                                TriggerWorldInputs::default()
                            ),
                            Ok(CallbackStep::Run(_))
                        ));
                        assert_eq!(
                            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 4),
                            Ok(ControlStep::ResumeCallbacks)
                        );
                        assert_eq!(
                            runtime.step_callbacks(
                                &mut objects,
                                owner,
                                TriggerWorldInputs::default()
                            ),
                            Ok(CallbackStep::Complete)
                        );
                    } else if visit < first_callback {
                        assert!(!runtime.begin_callbacks(&objects, owner).unwrap());
                    }
                    let actor = objects.get(owner).unwrap();
                    let callbacks = if visit < first_callback {
                        0
                    } else {
                        (visit - first_callback + 1).min(7)
                    };
                    assert_eq!(
                        actor.extension.relative_rotation.pitch.units(),
                        250u8.wrapping_add(callbacks as u8 * 8)
                    );
                    assert_eq!(
                        actor.extension.relative_rotation.roll.units(),
                        253u8.wrapping_add(callbacks as u8 * 6)
                    );
                    assert_eq!(actor.extension.relative_rotation.yaw.units(), 88);
                    let flicker_count = if visit < first_flicker {
                        0
                    } else {
                        (visit.min(last_visit - 1) - first_flicker + 1) as u8
                    };
                    let phase = if visit < first_callback {
                        1
                    } else if visit >= first_callback + 6 {
                        0
                    } else if exit == 2 || visit >= first_flicker {
                        1
                    } else {
                        10
                    };
                    assert_eq!(
                        actor.extension.path_state.motion_phase,
                        (u16::from(flicker_count) << 8) | phase
                    );
                    let animation = if visit < first_callback {
                        128 + visit as u8 + 1
                    } else if visit >= first_flicker && visit < last_visit {
                        [136, 135, 134, 133, 134, 135, 134][usize::from(flicker_count - 1)]
                    } else {
                        137
                    };
                    assert_eq!(
                        actor.extension.path_state.animation.shape.packed(),
                        animation
                    );
                    assert_eq!(actor.base.position, copied_position);
                    assert_eq!(actor.base.velocity, Vector3::default());
                    assert_eq!(actor.base.attachment, Some(linked));
                    assert_eq!(actor.base.shape, ShapeId::from_catalog_index(31));
                    assert!(actor.base.flags.collision_disabled);
                    assert!(!actor.base.flags.casts_shadow);
                    assert_eq!(actor.extension.texture_scroll_x, initial_activity);
                    assert_eq!(
                        activity.recent_spawn,
                        if visit < first_callback {
                            initial_activity
                        } else {
                            0
                        }
                    );
                    if visit == last_visit {
                        assert_eq!(
                            protection.control(),
                            if exit == 0 {
                                0xC0
                            } else if exit == 1 {
                                1
                            } else {
                                0
                            }
                        );
                    }
                }
                assert_eq!(random, original_random);
                assert_eq!(
                    audio
                        .take_events()
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>(),
                    if initial_activity == 0 {
                        vec![SoundEvent::Authored(AuthoredCue::new(
                            20,
                            0,
                            PlayerTarget::Secondary,
                        ))]
                    } else {
                        vec![]
                    }
                );
                assert_eq!(objects.len(), 3);
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }

    #[test]
    fn authored_protection_flicker_indexes_the_full_phase_byte_and_wraps_it() {
        use super::super::authored_paths;
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_protection::{
            DeflectionProtection, LinkedEffectActivity, LinkedProtection, PathProtection,
            ProtectionRules,
        };
        let catalog = authored_paths::catalog();
        for (index, animation) in [
            (0_u8, 136),
            (1, 135),
            (15, 129),
            (16, 15),
            (254, 1),
            (255, 0),
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let player = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            objects.get_mut(owner).unwrap().base.path =
                Some(authored_paths::LINKED_PROTECTION_EFFECT);
            objects.get_mut(owner).unwrap().base.attachment = Some(player);
            let mut activity = LinkedEffectActivity { recent_spawn: 1 };
            let mut protection = DeflectionProtection::from_control(1);
            let mut inputs = world(&mut random);
            inputs.selected = Some(player);
            inputs.linked_effect_activity = Some(&mut activity);
            inputs.surface_mode = Some(SurfaceMode { flags: 1 });
            inputs.protection = Some(PathProtection {
                rules: ProtectionRules {
                    contacts_enabled: true,
                    ..Default::default()
                },
                linked: Some(LinkedProtection {
                    owner: player,
                    state: &mut protection,
                }),
            });
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 30),
                Ok(ControlStep::Movement)
            );
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .motion_phase = (u16::from(index) << 8) | 1;
            assert!(runtime.begin_callbacks(&objects, owner).unwrap());
            assert!(matches!(
                runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                Ok(CallbackStep::Run(_))
            ));
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 4),
                Ok(ControlStep::ResumeCallbacks)
            );
            assert_eq!(
                runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                Ok(CallbackStep::Complete)
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                actor.extension.path_state.motion_phase,
                (u16::from(index.wrapping_add(1)) << 8) | 1
            );
            assert_eq!(
                actor.extension.path_state.animation.shape.packed(),
                animation
            );
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }

    #[test]
    fn authored_offset_projectile_distance_bands_preserve_random_order_and_local_angles() {
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_sound::{CueListener, CueMarker, MarkerInputs, PathAudio};
        use super::super::{authored_paths, Angle, AudioState, Difficulty, SpatialLoop};
        let catalog = authored_paths::catalog();
        for (distance, speed) in [
            (4999, 35),
            (5000, 38),
            (7499, 38),
            (7500, 41),
            (9999, 41),
            (10000, 45),
            (12999, 45),
            (13000, 0),
            (13001, 0),
        ] {
            for seed in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, _) = setup();
                let mut random = RandomState::new([seed, 71, 131, 211]);
                let mut expected_random = random;
                let pitch_index = expected_random.next_byte() & 7;
                let local_yaw = (expected_random.next_byte() & 7) + 4;
                let (part, phase_high) = if speed == 0 {
                    (0, pitch_index)
                } else {
                    (expected_random.next_byte() & 3, expected_random.next_byte())
                };
                let mut target =
                    Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
                target.base.position.z = distance;
                let player = objects.allocate(target).unwrap();
                objects.get_mut(owner).unwrap().base.path =
                    Some(authored_paths::OFFSET_GUIDED_PROJECTILE);
                let mut audio = AudioState::default();
                let mut inputs = world(&mut random);
                inputs.selected = Some(player);
                inputs.campaign = Some(CampaignPathInputs {
                    difficulty: Difficulty::Expert,
                    encounter_variant: 0,
                });
                inputs.surface_mode = Some(SurfaceMode { flags: 0 });
                inputs.audio = Some(PathAudio {
                    events: &mut audio,
                    listeners: [CueListener::PrimaryPlayer; 2],
                    markers: Some(MarkerInputs {
                        selected_sides: [PlayerTarget::Primary; 2],
                        markers: [CueMarker {
                            identity: CueListener::Other,
                            position: Default::default(),
                            bearing: Angle::ZERO,
                        }; 2],
                    }),
                });
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 80),
                    Ok(if speed == 0 {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    })
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(
                    (
                        actor.base.hit_points,
                        actor.base.attack_power,
                        actor.base.speed
                    ),
                    (1, 6, speed)
                );
                assert_eq!(
                    actor.extension.relative_rotation.pitch.units(),
                    [1, 1, 1, 1, 2, 2, 3, 3][usize::from(pitch_index)]
                );
                assert_eq!(actor.extension.relative_rotation.yaw.units(), local_yaw);
                assert_eq!(actor.extension.path_state.part, part);
                assert_eq!(
                    actor.extension.path_state.motion_phase,
                    u16::from(phase_high) << 8
                );
                assert_eq!(
                    actor.extension.spatial_loop,
                    SpatialLoop::from_authored_control(12)
                );
                assert_eq!(actor.base.shape, ShapeId::from_catalog_index(31));
                assert_eq!(actor.extension.texture_scroll_x, 5);
                assert!(!actor.base.flags.casts_shadow);
                assert_eq!(actor.base.velocity, Default::default());
                assert_eq!(
                    actor
                        .extension
                        .path_state
                        .triggers
                        .entries(&runtime.resources, owner)
                        .unwrap()
                        .len(),
                    if speed == 0 { 1 } else { 2 }
                );
                assert_eq!(random, expected_random);
                assert_eq!(objects.len(), 2);
                assert_eq!(runtime.spawns.last_spawn, None);
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }

    #[test]
    fn authored_offset_projectile_chase_hold_and_nonzero_waits_keep_source_callback_order() {
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_sound::{
            AuthoredCue, CueListener, CueMarker, MarkerInputs, PathAudio,
        };
        use super::super::world_occupancy::{
            MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle,
        };
        use super::super::{authored_paths, Angle, AudioState, Difficulty, SoundEvent, Vector3};
        let catalog = authored_paths::catalog();
        let mut occupancy = WorldOccupancy::default();
        occupancy.apply(
            &MarkerCoverage::from_rectangle(WorldRectangle {
                x: 0,
                z: 0,
                width: 1,
                depth: 1,
            })
            .unwrap(),
            OccupancyChange::Mark,
        );
        for mode in [0, 1, 8] {
            for auxiliary_mode in [0, 31] {
                for (difficulty, attack) in [
                    (Difficulty::Normal, 2),
                    (Difficulty::Hard, 4),
                    (Difficulty::Expert, 6),
                ] {
                    // Expiry, new contact, counter limit, occupancy during initial
                    // collision-disabled wait, ground, or a newly encountered surface.
                    for exit in 0..7 {
                        if mode == 0 && (4..6).contains(&exit) {
                            continue;
                        }
                        let last_visit = match exit {
                            0 => {
                                if mode == 0 {
                                    110
                                } else {
                                    53
                                }
                            }
                            1 => 9,
                            2 => 3,
                            3 => 2,
                            6 => {
                                if mode == 0 {
                                    111
                                } else {
                                    53
                                }
                            }
                            _ => 5,
                        };
                        let (mut runtime, mut objects, owner, mut random) = setup();
                        let mut support = Object::new(
                            ObjectKind::Scenery,
                            ShapeId::from_catalog_index(156),
                            Behavior::FollowPath,
                        );
                        support.base.position.x = 1000;
                        support.base.contacts.first_strategy_visit = false;
                        let support = objects.allocate(support).unwrap();
                        let mut expected_random = random;
                        expected_random.next_byte();
                        if mode == 0 {
                            for _ in 0..3 {
                                expected_random.next_byte();
                            }
                        }
                        let mut target =
                            Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
                        target.base.position = Vector3 {
                            x: 0,
                            y: -500,
                            z: 8000,
                        };
                        let player = objects.allocate(target).unwrap();
                        let actor = objects.get_mut(owner).unwrap();
                        actor.base.path = Some(authored_paths::OFFSET_GUIDED_PROJECTILE);
                        actor.base.position.y = -500;
                        let initial_counter: u16 = match exit {
                            2 => 107,
                            6 => u16::MAX,
                            _ => 0,
                        };
                        actor.extension.path_state.script_value = initial_counter;
                        let mut auxiliary = SelectedAuxiliaryState {
                            mode: auxiliary_mode,
                            action_flags: 0,
                        };
                        let mut audio = AudioState::default();
                        let mut timed_runs = 0;
                        let mut expired = 0;
                        for visit in 0..=last_visit {
                            if visit == 3 {
                                objects.get_mut(player).unwrap().base.position.z = 2500;
                            }
                            if visit == 6 {
                                objects.get_mut(player).unwrap().base.position.z = 1000;
                            }
                            if exit == 4 && visit == last_visit - 1 {
                                objects.get_mut(owner).unwrap().base.position.y = 0;
                            }
                            if exit == 5 && visit == last_visit - 1 {
                                objects.get_mut(support).unwrap().base.position.x = 0;
                                objects.get_mut(owner).unwrap().base.position.y = -403;
                            }
                            // The first chase crosses its threshold on visit 3
                            // and immediately executes the second offset aim.
                            let mut expected_pose = objects.clone();
                            if mode == 0 && visit < last_visit && visit <= 6 {
                                for _ in 0..if visit == 3 { 2 } else { 1 } {
                                    super::super::path_steering::face_selected_offset(
                                        &mut expected_pose,
                                        owner,
                                        Some(player),
                                        super::super::path_steering::AimOffset {
                                            x: 0,
                                            y: 0,
                                            z: 64,
                                        },
                                        &mut Default::default(),
                                    )
                                    .unwrap();
                                }
                            }
                            let occupied = exit == 3 && visit == last_visit - 1;
                            let mut inputs = world(&mut random);
                            // The nonzero branch never needs a selected object.
                            if mode == 0 {
                                inputs.selected = Some(player);
                            }
                            inputs.selected_occupancy_exempt = Some(!occupied);
                            if occupied {
                                inputs.occupancy = Some(&occupancy);
                            }
                            if visit == 0 || (mode != 0 && !occupied) {
                                inputs.surface_mode = Some(SurfaceMode {
                                    flags: if exit == 5 && visit >= last_visit - 1 {
                                        0
                                    } else {
                                        mode
                                    },
                                });
                            }
                            if visit == 0 {
                                inputs.campaign = Some(CampaignPathInputs {
                                    difficulty,
                                    encounter_variant: 0,
                                });
                                inputs.selected_auxiliary = Some(&mut auxiliary);
                                inputs.audio = Some(PathAudio {
                                    events: &mut audio,
                                    listeners: [CueListener::PrimaryPlayer; 2],
                                    markers: Some(MarkerInputs {
                                        selected_sides: [PlayerTarget::Primary; 2],
                                        markers: [CueMarker {
                                            identity: CueListener::Other,
                                            position: Vector3 {
                                                x: 0,
                                                y: -500,
                                                z: 0,
                                            },
                                            bearing: Angle::ZERO,
                                        }; 2],
                                    }),
                                });
                            }
                            let result = runtime
                                .enter_program(&catalog, &mut objects, owner, &mut inputs, 80)
                                .unwrap();
                            if mode == 0 {
                                let actual = &objects.get(owner).unwrap().base;
                                let expected = &expected_pose.get(owner).unwrap().base;
                                assert_eq!(
                                    (actual.pitch, actual.yaw),
                                    (expected.pitch, expected.yaw)
                                );
                            }
                            if visit == last_visit {
                                assert_eq!(
                                    result,
                                    ControlStep::Ended,
                                    "mode={mode} exit={exit} visit={visit}"
                                );
                            } else {
                                assert_eq!(result, ControlStep::Movement);
                                assert_eq!(
                                    objects
                                        .get(owner)
                                        .unwrap()
                                        .extension
                                        .path_state
                                        .hold_latched,
                                    mode == 0 && visit >= 6
                                );
                                objects
                                    .get_mut(owner)
                                    .unwrap()
                                    .base
                                    .contacts
                                    .new_contact_latched = exit == 1 && visit == last_visit - 1;
                                assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                                let mut runs = 0;
                                loop {
                                    match runtime
                                        .step_callbacks(
                                            &mut objects,
                                            owner,
                                            TriggerWorldInputs {
                                                strategy_tick: visit as u8,
                                                ..Default::default()
                                            },
                                        )
                                        .unwrap()
                                    {
                                        CallbackStep::Complete => break,
                                        CallbackStep::Run(_) => {
                                            runs += 1;
                                            assert_eq!(
                                                runtime.resume_program(
                                                    &catalog,
                                                    &mut objects,
                                                    owner,
                                                    &mut inputs,
                                                    16
                                                ),
                                                Ok(ControlStep::ResumeCallbacks)
                                            );
                                        }
                                        CallbackStep::Expired => expired += 1,
                                        CallbackStep::Skipped => {}
                                    }
                                }
                                let timed = mode == 0 && (6..19).contains(&visit);
                                timed_runs += usize::from(timed);
                                assert_eq!(
                                    runs,
                                    1 + usize::from(timed)
                                        + usize::from(exit == 1 && visit == last_visit - 1)
                                );
                            }
                            let actor = objects.get(owner).unwrap();
                            assert_eq!(
                                (actor.base.hit_points, actor.base.attack_power),
                                (1, attack)
                            );
                            assert_eq!(actor.base.speed, if mode == 0 { 41 } else { 30 });
                            assert_eq!(actor.base.velocity, Vector3::default());
                            let movement_visit = visit.min(last_visit - 1);
                            assert_eq!(
                                actor.base.flags.collision_disabled,
                                mode != 0 && movement_visit < 3
                            );
                            if mode != 0 {
                                assert_eq!(
                                    actor.base.pitch.units(),
                                    if auxiliary_mode == 31 { 0 } else { 253 }
                                );
                            }
                            if mode == 0 && movement_visit >= 3 {
                                assert_eq!(
                                    (actor.base.target_speed, actor.base.acceleration),
                                    (if visit >= 6 { 25 } else { 38 }, 2)
                                );
                            }
                            let increments = (visit + 1).min(last_visit)
                                - usize::from(exit == 3 && visit >= last_visit - 1);
                            assert_eq!(
                                actor.extension.path_state.script_value,
                                initial_counter.wrapping_add(increments as u16),
                                "counter mode={mode} exit={exit} visit={visit}"
                            );
                            if visit == last_visit {
                                let remaining = actor
                                    .extension
                                    .path_state
                                    .triggers
                                    .entries(&runtime.resources, owner)
                                    .unwrap()
                                    .len();
                                // Cleanup explicitly cancels the zero-mode callback,
                                // even when the nonzero-mode callback forced it.
                                assert_eq!(
                                    remaining,
                                    if (exit == 0 || exit == 6) && mode == 0 {
                                        0
                                    } else if exit == 1 {
                                        if mode == 0 {
                                            3
                                        } else {
                                            2
                                        }
                                    } else if exit == 0 || exit == 6 {
                                        2
                                    } else if mode == 0 {
                                        0
                                    } else {
                                        1
                                    }
                                );
                            }
                        }
                        if mode == 0 && (exit == 0 || exit == 6) {
                            assert_eq!((timed_runs, expired), (13, 1));
                        }
                        assert_eq!(random, expected_random);
                        assert_eq!(
                            audio
                                .take_events()
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>(),
                            [SoundEvent::Authored(AuthoredCue::new(
                                115,
                                32, // Coincident X/Z uses the source's right-bearing sector.
                                PlayerTarget::Secondary
                            ))]
                        );
                        assert_eq!(objects.len(), 3);
                        runtime.release_actor_programs(&mut objects, owner).unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn authored_variant_projectile_rejects_far_targets_before_audio_or_spawn() {
        use super::super::authored_paths;
        let catalog = authored_paths::catalog();
        for distance in [9999, 10000, 10001] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let mut player =
                Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
            player.base.position.z = distance;
            let selected = objects.allocate(player).unwrap();
            objects.get_mut(owner).unwrap().base.path =
                Some(authored_paths::VARIANT_GUIDED_PROJECTILE);
            let mut inputs = world(&mut random);
            inputs.selected = Some(selected);
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 20),
                if distance < 10000 {
                    Err(ProgramError::MissingAudio)
                } else {
                    Ok(ControlStep::Ended)
                }
            );
            assert_eq!(objects.len(), 2);
            assert_eq!(runtime.spawns.last_spawn, None);
            assert_eq!(
                (
                    objects.get(owner).unwrap().base.hit_points,
                    objects.get(owner).unwrap().base.attack_power
                ),
                (1, 6)
            );
            assert_eq!(objects.get(owner).unwrap().base.shape, ShapeId::EMPTY);
        }
    }

    #[test]
    fn authored_variant_projectile_yaw_gate_controls_collision_and_forward_exit() {
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_sound::{CueListener, PathAudio};
        use super::super::{
            authored_paths, path_conditions, path_control, Angle, AudioState, ObjectSpawnDefaults,
            Rotation, Vector3,
        };
        let catalog = authored_paths::catalog();
        let mut covered = [false; 3];
        for yaw in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let mut player =
                Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
            player.base.position = Vector3 {
                x: 0,
                y: -1000,
                z: 2000,
            };
            player.base.yaw = Angle::from_units(yaw);
            let target = player.base.position;
            let selected = objects.allocate(player).unwrap();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::VARIANT_GUIDED_PROJECTILE);
            actor.base.position.y = -1000;
            actor.base.flags.collision_disabled = true;
            let position = actor.base.position;
            let in_arc = path_conditions::target_relative_yaw_between(
                position,
                target,
                Angle::from_units(yaw),
                206,
                50,
            );
            let forward_exit = !in_arc
                && path_control::forward_plane_projection(
                    target,
                    Rotation {
                        yaw: Angle::from_units(yaw),
                        ..Rotation::default()
                    },
                    position,
                ) < 0;
            covered[if in_arc {
                0
            } else if forward_exit {
                1
            } else {
                2
            }] = true;
            let mut audio = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.selected = Some(selected);
            inputs.surface_mode = Some(SurfaceMode { flags: 8 });
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.audio = Some(PathAudio {
                events: &mut audio,
                listeners: [CueListener::PrimaryPlayer; 2],
                markers: None,
            });
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 48),
                Ok(ControlStep::Movement)
            );
            if forward_exit {
                inputs.surface_mode = None;
            }
            assert!(runtime.begin_callbacks(&objects, owner).unwrap());
            loop {
                match runtime
                    .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                    .unwrap()
                {
                    CallbackStep::Complete => break,
                    CallbackStep::Run(_) => assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 24),
                        Ok(ControlStep::ResumeCallbacks)
                    ),
                    CallbackStep::Skipped | CallbackStep::Expired => {}
                }
            }
            assert_eq!(
                objects.get(owner).unwrap().base.flags.collision_disabled,
                !in_arc && !forward_exit
            );
            assert!(!objects.get(owner).unwrap().base.contacts.hit_marked);
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16),
                if forward_exit {
                    Ok(ControlStep::Ended)
                } else {
                    Ok(ControlStep::Movement)
                }
            );
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
        assert_eq!(covered, [true; 3]);
    }

    #[test]
    fn authored_variant_projectile_runs_rise_trail_callbacks_and_terminal_branches() {
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_sound::{AuthoredCue, CueListener, PathAudio};
        use super::super::{
            authored_paths, Angle, AudioState, ObjectSpawnDefaults, SoundEvent, SpatialLoop,
            Vector3,
        };
        let catalog = authored_paths::catalog();
        for variant in [0, 1, 2, 255] {
            for mode in [0, 1, 8] {
                // Expiry, near-target wait/hold, new contact, surface, ground.
                for exit in 0..5 {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let initial_random = random;
                    let mut player =
                        Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
                    player.base.position = Vector3 {
                        x: 0,
                        y: -1000,
                        z: 2000,
                    };
                    // Keep the collision-enabled side of the authored yaw arc.
                    let bearing = (sf_core::aim_angle::sf2_atan16(0, -2000) >> 8) as u8;
                    player.base.yaw = Angle::from_units(bearing.wrapping_neg());
                    let selected = objects.allocate(player).unwrap();
                    let mut surface = Object::new(
                        ObjectKind::Scenery,
                        ShapeId::from_catalog_index(156),
                        Behavior::FollowPath,
                    );
                    surface.base.position.x = 1000;
                    surface.base.contacts.first_strategy_visit = false;
                    let support = objects.allocate(surface).unwrap();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(authored_paths::VARIANT_GUIDED_PROJECTILE);
                    actor.base.position.y = -1000;
                    actor.extension.path_state.script_parameter = variant;
                    actor.extension.path_state.motion_phase = 0xABCD;
                    actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
                    let setup_visit = usize::from(variant == 1) * 9;
                    let trigger_visit = setup_visit + 6;
                    let last_visit = if exit == 0 || (exit == 4 && mode == 0) {
                        setup_visit + 119
                    } else if exit == 1 {
                        trigger_visit + 31
                    } else {
                        trigger_visit + 1
                    };
                    let mut audio = AudioState::default();
                    let mut child = None;
                    let mut child_steps = 0;
                    for visit in 0..=last_visit {
                        if visit == trigger_visit {
                            if exit == 1 {
                                objects.get_mut(selected).unwrap().base.position.z =
                                    if variant == 2 { 50 } else { 250 };
                            }
                            if exit == 2 {
                                objects
                                    .get_mut(owner)
                                    .unwrap()
                                    .base
                                    .contacts
                                    .new_contact_latched = true;
                            }
                            if exit == 3 {
                                objects.get_mut(support).unwrap().base.position.x = 0;
                                objects.get_mut(owner).unwrap().base.position.y = -403;
                            }
                            if exit == 4 {
                                objects.get_mut(owner).unwrap().base.position.y = 0;
                            }
                        }
                        let mut inputs = world(&mut random);
                        inputs.selected = Some(selected);
                        // Near-target callback returns before surface lookup;
                        // after cancellation the wait needs no surface service.
                        if !(exit == 1 && visit >= trigger_visit) {
                            inputs.surface_mode = Some(SurfaceMode { flags: mode });
                        }
                        if visit == 0 {
                            inputs.audio = Some(PathAudio {
                                events: &mut audio,
                                listeners: [CueListener::PrimaryPlayer; 2],
                                markers: None,
                            });
                        }
                        if visit == setup_visit {
                            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                        }
                        let outcome = runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 64)
                            .unwrap();
                        assert_eq!(
                            outcome,
                            if visit == last_visit && exit != 1 {
                                ControlStep::Ended
                            } else {
                                ControlStep::Movement
                            },
                            "variant={variant} mode={mode} exit={exit} visit={visit}"
                        );
                        if visit == setup_visit {
                            child = runtime.spawns.last_spawn;
                            let effect = objects.get(child.unwrap()).unwrap();
                            assert_eq!(effect.base.kind, ObjectKind::Effect);
                            assert_eq!(effect.base.attachment, Some(owner));
                            assert_eq!(effect.extension.parent, Some(owner));
                            assert!(effect.base.flags.remove_with_parent);
                            assert!(!effect.base.flags.reclaim_on_pool_pressure);
                            assert_eq!(
                                effect.extension.relative_position,
                                Vector3 {
                                    x: 0,
                                    y: 0,
                                    z: match variant {
                                        1 => -60,
                                        2 => -480,
                                        _ => -120,
                                    }
                                }
                            );
                            assert_eq!(effect.base.position, Vector3::default()); // Child publication is a separate service.
                            assert_eq!(
                                (effect.base.hit_points, effect.base.attack_power),
                                (if variant == 2 { 80 } else { 1 }, 1)
                            );
                        }
                        if outcome == ControlStep::Movement {
                            assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                            loop {
                                match runtime
                                    .step_callbacks(
                                        &mut objects,
                                        owner,
                                        TriggerWorldInputs::default(),
                                    )
                                    .unwrap()
                                {
                                    CallbackStep::Complete => break,
                                    CallbackStep::Run(_) => assert_eq!(
                                        runtime.resume_program(
                                            &catalog,
                                            &mut objects,
                                            owner,
                                            &mut inputs,
                                            24
                                        ),
                                        Ok(ControlStep::ResumeCallbacks)
                                    ),
                                    CallbackStep::Skipped | CallbackStep::Expired => {}
                                }
                            }
                        }
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(
                            (actor.base.hit_points, actor.base.attack_power),
                            (
                                if exit == 1 && visit == last_visit {
                                    0
                                } else {
                                    1
                                },
                                6
                            )
                        );
                        assert_eq!(
                            actor.base.shape,
                            ShapeId::from_catalog_index(match variant {
                                1 => 110,
                                2 => 109,
                                _ => 111,
                            })
                        );
                        assert_eq!(
                            actor.extension.spatial_loop,
                            SpatialLoop::from_authored_control(2)
                        );
                        assert_eq!(
                            actor.extension.path_state.motion_phase,
                            0xAB00 | u16::from(mode)
                        );
                        assert_eq!(
                            actor.base.speed,
                            if visit < setup_visit || variant != 2 {
                                2
                            } else {
                                45
                            }
                        );
                        assert_eq!(
                            (actor.base.target_speed, actor.base.acceleration),
                            if visit >= setup_visit && variant != 2 {
                                (20, 2)
                            } else {
                                (0, 0)
                            }
                        );
                        assert_eq!(
                            actor.base.contacts.hit_marked,
                            visit >= trigger_visit && (exit == 3 || (exit == 4 && mode != 0))
                        );
                        if visit < setup_visit {
                            assert_eq!(actor.base.position.y, -1000 + (visit as i16 + 1) * 10);
                        }
                        if exit == 1 && visit == last_visit {
                            assert!(actor.extension.path_state.hold_latched);
                            assert_eq!(actor.base.behavior, Behavior::PathMovement);
                            assert!(!actor.base.flags.remove_after_tick);
                        }
                        // The independently scheduled attached effect loops until
                        // its OWN health predicate redirects it to END.
                        if let Some(effect_id) = child {
                            if child_steps < 5 {
                                if child_steps == 3 {
                                    objects.get_mut(effect_id).unwrap().base.hit_points = 0;
                                }
                                assert_eq!(
                                    runtime
                                        .enter_program(
                                            &catalog,
                                            &mut objects,
                                            effect_id,
                                            &mut world(&mut random),
                                            16
                                        )
                                        .unwrap(),
                                    if child_steps == 4 {
                                        ControlStep::Ended
                                    } else {
                                        ControlStep::Movement
                                    }
                                );
                                if child_steps < 4 {
                                    assert!(runtime.begin_callbacks(&objects, effect_id).unwrap());
                                    loop {
                                        match runtime
                                            .step_callbacks(
                                                &mut objects,
                                                effect_id,
                                                TriggerWorldInputs::default(),
                                            )
                                            .unwrap()
                                        {
                                            CallbackStep::Complete => break,
                                            CallbackStep::Run(_) => assert_eq!(
                                                runtime.resume_program(
                                                    &catalog,
                                                    &mut objects,
                                                    effect_id,
                                                    &mut world(&mut random),
                                                    3
                                                ),
                                                Ok(ControlStep::ResumeCallbacks)
                                            ),
                                            CallbackStep::Skipped | CallbackStep::Expired => {}
                                        }
                                    }
                                }
                                let effect = objects.get(effect_id).unwrap();
                                assert!(effect.base.flags.collision_disabled);
                                assert!(effect.base.flags.suppress_death_effects);
                                assert_eq!(
                                    effect.extension.texture_scroll_x,
                                    if variant == 2 { 96 } else { 17 }
                                );
                                assert_eq!(
                                    effect.base.hit_points,
                                    if child_steps < 3 { 100 } else { 0 }
                                );
                                assert_eq!(
                                    effect.extension.path_state.animation.color.fixed_frame(),
                                    Some(if child_steps < 4 {
                                        (1 - child_steps % 2) as u8
                                    } else {
                                        0
                                    })
                                );
                                child_steps += 1;
                            }
                        }
                    }
                    assert_eq!(
                        audio
                            .take_events()
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>(),
                        [SoundEvent::Authored(AuthoredCue::new(
                            116,
                            0,
                            PlayerTarget::Primary
                        ))]
                    );
                    assert_eq!(random, initial_random);
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                    runtime
                        .release_actor_programs(&mut objects, child.unwrap())
                        .unwrap();
                }
            }
        }
    }

    #[test]
    fn authored_homing_entries_preserve_motion_prefix_and_difficulty_distance_gate() {
        use super::super::{authored_paths, path_motion, Angle, Difficulty, Vector3};
        let catalog = authored_paths::catalog();
        for (root, speed, multiplier) in [
            (authored_paths::DOUBLED_MOTION_HOMING_PROJECTILE, 75, 2),
            (authored_paths::PRIMARY_MOTION_HOMING_PROJECTILE, 90, 1),
            (authored_paths::DIFFICULTY_HOMING_PROJECTILE, 0, 0),
        ] {
            for distance in [11999, 12000, 12001] {
                for auxiliary_mode in [0, 31] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let original_random = random;
                    let mut target =
                        Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
                    target.base.position.z = distance;
                    target.base.velocity = Vector3 {
                        x: 32760,
                        y: -200,
                        z: -32760,
                    };
                    let player = objects.allocate(target).unwrap();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(root);
                    actor.base.pitch = Angle::from_units(29);
                    actor.base.yaw = Angle::from_units(85);
                    let generated =
                        path_motion::direction_velocity(actor.base.pitch, actor.base.yaw, speed, 1);
                    let mut inputs = world(&mut random);
                    inputs.selected = Some(player);
                    inputs.primary_player = Some(player);
                    let displacement = Vector3 {
                        x: -32760,
                        y: 300,
                        z: 32760,
                    };
                    inputs.primary_motion = Some(PrimaryMotionInput {
                        auxiliary_mode,
                        displacement,
                    });
                    inputs.campaign = Some(CampaignPathInputs {
                        difficulty: Difficulty::Expert,
                        encounter_variant: 0,
                    });
                    let result =
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 40);
                    assert_eq!(
                        result,
                        if multiplier == 0 && distance >= 12000 {
                            Ok(ControlStep::Ended)
                        } else {
                            Err(ProgramError::MissingAudio)
                        }
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(
                        (
                            actor.base.hit_points,
                            actor.base.attack_power,
                            actor.base.target_speed
                        ),
                        if multiplier == 0 {
                            (10, 4, 45)
                        } else {
                            (10, 2, 33)
                        }
                    );
                    if multiplier != 0 {
                        let inherited = if auxiliary_mode == 31 {
                            objects.get(player).unwrap().base.velocity
                        } else {
                            displacement
                        };
                        assert_eq!(
                            actor.base.velocity,
                            Vector3 {
                                x: generated
                                    .x
                                    .wrapping_mul(multiplier)
                                    .wrapping_add(inherited.x),
                                y: generated.y.wrapping_mul(multiplier),
                                z: generated
                                    .z
                                    .wrapping_mul(multiplier)
                                    .wrapping_add(inherited.z),
                            }
                        );
                    }
                    assert_eq!(actor.base.position, Vector3::default());
                    assert_eq!(objects.len(), 2);
                    assert_eq!(runtime.spawns.last_spawn, None);
                    assert_eq!(random, original_random);
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn authored_homing_chase_contracts_three_times_per_yield_then_enters_forty_loop() {
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_sound::{CueListener, CueMarker, MarkerInputs, PathAudio};
        use super::super::{
            authored_paths, path_math, Angle, AudioState, Difficulty, ObjectSpawnDefaults, Vector3,
        };
        let catalog = authored_paths::catalog();
        for initial_distance in [999, 1000, 3000] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let mut target =
                Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
            target.base.position = Vector3 {
                x: 0,
                y: -500,
                z: initial_distance,
            };
            let player = objects.allocate(target).unwrap();
            objects.get_mut(owner).unwrap().base.path =
                Some(authored_paths::DIFFICULTY_HOMING_PROJECTILE);
            objects.get_mut(owner).unwrap().base.position.y = -500;
            let initial_position = objects.get(owner).unwrap().base.position;
            let mut expected_position = initial_position;
            let chase_visits = if initial_distance < 1000 {
                0
            } else if initial_distance == 1000 {
                1
            } else {
                3
            };
            let mut audio = AudioState::default();
            for visit in 0..chase_visits + 40 {
                if visit == chase_visits && chase_visits != 0 {
                    objects.get_mut(player).unwrap().base.position = Vector3 {
                        z: expected_position.z - 100,
                        ..expected_position
                    };
                }
                if visit < chase_visits {
                    for _ in 0..3 {
                        expected_position = path_math::change_radius(
                            expected_position,
                            objects.get(player).unwrap().base.position,
                            127,
                        );
                    }
                }
                let mut inputs = world(&mut random);
                inputs.selected = Some(player);
                inputs.selected_occupancy_exempt = Some(true);
                if visit == 0 {
                    inputs.campaign = Some(CampaignPathInputs {
                        difficulty: Difficulty::Normal,
                        encounter_variant: 0,
                    });
                    inputs.surface_mode = Some(SurfaceMode { flags: 0 });
                    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                    inputs.audio = Some(PathAudio {
                        events: &mut audio,
                        listeners: [CueListener::PrimaryPlayer; 2],
                        markers: Some(MarkerInputs {
                            selected_sides: [PlayerTarget::Primary; 2],
                            markers: [CueMarker {
                                identity: CueListener::Other,
                                position: initial_position,
                                bearing: Angle::ZERO,
                            }; 2],
                        }),
                    });
                }
                let terminal = visit == chase_visits + 39;
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 80)
                        .unwrap(),
                    if terminal {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                if !terminal {
                    assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                    loop {
                        match runtime
                            .step_callbacks(
                                &mut objects,
                                owner,
                                TriggerWorldInputs {
                                    strategy_tick: visit as u8,
                                    player_projections: [Some(1); 2],
                                    ..TriggerWorldInputs::default()
                                },
                            )
                            .unwrap()
                        {
                            CallbackStep::Complete => break,
                            CallbackStep::Run(_) => assert_eq!(
                                runtime.resume_program(
                                    &catalog,
                                    &mut objects,
                                    owner,
                                    &mut inputs,
                                    12
                                ),
                                Ok(ControlStep::ResumeCallbacks)
                            ),
                            CallbackStep::Skipped | CallbackStep::Expired => {}
                        }
                    }
                }
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.position, expected_position);
                assert_eq!(
                    actor.extension.path_state.script_value,
                    (visit + usize::from(!terminal)) as u16
                );
                assert_eq!(
                    actor
                        .extension
                        .path_state
                        .triggers
                        .entries(&runtime.resources, owner)
                        .unwrap()
                        .len(),
                    if visit < chase_visits { 2 } else { 4 }
                );
                assert_eq!(
                    objects
                        .get(runtime.spawns.last_spawn.unwrap())
                        .unwrap()
                        .base
                        .position,
                    initial_position
                );
            }
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }

    #[test]
    fn authored_homing_roots_run_shared_lifetimes_and_cancel_crossing_callbacks() {
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_sound::{
            AuthoredCue, CueListener, CueMarker, MarkerInputs, PathAudio,
        };
        use super::super::world_occupancy::{
            MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle,
        };
        use super::super::{
            authored_paths, AudioState, Difficulty, ObjectSpawnDefaults, SoundEvent, Vector3,
        };
        let catalog = authored_paths::catalog();
        let mut occupancy = WorldOccupancy::default();
        occupancy.apply(
            &MarkerCoverage::from_rectangle(WorldRectangle {
                x: 0,
                z: 0,
                width: 1,
                depth: 1,
            })
            .unwrap(),
            OccupancyChange::Mark,
        );
        for (root, limit, inherits) in [
            (authored_paths::DOUBLED_MOTION_HOMING_PROJECTILE, 33, true),
            (authored_paths::PRIMARY_MOTION_HOMING_PROJECTILE, 33, true),
            (authored_paths::DIFFICULTY_HOMING_PROJECTILE, 45, false),
        ] {
            for mode in [0, 1, 8] {
                // Normal expiry, new contact, counter threshold, occupancy,
                // crossing + fifteen waits, or nonzero-mode ground contact.
                let exits: &[(u8, usize)] = if mode == 0 {
                    &[(0, 39), (1, 5), (2, 3), (3, 6), (4, 19)]
                } else {
                    &[(0, limit - 1), (1, 5), (5, 6)]
                };
                for &(exit, last_visit) in exits {
                    for (difficulty, attack) in [
                        (Difficulty::Normal, 2),
                        (Difficulty::Hard, 3),
                        (Difficulty::Expert, 4),
                    ] {
                        let (mut runtime, mut objects, owner, mut random) = setup();
                        let initial_random = random;
                        let mut target =
                            Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
                        target.base.position = Vector3 {
                            x: 0,
                            y: -500,
                            z: -100,
                        };
                        target.base.velocity = Vector3 {
                            x: 101,
                            y: 202,
                            z: -303,
                        };
                        let player = objects.allocate(target).unwrap();
                        let actor = objects.get_mut(owner).unwrap();
                        actor.base.path = Some(root);
                        actor.base.position.y = -500;
                        actor.extension.path_state.motion_phase = 0xABCD;
                        actor.extension.path_state.script_value =
                            if exit == 2 { 56 } else { u16::MAX };
                        actor.extension.path_state.conditions.selected_player =
                            PlayerTarget::Secondary;
                        let prefix_speed =
                            if root == authored_paths::DOUBLED_MOTION_HOMING_PROJECTILE {
                                75
                            } else {
                                90
                            };
                        let prefix_scale =
                            if root == authored_paths::DOUBLED_MOTION_HOMING_PROJECTILE {
                                2
                            } else {
                                1
                            };
                        let direction = super::super::path_motion::direction_velocity(
                            super::super::Angle::ZERO,
                            super::super::Angle::ZERO,
                            prefix_speed,
                            1,
                        );
                        let inherited_velocity = if inherits {
                            Vector3 {
                                x: direction.x.wrapping_mul(prefix_scale).wrapping_add(101),
                                y: direction.y.wrapping_mul(prefix_scale),
                                z: direction.z.wrapping_mul(prefix_scale).wrapping_add(-303),
                            }
                        } else {
                            Vector3::default()
                        };
                        let mut audio = AudioState::default();
                        let marker = CueMarker {
                            identity: CueListener::Other,
                            position: Vector3 {
                                x: 0,
                                y: -500,
                                z: -100,
                            },
                            bearing: super::super::Angle::ZERO,
                        };
                        let mut child = None;
                        for visit in 0..=last_visit {
                            if exit == 5 && visit == last_visit {
                                objects.get_mut(owner).unwrap().base.position.y = 0;
                            }
                            let mut inputs = world(&mut random);
                            // Later visits do not require any initialization inputs.
                            inputs.selected = Some(player);
                            if mode != 0 || visit == 0 {
                                inputs.surface_mode = Some(SurfaceMode { flags: mode });
                            }
                            if visit == 0 {
                                inputs.campaign = Some(CampaignPathInputs {
                                    difficulty,
                                    encounter_variant: 0,
                                });
                                if inherits {
                                    inputs.primary_player = Some(player);
                                    inputs.primary_motion = Some(PrimaryMotionInput {
                                        auxiliary_mode: 31,
                                        displacement: Vector3::default(),
                                    });
                                }
                                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                                inputs.audio = Some(PathAudio {
                                    events: &mut audio,
                                    listeners: [CueListener::PrimaryPlayer; 2],
                                    markers: Some(MarkerInputs {
                                        selected_sides: [PlayerTarget::Primary; 2],
                                        markers: [marker; 2],
                                    }),
                                });
                            }
                            let occupied = exit == 3 && visit == last_visit;
                            inputs.selected_occupancy_exempt = Some(!occupied);
                            if occupied {
                                inputs.occupancy = Some(&occupancy);
                            }
                            let mut outcome = runtime
                                .enter_program(&catalog, &mut objects, owner, &mut inputs, 80)
                                .unwrap();
                            if visit == 0 {
                                child = runtime.spawns.last_spawn;
                                let spawned = objects.get(child.unwrap()).unwrap();
                                assert_eq!(spawned.base.kind, ObjectKind::Effect);
                                assert_eq!(spawned.base.shape, ShapeId::from_catalog_index(19));
                                assert_eq!(
                                    spawned.base.position,
                                    objects.get(owner).unwrap().base.position
                                );
                                assert_eq!(spawned.extension.parent, None);
                                assert_eq!(spawned.base.attachment, None);
                            }
                            if outcome == ControlStep::Movement {
                                objects
                                    .get_mut(owner)
                                    .unwrap()
                                    .base
                                    .contacts
                                    .new_contact_latched = exit == 1 && visit == last_visit;
                                assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                                let mut runs = 0;
                                loop {
                                    let triggers = TriggerWorldInputs {
                                        strategy_tick: visit as u8,
                                        player_projections: [
                                            Some(1),
                                            Some(if exit == 4 && visit >= 3 { -1 } else { 1 }),
                                        ],
                                        ..TriggerWorldInputs::default()
                                    };
                                    match runtime
                                        .step_callbacks(&mut objects, owner, triggers)
                                        .unwrap()
                                    {
                                        CallbackStep::Complete => break,
                                        CallbackStep::Run(_) => {
                                            runs += 1;
                                            assert_eq!(
                                                runtime.resume_program(
                                                    &catalog,
                                                    &mut objects,
                                                    owner,
                                                    &mut inputs,
                                                    12
                                                ),
                                                Ok(ControlStep::ResumeCallbacks)
                                            );
                                        }
                                        CallbackStep::Skipped | CallbackStep::Expired => {}
                                    }
                                }
                                if mode == 0 && exit != 1 {
                                    let steering_due = visit % 2 == 0 && !(exit == 4 && visit >= 4);
                                    assert_eq!(
                                        runs,
                                        1 + usize::from(steering_due)
                                            + usize::from(exit == 4 && visit == 3)
                                    );
                                }
                                if visit == last_visit {
                                    assert_eq!(
                                        catalog
                                            .statement(
                                                objects.get(owner).unwrap().base.path.unwrap()
                                            )
                                            .unwrap(),
                                        Statement::Control(ControlCommand::End)
                                    );
                                    outcome = runtime
                                        .enter_program(
                                            &catalog,
                                            &mut objects,
                                            owner,
                                            &mut inputs,
                                            2,
                                        )
                                        .unwrap();
                                }
                            }
                            assert_eq!(
                                outcome,
                                if visit == last_visit {
                                    ControlStep::Ended
                                } else {
                                    ControlStep::Movement
                                },
                                "mode={mode} exit={exit} visit={visit}"
                            );
                            let actor = objects.get(owner).unwrap();
                            assert_eq!(
                                (actor.base.hit_points, actor.base.attack_power),
                                (10, if inherits { 2 } else { attack })
                            );
                            assert_eq!(actor.base.target_speed, limit as u8);
                            assert_eq!(actor.base.speed, if mode == 0 { 63 } else { 30 });
                            // Per-step generation was enabled before later SETVELs.
                            // Path dispatch alone retains the prefix velocity;
                            // the scheduler regenerates it at movement entry.
                            assert_eq!(actor.base.velocity, inherited_velocity);
                            if visit == 0 {
                                let mut moved = actor.clone();
                                super::super::path_motion::before_callbacks(
                                    &mut moved,
                                    owner,
                                    super::super::path_motion::PlayerDisplacement::default(),
                                );
                                assert_eq!(
                                    moved.base.velocity,
                                    super::super::path_motion::direction_velocity(
                                        actor.base.pitch,
                                        actor.base.yaw,
                                        actor.base.speed,
                                        4
                                    )
                                );
                                assert_eq!(
                                    moved.base.position,
                                    Vector3 {
                                        x: actor
                                            .base
                                            .position
                                            .x
                                            .wrapping_add(moved.base.velocity.x),
                                        y: actor
                                            .base
                                            .position
                                            .y
                                            .wrapping_add(moved.base.velocity.y),
                                        z: actor
                                            .base
                                            .position
                                            .z
                                            .wrapping_add(moved.base.velocity.z),
                                    }
                                );
                            }
                            assert_eq!(actor.base.shape, ShapeId::from_catalog_index(357));
                            assert!(!actor.base.flags.casts_shadow);
                            assert_eq!(
                                actor.extension.path_state.motion_phase,
                                0xAB00 | u16::from(mode)
                            );
                            let records = actor
                                .extension
                                .path_state
                                .triggers
                                .entries(&runtime.resources, owner)
                                .unwrap();
                            if mode == 0 {
                                assert_eq!(
                                    records.len(),
                                    if exit == 4 && visit >= 4 { 2 } else { 4 }
                                );
                                let increments = if exit == 3 && visit == last_visit {
                                    visit
                                } else {
                                    (visit + 1).min(last_visit)
                                };
                                // Terminal visits do not execute callbacks, except forced exits.
                                let increments = if (exit == 1 || exit == 2) && visit == last_visit
                                {
                                    visit + 1
                                } else {
                                    increments
                                };
                                assert_eq!(
                                    actor.extension.path_state.script_value,
                                    (if exit == 2 { 56u16 } else { u16::MAX })
                                        .wrapping_add(increments as u16)
                                );
                            } else {
                                assert_eq!(records.len(), 3); // Both source NewContact registrations survive.
                                assert_eq!(actor.extension.path_state.script_value, u16::MAX);
                            }
                        }
                        assert_eq!(
                            audio
                                .take_events()
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>(),
                            [SoundEvent::Authored(AuthoredCue::new(
                                114,
                                0,
                                PlayerTarget::Secondary
                            ))]
                        );
                        assert_eq!(random, initial_random);
                        assert_eq!(objects.len(), 3);
                        runtime.release_actor_programs(&mut objects, owner).unwrap();
                        runtime
                            .release_actor_programs(&mut objects, child.unwrap())
                            .unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn authored_occupancy_surface_root_spawns_independently_then_runs_all_lifetime_exits() {
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_sound::{
            AuthoredCue, CueListener, CueMarker, MarkerInputs, PathAudio,
        };
        use super::super::world_occupancy::{
            MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle,
        };
        use super::super::{
            authored_paths, path_motion, Angle, AudioState, Difficulty, ObjectSpawnDefaults,
            SoundEvent, SpatialLoop, Vector3,
        };
        let catalog = authored_paths::catalog();
        let mut occupancy = WorldOccupancy::default();
        occupancy.apply(
            &MarkerCoverage::from_rectangle(WorldRectangle {
                x: 0,
                z: 0,
                width: 1,
                depth: 1,
            })
            .unwrap(),
            OccupancyChange::Mark,
        );
        for (difficulty, attack) in [
            (Difficulty::Normal, 2),
            (Difficulty::Hard, 4),
            (Difficulty::Expert, 6),
        ] {
            for auxiliary_mode in [0, 31] {
                // Expiry, occupancy, ground, surface, and new-contact exits.
                for (exit, last_visit) in [(0, 53), (1, 4), (2, 5), (3, 6), (4, 7)] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let mut surface = Object::new(
                        ObjectKind::Scenery,
                        ShapeId::from_catalog_index(156),
                        Behavior::FollowPath,
                    );
                    surface.base.position.x = 1000;
                    surface.base.contacts.first_strategy_visit = false;
                    let support = objects.allocate(surface).unwrap();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(authored_paths::OCCUPANCY_SURFACE_LIMITED);
                    actor.base.position.y = -1000;
                    actor.extension.spawn_group = 17;
                    actor.extension.surface_contact.group = 171;
                    actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
                    let pitch = Angle::from_units(if auxiliary_mode == 31 { 0 } else { 253 });
                    let velocity = path_motion::direction_velocity(pitch, Angle::ZERO, 20, 4);
                    let initial_random = random;
                    let mut audio = AudioState::default();
                    let mut auxiliary = SelectedAuxiliaryState {
                        mode: auxiliary_mode,
                        action_flags: 0,
                    };
                    let marker = CueMarker {
                        identity: CueListener::Other,
                        position: Vector3 {
                            x: 0,
                            y: 0,
                            z: -100,
                        },
                        bearing: Angle::ZERO,
                    };
                    let mut child = None;
                    for visit in 0..=last_visit {
                        if visit == last_visit && exit == 2 {
                            objects.get_mut(owner).unwrap().base.position.y = 0;
                        }
                        if visit == last_visit && exit == 3 {
                            objects.get_mut(support).unwrap().base.position.x = 0;
                            objects.get_mut(owner).unwrap().base.position.y = -403;
                        }
                        let mut inputs = world(&mut random);
                        if visit == 0 {
                            inputs.campaign = Some(CampaignPathInputs {
                                difficulty,
                                encounter_variant: 0,
                            });
                            inputs.selected_auxiliary = Some(&mut auxiliary);
                            inputs.spawn_defaults = Some(ObjectSpawnDefaults {
                                group: 99,
                                run_when_paused: true,
                            });
                            inputs.audio = Some(PathAudio {
                                events: &mut audio,
                                listeners: [CueListener::PrimaryPlayer; 2],
                                markers: Some(MarkerInputs {
                                    selected_sides: [PlayerTarget::Primary; 2],
                                    markers: [marker; 2],
                                }),
                            });
                        }
                        if visit >= 3 {
                            let occupied = exit == 1 && visit == last_visit;
                            inputs.selected_occupancy_exempt = Some(!occupied);
                            if occupied {
                                inputs.occupancy = Some(&occupancy);
                            }
                            // Short-circuit exits need no surface-mode input.
                            if !occupied && !(exit == 2 && visit == last_visit) {
                                inputs.surface_mode = Some(SurfaceMode { flags: 0 });
                            }
                        }
                        let mut outcome = runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 40)
                            .unwrap();
                        if visit == 0 {
                            child = runtime.spawns.last_spawn;
                            let spawned = objects.get(child.unwrap()).unwrap();
                            assert_eq!(spawned.base.kind, ObjectKind::Effect);
                            assert_eq!(spawned.base.shape, ShapeId::from_catalog_index(19));
                            assert_eq!(
                                spawned.base.position,
                                Vector3 {
                                    x: 0,
                                    y: -1000,
                                    z: 0
                                }
                            );
                            assert_eq!(spawned.base.pitch, Angle::ZERO); // Allocated before parent's -3 pitch.
                            assert_eq!(spawned.base.velocity, Vector3::default());
                            assert_eq!(
                                (spawned.base.hit_points, spawned.base.attack_power),
                                (1, 1)
                            );
                            assert_eq!(spawned.extension.spawn_group, 17);
                            assert_eq!(spawned.extension.parent, None);
                            assert_eq!(spawned.base.attachment, None);
                            assert!(spawned.base.contacts.run_when_paused);
                            assert!(!spawned.base.flags.reclaim_on_pool_pressure);
                            assert!(spawned.extension.path_state.needs_path_initialization);
                        }
                        if outcome == ControlStep::Movement {
                            if exit == 4 && visit == last_visit {
                                objects
                                    .get_mut(owner)
                                    .unwrap()
                                    .base
                                    .contacts
                                    .new_contact_latched = true;
                            }
                            let callbacks = runtime.begin_callbacks(&objects, owner).unwrap();
                            assert_eq!(callbacks, visit >= 3);
                            if callbacks {
                                loop {
                                    match runtime
                                        .step_callbacks(
                                            &mut objects,
                                            owner,
                                            TriggerWorldInputs::default(),
                                        )
                                        .unwrap()
                                    {
                                        CallbackStep::Complete => break,
                                        CallbackStep::Run(_) => assert_eq!(
                                            runtime.resume_program(
                                                &catalog,
                                                &mut objects,
                                                owner,
                                                &mut inputs,
                                                8
                                            ),
                                            Ok(ControlStep::ResumeCallbacks)
                                        ),
                                        CallbackStep::Skipped | CallbackStep::Expired => {}
                                    }
                                }
                            }
                            if visit == last_visit {
                                assert_eq!(
                                    catalog
                                        .statement(objects.get(owner).unwrap().base.path.unwrap())
                                        .unwrap(),
                                    Statement::Control(ControlCommand::End)
                                );
                                outcome = runtime
                                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 2)
                                    .unwrap();
                            }
                        }
                        assert_eq!(
                            outcome,
                            if visit == last_visit {
                                ControlStep::Ended
                            } else {
                                ControlStep::Movement
                            }
                        );
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(
                            (actor.base.hit_points, actor.base.attack_power),
                            (120, attack)
                        );
                        assert_eq!(actor.base.shape, ShapeId::from_catalog_index(44));
                        assert_eq!(actor.base.pitch, pitch);
                        assert_eq!(actor.base.velocity, velocity);
                        assert_eq!(actor.base.flags.collision_disabled, visit < 3);
                        assert_eq!(
                            actor.extension.spatial_loop,
                            SpatialLoop::from_authored_control(3)
                        );
                        assert_eq!(actor.extension.surface_contact.group, 171);
                        assert_eq!(
                            actor.extension.surface_contact.supporting_object,
                            if exit == 3 && visit == last_visit {
                                Some(support)
                            } else {
                                None
                            }
                        );
                        let child = child.unwrap();
                        if visit < 3 {
                            assert_eq!(
                                runtime
                                    .enter_program(
                                        &catalog,
                                        &mut objects,
                                        child,
                                        &mut world(&mut random),
                                        10
                                    )
                                    .unwrap(),
                                if visit == 2 {
                                    ControlStep::Ended
                                } else {
                                    ControlStep::Movement
                                }
                            );
                            let child = objects.get(child).unwrap();
                            assert!(child.base.flags.collision_disabled);
                            assert_eq!(child.extension.texture_scroll_x, 16);
                            assert_eq!(
                                child.extension.path_state.animation.color.fixed_frame(),
                                Some(if visit == 1 { 0 } else { 1 })
                            );
                        }
                    }
                    assert_eq!(objects.len(), 3);
                    assert_eq!(
                        audio
                            .take_events()
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>(),
                        [SoundEvent::Authored(AuthoredCue::new(
                            117,
                            0,
                            PlayerTarget::Secondary
                        ))]
                    );
                    assert_eq!(random, initial_random);
                    runtime
                        .release_actor_programs(&mut objects, child.unwrap())
                        .unwrap();
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn authored_primary_motion_surface_root_initializes_once_and_runs_forty_visits() {
        use super::super::collision_surface::SurfaceMode;
        use super::super::path_sound::{
            AuthoredCue, CueListener, CueMarker, MarkerInputs, PathAudio,
        };
        use super::super::{
            authored_paths, path_motion, Angle, AudioState, SoundEvent, SpatialLoop, Vector3,
        };
        for auxiliary_mode in [0, 31] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let mut primary =
                Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
            primary.base.velocity = Vector3 {
                x: 100,
                y: 200,
                z: -300,
            };
            let player = objects.allocate(primary).unwrap();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::PRIMARY_MOTION_SURFACE_LIMITED);
            actor.base.position.y = -500;
            actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
            actor.extension.path_state.motion_phase = 0xABCD;
            let initial_random = random;
            let direction = path_motion::direction_velocity(Angle::ZERO, Angle::ZERO, 80, 1);
            let inherited = if auxiliary_mode == 31 {
                objects.get(player).unwrap().base.velocity
            } else {
                Vector3 { x: -7, y: 8, z: 9 }
            };
            let velocity = Vector3 {
                x: direction.x.wrapping_add(inherited.x),
                y: direction.y,
                z: direction.z.wrapping_add(inherited.z),
            };
            let catalog = authored_paths::catalog();
            let mut audio = AudioState::default();
            let marker = CueMarker {
                identity: CueListener::Other,
                position: Vector3 {
                    x: 0,
                    y: 1000,
                    z: -100,
                },
                bearing: Angle::ZERO,
            };
            for visit in 0..40 {
                let mut inputs = world(&mut random);
                inputs.surface_mode = Some(SurfaceMode { flags: 0 });
                if visit == 0 {
                    inputs.primary_player = Some(player);
                    inputs.primary_motion = Some(PrimaryMotionInput {
                        auxiliary_mode,
                        displacement: Vector3 { x: -7, y: 8, z: 9 },
                    });
                    inputs.audio = Some(PathAudio {
                        events: &mut audio,
                        listeners: [CueListener::PrimaryPlayer; 2],
                        markers: Some(MarkerInputs {
                            selected_sides: [PlayerTarget::Primary; 2],
                            markers: [marker; 2],
                        }),
                    });
                }
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 32)
                        .unwrap(),
                    if visit == 39 {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                if visit < 39 {
                    assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                    let mut executed = 0;
                    loop {
                        match runtime
                            .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                            .unwrap()
                        {
                            CallbackStep::Complete => break,
                            CallbackStep::Run(_) => {
                                executed += 1;
                                assert_eq!(
                                    runtime.resume_program(
                                        &catalog,
                                        &mut objects,
                                        owner,
                                        &mut inputs,
                                        4
                                    ),
                                    Ok(ControlStep::ResumeCallbacks)
                                );
                            }
                            CallbackStep::Skipped | CallbackStep::Expired => {}
                        }
                    }
                    assert_eq!(executed, 1);
                }
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.hit_points, 10);
                assert_eq!(actor.base.attack_power, 4);
                assert_eq!(actor.base.target_speed, 40);
                assert_eq!(actor.base.speed, 80);
                assert_eq!(actor.base.velocity, velocity);
                assert_eq!(
                    actor.base.position,
                    Vector3 {
                        x: 0,
                        y: -500,
                        z: 0
                    }
                );
                assert_eq!(actor.base.shape, ShapeId::from_catalog_index(31));
                assert!(actor.base.flags.scaled_sprite);
                assert!(!actor.base.flags.casts_shadow);
                assert_eq!(actor.extension.depth_offset, 0);
                assert_eq!(actor.extension.texture_scroll_x, 5);
                assert_eq!(
                    actor.extension.spatial_loop,
                    SpatialLoop::from_authored_control(12)
                );
                assert_eq!(actor.extension.path_state.motion_phase, 0xAB00);
                // Changes after setup cannot be inherited a second time.
                objects.get_mut(player).unwrap().base.velocity = Vector3 {
                    x: -2000,
                    y: 3000,
                    z: 4000,
                };
            }
            assert_eq!(
                audio
                    .take_events()
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>(),
                [SoundEvent::Authored(AuthoredCue::new(
                    115,
                    0,
                    PlayerTarget::Secondary
                ))]
            );
            assert_eq!(random, initial_random);
            assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }

    #[test]
    fn authored_surface_root_keeps_ten_visit_loop_and_live_surface_ground_contact_callbacks() {
        use super::super::{
            authored_paths,
            collision_surface::{ActorSurfaceContact, SurfaceMode},
        };
        let catalog = authored_paths::catalog();
        // Mode selection is sampled once, while the surface search mode and
        // geometry remain live on every callback. Ground short-circuits the
        // surface query only in the nonzero-mode branch.
        for initial_mode in [0, 1, 8] {
            for homing in [false, true] {
                for (surface_visit, ground_visit, contact_visit, last_visit) in [
                    (None, None, None, 9),
                    (Some(2), None, None, 2),
                    (None, Some(3), None, 3),
                    (None, None, Some(4), 4),
                ] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let mut target =
                        Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
                    target.base.position.z = 1000;
                    let player = objects.allocate(target).unwrap();
                    let mut surface = Object::new(
                        ObjectKind::Enemy,
                        ShapeId::from_catalog_index(156),
                        Behavior::FollowPath,
                    );
                    surface.base.contacts.first_strategy_visit = false;
                    surface.base.position.x = 1000;
                    let support = objects.allocate(surface).unwrap();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(authored_paths::SURFACE_OR_GROUND_LIMITED);
                    actor.base.shape = ShapeId::from_catalog_index(if homing { 363 } else { 0 });
                    actor.base.position.y = -500;
                    actor.base.wait_timer = 37;
                    actor.extension.path_state.motion_phase = 0xABCD;
                    actor.extension.surface_contact = ActorSurfaceContact {
                        supporting_object: Some(player),
                        group: 171,
                        flags: 99,
                    };
                    let initial_random = random;
                    for visit in 0..=last_visit {
                        if surface_visit == Some(visit) {
                            objects.get_mut(support).unwrap().base.position.x = 0;
                            objects.get_mut(owner).unwrap().base.position.y = -403;
                        }
                        if ground_visit == Some(visit) {
                            objects.get_mut(owner).unwrap().base.position.y = 0;
                        }
                        let mut inputs = world(&mut random);
                        // All variants switch to reduced search AFTER setup,
                        // without reselecting the installed callback path.
                        inputs.surface_mode = Some(SurfaceMode {
                            flags: if visit == 0 { initial_mode } else { 1 },
                        });
                        inputs.primary_player = Some(player);
                        inputs.selected = Some(player);
                        inputs.fixed_players = [Some(player); 2];
                        let mut outcome = runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 32)
                            .unwrap();
                        if outcome == ControlStep::Movement {
                            if contact_visit == Some(visit) {
                                objects
                                    .get_mut(owner)
                                    .unwrap()
                                    .base
                                    .contacts
                                    .new_contact_latched = true;
                            }
                            // The nonzero callback must bypass missing surface
                            // inputs if its ground test succeeds first.
                            if initial_mode != 0 && ground_visit == Some(visit) {
                                inputs.surface_mode = None;
                            }
                            assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                            loop {
                                match runtime
                                    .step_callbacks(
                                        &mut objects,
                                        owner,
                                        TriggerWorldInputs::default(),
                                    )
                                    .unwrap()
                                {
                                    CallbackStep::Complete => break,
                                    CallbackStep::Run(_) => assert_eq!(
                                        runtime.resume_program(
                                            &catalog,
                                            &mut objects,
                                            owner,
                                            &mut inputs,
                                            8
                                        ),
                                        Ok(ControlStep::ResumeCallbacks)
                                    ),
                                    CallbackStep::Skipped | CallbackStep::Expired => {}
                                }
                            }
                            if visit == last_visit {
                                assert_eq!(
                                    catalog
                                        .statement(objects.get(owner).unwrap().base.path.unwrap())
                                        .unwrap(),
                                    Statement::Control(ControlCommand::End)
                                );
                                outcome = runtime
                                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 2)
                                    .unwrap();
                            }
                        }
                        assert_eq!(
                            outcome,
                            if visit == last_visit {
                                ControlStep::Ended
                            } else {
                                ControlStep::Movement
                            }
                        );
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(
                            actor.extension.path_state.motion_phase,
                            0xAB00 | u16::from(initial_mode)
                        );
                        assert_eq!(actor.extension.surface_contact.group, 171);
                        assert_eq!(
                            actor.extension.surface_contact.supporting_object,
                            if surface_visit == Some(visit) {
                                Some(support)
                            } else {
                                None
                            }
                        );
                        assert_eq!(
                            actor.extension.surface_contact.flags,
                            if surface_visit == Some(visit) { 6 } else { 0 }
                        );
                        assert_eq!(actor.base.speed, 50);
                        assert_eq!(actor.base.target_speed, 10);
                        assert!(!actor.base.flags.casts_shadow);
                        assert!(!runtime.branch.invert_next);
                    }
                    assert_eq!(random, initial_random);
                    assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn surface_branch_updates_support_and_flags_preserves_group_and_uses_wrapped_sign() {
        use super::super::collision_surface::{ActorSurfaceContact, SurfaceSearch};
        let catalog = PathCatalog::new(vec![vec![Statement::AtOrAboveSurface {
            taken: cursor(0, 2),
            next: cursor(0, 1),
        }]])
        .unwrap();
        for (search, x, y, contact, taken) in [
            (SurfaceSearch::Full, 0, -404, true, false),
            (SurfaceSearch::Full, 0, -403, true, true),
            (SurfaceSearch::Full, 0, -402, true, true),
            (SurfaceSearch::Full, 0, 399, true, true),
            (SurfaceSearch::Full, 0, 400, false, false),
            (SurfaceSearch::Reduced, 0, 400, false, true),
            (SurfaceSearch::Full, 1000, i16::MIN, false, true),
            (SurfaceSearch::Full, 1000, -16385, false, true),
            (SurfaceSearch::Full, 1000, -16384, false, false),
            (SurfaceSearch::Full, 1000, 16383, false, false),
            (SurfaceSearch::Full, 1000, 16384, false, true),
            (SurfaceSearch::Full, 1000, i16::MAX, false, true),
            (SurfaceSearch::Reduced, 1000, -1, false, false),
            (SurfaceSearch::Reduced, 1000, 0, false, true),
        ] {
            for inverted in [true, false] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let before_random = random;
                let mut surface = Object::new(
                    ObjectKind::Enemy,
                    ShapeId::from_catalog_index(156),
                    Behavior::FollowPath,
                );
                surface.base.contacts.first_strategy_visit = false;
                surface.base.flags.collision_disabled = true;
                let candidate = objects.allocate(surface).unwrap();
                runtime.branch.invert_next = inverted;
                let actor = objects.get_mut(owner).unwrap();
                actor.base.position.x = x;
                actor.base.position.y = y;
                actor.base.wait_timer = 43;
                actor.extension.surface_contact = ActorSurfaceContact {
                    supporting_object: Some(owner),
                    group: 171,
                    flags: 220,
                };
                let mut expected = objects.clone();
                let expected_cursor = if taken { cursor(0, 2) } else { cursor(0, 1) };
                let expected_actor = expected.get_mut(owner).unwrap();
                expected_actor.base.path = Some(expected_cursor);
                expected_actor.extension.surface_contact = ActorSurfaceContact {
                    supporting_object: contact.then_some(candidate),
                    group: 171,
                    flags: if contact { 6 } else { 0 },
                };
                let mut inputs = world(&mut random);
                inputs.surface_mode = Some(super::super::collision_surface::SurfaceMode {
                    flags: if search == SurfaceSearch::Full { 0 } else { 1 },
                });
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: expected_cursor,
                        executed: 1
                    })
                );
                assert_eq!(objects, expected, "search {search:?}, x {x}, y {y}");
                assert_eq!(runtime.branch.invert_next, inverted);
                assert_eq!(random, before_random);
            }
        }
    }

    #[test]
    fn surface_branch_missing_mode_unknown_shape_and_zero_budget_are_atomic() {
        use super::super::collision_surface::{ActorSurfaceContact, SurfaceQueryError};
        let catalog = PathCatalog::new(vec![vec![Statement::AtOrAboveSurface {
            taken: cursor(0, 2),
            next: cursor(0, 1),
        }]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let unknown_shape = ShapeId::from_catalog_index(u16::MAX);
        let mut surface = Object::new(ObjectKind::Enemy, unknown_shape, Behavior::FollowPath);
        surface.base.contacts.first_strategy_visit = false;
        let candidate = objects.allocate(surface).unwrap();
        objects.get_mut(owner).unwrap().extension.surface_contact = ActorSurfaceContact {
            supporting_object: Some(candidate),
            group: 125,
            flags: 83,
        };
        runtime.branch.invert_next = true;
        let before = objects.clone();
        let before_random = random;
        let mut inputs = world(&mut random);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::MissingSurfaceMode)
        );
        inputs.surface_mode = Some(super::super::collision_surface::SurfaceMode { flags: 0 });
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 0),
                executed: 0
            })
        );
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::SurfaceQuery(
                SurfaceQueryError::UnknownShape {
                    object: candidate,
                    shape: unknown_shape
                }
            ))
        );
        assert_eq!(objects, before);
        assert!(runtime.branch.invert_next);
        assert_eq!(random, before_random);
    }

    #[test]
    fn occupancy_branch_samples_live_owner_position_and_preserves_ifnot_and_wait() {
        use super::super::world_occupancy::{
            MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle,
        };
        use super::super::Vector3;
        let catalog = PathCatalog::new(vec![vec![Statement::OccupiedCell {
            taken: cursor(0, 2),
            next: cursor(0, 1),
        }]])
        .unwrap();
        let marker = MarkerCoverage::from_rectangle(WorldRectangle {
            x: -512,
            z: 512,
            width: 1,
            depth: 1,
        })
        .unwrap();
        let mut occupancy = WorldOccupancy::default();
        for occupied in [true, false, true] {
            occupancy.apply(
                &marker,
                if occupied {
                    OccupancyChange::Mark
                } else {
                    OccupancyChange::Erase
                },
            );
            for exempt in [true, false] {
                for inverted in [true, false] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let before_random = random;
                    runtime.branch.invert_next = inverted;
                    objects.get_mut(owner).unwrap().base.wait_timer = 59;
                    for (x, z, inside) in [
                        (-512, 512, true),
                        (-1, 1023, true),
                        (-513, 512, false),
                        (0, 512, false),
                        (-512, 511, false),
                        (-512, 1024, false),
                    ] {
                        for y in [i16::MIN, 0, i16::MAX] {
                            objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
                            objects.get_mut(owner).unwrap().base.position = Vector3 { x, y, z };
                            let mut expected = objects.clone();
                            let expected_cursor = if occupied && !exempt && inside {
                                cursor(0, 2)
                            } else {
                                cursor(0, 1)
                            };
                            expected.get_mut(owner).unwrap().base.path = Some(expected_cursor);
                            let mut inputs = world(&mut random);
                            inputs.selected_occupancy_exempt = Some(exempt);
                            // An exempt actor must not need a map at all.
                            if !exempt {
                                inputs.occupancy = Some(&occupancy);
                            }
                            assert_eq!(
                                runtime.resume_program(
                                    &catalog,
                                    &mut objects,
                                    owner,
                                    &mut inputs,
                                    1
                                ),
                                Err(ProgramError::BudgetExceeded {
                                    cursor: expected_cursor,
                                    executed: 1
                                })
                            );
                            assert_eq!(objects, expected);
                            assert_eq!(runtime.branch.invert_next, inverted);
                            assert_eq!(random, before_random);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn occupancy_missing_inputs_fault_before_changing_any_actor_or_branch_state() {
        let catalog = PathCatalog::new(vec![vec![Statement::OccupiedCell {
            taken: cursor(0, 2),
            next: cursor(0, 1),
        }]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        runtime.branch.invert_next = true;
        let before = objects.clone();
        let before_random = random;
        let mut inputs = world(&mut random);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::MissingOccupancyExemption)
        );
        inputs.selected_occupancy_exempt = Some(false);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::MissingOccupancy)
        );
        assert_eq!(objects, before);
        assert!(runtime.branch.invert_next);
        assert_eq!(random, before_random);
    }

    #[test]
    fn guidance_word_transfers_preserve_all_bits_and_unrelated_state() {
        use super::super::path_fields::WordField;
        for command in [
            GuidanceCommand::CopyTo(WordField::ScriptValue),
            GuidanceCommand::Assign(WordOperand::Actor(WordField::ScriptValue)),
        ] {
            let catalog = PathCatalog::new(vec![vec![Statement::Guidance {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.wait_timer = 57;
            runtime.branch.invert_next = true;
            let before = objects.clone();
            let before_random = random;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingGuidance)
            );
            assert_eq!(objects, before);
            for flags in 0..=u16::MAX {
                objects = before.clone();
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .script_value = flags ^ 0xA55A;
                let mut expected = objects.clone();
                expected.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
                let mut history = GuidanceHistory { flags };
                let mut inputs = world(&mut random);
                inputs.guidance = Some(&mut history);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0, 1),
                        executed: 1
                    })
                );
                match command {
                    GuidanceCommand::CopyTo(_) => {
                        expected
                            .get_mut(owner)
                            .unwrap()
                            .extension
                            .path_state
                            .script_value = flags
                    }
                    GuidanceCommand::Assign(_) => assert_eq!(history.flags, flags ^ 0xA55A),
                }
                assert_eq!(objects, expected);
                assert!(runtime.branch.invert_next);
                assert_eq!(random, before_random);
            }
        }
    }

    #[test]
    fn control_style_import_preserves_high_byte_and_missing_input_is_atomic() {
        use super::super::path_fields::{ByteField, BytePart, WordField};
        use super::super::FlightControlStyle;
        let catalog = PathCatalog::new(vec![vec![Statement::ImportControlStyle {
            destination: ByteField::WordPart {
                field: WordField::MotionPhase,
                part: BytePart::Low,
            },
            next: cursor(0, 1),
        }]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0xABCD;
        objects.get_mut(owner).unwrap().base.wait_timer = 57;
        runtime.branch.invert_next = true;
        let before = objects.clone();
        let before_random = random;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingControlStyle)
        );
        assert_eq!(objects, before);
        for (style, value) in [
            (FlightControlStyle::TypeA, 0),
            (FlightControlStyle::TypeB, 1),
            (FlightControlStyle::TypeA, 0),
        ] {
            objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
            let mut inputs = world(&mut random);
            inputs.control_style = Some(style);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            let mut expected = before.clone();
            let actor = expected.get_mut(owner).unwrap();
            actor.base.path = Some(cursor(0, 1));
            actor.extension.path_state.motion_phase = 0xAB00 | value;
            assert_eq!(objects, expected);
            assert!(runtime.branch.invert_next);
            assert_eq!(random, before_random);
        }
    }

    #[test]
    fn first_control_guidance_preserves_history_difficulty_layout_and_all_waits() {
        use super::super::path_countdown::PathCountdown;
        use super::super::path_radio::{PathRadio, RadioLayout, RadioRequest};
        use super::super::{authored_paths, Difficulty, FlightControlStyle};
        let catalog = authored_paths::catalog();
        for difficulty in [Difficulty::Normal, Difficulty::Hard, Difficulty::Expert] {
            for style in [FlightControlStyle::TypeA, FlightControlStyle::TypeB] {
                for flags in [0, 0x0100, 0xFEFF, 0xFFFF] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let before_random = random;
                    objects.get_mut(owner).unwrap().base.path =
                        Some(authored_paths::FIRST_CONTROL_GUIDANCE);
                    objects
                        .get_mut(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .motion_phase = 0xAB00;
                    let mut history = GuidanceHistory { flags };
                    let mut auxiliary = SelectedAuxiliaryState {
                        mode: 0xA2,
                        action_flags: 0xFF,
                    };
                    let mut request = RadioRequest::default();
                    let mut countdown = PathCountdown { remaining: 17 };
                    let final_visit = if difficulty != Difficulty::Normal {
                        0
                    } else if flags & 0x0100 != 0 {
                        16
                    } else {
                        375
                    };
                    let mut messages = Vec::new();
                    for visit in 0..=final_visit {
                        request.pending = false;
                        let before_countdown = countdown.remaining;
                        let mut inputs = world(&mut random);
                        // Supply each shared input only where the graph needs it.
                        if visit == 0 {
                            inputs.campaign = Some(CampaignPathInputs {
                                difficulty,
                                encounter_variant: 0,
                            });
                        }
                        if visit == 16 {
                            inputs.guidance = Some(&mut history);
                            inputs.selected_auxiliary = Some(&mut auxiliary);
                        }
                        if [46, 112, 178, 244, 310].contains(&visit) {
                            inputs.countdown = Some(&mut countdown);
                            inputs.radio = Some(PathRadio {
                                request: &mut request,
                                layout: RadioLayout {
                                    compact_panel: false,
                                    tracked_screen_y: 100,
                                },
                            });
                        }
                        if visit == 244 {
                            inputs.control_style = Some(style);
                        }
                        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 64), Ok(if visit == final_visit { ControlStep::Ended } else { ControlStep::Movement }), "difficulty {difficulty:?}, style {style:?}, flags {flags:04x}, visit {visit}");
                        if request.pending {
                            messages.push((visit, request.message.index() + 1));
                            assert_eq!(countdown.remaining, 80);
                        } else {
                            assert_eq!(countdown.remaining, before_countdown);
                        }
                        countdown.remaining = countdown.remaining.wrapping_sub(1);
                        assert_eq!(
                            history.flags,
                            if visit >= 16 { flags | 0x0100 } else { flags }
                        );
                        assert_eq!(auxiliary.mode, if visit >= 16 { 0xA4 } else { 0xA2 });
                        assert_eq!(auxiliary.action_flags, 0xFF);
                        assert_eq!(random, before_random);
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.base.hit_points, 100);
                        assert!(!actor.base.flags.visible);
                        assert!(actor.base.flags.collision_disabled);
                        assert_eq!(actor.base.flags.remove_after_tick, visit == final_visit);
                        assert_eq!(actor.extension.path_state.motion_phase & 0xFF00, 0xAB00);
                        assert!(!runtime.branch.invert_next);
                    }
                    assert_eq!(
                        messages,
                        if final_visit == 375 {
                            vec![
                                (46, 204),
                                (112, 205),
                                (178, 206),
                                (
                                    244,
                                    if style == FlightControlStyle::TypeA {
                                        207
                                    } else {
                                        213
                                    },
                                ),
                                (310, 208),
                            ]
                        } else {
                            vec![]
                        }
                    );
                    if final_visit == 375 {
                        assert_eq!(
                            objects
                                .get(owner)
                                .unwrap()
                                .extension
                                .path_state
                                .script_value,
                            209
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn campaign_imports_keep_byte_width_and_resample_live_inputs_without_control_effects() {
        use super::super::path_fields::{ByteField, BytePart, WordField};
        use super::super::Difficulty;
        let destination = ByteField::WordPart {
            field: WordField::ScriptValue,
            part: BytePart::Low,
        };
        for source in [CampaignByte::Difficulty, CampaignByte::EncounterVariant] {
            let catalog = PathCatalog::new(vec![vec![Statement::ImportCampaignByte {
                source,
                destination,
                next: cursor(0, 1),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.branch.invert_next = true;
            objects.get_mut(owner).unwrap().base.wait_timer = 57;
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .script_value = 0xABCD;
            let before = objects.clone();
            let before_random = random;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingCampaign)
            );
            assert_eq!(objects, before);
            for (difficulty, code) in [
                (Difficulty::Normal, 0),
                (Difficulty::Hard, 1),
                (Difficulty::Expert, 2),
            ] {
                for encounter_variant in 0..=u8::MAX {
                    objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
                    let mut inputs = world(&mut random);
                    inputs.campaign = Some(CampaignPathInputs {
                        difficulty,
                        encounter_variant,
                    });
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(0, 1),
                            executed: 1
                        })
                    );
                    let value = if source == CampaignByte::Difficulty {
                        code
                    } else {
                        encounter_variant
                    };
                    let mut expected = before.clone();
                    expected.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
                    expected
                        .get_mut(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .script_value = 0xAB00 | u16::from(value);
                    assert_eq!(objects, expected);
                    assert!(runtime.branch.invert_next);
                    assert_eq!(random, before_random);
                }
            }
        }
    }

    #[test]
    fn complete_encounter_radio_path_selects_messages_and_preserves_timed_sequence() {
        use super::super::path_radio::{PathRadio, RadioLayout, RadioRequest};
        use super::super::{authored_paths, Difficulty};
        let catalog = authored_paths::catalog();
        for difficulty in [Difficulty::Normal, Difficulty::Hard, Difficulty::Expert] {
            for encounter_variant in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let before_random = random;
                objects.get_mut(owner).unwrap().base.path =
                    Some(authored_paths::ENCOUNTER_RADIO_SERVICE);
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase = 0xAB00;
                let mut request = RadioRequest::default();
                let mut messages = Vec::new();
                let final_visit = if encounter_variant == 4 { 102 } else { 10 };
                for visit in 0..=final_visit {
                    request.pending = false;
                    let mut inputs = world(&mut random);
                    if visit >= 10 {
                        inputs.campaign = Some(CampaignPathInputs {
                            difficulty,
                            encounter_variant,
                        });
                        inputs.radio = Some(PathRadio {
                            request: &mut request,
                            layout: RadioLayout {
                                compact_panel: true,
                                tracked_screen_y: 100,
                            },
                        });
                    }
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 32),
                        Ok(if visit == final_visit {
                            ControlStep::Ended
                        } else {
                            ControlStep::Movement
                        }),
                        "difficulty {difficulty:?}, variant {encounter_variant}, visit {visit}"
                    );
                    assert_eq!(random, before_random);
                    if request.pending {
                        messages.push((visit, request.message.index() + 1));
                        assert_eq!(request.panel_y, 139);
                        assert!(!request.top_placement);
                    }
                    let actor = objects.get(owner).unwrap();
                    assert!(!actor.base.flags.visible);
                    assert!(actor.base.flags.collision_disabled);
                    assert_eq!(actor.base.flags.remove_after_tick, visit == final_visit);
                    assert_eq!(actor.extension.path_state.motion_phase & 0xFF00, 0xAB00);
                }
                assert_eq!(
                    messages,
                    if encounter_variant == 4 {
                        vec![(10, 23), (41, 24), (72, 25)]
                    } else {
                        vec![(
                            10,
                            match difficulty {
                                Difficulty::Normal => 112,
                                Difficulty::Hard => 119,
                                Difficulty::Expert => 85,
                            },
                        )]
                    }
                );
                assert_eq!(objects.get(owner).unwrap().base.wait_timer, 0);
            }
        }
    }

    #[test]
    fn encounter_radio_resamples_variant_at_the_later_difficulty_specific_branch() {
        use super::super::path_radio::{PathRadio, RadioLayout, RadioRequest};
        use super::super::{authored_paths, Difficulty};
        for (difficulty, message) in [(Difficulty::Hard, 117), (Difficulty::Expert, 120)] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.path =
                Some(authored_paths::ENCOUNTER_RADIO_SERVICE);
            objects.get_mut(owner).unwrap().base.wait_timer = 10;
            let catalog = authored_paths::catalog();
            let mut inputs = world(&mut random);
            inputs.campaign = Some(CampaignPathInputs {
                difficulty,
                encounter_variant: 0,
            });
            let Err(ProgramError::BudgetExceeded {
                cursor: late_import,
                executed: 7,
            }) = runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 7)
            else {
                panic!("first seven statements stop before the second variant import");
            };
            assert!(matches!(
                catalog.statement(late_import).unwrap(),
                Statement::ImportCampaignByte {
                    source: CampaignByte::EncounterVariant,
                    ..
                }
            ));
            inputs.campaign.as_mut().unwrap().encounter_variant = 4;
            let mut request = RadioRequest::default();
            inputs.radio = Some(PathRadio {
                request: &mut request,
                layout: RadioLayout {
                    compact_panel: false,
                    tracked_screen_y: 100,
                },
            });
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 8),
                Ok(ControlStep::Ended)
            );
            assert_eq!(request.message.index() + 1, message);
            assert!(request.pending);
        }
    }

    #[test]
    fn missing_auxiliary_observation_errors_at_branch_without_silent_fallthrough() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![
            Statement::SelectedAuxiliaryBranch {
                condition: SelectedAuxiliaryCondition::Continuation,
                taken: cursor(0, 2),
                next: cursor(0, 1),
            },
            Statement::Control(ControlCommand::WaitOne { next: cursor(0, 2) }),
            Statement::Control(ControlCommand::End),
        ]])
        .unwrap();
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 3),
            Err(ProgramError::MissingSelectedAuxiliary)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 0)));
        let mut auxiliary = SelectedAuxiliaryState {
            mode: 0,
            action_flags: 0,
        };
        let mut inputs = world(&mut random);
        inputs.selected_auxiliary = Some(&mut auxiliary);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 2),
            Ok(ControlStep::Movement)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 2)));
        assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
    }

    #[test]
    fn authored_primary_motion_path_exits_on_loop_limit_ground_or_new_contact() {
        use super::super::collision_pass::ExclusionGroups;
        use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
        use super::super::{authored_paths, path_motion, Vector3};
        // No movement is requested here: each yield remains an explicit
        // scheduler boundary. Independently exercise all three source exits.
        for (ground_visit, contact_visit, last_visit) in [
            (None, None, 9),
            (Some(0), None, 0),
            (Some(3), None, 3),
            (None, Some(2), 2),
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let mut primary =
                Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
            primary.base.velocity = Vector3 {
                x: 123,
                y: 456,
                z: -789,
            };
            let primary = objects.allocate(primary).unwrap();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::PRIMARY_MOTION_GROUND_LIMITED);
            actor.base.position.y = -100;
            actor.base.flags.casts_shadow = true;
            actor.base.contacts.exclusion_groups = ExclusionGroups::PATH_SPAWN;
            actor.base.contacts.first_strategy_visit = true;
            actor.base.contacts.suppress_attack_damage = true;
            actor.base.wait_timer = 37;
            let initial_random = random;
            let direction =
                path_motion::direction_velocity(actor.base.pitch, actor.base.yaw, 60, 1);
            let expected_velocity = Vector3 {
                x: direction.x.wrapping_add(123),
                y: direction.y,
                z: direction.z.wrapping_add(-789),
            };
            let primary_before = objects.get(primary).unwrap().clone();
            let catalog = authored_paths::catalog();
            for visit in 0..=last_visit {
                if ground_visit == Some(visit) {
                    objects.get_mut(owner).unwrap().base.position.y = 0;
                }
                let mut inputs = world(&mut random);
                // The inline action runs once. Later iterations require no
                // primary observations, and must not add its motion again.
                if visit == 0 {
                    inputs.primary_player = Some(primary);
                    inputs.primary_motion = Some(PrimaryMotionInput {
                        auxiliary_mode: 0x1F,
                        displacement: Vector3 {
                            x: -1,
                            y: -2,
                            z: -3,
                        },
                    });
                }
                let mut outcome = runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 32)
                    .unwrap();
                if contact_visit == Some(visit) {
                    assert_eq!(outcome, ControlStep::Movement);
                    objects
                        .get_mut(owner)
                        .unwrap()
                        .base
                        .contacts
                        .new_contact_latched = true;
                    assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                    assert!(matches!(
                        runtime
                            .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                            .unwrap(),
                        CallbackStep::Run(_)
                    ));
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 4),
                        Ok(ControlStep::ResumeCallbacks)
                    );
                    assert_eq!(
                        runtime
                            .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                            .unwrap(),
                        CallbackStep::Complete
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(
                        catalog.statement(actor.base.path.unwrap()).unwrap(),
                        Statement::Control(ControlCommand::End)
                    );
                    assert!(!actor.base.flags.remove_after_tick);
                    outcome = runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 2)
                        .unwrap();
                }
                assert_eq!(
                    outcome,
                    if visit == last_visit {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.velocity, expected_velocity);
                assert_eq!(
                    actor.base.position,
                    Vector3 {
                        x: 0,
                        y: if ground_visit == Some(visit) { 0 } else { -100 },
                        z: 0
                    }
                );
                assert_eq!(actor.base.hit_points, 120);
                assert_eq!(actor.base.attack_power, 2);
                assert_eq!(actor.base.target_speed, 10);
                assert_eq!(actor.base.speed, 60);
                assert!(!actor.base.flags.casts_shadow);
                assert!(actor.base.contacts.credits_hit_side);
                assert!(actor.base.contacts.mutually_non_damaging);
                assert!(actor.base.contacts.first_strategy_visit);
                assert!(actor.base.contacts.suppress_attack_damage);
                assert!(actor.base.contacts.suppress_hit_marker);
                assert_eq!(
                    actor.base.contacts.exclusion_groups,
                    ExclusionGroups::from_authored_class(0x88)
                );
                assert_eq!(actor.base.flags.remove_after_tick, visit == last_visit);
                assert_eq!(objects.get(primary).unwrap(), &primary_before);
                assert_eq!(random, initial_random);
            }
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }

    #[test]
    fn player_control_requires_live_primary_and_target_record_before_mutation() {
        use super::super::path_player_control::{
            primary_position, PlayerControlCommand, PlayerTargetControl, PrimaryControl,
        };
        for command in [
            PlayerControlCommand::LockToProjectile,
            PlayerControlCommand::Configure(-8),
            PlayerControlCommand::ConfigureDoubledLowByte(-8),
            PlayerControlCommand::ConfigureAlternateAxes(-8),
            PlayerControlCommand::LockForLinkedMode,
            PlayerControlCommand::FollowPrimaryPosition,
            PlayerControlCommand::RefreshOwnedOrigin,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let before = objects.get(owner).unwrap().clone();
            let original_random = random;
            let catalog = PathCatalog::new(vec![vec![Statement::PlayerControl {
                command,
                next: cursor(0, 1),
            }]])
            .unwrap();
            let mut inputs = world(&mut random);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::MissingPrimaryPlayer)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            inputs.primary_player = Some(owner);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::MissingPrimaryControl)
            );
            assert_eq!(objects.get(owner).unwrap(), &before);
            assert_eq!(inputs.random, &original_random);
            let mut control = PlayerTargetControl {
                owner: Some(owner),
                ..PlayerTargetControl::default()
            };
            let mut expected_control = control;
            let mut expected = before.clone();
            match command {
                PlayerControlCommand::LockToProjectile => {
                    expected_control.lock_to_projectile(owner, before.base.position)
                }
                PlayerControlCommand::Configure(range) => {
                    expected_control.configure(owner, before.base.position, range)
                }
                PlayerControlCommand::ConfigureDoubledLowByte(range) => {
                    expected_control.configure_doubled_low_byte(owner, before.base.position, range)
                }
                PlayerControlCommand::ConfigureAlternateAxes(range) => {
                    expected_control.configure_alternate_axes(owner, before.base.position, range)
                }
                PlayerControlCommand::LockForLinkedMode => {
                    expected_control.lock_for_linked_mode(true)
                }
                PlayerControlCommand::FollowPrimaryPosition => {
                    expected.base.position = primary_position(
                        before.base.position,
                        before.base.pitch,
                        before.base.yaw,
                        true,
                    )
                }
                PlayerControlCommand::RefreshOwnedOrigin => {
                    expected_control.refresh_owned_origin(owner, before.base.position)
                }
            }
            expected.base.path = Some(cursor(0, 1));
            inputs.primary_control = Some(PrimaryControl {
                target: &mut control,
                linked_mode: true,
            });
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert_eq!(
                *inputs.primary_control.as_ref().unwrap().target,
                expected_control
            );
            assert_eq!(inputs.random, &original_random);
        }
    }

    #[test]
    fn authored_primary_target_follower_runs_eight_live_updates_and_configures_only_once() {
        use super::super::path_player_control::{
            primary_position, PlayerTargetControl, PrimaryControl,
        };
        use super::super::path_sound::{AuthoredCue, CueListener, PathAudio};
        use super::super::{authored_paths, Angle, AudioState, SoundEvent, Vector3};
        for initially_linked in [false, true] {
            for locked in [false, true] {
                for owned in [false, true] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let primary = objects
                        .allocate(Object::new(
                            ObjectKind::Player,
                            ShapeId::EMPTY,
                            Behavior::PlayerFlight,
                        ))
                        .unwrap();
                    let original_random = random;
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(authored_paths::PRIMARY_TARGET_FOLLOWER);
                    actor.base.position = Vector3 { x: 1, y: 2, z: 3 };
                    actor.base.velocity = Vector3 { x: 9, y: -8, z: 7 };
                    actor.base.wait_timer = 57;
                    actor.base.pitch = Angle::from_units(21);
                    actor.base.yaw = Angle::from_units(22);
                    actor.base.roll = Angle::from_units(23);
                    actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
                    let stable_owner = actor.clone();
                    let mut control = PlayerTargetControl {
                        configuration_locked: locked,
                        offset_enabled: true,
                        owner: Some(if owned { owner } else { primary }),
                        origin: Vector3 {
                            x: -10,
                            y: -20,
                            z: -30,
                        },
                        range: -100,
                        positive_range: 200,
                        mode: 7,
                        ..PlayerTargetControl::default()
                    };
                    let mut expected_control = control;
                    expected_control.configure(owner, actor.base.position, -8);
                    expected_control.lock_for_linked_mode(initially_linked);
                    let mut audio = AudioState::default();
                    let catalog = authored_paths::catalog();
                    for visit in 0..8 {
                        let linked = if visit == 0 {
                            initially_linked
                        } else {
                            visit % 2 == 0
                        };
                        let player = objects.get_mut(primary).unwrap();
                        player.base.position = Vector3 {
                            x: i16::MAX - visit * 100,
                            y: i16::MIN + visit * 200,
                            z: visit * 300,
                        };
                        player.base.pitch = Angle::from_units((visit * 37) as u8);
                        player.base.yaw = Angle::from_units((visit * 51) as u8);
                        let expected_position = primary_position(
                            player.base.position,
                            player.base.pitch,
                            player.base.yaw,
                            linked,
                        );
                        let primary_before = player.clone();
                        expected_control.refresh_owned_origin(owner, expected_position);
                        let mut inputs = world(&mut random);
                        inputs.primary_player = Some(primary);
                        // A distinct selected actor must never replace primary.
                        inputs.selected = Some(owner);
                        inputs.primary_control = Some(PrimaryControl {
                            target: &mut control,
                            linked_mode: linked,
                        });
                        inputs.audio = Some(PathAudio {
                            events: &mut audio,
                            listeners: [CueListener::PrimaryPlayer, CueListener::Other],
                            markers: None,
                        });
                        runtime.branch.invert_next = true;
                        assert_eq!(
                            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16),
                            Ok(if visit < 7 {
                                ControlStep::Movement
                            } else {
                                ControlStep::Ended
                            })
                        );
                        assert_eq!(
                            *inputs.primary_control.as_ref().unwrap().target,
                            expected_control
                        );
                        let expected_cues = if visit == 0 {
                            vec![SoundEvent::Authored(AuthoredCue::new(
                                50,
                                0,
                                PlayerTarget::Secondary,
                            ))]
                        } else {
                            vec![]
                        };
                        assert_eq!(
                            inputs
                                .audio
                                .as_mut()
                                .unwrap()
                                .events
                                .take_events()
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>(),
                            expected_cues
                        );
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.base.position, expected_position);
                        assert_eq!(actor.base.velocity, stable_owner.base.velocity);
                        assert_eq!(
                            (actor.base.pitch, actor.base.yaw, actor.base.roll),
                            (
                                stable_owner.base.pitch,
                                stable_owner.base.yaw,
                                stable_owner.base.roll
                            )
                        );
                        assert_eq!(actor.base.wait_timer, 57);
                        assert!(actor.base.flags.collision_disabled);
                        assert_eq!(actor.base.flags.remove_after_tick, visit == 7);
                        assert_eq!(objects.get(primary).unwrap(), &primary_before);
                        assert_eq!(inputs.random, &original_random);
                        assert!(runtime.branch.invert_next);
                    }
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }

    #[test]
    fn authored_alternate_exhaust_runs_complete_graph_with_two_movement_yields() {
        use super::super::{authored_paths, path_appearance, path_motion};
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::ALTERNATE_EXHAUST);
        objects.get_mut(owner).unwrap().base.velocity.x = 7;
        let catalog = authored_paths::catalog();
        assert_eq!(authored_paths::LOWERED_ROOT_COUNT, 64);
        assert_eq!(authored_paths::LOWERED_COMMAND_COUNT, 1052);
        assert_eq!(authored_paths::LOWERED_SOURCE_COMMAND_COUNT, 1061);
        // Source DO 3 executes ADDCOL three times; NEXT only yields on its
        // first two decrements. The final pass reaches END without movement.
        for (invocation, color) in [1, 0, 1].into_iter().enumerate() {
            let outcome = runtime
                .enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8)
                .unwrap();
            assert_eq!(
                outcome,
                if invocation < 2 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            if outcome == ControlStep::Movement {
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
            }
            let actor = objects.get_mut(owner).unwrap();
            let clock = 49 + invocation as u8;
            path_appearance::publish_animation(actor, clock);
            assert_eq!(actor.extension.color_frame, color);
            assert_eq!(actor.extension.animation_frame, clock);
            assert!(actor.base.flags.scaled_sprite);
            assert_eq!(actor.extension.depth_offset, 0);
            assert_eq!(actor.extension.texture_scroll_x, 0);
            assert_eq!(actor.base.position.x, 7 * (invocation + 1).min(2) as i16);
            assert_eq!(actor.base.flags.remove_after_tick, invocation == 2);
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }

    #[test]
    fn authored_color_cycle_sprite_waits_on_zero_then_cycles_before_retirement() {
        use super::super::{authored_paths, path_appearance, path_motion};
        let (mut runtime, mut objects, owner, mut random) = setup();
        {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::COLOR_CYCLE_SPRITE);
            actor.base.hit_points = 10;
            actor.base.hit_flags = 0x40;
            actor.base.contacts.new_contact_latched = true;
            actor.base.velocity.x = 7;
        }
        let catalog = authored_paths::catalog();
        // INITCOL 0 / WAITONE supplies the first visible color. DO 7 then
        // adds 1..7; the last NEXT completes and END retires without a yield.
        for color in 0..8u8 {
            let outcome = runtime
                .enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8)
                .unwrap();
            assert_eq!(
                outcome,
                if color < 7 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            {
                let actor = objects.get_mut(owner).unwrap();
                path_appearance::publish_animation(actor, 133 + color);
                assert_eq!(actor.extension.color_frame, color);
                assert_eq!(actor.extension.animation_frame, 5 + color);
                assert!(actor.base.flags.collision_disabled);
                assert!(actor.base.flags.scaled_sprite);
                assert_eq!(actor.base.hit_points, 10);
                assert_eq!(actor.base.hit_flags, 0x40);
                assert_eq!(actor.extension.depth_offset, 0);
                assert_eq!(actor.extension.texture_scroll_x, 10);
                if color == 0 {
                    assert!(actor.base.contacts.new_contact_latched);
                }
            }
            if outcome == ControlStep::Movement {
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
            }
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.position.x, 7 * i16::from((color + 1).min(7)));
            assert_eq!(actor.base.flags.remove_after_tick, color == 7);
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }

    #[test]
    fn authored_particle_draws_six_shared_bytes_once_and_exits_at_phase_seven() {
        use super::super::{authored_paths, path_motion, Vector3};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let initial = Vector3 {
            x: i16::MAX,
            y: i16::MIN,
            z: 42,
        };
        {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::RANDOMIZED_COLOR_PARTICLE);
            actor.base.position = initial;
            actor.extension.path_state.motion_phase = 0xA500;
        }
        let mut expected_random = random;
        let mut jitter = || {
            let high = expected_random.next_byte();
            let low = expected_random.next_byte();
            (u16::from_be_bytes([high, low]) & 15) as i16 - 7
        };
        let expected = Vector3 {
            x: initial.x.wrapping_add(jitter()),
            y: initial.y.wrapping_add(jitter()),
            z: initial.z.wrapping_add(jitter()),
        };
        let catalog = authored_paths::catalog();
        for phase in 1..=7 {
            let mut inputs = PathWorld {
                scene: ScenePathInputs::default(),
                scenery_distance: None,
                targeting_upgrade: None,
                shield_recovery: None,
                action_gate: None,
                environment_plane_height: None,
                projectile_trigger: None,
                primary_pitch_recoil: None,
                linked_effect_activity: None,
                protection: None,
                audio: None,
                radio: None,
                campaign: None,
                guidance: None,
                pickup_history: None,
                control_style: None,
                selected_occupancy_exempt: None,
                occupancy: None,
                surface_mode: None,
                primary_player: None,
                selected: None,
                fixed_players: [None; 2],
                primary_motion: None,
                published_motion: None,
                active_charge_threshold: None,
                selected_charge: None,
                primary_control: None,
                primary_target: None,
                active_node_flags: None,
                selected_auxiliary: None,
                selected_equipment: None,
                selected_score: None,
                countdown: None,
                spawn_defaults: None,
                random: &mut random,
                animation_clock: 93,
            };
            let outcome = runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 16)
                .unwrap();
            assert_eq!(
                outcome,
                if phase < 7 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            assert_eq!(random, expected_random); // no per-iteration re-draw
            {
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.extension.path_state.motion_phase, 0xA500 | phase);
                assert_eq!(actor.extension.color_frame, phase as u8);
                assert_eq!(actor.extension.animation_frame, 93);
                assert_eq!(actor.base.target_speed, 0);
                assert_eq!(actor.base.acceleration, 5);
                assert_eq!(actor.base.position, expected);
                assert_eq!(actor.base.velocity.y, if phase < 7 { -1 } else { 0 });
            }
            if outcome == ControlStep::Movement {
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
                // Zero initial speed stays zero: the source's acceleration
                // service regenerates velocity before integrating the pose.
                assert_eq!(objects.get(owner).unwrap().base.position, expected);
            }
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
        // The next actor uses the SAME world stream, not a per-path seed.
        let mut second = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
        second.base.path = Some(authored_paths::RANDOMIZED_COLOR_PARTICLE);
        let second = objects.allocate(second).unwrap();
        for _ in 0..6 {
            expected_random.next_byte();
        }
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, second, &mut world(&mut random), 16),
            Ok(ControlStep::Movement)
        );
        assert_eq!(random, expected_random);
    }

    #[test]
    fn authored_local_jitter_returns_from_subroutine_then_runs_inverted_loop() {
        use super::super::{authored_paths, path_motion, Vector3};
        let (mut runtime, mut objects, owner, mut random) = setup();
        let initial = Vector3 {
            x: 100,
            y: -200,
            z: i16::MAX,
        };
        let velocity = Vector3 { x: 1, y: -2, z: 3 };
        {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::LOCAL_JITTER_SPRITE);
            actor.extension.relative_position = initial;
            actor.extension.path_state.motion.relative_coordinates = true;
            actor.extension.path_state.motion_phase = 0x5500;
            actor.base.velocity = velocity;
            actor.base.wait_timer = 97;
        }
        let mut expected_random = random;
        let mut jitter = || {
            let high = expected_random.next_byte();
            let low = expected_random.next_byte();
            (u16::from_be_bytes([high, low]) & 31) as i16 - 15
        };
        let mut expected = Vector3 {
            x: initial.x.wrapping_add(jitter()),
            y: initial.y.wrapping_add(jitter()),
            z: initial.z.wrapping_add(jitter()),
        };
        let catalog = authored_paths::catalog();
        for phase in 1..=3 {
            let outcome = runtime
                .enter_program(
                    &catalog,
                    &mut objects,
                    owner,
                    &mut PathWorld {
                        scene: ScenePathInputs::default(),
                        scenery_distance: None,
                        targeting_upgrade: None,
                        shield_recovery: None,
                        action_gate: None,
                        environment_plane_height: None,
                        projectile_trigger: None,
                        primary_pitch_recoil: None,
                        linked_effect_activity: None,
                        protection: None,
                        audio: None,
                        radio: None,
                        campaign: None,
                        guidance: None,
                        pickup_history: None,
                        control_style: None,
                        selected_occupancy_exempt: None,
                        occupancy: None,
                        surface_mode: None,
                        primary_player: None,
                        selected: None,
                        fixed_players: [None; 2],
                        primary_motion: None,
                        published_motion: None,
                        active_charge_threshold: None,
                        selected_charge: None,
                        primary_control: None,
                        primary_target: None,
                        active_node_flags: None,
                        selected_auxiliary: None,
                        selected_equipment: None,
                        selected_score: None,
                        countdown: None,
                        spawn_defaults: None,
                        random: &mut random,
                        animation_clock: 29,
                    },
                    16,
                )
                .unwrap();
            assert_eq!(
                outcome,
                if phase < 3 {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            assert_eq!(random, expected_random);
            assert!(!runtime.branch.invert_next);
            {
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.extension.relative_position, expected);
                assert_eq!(actor.base.position, Vector3::default());
                assert_eq!(actor.extension.path_state.motion_phase, 0x5500 | phase);
                assert_eq!(actor.extension.color_frame, (phase % 3) as u8);
                assert_eq!(actor.extension.animation_frame, 29);
                assert_eq!(actor.extension.texture_scroll_x, 252);
                assert_eq!(actor.base.wait_timer, 97);
            }
            if outcome == ControlStep::Movement {
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
                expected.x = expected.x.wrapping_add(velocity.x);
                expected.y = expected.y.wrapping_add(velocity.y);
                expected.z = expected.z.wrapping_add(velocity.z);
            }
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }

    #[test]
    fn immediate_loop_samples_mutations_and_preserves_wait_boundary() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![
            health(ByteOperation::Assign(ByteOperand::Literal(3)), cursor(0, 1)),
            health(ByteOperation::Decrement, cursor(0, 2)),
            Statement::Compare {
                condition: ActorCondition::NonzeroByte(ByteOperand::Actor(ByteField::Health)),
                taken: cursor(0, 1),
                next: cursor(0, 3),
            },
            Statement::Control(ControlCommand::WaitOne { next: cursor(0, 4) }),
            Statement::Control(ControlCommand::End),
        ]])
        .unwrap();
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8),
            Ok(ControlStep::Movement)
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.hit_points, 0);
        assert_eq!(actor.base.path, Some(cursor(0, 4)));
        assert!(!actor.base.flags.remove_after_tick);
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Ok(ControlStep::Ended)
        );
    }

    #[test]
    fn called_path_waits_then_resumes_saved_caller() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![
            vec![
                Statement::Control(ControlCommand::Call {
                    target: cursor(1, 0),
                    next: cursor(0, 1),
                }),
                Statement::Control(ControlCommand::End),
            ],
            vec![
                Statement::Wait {
                    duration: ByteOperand::Actor(ByteField::Health),
                    next: cursor(1, 1),
                },
                Statement::Control(ControlCommand::Return),
            ],
        ])
        .unwrap();
        objects.get_mut(owner).unwrap().base.hit_points = 2;
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 2),
            Ok(ControlStep::Movement)
        );
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, 1);
        // Change the live operand between invocations: retained/predecoded
        // duration=2 would yield again instead of completing this call.
        objects.get_mut(owner).unwrap().base.hit_points = 1;
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 3),
            Ok(ControlStep::Ended)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 1)));
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, 0);
    }

    #[test]
    fn callback_return_yields_to_batch_not_interrupted_main_program() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![
            vec![Statement::Control(ControlCommand::End)],
            vec![
                health(ByteOperation::Assign(ByteOperand::Literal(7)), cursor(1, 1)),
                Statement::Control(ControlCommand::Return),
            ],
        ])
        .unwrap();
        runtime
            .add_trigger(
                &mut objects,
                owner,
                Trigger {
                    path: cursor(1, 0),
                    kind: TriggerKind::Always,
                    timer: 0,
                },
            )
            .unwrap();
        assert!(runtime.begin_callbacks(&objects, owner).unwrap());
        assert_eq!(
            runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
            Ok(CallbackStep::Run(cursor(1, 0)))
        );
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 2),
            Ok(ControlStep::ResumeCallbacks)
        );
        assert_eq!(objects.get(owner).unwrap().base.hit_points, 7);
        assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
        assert_eq!(
            runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
            Ok(CallbackStep::Complete)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(0, 0)));
    }

    #[test]
    fn immediate_resume_does_not_reselect_player() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![Statement::Control(ControlCommand::WaitOne {
            next: cursor(0, 0),
        })]])
        .unwrap();
        runtime.enter(&objects, owner).unwrap();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Secondary;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Ok(ControlStep::Movement)
        );
        assert_eq!(runtime.selected_player(), PlayerTarget::Primary);
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Ok(ControlStep::Movement)
        );
        assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
    }

    #[test]
    fn missing_statements_and_runaway_paths_never_become_successful_ticks() {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let catalog = PathCatalog::new(vec![vec![
            health(ByteOperation::Increment, cursor(0, 1)),
            Statement::Control(ControlCommand::Jump {
                target: cursor(0, 0),
            }),
        ]])
        .unwrap();
        objects.get_mut(owner).unwrap().base.hit_points = 0;
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 3),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 1),
                executed: 3
            })
        );
        assert_eq!(objects.get(owner).unwrap().base.hit_points, 2);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0, 1),
                executed: 0
            })
        );
        objects.get_mut(owner).unwrap().base.path = Some(cursor(1, 0));
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 5),
            Err(ProgramError::MissingStatement(cursor(1, 0)))
        );
    }
}
