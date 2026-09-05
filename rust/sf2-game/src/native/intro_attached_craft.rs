//! First attached opening Arwing, its independently departing copy, retained
//! flare, scheduled bursts and common destruction. All motion is source-time.

use super::intro_destruction::{
    IntroDestructionCapacityError, IntroDestructionContext, IntroDestructionEffects,
    IntroExplosionActor, IntroExplosionBirthTiming, IntroExplosionProfile, IntroExplosionVolume,
};
use super::intro_free_craft::IntroAuxiliaryEffect;
use super::intro_motion::{IntroAttachment, IntroScenePose};
use super::object::{Angle, ObjectId, ShapeId, StereoPosition, Vector3};
use super::render::Rotation;
use super::state::RandomState;

const CRAFT_SHAPE: ShapeId = ShapeId::from_catalog_index(64);
const NO_MESH: ShapeId = ShapeId::from_catalog_index(0);
const FLARE_SHAPE: ShapeId = ShapeId::from_catalog_index(48);
const LARGE_BURST_SHAPE: ShapeId = ShapeId::from_catalog_index(11);
const HUGE_BURST_SHAPE: ShapeId = ShapeId::from_catalog_index(12);
const SPLIT_WAIT: u8 = 36;
const ATTACHED_HOLD: u8 = 28;
const ATTACHED_BURSTS: u8 = 4;
const ATTACHED_DEPTH_STEP: i16 = 7;
const DEPARTURE_WAIT: u8 = 8;
const DEPARTURE_DRIFT: u8 = 16;
const DEPARTURE_BURSTS: u8 = 3;
const DEPARTURE_DEPTH_STEP: i16 = 15;
const DEPARTURE_LATERAL_SHIFT: i16 = -200;
const DRIFT_CALLBACKS: u8 = 20;
const DRIFT_STEP: Vector3 = Vector3 { x: -3, y: -6, z: 0 };
const FLARE_UPDATES: u8 = 65;
const BURST_UPDATES: u8 = 8;
const BURST_SIZE_BIAS: u8 = 2;
const BURST_EMISSION_PHASE: u8 = 1;
const BURST_SIZE_PHASE: u8 = 4;
const SOUND_RANDOM_THRESHOLD: u8 = 127;
const HORIZONTAL_RANDOM_MASK: u16 = 511;
const VERTICAL_RANDOM_MASK: u16 = 255;
const BURST_VERTICAL_SHIFT: i16 = -256;
const DEPARTURE_DEPTH_OFFSET: u8 = 2;
const FAR_SORT_DEPTH: i16 = 15_000;
const FLYBY_EFFECT_RANGE: i16 = 1;
const DEPARTURE_EFFECT_RANGE: i16 = 2;
const SOUND_NEAR_LIMIT: u32 = 800;
const SOUND_FAR_LIMIT: u32 = 1_300;
const SOUND_RIGHT_START: u8 = 16;
const SOUND_REAR_CENTER_START: u8 = 112;
const SOUND_LEFT_START: u8 = 144;
const SOUND_FRONT_CENTER_START: u8 = 240;
const ANGLE_FRACTION_BITS: u32 = 8;
const FLARE_ATTACHMENT: IntroAttachment = IntroAttachment {
    offset: Vector3 { x: 50, y: 5, z: 0 },
    rotation: Rotation {
        pitch: Angle::ZERO,
        yaw: Angle::from_units(3),
        roll: Angle::from_units(74),
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningBurstSound {
    Burst,
    Departure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningBurstAudio {
    pub sound: OpeningBurstSound,
    pub source: Vector3,
}

impl OpeningBurstAudio {
    /// Selected-listener class-two audio uses an integer Euclidean X/Z
    /// distance, unlike the approximation used by common explosion audio.
    pub fn spatial(self, listener: IntroScenePose) -> (IntroExplosionVolume, StereoPosition) {
        let x = self.source.x.wrapping_sub(listener.position.x);
        let z = self.source.z.wrapping_sub(listener.position.z);
        let squared = i64::from(x) * i64::from(x) + i64::from(z) * i64::from(z);
        let distance = (squared as u32).isqrt();
        if distance >= SOUND_FAR_LIMIT {
            return (IntroExplosionVolume::Far, StereoPosition::Center);
        }
        let volume = if distance < SOUND_NEAR_LIMIT {
            IntroExplosionVolume::Near
        } else {
            IntroExplosionVolume::Middle
        };
        let relative = ((sf_core::aim_angle::sf2_atan16(x, z) >> ANGLE_FRACTION_BITS) as u8)
            .wrapping_sub(listener.rotation.yaw.units());
        let stereo = if (SOUND_RIGHT_START..SOUND_REAR_CENTER_START).contains(&relative) {
            StereoPosition::Right
        } else if (SOUND_LEFT_START..SOUND_FRONT_CENTER_START).contains(&relative) {
            StereoPosition::Left
        } else {
            StereoPosition::Center
        };
        (volume, stereo)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningBurstParticle {
    pub pose: IntroScenePose,
    pub shape: ShapeId,
    pub color_frame: u8,
    updates_left: u8,
}

impl OpeningBurstParticle {
    pub const fn size_bias(&self) -> u8 {
        BURST_SIZE_BIAS
    }
    pub fn is_finished(&self) -> bool {
        self.updates_left == 0
    }

    /// Run one particle visit; the caller chooses whether its birth is visited
    /// in this traversal or the next one.
    pub fn tick(&mut self) {
        if !self.is_finished() {
            self.color_frame = (self.color_frame + 1) % BURST_UPDATES;
            self.updates_left -= 1;
        }
    }
}

fn centered_random(random: &mut RandomState, mask: u16) -> i16 {
    let high = random.next_byte();
    let low = random.next_byte();
    ((u16::from_be_bytes([high, low]) & mask) as i16).wrapping_sub((mask >> 1) as i16)
}

/// One shared scene-clock burst callback. Sound selection consumes randomness
/// before the three independently centered position offsets.
pub fn opening_burst(
    pose: IntroScenePose,
    scene_phase: u8,
    random: &mut RandomState,
) -> Option<(OpeningBurstParticle, Option<OpeningBurstSound>)> {
    if scene_phase & BURST_EMISSION_PHASE != 0 {
        return None;
    }
    let sound = if random.next_byte() < SOUND_RANDOM_THRESHOLD {
        None
    } else if random.next_byte() < SOUND_RANDOM_THRESHOLD {
        Some(OpeningBurstSound::Departure)
    } else {
        Some(OpeningBurstSound::Burst)
    };
    let mut pose = pose;
    pose.position.x = pose
        .position
        .x
        .wrapping_add(centered_random(random, HORIZONTAL_RANDOM_MASK));
    pose.position.y = pose
        .position
        .y
        .wrapping_add(centered_random(random, VERTICAL_RANDOM_MASK))
        .wrapping_add(BURST_VERTICAL_SHIFT);
    pose.position.z = pose
        .position
        .z
        .wrapping_add(centered_random(random, HORIZONTAL_RANDOM_MASK));
    Some((
        OpeningBurstParticle {
            pose,
            shape: if scene_phase & BURST_SIZE_PHASE == 0 {
                HUGE_BURST_SHAPE
            } else {
                LARGE_BURST_SHAPE
            },
            color_frame: 0,
            updates_left: BURST_UPDATES,
        },
        sound,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningAttachedCraftPhase {
    WaitingForSplit { updates_left: u8 },
    Holding { updates_left: u8 },
    Emitting { updates_left: u8 },
    AwaitingDestruction,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningDepartingCraftPhase {
    Waiting { updates_left: u8 },
    Drifting { updates_left: u8 },
    Emitting { updates_left: u8 },
    AwaitingDestruction,
    Finished,
}

/// Contextual variants retain authored style selections without assigning an
/// unverified shader meaning to them. Their rendering remains a separate gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningCraftStyle {
    Initial,
    AttachedDeparture,
    IndependentDeparture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningAttachedCraft {
    pub pose: IntroScenePose,
    pub attachment: IntroAttachment,
    pub velocity: Vector3,
    pub style: OpeningCraftStyle,
    pub phase: OpeningAttachedCraftPhase,
}

/// Requests made by one attached-craft strategy visit. The scene owns child
/// allocation, burst callbacks and common destruction; none run implicitly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningAttachedCraftStep {
    pub split: bool,
    pub emit_burst: bool,
    pub request_destruction: bool,
}

impl OpeningAttachedCraft {
    pub fn new(attachment: IntroAttachment) -> Self {
        Self {
            pose: IntroScenePose::default(),
            attachment,
            velocity: Vector3 {
                x: 0,
                y: 0,
                z: ATTACHED_DEPTH_STEP,
            },
            style: OpeningCraftStyle::Initial,
            phase: OpeningAttachedCraftPhase::WaitingForSplit {
                updates_left: SPLIT_WAIT,
            },
        }
    }

    pub fn publish_from_parent(&mut self, parent: IntroScenePose) {
        if self.is_visible() {
            self.pose = self.attachment.world_pose(parent);
        }
    }

    /// Advance only this actor. A split requests two children, without visiting
    /// either; their birth-update scheduling belongs to the scene traversal.
    /// Common destruction is requested on its own following strategy visit.
    pub fn tick(
        &mut self,
        actor: ObjectId,
        auxiliary: &mut IntroAuxiliaryEffect,
    ) -> OpeningAttachedCraftStep {
        let mut step = OpeningAttachedCraftStep::default();
        loop {
            match self.phase {
                OpeningAttachedCraftPhase::WaitingForSplit { updates_left } if updates_left > 0 => {
                    self.phase = OpeningAttachedCraftPhase::WaitingForSplit {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                OpeningAttachedCraftPhase::WaitingForSplit { .. } => {
                    self.style = OpeningCraftStyle::AttachedDeparture;
                    self.phase = OpeningAttachedCraftPhase::Holding {
                        updates_left: ATTACHED_HOLD,
                    };
                    step.split = true;
                }
                OpeningAttachedCraftPhase::Holding { updates_left } if updates_left > 0 => {
                    self.phase = OpeningAttachedCraftPhase::Holding {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                OpeningAttachedCraftPhase::Holding { .. } => {
                    self.phase = OpeningAttachedCraftPhase::Emitting {
                        updates_left: ATTACHED_BURSTS,
                    };
                }
                OpeningAttachedCraftPhase::Emitting { updates_left } => {
                    step.emit_burst = true;
                    if updates_left == 0 {
                        auxiliary.configure_flyby(actor, self.pose, FLYBY_EFFECT_RANGE);
                        self.phase = OpeningAttachedCraftPhase::AwaitingDestruction;
                    } else {
                        self.phase = OpeningAttachedCraftPhase::Emitting {
                            updates_left: updates_left - 1,
                        };
                    }
                    break;
                }
                OpeningAttachedCraftPhase::AwaitingDestruction => {
                    step.request_destruction = true;
                    return step;
                }
                OpeningAttachedCraftPhase::Finished => return step,
            }
        }
        self.attachment.advance(self.velocity);
        step
    }

    pub fn is_visible(&self) -> bool {
        self.phase != OpeningAttachedCraftPhase::Finished
    }
    pub const fn shape(&self) -> ShapeId {
        CRAFT_SHAPE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningCraftFlare {
    pub pose: IntroScenePose,
    pub attachment: IntroAttachment,
    updates_left: u8,
    finished: bool,
}

impl OpeningCraftFlare {
    pub fn new() -> Self {
        Self {
            pose: IntroScenePose::default(),
            attachment: FLARE_ATTACHMENT,
            updates_left: FLARE_UPDATES,
            finished: false,
        }
    }

    pub fn publish_from_owner(&mut self, owner: IntroScenePose) {
        if self.is_visible() {
            self.pose = self.attachment.world_pose(owner);
        }
    }

    /// Only the strategy timer advances here, never attachment publication.
    pub fn tick(&mut self) {
        if self.updates_left == 0 {
            self.finished = true;
        } else {
            self.updates_left -= 1;
        }
    }

    pub fn is_visible(&self) -> bool {
        !self.finished
    }
    pub const fn shape(&self) -> ShapeId {
        FLARE_SHAPE
    }
    pub const fn sort_depth_override(&self) -> Option<i16> {
        Some(FAR_SORT_DEPTH)
    }
}

impl Default for OpeningCraftFlare {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningDepartingCraft {
    pub pose: IntroScenePose,
    pub velocity: Vector3,
    pub shape: ShapeId,
    pub phase: OpeningDepartingCraftPhase,
    drift_left: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningDepartingCraftStep {
    /// Burst callbacks use the post-flight pose exposed by this visit.
    pub emit_burst: bool,
    pub request_destruction: bool,
}

impl OpeningDepartingCraft {
    pub fn new(pose: IntroScenePose) -> Self {
        Self {
            pose,
            velocity: Vector3 {
                x: 0,
                y: 0,
                z: DEPARTURE_DEPTH_STEP,
            },
            shape: CRAFT_SHAPE,
            phase: OpeningDepartingCraftPhase::Waiting {
                updates_left: DEPARTURE_WAIT,
            },
            drift_left: 0,
        }
    }

    pub fn tick(
        &mut self,
        actor: ObjectId,
        auxiliary: &mut IntroAuxiliaryEffect,
    ) -> OpeningDepartingCraftStep {
        let mut step = OpeningDepartingCraftStep::default();
        loop {
            match self.phase {
                OpeningDepartingCraftPhase::Waiting { updates_left } if updates_left > 0 => {
                    self.phase = OpeningDepartingCraftPhase::Waiting {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                OpeningDepartingCraftPhase::Waiting { .. } => {
                    self.drift_left = DRIFT_CALLBACKS;
                    self.phase = OpeningDepartingCraftPhase::Drifting {
                        updates_left: DEPARTURE_DRIFT,
                    };
                }
                OpeningDepartingCraftPhase::Drifting { updates_left } if updates_left > 0 => {
                    self.phase = OpeningDepartingCraftPhase::Drifting {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                OpeningDepartingCraftPhase::Drifting { .. } => {
                    self.shape = NO_MESH;
                    self.pose.position.x =
                        self.pose.position.x.wrapping_add(DEPARTURE_LATERAL_SHIFT);
                    auxiliary.configure_departure(actor, self.pose, DEPARTURE_EFFECT_RANGE);
                    self.phase = OpeningDepartingCraftPhase::Emitting {
                        updates_left: DEPARTURE_BURSTS,
                    };
                }
                OpeningDepartingCraftPhase::Emitting { updates_left } => {
                    step.emit_burst = true;
                    self.phase = if updates_left == 0 {
                        OpeningDepartingCraftPhase::AwaitingDestruction
                    } else {
                        OpeningDepartingCraftPhase::Emitting {
                            updates_left: updates_left - 1,
                        }
                    };
                    break;
                }
                OpeningDepartingCraftPhase::AwaitingDestruction => {
                    step.request_destruction = true;
                    return step;
                }
                OpeningDepartingCraftPhase::Finished => return step,
            }
        }
        self.pose.position.z = self.pose.position.z.wrapping_add(self.velocity.z);
        if self.drift_left > 0 {
            self.pose.position.x = self.pose.position.x.wrapping_add(DRIFT_STEP.x);
            self.pose.position.y = self.pose.position.y.wrapping_add(DRIFT_STEP.y);
            self.drift_left -= 1;
        }
        step
    }

    pub fn is_visible(&self) -> bool {
        self.shape != NO_MESH && !self.is_finished()
    }
    pub fn is_finished(&self) -> bool {
        self.phase == OpeningDepartingCraftPhase::Finished
    }
    pub const fn style(&self) -> OpeningCraftStyle {
        OpeningCraftStyle::IndependentDeparture
    }
    pub const fn depth_offset(&self) -> u8 {
        DEPARTURE_DEPTH_OFFSET
    }
    pub const fn sort_depth_override(&self) -> Option<i16> {
        Some(FAR_SORT_DEPTH)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OpeningAttachedCraftEvents {
    pub split: bool,
    pub attached_retired: bool,
    pub departing_retired: bool,
    pub selected_audio: Option<OpeningBurstAudio>,
    pub explosion_audio: Vec<IntroExplosionVolume>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningAttachedCraftSequence {
    pub craft: OpeningAttachedCraft,
    pub departing: Option<OpeningDepartingCraft>,
    pub flare: Option<OpeningCraftFlare>,
    craft_id: ObjectId,
    departing_id: ObjectId,
    bursts: Vec<OpeningBurstParticle>,
    destruction: Vec<IntroDestructionEffects>,
}

impl OpeningAttachedCraftSequence {
    pub fn new(craft_id: ObjectId, departing_id: ObjectId, attachment: IntroAttachment) -> Self {
        Self {
            craft: OpeningAttachedCraft::new(attachment),
            departing: None,
            flare: None,
            craft_id,
            departing_id,
            bursts: Vec::new(),
            destruction: Vec::new(),
        }
    }

    pub fn bursts(&self) -> impl Iterator<Item = &OpeningBurstParticle> {
        self.bursts.iter()
    }
    pub fn explosions(&self) -> impl Iterator<Item = &IntroExplosionActor> {
        self.destruction
            .iter()
            .flat_map(IntroDestructionEffects::actors)
    }
    pub fn is_finished(&self) -> bool {
        !self.craft.is_visible()
            && self
                .departing
                .as_ref()
                .is_none_or(OpeningDepartingCraft::is_finished)
            && self.flare.as_ref().is_none_or(|flare| !flare.is_visible())
            && self.bursts.iter().all(OpeningBurstParticle::is_finished)
            && self
                .destruction
                .iter()
                .all(IntroDestructionEffects::is_finished)
    }

    fn reserve(
        context: &mut IntroDestructionContext,
        count: usize,
    ) -> Result<(), IntroDestructionCapacityError> {
        if context.available_slots < count {
            return Err(IntroDestructionCapacityError {
                required_slots: count,
                available_slots: context.available_slots,
            });
        }
        context.available_slots -= count;
        Ok(())
    }

    fn destroy(
        &mut self,
        shape: ShapeId,
        position: Vector3,
        context: &mut IntroDestructionContext,
        events: &mut OpeningAttachedCraftEvents,
    ) -> Result<(), IntroDestructionCapacityError> {
        let profile = IntroExplosionProfile::for_shape(shape)
            .expect("authored opening shape belongs to catalog");
        let (family, audio) = IntroDestructionEffects::spawn(profile, position, context)?;
        Self::reserve(context, family.actors().count())?;
        self.destruction.push(family);
        events.explosion_audio.extend(audio);
        Ok(())
    }

    fn emit(
        &mut self,
        pose: IntroScenePose,
        scene_phase: u8,
        random: &mut RandomState,
        context: &mut IntroDestructionContext,
        events: &mut OpeningAttachedCraftEvents,
    ) -> Result<(), IntroDestructionCapacityError> {
        if scene_phase & BURST_EMISSION_PHASE == 0 {
            Self::reserve(context, 1)?;
            let (mut particle, sound) = opening_burst(pose, scene_phase, random).unwrap();
            particle.tick(); // QuickSpawn inserts immediately after its emitter.
            self.bursts.push(particle);
            if let Some(sound) = sound {
                events.selected_audio = Some(OpeningBurstAudio {
                    sound,
                    source: pose.position,
                });
            }
        }
        Ok(())
    }

    /// Parent pose is the current update's published root pose. Common-death
    /// births are inserted behind the already-visited root and run next update;
    /// path-created particles and the departing copy run on their birth update.
    pub fn tick(
        &mut self,
        parent: IntroScenePose,
        scene_phase: u8,
        random: &mut RandomState,
        auxiliary: &mut IntroAuxiliaryEffect,
        context: &IntroDestructionContext,
    ) -> Result<OpeningAttachedCraftEvents, IntroDestructionCapacityError> {
        if self.is_finished() {
            return Ok(OpeningAttachedCraftEvents::default());
        }
        let mut events = OpeningAttachedCraftEvents::default();
        let mut context = *context;
        self.craft.publish_from_parent(parent);
        if let Some(flare) = &mut self.flare {
            flare.publish_from_owner(self.craft.pose);
        }
        // Existing common-death actors precede both craft in the active list.
        for family in self.destruction.iter_mut().rev() {
            let before = family.actors().count();
            events.explosion_audio.extend(
                family.tick_with_birth_timing(&context, IntroExplosionBirthTiming::NextUpdate)?,
            );
            Self::reserve(&mut context, family.actors().count() - before)?;
        }
        for burst in &mut self.bursts {
            burst.tick();
        }

        // Reserve the split before advancing the strategy, preserving the
        // wrapper's no-half-family behavior on insufficient capacity.
        if self.craft.phase == (OpeningAttachedCraftPhase::WaitingForSplit { updates_left: 0 }) {
            Self::reserve(&mut context, 2)?;
        }
        let attachment_before = self.craft.attachment;
        let step = self.craft.tick(self.craft_id, auxiliary);
        if step.split {
            self.flare = Some(OpeningCraftFlare::new());
            self.departing = Some(OpeningDepartingCraft::new(self.craft.pose));
            events.split = true;
        }
        if step.request_destruction {
            self.destroy(
                CRAFT_SHAPE,
                self.craft.pose.position,
                &mut context,
                &mut events,
            )?;
            self.craft.phase = OpeningAttachedCraftPhase::Finished;
            events.attached_retired = true;
        }
        if step.emit_burst {
            if let Err(error) = self.emit(
                self.craft.pose,
                scene_phase,
                random,
                &mut context,
                &mut events,
            ) {
                // The wrapper historically stops before local motion when
                // its burst allocation fails. Preserve that error boundary.
                self.craft.attachment = attachment_before;
                return Err(error);
            }
        }
        if let Some(mut departing) = self.departing {
            let step = departing.tick(self.departing_id, auxiliary);
            if step.request_destruction {
                self.destroy(
                    departing.shape,
                    departing.pose.position,
                    &mut context,
                    &mut events,
                )?;
                departing.phase = OpeningDepartingCraftPhase::Finished;
                events.departing_retired = true;
            }
            if step.emit_burst {
                self.emit(
                    departing.pose,
                    scene_phase,
                    random,
                    &mut context,
                    &mut events,
                )?;
            }
            self.departing = Some(departing);
        }
        if let Some(flare) = &mut self.flare {
            flare.tick();
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::super::object::{Behavior, Object, ObjectKind, ObjectStore, OBJECT_CAPACITY};
    use super::*;

    fn sequence() -> OpeningAttachedCraftSequence {
        let mut objects = ObjectStore::new();
        let craft = objects
            .allocate(Object::new(
                ObjectKind::Effect,
                CRAFT_SHAPE,
                Behavior::Effect,
            ))
            .unwrap();
        let departing = objects
            .allocate(Object::new(
                ObjectKind::Effect,
                CRAFT_SHAPE,
                Behavior::Effect,
            ))
            .unwrap();
        OpeningAttachedCraftSequence::new(craft, departing, IntroAttachment::default())
    }

    #[test]
    fn burst_gate_preserves_random_state_and_last_animation_frame_retires() {
        let mut random = RandomState::default();
        let before = random;
        assert!(
            opening_burst(IntroScenePose::default(), BURST_EMISSION_PHASE, &mut random).is_none()
        );
        assert_eq!(random, before);
        let (mut burst, _) = opening_burst(IntroScenePose::default(), 0, &mut random).unwrap();
        assert_ne!(random, before);
        for _ in 0..BURST_UPDATES - 1 {
            burst.tick();
            assert!(!burst.is_finished());
        }
        burst.tick();
        assert!(burst.is_finished());
        assert_eq!(burst.color_frame, 0);
        let finished = burst;
        burst.tick();
        assert_eq!(burst, finished);
    }

    #[test]
    fn new_flare_waits_until_the_following_root_publication() {
        let mut sequence = sequence();
        let mut random = RandomState::default();
        let mut auxiliary = IntroAuxiliaryEffect::default();
        let context = IntroDestructionContext {
            available_slots: OBJECT_CAPACITY,
            ..Default::default()
        };
        let parent = TARGET_TEST_POSE;
        for _ in 0..SPLIT_WAIT {
            sequence
                .tick(parent, 0, &mut random, &mut auxiliary, &context)
                .unwrap();
        }
        assert!(
            sequence
                .tick(parent, 0, &mut random, &mut auxiliary, &context)
                .unwrap()
                .split
        );
        assert_eq!(sequence.flare.unwrap().pose, IntroScenePose::default());
        sequence
            .tick(parent, 0, &mut random, &mut auxiliary, &context)
            .unwrap();
        let flare = sequence.flare.unwrap();
        assert_eq!(flare.pose, flare.attachment.world_pose(sequence.craft.pose));
    }

    const TARGET_TEST_POSE: IntroScenePose = IntroScenePose {
        position: Vector3 {
            x: 500,
            y: 200,
            z: -900,
        },
        rotation: Rotation {
            pitch: Angle::ZERO,
            yaw: Angle::HALF_TURN,
            roll: Angle::ZERO,
        },
    };

    #[test]
    fn standalone_attached_actor_requests_children_and_defers_common_death() {
        let mut family = sequence();
        let actor = family.craft_id;
        let craft = &mut family.craft;
        let mut auxiliary = IntroAuxiliaryEffect::default();
        for _ in 0..SPLIT_WAIT {
            craft.publish_from_parent(TARGET_TEST_POSE);
            assert_eq!(
                craft.tick(actor, &mut auxiliary),
                OpeningAttachedCraftStep::default()
            );
        }
        let published = craft.attachment.world_pose(TARGET_TEST_POSE);
        craft.publish_from_parent(TARGET_TEST_POSE);
        let step = craft.tick(actor, &mut auxiliary);
        assert_eq!(
            step,
            OpeningAttachedCraftStep {
                split: true,
                ..Default::default()
            }
        );
        assert_eq!(craft.pose, published);
        assert_eq!(
            craft.phase,
            OpeningAttachedCraftPhase::Holding {
                updates_left: ATTACHED_HOLD - 1
            }
        );
        assert!(family.departing.is_none() && family.flare.is_none());
        for _ in 0..ATTACHED_HOLD - 1 {
            assert_eq!(
                craft.tick(actor, &mut auxiliary),
                OpeningAttachedCraftStep::default()
            );
        }
        for _ in 0..=ATTACHED_BURSTS {
            assert_eq!(
                craft.tick(actor, &mut auxiliary),
                OpeningAttachedCraftStep {
                    emit_burst: true,
                    ..Default::default()
                }
            );
        }
        assert_eq!(craft.phase, OpeningAttachedCraftPhase::AwaitingDestruction);
        let before = (*craft, auxiliary);
        assert_eq!(
            craft.tick(actor, &mut auxiliary),
            OpeningAttachedCraftStep {
                request_destruction: true,
                ..Default::default()
            }
        );
        assert_eq!((*craft, auxiliary), before);
    }

    #[test]
    fn standalone_departing_actor_emits_after_motion_and_defers_common_death() {
        let actor = sequence().departing_id;
        let mut craft = OpeningDepartingCraft::new(TARGET_TEST_POSE);
        let mut auxiliary = IntroAuxiliaryEffect::default();
        for _ in 0..DEPARTURE_WAIT + DEPARTURE_DRIFT {
            assert_eq!(
                craft.tick(actor, &mut auxiliary),
                OpeningDepartingCraftStep::default()
            );
        }
        assert_eq!(craft.shape, CRAFT_SHAPE);
        for _ in 0..=DEPARTURE_BURSTS {
            let before = craft.pose.position;
            assert_eq!(
                craft.tick(actor, &mut auxiliary),
                OpeningDepartingCraftStep {
                    emit_burst: true,
                    ..Default::default()
                }
            );
            assert_eq!(
                craft.pose.position.z,
                before.z.wrapping_add(DEPARTURE_DEPTH_STEP)
            );
            assert_eq!(craft.shape, NO_MESH);
        }
        assert_eq!(craft.phase, OpeningDepartingCraftPhase::AwaitingDestruction);
        let before = (craft, auxiliary);
        assert_eq!(
            craft.tick(actor, &mut auxiliary),
            OpeningDepartingCraftStep {
                request_destruction: true,
                ..Default::default()
            }
        );
        assert_eq!((craft, auxiliary), before);
    }

    #[test]
    fn standalone_flare_keeps_publication_separate_from_its_lifetime() {
        let mut flare = OpeningCraftFlare::new();
        flare.tick();
        assert_eq!(flare.pose, IntroScenePose::default());
        flare.publish_from_owner(TARGET_TEST_POSE);
        let pose = flare.pose;
        for _ in 1..FLARE_UPDATES {
            flare.tick();
            assert!(flare.is_visible());
            assert_eq!(flare.pose, pose);
        }
        flare.tick();
        assert!(!flare.is_visible());
        let before = flare;
        flare.publish_from_owner(IntroScenePose::default());
        flare.tick();
        assert_eq!(flare, before);
    }

    #[test]
    fn insufficient_split_capacity_is_an_error_without_half_a_family() {
        let mut sequence = sequence();
        let mut random = RandomState::default();
        let mut auxiliary = IntroAuxiliaryEffect::default();
        let context = IntroDestructionContext {
            available_slots: 1,
            ..Default::default()
        };
        for _ in 0..SPLIT_WAIT {
            sequence
                .tick(TARGET_TEST_POSE, 0, &mut random, &mut auxiliary, &context)
                .unwrap();
        }
        let error = sequence
            .tick(TARGET_TEST_POSE, 0, &mut random, &mut auxiliary, &context)
            .unwrap_err();
        assert_eq!(
            error,
            IntroDestructionCapacityError {
                required_slots: 2,
                available_slots: 1
            }
        );
        assert!(sequence.flare.is_none() && sequence.departing.is_none());
    }

    #[test]
    fn failed_attached_burst_stops_before_local_motion_and_randomness() {
        let mut sequence = sequence();
        sequence.craft.phase = OpeningAttachedCraftPhase::Emitting { updates_left: 0 };
        let attachment = sequence.craft.attachment;
        let mut random = RandomState::default();
        let original_random = random;
        let mut auxiliary = IntroAuxiliaryEffect::default();
        let error = sequence
            .tick(
                TARGET_TEST_POSE,
                0,
                &mut random,
                &mut auxiliary,
                &IntroDestructionContext {
                    available_slots: 0,
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert_eq!(
            error,
            IntroDestructionCapacityError {
                required_slots: 1,
                available_slots: 0
            }
        );
        assert_eq!(sequence.craft.attachment, attachment);
        assert_eq!(random, original_random);
        assert_eq!(
            sequence.craft.phase,
            OpeningAttachedCraftPhase::AwaitingDestruction
        );
        let mut expected_auxiliary = IntroAuxiliaryEffect::default();
        expected_auxiliary.configure_flyby(
            sequence.craft_id,
            sequence.craft.pose,
            FLYBY_EFFECT_RANGE,
        );
        assert_eq!(auxiliary, expected_auxiliary);
    }

    #[test]
    fn finished_family_does_not_replay_effects_or_consume_randomness() {
        let mut sequence = sequence();
        let mut random = RandomState::default();
        let mut auxiliary = IntroAuxiliaryEffect::default();
        let context = IntroDestructionContext {
            available_slots: OBJECT_CAPACITY,
            ..Default::default()
        };
        for _ in 0..=SPLIT_WAIT + FLARE_UPDATES {
            sequence
                .tick(TARGET_TEST_POSE, 0, &mut random, &mut auxiliary, &context)
                .unwrap();
        }
        assert!(sequence.is_finished());
        let before = (sequence.clone(), random, auxiliary);
        assert_eq!(
            sequence
                .tick(
                    IntroScenePose::default(),
                    0,
                    &mut random,
                    &mut auxiliary,
                    &context
                )
                .unwrap(),
            OpeningAttachedCraftEvents::default()
        );
        assert_eq!((sequence, random, auxiliary), before);
    }
}
