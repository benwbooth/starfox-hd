use super::campaign_major_objectives::{
    BATTLE_CARRIER_MISSION_SELECTION, ELADARD_MISSION_SELECTION, TITANIA_MISSION_SELECTION,
    TITANIA_SURFACE_SWITCH_COUNT,
};
use super::input::InputState;
use super::object::{Angle, ObjectId, ObjectStore};
use super::render::Camera;

pub const SELECTED_PILOT_COUNT: usize = 2;
pub const ROSTER_PILOT_COUNT: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomState {
    bytes: [u8; 4],
}

impl RandomState {
    pub const fn new(bytes: [u8; 4]) -> Self {
        Self { bytes }
    }

    pub const fn bytes(self) -> [u8; 4] {
        self.bytes
    }

    pub fn next_byte(&mut self) -> u8 {
        fn subtract_with_borrow(left: u8, right: u8, no_borrow: bool) -> (u8, bool) {
            let borrow = u16::from(!no_borrow);
            let subtrahend = u16::from(right) + borrow;
            (
                left.wrapping_sub(right).wrapping_sub(borrow as u8),
                u16::from(left) >= subtrahend,
            )
        }

        let original_first = self.bytes[0];
        let (mut value, mut no_borrow) = subtract_with_borrow(original_first, self.bytes[1], false);
        self.bytes[1] = value;
        (value, no_borrow) = subtract_with_borrow(value, self.bytes[2], no_borrow);
        self.bytes[2] = value;
        (value, no_borrow) = subtract_with_borrow(value, self.bytes[3], no_borrow);
        self.bytes[3] = value;
        (value, _) = subtract_with_borrow(value, original_first, no_borrow);
        self.bytes[0] = value;
        value
    }
}

impl Default for RandomState {
    fn default() -> Self {
        Self::new([0; 4])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pilot {
    Fox,
    Falco,
    Peppy,
    Slippy,
    Miyu,
    Fay,
}

/// The three craft configurations shared by the six selectable pilots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PilotCraftClass {
    FoxFalco,
    PeppySlippy,
    MiyuFay,
}

/// Pilot-dependent gameplay values decoded from the retail six-entry tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PilotCraftProfile {
    pub class: PilotCraftClass,
    pub maximum_shield: u8,
    pub charge_threshold: u8,
}

/// Pilot-dependent Walker jump response decoded from the retail six-entry
/// tables. Values remain ordinary signed motion units in the native port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkerMotionProfile {
    pub maximum_ascent_impulse: i16,
    pub initial_ascent_impulse: i16,
    pub held_ascent_step: i16,
    pub launch_ticks: u8,
    pub pose_extension_step: u16,
}

const FOX_FALCO_CRAFT_PROFILE: PilotCraftProfile = PilotCraftProfile {
    class: PilotCraftClass::FoxFalco,
    maximum_shield: 32,
    charge_threshold: 25,
};
const PEPPY_SLIPPY_CRAFT_PROFILE: PilotCraftProfile = PilotCraftProfile {
    class: PilotCraftClass::PeppySlippy,
    maximum_shield: 40,
    charge_threshold: 35,
};
const MIYU_FAY_CRAFT_PROFILE: PilotCraftProfile = PilotCraftProfile {
    class: PilotCraftClass::MiyuFay,
    maximum_shield: 24,
    charge_threshold: 10,
};

const FOX_WALKER_MOTION_PROFILE: WalkerMotionProfile = WalkerMotionProfile {
    maximum_ascent_impulse: -2_048,
    initial_ascent_impulse: -672,
    held_ascent_step: 192,
    launch_ticks: 8,
    pose_extension_step: 256,
};
const STANDARD_WALKER_MOTION_PROFILE: WalkerMotionProfile = WalkerMotionProfile {
    maximum_ascent_impulse: -2_048,
    initial_ascent_impulse: -512,
    held_ascent_step: 192,
    launch_ticks: 8,
    pose_extension_step: 192,
};
const MIYU_FAY_WALKER_MOTION_PROFILE: WalkerMotionProfile = WalkerMotionProfile {
    maximum_ascent_impulse: -2_048,
    initial_ascent_impulse: -672,
    held_ascent_step: 192,
    launch_ticks: 8,
    pose_extension_step: 256,
};

impl Pilot {
    pub const ALL: [Self; ROSTER_PILOT_COUNT] = [
        Self::Fox,
        Self::Falco,
        Self::Peppy,
        Self::Slippy,
        Self::Miyu,
        Self::Fay,
    ];

    pub const fn previous(self) -> Self {
        match self {
            Self::Fox => Self::Fay,
            Self::Falco => Self::Fox,
            Self::Peppy => Self::Falco,
            Self::Slippy => Self::Peppy,
            Self::Miyu => Self::Slippy,
            Self::Fay => Self::Miyu,
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Fox => Self::Falco,
            Self::Falco => Self::Peppy,
            Self::Peppy => Self::Slippy,
            Self::Slippy => Self::Miyu,
            Self::Miyu => Self::Fay,
            Self::Fay => Self::Fox,
        }
    }

    pub const fn craft_profile(self) -> PilotCraftProfile {
        match self {
            Self::Fox | Self::Falco => FOX_FALCO_CRAFT_PROFILE,
            Self::Peppy | Self::Slippy => PEPPY_SLIPPY_CRAFT_PROFILE,
            Self::Miyu | Self::Fay => MIYU_FAY_CRAFT_PROFILE,
        }
    }

    pub const fn walker_motion_profile(self) -> WalkerMotionProfile {
        match self {
            Self::Fox => FOX_WALKER_MOTION_PROFILE,
            Self::Falco | Self::Peppy | Self::Slippy => STANDARD_WALKER_MOTION_PROFILE,
            Self::Miyu | Self::Fay => MIYU_FAY_WALKER_MOTION_PROFILE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roster {
    pub available: [Pilot; ROSTER_PILOT_COUNT],
    pub selected: [Option<Pilot>; SELECTED_PILOT_COUNT],
}

impl Default for Roster {
    fn default() -> Self {
        Self {
            available: Pilot::ALL,
            selected: [None; SELECTED_PILOT_COUNT],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PilotSelectionPhase {
    Revealing,
    ChoosingPrimary,
    ChoosingWingmate,
    Ready,
    Launching,
}

impl Default for PilotSelectionPhase {
    fn default() -> Self {
        Self::Revealing
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PilotSelectionState {
    pub phase: PilotSelectionPhase,
    pub cursor: Pilot,
}

impl Default for Pilot {
    fn default() -> Self {
        Self::Fox
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Normal,
    Hard,
    Expert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitlePage {
    MainMenu,
    Difficulty,
}

impl Default for TitlePage {
    fn default() -> Self {
        Self::MainMenu
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleMenuItem {
    Mission,
    Records,
    SoundMode,
}

impl TitleMenuItem {
    pub const fn previous(self) -> Self {
        match self {
            Self::Mission => Self::SoundMode,
            Self::Records => Self::Mission,
            Self::SoundMode => Self::Records,
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Mission => Self::Records,
            Self::Records => Self::SoundMode,
            Self::SoundMode => Self::Mission,
        }
    }
}

impl Default for TitleMenuItem {
    fn default() -> Self {
        Self::Mission
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioOutput {
    Stereo,
    Mono,
}

impl AudioOutput {
    pub const fn toggled(self) -> Self {
        match self {
            Self::Stereo => Self::Mono,
            Self::Mono => Self::Stereo,
        }
    }
}

impl Default for AudioOutput {
    fn default() -> Self {
        Self::Stereo
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TitleState {
    pub page: TitlePage,
    pub menu_item: TitleMenuItem,
    pub audio_output: AudioOutput,
}

impl Default for Difficulty {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MissionId(u16);

impl MissionId {
    /// First sortie selected by the certified default retail campaign trace.
    pub const OPENING_SORTIE: Self = Self(6);

    /// Three-fighter defense reached after the missile interception. Retail
    /// selects the same space-combat arena as the opening engagement.
    pub const FIGHTER_INTERCEPT: Self = Self(6);

    /// Star Wolf duel reached after the three-fighter defense.
    pub const PIGMA_DUEL: Self = Self(7);

    /// Star Wolf duel at Astropolis reached after the Battle Carrier assault.
    /// Retail selects the same space-combat stage catalog entry as the first
    /// rival duel; the campaign visit identifies Leon and his presentation.
    pub const LEON_DUEL: Self = Self(7);

    /// Titania planetary-base assault, selected independently on the command map.
    pub const TITANIA_BASE: Self = Self(TITANIA_MISSION_SELECTION);

    /// Eladard planetary-base assault reached after the Pigma duel.
    pub const ELADARD_BASE: Self = Self(ELADARD_MISSION_SELECTION);

    /// Interior assault shared by the campaign's two Battle Carriers.
    pub const BATTLE_CARRIER: Self = Self(BATTLE_CARRIER_MISSION_SELECTION);

    /// Timed defensive sortie against the three missiles approaching Corneria.
    pub const MISSILE_INTERCEPTION: Self = Self(7);

    /// All-range battle with Mirage Dragon reached from the post-Leon map.
    pub const MIRAGE_DRAGON: Self = Self(9);

    /// The last recurring Wolfen pursuer and the separately allocated final
    /// blockade both use the retail rival-combat catalog entry.
    pub const FINAL_PURSUER: Self = Self(7);
    pub const WOLF_BLOCKADE: Self = Self(7);

    /// Interior assault on Astropolis after the final blockade retires.
    pub const ASTROPOLIS: Self = Self(11);

    pub const fn from_catalog_index(index: u16) -> Self {
        Self(index)
    }

    pub const fn catalog_index(self) -> u16 {
        self.0
    }
}

/// The next certified campaign visit. This replaces the old numeric sortie
/// counter with a semantic route state; strategic objectives below remain
/// independent so the randomized late campaign does not become another
/// hard-coded linear sequence.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CampaignRouteStep {
    #[default]
    OpeningEngagement,
    Reengagement,
    MissileInterception,
    FighterIntercept,
    PigmaDuel,
    EladardBase,
    FirstBattleCarrier,
    LeonDuel,
    MirageDragon,
    StrategicPressure,
    WolfBlockade,
    AstropolisAssault,
}

impl CampaignRouteStep {
    pub const fn after_completion(self) -> Self {
        match self {
            Self::OpeningEngagement => Self::Reengagement,
            Self::Reengagement => Self::MissileInterception,
            Self::MissileInterception => Self::FighterIntercept,
            Self::FighterIntercept => Self::PigmaDuel,
            Self::PigmaDuel => Self::EladardBase,
            Self::EladardBase => Self::FirstBattleCarrier,
            Self::FirstBattleCarrier => Self::LeonDuel,
            Self::LeonDuel => Self::MirageDragon,
            Self::MirageDragon | Self::StrategicPressure => Self::StrategicPressure,
            Self::WolfBlockade => Self::AstropolisAssault,
            Self::AstropolisAssault => Self::AstropolisAssault,
        }
    }

    const fn completed_certified_visits(self) -> u16 {
        match self {
            Self::OpeningEngagement => 0,
            Self::Reengagement => 1,
            Self::MissileInterception => 2,
            Self::FighterIntercept => 3,
            Self::PigmaDuel => 4,
            Self::EladardBase => 5,
            Self::FirstBattleCarrier => 6,
            Self::LeonDuel => 7,
            Self::MirageDragon => 8,
            Self::StrategicPressure => 9,
            Self::WolfBlockade => 12,
            Self::AstropolisAssault => 13,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetObjectiveStatus {
    Occupied,
    Rescued,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierObjectiveStatus {
    Operational,
    Destroyed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WolfBlockadeStatus {
    Unavailable,
    Active,
    Defeated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AstropolisStatus {
    Locked,
    BlockedByWolf,
    Vulnerable,
    Assaulted,
}

/// A semantic count used by the strategic simulation. The source game keeps
/// several bookkeeping totals, but the shipping port exposes only their game
/// meaning and never their storage addresses.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StrategicThreatCount(u8);

impl StrategicThreatCount {
    pub const NONE: Self = Self(0);
    pub const INITIAL_MISSILES: Self = Self(3);

    pub const fn new(count: u8) -> Self {
        Self(count)
    }

    pub const fn remaining(self) -> u8 {
        self.0
    }

    pub const fn is_clear(self) -> bool {
        self.0 == 0
    }

    pub fn remove_one(&mut self) {
        self.0 = self.0.saturating_sub(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignObjectives {
    pub eladard: PlanetObjectiveStatus,
    pub titania: PlanetObjectiveStatus,
    pub first_carrier: CarrierObjectiveStatus,
    pub second_carrier: CarrierObjectiveStatus,
    pub missiles: StrategicThreatCount,
    pub live_attackers: StrategicThreatCount,
    pub wolf_blockade: WolfBlockadeStatus,
    pub astropolis: AstropolisStatus,
}

impl Default for CampaignObjectives {
    fn default() -> Self {
        Self {
            eladard: PlanetObjectiveStatus::Occupied,
            titania: PlanetObjectiveStatus::Occupied,
            first_carrier: CarrierObjectiveStatus::Operational,
            second_carrier: CarrierObjectiveStatus::Operational,
            missiles: StrategicThreatCount::INITIAL_MISSILES,
            live_attackers: StrategicThreatCount::NONE,
            wolf_blockade: WolfBlockadeStatus::Unavailable,
            astropolis: AstropolisStatus::Locked,
        }
    }
}

impl CampaignObjectives {
    pub const fn major_objectives_complete(self) -> bool {
        matches!(self.eladard, PlanetObjectiveStatus::Rescued)
            && matches!(self.titania, PlanetObjectiveStatus::Rescued)
            && matches!(self.first_carrier, CarrierObjectiveStatus::Destroyed)
            && matches!(self.second_carrier, CarrierObjectiveStatus::Destroyed)
    }

    pub const fn final_gate_clear(self) -> bool {
        self.major_objectives_complete()
            && self.missiles.is_clear()
            && self.live_attackers.is_clear()
    }

    pub fn refresh_final_access(&mut self) {
        if self.final_gate_clear() && matches!(self.wolf_blockade, WolfBlockadeStatus::Unavailable)
        {
            self.wolf_blockade = WolfBlockadeStatus::Active;
            self.astropolis = AstropolisStatus::BlockedByWolf;
        }
    }

    pub fn record_wolf_defeated(&mut self) {
        if matches!(self.wolf_blockade, WolfBlockadeStatus::Active) {
            self.wolf_blockade = WolfBlockadeStatus::Defeated;
            self.astropolis = AstropolisStatus::Vulnerable;
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CampaignState {
    pub elapsed_frames: u64,
    /// Damage shown by the retail strategic-map HUD, in whole percent.
    pub corneria_damage_percent: u8,
    /// Typed campaign pressure affecting Corneria after the eighth certified
    /// sortie. It advances while the command map is live and pauses during an
    /// action mission, just like the retail strategic simulation.
    pub corneria_defense: CorneriaDefenseState,
    pub difficulty: Difficulty,
    pub active_threats: Vec<ObjectId>,
    pub route_step: CampaignRouteStep,
    pub objectives: CampaignObjectives,
}

impl CampaignState {
    /// Derived presentation/automation progress. The runtime stores semantic
    /// route and objective state; it does not maintain a second numeric sortie
    /// counter that can drift out of sync.
    pub const fn completed_campaign_visits(&self) -> u16 {
        let completed = self.route_step.completed_certified_visits();
        match self.route_step {
            CampaignRouteStep::StrategicPressure => {
                completed
                    + match self.objectives.titania {
                        PlanetObjectiveStatus::Occupied => 0,
                        PlanetObjectiveStatus::Rescued => 1,
                    }
                    + match self.objectives.second_carrier {
                        CarrierObjectiveStatus::Operational => 0,
                        CarrierObjectiveStatus::Destroyed => 1,
                    }
            }
            CampaignRouteStep::AstropolisAssault => {
                completed
                    + match self.objectives.astropolis {
                        AstropolisStatus::Assaulted => 1,
                        AstropolisStatus::Locked
                        | AstropolisStatus::BlockedByWolf
                        | AstropolisStatus::Vulnerable => 0,
                    }
            }
            CampaignRouteStep::OpeningEngagement
            | CampaignRouteStep::Reengagement
            | CampaignRouteStep::MissileInterception
            | CampaignRouteStep::FighterIntercept
            | CampaignRouteStep::PigmaDuel
            | CampaignRouteStep::EladardBase
            | CampaignRouteStep::FirstBattleCarrier
            | CampaignRouteStep::LeonDuel
            | CampaignRouteStep::MirageDragon
            | CampaignRouteStep::WolfBlockade => completed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorneriaDefensePhase {
    Inactive,
    PostLeonPressure,
    PlanetCannonWarning,
    PlanetCannonCinematic,
    CarrierApproach,
    CarrierWarning,
    CarrierCinematic,
    Complete,
}

impl Default for CorneriaDefensePhase {
    fn default() -> Self {
        Self::Inactive
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CorneriaDefenseState {
    pub phase: CorneriaDefensePhase,
    /// Elapsed presentation time in retail frames. The native game advances
    /// this typed timeline at its four-retail-frames-per-tick cadence.
    pub elapsed_retail_frames: u16,
    pub damage_steps_applied: u8,
}

impl CorneriaDefenseState {
    pub const fn post_leon() -> Self {
        Self {
            phase: CorneriaDefensePhase::PostLeonPressure,
            elapsed_retail_frames: 0,
            damage_steps_applied: 0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapPoint {
    pub x: i16,
    pub y: i16,
}

pub const STRATEGIC_MAP_ACTOR_CAPACITY: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategicMapActorKind {
    NorthernInstallation,
    SouthernInstallation,
    EnemyCarrier,
    EnemyFormation,
    EasternInterceptor,
    PatrolShip,
    MissileTrail,
    Missile,
    AttackingFighter,
    RivalFighter,
    FighterProjectile,
    UnknownSignal,
    DefensePlatform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategicMapAppearance {
    OpeningAssault,
    EscalatedAssault,
    PostInterception,
    PostFighterIntercept,
    PostPigma,
    PostEladard,
    PostCarrier,
    PostLeon,
    PostMirage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategicMapActor {
    pub kind: StrategicMapActorKind,
    pub appearance: StrategicMapAppearance,
    pub position: MapPoint,
}

impl StrategicMapActor {
    pub const fn new(
        kind: StrategicMapActorKind,
        appearance: StrategicMapAppearance,
        x: i16,
        y: i16,
    ) -> Self {
        Self {
            kind,
            appearance,
            position: MapPoint { x, y },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategicMapTutorialPage {
    Movement,
    Engagement,
}

impl Default for StrategicMapTutorialPage {
    fn default() -> Self {
        Self::Movement
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategicMapPhase {
    OpeningOverview,
    Tutorial(StrategicMapTutorialPage),
    Planning,
    Traveling,
}

impl Default for StrategicMapPhase {
    fn default() -> Self {
        Self::OpeningOverview
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StrategicMapState {
    pub phase: StrategicMapPhase,
    pub primary_player: Option<ObjectId>,
    pub selected_target: Option<ObjectId>,
    pub player_position: super::object::Vector3,
    pub target_position: super::object::Vector3,
    pub formation_yaw: super::object::Angle,
    pub marker_phase: u8,
    pub player_map_position: MapPoint,
    pub destination: MapPoint,
    /// Recommended live threat for the hands-off verification pilot. This is
    /// ordinary campaign state and is also useful to player-facing route hints.
    pub recommended_destination: MapPoint,
    /// Semantic combat choice selected on the live strategic map. This keeps
    /// campaign routing independent from source object slots or addresses.
    pub selected_encounter: Option<StrategicEncounter>,
    pub travel_origin: MapPoint,
    pub travel_ticks_remaining: u16,
    pub travel_total_ticks: u16,
    pub travel_origin_damage_percent: u8,
    pub travel_destination_damage_percent: u8,
    pub actors: [Option<StrategicMapActor>; STRATEGIC_MAP_ACTOR_CAPACITY],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategicEncounter {
    TitaniaBase,
    SecondBattleCarrier,
    RecurringAttackers,
    LeonPressure,
    FinalPursuer,
    WolfBlockade,
    AstropolisAssault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionVisit {
    OpeningEngagement,
    Reengagement,
    MissileInterception,
    FighterIntercept,
    PigmaDuel,
    EladardBase,
    TitaniaBase,
    FirstBattleCarrier,
    SecondBattleCarrier,
    LeonDuel,
    MirageDragon,
    RecurringAttackers,
    LeonPressure,
    FinalPursuer,
    WolfBlockade,
    AstropolisAssault,
}

impl Default for MissionVisit {
    fn default() -> Self {
        Self::OpeningEngagement
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PlayerBlasterState {
    #[default]
    Ready,
    Holding {
        held_ticks: u8,
        charge_orb: Option<ObjectId>,
    },
}

/// Direction of an in-progress Arwing/Walker transformation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerCraftTransformationDirection {
    ToWalker,
    ToFlight,
}

/// Semantic transformation progress. The retail animation advances in source
/// presentation frames while the native game ticks at a four-frame cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerCraftTransformation {
    pub direction: PlayerCraftTransformationDirection,
    pub elapsed_retail_frames: u8,
}

/// The player's current gameplay form. Transitional geometry is derived from
/// the typed direction and elapsed time; no encoded shape or source state is
/// stored here.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PlayerCraftForm {
    #[default]
    Flight,
    Transforming(PlayerCraftTransformation),
    Walker,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MissionState {
    pub active: bool,
    pub phase: MissionPhase,
    pub mission: Option<MissionId>,
    pub visit: MissionVisit,
    pub primary_player: Option<ObjectId>,
    pub wingmate: Option<ObjectId>,
    pub score: u32,
    pub objects_destroyed: u32,
    pub item_count: u8,
    pub elapsed_time_tenths: u16,
    pub player_blaster: PlayerBlasterState,
    /// Persistent analog flight response recovered from the retail player
    /// object. These are ordinary gameplay values, separate from the visible
    /// craft angles so steering remains smooth while the craft leans.
    pub player_flight: PlayerFlightState,
    pub player_walker: PlayerWalkerState,
    pub player_craft_form: PlayerCraftForm,
    /// Becomes true when steering leaves the certified neutral path.
    /// The port then continues from that typed pose with native flight rules.
    pub departed_certified_neutral_path: bool,
    pub camera_follow_offset: super::object::Vector3,
    pub eladard: EladardMissionState,
    pub titania: TitaniaMissionState,
    pub carrier_assault: CarrierAssaultState,
    pub astropolis: AstropolisMissionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AstropolisPhase {
    ExteriorApproach,
    BaseEntry,
    InteriorCorridor,
    SecurityTurret,
    BranchCorridor,
    CoreSpikes,
    ExposedCube,
    AndrossMask,
    FinalCore,
    MaskReforming,
    CoreDestruction,
    Escape,
}

impl Default for AstropolisPhase {
    fn default() -> Self {
        Self::ExteriorApproach
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AstropolisBranch {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AstropolisEye {
    Left,
    Right,
}

/// One of the four armor spikes around Astropolis's first central core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AstropolisCoreSpike {
    pub durability: u16,
}

impl AstropolisCoreSpike {
    pub const fn is_active(self) -> bool {
        self.durability != 0
    }

    fn damage(&mut self, amount: u16) {
        self.durability = self.durability.saturating_sub(amount);
    }
}

/// The two independently vulnerable eyes in the Andross mask encounter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AstropolisEyes {
    pub left_durability: u16,
    pub right_durability: u16,
}

impl AstropolisEyes {
    pub const fn are_destroyed(self) -> bool {
        self.left_durability == 0 && self.right_durability == 0
    }

    fn damage(&mut self, eye: AstropolisEye, amount: u16) {
        let durability = match eye {
            AstropolisEye::Left => &mut self.left_durability,
            AstropolisEye::Right => &mut self.right_durability,
        };
        *durability = durability.saturating_sub(amount);
    }
}

/// Typed state for the complete final-base objective chain. Each field is a
/// game concept with the same independent lifetime as its retail counterpart:
/// the security turret, four armor spikes, intermediate cube, two eyes, and
/// timed final core. No byte-addressed source state is retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AstropolisMissionState {
    pub phase: AstropolisPhase,
    pub phase_started_retail_frame: u16,
    pub corridor_progress: u16,
    pub security_turret_durability: u16,
    pub security_door_open: bool,
    pub branch: Option<AstropolisBranch>,
    pub core_spikes: [AstropolisCoreSpike; super::astropolis_assault::CORE_SPIKE_COUNT],
    pub exposed_cube_durability: u16,
    pub eyes: AstropolisEyes,
    pub final_core_durability: u16,
    pub core_exposure_retail_frames_remaining: u16,
    pub core_rebuild_count: u16,
}

impl Default for AstropolisMissionState {
    fn default() -> Self {
        use super::astropolis_assault::{
            CORE_EXPOSURE_RETAIL_FRAMES, CORE_SPIKE_DURABILITY, EXPOSED_CUBE_DURABILITY,
            FINAL_CORE_DURABILITY, MASK_EYE_DURABILITY, SECURITY_TURRET_DURABILITY,
        };

        Self {
            phase: AstropolisPhase::ExteriorApproach,
            phase_started_retail_frame: 0,
            corridor_progress: 0,
            security_turret_durability: SECURITY_TURRET_DURABILITY,
            security_door_open: false,
            branch: None,
            core_spikes: [AstropolisCoreSpike {
                durability: CORE_SPIKE_DURABILITY,
            }; super::astropolis_assault::CORE_SPIKE_COUNT],
            exposed_cube_durability: EXPOSED_CUBE_DURABILITY,
            eyes: AstropolisEyes {
                left_durability: MASK_EYE_DURABILITY,
                right_durability: MASK_EYE_DURABILITY,
            },
            final_core_durability: FINAL_CORE_DURABILITY,
            core_exposure_retail_frames_remaining: CORE_EXPOSURE_RETAIL_FRAMES,
            core_rebuild_count: 0,
        }
    }
}

impl AstropolisMissionState {
    fn enter_phase(&mut self, phase: AstropolisPhase, retail_frame: u16) {
        self.phase = phase;
        self.phase_started_retail_frame = retail_frame;
    }

    pub fn enter_security_room(&mut self, retail_frame: u16) {
        self.enter_phase(AstropolisPhase::SecurityTurret, retail_frame);
    }

    pub fn damage_security_turret(&mut self, amount: u16, retail_frame: u16) -> bool {
        if self.phase != AstropolisPhase::SecurityTurret {
            return false;
        }
        self.security_turret_durability = self.security_turret_durability.saturating_sub(amount);
        if self.security_turret_durability == 0 {
            self.security_door_open = true;
            self.enter_phase(AstropolisPhase::BranchCorridor, retail_frame);
            true
        } else {
            false
        }
    }

    pub fn choose_branch(&mut self, branch: AstropolisBranch) -> bool {
        if self.phase != AstropolisPhase::BranchCorridor {
            return false;
        }
        self.branch = Some(branch);
        true
    }

    pub fn enter_core_chamber(&mut self, retail_frame: u16) -> bool {
        if self.phase != AstropolisPhase::BranchCorridor || self.branch.is_none() {
            return false;
        }
        self.enter_phase(AstropolisPhase::CoreSpikes, retail_frame);
        true
    }

    pub fn damage_core_spike(&mut self, index: usize, amount: u16, retail_frame: u16) -> bool {
        if self.phase != AstropolisPhase::CoreSpikes {
            return false;
        }
        let Some(spike) = self.core_spikes.get_mut(index) else {
            return false;
        };
        spike.damage(amount);
        if self.core_spikes.iter().all(|spike| !spike.is_active()) {
            self.enter_phase(AstropolisPhase::ExposedCube, retail_frame);
            true
        } else {
            false
        }
    }

    pub fn damage_exposed_cube(&mut self, amount: u16, retail_frame: u16) -> bool {
        if self.phase != AstropolisPhase::ExposedCube {
            return false;
        }
        self.exposed_cube_durability = self.exposed_cube_durability.saturating_sub(amount);
        if self.exposed_cube_durability == 0 {
            self.enter_phase(AstropolisPhase::AndrossMask, retail_frame);
            true
        } else {
            false
        }
    }

    pub fn damage_eye(&mut self, eye: AstropolisEye, amount: u16, retail_frame: u16) -> bool {
        if self.phase != AstropolisPhase::AndrossMask {
            return false;
        }
        self.eyes.damage(eye, amount);
        if self.eyes.are_destroyed() {
            self.final_core_durability = super::astropolis_assault::FINAL_CORE_DURABILITY;
            self.core_exposure_retail_frames_remaining =
                super::astropolis_assault::CORE_EXPOSURE_RETAIL_FRAMES;
            self.enter_phase(AstropolisPhase::FinalCore, retail_frame);
            true
        } else {
            false
        }
    }

    pub fn damage_final_core(&mut self, amount: u16, retail_frame: u16) -> bool {
        if self.phase != AstropolisPhase::FinalCore {
            return false;
        }
        self.final_core_durability = self.final_core_durability.saturating_sub(amount);
        if self.final_core_durability == 0 {
            self.core_exposure_retail_frames_remaining = 0;
            self.enter_phase(AstropolisPhase::CoreDestruction, retail_frame);
            true
        } else {
            false
        }
    }

    /// Advance only the vulnerable-core window. Returns true when the retail
    /// timeout closes the core and starts rebuilding the mask.
    pub fn advance_core_exposure(&mut self, retail_frames: u16, current_retail_frame: u16) -> bool {
        if self.phase != AstropolisPhase::FinalCore || self.final_core_durability == 0 {
            return false;
        }
        self.core_exposure_retail_frames_remaining = self
            .core_exposure_retail_frames_remaining
            .saturating_sub(retail_frames);
        if self.core_exposure_retail_frames_remaining == 0 {
            self.eyes = AstropolisEyes {
                left_durability: super::astropolis_assault::MASK_EYE_DURABILITY,
                right_durability: super::astropolis_assault::MASK_EYE_DURABILITY,
            };
            self.final_core_durability = super::astropolis_assault::FINAL_CORE_DURABILITY;
            self.core_rebuild_count = self.core_rebuild_count.saturating_add(1);
            self.enter_phase(AstropolisPhase::MaskReforming, current_retail_frame);
            true
        } else {
            false
        }
    }

    pub fn complete_mask_reformation(&mut self, retail_frame: u16) -> bool {
        if self.phase != AstropolisPhase::MaskReforming {
            return false;
        }
        self.enter_phase(AstropolisPhase::AndrossMask, retail_frame);
        true
    }

    pub fn begin_escape(&mut self, retail_frame: u16) -> bool {
        if self.phase != AstropolisPhase::CoreDestruction {
            return false;
        }
        self.enter_phase(AstropolisPhase::Escape, retail_frame);
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EladardPhase {
    SurfaceApproach,
    SurfaceBarriers,
    BaseEntrance,
    WalkerTransformation,
    PlatformSwitch,
    WallSpider,
    Generator,
    BaseDestruction,
    ReturnFlight,
}

impl Default for EladardPhase {
    fn default() -> Self {
        Self::SurfaceApproach
    }
}

/// Typed objective state for the Eladard base assault. These fields model the
/// mission concepts directly; no source-machine memory window is retained.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EladardMissionState {
    pub phase: EladardPhase,
    pub phase_started_retail_frame: u16,
    pub surface_barriers_remaining: u8,
    pub platform_switch_pressed: bool,
    pub wall_spider_hit_points: u8,
    pub generator_active: bool,
    pub generator_hit_points: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitaniaSurfaceSwitchStatus {
    Active,
    Disabled,
}

impl Default for TitaniaSurfaceSwitchStatus {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitaniaReactorStatus {
    Shielded,
    Exposed,
    Destroyed,
}

impl Default for TitaniaReactorStatus {
    fn default() -> Self {
        Self::Shielded
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitaniaPhase {
    SurfaceApproach,
    SurfaceSwitches,
    BaseEntry,
    Interior,
    Reactor,
    ReturnFlight,
}

impl Default for TitaniaPhase {
    fn default() -> Self {
        Self::SurfaceApproach
    }
}

/// Typed objective state for Titania. The two exterior switches and reactor
/// are the retail mission concepts; the port does not expose their original
/// storage locations or retain a byte-addressed work area.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TitaniaMissionState {
    pub phase: TitaniaPhase,
    pub phase_started_retail_frame: u16,
    pub surface_switches: [TitaniaSurfaceSwitchStatus; TITANIA_SURFACE_SWITCH_COUNT],
    pub reactor: TitaniaReactorStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierAssaultPhase {
    ExteriorApproach,
    InteriorCorridor,
    ReactorApproach,
    ReactorCombat,
    CoreDestruction,
    ReturnFlight,
}

impl Default for CarrierAssaultPhase {
    fn default() -> Self {
        Self::ExteriorApproach
    }
}

/// One of the two rotating armor panels protecting a Battle Carrier's energy
/// core. Integrity follows the retail mission's ordinary combat scale: each
/// effective laser hit removes one point and the panel breaks at 90.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarrierReactorPanel {
    pub integrity: u8,
    pub active: bool,
}

impl Default for CarrierReactorPanel {
    fn default() -> Self {
        Self {
            integrity: 100,
            active: true,
        }
    }
}

/// Typed objective state for the Battle Carrier assault. It models the
/// exterior approach, corridor, Walker transformation, and two reactor panels
/// directly; the shipping game does not retain a byte-addressed memory image.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CarrierAssaultState {
    pub phase: CarrierAssaultPhase,
    pub phase_started_retail_frame: u16,
    pub corridor_progress: u16,
    pub reactor_room_open: bool,
    pub port_panel: CarrierReactorPanel,
    pub starboard_panel: CarrierReactorPanel,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerFlightState {
    pub pitch_accumulator: i16,
    pub yaw_accumulator: i16,
    pub pitch_lean: i8,
}

/// The active retail Walker jump controller plus its local vertical motion.
/// `surface_height + height_offset` is the native world height; the remaining
/// fields preserve the one-tick ascent pipeline and the retail fall curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkerJumpMotion {
    pub launch_ticks_remaining: u8,
    pub ascent_impulse: i16,
    pub pose_extension: u16,
    pub motion_ticks_elapsed: u8,
    pub height_offset: i16,
    pub fall_velocity: i16,
    pub ascent_velocity: i16,
    pub surface_height: i16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WalkerJumpState {
    #[default]
    Grounded,
    Active(WalkerJumpMotion),
}

/// Persistent Walker control state recovered from the retail player object.
/// Heading, turn spring, turn velocity, and jump progression are represented
/// directly; there is no byte-addressed or segmented memory backing them.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerWalkerState {
    pub heading_offset: Option<Angle>,
    pub turn_spring: i16,
    pub turn_velocity: i8,
    pub jump: WalkerJumpState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionPhase {
    Loading,
    EntryCinematic,
    Active,
    ReturningToStrategicMap,
}

impl Default for MissionPhase {
    fn default() -> Self {
        Self::Loading
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroPhase {
    Boot,
    ArgonautLogo,
    NintendoLogo,
    Formation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Intro(IntroPhase),
    Title,
    Records,
    Briefing,
    StrategicMap,
    PilotSelection,
    Mission,
    Results,
    Ending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndingPhase {
    EscapeFlash,
    Credits,
    EndScreen,
}

impl Default for EndingPhase {
    fn default() -> Self {
        Self::EscapeFlash
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EndingState {
    pub phase: EndingPhase,
    pub retail_frame: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    pub frame: u64,
    pub mode_frame: u32,
    pub mode: GameMode,
    pub title: TitleState,
    pub roster: Roster,
    pub campaign: CampaignState,
    pub strategic_map: StrategicMapState,
    pub pilot_selection: PilotSelectionState,
    pub mission: MissionState,
    pub ending: EndingState,
    pub objects: ObjectStore,
    pub camera: Camera,
    pub input: InputState,
    pub random: RandomState,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            frame: 0,
            mode_frame: 0,
            mode: GameMode::Intro(IntroPhase::Boot),
            title: TitleState::default(),
            roster: Roster::default(),
            campaign: CampaignState::default(),
            strategic_map: StrategicMapState::default(),
            pilot_selection: PilotSelectionState::default(),
            mission: MissionState::default(),
            ending: EndingState::default(),
            objects: ObjectStore::new(),
            camera: Camera::default(),
            input: InputState::default(),
            random: RandomState::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn astropolis_objectives_keep_independent_typed_durability() {
        let mut mission = AstropolisMissionState::default();
        mission.enter_security_room(10);
        assert!(mission.damage_security_turret(
            super::super::astropolis_assault::SECURITY_TURRET_DURABILITY,
            20,
        ));
        assert!(mission.security_door_open);
        assert!(mission.choose_branch(AstropolisBranch::Left));
        assert!(mission.enter_core_chamber(30));

        for index in 0..super::super::astropolis_assault::CORE_SPIKE_COUNT {
            let opened = mission.damage_core_spike(
                index,
                super::super::astropolis_assault::CORE_SPIKE_DURABILITY,
                40 + index as u16,
            );
            assert_eq!(
                opened,
                index + 1 == super::super::astropolis_assault::CORE_SPIKE_COUNT
            );
        }
        assert_eq!(mission.phase, AstropolisPhase::ExposedCube);
        assert!(mission.damage_exposed_cube(
            super::super::astropolis_assault::EXPOSED_CUBE_DURABILITY,
            50,
        ));
        assert_eq!(mission.phase, AstropolisPhase::AndrossMask);
    }

    #[test]
    fn astropolis_core_timeout_rebuilds_both_eyes_at_full_durability() {
        let mut mission = AstropolisMissionState {
            phase: AstropolisPhase::AndrossMask,
            ..AstropolisMissionState::default()
        };
        assert!(!mission.damage_eye(
            AstropolisEye::Left,
            super::super::astropolis_assault::MASK_EYE_DURABILITY,
            10,
        ));
        assert!(mission.damage_eye(
            AstropolisEye::Right,
            super::super::astropolis_assault::MASK_EYE_DURABILITY,
            20,
        ));
        assert_eq!(mission.phase, AstropolisPhase::FinalCore);
        assert_eq!(
            mission.final_core_durability,
            super::super::astropolis_assault::FINAL_CORE_DURABILITY
        );

        assert!(mission.advance_core_exposure(
            super::super::astropolis_assault::CORE_EXPOSURE_RETAIL_FRAMES,
            30,
        ));
        assert_eq!(mission.phase, AstropolisPhase::MaskReforming);
        assert_eq!(mission.core_rebuild_count, 1);
        assert_eq!(
            mission.eyes,
            AstropolisEyes {
                left_durability: super::super::astropolis_assault::MASK_EYE_DURABILITY,
                right_durability: super::super::astropolis_assault::MASK_EYE_DURABILITY,
            }
        );
        assert!(mission.complete_mask_reformation(40));
        assert_eq!(mission.phase, AstropolisPhase::AndrossMask);
    }

    #[test]
    fn astropolis_core_death_cannot_take_the_timeout_branch() {
        let mut mission = AstropolisMissionState {
            phase: AstropolisPhase::FinalCore,
            ..AstropolisMissionState::default()
        };
        assert!(
            mission.damage_final_core(super::super::astropolis_assault::FINAL_CORE_DURABILITY, 10,)
        );
        assert_eq!(mission.phase, AstropolisPhase::CoreDestruction);
        assert_eq!(mission.core_exposure_retail_frames_remaining, 0);
        assert!(!mission.advance_core_exposure(
            super::super::astropolis_assault::CORE_EXPOSURE_RETAIL_FRAMES,
            20,
        ));
        assert_eq!(mission.core_rebuild_count, 0);
        assert!(mission.begin_escape(30));
        assert_eq!(mission.phase, AstropolisPhase::Escape);
    }

    #[test]
    fn random_state_matches_the_certified_four_byte_sequence() {
        const INITIAL: [u8; 4] = [17, 40, 233, 155];
        const EXPECTED_VALUE: u8 = 81;
        const EXPECTED_STATE: [u8; 4] = [81, 232, 254, 98];

        let mut random = RandomState::new(INITIAL);
        assert_eq!(random.next_byte(), EXPECTED_VALUE);
        assert_eq!(random.bytes(), EXPECTED_STATE);
    }

    #[test]
    fn astropolis_requires_every_typed_objective_then_the_wolf_blockade() {
        const ONE_LIVE_ATTACKER: StrategicThreatCount = StrategicThreatCount::new(1);

        let mut objectives = CampaignObjectives {
            eladard: PlanetObjectiveStatus::Rescued,
            titania: PlanetObjectiveStatus::Rescued,
            first_carrier: CarrierObjectiveStatus::Destroyed,
            second_carrier: CarrierObjectiveStatus::Destroyed,
            missiles: StrategicThreatCount::NONE,
            live_attackers: ONE_LIVE_ATTACKER,
            ..CampaignObjectives::default()
        };

        objectives.refresh_final_access();
        assert_eq!(objectives.wolf_blockade, WolfBlockadeStatus::Unavailable);
        assert_eq!(objectives.astropolis, AstropolisStatus::Locked);

        objectives.live_attackers.remove_one();
        objectives.refresh_final_access();
        assert_eq!(objectives.wolf_blockade, WolfBlockadeStatus::Active);
        assert_eq!(objectives.astropolis, AstropolisStatus::BlockedByWolf);

        objectives.record_wolf_defeated();
        assert_eq!(objectives.wolf_blockade, WolfBlockadeStatus::Defeated);
        assert_eq!(objectives.astropolis, AstropolisStatus::Vulnerable);
    }
}
