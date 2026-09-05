//! Source-authored Nintendo-logo assembly and actor motion.
//!
//! These operate once per scene update, not per display frame. The renderer
//! and attract-scene camera remain separate systems; these types do not yet
//! replace the application's recorded intro.

use super::object::{Angle, ShapeId, Vector3};
use super::render::{MaterialSetId, Rotation};
use super::state::RandomState;

const GLYPH_COUNT: usize = 9;
const SPACING_SCALE: i16 = 8;
const SWEEP_START_X: i16 = -750;
const HOLD_UPDATES: u8 = 92;
const APPROACH_UPDATES: u8 = 20;
const APPROACH_DEPTH_STEP: i16 = 50;
const PITCH_STEP: i8 = 8;
const REVEAL_UPDATES: u8 = 10;
const TEXTURE_SCROLL_STEP: u8 = 4;
const DEPARTURE_UPDATES: u8 = 40;
const DEPARTURE_SPEED: u8 = 40;
const SPIN_RANDOM_MASK: u8 = 31;
const SPIN_RANDOM_ORIGIN: i8 = -16;
const PRIMARY_DEPTH_OFFSET: u8 = 1;
const SECONDARY_DEPTH_OFFSET: u8 = 3;
const LOGO_MATERIAL: MaterialSetId = MaterialSetId::from_catalog_token(34_519);
const SWEEP_DEPTH_OFFSET: i16 = 1_500;
const SWEEP_HORIZONTAL_OFFSET: i16 = -100;
const SWEEP_ROLL: Angle = Angle::from_units(80);
const SWEEP_DELAY_UPDATES: u8 = 19;
const SWEEP_TRAVERSE_UPDATES: u8 = 13;
const SWEEP_HORIZONTAL_STEP: i16 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoGlyph {
    Outline,
    CapitalN,
    LowercaseI,
    LowercaseN,
    LowercaseT,
    LowercaseE,
    LowercaseD,
    LowercaseO,
}

impl LogoGlyph {
    pub const fn shape(self) -> ShapeId {
        // Decoded catalog identities, not source pointers.
        const OUTLINE: ShapeId = ShapeId::from_catalog_index(371);
        const CAPITAL_N: ShapeId = ShapeId::from_catalog_index(376);
        const LOWERCASE_I: ShapeId = ShapeId::from_catalog_index(375);
        const LOWERCASE_N: ShapeId = ShapeId::from_catalog_index(377);
        const LOWERCASE_T: ShapeId = ShapeId::from_catalog_index(379);
        const LOWERCASE_E: ShapeId = ShapeId::from_catalog_index(374);
        const LOWERCASE_D: ShapeId = ShapeId::from_catalog_index(373);
        const LOWERCASE_O: ShapeId = ShapeId::from_catalog_index(378);
        match self {
            Self::Outline => OUTLINE,
            Self::CapitalN => CAPITAL_N,
            Self::LowercaseI => LOWERCASE_I,
            Self::LowercaseN => LOWERCASE_N,
            Self::LowercaseT => LOWERCASE_T,
            Self::LowercaseE => LOWERCASE_E,
            Self::LowercaseD => LOWERCASE_D,
            Self::LowercaseO => LOWERCASE_O,
        }
    }
}

const GLYPHS: [(LogoGlyph, i8); GLYPH_COUNT] = [
    (LogoGlyph::Outline, -70),
    (LogoGlyph::CapitalN, 19),
    (LogoGlyph::LowercaseI, 18),
    (LogoGlyph::LowercaseN, 19),
    (LogoGlyph::LowercaseT, 19),
    (LogoGlyph::LowercaseE, 23),
    (LogoGlyph::LowercaseN, 23),
    (LogoGlyph::LowercaseD, 23),
    (LogoGlyph::LowercaseO, 0),
];

/// Both authored layers are spawned at the same position with the same mesh.
/// Their distinct material and visibility policies are applied by the actors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogoGlyphPair {
    pub glyph: LogoGlyph,
    pub position: Vector3,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LogoAssemblyEvents {
    pub glyph_pair: Option<LogoGlyphPair>,
    pub sweep_position: Option<Vector3>,
    pub release: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssemblyPhase {
    Building { next_glyph: usize },
    Holding { updates_left: u8 },
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NintendoLogoAssembly {
    position: Vector3,
    phase: AssemblyPhase,
}

impl NintendoLogoAssembly {
    pub fn new(position: Vector3) -> Self {
        Self {
            position,
            phase: AssemblyPhase::Building { next_glyph: 0 },
        }
    }

    pub fn tick(&mut self) -> LogoAssemblyEvents {
        let mut events = LogoAssemblyEvents::default();
        if let AssemblyPhase::Building { next_glyph } = self.phase {
            let (glyph, spacing) = GLYPHS[next_glyph];
            events.glyph_pair = Some(LogoGlyphPair {
                glyph,
                position: self.position,
            });
            self.position.x = self
                .position
                .x
                .wrapping_add(i16::from(spacing) * SPACING_SCALE);
            let next_glyph = next_glyph + 1;
            if next_glyph < GLYPH_COUNT {
                self.phase = AssemblyPhase::Building { next_glyph };
                return events;
            }
            // The final loop iteration falls through immediately: the sweep
            // spawns and the first hold update occur alongside the last pair.
            self.position.x = SWEEP_START_X;
            events.sweep_position = Some(self.position);
            self.phase = AssemblyPhase::Holding {
                updates_left: HOLD_UPDATES,
            };
        }
        if let AssemblyPhase::Holding { updates_left } = self.phase {
            if updates_left == 0 {
                events.release = true;
                self.phase = AssemblyPhase::Complete;
            } else {
                self.phase = AssemblyPhase::Holding {
                    updates_left: updates_left - 1,
                };
            }
        }
        events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoArrivalPhase {
    Approaching { updates_left: u8 },
    Settling,
    Holding,
}

/// The common translation/rotation segment of the two logo layers. The
/// release-controlled dispersal segment begins after this arrival segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NintendoLogoArrival {
    pub position: Vector3,
    pub rotation: Rotation,
    phase: LogoArrivalPhase,
}

impl NintendoLogoArrival {
    pub fn new(position: Vector3, rotation: Rotation) -> Self {
        Self {
            position,
            rotation,
            phase: LogoArrivalPhase::Approaching {
                updates_left: APPROACH_UPDATES,
            },
        }
    }

    pub fn phase(&self) -> LogoArrivalPhase {
        self.phase
    }

    pub fn tick(&mut self) {
        advance_arrival(&mut self.position, &mut self.rotation, &mut self.phase);
    }
}

fn advance_arrival(position: &mut Vector3, rotation: &mut Rotation, phase: &mut LogoArrivalPhase) {
    if let LogoArrivalPhase::Approaching { updates_left } = *phase {
        position.z = position.z.wrapping_add(APPROACH_DEPTH_STEP);
        rotation.pitch = rotation.pitch.wrapping_add(PITCH_STEP);
        if updates_left > 1 {
            *phase = LogoArrivalPhase::Approaching {
                updates_left: updates_left - 1,
            };
            return;
        }
        // The final approach iteration does not yield. It enters the
        // settling test in this same update, possibly rotating again.
        *phase = LogoArrivalPhase::Settling;
    }
    if *phase == LogoArrivalPhase::Settling {
        if rotation.pitch == Angle::ZERO {
            *phase = LogoArrivalPhase::Holding;
        } else {
            rotation.pitch = rotation.pitch.wrapping_add(PITCH_STEP);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoLayer {
    Primary,
    Secondary,
}

/// The source selects two special drawing policies during assembly, then
/// restores normal drawing for the departing primary layer. Their GSU
/// rasterization still needs a renderer-side reconstruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoDrawStyle {
    PrimaryAssembly,
    SecondaryAssembly,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoExitPolicy {
    Disperse,
    Remove,
}

/// Common scene scrolling is separate from each actor's own velocity.
/// The selected player can suppress horizontal scrolling but not depth.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LogoSceneScroll {
    pub horizontal: i16,
    pub depth: i16,
    pub horizontal_locked: bool,
}

impl LogoSceneScroll {
    fn apply(self, position: &mut Vector3) {
        if !self.horizontal_locked {
            position.x = position.x.wrapping_add(self.horizontal);
        }
        position.z = position.z.wrapping_add(self.depth);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoSweepPhase {
    Delayed { updates_left: u8 },
    Traversing { updates_left: u8 },
    Holding,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NintendoLogoSweep {
    pub position: Vector3,
    pub rotation: Rotation,
    phase: LogoSweepPhase,
}

impl NintendoLogoSweep {
    pub const SHAPE: ShapeId = ShapeId::from_catalog_index(48);

    pub fn new(mut position: Vector3) -> Self {
        position.x = position.x.wrapping_add(SWEEP_HORIZONTAL_OFFSET);
        position.z = position.z.wrapping_add(SWEEP_DEPTH_OFFSET);
        Self {
            position,
            rotation: Rotation {
                roll: SWEEP_ROLL,
                ..Rotation::default()
            },
            phase: LogoSweepPhase::Delayed {
                updates_left: SWEEP_DELAY_UPDATES,
            },
        }
    }

    pub fn phase(&self) -> LogoSweepPhase {
        self.phase
    }

    /// Returns true only on the update that removes the sweep.
    pub fn tick(&mut self, release: bool, scroll: LogoSceneScroll) -> bool {
        if self.phase == LogoSweepPhase::Finished {
            return false;
        }
        if let LogoSweepPhase::Delayed { updates_left } = self.phase {
            if updates_left > 0 {
                self.phase = LogoSweepPhase::Delayed {
                    updates_left: updates_left - 1,
                };
                scroll.apply(&mut self.position);
                return false;
            }
            self.phase = LogoSweepPhase::Traversing {
                updates_left: SWEEP_TRAVERSE_UPDATES,
            };
        }
        if let LogoSweepPhase::Traversing { updates_left } = self.phase {
            self.position.x = self.position.x.wrapping_add(SWEEP_HORIZONTAL_STEP);
            self.phase = if updates_left > 1 {
                LogoSweepPhase::Traversing {
                    updates_left: updates_left - 1,
                }
            } else {
                LogoSweepPhase::Holding
            };
        }
        if self.phase == LogoSweepPhase::Holding && release {
            self.phase = LogoSweepPhase::Finished;
            return true;
        }
        scroll.apply(&mut self.position);
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoActorPhase {
    Arriving(LogoArrivalPhase),
    Dispersing { updates_left: u8 },
    Finished,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LogoActorEvents {
    pub spawn_outline_child: bool,
    pub finished: bool,
}

/// Complete motion/visibility/scroll lifecycle of one authored logo layer.
/// The outline's independently scheduled child is emitted as a spawn event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NintendoLogoActor {
    pub glyph: LogoGlyph,
    pub layer: LogoLayer,
    pub position: Vector3,
    pub rotation: Rotation,
    pub velocity: Vector3,
    pub visible: bool,
    pub depth_offset: u8,
    pub material_override: Option<MaterialSetId>,
    pub texture_scroll_y: u8,
    pub draw_style: LogoDrawStyle,
    pub exit_policy: LogoExitPolicy,
    phase: LogoActorPhase,
    reveal_updates_left: u8,
    pitch_spin: i8,
    yaw_spin: i8,
    outline_child_pending: bool,
}

impl NintendoLogoActor {
    pub fn new(pair: LogoGlyphPair, layer: LogoLayer, rotation: Rotation) -> Self {
        let primary = layer == LogoLayer::Primary;
        Self {
            glyph: pair.glyph,
            layer,
            position: pair.position,
            rotation,
            velocity: Vector3::default(),
            visible: !primary,
            depth_offset: if primary {
                PRIMARY_DEPTH_OFFSET
            } else {
                SECONDARY_DEPTH_OFFSET
            },
            material_override: primary.then_some(LOGO_MATERIAL),
            texture_scroll_y: 0,
            draw_style: if primary {
                LogoDrawStyle::PrimaryAssembly
            } else {
                LogoDrawStyle::SecondaryAssembly
            },
            exit_policy: LogoExitPolicy::Disperse,
            phase: LogoActorPhase::Arriving(LogoArrivalPhase::Approaching {
                updates_left: APPROACH_UPDATES,
            }),
            reveal_updates_left: if primary { REVEAL_UPDATES } else { 0 },
            pitch_spin: 0,
            yaw_spin: 0,
            outline_child_pending: primary && pair.glyph == LogoGlyph::Outline,
        }
    }

    pub fn phase(&self) -> LogoActorPhase {
        self.phase
    }

    pub fn is_visible(&self) -> bool {
        self.visible && self.phase != LogoActorPhase::Finished
    }

    pub fn tick(
        &mut self,
        release: bool,
        scroll: LogoSceneScroll,
        random: &mut RandomState,
    ) -> LogoActorEvents {
        let mut events = LogoActorEvents {
            spawn_outline_child: std::mem::take(&mut self.outline_child_pending),
            finished: false,
        };
        if self.phase == LogoActorPhase::Finished {
            return events;
        }
        if let LogoActorPhase::Arriving(ref mut phase) = self.phase {
            advance_arrival(&mut self.position, &mut self.rotation, phase);
            if *phase == LogoArrivalPhase::Holding && release {
                if self.layer == LogoLayer::Secondary || self.exit_policy == LogoExitPolicy::Remove
                {
                    self.phase = LogoActorPhase::Finished;
                    events.finished = true;
                    return events;
                }
                let direction_pitch = Angle::from_units(random.next_byte());
                let direction_yaw = Angle::from_units(random.next_byte());
                self.velocity = super::game::flight_velocity(
                    direction_pitch,
                    direction_yaw,
                    DEPARTURE_SPEED,
                    1,
                );
                // Two pushes followed by pulls to pitch then yaw exchange
                // the retained axes. The departure direction is independent.
                std::mem::swap(&mut self.rotation.pitch, &mut self.rotation.yaw);
                self.pitch_spin =
                    (random.next_byte() & SPIN_RANDOM_MASK) as i8 + SPIN_RANDOM_ORIGIN;
                self.yaw_spin = (random.next_byte() & SPIN_RANDOM_MASK) as i8 + SPIN_RANDOM_ORIGIN;
                self.draw_style = LogoDrawStyle::Normal;
                self.phase = LogoActorPhase::Dispersing {
                    updates_left: DEPARTURE_UPDATES,
                };
            }
        }
        if let LogoActorPhase::Dispersing { updates_left } = self.phase {
            self.rotation.pitch = self.rotation.pitch.wrapping_add(self.pitch_spin);
            self.rotation.yaw = self.rotation.yaw.wrapping_add(self.yaw_spin);
            if updates_left == 1 {
                // End does not run the usual movement or trigger pass:
                // forty rotations accompany only thirty-nine translations.
                self.phase = LogoActorPhase::Finished;
                events.finished = true;
                return events;
            }
            self.phase = LogoActorPhase::Dispersing {
                updates_left: updates_left - 1,
            };
        }
        scroll.apply(&mut self.position);
        self.position.x = self.position.x.wrapping_add(self.velocity.x);
        self.position.y = self.position.y.wrapping_add(self.velocity.y);
        self.position.z = self.position.z.wrapping_add(self.velocity.z);
        self.texture_scroll_y = self.texture_scroll_y.wrapping_add(TEXTURE_SCROLL_STEP);
        if self.reveal_updates_left > 0 {
            self.reveal_updates_left -= 1;
            if self.reveal_updates_left == 0 {
                self.visible = true;
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_layers_reveal_separately_and_scroll_the_texture() {
        let pair = LogoGlyphPair {
            glyph: LogoGlyph::CapitalN,
            position: Vector3::default(),
        };
        let mut primary = NintendoLogoActor::new(pair, LogoLayer::Primary, Rotation::default());
        let mut secondary = NintendoLogoActor::new(pair, LogoLayer::Secondary, Rotation::default());
        let mut random = RandomState::default();
        for update in 1..=REVEAL_UPDATES {
            primary.tick(false, LogoSceneScroll::default(), &mut random);
            secondary.tick(false, LogoSceneScroll::default(), &mut random);
            assert_eq!(primary.is_visible(), update == REVEAL_UPDATES);
            assert!(secondary.is_visible());
            assert_eq!(primary.texture_scroll_y, update * TEXTURE_SCROLL_STEP);
            assert_eq!(secondary.texture_scroll_y, primary.texture_scroll_y);
        }
    }

    #[test]
    fn secondary_layer_exits_without_drawing_or_consuming_randomness() {
        let pair = LogoGlyphPair {
            glyph: LogoGlyph::Outline,
            position: Vector3::default(),
        };
        let mut actor = NintendoLogoActor::new(pair, LogoLayer::Secondary, Rotation::default());
        let original_random = RandomState::default();
        let mut random = original_random;
        for _ in 0..APPROACH_UPDATES * 2 {
            actor.tick(true, LogoSceneScroll::default(), &mut random);
        }
        assert_eq!(actor.phase(), LogoActorPhase::Finished);
        assert!(!actor.is_visible());
        assert_eq!(random, original_random);
    }

    #[test]
    fn final_approach_iteration_also_starts_settling() {
        let mut actor = NintendoLogoArrival::new(Vector3::default(), Rotation::default());
        for _ in 0..APPROACH_UPDATES {
            actor.tick();
        }
        assert_eq!(
            actor.position.z,
            i16::from(APPROACH_UPDATES) * APPROACH_DEPTH_STEP
        );
        assert_eq!(
            actor.rotation.pitch.units(),
            (APPROACH_UPDATES + 1) * PITCH_STEP as u8
        );
        assert_eq!(actor.phase(), LogoArrivalPhase::Settling);
    }

    #[test]
    fn assembly_emits_one_release_and_never_restarts() {
        let mut assembly = NintendoLogoAssembly::new(Vector3::default());
        let mut pairs = 0;
        let mut sweeps = 0;
        let mut releases = 0;
        for _ in 0..GLYPH_COUNT + usize::from(HOLD_UPDATES) * 2 {
            let events = assembly.tick();
            pairs += usize::from(events.glyph_pair.is_some());
            sweeps += usize::from(events.sweep_position.is_some());
            releases += usize::from(events.release);
        }
        assert_eq!(pairs, GLYPH_COUNT);
        assert_eq!(sweeps, 1);
        assert_eq!(releases, 1);
    }
}
