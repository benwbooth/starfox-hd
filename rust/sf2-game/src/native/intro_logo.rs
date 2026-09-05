//! Source-authored Nintendo-logo assembly and arrival motion.
//!
//! These operate once per scene update, not per display frame. Visibility,
//! materials, dispersal and the attract-scene camera are separate systems;
//! these types do not yet replace the application's recorded intro.

use super::object::{Angle, ShapeId, Vector3};
use super::render::Rotation;

const GLYPH_COUNT: usize = 9;
const SPACING_SCALE: i16 = 8;
const SWEEP_START_X: i16 = -750;
const HOLD_UPDATES: u8 = 92;
const APPROACH_UPDATES: u8 = 20;
const APPROACH_DEPTH_STEP: i16 = 50;
const PITCH_STEP: i8 = 8;

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
        if let LogoArrivalPhase::Approaching { updates_left } = self.phase {
            self.position.z = self.position.z.wrapping_add(APPROACH_DEPTH_STEP);
            self.rotation.pitch = self.rotation.pitch.wrapping_add(PITCH_STEP);
            if updates_left > 1 {
                self.phase = LogoArrivalPhase::Approaching {
                    updates_left: updates_left - 1,
                };
                return;
            }
            // The final approach iteration does not yield. It enters the
            // settling test in this same update, possibly rotating again.
            self.phase = LogoArrivalPhase::Settling;
        }
        if self.phase == LogoArrivalPhase::Settling {
            if self.rotation.pitch == Angle::ZERO {
                self.phase = LogoArrivalPhase::Holding;
            } else {
                self.rotation.pitch = self.rotation.pitch.wrapping_add(PITCH_STEP);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
