use sf2_data::shape_data::{ShapeDataEntry, SHAPE_DATA};

use super::render::MaterialSetId;

pub const OBJECT_CAPACITY: usize = 60;

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

    /// Door and wall sections enclosing the Battle Carrier reactor room.
    pub const CARRIER_REACTOR_ENTRY: Self = Self(195);
    pub const CARRIER_REACTOR_REAR_WALL: Self = Self(235);
    pub const CARRIER_REACTOR_SIDE_WALL: Self = Self(237);

    /// Energy-core assembly in the Battle Carrier reactor room.
    pub const CARRIER_REACTOR_CORE: Self = Self(142);

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
    EnemyFlight,
    MissionScriptedProjectile,
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

/// Flat, typed flight variables for the three-fighter interception. The
/// corridor fields are ordinary world-space maneuver targets: lateral drift,
/// altitude, and longitudinal drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FighterInterceptFlightState {
    pub vertical_wave_phase: Angle,
    pub cruise_target_speed: u8,
    pub cruise_acceleration: u8,
    pub corridor_drift_x: i16,
    pub corridor_altitude: i16,
    pub corridor_drift_z: i16,
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
    EladardDefender(EladardDefenderState),
    EladardDefenderProjectile(EladardDefenderProjectileState),
    PlayerProjectile(PlayerProjectileState),
    PlayerChargeOrb(PlayerChargeOrbState),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ObjectFlags {
    pub active: bool,
    pub visible: bool,
    pub exploding: bool,
    pub on_fire: bool,
    pub casts_shadow: bool,
    /// Set for the one native simulation tick in which contact occurred.
    pub collided: bool,
    pub collision_disabled: bool,
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
    pub flags: ObjectFlags,
    pub kind: ObjectKind,
    pub explosion_timer: u8,
    pub general_timer: u8,
    pub position: Vector3,
    pub pitch: Angle,
    pub yaw: Angle,
    pub roll: Angle,
    pub speed: u8,
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
}

/// Typed counterpart of the original parallel object-extension record.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ObjectExtension {
    pub depth_offset: u8,
    pub color_frame: u8,
    pub animation_frame: u8,
    pub material_set: Option<MaterialSetId>,
    pub relative_position: Vector3,
    pub parent: Option<ObjectId>,
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

impl Object {
    pub fn new(kind: ObjectKind, shape: ShapeId, behavior: Behavior) -> Self {
        Self {
            base: ObjectBase {
                next: None,
                previous: None,
                shape,
                attachment: None,
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
}

impl ObjectStore {
    pub fn new() -> Self {
        let free = (0..OBJECT_CAPACITY).rev().map(ObjectId).collect();
        Self {
            slots: vec![None; OBJECT_CAPACITY],
            free,
            active: Vec::with_capacity(OBJECT_CAPACITY),
        }
    }

    pub fn allocate(&mut self, object: Object) -> Option<ObjectId> {
        let id = self.free.pop()?;
        let old_head = self.active.first().copied();
        self.slots[id.index()] = Some(object);
        if let Some(value) = self.slots[id.index()].as_mut() {
            value.base.next = old_head;
        }
        if let Some(old_head) = old_head {
            if let Some(value) = self.slots[old_head.index()].as_mut() {
                value.base.previous = Some(id);
            }
        }
        self.active.insert(0, id);
        Some(id)
    }

    pub fn remove(&mut self, id: ObjectId) -> Option<Object> {
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

    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.slots.get(id.index())?.as_ref()
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
}
