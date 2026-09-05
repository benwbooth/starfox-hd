//! The opening flyby's articulated chain, authored controls and departure.
//! Links are object identities; local drift and published world motion remain
//! distinct even after a segment takes responsibility for its own transform.

use super::game::flight_velocity;
use super::intro_formation::chase_formation_angle;
use super::intro_motion::{follow_intro_predecessor, IntroAttachment, IntroScenePose};
use super::object::{Angle, ObjectId, ShapeId, Vector3};
use super::render::Rotation;
use super::state::RandomState;

pub const OPENING_CHAIN_SEGMENT_COUNT: usize = 9;
const BODY_SHAPE: ShapeId = ShapeId::from_catalog_index(340);
const TAIL_SHAPE: ShapeId = ShapeId::from_catalog_index(342);
const BURST_SHAPE: ShapeId = ShapeId::from_catalog_index(11);
const INITIAL_HEALTH: u8 = 2;
const FOLLOWING_HEALTH: u8 = 15;
const RECOVERED_HEALTH: u8 = 10;
const TRAIL_STYLE: u8 = 134;
const CONTACT_PAYLOAD: u8 = 1;
const SETTLED_PITCH: Angle = Angle::from_units(196);
const BANK_PITCHES: [u8; OPENING_CHAIN_SEGMENT_COUNT] = [206, 216, 236, 0, 30, 40, 50, 60, 150];
const ANGLE_CHASE_STEPS: usize = 3;
const BANK_YAW_STEP: i8 = 10;
const FOLLOWING_DEPTH_OFFSET: u16 = 3;
const DEPARTURE_DEPTH_OFFSET: u16 = 4;
const DEPARTURE_SPEED: u8 = 216;
const DEPARTURE_VELOCITY_SCALE: i16 = 4;
const DEPARTURE_DURATION_ORIGIN: u8 = 20;
const DEPARTURE_PITCH_STEP: i8 = 32;
const DEPARTURE_YAW_STEP: i8 = 16;
const DEPARTURE_ROLL_STEP: i8 = 8;
const BURST_UPDATES: u8 = 8;
const BURST_SIZE_BIAS: u8 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningChainPart {
    First,
    Second,
    Third,
    Fourth,
    Fifth,
    Sixth,
    Seventh,
    Eighth,
    Tail,
}

impl OpeningChainPart {
    pub const ALL: [Self; OPENING_CHAIN_SEGMENT_COUNT] = [
        Self::First,
        Self::Second,
        Self::Third,
        Self::Fourth,
        Self::Fifth,
        Self::Sixth,
        Self::Seventh,
        Self::Eighth,
        Self::Tail,
    ];

    pub const fn ordinal(self) -> u8 {
        match self {
            Self::First => 1,
            Self::Second => 2,
            Self::Third => 3,
            Self::Fourth => 4,
            Self::Fifth => 5,
            Self::Sixth => 6,
            Self::Seventh => 7,
            Self::Eighth => 8,
            Self::Tail => 9,
        }
    }

    pub const fn depth(self) -> i8 {
        if matches!(self, Self::First) {
            -11
        } else {
            -25
        }
    }
}

/// Scene signals retain their independent sampling points. The initial contact
/// branch is superseded by hiding, sort override is sampled on reveal, and the other controls
/// in each following update. Departure takes precedence over all following work.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningChainControls {
    pub suppress_initial_contact: bool,
    pub sort_override_on_reveal: bool,
    pub depart: bool,
    pub raise_depth_offset: bool,
    pub settle_pitch: bool,
    pub bank_by_part: bool,
    pub level_pitch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningChainPhase {
    Initializing,
    HiddenUntilNextUpdate,
    Following,
    Departing { updates_left: u8 },
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningChainSegment {
    pub pose: IntroScenePose,
    pub local_offset: Vector3,
    pub velocity: Vector3,
    pub depth_offset: u16,
    actor: ObjectId,
    parent: ObjectId,
    predecessor: ObjectId,
    part: OpeningChainPart,
    phase: OpeningChainPhase,
    health: u8,
    contact_disabled: bool,
    ordinary_contact_payload: Option<u8>,
    suppress_peer_contacts: bool,
    sort_override: bool,
    trail_style: Option<u8>,
    health_response_armed: bool,
}

impl OpeningChainSegment {
    pub fn new(
        actor: ObjectId,
        parent: ObjectId,
        predecessor: ObjectId,
        part: OpeningChainPart,
    ) -> Self {
        Self {
            pose: IntroScenePose::default(),
            local_offset: Vector3 {
                x: 0,
                y: 0,
                z: i16::from(part.depth()),
            },
            velocity: Vector3::default(),
            depth_offset: 0,
            actor,
            parent,
            predecessor,
            part,
            phase: OpeningChainPhase::Initializing,
            health: 1,
            contact_disabled: false,
            ordinary_contact_payload: None,
            suppress_peer_contacts: false,
            sort_override: false,
            trail_style: None,
            health_response_armed: false,
        }
    }

    pub fn actor(&self) -> ObjectId {
        self.actor
    }
    pub fn parent(&self) -> ObjectId {
        self.parent
    }
    pub fn predecessor(&self) -> ObjectId {
        self.predecessor
    }
    pub fn part(&self) -> OpeningChainPart {
        self.part
    }
    pub fn phase(&self) -> OpeningChainPhase {
        self.phase
    }
    pub fn health(&self) -> u8 {
        self.health
    }
    pub fn contact_disabled(&self) -> bool {
        self.contact_disabled
    }
    pub fn ordinary_contact_payload(&self) -> Option<u8> {
        self.ordinary_contact_payload
    }
    pub fn suppresses_peer_contacts(&self) -> bool {
        self.suppress_peer_contacts
    }
    pub fn sort_override(&self) -> bool {
        self.sort_override
    }
    pub fn trail_style(&self) -> Option<u8> {
        self.trail_style
    }
    pub fn health_response_armed(&self) -> bool {
        self.health_response_armed
    }

    pub fn shape(&self) -> ShapeId {
        if self.part == OpeningChainPart::Tail && self.phase != OpeningChainPhase::Initializing {
            TAIL_SHAPE
        } else {
            BODY_SHAPE
        }
    }

    pub fn is_visible(&self) -> bool {
        !matches!(
            self.phase,
            OpeningChainPhase::HiddenUntilNextUpdate | OpeningChainPhase::Finished
        )
    }

    /// Parent publication can affect a newborn before its constructor executes.
    /// Once initialized it owns its world transform, despite retaining the
    /// distinct parent and predecessor links.
    pub fn publish_from_parent(&mut self, parent: ObjectId, pose: IntroScenePose) {
        if self.phase == OpeningChainPhase::Initializing && self.parent == parent {
            self.pose = IntroAttachment {
                offset: self.local_offset,
                rotation: Rotation::default(),
            }
            .world_pose(pose);
        }
    }

    fn resolve_health_condition(&mut self) {
        if self.health == 0 && self.health_response_armed {
            self.health = RECOVERED_HEALTH;
            self.contact_disabled = true;
        }
    }

    /// Supply health only after the outer engine has selected this actor's
    /// strategy for execution. A zero value by itself does not grant the
    /// engine's one-update bypass of common destruction.
    pub fn set_health_at_strategy_entry(&mut self, health: u8) {
        if self.phase != OpeningChainPhase::Finished {
            self.health = health;
        }
    }

    /// Execute one authored path update. The caller supplies the main craft
    /// and the predecessor's already-updated pose separately. An emitted burst
    /// inherits the final world pose before this segment is retired.
    pub fn tick(
        &mut self,
        parent_pose: IntroScenePose,
        predecessor_pose: IntroScenePose,
        controls: OpeningChainControls,
        random: &mut RandomState,
    ) -> Option<OpeningChainDepartureBurst> {
        loop {
            match self.phase {
                OpeningChainPhase::Initializing => {
                    self.health = INITIAL_HEALTH;
                    // InvisibleOn supersedes the optional initial contact
                    // suppression branch, irrespective of its control bit.
                    self.contact_disabled = true;
                    self.ordinary_contact_payload = Some(CONTACT_PAYLOAD);
                    self.pose = follow_intro_predecessor(self.pose, parent_pose, self.part.depth());
                    self.phase = OpeningChainPhase::HiddenUntilNextUpdate;
                    break;
                }
                OpeningChainPhase::HiddenUntilNextUpdate => {
                    // InvisibleOff independently re-enables contact.
                    self.contact_disabled = false;
                    self.health = FOLLOWING_HEALTH;
                    self.trail_style = Some(TRAIL_STYLE);
                    self.suppress_peer_contacts = true;
                    self.health_response_armed = true;
                    self.sort_override = controls.sort_override_on_reveal;
                    self.phase = OpeningChainPhase::Following;
                }
                OpeningChainPhase::Following => {
                    if controls.depart {
                        self.depth_offset = DEPARTURE_DEPTH_OFFSET;
                        self.health_response_armed = false;
                        // FaceMother changes only heading, not placement.
                        self.pose.rotation =
                            follow_intro_predecessor(self.pose, parent_pose, 0).rotation;
                        self.velocity = flight_velocity(
                            self.pose.rotation.pitch,
                            self.pose.rotation.yaw,
                            DEPARTURE_SPEED,
                            DEPARTURE_VELOCITY_SCALE,
                        );
                        self.health = 1;
                        self.pose.rotation = Rotation {
                            pitch: Angle::from_units(random.next_byte()),
                            yaw: Angle::from_units(random.next_byte()),
                            roll: Angle::from_units(random.next_byte()),
                        };
                        self.phase = OpeningChainPhase::Departing {
                            updates_left: DEPARTURE_DURATION_ORIGIN - self.part.ordinal(),
                        };
                        continue;
                    }
                    self.pose =
                        follow_intro_predecessor(self.pose, predecessor_pose, self.part.depth());
                    if controls.raise_depth_offset {
                        self.depth_offset = FOLLOWING_DEPTH_OFFSET;
                    }
                    if controls.settle_pitch {
                        for _ in 0..ANGLE_CHASE_STEPS {
                            self.pose.rotation.pitch =
                                chase_formation_angle(self.pose.rotation.pitch, SETTLED_PITCH);
                        }
                    }
                    if controls.bank_by_part {
                        let target =
                            Angle::from_units(BANK_PITCHES[usize::from(self.part.ordinal() - 1)]);
                        for _ in 0..ANGLE_CHASE_STEPS {
                            self.pose.rotation.pitch =
                                chase_formation_angle(self.pose.rotation.pitch, target);
                        }
                        self.pose.rotation.yaw = self.pose.rotation.yaw.wrapping_add(BANK_YAW_STEP);
                    }
                    if controls.level_pitch {
                        self.pose.rotation.pitch = Angle::ZERO;
                    }
                    break;
                }
                OpeningChainPhase::Departing { updates_left } => {
                    advance_position(&mut self.pose.position, self.velocity);
                    self.pose.rotation.pitch =
                        self.pose.rotation.pitch.wrapping_add(DEPARTURE_PITCH_STEP);
                    self.pose.rotation.yaw =
                        self.pose.rotation.yaw.wrapping_add(DEPARTURE_YAW_STEP);
                    self.pose.rotation.roll =
                        self.pose.rotation.roll.wrapping_add(DEPARTURE_ROLL_STEP);
                    if updates_left == 1 {
                        self.phase = OpeningChainPhase::Finished;
                        // End skips the common tail: final world motion runs,
                        // but this update has no additional local drift.
                        return Some(OpeningChainDepartureBurst::new(self.actor, self.pose));
                    }
                    self.phase = OpeningChainPhase::Departing {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                OpeningChainPhase::Finished => return None,
            }
        }
        self.resolve_health_condition();
        advance_position(&mut self.local_offset, self.velocity);
        None
    }
}

fn advance_position(position: &mut Vector3, velocity: Vector3) {
    position.x = position.x.wrapping_add(velocity.x);
    position.y = position.y.wrapping_add(velocity.y);
    position.z = position.z.wrapping_add(velocity.z);
}

/// Eight-update departure sprite. Its authored size delta is zero; the spawn
/// parameter supplies a fixed size bias after Sprite initializes its channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningChainDepartureBurst {
    pub source: ObjectId,
    pub pose: IntroScenePose,
    pub color_frame: u8,
    size_bias: u8,
    initialized: bool,
    updates_left: u8,
}

impl OpeningChainDepartureBurst {
    pub fn new(source: ObjectId, pose: IntroScenePose) -> Self {
        Self {
            source,
            pose,
            color_frame: 0,
            size_bias: 0,
            initialized: false,
            updates_left: BURST_UPDATES,
        }
    }
    pub const fn shape(&self) -> ShapeId {
        BURST_SHAPE
    }
    pub fn is_finished(&self) -> bool {
        self.updates_left == 0
    }
    pub fn updates_left(&self) -> u8 {
        self.updates_left
    }
    pub fn is_sprite(&self) -> bool {
        self.initialized
    }
    pub const fn depth_offset(&self) -> u8 {
        0
    }
    pub fn size_bias(&self) -> u8 {
        self.size_bias
    }
    pub fn tick(&mut self) {
        if !self.is_finished() {
            self.initialized = true;
            self.size_bias = BURST_SIZE_BIAS;
            self.color_frame = (self.color_frame + 1) % BURST_UPDATES;
            self.updates_left -= 1;
        }
    }
}

/// A bounded chain family with caller-reserved identities. Recursive children
/// become live during their predecessor's first update, not a frame later.
/// The enclosing scene still owns allocation and release of the reserved IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningChainFamily {
    segments: [OpeningChainSegment; OPENING_CHAIN_SEGMENT_COUNT],
    spawned_count: usize,
    initialized_count: usize,
    bursts: Vec<OpeningChainDepartureBurst>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OpeningChainFrameEvents {
    pub retired_segments: Vec<ObjectId>,
    pub spawned_bursts: usize,
    pub allocation_pressure: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningChainAllocationContext {
    /// Free slots before cleanup, not including retiring segments or sprites.
    pub available_slots: usize,
    /// The source pressure sweep omits the final actor in the active list.
    /// Only that actual traversal boundary can spare the oldest chain burst.
    pub oldest_burst_at_list_tail: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningChainCapacityError {
    pub required_slots: usize,
    pub available_slots: usize,
}

impl std::fmt::Display for OpeningChainCapacityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "opening chain departure needs {} free object slots; {} available",
            self.required_slots, self.available_slots
        )
    }
}
impl std::error::Error for OpeningChainCapacityError {}

impl OpeningChainFamily {
    pub fn new(parent: ObjectId, identities: [ObjectId; OPENING_CHAIN_SEGMENT_COUNT]) -> Self {
        assert!(
            identities
                .iter()
                .enumerate()
                .all(|(index, id)| *id != parent && !identities[..index].contains(id)),
            "chain identities must be distinct from their parent and one another"
        );
        Self {
            segments: std::array::from_fn(|index| {
                OpeningChainSegment::new(
                    identities[index],
                    parent,
                    if index == 0 {
                        parent
                    } else {
                        identities[index - 1]
                    },
                    OpeningChainPart::ALL[index],
                )
            }),
            spawned_count: 1,
            initialized_count: 0,
            bursts: Vec::new(),
        }
    }

    pub fn segments(&self) -> &[OpeningChainSegment] {
        &self.segments[..self.spawned_count]
    }
    pub fn bursts(&self) -> &[OpeningChainDepartureBurst] {
        &self.bursts
    }
    pub fn initialized_count(&self) -> usize {
        self.initialized_count
    }
    pub fn set_health_at_strategy_entry(&mut self, part: OpeningChainPart, health: u8) {
        let index = usize::from(part.ordinal() - 1);
        if index < self.spawned_count {
            self.segments[index].set_health_at_strategy_entry(health);
        }
    }
    pub fn is_finished(&self) -> bool {
        self.initialized_count == OPENING_CHAIN_SEGMENT_COUNT
            && self
                .segments
                .iter()
                .all(|segment| segment.phase == OpeningChainPhase::Finished)
            && self.bursts.is_empty()
    }

    pub fn publish_from_parent(&mut self, parent: ObjectId, pose: IntroScenePose) {
        // Later children are not yet live at this earlier parent publication.
        for segment in &mut self.segments[..self.spawned_count] {
            segment.publish_from_parent(parent, pose);
        }
    }

    /// Update the family in predecessor order. QuickSpawn inserts immediately
    /// after the departing segment, so its burst runs in this same traversal.
    /// Exhaustion is an explicit error, corresponding to the source's
    /// diagnostic halt. The pending native update is preserved for recovery.
    /// Consuming the last slot sweeps older eligible effects for retirement.
    pub fn tick(
        &mut self,
        parent_pose: IntroScenePose,
        controls: OpeningChainControls,
        random: &mut RandomState,
        allocation: OpeningChainAllocationContext,
    ) -> Result<OpeningChainFrameEvents, OpeningChainCapacityError> {
        let required_slots = self
            .segments
            .iter()
            .filter(|segment| segment.phase == OpeningChainPhase::Departing { updates_left: 1 })
            .count();
        if allocation.available_slots < required_slots {
            return Err(OpeningChainCapacityError {
                required_slots,
                available_slots: allocation.available_slots,
            });
        }
        let mut available_burst_slots = allocation.available_slots;
        for burst in &mut self.bursts {
            burst.tick();
        }
        let mut events = OpeningChainFrameEvents::default();
        let mut index = 0;
        while index < self.spawned_count {
            if self.segments[index].phase == OpeningChainPhase::Initializing {
                self.initialized_count += 1;
                if index + 1 < OPENING_CHAIN_SEGMENT_COUNT {
                    self.spawned_count += 1;
                }
            }
            let predecessor_pose = if index == 0 {
                parent_pose
            } else {
                self.segments[index - 1].pose
            };
            if let Some(mut burst) =
                self.segments[index].tick(parent_pose, predecessor_pose, controls, random)
            {
                events.retired_segments.push(self.segments[index].actor());
                available_burst_slots -= 1;
                if available_burst_slots == 0 {
                    events.allocation_pressure = true;
                    // Old bursts are after the spawner in newest-first list
                    // order. The sweep reaches every one except a genuine
                    // list-tail actor; cleanup has not removed it yet.
                    self.bursts
                        .truncate(usize::from(allocation.oldest_burst_at_list_tail));
                }
                burst.tick();
                self.bursts.push(burst);
                events.spawned_bursts += 1;
            }
            index += 1;
        }
        self.bursts.retain(|burst| !burst.is_finished());
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::super::object::{Behavior, Object, ObjectKind, ObjectStore};
    use super::*;

    fn family() -> (ObjectId, OpeningChainFamily) {
        let mut objects = ObjectStore::new();
        let mut allocate = || {
            objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    BODY_SHAPE,
                    Behavior::Effect,
                ))
                .unwrap()
        };
        let parent = allocate();
        let identities = std::array::from_fn(|_| allocate());
        (parent, OpeningChainFamily::new(parent, identities))
    }

    const AVAILABLE: OpeningChainAllocationContext = OpeningChainAllocationContext {
        available_slots: 100,
        oldest_burst_at_list_tail: true,
    };

    #[test]
    fn recursive_birth_reveal_sampling_and_pitch_precedence_are_explicit() {
        let (parent, mut family) = family();
        let mut random = RandomState::default();
        let pose = IntroScenePose {
            position: Vector3 {
                x: 1900,
                y: -270,
                z: 700,
            },
            rotation: Rotation {
                pitch: Angle::from_units(51),
                yaw: Angle::from_units(203),
                roll: Angle::from_units(127),
            },
        };
        assert_eq!(family.segments().len(), 1);
        assert_eq!(family.initialized_count(), 0);
        family.publish_from_parent(parent, pose);
        family
            .tick(
                pose,
                OpeningChainControls::default(),
                &mut random,
                AVAILABLE,
            )
            .unwrap();
        assert_eq!(family.segments().len(), OPENING_CHAIN_SEGMENT_COUNT);
        assert_eq!(family.initialized_count(), OPENING_CHAIN_SEGMENT_COUNT);
        for segment in family.segments() {
            assert_eq!(segment.phase(), OpeningChainPhase::HiddenUntilNextUpdate);
            assert_eq!(segment.health(), INITIAL_HEALTH);
            assert!(!segment.is_visible());
            assert!(segment.contact_disabled());
            assert_eq!(segment.ordinary_contact_payload(), Some(CONTACT_PAYLOAD));
            assert_eq!(segment.parent(), parent);
        }
        let controls = OpeningChainControls {
            sort_override_on_reveal: true,
            settle_pitch: true,
            bank_by_part: true,
            level_pitch: true,
            raise_depth_offset: true,
            ..Default::default()
        };
        family.tick(pose, controls, &mut random, AVAILABLE).unwrap();
        for segment in family.segments() {
            assert_eq!(segment.phase(), OpeningChainPhase::Following);
            assert_eq!(segment.health(), FOLLOWING_HEALTH);
            assert!(segment.is_visible());
            assert!(!segment.contact_disabled());
            assert!(segment.sort_override());
            assert_eq!(segment.pose.rotation.pitch, Angle::ZERO);
            assert_eq!(segment.depth_offset, FOLLOWING_DEPTH_OFFSET);
        }
        family
            .tick(
                pose,
                OpeningChainControls::default(),
                &mut random,
                AVAILABLE,
            )
            .unwrap();
        assert!(family
            .segments()
            .iter()
            .all(|segment| segment.sort_override()));
        assert_eq!(random, RandomState::default());
    }

    #[test]
    fn health_response_persists_until_departure_cancels_it_before_trigger_resolution() {
        let (_, mut family) = family();
        let pose = IntroScenePose::default();
        let mut random = RandomState::default();
        for _ in 0..2 {
            family
                .tick(
                    pose,
                    OpeningChainControls::default(),
                    &mut random,
                    AVAILABLE,
                )
                .unwrap();
        }
        for _ in 0..2 {
            family.set_health_at_strategy_entry(OpeningChainPart::First, 0);
            family
                .tick(
                    pose,
                    OpeningChainControls::default(),
                    &mut random,
                    AVAILABLE,
                )
                .unwrap();
            assert_eq!(family.segments()[0].health(), RECOVERED_HEALTH);
            assert!(family.segments()[0].contact_disabled());
            assert!(family.segments()[0].health_response_armed());
        }
        family.set_health_at_strategy_entry(OpeningChainPart::Second, 0);
        family
            .tick(
                pose,
                OpeningChainControls {
                    depart: true,
                    ..Default::default()
                },
                &mut random,
                AVAILABLE,
            )
            .unwrap();
        assert_eq!(family.segments()[1].health(), 1);
        assert!(!family.segments()[1].contact_disabled());
        assert!(!family.segments()[1].health_response_armed());
        let mut expected_random = RandomState::default();
        for _ in 0..OPENING_CHAIN_SEGMENT_COUNT * 3 {
            expected_random.next_byte();
        }
        assert_eq!(random, expected_random);
    }

    #[test]
    fn complete_departure_runs_tail_first_and_terminal_updates_are_inert() {
        let (_, mut family) = family();
        let pose = IntroScenePose::default();
        let mut random = RandomState::new([1, 2, 3, 4]);
        let mut retired_parts = Vec::new();
        for update in 0..40 {
            let events = family
                .tick(
                    pose,
                    OpeningChainControls {
                        depart: update >= 2,
                        ..Default::default()
                    },
                    &mut random,
                    AVAILABLE,
                )
                .unwrap();
            for id in events.retired_segments {
                retired_parts.push(
                    family
                        .segments()
                        .iter()
                        .find(|part| part.actor() == id)
                        .unwrap()
                        .part(),
                );
            }
            for burst in family.bursts() {
                assert!(burst.is_sprite());
                assert_eq!(burst.size_bias(), BURST_SIZE_BIAS);
                assert_eq!(burst.depth_offset(), 0);
            }
        }
        assert_eq!(
            retired_parts,
            OpeningChainPart::ALL.into_iter().rev().collect::<Vec<_>>()
        );
        assert!(family.is_finished());
        let ended = family.clone();
        let ended_random = random;
        let events = family
            .tick(
                pose,
                OpeningChainControls::default(),
                &mut random,
                AVAILABLE,
            )
            .unwrap();
        assert_eq!(events, OpeningChainFrameEvents::default());
        assert_eq!(family, ended);
        assert_eq!(random, ended_random);
    }

    #[test]
    fn capacity_error_preserves_the_pending_departure_and_random_state() {
        let (_, mut family) = family();
        let mut random = RandomState::default();
        let pose = IntroScenePose::default();
        let depart = OpeningChainControls {
            depart: true,
            ..Default::default()
        };
        for _ in 0..11 {
            family.tick(pose, depart, &mut random, AVAILABLE).unwrap();
        }
        assert_eq!(
            family.segments()[8].phase(),
            OpeningChainPhase::Departing { updates_left: 1 }
        );
        let saved = family.clone();
        let saved_random = random;
        assert_eq!(
            family.tick(
                pose,
                depart,
                &mut random,
                OpeningChainAllocationContext {
                    available_slots: 0,
                    ..AVAILABLE
                }
            ),
            Err(OpeningChainCapacityError {
                required_slots: 1,
                available_slots: 0
            })
        );
        assert_eq!(family, saved);
        assert_eq!(random, saved_random);
        let events = family.tick(pose, depart, &mut random, AVAILABLE).unwrap();
        assert_eq!(events.retired_segments.len(), 1);
        assert_eq!(events.spawned_bursts, 1);
        assert_eq!(family.bursts()[0].color_frame, 1);
    }
}
