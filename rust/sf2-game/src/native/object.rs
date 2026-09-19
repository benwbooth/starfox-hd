use sf2_data::shape_data::{ShapeDataEntry, SHAPE_DATA};

use super::render::MaterialSetId;

pub const OBJECT_CAPACITY: usize = 60;
// Five authored sprite shapes selected by the allocation-pressure sweep.
const POOL_RECLAIMABLE_SPRITES: [ShapeId; 5] = [
    ShapeId(9),
    ShapeId(10),
    ShapeId(11),
    ShapeId(12),
    ShapeId(13),
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Vector3 {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Angle(u8);

impl Angle {
    pub const ZERO: Self = Self(0);
    pub const HALF_TURN: Self = Self(128);

    pub const fn from_units(units: u8) -> Self {
        Self(units)
    }

    pub const fn units(self) -> u8 {
        self.0
    }

    pub fn wrapping_add(self, delta: i8) -> Self {
        Self(self.0.wrapping_add_signed(delta))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(usize);

impl ObjectId {
    pub const fn index(self) -> usize {
        self.0
    }

    pub const fn stable_render_id(self) -> u16 {
        self.0 as u16 + 1
    }
}

/// Identity of one allocation in the object pool.
///
/// `ObjectId` intentionally remains the source-semantic slot index: links,
/// ordering, and gameplay state all use it.  This token is only for the
/// presentation bridge, where a slot reused by a later allocation must not
/// be treated as the same object for interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectLifetimeId {
    slot: ObjectId,
    generation: u32,
}

impl ObjectLifetimeId {
    const SLOT_BITS: u32 = u16::BITS;

    pub const fn slot(self) -> ObjectId {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// Compact presentation identity consumed by the shared draw list.
    pub const fn render_id(self) -> u64 {
        (self.generation as u64) << Self::SLOT_BITS | self.slot.stable_render_id() as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialLoop {
    CapitalEngine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialDistance {
    Close,
    Near,
    Far,
    Distant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StereoPosition {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpatialSound {
    pub source: ObjectId,
    pub sound: SpatialLoop,
    pub distance: SpatialDistance,
    pub position: StereoPosition,
}

/// Index into the generated, decoded shape catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShapeId(u16);

impl ShapeId {
    /// Moving formation effect used behind the title craft.
    pub const TITLE_FORMATION_EFFECT: Self = Self(64);

    /// Craft mesh used by the three-ship title flyby.
    pub const TITLE_CRAFT: Self = Self(89);

    /// The decoded empty catalog entry used while an object has no geometry.
    ///
    /// This is a semantic no-shape state: the source entry has no vertices or
    /// faces, and native gameplay keeps objects in this state non-visible.
    pub const EMPTY: Self = Self(0);

    /// Full-size craft meshes selected by the retail active-flight class table.
    pub const FOX_FALCO_FLIGHT_CRAFT: Self = Self(52);
    pub const PEPPY_SLIPPY_FLIGHT_CRAFT: Self = Self(53);
    pub const MIYU_FAY_FLIGHT_CRAFT: Self = Self(85);

    /// The two animated folding meshes for each pilot class. Transforming to
    /// Walker traverses the flight-side mesh first and the walker-side mesh
    /// second; transforming back traverses them in reverse.
    pub const FOX_FALCO_WALKER_SIDE_TRANSITION: Self = Self(54);
    pub const FOX_FALCO_FLIGHT_SIDE_TRANSITION: Self = Self(55);
    pub const PEPPY_SLIPPY_WALKER_SIDE_TRANSITION: Self = Self(58);
    pub const PEPPY_SLIPPY_FLIGHT_SIDE_TRANSITION: Self = Self(59);
    pub const MIYU_FAY_WALKER_SIDE_TRANSITION: Self = Self(56);
    pub const MIYU_FAY_FLIGHT_SIDE_TRANSITION: Self = Self(57);

    /// Large craft used by the certified first-sortie entry formation.
    pub const ENTRY_LARGE_CRAFT: Self = Self(524);

    /// Companion entry craft used by the same formation.
    pub const ENTRY_FORMATION_CRAFT: Self = Self(415);

    /// Player rapid-shot meshes, in their observed launch order.
    pub const PLAYER_RAPID_LASER_LAUNCH: Self = Self(359);
    pub const PLAYER_RAPID_LASER_EXPANDED: Self = Self(138);
    pub const PLAYER_RAPID_LASER_FAST: Self = Self(406);
    pub const PLAYER_RAPID_LASER_DISTANT: Self = Self(407);

    /// Charge orb held at the player's muzzle before the charged shot exists.
    pub const PLAYER_CHARGE_ORB_BUILDING: Self = Self(15);
    pub const PLAYER_CHARGE_ORB_READY: Self = Self(17);

    /// Distinct travelling charged-shot meshes.
    pub const PLAYER_CHARGED_LASER_LAUNCH: Self = Self(126);
    pub const PLAYER_CHARGED_LASER_ACTIVE: Self = Self(127);

    /// Enemy laser used by the opening-sortie encounter craft.
    pub const ENEMY_LASER: Self = Self(357);

    /// Campaign missile mesh used by the timed Corneria interception sortie.
    pub const CAMPAIGN_MISSILE: Self = Self(181);

    /// Enemy craft used by the three-fighter defense after the interception.
    pub const INTERCEPT_FIGHTER: Self = Self(486);

    /// The two semantic craft classes in the recurring four-attacker pressure
    /// encounter. They use decoded catalog meshes, not source shape tokens.
    pub const PRESSURE_ASSAULT_FIGHTER: Self = Self(486);
    pub const PRESSURE_STRIKE_FIGHTER: Self = Self(415);

    /// Pigma's Wolfen craft used by the first Star Wolf duel.
    pub const PIGMA_CRAFT: Self = Self(61);

    /// Leon's Wolfen craft used by the Astropolis Star Wolf duel. Both rival
    /// craft share the decoded mesh catalog entry but remain semantic shapes
    /// in the native game state.
    pub const LEON_CRAFT: Self = Self(61);

    /// The last recurring Wolfen pursuer. It shares the standard decoded
    /// Wolfen mesh while remaining distinct campaign state.
    pub const FINAL_PURSUER_CRAFT: Self = Self(61);

    /// Upgraded Wolfen allocated by the final gate after every ordinary
    /// strategic threat has retired.
    pub const WOLF_BLOCKADE_CRAFT: Self = Self(380);

    /// Armored head of Mirage Dragon. The articulated body is represented by
    /// typed segment objects that follow the boss path.
    pub const MIRAGE_DRAGON_HEAD: Self = Self(339);
    pub const MIRAGE_DRAGON_BODY: Self = Self(341);
    pub const MIRAGE_DRAGON_TAIL: Self = Self(343);

    /// Five large hull sections forming the exterior Battle Carrier model.
    pub const CARRIER_HULL_AFT_PORT: Self = Self(384);
    pub const CARRIER_HULL_FORWARD_PORT: Self = Self(387);
    pub const CARRIER_HULL_CENTER: Self = Self(390);
    pub const CARRIER_HULL_FORWARD_STARBOARD: Self = Self(393);
    pub const CARRIER_HULL_AFT_STARBOARD: Self = Self(396);

    /// Pilot walker meshes selected by the retail six-entry pilot table.
    pub const FOX_FALCO_WALKER: Self = Self(114);
    pub const PEPPY_SLIPPY_WALKER: Self = Self(116);
    pub const MIYU_FAY_WALKER: Self = Self(115);

    /// Repeating side-wall section used by the Battle Carrier corridor.
    pub const CARRIER_CORRIDOR_WALL: Self = Self(197);

    /// Central doorway at the end of the Battle Carrier corridor.
    pub const CARRIER_CORRIDOR_DOOR: Self = Self(204);
    pub const CARRIER_CORRIDOR_SIDE_DOOR: Self = Self(426);

    /// Midship and reactor control assemblies on the Battle Carrier rail.
    /// Each family has its own decoded active, transition, and settled mesh.
    pub const CARRIER_MIDSHIP_CONTROL_ACTIVE: Self = Self(194);
    pub const CARRIER_MIDSHIP_CONTROL_ACTIVATING: Self = Self(193);
    pub const CARRIER_MIDSHIP_CONTROL_COMPLETE: Self = Self(490);
    pub const CARRIER_REACTOR_CONTROL_ACTIVE: Self = Self(410);
    pub const CARRIER_REACTOR_CONTROL_ACTIVATING: Self = Self(409);
    pub const CARRIER_REACTOR_CONTROL_COMPLETE: Self = Self(230);

    /// The central bulkhead, its paired rotating leaves, and the nested wall
    /// sections visible along the carrier rail.
    pub const CARRIER_MIDSHIP_BULKHEAD: Self = Self(506);
    pub const CARRIER_ROTATING_DOOR_LEAF: Self = Self(205);
    pub const CARRIER_BULKHEAD_INNER_WALL: Self = Self(151);
    pub const CARRIER_BULKHEAD_OUTER_WALL: Self = Self(236);

    /// One-hit sentry that crosses the Battle Carrier's central rail before
    /// firing a three-shot volley and withdrawing into the wall.
    pub const CARRIER_CORRIDOR_DEFENDER: Self = Self(452);

    /// Door and wall sections enclosing the Battle Carrier reactor room.
    pub const CARRIER_REACTOR_ENTRY: Self = Self(195);
    pub const CARRIER_REACTOR_REAR_WALL: Self = Self(235);
    pub const CARRIER_REACTOR_SIDE_WALL: Self = Self(237);

    /// Energy-core assembly and rotating overhead coupling in the Battle
    /// Carrier reactor room.
    pub const CARRIER_REACTOR_CORE: Self = Self(142);
    pub const CARRIER_REACTOR_ROTATING_COUPLING: Self = Self(141);

    /// Intact rotating armor panel protecting the carrier core.
    pub const CARRIER_REACTOR_PANEL: Self = Self(143);

    /// Broken panel mesh shown after ten effective hits.
    pub const CARRIER_REACTOR_PANEL_DESTROYED: Self = Self(144);

    /// Paired defensive structures flanking Eladard's surface entrance.
    pub const ELADARD_SURFACE_BARRIER: Self = Self(244);

    /// Frame and destructible core of Eladard's interior generator.
    pub const ELADARD_GENERATOR_FRAME: Self = Self(427);
    pub const ELADARD_GENERATOR_CORE: Self = Self(428);

    /// The access-room pressure switch and its depressed mesh.
    pub const ELADARD_ACCESS_SWITCH_ACTIVE: Self = Self(464);
    pub const ELADARD_ACCESS_SWITCH_PRESSED: Self = Self(465);

    /// Closed and open meshes shared by Eladard's two interior doors.
    pub const ELADARD_INTERIOR_DOOR_CLOSED: Self = Self(434);
    pub const ELADARD_INTERIOR_DOOR_OPEN: Self = Self(435);

    /// Paired pop-up defenders in Eladard's first interior chamber.
    pub const ELADARD_INTERIOR_DEFENDER: Self = Self(452);

    /// Structural meshes used by Eladard's two retail interior chambers.
    pub const ELADARD_ACCESS_REAR_STRUCTURE: Self = Self(429);
    pub const ELADARD_ACCESS_WEST_WALL: Self = Self(234);
    pub const ELADARD_ACCESS_EAST_WALL: Self = Self(196);
    pub const ELADARD_ACCESS_REAR_CORNER: Self = Self(149);
    pub const ELADARD_INTERIOR_CORNER_WALL: Self = Self(236);
    pub const ELADARD_INTERIOR_WALL_PANEL: Self = Self(151);
    pub const ELADARD_ACCESS_CENTER_STRUCTURE: Self = Self(200);
    pub const ELADARD_TRANSIT_GATEHOUSE: Self = Self(191);
    pub const ELADARD_TRANSIT_CONNECTOR: Self = Self(194);
    pub const ELADARD_TRANSIT_LONG_WALL: Self = Self(532);
    pub const ELADARD_TRANSIT_CEILING: Self = Self(153);
    pub const ELADARD_TRANSIT_TOWER: Self = Self(514);

    /// Titania's exterior and final interior pressure switches. These names
    /// keep the shipping runtime semantic while the values select decoded
    /// catalog entries at the data boundary.
    pub const TITANIA_SWITCH_ACTIVE: Self = Self(464);
    pub const TITANIA_SWITCH_PRESSED: Self = Self(465);

    /// Raised transit structure between Titania's two exterior switch areas.
    pub const TITANIA_ROUTE_LIFT: Self = Self(450);

    /// Central Titania installation before and after both exterior switches
    /// have opened its east-side entrance.
    pub const TITANIA_BASE_CLOSED: Self = Self(238);
    pub const TITANIA_BASE_OPEN: Self = Self(239);

    /// Macbeth surface objectives and installation structures. These values
    /// are decoded shape-catalog indices; the source header addresses remain
    /// confined to extraction and oracle tooling.
    pub const MACBETH_SWITCH_ACTIVE: Self = Self(464);
    pub const MACBETH_SWITCH_PRESSED: Self = Self(465);
    pub const MACBETH_DEFENSE_TOWER: Self = Self(571);
    pub const MACBETH_TOWER_GUN: Self = Self(564);
    pub const MACBETH_INSTALLATION_OPEN: Self = Self(239);
    pub const MACBETH_KNIGHT: Self = Self(410);
    pub const MACBETH_FIRE_BARRIER: Self = Self(234);
    pub const MACBETH_INTERIOR_GATE: Self = Self(191);
    pub const MACBETH_INTERIOR_MARKER: Self = Self(434);
    pub const MACBETH_CORE_CONTROLLER: Self = Self(497);
    pub const MACBETH_CORE_SHIELD: Self = Self(498);
    pub const MACBETH_CORE_TURRET_HEAD: Self = Self(499);
    pub const MACBETH_CORE_TURRET: Self = Self(500);
    pub const MACBETH_CORE: Self = Self(428);

    /// Fortuna mission meshes selected through the flat decoded catalog. The
    /// installation shares retail geometry with other planetary bases, while
    /// its guardian has a dedicated catalog entry.
    pub const FORTUNA_SWITCH_ACTIVE: Self = Self(464);
    pub const FORTUNA_SWITCH_PRESSED: Self = Self(465);
    pub const FORTUNA_INSTALLATION_OPEN: Self = Self(239);
    pub const FORTUNA_KICK_GUNNER: Self = Self(416);
    /// The guardian's linked weapon mount and its projectile use decoded
    /// catalog entries with no polygon geometry. They remain distinct typed
    /// collision objects in the native game.
    pub const FORTUNA_KICK_GUNNER_MOUNT: Self = Self(20);
    pub const FORTUNA_KICK_GUNNER_PROJECTILE: Self = Self(44);
    pub const FORTUNA_INTERIOR_DOORWAY: Self = Self(191);
    pub const FORTUNA_CORE_CONTROLLER: Self = Self(497);
    pub const FORTUNA_CORE_SHIELD: Self = Self(498);
    pub const FORTUNA_CORE_TURRET_HEAD: Self = Self(499);
    pub const FORTUNA_CORE_TURRET: Self = Self(500);
    pub const FORTUNA_CORE: Self = Self(428);

    /// Venom base mission meshes selected from the decoded flat catalog.
    /// Shared installations deliberately reuse shared retail geometry while
    /// retaining mission-semantic names in the native runtime.
    pub const VENOM_SWITCH_ACTIVE: Self = Self(464);
    pub const VENOM_SWITCH_PRESSED: Self = Self(465);
    pub const VENOM_INSTALLATION_OPEN: Self = Self(239);
    pub const VENOM_INTERIOR_DOOR_CLOSED: Self = Self(434);
    pub const VENOM_INTERIOR_DOOR_OPEN: Self = Self(435);
    pub const VENOM_KNIGHT: Self = Self(410);
    pub const VENOM_REACTOR_PARENT: Self = Self(427);
    pub const VENOM_REACTOR_CORE: Self = Self(428);

    pub const fn from_catalog_index(index: u16) -> Self {
        Self(index)
    }

    pub const fn catalog_index(self) -> usize {
        self.0 as usize
    }

    /// Disjoint flat id used by the shared SF1/SF2 renderer. This is derived
    /// solely from the decoded catalog index, never from a source address.
    pub const fn flat_render_id(self) -> u16 {
        sf_core::shape::sf2_shape_id(self.0)
    }

    pub fn catalog_entry(self) -> Option<&'static ShapeDataEntry> {
        SHAPE_DATA.get(self.catalog_index())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathId(u16);

impl PathId {
    pub const fn from_catalog_index(index: u16) -> Self {
        Self(index)
    }

    pub const fn catalog_index(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathCursor {
    pub path: PathId,
    pub command_index: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Player,
    Wingmate,
    Enemy,
    Projectile,
    Scenery,
    Effect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behavior {
    MissionEntryFlyby,
    PlayerSelection,
    PlayerFlight,
    FollowPath,
    /// Source PATHHOLD installs the movement service without path dispatch.
    PathMovement,
    EnemyFlight,
    Projectile,
    Effect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponKind {
    None,
    Laser,
    ChargedLaser,
    NovaBomb,
    EnemyLaser,
    Missile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionClass {
    None,
    Player,
    Enemy,
    PlayerWeapon,
    EnemyWeapon,
    Scenery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterWaveDirection {
    Forward,
    Reverse,
}

impl FighterWaveDirection {
    pub fn advance(self, phase: Angle, step: u8) -> Angle {
        match self {
            Self::Forward => phase.wrapping_add(step as i8),
            Self::Reverse => phase.wrapping_add(-(step as i8)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterWavePolarity {
    Standard,
    Mirrored,
}

impl FighterWavePolarity {
    pub const fn apply(self, sample: i8) -> i8 {
        match self {
            Self::Standard => sample,
            Self::Mirrored => -sample,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterWaveOrder {
    BeforeSteering,
    AfterSteering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterCenteringTargetOrder {
    BeforeSteering,
    AfterSteering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterAltitudePhase {
    Wave,
    Centering { ticks_remaining: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterLogicCadence {
    EntryChase,
    Combat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FighterAngles {
    pub pitch: Angle,
    pub yaw: Angle,
    pub roll: Angle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterWeaponPhase {
    Ready,
    Restoring {
        flight_angles: FighterAngles,
        ticks_remaining: u8,
    },
}

impl Default for FighterWeaponPhase {
    fn default() -> Self {
        Self::Ready
    }
}

/// Typed flight variables used by the opening-sortie fighter behavior. The
/// values are gameplay concepts: a vertical wave, a maneuver bank, two
/// activity timers, and the short weapon-aim phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FighterFlightState {
    pub logic_credit: u8,
    pub logic_cadence: FighterLogicCadence,
    pub vertical_wave_phase: Angle,
    pub vertical_pitch_target: Angle,
    pub vertical_wave_direction: FighterWaveDirection,
    pub vertical_wave_polarity: FighterWavePolarity,
    pub vertical_wave_order: FighterWaveOrder,
    pub centering_target_order: FighterCenteringTargetOrder,
    pub pending_velocity: Vector3,
    pub pending_vertical_displacement: i16,
    pub altitude_phase: FighterAltitudePhase,
    pub maneuver_bank: Angle,
    pub maneuver_ticks_remaining: u8,
    pub fire_ticks_remaining: u8,
    pub weapon_phase: FighterWeaponPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReengagementFighterMovementPhase {
    Ready,
    HorizontalApplied,
}

/// Flat, typed flight variables for the fighters in the first strategic-map
/// re-engagement. These are gameplay concepts rather than an emulated actor
/// record: vertical wave progress, maneuver steering, altitude centering, and
/// the one cooperative movement continuation exposed by retail presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReengagementFighterFlightState {
    pub vertical_wave_phase: Angle,
    pub vertical_wave_sample: i8,
    pub vertical_wave_quarters_applied: u8,
    pub vertical_pitch_target: Angle,
    pub maneuver_bank: Angle,
    pub altitude_phase: FighterAltitudePhase,
    pub pending_velocity: Vector3,
    pub movement_phase: ReengagementFighterMovementPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterInterceptMovementPhase {
    Ready,
    HorizontalApplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterInterceptWeaponPhase {
    Flight,
    Aiming { flight_pitch: Angle },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterInterceptCombatPhase {
    ScriptedFlight,
    StraightApproach { ticks_elapsed: u8 },
    BankingForAttack { target_bank: Angle },
    BankedApproach { ticks_elapsed: u8 },
    LevelingForAttack,
    Attacking { fire_counter: u8 },
    BankingForDeparture { target_bank: Angle },
    DepartureArc,
    LevelingForApproach,
}

/// Flat, typed flight variables for fighter interception and recurring
/// attacks. The maneuver fields are world-space drift and altitude targets;
/// the combat phase is the craft's ordinary attack/departure state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FighterInterceptFlightState {
    pub logic_credit: u8,
    /// Semantic strategy clock used by scheduled combat actions.
    pub strategy_frame: u8,
    pub combat_phase: FighterInterceptCombatPhase,
    pub vertical_wave_phase: Angle,
    pub cruise_target_speed: u8,
    pub cruise_acceleration: u8,
    pub maneuver_drift_x: i16,
    pub maneuver_altitude_target: i16,
    pub maneuver_drift_z: i16,
    pub pending_velocity: Vector3,
    pub movement_phase: FighterInterceptMovementPhase,
    pub weapon_phase: FighterInterceptWeaponPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterceptionMissileSteering {
    Straight,
    Climb,
    Dive,
    Clockwise,
    CounterClockwise,
}

/// Flat, typed flight variables for the three strategic campaign missiles.
/// Their world transform remains in the ordinary object fields; this records
/// the most recent steering adjustment selected by the maneuver sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterceptionMissileFlightState {
    pub last_steering_adjustment: InterceptionMissileSteering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapitalMovementPhase {
    Ready,
    HorizontalApplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapitalFlightAngles {
    pub pitch: Angle,
    pub yaw: Angle,
    pub roll: Angle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapitalWeaponPhase {
    Ready,
    Aiming { flight_angles: CapitalFlightAngles },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapitalFlightState {
    pub vertical_wave_phase: Angle,
    pub previous_player_position: Vector3,
    pub two_ticks_ago_player_position: Vector3,
    pub pending_velocity: Vector3,
    pub movement_phase: CapitalMovementPhase,
    pub weapon_phase: CapitalWeaponPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostileProjectileFlightPhase {
    Homing,
    AimCorrection,
    Cruise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostileProjectileMovementPhase {
    Ready,
    TargetContractionPending { altitude: i16, depth: i16 },
}

/// Flat, typed flight variables shared by hostile mission projectiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostileProjectileFlightState {
    pub phase: HostileProjectileFlightPhase,
    pub motion_steps_elapsed: u16,
    pub movement_phase: HostileProjectileMovementPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PigmaRivalFlightPhase {
    AwaitingEntrance,
    Approach,
    CombatManeuver,
    Attack,
    SecondApproach,
    Deceleration,
    Escape,
}

/// Flat, typed flight variables for Pigma's first Star Wolf duel.
///
/// The transform remains in the ordinary object fields. These values are the
/// authored maneuver phase, speed approach, wave progress, and player-height
/// history needed when two cooperative path steps share one presentation
/// tick; they are not an emulated actor record or byte-addressed memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PigmaRivalFlightState {
    pub phase: PigmaRivalFlightPhase,
    pub target_speed: u8,
    pub acceleration: u8,
    pub motion_steps_elapsed: u16,
    pub second_approach_wave_step: u8,
    pub escape_wobble_step: u8,
    pub earlier_player_altitude: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeonRivalFlightPhase {
    AwaitingEntrance,
    Approach,
    CombatManeuver,
    Attack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeonRivalMovementPhase {
    Ready,
    PreparedAdvance,
}

/// Flat, typed flight variables for Leon's campaign duel.
///
/// Position, orientation, and speed remain in the ordinary object fields;
/// this holds only authored maneuver state that is not part of the transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeonRivalFlightState {
    pub phase: LeonRivalFlightPhase,
    pub movement_phase: LeonRivalMovementPhase,
    pub target_speed: u8,
    pub acceleration: u8,
    pub motion_steps_elapsed: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalRivalFlightPhase {
    AwaitingEntrance,
    Approach,
    CombatManeuver,
    Attack,
    Departure,
}

/// Flat, typed flight variables shared by the recurring final pursuer and
/// the upgraded Wolf blockade craft. Transform and combat values remain in
/// the ordinary object fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinalRivalFlightState {
    pub phase: FinalRivalFlightPhase,
    pub target_speed: u8,
    pub acceleration: u8,
    pub motion_steps_elapsed: u16,
}

/// Native movement scheduler for Meteor's Queen Dragoon. The visible body
/// transform stays in [`ObjectBase`]; these fields only retain progress
/// through the retail 7, 7, 8-frame movement cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueenDragoonFlightState {
    pub retail_frame_credit: u8,
    pub cadence_index: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerProjectileKind {
    Rapid,
    Charged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerProjectileState {
    pub kind: PlayerProjectileKind,
    pub age_ticks: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerChargeOrbPhase {
    Building,
    Ready,
    Releasing { ticks_remaining: u8, ready: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerChargeOrbState {
    pub phase: PlayerChargeOrbPhase,
    pub age_ticks: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EladardDefenderPhase {
    VolleyMotion {
        next_motion_step: u8,
        retail_frame_accumulator: u8,
    },
    Cooldown {
        retail_frames_remaining: u16,
    },
}

/// Flat behavior state for an Eladard interior defender. Position, aim, and
/// combat values remain in the ordinary object fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EladardDefenderState {
    pub phase: EladardDefenderPhase,
}

/// Lifetime state for a laser fired by an Eladard interior defender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EladardDefenderProjectileState {
    pub age_retail_frames: u8,
}

/// Lifetime state for a laser emitted by Fortuna's exposed core assembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FortunaCoreProjectileState {
    pub age_retail_frames: u16,
}

/// Lifetime state for a shot fired by Fortuna's interior guardian.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FortunaKickGunnerProjectileState {
    pub age_retail_frames: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierCorridorDefenderPhase {
    Crossing {
        lateral_steps_remaining: u8,
        retail_frame_accumulator: u8,
    },
    Volley {
        elapsed_retail_frames: u16,
        shots_fired: u8,
    },
    Withdrawing {
        lateral_steps_remaining: u8,
        retail_frame_accumulator: u8,
    },
}

/// Flat behavior state for one Battle Carrier rail sentry. The signed lateral
/// step is an ordinary world-space direction; no source-machine object window
/// or strategy address is retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarrierCorridorDefenderState {
    pub phase: CarrierCorridorDefenderPhase,
    pub lateral_step: i16,
}

/// Lifetime state for a laser fired by a Battle Carrier rail sentry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarrierCorridorProjectileState {
    pub age_retail_frames: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirageDragonHeadPhase {
    AwaitingEntrance,
    Following,
    Departing,
}

/// Flat behavior state for Mirage Dragon's head. Its ordinary object
/// transform and velocity hold the live path state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MirageDragonHeadState {
    pub phase: MirageDragonHeadPhase,
    pub departure_motion_updates: u16,
    pub departure_turn_updates: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirageDragonSegmentPhase {
    AwaitingEntrance,
    Entering,
    Following,
    Departing,
}

/// Flat behavior state for one articulated Mirage Dragon part. The ordinary
/// object transform holds its live pose, `linked_object` names its stable
/// predecessor, and `parent` names the encounter head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MirageDragonSegmentState {
    pub ordinal: u8,
    pub authored_depth: i8,
    pub phase: MirageDragonSegmentPhase,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ObjectActivity {
    #[default]
    None,
    FighterFlight(FighterFlightState),
    ReengagementFighterFlight(ReengagementFighterFlightState),
    FighterInterceptFlight(FighterInterceptFlightState),
    InterceptionMissileFlight(InterceptionMissileFlightState),
    CapitalFlight(CapitalFlightState),
    HostileProjectileFlight(HostileProjectileFlightState),
    PigmaRivalFlight(PigmaRivalFlightState),
    LeonRivalFlight(LeonRivalFlightState),
    FinalRivalFlight(FinalRivalFlightState),
    QueenDragoonFlight(QueenDragoonFlightState),
    EladardDefender(EladardDefenderState),
    EladardDefenderProjectile(EladardDefenderProjectileState),
    FortunaCoreProjectile(FortunaCoreProjectileState),
    FortunaKickGunnerProjectile(FortunaKickGunnerProjectileState),
    CarrierCorridorDefender(CarrierCorridorDefenderState),
    CarrierCorridorProjectile(CarrierCorridorProjectileState),
    MirageDragonHead(MirageDragonHeadState),
    MirageDragonSegment(MirageDragonSegmentState),
    PlayerProjectile(PlayerProjectileState),
    PlayerChargeOrb(PlayerChargeOrbState),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ObjectFlags {
    pub active: bool,
    pub visible: bool,
    /// Last draw-list admission observation (source 08 bit 10), not the
    /// authored invisibility controls. Initialized before the first draw,
    /// cleared during list preparation, set by `$03:85E4` for admitted actors.
    pub draw_list_admitted: bool,
    /// Source 22 bit 04 admits an actor to general searches and bulk cleanup.
    /// Shape-specific searches have their own selection rules.
    pub general_search_eligible: bool,
    pub scaled_sprite: bool,
    pub exploding: bool,
    pub on_fire: bool,
    pub casts_shadow: bool,
    /// Set for the one native simulation tick in which contact occurred.
    pub collided: bool,
    pub collision_disabled: bool,
    /// Source transient-effect flag: retire this object when the final free
    /// slot is consumed, provided the pressure traversal reaches it.
    pub reclaim_on_pool_pressure: bool,
    /// Attached child is marked for deferred removal when its owner retires
    /// (`$7F:34B5..34C3`). Detachment alone does not free the child.
    pub remove_with_parent: bool,
    pub remove_after_tick: bool,
}

/// Typed counterpart of the original base object record. Fields follow the
/// original conceptual order: list links, shape/attachment, state, transform,
/// behavior, interaction links, behavior data, path/combat data, and motion.
/// The source's separately indexed extension record follows in [`Object`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectBase {
    pub next: Option<ObjectId>,
    pub previous: Option<ObjectId>,
    pub shape: ShapeId,
    pub attachment: Option<ObjectId>,
    /// Authored sibling identifier (source base 13), independent of pitch.
    pub child_number: u8,
    pub flags: ObjectFlags,
    pub kind: ObjectKind,
    pub explosion_timer: u8,
    pub general_timer: u8,
    pub position: Vector3,
    pub pitch: Angle,
    pub yaw: Angle,
    pub roll: Angle,
    pub speed: u8,
    /// Authored speed target; contact callbacks read this same byte as their
    /// other-actor parameter (source 0A), not a separate contact value.
    pub target_speed: u8,
    pub acceleration: u8,
    pub behavior: Behavior,
    pub linked_object: Option<ObjectId>,
    pub first_child: Option<ObjectId>,
    pub next_sibling: Option<ObjectId>,
    pub wait_timer: u8,
    pub behavior_phase: u8,
    pub behavior_parameter: i16,
    pub path: Option<PathCursor>,
    pub hit_points: u8,
    pub attack_power: u8,
    pub weapon: WeaponKind,
    pub collision_delay: u8,
    pub collision_class: CollisionClass,
    pub velocity: Vector3,
    pub hit_flags: u8,
    pub contacts: super::collision_pass::ActorContacts,
}

/// Typed counterpart of the original parallel object-extension record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ObjectExtension {
    pub path_state: super::path_runtime::ActorPathState,
    /// Scene-owned snapshot; retirement detaches it before contact callbacks.
    pub scene_proxy: Option<super::scene_proxy::SceneProxyId>,
    pub depth_offset: u8,
    pub color_frame: u8,
    pub animation_frame: u8,
    pub material_set: Option<MaterialSetId>,
    pub relative_position: Vector3,
    pub relative_rotation: super::render::Rotation,
    pub parent: Option<ObjectId>,
    /// Allocation group's byte identity (source 1CF0). Child-producing paths
    /// inherit it from their caller; `$0D:D8DD` retires a matching group.
    pub spawn_group: u8,
    pub texture_scroll_x: u8,
    pub texture_scroll_y: u8,
    pub spatial_loop: Option<SpatialLoop>,
    pub activity: ObjectActivity,
    pub auxiliary_links: Vec<ObjectId>,
    pub render_parameter: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    pub base: ObjectBase,
    pub extension: ObjectExtension,
}

/// Live world inputs sampled by the source fresh-object initializer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ObjectSpawnDefaults {
    /// Shared initializer mode (source 1B84 bit 02), not current pause state.
    pub run_when_paused: bool,
    /// Current allocation group (source 190E).
    pub group: u8,
}

impl Object {
    /// Source fresh-record defaults (`$7F:29BC..2A16`), then the caller's
    /// decoded shape and assigned behavior. List insertion remains the pool's
    /// responsibility; this constructor never copies an old actor's state.
    ///
    /// The ordinary constructor is retained for the existing higher-level
    /// strategies, which do not yet all use source initialization semantics.
    pub fn new_authored(
        kind: ObjectKind,
        shape: ShapeId,
        behavior: Behavior,
        defaults: ObjectSpawnDefaults,
    ) -> Self {
        let mut object = Self::new(kind, shape, behavior);
        object.base.flags.draw_list_admitted = true;
        object.base.flags.general_search_eligible = true;
        object.base.contacts.run_when_paused = defaults.run_when_paused;
        object.extension.path_state.hold_latched = true;
        object.extension.spawn_group = defaults.group;
        object
    }

    pub fn new(kind: ObjectKind, shape: ShapeId, behavior: Behavior) -> Self {
        Self {
            base: ObjectBase {
                next: None,
                previous: None,
                shape,
                attachment: None,
                child_number: 0,
                flags: ObjectFlags {
                    active: true,
                    visible: true,
                    ..ObjectFlags::default()
                },
                kind,
                explosion_timer: 0,
                general_timer: 0,
                position: Vector3::default(),
                pitch: Angle::ZERO,
                yaw: Angle::ZERO,
                roll: Angle::ZERO,
                speed: 0,
                target_speed: 0,
                acceleration: 0,
                behavior,
                linked_object: None,
                first_child: None,
                next_sibling: None,
                wait_timer: 0,
                behavior_phase: 0,
                behavior_parameter: 0,
                path: None,
                hit_points: 0,
                attack_power: 0,
                weapon: WeaponKind::None,
                collision_delay: 0,
                collision_class: CollisionClass::None,
                velocity: Vector3::default(),
                hit_flags: 0,
                contacts: super::collision_pass::ActorContacts {
                    first_strategy_visit: true,
                    ..super::collision_pass::ActorContacts::default()
                },
            },
            extension: ObjectExtension::default(),
        }
    }
}

/// Fixed-capacity typed object pool. IDs are stable slot indices, and
/// allocation preserves the original game's ascending initial slot order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStore {
    slots: Vec<Option<Object>>,
    free: Vec<ObjectId>,
    active: Vec<ObjectId>,
    generations: Vec<u32>,
}

impl ObjectStore {
    pub fn new() -> Self {
        let free = (0..OBJECT_CAPACITY).rev().map(ObjectId).collect();
        Self {
            slots: vec![None; OBJECT_CAPACITY],
            free,
            active: Vec::with_capacity(OBJECT_CAPACITY),
            generations: vec![0; OBJECT_CAPACITY],
        }
    }

    pub fn allocate(&mut self, object: Object) -> Option<ObjectId> {
        self.allocate_after(None, object)
    }

    /// Allocate at the head, or directly after a live actor. Authored child
    /// spawns use the latter so an in-progress traversal can visit the child
    /// before the spawner's former successor. An invalid anchor changes nothing.
    pub fn allocate_after(&mut self, after: Option<ObjectId>, object: Object) -> Option<ObjectId> {
        self.allocate_with_pressure_head(after, None, object)
    }

    /// Shared weapon allocation (`$0D:E017`). Insert after the firing actor;
    /// while allocating, the source temporarily makes that actor the list
    /// head, so the last-slot pressure sweep visits only its suffix.
    pub fn allocate_weapon_after(&mut self, source: ObjectId, object: Object) -> Option<ObjectId> {
        self.allocate_scoped_after(source, object)
    }

    /// Path and weapon spawners temporarily scope the allocation head to
    /// their caller. Insert immediately after it and limit a last-slot
    /// pressure sweep to that suffix; never persist a different global head.
    pub fn allocate_scoped_after(&mut self, source: ObjectId, object: Object) -> Option<ObjectId> {
        self.allocate_with_pressure_head(Some(source), Some(source), object)
    }

    fn allocate_with_pressure_head(
        &mut self,
        after: Option<ObjectId>,
        pressure_head: Option<ObjectId>,
        object: Object,
    ) -> Option<ObjectId> {
        let position = match after {
            Some(id) => self.active.iter().position(|candidate| *candidate == id)? + 1,
            None => 0,
        };
        let id = self.free.pop()?;
        let generation = &mut self.generations[id.index()];
        *generation = generation.wrapping_add(1).max(1);
        let next = self.active.get(position).copied();
        self.slots[id.index()] = Some(object);
        if let Some(value) = self.slots[id.index()].as_mut() {
            value.base.previous = after;
            value.base.next = next;
        }
        if let Some(next) = next {
            if let Some(value) = self.slots[next.index()].as_mut() {
                value.base.previous = Some(id);
            }
        }
        if let Some(after) = after {
            self.slots[after.index()].as_mut().unwrap().base.next = Some(id);
        }
        self.active.insert(position, id);
        if self.free.is_empty() {
            self.mark_pressure_retirements(pressure_head, id);
        }
        Some(id)
    }

    /// `$7F:295D..29BB`: consuming the last free slot marks eligible effects
    /// for the later cleanup pass, but does not reclaim them immediately.
    /// The actual list tail is never tested. The fresh allocation's source
    /// record is cleared after this sweep, so none of its transient flags
    /// survive; do not test its newly initialized native shape here.
    fn mark_pressure_retirements(&mut self, head: Option<ObjectId>, fresh: ObjectId) {
        let first = head.map_or(0, |head| {
            self.active
                .iter()
                .position(|id| *id == head)
                .expect("validated allocation head")
        });
        let end = self.active.len().saturating_sub(1);
        for index in first..end {
            let id = self.active[index];
            if id == fresh {
                continue;
            }
            let object = self.slots[id.index()]
                .as_mut()
                .expect("live allocation list");
            if object.base.flags.reclaim_on_pool_pressure
                || POOL_RECLAIMABLE_SPRITES.contains(&object.base.shape)
            {
                object.base.flags.remove_after_tick = true;
            }
        }
    }

    /// Retire the pool-owned record and its object relationships. Other
    /// systems still own their contact entries, path callbacks, and scene
    /// handles; those owners must release them as part of world retirement.
    pub fn remove(&mut self, id: ObjectId) -> Option<Object> {
        self.get(id)?;
        self.detach_relationships(id);
        self.remove_detached(id)
    }

    /// Final pool step, after the world has completed callback-bearing cleanup.
    pub(super) fn remove_detached(&mut self, id: ObjectId) -> Option<Object> {
        let position = self.active.iter().position(|candidate| *candidate == id)?;
        let object = self.slots.get_mut(id.index())?.take()?;
        let previous = object.base.previous;
        let next = object.base.next;
        if let Some(previous) = previous {
            if let Some(value) = self.slots[previous.index()].as_mut() {
                value.base.next = next;
            }
        }
        if let Some(next) = next {
            if let Some(value) = self.slots[next.index()].as_mut() {
                value.base.previous = previous;
            }
        }
        self.active.remove(position);
        self.free.push(id);
        Some(object)
    }

    /// Source `$7F:344F..34E6`, expressed using the native split child/sibling
    /// fields. Removing a child splices it out; removing an owner detaches
    /// its children and marks only those with the authored lifetime flag.
    /// Finally clear incoming interaction/attachment references BEFORE the
    /// slot can be reused. A weapon's reciprocal link is not child ownership.
    pub(super) fn detach_relationships(&mut self, id: ObjectId) {
        let object = self.get(id).expect("validated retiring actor");
        let successor = object.base.next_sibling;
        let mut child = object.base.first_child;
        // Valid source chains are acyclic. Bound malformed imported/native
        // chains by the pool capacity instead of risking an infinite loop.
        for _ in 0..OBJECT_CAPACITY {
            let Some(child_id) = child else { break };
            let object = self
                .get_mut(child_id)
                .expect("child chain references a live actor");
            child = object.base.next_sibling;
            object.base.attachment = None;
            object.extension.parent = None;
            if object.base.flags.remove_with_parent {
                object.base.flags.remove_after_tick = true;
            }
        }
        assert!(child.is_none(), "cyclic child ownership chain");
        for object in self.slots.iter_mut().flatten() {
            if object.base.first_child == Some(id) {
                object.base.first_child = successor;
            }
            if object.base.next_sibling == Some(id) {
                object.base.next_sibling = successor;
            }
            if object.base.linked_object == Some(id) {
                object.base.linked_object = None;
            }
            if object.base.attachment == Some(id) {
                object.base.attachment = None;
            }
            // Some native authored actors keep their attachment owner in
            // the extension's named parent field rather than base attachment.
            if object.extension.parent == Some(id) {
                object.extension.parent = None;
            }
        }
    }

    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.slots.get(id.index())?.as_ref()
    }

    pub fn lifetime_id(&self, id: ObjectId) -> Option<ObjectLifetimeId> {
        self.get(id).map(|_| ObjectLifetimeId {
            slot: id,
            generation: self.generations[id.index()],
        })
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.slots.get_mut(id.index())?.as_mut()
    }

    pub fn active_ids(&self) -> &[ObjectId] {
        &self.active
    }

    pub fn active_objects(&self) -> impl Iterator<Item = (ObjectId, &Object)> {
        self.active
            .iter()
            .copied()
            .filter_map(|id| self.get(id).map(|object| (id, object)))
    }

    pub fn len(&self) -> usize {
        self.active.len()
    }

    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }
}

impl Default for ObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_initialization_sets_only_source_defaults_and_supplied_metadata() {
        for run_when_paused in [false, true] {
            for group in [0, 1, 127, 128, 255] {
                let object = Object::new_authored(
                    ObjectKind::Effect,
                    ShapeId::TITLE_FORMATION_EFFECT,
                    Behavior::FollowPath,
                    ObjectSpawnDefaults {
                        run_when_paused,
                        group,
                    },
                );
                let mut expected = Object::new(
                    ObjectKind::Effect,
                    ShapeId::TITLE_FORMATION_EFFECT,
                    Behavior::FollowPath,
                );
                expected.base.flags.draw_list_admitted = true;
                expected.base.flags.general_search_eligible = true;
                expected.base.contacts.run_when_paused = run_when_paused;
                expected.extension.path_state.hold_latched = true;
                expected.extension.spawn_group = group;
                assert_eq!(object, expected);
                assert!(object.base.contacts.first_strategy_visit);
                assert!(object.base.flags.visible);
                assert_eq!(object.base.position, Vector3::default());
                assert_eq!(object.base.path, None);
                assert_eq!(object.base.hit_points, 0);
            }
        }
    }

    #[test]
    fn authored_reallocation_discards_old_state_and_pool_alone_sets_list_links() {
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(effect()).unwrap();
        let mut old = effect();
        old.base.position.x = 321;
        old.base.velocity.y = -456;
        old.base.hit_points = 128;
        old.base.flags.visible = false;
        old.base.flags.collision_disabled = true;
        old.extension.spawn_group = 255;
        old.extension.relative_position.z = 789;
        old.extension.path_state.motion_phase = u16::MAX;
        let retired = objects.allocate_after(Some(owner), old).unwrap();
        let old_lifetime = objects.lifetime_id(retired).unwrap();
        objects.remove(retired);
        let fresh = Object::new_authored(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::FollowPath,
            ObjectSpawnDefaults::default(),
        );
        let mut expected = fresh.clone();
        let id = objects.allocate_after(Some(owner), fresh).unwrap();
        assert_eq!(id, retired);
        expected.base.previous = Some(owner);
        assert_eq!(objects.get(id), Some(&expected));
        assert_eq!(objects.get(owner).unwrap().base.next, Some(id));
        assert_ne!(objects.lifetime_id(id).unwrap(), old_lifetime);
    }

    fn effect() -> Object {
        Object::new(
            ObjectKind::Effect,
            ShapeId::FOX_FALCO_FLIGHT_CRAFT,
            Behavior::Effect,
        )
    }

    #[test]
    fn pool_allocates_stable_slots_and_maintains_typed_links() {
        let mut objects = ObjectStore::new();
        let first = objects.allocate(effect()).unwrap();
        let second = objects.allocate(effect()).unwrap();
        assert_eq!((first.index(), second.index()), (0, 1));
        assert_eq!(objects.active_ids(), &[second, first]);
        assert_eq!(objects.get(second).unwrap().base.next, Some(first));
        assert_eq!(objects.get(first).unwrap().base.previous, Some(second));

        objects.remove(second).unwrap();
        assert_eq!(objects.active_ids(), &[first]);
        assert_eq!(objects.get(first).unwrap().base.previous, None);
    }

    #[test]
    fn pool_has_the_recovered_sixty_object_capacity() {
        let mut objects = ObjectStore::new();
        for expected in 0..OBJECT_CAPACITY {
            assert_eq!(objects.allocate(effect()).unwrap().index(), expected);
        }
        assert!(objects.allocate(effect()).is_none());
    }

    #[test]
    fn child_insertion_preserves_the_live_cursor_and_both_neighbor_links() {
        let mut objects = ObjectStore::new();
        let tail = objects.allocate(effect()).unwrap();
        let parent = objects.allocate(effect()).unwrap();
        let first_child = objects.allocate_after(Some(parent), effect()).unwrap();
        let second_child = objects.allocate_after(Some(parent), effect()).unwrap();
        assert_eq!(
            objects.active_ids(),
            &[parent, second_child, first_child, tail]
        );
        assert_eq!(objects.get(parent).unwrap().base.next, Some(second_child));
        assert_eq!(
            objects.get(second_child).unwrap().base.previous,
            Some(parent)
        );
        assert_eq!(
            objects.get(second_child).unwrap().base.next,
            Some(first_child)
        );
        assert_eq!(
            objects.get(first_child).unwrap().base.previous,
            Some(second_child)
        );
        assert_eq!(objects.get(first_child).unwrap().base.next, Some(tail));
        assert_eq!(objects.get(tail).unwrap().base.previous, Some(first_child));
        objects.remove(first_child).unwrap();
        let saved = objects.clone();
        assert!(objects
            .allocate_after(Some(first_child), effect())
            .is_none());
        assert_eq!(objects, saved);
        let new_tail = objects.allocate_after(Some(tail), effect()).unwrap();
        assert_eq!(new_tail, first_child);
        assert_eq!(objects.get(tail).unwrap().base.next, Some(new_tail));
        assert_eq!(objects.get(new_tail).unwrap().base.next, None);
    }

    #[test]
    fn pool_reuse_gets_a_distinct_lifetime_identity() {
        let mut objects = ObjectStore::new();
        let first = objects.allocate(effect()).unwrap();
        let first_lifetime = objects.lifetime_id(first).unwrap();
        objects.remove(first).unwrap();
        let replacement = objects.allocate(effect()).unwrap();
        let replacement_lifetime = objects.lifetime_id(replacement).unwrap();

        assert_eq!(
            first, replacement,
            "the source slot is intentionally reused"
        );
        assert_ne!(first_lifetime, replacement_lifetime);
        assert_ne!(first_lifetime.render_id(), replacement_lifetime.render_id());
        assert_eq!(first_lifetime.slot(), replacement_lifetime.slot());
        assert_eq!(replacement_lifetime.generation(), 2);
    }

    #[test]
    fn retirement_clears_incoming_links_before_slot_reuse() {
        let mut objects = ObjectStore::new();
        let target = objects.allocate(effect()).unwrap();
        let unrelated = objects.allocate(effect()).unwrap();
        let mut follower = effect();
        follower.base.attachment = Some(target);
        follower.base.linked_object = Some(target);
        follower.extension.parent = Some(target);
        let follower = objects.allocate(follower).unwrap();
        let mut preserved = effect();
        preserved.base.linked_object = Some(unrelated);
        let preserved = objects.allocate(preserved).unwrap();
        objects.remove(target).unwrap();
        assert_eq!(objects.allocate(effect()), Some(target));
        let follower = objects.get(follower).unwrap();
        assert_eq!(follower.base.attachment, None);
        assert_eq!(follower.base.linked_object, None);
        assert_eq!(follower.extension.parent, None);
        assert_eq!(
            objects.get(preserved).unwrap().base.linked_object,
            Some(unrelated)
        );
    }

    #[test]
    fn retirement_splices_first_middle_and_last_children() {
        for removed_index in 0..3 {
            let mut objects = ObjectStore::new();
            let parent = objects.allocate(effect()).unwrap();
            let children: Vec<_> = (0..3)
                .map(|_| objects.allocate(effect()).unwrap())
                .collect();
            objects.get_mut(parent).unwrap().base.first_child = Some(children[0]);
            for (index, child) in children.iter().copied().enumerate() {
                let object = objects.get_mut(child).unwrap();
                object.extension.parent = Some(parent);
                object.base.next_sibling = children.get(index + 1).copied();
            }
            objects.remove(children[removed_index]).unwrap();
            let expected: Vec<_> = children
                .into_iter()
                .enumerate()
                .filter_map(|(index, child)| (index != removed_index).then_some(child))
                .collect();
            let mut cursor = objects.get(parent).unwrap().base.first_child;
            for child in expected {
                assert_eq!(cursor, Some(child));
                let object = objects.get(child).unwrap();
                assert_eq!(object.extension.parent, Some(parent));
                cursor = object.base.next_sibling;
            }
            assert_eq!(cursor, None);
        }
    }

    #[test]
    fn parent_retirement_detaches_children_and_only_marks_owned_lifetimes() {
        let mut objects = ObjectStore::new();
        let parent = objects.allocate(effect()).unwrap();
        let survivor = objects.allocate(effect()).unwrap();
        let dependent = objects.allocate(effect()).unwrap();
        let grandchild = objects.allocate(effect()).unwrap();
        objects.get_mut(parent).unwrap().base.first_child = Some(survivor);
        objects.get_mut(survivor).unwrap().base.next_sibling = Some(dependent);
        for child in [survivor, dependent] {
            let object = objects.get_mut(child).unwrap();
            object.base.attachment = Some(parent);
            object.extension.parent = Some(parent);
        }
        let object = objects.get_mut(dependent).unwrap();
        object.base.flags.remove_with_parent = true;
        object.base.first_child = Some(grandchild);
        objects.get_mut(grandchild).unwrap().extension.parent = Some(dependent);
        objects.remove(parent).unwrap();
        assert_eq!(objects.len(), 3, "marking is not recursive freeing");
        assert!(!objects.get(survivor).unwrap().base.flags.remove_after_tick);
        assert!(objects.get(dependent).unwrap().base.flags.remove_after_tick);
        for child in [survivor, dependent] {
            let object = objects.get(child).unwrap();
            assert_eq!(object.base.attachment, None);
            assert_eq!(object.extension.parent, None);
        }
        assert_eq!(
            objects.get(grandchild).unwrap().extension.parent,
            Some(dependent)
        );
    }

    #[test]
    fn clearing_a_scene_preserves_lifetime_barrier_for_next_scene() {
        let mut objects = ObjectStore::new();
        let old_scene = objects.allocate(effect()).unwrap();
        let old_lifetime = objects.lifetime_id(old_scene).unwrap();
        for id in objects.active_ids().to_vec() {
            objects.remove(id).unwrap();
        }

        let new_scene = objects.allocate(effect()).unwrap();
        let new_lifetime = objects.lifetime_id(new_scene).unwrap();
        assert_eq!(old_scene, new_scene);
        assert_ne!(old_lifetime, new_lifetime);
    }

    #[test]
    fn final_slot_marks_effects_but_preserves_tail_and_fresh_object() {
        let mut objects = ObjectStore::new();
        let mut sprite = effect();
        sprite.base.shape = POOL_RECLAIMABLE_SPRITES[0];
        let tail = objects.allocate(sprite.clone()).unwrap();
        let sprites: Vec<_> = POOL_RECLAIMABLE_SPRITES
            .into_iter()
            .map(|shape| {
                let mut object = effect();
                object.base.shape = shape;
                objects.allocate(object).unwrap()
            })
            .collect();
        let mut transient = effect();
        transient.base.flags.reclaim_on_pool_pressure = true;
        let transient = objects.allocate(transient).unwrap();
        while objects.len() < OBJECT_CAPACITY - 1 {
            objects.allocate(effect()).unwrap();
        }
        assert!(objects
            .active_objects()
            .all(|(_, object)| !object.base.flags.remove_after_tick));
        let fresh = objects.allocate(sprite).unwrap();
        assert!(sprites.into_iter().chain([transient]).all(|id| objects
            .get(id)
            .unwrap()
            .base
            .flags
            .remove_after_tick));
        assert!(!objects.get(tail).unwrap().base.flags.remove_after_tick);
        assert!(!objects.get(fresh).unwrap().base.flags.remove_after_tick);
        assert_eq!(objects.len(), OBJECT_CAPACITY);
        let saved = objects.clone();
        assert!(objects.allocate(effect()).is_none());
        assert_eq!(
            objects, saved,
            "exhaustion does not perform a second sweep or steal a slot"
        );
        // Only the cleanup owner releases marked objects, and released slots
        // become available in normal last-freed-first order.
        objects.remove(transient).unwrap();
        assert_eq!(objects.allocate(effect()), Some(transient));
    }

    #[test]
    fn weapon_pressure_starts_at_source_and_does_not_visit_earlier_actors() {
        let mut objects = ObjectStore::new();
        let mut transient = effect();
        transient.base.flags.reclaim_on_pool_pressure = true;
        let tail = objects.allocate(transient.clone()).unwrap();
        let later = objects.allocate(transient.clone()).unwrap();
        let source = objects.allocate(transient.clone()).unwrap();
        let earlier = objects.allocate(transient).unwrap();
        while objects.len() < OBJECT_CAPACITY - 1 {
            objects.allocate(effect()).unwrap();
        }
        let old_head = objects.active_ids()[0];
        let projectile = objects.allocate_weapon_after(source, effect()).unwrap();
        assert_eq!(objects.active_ids()[0], old_head);
        assert_eq!(objects.get(source).unwrap().base.next, Some(projectile));
        assert_eq!(objects.get(projectile).unwrap().base.next, Some(later));
        assert_eq!(objects.get(later).unwrap().base.previous, Some(projectile));
        for id in [source, later] {
            assert!(objects.get(id).unwrap().base.flags.remove_after_tick);
        }
        for id in [earlier, tail, projectile] {
            assert!(!objects.get(id).unwrap().base.flags.remove_after_tick);
        }
    }
}
