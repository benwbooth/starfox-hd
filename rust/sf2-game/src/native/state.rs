use super::campaign_major_objectives::TITANIA_SURFACE_SWITCH_COUNT;
use super::campaign_world_assignments::{
    CampaignWorld, MAX_OCCUPIED_WORLD_COUNT, NORMAL_CAMPAIGN_ASSIGNMENT_COUNT,
    NORMAL_CAMPAIGN_WORLD_ASSIGNMENTS, THREE_WORLD_CAMPAIGN_ASSIGNMENTS,
    THREE_WORLD_CAMPAIGN_ASSIGNMENT_COUNT,
};
use super::input::InputState;
use super::object::{Angle, ObjectId, ObjectStore, Vector3};
use super::render::Camera;

pub const SELECTED_PILOT_COUNT: usize = 2;
pub const ROSTER_PILOT_COUNT: usize = 6;
/// Two scripted hostile shots, the player weapon, and a mission-radio cue can
/// begin on the same presentation boundary.
pub const SOUND_EVENT_CAPACITY: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundEvent {
    RapidLaser,
    ChargedLaser,
    HostileLaser,
    RadioMessageOpen,
    RadioMessageClose,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ChargeSound {
    #[default]
    Silent,
    Building,
    Ready,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AudioState {
    pending_events: [Option<SoundEvent>; SOUND_EVENT_CAPACITY],
    spatial_listener_yaw: Option<Angle>,
}

impl AudioState {
    pub fn begin_tick(&mut self) {
        self.pending_events.fill(None);
    }

    pub fn queue(&mut self, event: SoundEvent) {
        if let Some(slot) = self.pending_events.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(event);
        }
    }

    pub fn take_events(&mut self) -> [Option<SoundEvent>; SOUND_EVENT_CAPACITY] {
        std::mem::take(&mut self.pending_events)
    }

    pub const fn spatial_listener_yaw(&self) -> Option<Angle> {
        self.spatial_listener_yaw
    }

    pub fn set_spatial_listener_yaw(&mut self, yaw: Angle) {
        self.spatial_listener_yaw = Some(yaw);
    }

    pub fn reset_spatial_listener(&mut self) {
        self.spatial_listener_yaw = None;
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PilotSelectionCursor {
    Pilot(Pilot),
    Control,
}

impl PilotSelectionCursor {
    pub const fn previous(self) -> Self {
        match self {
            Self::Pilot(Pilot::Fox) => Self::Control,
            Self::Pilot(pilot) => Self::Pilot(pilot.previous()),
            Self::Control => Self::Pilot(Pilot::Fay),
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Pilot(Pilot::Fay) => Self::Control,
            Self::Pilot(pilot) => Self::Pilot(pilot.next()),
            Self::Control => Self::Pilot(Pilot::Fox),
        }
    }

    pub const fn pilot(self) -> Option<Pilot> {
        match self {
            Self::Pilot(pilot) => Some(pilot),
            Self::Control => None,
        }
    }
}

impl Default for PilotSelectionCursor {
    fn default() -> Self {
        Self::Pilot(Pilot::Fox)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FlightControlStyle {
    #[default]
    TypeA,
    TypeB,
}

impl Default for PilotSelectionPhase {
    fn default() -> Self {
        Self::Revealing
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PilotSelectionState {
    pub phase: PilotSelectionPhase,
    pub cursor: PilotSelectionCursor,
    pub control_style: FlightControlStyle,
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

/// An immutable semantic force count from the retail difficulty profile.
/// Runtime systems convert these values into their own typed mutable state;
/// source-machine counters never cross into the native game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignForceCount(u8);

impl CampaignForceCount {
    pub const fn new(count: u8) -> Self {
        Self(count)
    }

    pub const fn count(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleCarrierDeployment {
    Single,
    Pair,
}

impl BattleCarrierDeployment {
    pub const fn count(self) -> CampaignForceCount {
        match self {
            Self::Single => CampaignForceCount::new(1),
            Self::Pair => CampaignForceCount::new(2),
        }
    }

    pub const fn deploys_second_carrier(self) -> bool {
        matches!(self, Self::Pair)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningAttackerWavePattern {
    NormalOpening,
    HardOpening,
    HardReinforcement,
    ExpertOpening,
    ExpertReinforcement,
}

pub const OPENING_ATTACKER_WAVE_CAPACITY: usize = 2;

/// Retail-derived strategic setup selected by the difficulty menu. Counts
/// express game concepts rather than the source program's storage layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DifficultyProfile {
    pub occupied_planets: CampaignForceCount,
    pub planetary_defense_units: CampaignForceCount,
    pub opening_attackers: CampaignForceCount,
    pub battle_carriers: BattleCarrierDeployment,
    pub opening_waves: [Option<OpeningAttackerWavePattern>; OPENING_ATTACKER_WAVE_CAPACITY],
}

impl DifficultyProfile {
    pub const NORMAL: Self = Self {
        occupied_planets: CampaignForceCount::new(2),
        planetary_defense_units: CampaignForceCount::new(2),
        opening_attackers: CampaignForceCount::new(2),
        battle_carriers: BattleCarrierDeployment::Single,
        opening_waves: [Some(OpeningAttackerWavePattern::NormalOpening), None],
    };

    pub const HARD: Self = Self {
        occupied_planets: CampaignForceCount::new(3),
        planetary_defense_units: CampaignForceCount::new(3),
        opening_attackers: CampaignForceCount::new(4),
        battle_carriers: BattleCarrierDeployment::Single,
        opening_waves: [
            Some(OpeningAttackerWavePattern::HardOpening),
            Some(OpeningAttackerWavePattern::HardReinforcement),
        ],
    };

    pub const EXPERT: Self = Self {
        occupied_planets: CampaignForceCount::new(3),
        planetary_defense_units: CampaignForceCount::new(6),
        opening_attackers: CampaignForceCount::new(4),
        battle_carriers: BattleCarrierDeployment::Pair,
        opening_waves: [
            Some(OpeningAttackerWavePattern::ExpertOpening),
            Some(OpeningAttackerWavePattern::ExpertReinforcement),
        ],
    };

    pub const fn total_opening_threat_units(self) -> CampaignForceCount {
        CampaignForceCount::new(
            self.planetary_defense_units
                .count()
                .saturating_add(self.opening_attackers.count()),
        )
    }
}

impl Difficulty {
    pub const fn profile(self) -> DifficultyProfile {
        match self {
            Self::Normal => DifficultyProfile::NORMAL,
            Self::Hard => DifficultyProfile::HARD,
            Self::Expert => DifficultyProfile::EXPERT,
        }
    }

    pub const fn previous(self, expert_unlocked: bool) -> Self {
        match (self, expert_unlocked) {
            (Self::Normal, true) => Self::Expert,
            (Self::Normal, false) => Self::Hard,
            (Self::Hard, _) => Self::Normal,
            (Self::Expert, _) => Self::Hard,
        }
    }

    pub const fn next(self, expert_unlocked: bool) -> Self {
        match (self, expert_unlocked) {
            (Self::Normal, _) => Self::Hard,
            (Self::Hard, true) => Self::Expert,
            (Self::Hard, false) | (Self::Expert, _) => Self::Normal,
        }
    }
}

/// Battery-backed campaign progress represented as semantic fields. Retail
/// exposes Expert only after a zero-damage Hard clear; the port stores that
/// meaning directly instead of carrying the cartridge-save bitfield through
/// gameplay.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CampaignProgress {
    pub expert_unlocked: bool,
}

impl CampaignProgress {
    pub fn record_clear(&mut self, difficulty: Difficulty, corneria_damage_percent: u8) -> bool {
        let newly_unlocked =
            !self.expert_unlocked && difficulty == Difficulty::Hard && corneria_damage_percent == 0;
        self.expert_unlocked |= newly_unlocked;
        newly_unlocked
    }
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
    FirstPlanetaryBase,
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
            Self::PigmaDuel => Self::FirstPlanetaryBase,
            Self::FirstPlanetaryBase => Self::FirstBattleCarrier,
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
            Self::FirstPlanetaryBase => 5,
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
    Unoccupied,
    Occupied,
    Rescued,
}

impl PlanetObjectiveStatus {
    pub const fn is_complete(self) -> bool {
        matches!(self, Self::Unoccupied | Self::Rescued)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierObjectiveStatus {
    NotDeployed,
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

/// The occupied planets chosen when a retail campaign begins. This is flat,
/// typed game state: source selection ordinals and table addresses remain in
/// the ROM generator, while the port stores only semantic world identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignWorldAssignment {
    occupied_worlds: [Option<CampaignWorld>; MAX_OCCUPIED_WORLD_COUNT],
}

impl CampaignWorldAssignment {
    pub fn from_timing_entropy(difficulty: Difficulty, timing_entropy: u64) -> Self {
        match difficulty {
            Difficulty::Normal => {
                let assignment = NORMAL_CAMPAIGN_WORLD_ASSIGNMENTS
                    [(timing_entropy % NORMAL_CAMPAIGN_ASSIGNMENT_COUNT as u64) as usize];
                Self {
                    occupied_worlds: [Some(assignment[0]), Some(assignment[1]), None],
                }
            }
            Difficulty::Hard | Difficulty::Expert => {
                let assignment = THREE_WORLD_CAMPAIGN_ASSIGNMENTS
                    [(timing_entropy % THREE_WORLD_CAMPAIGN_ASSIGNMENT_COUNT as u64) as usize];
                Self {
                    occupied_worlds: [
                        Some(assignment[0]),
                        Some(assignment[1]),
                        Some(assignment[2]),
                    ],
                }
            }
        }
    }

    pub const fn occupied_worlds(self) -> [Option<CampaignWorld>; MAX_OCCUPIED_WORLD_COUNT] {
        self.occupied_worlds
    }

    pub const fn first_occupied_world(self) -> Option<CampaignWorld> {
        self.occupied_worlds[0]
    }

    pub fn contains(self, world: CampaignWorld) -> bool {
        self.occupied_worlds.contains(&Some(world))
    }
}

impl Default for CampaignWorldAssignment {
    fn default() -> Self {
        Self::from_timing_entropy(Difficulty::Normal, 0)
    }
}

/// Independent objective state for every named campaign world. The retail
/// game keeps one status per world; the port mirrors that semantic layout as
/// ordinary fields rather than a numeric selector or byte-addressed table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignPlanetObjectives {
    pub venom: PlanetObjectiveStatus,
    pub titania: PlanetObjectiveStatus,
    pub macbeth: PlanetObjectiveStatus,
    pub eladard: PlanetObjectiveStatus,
    pub meteor: PlanetObjectiveStatus,
    pub fortuna: PlanetObjectiveStatus,
}

impl CampaignPlanetObjectives {
    pub fn from_assignment(assignment: CampaignWorldAssignment) -> Self {
        let status = |world| {
            if assignment.contains(world) {
                PlanetObjectiveStatus::Occupied
            } else {
                PlanetObjectiveStatus::Unoccupied
            }
        };
        Self {
            venom: status(CampaignWorld::Venom),
            titania: status(CampaignWorld::Titania),
            macbeth: status(CampaignWorld::Macbeth),
            eladard: status(CampaignWorld::Eladard),
            meteor: status(CampaignWorld::Meteor),
            fortuna: status(CampaignWorld::Fortuna),
        }
    }

    pub const fn status(self, world: CampaignWorld) -> PlanetObjectiveStatus {
        match world {
            CampaignWorld::Venom => self.venom,
            CampaignWorld::Titania => self.titania,
            CampaignWorld::Macbeth => self.macbeth,
            CampaignWorld::Eladard => self.eladard,
            CampaignWorld::Meteor => self.meteor,
            CampaignWorld::Fortuna => self.fortuna,
        }
    }

    pub fn rescue(&mut self, world: CampaignWorld) {
        let status = match world {
            CampaignWorld::Venom => &mut self.venom,
            CampaignWorld::Titania => &mut self.titania,
            CampaignWorld::Macbeth => &mut self.macbeth,
            CampaignWorld::Eladard => &mut self.eladard,
            CampaignWorld::Meteor => &mut self.meteor,
            CampaignWorld::Fortuna => &mut self.fortuna,
        };
        if *status == PlanetObjectiveStatus::Occupied {
            *status = PlanetObjectiveStatus::Rescued;
        }
    }

    pub const fn all_complete(self) -> bool {
        self.venom.is_complete()
            && self.titania.is_complete()
            && self.macbeth.is_complete()
            && self.eladard.is_complete()
            && self.meteor.is_complete()
            && self.fortuna.is_complete()
    }

    pub const fn rescued_count(self) -> u16 {
        let worlds = [
            self.venom,
            self.titania,
            self.macbeth,
            self.eladard,
            self.meteor,
            self.fortuna,
        ];
        let mut index = 0;
        let mut count = 0;
        while index < worlds.len() {
            if matches!(worlds[index], PlanetObjectiveStatus::Rescued) {
                count += 1;
            }
            index += 1;
        }
        count
    }
}

impl Default for CampaignPlanetObjectives {
    fn default() -> Self {
        Self::from_assignment(CampaignWorldAssignment::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignObjectives {
    pub planets: CampaignPlanetObjectives,
    pub first_carrier: CarrierObjectiveStatus,
    pub second_carrier: CarrierObjectiveStatus,
    pub missiles: StrategicThreatCount,
    pub live_attackers: StrategicThreatCount,
    pub wolf_blockade: WolfBlockadeStatus,
    pub astropolis: AstropolisStatus,
}

impl Default for CampaignObjectives {
    fn default() -> Self {
        Self::for_campaign(Difficulty::Normal, CampaignWorldAssignment::default())
    }
}

impl CampaignObjectives {
    pub fn for_campaign(difficulty: Difficulty, world_assignment: CampaignWorldAssignment) -> Self {
        let second_carrier = if difficulty
            .profile()
            .battle_carriers
            .deploys_second_carrier()
        {
            CarrierObjectiveStatus::Operational
        } else {
            CarrierObjectiveStatus::NotDeployed
        };
        Self {
            planets: CampaignPlanetObjectives::from_assignment(world_assignment),
            first_carrier: CarrierObjectiveStatus::Operational,
            second_carrier,
            missiles: StrategicThreatCount::INITIAL_MISSILES,
            live_attackers: StrategicThreatCount::NONE,
            wolf_blockade: WolfBlockadeStatus::Unavailable,
            astropolis: AstropolisStatus::Locked,
        }
    }

    pub const fn major_objectives_complete(self) -> bool {
        self.planets.all_complete()
            && matches!(self.first_carrier, CarrierObjectiveStatus::Destroyed)
            && matches!(
                self.second_carrier,
                CarrierObjectiveStatus::NotDeployed | CarrierObjectiveStatus::Destroyed
            )
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignState {
    pub elapsed_frames: u64,
    /// Damage shown by the retail strategic-map HUD, in whole percent.
    pub corneria_damage_percent: u8,
    /// Typed campaign pressure affecting Corneria after the eighth certified
    /// sortie. It advances while the command map is live and pauses during an
    /// action mission, just like the retail strategic simulation.
    pub corneria_defense: CorneriaDefenseState,
    pub difficulty: Difficulty,
    pub world_assignment: CampaignWorldAssignment,
    pub active_threats: Vec<ObjectId>,
    pub route_step: CampaignRouteStep,
    pub objectives: CampaignObjectives,
}

impl CampaignState {
    pub fn new(difficulty: Difficulty) -> Self {
        Self::for_new_game(difficulty, 0)
    }

    /// Create a fresh campaign. Retail mixes front-end timing into its choice;
    /// the port passes that timing as a semantic input and immediately stores
    /// the resulting world identities. Retry keeps this state unchanged.
    pub fn for_new_game(difficulty: Difficulty, timing_entropy: u64) -> Self {
        let world_assignment =
            CampaignWorldAssignment::from_timing_entropy(difficulty, timing_entropy);
        Self {
            elapsed_frames: 0,
            corneria_damage_percent: 0,
            corneria_defense: CorneriaDefenseState::default(),
            difficulty,
            world_assignment,
            active_threats: Vec::new(),
            route_step: CampaignRouteStep::default(),
            objectives: CampaignObjectives::for_campaign(difficulty, world_assignment),
        }
    }

    pub const fn difficulty_profile(&self) -> DifficultyProfile {
        self.difficulty.profile()
    }

    /// Derived presentation/automation progress. The runtime stores semantic
    /// route and objective state; it does not maintain a second numeric sortie
    /// counter that can drift out of sync.
    pub const fn completed_campaign_visits(&self) -> u16 {
        let completed = self.route_step.completed_certified_visits();
        match self.route_step {
            CampaignRouteStep::StrategicPressure => {
                let first_planetary_rescue = match self.world_assignment.first_occupied_world() {
                    Some(world)
                        if matches!(
                            self.objectives.planets.status(world),
                            PlanetObjectiveStatus::Rescued
                        ) =>
                    {
                        1
                    }
                    Some(_) | None => 0,
                };
                completed
                    + self
                        .objectives
                        .planets
                        .rescued_count()
                        .saturating_sub(first_planetary_rescue)
                    + match self.objectives.second_carrier {
                        CarrierObjectiveStatus::NotDeployed
                        | CarrierObjectiveStatus::Operational => 0,
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
            | CampaignRouteStep::FirstPlanetaryBase
            | CampaignRouteStep::FirstBattleCarrier
            | CampaignRouteStep::LeonDuel
            | CampaignRouteStep::MirageDragon
            | CampaignRouteStep::WolfBlockade => completed,
        }
    }
}

impl Default for CampaignState {
    fn default() -> Self {
        Self::new(Difficulty::Normal)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategicOpeningPage {
    TerribleNews,
    AndrossReturned,
    AssaultUnderway,
    BattleCarriers,
    ForcesAdvancing,
    EnemyBases,
    PlanetaryMissiles,
    RequestAssistance,
    MinorDamage,
    TotalDamage,
    DefendCorneria,
    GoodLuck,
}

impl Default for StrategicOpeningPage {
    fn default() -> Self {
        Self::TerribleNews
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StrategicOpeningState {
    pub page: StrategicOpeningPage,
    pub presentation_tick: u16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StrategicMapState {
    pub phase: StrategicMapPhase,
    pub opening: StrategicOpeningState,
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
    VenomBase,
    TitaniaBase,
    MacbethBase,
    EladardBase,
    MeteorBase,
    FortunaBase,
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
    VenomBase,
    EladardBase,
    TitaniaBase,
    MacbethBase,
    MeteorBase,
    FortunaBase,
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

/// Collision response for the active player craft. Retail allows a hit that
/// reaches zero shield to survive; only a later hit received while already at
/// zero begins destruction.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PlayerDamageState {
    #[default]
    Ready,
    Recovering {
        retail_frames_remaining: u8,
    },
    Destroying {
        elapsed_retail_frames: u16,
    },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GameOverChoice {
    #[default]
    ContinueWithWingmate,
    EndCampaign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameOverDestination {
    StrategicMap,
    Results,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GameOverPhase {
    #[default]
    AndrossTaunt,
    Choosing(GameOverChoice),
    Leaving {
        destination: GameOverDestination,
        elapsed_retail_frames: u16,
    },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GameOverState {
    pub phase: GameOverPhase,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ResultsChoice {
    #[default]
    Retry,
    Title,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ResultsPhase {
    #[default]
    Revealing,
    OpeningChoices {
        elapsed_retail_frames: u16,
    },
    Choosing(ResultsChoice),
    Leaving {
        choice: ResultsChoice,
        elapsed_retail_frames: u16,
    },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ResultsState {
    pub phase: ResultsPhase,
    pub choice_presentation_retail_frames: u16,
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

/// Authored mission guidance selected by gameplay meaning rather than by a
/// numeric storage index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionMessage {
    FlyFasterByPressingYButton,
}

/// The five interference pictures used while a mission-radio portrait opens
/// and closes. The renderer owns their actual artwork.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionMessageIrisFrame {
    ThinLine,
    EmptyPanel,
    SparseInterference,
    DenseInterference,
    FullInterference,
}

/// Typed visible phase of the mission-radio presentation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MissionMessagePhase {
    #[default]
    Hidden,
    Opening(MissionMessageIrisFrame),
    Open,
    Closing(MissionMessageIrisFrame),
}

/// Flat mission-owned radio state. Message identity, elapsed presentation
/// time, and portrait animation remain ordinary domain fields.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MissionMessageState {
    pub message: Option<MissionMessage>,
    pub phase: MissionMessagePhase,
    pub elapsed_retail_frames: u16,
    pub portrait_talking: bool,
}

pub const RECURRING_ATTACKER_COUNT: usize = 4;

/// Semantic identity of one craft in the recurring four-attacker encounter.
/// Object-store identities remain runtime implementation details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurringAttacker {
    Vanguard,
    HighGuard,
    Flanker,
    Pursuer,
}

impl RecurringAttacker {
    pub const ALL: [Self; RECURRING_ATTACKER_COUNT] = [
        Self::Vanguard,
        Self::HighGuard,
        Self::Flanker,
        Self::Pursuer,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::Vanguard => 0,
            Self::HighGuard => 1,
            Self::Flanker => 2,
            Self::Pursuer => 3,
        }
    }
}

/// Lifecycle of a recurring attacker. A craft that leaves the combat volume
/// has departed, not been defeated, and therefore cannot satisfy the encounter
/// objective.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RecurringAttackerStatus {
    #[default]
    AwaitingDeployment,
    Active,
    Departed,
    Defeated,
}

/// Flat, typed objective state for the recurring four-attacker encounter.
/// Successful return timing begins only after all four destruction sequences
/// have completed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RecurringAttackersState {
    pub vanguard: RecurringAttackerStatus,
    pub high_guard: RecurringAttackerStatus,
    pub flanker: RecurringAttackerStatus,
    pub pursuer: RecurringAttackerStatus,
    pub all_defeated_retail_frame: Option<u16>,
}

impl RecurringAttackersState {
    pub const fn deployed() -> Self {
        Self {
            vanguard: RecurringAttackerStatus::Active,
            high_guard: RecurringAttackerStatus::Active,
            flanker: RecurringAttackerStatus::Active,
            pursuer: RecurringAttackerStatus::Active,
            all_defeated_retail_frame: None,
        }
    }

    pub fn status(&self, attacker: RecurringAttacker) -> RecurringAttackerStatus {
        *self.status_ref(attacker)
    }

    pub fn record_departure(&mut self, attacker: RecurringAttacker) {
        let status = self.status_mut(attacker);
        if *status == RecurringAttackerStatus::Active {
            *status = RecurringAttackerStatus::Departed;
        }
    }

    pub fn record_defeat(&mut self, attacker: RecurringAttacker, retail_frame: u16) {
        *self.status_mut(attacker) = RecurringAttackerStatus::Defeated;
        if self.all_defeated() {
            self.all_defeated_retail_frame.get_or_insert(retail_frame);
        }
    }

    pub fn all_defeated(&self) -> bool {
        RecurringAttacker::ALL
            .into_iter()
            .all(|attacker| self.status(attacker) == RecurringAttackerStatus::Defeated)
    }

    fn status_ref(&self, attacker: RecurringAttacker) -> &RecurringAttackerStatus {
        match attacker {
            RecurringAttacker::Vanguard => &self.vanguard,
            RecurringAttacker::HighGuard => &self.high_guard,
            RecurringAttacker::Flanker => &self.flanker,
            RecurringAttacker::Pursuer => &self.pursuer,
        }
    }

    fn status_mut(&mut self, attacker: RecurringAttacker) -> &mut RecurringAttackerStatus {
        match attacker {
            RecurringAttacker::Vanguard => &mut self.vanguard,
            RecurringAttacker::HighGuard => &mut self.high_guard,
            RecurringAttacker::Flanker => &mut self.flanker,
            RecurringAttacker::Pursuer => &mut self.pursuer,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MirageDragonCameraPhase {
    #[default]
    Dormant,
    TrackingFocus,
    PlayerFollow,
    TargetTracking,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MirageDragonFollowDistancePhase {
    #[default]
    ApproachingNear,
    HoldingNear,
    ApproachingFar,
    HoldingFar,
}

/// Fine camera orientation with 65,536 subunits per turn. Keeping these
/// authored subunits between updates preserves the scene's gradual orientation
/// chase without exposing processor state in the native game.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CameraOrientationSubunits {
    pub pitch: u16,
    pub yaw: u16,
    pub roll: u16,
}

/// Signed coarse-turn corrections used to preserve a camera transition while
/// the new camera strategy settles.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CameraOrientationOffsets {
    pub pitch: i8,
    pub yaw: i8,
    pub roll: i8,
}

/// Typed state for the Mirage Dragon camera. The retail scene changes from a
/// moving-focus intro to player follow and target-tracking strategies; the port
/// represents those concepts directly instead of retaining byte-addressed
/// object storage.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MirageDragonCameraState {
    pub phase: MirageDragonCameraPhase,
    pub strategy_step: u8,
    pub focus_position: Vector3,
    pub relative_anchor_position: Vector3,
    pub anchor_depth_motion: i16,
    pub anchor_position: Vector3,
    pub orientation: CameraOrientationSubunits,
    pub follow_vertical_offset: i16,
    pub follow_rear_distance: i16,
    pub follow_distance_phase: MirageDragonFollowDistancePhase,
    pub follow_hold_updates_remaining: u8,
    pub follow_view_orientation: CameraOrientationSubunits,
    pub ambient_height_phase: u8,
    pub ambient_height_offset: i16,
    pub continuity_translation: Vector3,
    pub continuity_orientation_offsets: CameraOrientationOffsets,
    pub continuity_reset_pending: bool,
    pub previous_output_position: Vector3,
    pub previous_output_orientation: CameraOrientationSubunits,
    pub follow_motion_updates_elapsed: u8,
    pub tracking_updates_elapsed: u8,
    pub tracking_bearing: u8,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MissionState {
    pub active: bool,
    pub phase: MissionPhase,
    pub visit: MissionVisit,
    pub primary_player: Option<ObjectId>,
    pub wingmate: Option<ObjectId>,
    pub score: u32,
    pub objects_destroyed: u32,
    pub item_count: u8,
    pub elapsed_time_tenths: u16,
    /// Retail presentation boundary at which the current rival finished its
    /// destruction sequence. Mission return timing is relative to an actual
    /// combat defeat, never a scripted disappearance.
    pub rival_defeated_retail_frame: Option<u16>,
    pub recurring_attackers: RecurringAttackersState,
    pub player_blaster: PlayerBlasterState,
    pub player_damage: PlayerDamageState,
    /// Persistent analog flight response recovered from the retail player
    /// object. These are ordinary gameplay values, separate from the visible
    /// craft angles so steering remains smooth while the craft leans.
    pub player_flight: PlayerFlightState,
    pub mirage_dragon_camera: MirageDragonCameraState,
    pub player_walker: PlayerWalkerState,
    pub player_craft_form: PlayerCraftForm,
    pub message: MissionMessageState,
    /// Becomes true when steering leaves the certified neutral path.
    /// The port then continues from that typed pose with native flight rules.
    pub departed_certified_neutral_path: bool,
    pub camera_follow_offset: super::object::Vector3,
    pub eladard: EladardMissionState,
    pub titania: TitaniaMissionState,
    pub macbeth: MacbethMissionState,
    pub fortuna: FortunaMissionState,
    pub meteor: MeteorMissionState,
    pub venom: VenomMissionState,
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
    InteriorPassage,
    GeneratorRoom,
    BaseDestruction,
    ReturnFlight,
}

impl Default for EladardPhase {
    fn default() -> Self {
        Self::SurfaceApproach
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EladardBarrierStatus {
    Active { durability: u8 },
    Destroyed,
}

impl Default for EladardBarrierStatus {
    fn default() -> Self {
        Self::Destroyed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EladardGeneratorStatus {
    Unreached,
    Active { durability: u8 },
    Destroyed,
}

impl Default for EladardGeneratorStatus {
    fn default() -> Self {
        Self::Unreached
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EladardInteriorRoom {
    Unreached,
    AccessChamber,
    TransitChamber,
    GeneratorChamber,
}

impl Default for EladardInteriorRoom {
    fn default() -> Self {
        Self::Unreached
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EladardSwitchStatus {
    Unreached,
    Active,
    Pressed,
}

impl Default for EladardSwitchStatus {
    fn default() -> Self {
        Self::Unreached
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EladardDoorStatus {
    Unreached,
    Closed,
    Opening { retail_frames_remaining: u16 },
    Open,
}

impl Default for EladardDoorStatus {
    fn default() -> Self {
        Self::Unreached
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EladardDefenderStatus {
    Unreached,
    Active,
    Destroyed,
}

impl Default for EladardDefenderStatus {
    fn default() -> Self {
        Self::Unreached
    }
}

/// Typed objective state for the Eladard base assault. These fields model the
/// mission concepts directly; no source-machine memory window is retained.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EladardMissionState {
    pub phase: EladardPhase,
    pub phase_started_retail_frame: u16,
    pub surface_barriers: [EladardBarrierStatus; 2],
    pub interior_room: EladardInteriorRoom,
    pub access_switch: EladardSwitchStatus,
    pub access_door: EladardDoorStatus,
    pub generator_door: EladardDoorStatus,
    pub interior_defenders: [EladardDefenderStatus; 2],
    pub generator: EladardGeneratorStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitaniaSurfaceSwitchStatus {
    Active,
    Pressed,
}

impl Default for TitaniaSurfaceSwitchStatus {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitaniaFinalSwitchStatus {
    Unreached,
    Active,
    Pressed,
}

impl Default for TitaniaFinalSwitchStatus {
    fn default() -> Self {
        Self::Unreached
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitaniaPhase {
    SurfaceApproach,
    FirstSwitch,
    SurfaceTransit,
    SecondSwitch,
    BaseOpening,
    BaseEntry,
    Interior,
    FinalSwitch,
    BaseEscape,
    ReturnFlight,
}

impl Default for TitaniaPhase {
    fn default() -> Self {
        Self::SurfaceApproach
    }
}

/// Typed objective state for Titania. The two exterior pressure switches and
/// final interior switch are the retail mission concepts; the port does not
/// expose their original storage locations or retain a byte-addressed work
/// area.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TitaniaMissionState {
    pub phase: TitaniaPhase,
    pub phase_started_retail_frame: u16,
    pub surface_switches: [TitaniaSurfaceSwitchStatus; TITANIA_SURFACE_SWITCH_COUNT],
    pub final_switch: TitaniaFinalSwitchStatus,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MacbethPhase {
    #[default]
    FirstSurfaceSwitch,
    TowerGuns,
    SecondSurfaceSwitch,
    BaseOpening,
    BaseEntry,
    KnightCombat,
    KnightDestruction,
    InteriorTransit,
    CoreTurrets,
    CoreShieldOpening,
    CoreCombat,
    CoreDestruction,
    ReturnFlight,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MacbethSwitchStatus {
    #[default]
    Active,
    Pressed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MacbethDefenderStatus {
    #[default]
    Dormant,
    Active {
        durability: u8,
    },
    Destroyed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MacbethInstallationStatus {
    #[default]
    Closed,
    Opening {
        retail_frames_remaining: u16,
    },
    Open,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MacbethCoreStatus {
    #[default]
    Shielded,
    Exposed {
        durability: u8,
    },
    Destroyed,
}

/// Flat mission-owned state for Macbeth's two surface switches, rotating
/// defense tower, interior guardian, and shielded installation core. Runtime
/// transforms and durability live on ordinary objects; no source addresses or
/// machine records are retained here.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MacbethMissionState {
    pub phase: MacbethPhase,
    pub phase_started_retail_frame: u16,
    pub surface_switches: [MacbethSwitchStatus; 2],
    pub tower_guns: [MacbethDefenderStatus; 2],
    pub installation: MacbethInstallationStatus,
    pub knight: MacbethDefenderStatus,
    pub core_turrets: [MacbethDefenderStatus; 2],
    pub core: MacbethCoreStatus,
}

pub const FORTUNA_SURFACE_SWITCH_COUNT: usize = 2;
pub const FORTUNA_MAXIMUM_CORE_DEFENDER_COUNT: usize = 2;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FortunaPhase {
    #[default]
    SurfaceSwitches,
    SurfaceEntry,
    KickGunnerCombat,
    InteriorTransit,
    CoreDefenders,
    CoreShieldOpening,
    CoreCombat,
    CoreDestruction,
    ReturnFlight,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FortunaSwitchStatus {
    #[default]
    Active,
    Pressed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FortunaDefenderStatus {
    #[default]
    NotInstalled,
    Dormant,
    Active {
        durability: u8,
    },
    Destroyed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FortunaCoreStatus {
    #[default]
    Shielded,
    OuterShell {
        durability: u8,
    },
    InnerCore {
        durability: u8,
    },
    Destroyed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FortunaKickGunnerPhase {
    #[default]
    Dormant,
    WaitingToDive {
        retail_frames_remaining: u16,
    },
    DivingToFloor {
        action_index: u8,
    },
    LongDive {
        action_index: u8,
    },
    SurfaceBobAfterDive {
        action_index: u8,
    },
    RestingAfterDive {
        actions_remaining: u8,
    },
    AttackPreparationBob {
        attack_index: u8,
        action_index: u8,
    },
    AttackLeap {
        attack_index: u8,
        action_index: u8,
    },
    AttackRecoveryBob {
        attack_index: u8,
        action_index: u8,
    },
    AttackPause {
        attack_index: u8,
        actions_remaining: u8,
    },
    AttackPostSpawnWait {
        attack_index: u8,
        actions_remaining: u8,
    },
    RetreatPreparationBob {
        action_index: u8,
    },
    Retreat {
        action_index: u8,
    },
    WaitingBeforeRouteSelection {
        retail_frames_remaining: u16,
    },
}

/// Semantic state for Fortuna's interior guardian. The original strategy's
/// movement direction and retreat destination are ordinary flat world state;
/// no source path pointer or object-window storage is retained.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FortunaKickGunnerMotionState {
    pub phase: FortunaKickGunnerPhase,
    pub movement_yaw: Angle,
    pub retreat_target: Vector3,
    pub action_retail_frame_accumulator: u8,
}

/// Flat mission-owned state for Fortuna's submerged switches, interior
/// guardian, shield defenses, and two-threshold installation core. Source
/// object slots and strategy storage never enter the native game state.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FortunaMissionState {
    pub phase: FortunaPhase,
    pub phase_started_retail_frame: u16,
    pub surface_switches: [FortunaSwitchStatus; FORTUNA_SURFACE_SWITCH_COUNT],
    pub kick_gunner: FortunaDefenderStatus,
    pub kick_gunner_motion: FortunaKickGunnerMotionState,
    pub core_defenders: [FortunaDefenderStatus; FORTUNA_MAXIMUM_CORE_DEFENDER_COUNT],
    pub core: FortunaCoreStatus,
    pub core_emitter_index: u8,
    pub core_emitter_wait_retail_frames: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MeteorPhase {
    #[default]
    SurfaceCombat,
    QueenDestruction,
    DroppedSwitch,
    BaseEntry,
    InteriorApproach,
    CoreArming,
    CoreCombat,
    CoreDestruction,
    ReturnFlight,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MeteorSwitchStatus {
    #[default]
    Hidden,
    Dropped,
    Pressed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MeteorCoreStatus {
    #[default]
    Dormant,
    Triggered,
    Armed,
    Active,
    Destroyed,
}

/// Flat objective state for Meteor's Queen Dragoon and installation route.
/// Runtime objects hold their ordinary transforms and durability while this
/// struct records only mission progression and authored presentation timing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MeteorMissionState {
    pub phase: MeteorPhase,
    pub phase_started_retail_frame: u16,
    pub surface_switch: MeteorSwitchStatus,
    pub installation_core: MeteorCoreStatus,
}

pub const VENOM_SURFACE_SWITCH_COUNT: usize = 2;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum VenomPhase {
    #[default]
    SurfaceSwitches,
    SurfaceEntry,
    FirstInteriorSwitch,
    InteriorTransit,
    ArmoredPassage,
    ReactorArming,
    ReactorCombat,
    ReactorDestruction,
    ReturnFlight,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum VenomSwitchStatus {
    #[default]
    Active,
    Pressed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum VenomDoorStatus {
    #[default]
    Closed,
    Open,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum VenomDefenderStatus {
    #[default]
    Dormant,
    Active {
        durability: u8,
    },
    Destroyed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum VenomReactorStatus {
    #[default]
    Dormant,
    Triggered,
    Armed,
    Active {
        durability: u8,
    },
    Destroyed,
}

/// Flat objective state for Venom's two surface activators, installation
/// access route, armored interior pressure, and final reactor. Runtime poses
/// and durability remain ordinary object fields; no source-machine records or
/// byte-addressed storage enter the native mission model.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VenomMissionState {
    pub phase: VenomPhase,
    pub phase_started_retail_frame: u16,
    pub surface_switches: [VenomSwitchStatus; VENOM_SURFACE_SWITCH_COUNT],
    pub access_switch: VenomSwitchStatus,
    pub access_door: VenomDoorStatus,
    pub reactor_door: VenomDoorStatus,
    pub knight: VenomDefenderStatus,
    pub reactor: VenomReactorStatus,
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
/// core. Integrity follows the observed retail object field: each effective
/// vulnerability hit removes two points and the panel changes to its broken
/// mesh at 90. The vulnerability window is semantic state rather than a
/// retained object-path cursor or byte-addressed work area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarrierReactorPanel {
    pub integrity: u8,
    pub active: bool,
    pub vulnerability: CarrierReactorVulnerabilityStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierReactorVulnerabilityStatus {
    Inactive,
    Waiting { retail_frames_remaining: u16 },
    Opening { elapsed_retail_frames: u16 },
    Exposed { elapsed_retail_frames: u16 },
    Destroyed,
}

impl Default for CarrierReactorVulnerabilityStatus {
    fn default() -> Self {
        Self::Inactive
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierCorridorDefenderStatus {
    Unreached,
    Active,
    Destroyed,
    Withdrawn,
}

pub const CARRIER_CORRIDOR_DEFENDER_COUNT: usize = 3;
pub const CARRIER_CORRIDOR_GATE_COUNT: usize = 2;
pub const CARRIER_ROTATING_DOOR_COUNT: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierCorridorControlStatus {
    Unreached,
    Active,
    Activating { elapsed_retail_frames: u16 },
    Complete,
}

impl Default for CarrierCorridorControlStatus {
    fn default() -> Self {
        Self::Unreached
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierCorridorPassageStatus {
    Unreached,
    Closed,
    Opening { elapsed_retail_frames: u16 },
    Open,
}

impl Default for CarrierCorridorPassageStatus {
    fn default() -> Self {
        Self::Unreached
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CarrierCorridorGateState {
    pub control: CarrierCorridorControlStatus,
    pub passage: CarrierCorridorPassageStatus,
}

impl Default for CarrierCorridorDefenderStatus {
    fn default() -> Self {
        Self::Unreached
    }
}

impl Default for CarrierReactorPanel {
    fn default() -> Self {
        Self {
            integrity: 100,
            active: true,
            vulnerability: CarrierReactorVulnerabilityStatus::Inactive,
        }
    }
}

/// Typed objective state for the Battle Carrier assault. It models the
/// player-driven exterior approach, rail corridor, automatic room-entry
/// transformation, and the two reactor panels plus their transient damage
/// relays directly; the shipping game does not retain a byte-addressed memory
/// image.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CarrierAssaultState {
    pub phase: CarrierAssaultPhase,
    pub phase_started_retail_frame: u16,
    pub corridor_progress: u16,
    pub reactor_room_open: bool,
    pub room_entry_transformation_started_retail_frame: Option<u16>,
    pub corridor_defenders: [CarrierCorridorDefenderStatus; CARRIER_CORRIDOR_DEFENDER_COUNT],
    pub corridor_gates: [CarrierCorridorGateState; CARRIER_CORRIDOR_GATE_COUNT],
    pub rotating_doors: [CarrierCorridorPassageStatus; CARRIER_ROTATING_DOOR_COUNT],
    pub port_panel: CarrierReactorPanel,
    pub starboard_panel: CarrierReactorPanel,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerFlightState {
    pub pitch_accumulator: i16,
    pub yaw_accumulator: i16,
    pub pitch_lean: i8,
    /// Current sample in the retail craft's subtle ambient bank wave.
    pub ambient_bank_phase: u8,
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
    TitleReveal,
    TitleSplash,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IntroState {
    pub presentation_tick: u16,
    pub title_menu_countdown: Option<u8>,
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
    GameOver,
    Results,
    Ending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndingPhase {
    StaffRoll,
    EndScreen,
    Leaving { elapsed_retail_frames: u16 },
}

impl Default for EndingPhase {
    fn default() -> Self {
        Self::StaffRoll
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EndingState {
    pub phase: EndingPhase,
    pub presentation_tick: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    pub frame: u64,
    pub mode_frame: u32,
    pub mode: GameMode,
    pub intro: IntroState,
    pub title: TitleState,
    pub roster: Roster,
    pub progress: CampaignProgress,
    pub campaign: CampaignState,
    pub strategic_map: StrategicMapState,
    pub pilot_selection: PilotSelectionState,
    pub mission: MissionState,
    pub game_over: GameOverState,
    pub results: ResultsState,
    pub ending: EndingState,
    pub audio: AudioState,
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
            intro: IntroState::default(),
            title: TitleState::default(),
            roster: Roster::default(),
            progress: CampaignProgress::default(),
            campaign: CampaignState::default(),
            strategic_map: StrategicMapState::default(),
            pilot_selection: PilotSelectionState::default(),
            mission: MissionState::default(),
            game_over: GameOverState::default(),
            results: ResultsState::default(),
            ending: EndingState::default(),
            audio: AudioState::default(),
            objects: ObjectStore::new(),
            camera: Camera::default(),
            input: InputState::default(),
            random: RandomState::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::campaign_world_assignments::NORMAL_OCCUPIED_WORLD_COUNT;
    use super::*;

    #[test]
    fn expert_unlock_requires_a_zero_damage_hard_clear() {
        let mut progress = CampaignProgress::default();
        assert!(!progress.record_clear(Difficulty::Normal, 0));
        assert!(!progress.expert_unlocked);
        assert!(!progress.record_clear(Difficulty::Hard, 1));
        assert!(!progress.expert_unlocked);
        assert!(progress.record_clear(Difficulty::Hard, 0));
        assert!(progress.expert_unlocked);
        assert!(!progress.record_clear(Difficulty::Hard, 0));
    }

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
    fn difficulty_profiles_keep_the_retail_strategic_force_counts_typed() {
        const NORMAL_COUNTS: [u8; 5] = [2, 2, 2, 4, 1];
        const HARD_COUNTS: [u8; 5] = [3, 3, 4, 7, 1];
        const EXPERT_COUNTS: [u8; 5] = [3, 6, 4, 10, 2];

        let counts = |difficulty: Difficulty| {
            let profile = difficulty.profile();
            [
                profile.occupied_planets.count(),
                profile.planetary_defense_units.count(),
                profile.opening_attackers.count(),
                profile.total_opening_threat_units().count(),
                profile.battle_carriers.count().count(),
            ]
        };

        assert_eq!(counts(Difficulty::Normal), NORMAL_COUNTS);
        assert_eq!(counts(Difficulty::Hard), HARD_COUNTS);
        assert_eq!(counts(Difficulty::Expert), EXPERT_COUNTS);
        assert_eq!(
            Difficulty::Normal.profile().opening_waves,
            [Some(OpeningAttackerWavePattern::NormalOpening), None]
        );
        assert_eq!(
            Difficulty::Hard.profile().opening_waves,
            [
                Some(OpeningAttackerWavePattern::HardOpening),
                Some(OpeningAttackerWavePattern::HardReinforcement),
            ]
        );
        assert_eq!(
            Difficulty::Expert.profile().opening_waves,
            [
                Some(OpeningAttackerWavePattern::ExpertOpening),
                Some(OpeningAttackerWavePattern::ExpertReinforcement),
            ]
        );
    }

    #[test]
    fn campaign_world_assignments_cover_every_retail_choice() {
        let normal_assignments: std::collections::BTreeSet<_> = (0
            ..NORMAL_CAMPAIGN_ASSIGNMENT_COUNT as u64)
            .map(|timing| {
                CampaignWorldAssignment::from_timing_entropy(Difficulty::Normal, timing)
                    .occupied_worlds()
            })
            .collect();
        assert_eq!(normal_assignments.len(), NORMAL_CAMPAIGN_ASSIGNMENT_COUNT);
        assert!(normal_assignments.iter().all(|assignment| {
            assignment.iter().flatten().count() == NORMAL_OCCUPIED_WORLD_COUNT
        }));
        assert!(normal_assignments.iter().all(|assignment| {
            !assignment.contains(&Some(CampaignWorld::Macbeth))
                && !assignment.contains(&Some(CampaignWorld::Fortuna))
        }));

        let hard_assignments: std::collections::BTreeSet<_> = (0
            ..THREE_WORLD_CAMPAIGN_ASSIGNMENT_COUNT as u64)
            .map(|timing| {
                CampaignWorldAssignment::from_timing_entropy(Difficulty::Hard, timing)
                    .occupied_worlds()
            })
            .collect();
        let expert_assignments: std::collections::BTreeSet<_> = (0
            ..THREE_WORLD_CAMPAIGN_ASSIGNMENT_COUNT as u64)
            .map(|timing| {
                CampaignWorldAssignment::from_timing_entropy(Difficulty::Expert, timing)
                    .occupied_worlds()
            })
            .collect();
        assert_eq!(
            hard_assignments.len(),
            THREE_WORLD_CAMPAIGN_ASSIGNMENT_COUNT
        );
        assert_eq!(expert_assignments, hard_assignments);
        assert!(CampaignWorld::ALL.iter().all(|world| {
            hard_assignments
                .iter()
                .any(|assignment| assignment.contains(&Some(*world)))
        }));
    }

    #[test]
    fn campaign_world_assignment_wraps_timing_without_source_ordinals() {
        let first = CampaignWorldAssignment::from_timing_entropy(Difficulty::Normal, 0);
        let wrapped = CampaignWorldAssignment::from_timing_entropy(
            Difficulty::Normal,
            NORMAL_CAMPAIGN_ASSIGNMENT_COUNT as u64,
        );
        assert_eq!(first, wrapped);
        assert_eq!(
            first.occupied_worlds(),
            [
                Some(CampaignWorld::Titania),
                Some(CampaignWorld::Venom),
                None,
            ]
        );
        assert!(first.contains(CampaignWorld::Titania));
        assert!(!first.contains(CampaignWorld::Macbeth));
    }

    #[test]
    fn campaign_planet_objectives_preserve_all_six_world_identities() {
        for difficulty in [Difficulty::Normal, Difficulty::Hard, Difficulty::Expert] {
            let assignment_count = match difficulty {
                Difficulty::Normal => NORMAL_CAMPAIGN_ASSIGNMENT_COUNT,
                Difficulty::Hard | Difficulty::Expert => THREE_WORLD_CAMPAIGN_ASSIGNMENT_COUNT,
            };
            for timing in 0..assignment_count {
                let assignment =
                    CampaignWorldAssignment::from_timing_entropy(difficulty, timing as u64);
                let mut planets = CampaignPlanetObjectives::from_assignment(assignment);
                for world in CampaignWorld::ALL {
                    let expected = if assignment.contains(world) {
                        PlanetObjectiveStatus::Occupied
                    } else {
                        PlanetObjectiveStatus::Unoccupied
                    };
                    assert_eq!(planets.status(world), expected);
                    if expected == PlanetObjectiveStatus::Unoccupied {
                        planets.rescue(world);
                        assert_eq!(planets.status(world), PlanetObjectiveStatus::Unoccupied);
                    }
                }
                assert!(!planets.all_complete());
                for world in assignment.occupied_worlds().into_iter().flatten() {
                    planets.rescue(world);
                }
                assert!(planets.all_complete());
                assert_eq!(
                    planets.rescued_count(),
                    assignment.occupied_worlds().into_iter().flatten().count() as u16
                );
            }
        }
    }

    #[test]
    fn only_expert_deploys_the_second_battle_carrier() {
        let complete_first_carrier_route = |difficulty| {
            let assignment = CampaignWorldAssignment::from_timing_entropy(difficulty, 0);
            let mut objectives = CampaignObjectives::for_campaign(difficulty, assignment);
            for world in assignment.occupied_worlds().into_iter().flatten() {
                objectives.planets.rescue(world);
            }
            objectives.first_carrier = CarrierObjectiveStatus::Destroyed;
            objectives.missiles = StrategicThreatCount::NONE;
            objectives.live_attackers = StrategicThreatCount::NONE;
            objectives
        };

        let normal = complete_first_carrier_route(Difficulty::Normal);
        let hard = complete_first_carrier_route(Difficulty::Hard);
        let expert = complete_first_carrier_route(Difficulty::Expert);

        assert_eq!(normal.second_carrier, CarrierObjectiveStatus::NotDeployed);
        assert_eq!(hard.second_carrier, CarrierObjectiveStatus::NotDeployed);
        assert_eq!(expert.second_carrier, CarrierObjectiveStatus::Operational);
        assert!(normal.final_gate_clear());
        assert!(hard.final_gate_clear());
        assert!(!expert.final_gate_clear());
    }

    #[test]
    fn astropolis_requires_every_typed_objective_then_the_wolf_blockade() {
        const ONE_LIVE_ATTACKER: StrategicThreatCount = StrategicThreatCount::new(1);

        let mut objectives = CampaignObjectives {
            planets: CampaignPlanetObjectives {
                venom: PlanetObjectiveStatus::Rescued,
                titania: PlanetObjectiveStatus::Rescued,
                macbeth: PlanetObjectiveStatus::Rescued,
                eladard: PlanetObjectiveStatus::Rescued,
                meteor: PlanetObjectiveStatus::Rescued,
                fortuna: PlanetObjectiveStatus::Rescued,
            },
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
