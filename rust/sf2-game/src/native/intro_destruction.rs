//! Common opening-scene destruction effects. Allocation, birth updates and
//! retirement remain distinct from display-frame interpolation.

use sf2_data::shape_data::SHAPE_DATA;
use sf_core::aim_angle::sf2_xz_angle_distance;

use super::object::{ShapeId, Vector3};

const SMALL_DIAMETER_LIMIT: u16 = 64;
const MEDIUM_DIAMETER_LIMIT: u16 = 128;
const LARGE_DIAMETER_LIMIT: u16 = 256;
const SMALL_UPDATES: u8 = 4;
const MEDIUM_UPDATES: u8 = 6;
const LARGE_UPDATES: u8 = 8;
const SMALL_BIAS_SHIFT: u32 = 3;
const MEDIUM_BIAS_SHIFT: u32 = 4;
const LARGE_BIAS_SHIFT: u32 = 5;
const COMPANION_INITIAL_UPDATES: u8 = 2;
const COMPANION_INITIAL_CHANNELS: [u8; 3] = [30, 30, 7];
const SOUND_NEAR_LIMIT: i16 = 800;
const SOUND_FAR_LIMIT: i16 = 1_300;
const MAX_EFFECTS: usize = 3;
const MAX_LISTENERS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntroDestructionCapacityError {
    pub required_slots: usize,
    pub available_slots: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroExplosionSize {
    Small,
    Medium,
    Large,
    Huge,
}

impl IntroExplosionSize {
    pub const fn shape(self) -> ShapeId {
        const SMALL: ShapeId = ShapeId::from_catalog_index(9);
        const MEDIUM: ShapeId = ShapeId::from_catalog_index(10);
        const LARGE: ShapeId = ShapeId::from_catalog_index(11);
        const HUGE: ShapeId = ShapeId::from_catalog_index(12);
        match self {
            Self::Small => SMALL,
            Self::Medium => MEDIUM,
            Self::Large => LARGE,
            Self::Huge => HUGE,
        }
    }

    const fn updates(self) -> u8 {
        match self {
            Self::Small => SMALL_UPDATES,
            Self::Medium => MEDIUM_UPDATES,
            Self::Large | Self::Huge => LARGE_UPDATES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntroExplosionProfile {
    pub size: IntroExplosionSize,
    pub sprite_size_bias: u8,
}

impl IntroExplosionProfile {
    pub fn for_shape(shape: ShapeId) -> Option<Self> {
        let header = SHAPE_DATA.get(shape.catalog_index() as usize)?;
        let diameter = header.bounds[0].max(header.bounds[1]).wrapping_mul(2);
        let (size, upper_limit, shift) = if diameter >= LARGE_DIAMETER_LIMIT {
            return Some(Self {
                size: IntroExplosionSize::Huge,
                sprite_size_bias: 0,
            });
        } else if diameter >= MEDIUM_DIAMETER_LIMIT {
            (
                IntroExplosionSize::Large,
                LARGE_DIAMETER_LIMIT,
                LARGE_BIAS_SHIFT,
            )
        } else if diameter >= SMALL_DIAMETER_LIMIT {
            (
                IntroExplosionSize::Medium,
                MEDIUM_DIAMETER_LIMIT,
                MEDIUM_BIAS_SHIFT,
            )
        } else {
            (
                IntroExplosionSize::Small,
                SMALL_DIAMETER_LIMIT,
                SMALL_BIAS_SHIFT,
            )
        };
        // The subtraction wraps before a logical shift. Replacing it with a
        // signed scale adjustment changes the low-byte presentation value.
        let scaled = diameter
            .wrapping_sub(upper_limit)
            .checked_shr(u32::from(header.shift))
            .unwrap_or(0);
        Some(Self {
            size,
            sprite_size_bias: (scaled >> shift) as u8,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroExplosionVolume {
    Near,
    Middle,
    Far,
}

impl IntroExplosionVolume {
    pub fn between(position: Vector3, listener: Vector3) -> Self {
        let distance = sf2_xz_angle_distance(
            listener.x.wrapping_sub(position.x),
            listener.z.wrapping_sub(position.z),
        );
        // Source branches test the sign of the wrapped subtraction, not an
        // unbounded distance or a floating-point Euclidean length.
        if distance.wrapping_sub(SOUND_FAR_LIMIT) >= 0 {
            Self::Far
        } else if distance.wrapping_sub(SOUND_NEAR_LIMIT) >= 0 {
            Self::Middle
        } else {
            Self::Near
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IntroDestructionContext {
    pub primary_listener: Vector3,
    pub secondary_listener: Option<Vector3>,
    /// Free slots before the dying actor is cleaned up, not after it.
    pub available_slots: usize,
    pub suppress_effects: bool,
    pub compensate_scroll: bool,
    pub scroll: Vector3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroExplosionAppearance {
    Sprite {
        size: IntroExplosionSize,
        size_bias: u8,
    },
    /// Source companion with no mesh and three separate style channels.
    /// The renderer's interpretation is a separate reconstruction boundary.
    Companion { channels: [u8; 3] },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroExplosionPhase {
    Animating { age: u8, limit: u8 },
    AwaitingDestruction,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntroExplosionActor {
    pub position: Vector3,
    pub appearance: IntroExplosionAppearance,
    pub color_frame: u8,
    phase: IntroExplosionPhase,
}

impl IntroExplosionActor {
    pub fn phase(&self) -> IntroExplosionPhase {
        self.phase
    }

    pub fn is_finished(&self) -> bool {
        self.phase == IntroExplosionPhase::Finished
    }

    pub fn shape(&self) -> ShapeId {
        const NO_MESH: ShapeId = ShapeId::from_catalog_index(0);
        match self.appearance {
            IntroExplosionAppearance::Sprite { size, .. } => size.shape(),
            IntroExplosionAppearance::Companion { .. } => NO_MESH,
        }
    }

    fn tick_animation(&mut self, context: &IntroDestructionContext) {
        match self.phase {
            IntroExplosionPhase::Animating { age, limit } => {
                if context.compensate_scroll {
                    self.position.x = self
                        .position
                        .x
                        .wrapping_sub(context.scroll.x.wrapping_mul(2));
                    self.position.z = self
                        .position
                        .z
                        .wrapping_sub(context.scroll.z.wrapping_mul(2));
                }
                let age = age + 1;
                if age < limit {
                    self.color_frame = self.color_frame.wrapping_add(1);
                    self.phase =
                        if matches!(self.appearance, IntroExplosionAppearance::Companion { .. }) {
                            // Its constructor leaves health at zero. The one-time
                            // newborn exemption permits this animation update, but
                            // the next dispatch is common destruction, not A279.
                            IntroExplosionPhase::AwaitingDestruction
                        } else {
                            IntroExplosionPhase::Animating { age, limit }
                        };
                } else {
                    self.phase = IntroExplosionPhase::Finished;
                }
            }
            IntroExplosionPhase::AwaitingDestruction => {
                unreachable!("family owns destruction handoff")
            }
            IntroExplosionPhase::Finished => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntroDestructionEffects {
    actors: Vec<IntroExplosionActor>,
}

impl IntroDestructionEffects {
    /// Construct in original allocation order. The caller performs the first
    /// tick in this same scene update, after construction and before cleanup.
    pub fn spawn(
        profile: IntroExplosionProfile,
        position: Vector3,
        context: &IntroDestructionContext,
    ) -> Result<(Self, Vec<IntroExplosionVolume>), IntroDestructionCapacityError> {
        let required_slots = if context.suppress_effects {
            0
        } else if profile.size == IntroExplosionSize::Huge {
            2
        } else {
            1
        };
        // Retail enters its out-of-objects diagnostic instead of returning
        // normally. Surface an explicit port error, never a successful drop.
        if context.available_slots < required_slots {
            return Err(IntroDestructionCapacityError {
                required_slots,
                available_slots: context.available_slots,
            });
        }
        let mut actors = Vec::with_capacity(MAX_EFFECTS);
        let mut audio = Vec::with_capacity(MAX_LISTENERS);
        if !context.suppress_effects {
            if let Some(listener) = context.secondary_listener {
                audio.push(IntroExplosionVolume::between(position, listener));
            }
            audio.push(IntroExplosionVolume::between(
                position,
                context.primary_listener,
            ));
            if context.available_slots != 0 {
                actors.push(IntroExplosionActor {
                    position,
                    appearance: IntroExplosionAppearance::Sprite {
                        size: profile.size,
                        size_bias: profile.sprite_size_bias,
                    },
                    color_frame: 0,
                    phase: IntroExplosionPhase::Animating {
                        age: 0,
                        limit: profile.size.updates(),
                    },
                });
                if profile.size == IntroExplosionSize::Huge && context.available_slots > 1 {
                    actors.push(IntroExplosionActor {
                        position,
                        appearance: IntroExplosionAppearance::Companion {
                            channels: COMPANION_INITIAL_CHANNELS,
                        },
                        color_frame: 0,
                        phase: IntroExplosionPhase::Animating {
                            age: 0,
                            limit: COMPANION_INITIAL_UPDATES,
                        },
                    });
                }
            }
        }
        Ok((Self { actors }, audio))
    }

    pub fn actors(&self) -> impl Iterator<Item = &IntroExplosionActor> {
        self.actors.iter()
    }

    pub fn is_finished(&self) -> bool {
        self.actors.iter().all(IntroExplosionActor::is_finished)
    }

    pub fn tick(
        &mut self,
        context: &IntroDestructionContext,
    ) -> Result<Vec<IntroExplosionVolume>, IntroDestructionCapacityError> {
        let mut audio = Vec::new();
        for index in (0..self.actors.len()).rev() {
            if self.actors[index].phase == IntroExplosionPhase::AwaitingDestruction {
                const NO_MESH: ShapeId = ShapeId::from_catalog_index(0);
                let profile = IntroExplosionProfile::for_shape(NO_MESH)
                    .expect("no-mesh entry belongs to the validated catalog");
                let (mut children, events) = Self::spawn(
                    profile,
                    self.actors[index].position,
                    &IntroDestructionContext {
                        suppress_effects: false,
                        ..*context
                    },
                )?;
                self.actors[index].phase = IntroExplosionPhase::Finished;
                audio.extend(events);
                // The new child runs before the next older sibling, in this
                // same update. Retain retired entries as separate lifetimes.
                for child in &mut children.actors {
                    child.tick_animation(context);
                }
                self.actors.extend(children.actors);
            } else {
                self.actors[index].tick_animation(context);
            }
        }
        Ok(audio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn large_profile() -> IntroExplosionProfile {
        IntroExplosionProfile::for_shape(ShapeId::from_catalog_index(64)).unwrap()
    }

    #[test]
    fn companion_hands_off_once_and_all_three_lifetimes_remain_distinct() {
        let context = IntroDestructionContext {
            available_slots: MAX_EFFECTS,
            compensate_scroll: true,
            scroll: Vector3 { x: 3, y: 71, z: 5 },
            ..Default::default()
        };
        let (mut family, audio) =
            IntroDestructionEffects::spawn(large_profile(), Vector3::default(), &context).unwrap();
        assert_eq!(audio.len(), 1);
        assert!(family.tick(&context).unwrap().is_empty());
        assert_eq!(family.actors.len(), 2);
        assert_eq!(
            family.actors[1].phase(),
            IntroExplosionPhase::AwaitingDestruction
        );
        let companion_position = family.actors[1].position;
        assert_eq!(family.tick(&context).unwrap().len(), 1);
        assert_eq!(family.actors.len(), MAX_EFFECTS);
        assert!(family.actors[1].is_finished());
        assert_eq!(family.actors[1].position, companion_position);
        assert_eq!(family.actors[2].color_frame, 1);
        assert_eq!(family.actors[2].shape(), IntroExplosionSize::Small.shape());
        for _ in 2..LARGE_UPDATES {
            assert!(family.tick(&context).unwrap().is_empty());
        }
        assert!(family.is_finished());
        let before = family.clone();
        assert!(family.tick(&context).unwrap().is_empty());
        assert_eq!(family, before);
    }

    #[test]
    fn secondary_allocation_failure_is_not_a_successful_missing_sprite() {
        let mut context = IntroDestructionContext {
            available_slots: 2,
            ..Default::default()
        };
        let (mut family, _) =
            IntroDestructionEffects::spawn(large_profile(), Vector3::default(), &context).unwrap();
        family.tick(&context).unwrap();
        context.available_slots = 0;
        let before = family.clone();
        assert_eq!(
            family.tick(&context),
            Err(IntroDestructionCapacityError {
                required_slots: 1,
                available_slots: 0,
            })
        );
        assert_eq!(family, before);
    }

    #[test]
    fn suppressed_destruction_needs_no_slots_and_emits_no_sound() {
        let context = IntroDestructionContext {
            suppress_effects: true,
            ..Default::default()
        };
        let (mut family, audio) =
            IntroDestructionEffects::spawn(large_profile(), Vector3::default(), &context).unwrap();
        assert!(audio.is_empty());
        assert!(family.is_finished());
        assert!(family.tick(&context).unwrap().is_empty());
    }
}
