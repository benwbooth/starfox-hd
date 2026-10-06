//! Native, object-owned SF1 particle pool (`MPART.MC`).
//!
//! Particle offsets are in view space, not world space. Each visible owner
//! advances and draws its particles during the painter walk. The completed
//! scene then ages the pool and retires particles whose owners were not seen.

use crate::cockpit_hud::CockpitHudLine;
use crate::particle_tables::{PARTICLE_CIRCLE, PARTICLE_COUNT_UNDERFLOW, PARTICLE_FADE};
use crate::point_field::{PointIdentity, PointPixel};
use crate::snes_trig::gsu_fmult_q15;
use std::num::NonZeroU16;

pub const PARTICLE_CAPACITY: usize = 300;
/// Source `al_sflags` / `dl_sflags` particle-role bit.
pub const PARTICLE_OBJECT_FLAG: u8 = 0x10;
pub const PARTICLE_OWNER_SEEN: u8 = 1;
pub const PARTICLE_GRAVITY: u8 = 2;
pub const PARTICLE_LINE: u8 = 4;
pub const PARTICLE_FADE_OUT: u8 = 16;
const GRAVITY_STEP: i8 = 2;
const FADE_LIFE_LIMIT: u8 = 32;
const LIFE_VARIATION_MASK: u16 = 15;
const LIFE_VARIATION_BIAS: u8 = 7;
const FIRE_LATERAL_MASK: u16 = 15;
const FIRE_LATERAL_BIAS: i8 = 7;
const FIRE_VERTICAL_MASK: u16 = 7;
const FIRE_UP_VELOCITY: i8 = -40;
const FIRE_DOWN_VELOCITY: i8 = 50;
const SMALL_DEPTH_MASK: u16 = 31;
const SMALL_DEPTH_BIAS: i8 = 15;
const LARGE_DEPTH_MASK: u16 = 63;
const LARGE_DEPTH_BIAS: i8 = 31;
const EXPLOSION_COLORS: [u8; 2] = [4, 14];
const PROJECTION_NEAR: i16 = 256;
const PROJECTION_MAX: i16 = 12_288;
const PROJECTION_NUMERATOR: i32 = 32_767 * 256;
const PROJECTION_CENTER: [i16; 2] = [112, 96];
// Two adjacent pixels must fit. The original excludes the outermost column
// and row even after allowing for the two-pixel particle footprint.
const PROJECTION_LIMIT: [u16; 2] = [222, 190];
const PALETTE_MASK: u8 = 15;
/// MAIN.ASM `initgame3d_l` restarts this stream without erasing its pool.
pub const PARTICLE_SCENE_RANDOM_SEED: u16 = 4_660;

/// One-based native object slot. As in the source, recycling a slot retains
/// the same particle owner identity; allocation-lifetime interpolation tokens
/// are intentionally not simulation ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleOwner(pub NonZeroU16);

impl ParticleOwner {
    pub fn new(object_id: u16) -> Option<Self> {
        NonZeroU16::new(object_id).map(Self)
    }

    pub fn object_id(self) -> u16 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleKind {
    LargeGeneratedExplosion,
    FireUp,
    FireDown,
    LargeTableExplosion,
    SmallGeneratedExplosion,
    SmallTableExplosion,
    FastTableExplosion,
}

impl ParticleKind {
    /// Source zero means continue an existing emitter, not allocate particles.
    pub const fn from_source(value: u8) -> Option<Self> {
        match value & 7 {
            0 => None,
            1 => Some(Self::LargeGeneratedExplosion),
            2 => Some(Self::FireUp),
            3 => Some(Self::FireDown),
            4 => Some(Self::LargeTableExplosion),
            5 => Some(Self::SmallGeneratedExplosion),
            6 => Some(Self::SmallTableExplosion),
            7 => Some(Self::FastTableExplosion),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Particle {
    pub life: u8,
    pub flags: u8,
    pub palette_index: u8,
    pub velocity: [i8; 3],
    pub position: [i16; 3],
    pub owner: Option<ParticleOwner>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleEmitter {
    pub owner: ParticleOwner,
    pub kind: Option<ParticleKind>,
    pub amount: u8,
    pub life: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParticleWork {
    pub allocation_slots_scanned: u32,
    pub particles_allocated: u32,
    pub owner_slots_scanned: u32,
    pub particles_visited: u32,
    pub aging_slots_scanned: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParticleFrame {
    pub draws: Vec<ParticleDraw>,
    pub work: ParticleWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticlePrimitive {
    Dot {
        corner: [u8; 2],
        palette_index: u8,
    },
    Line {
        start: [u8; 2],
        end: [u8; 2],
        palette_index: u8,
    },
}

impl ParticlePrimitive {
    pub fn visit_pixels(self, mut plot: impl FnMut(PointPixel)) {
        let mut sample = |x, y, palette_index| {
            plot(PointPixel {
                x,
                y,
                palette_index,
                identity: PointIdentity::Untracked,
            })
        };
        match self {
            Self::Dot {
                corner: [x, y],
                palette_index,
            } => {
                sample(x, y, palette_index);
                sample(x + 1, y, palette_index);
                sample(x, y + 1, palette_index);
                sample(x + 1, y + 1, palette_index);
            }
            Self::Line {
                start,
                end,
                palette_index,
            } => {
                // MPART and MHUD use the same inclusive integer line walk.
                CockpitHudLine {
                    start,
                    end,
                    palette_index,
                }
                .visit_pixels(|x, y| sample(x, y, palette_index));
            }
        }
    }
}

/// Completed source draws at one owner's position in the painter ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticleDraw {
    pub owner: ParticleOwner,
    pub primitives: Vec<ParticlePrimitive>,
    pub particles_visited: u16,
}

impl ParticleDraw {
    pub fn pixels(&self) -> Vec<PointPixel> {
        let mut pixels = Vec::new();
        for primitive in &self.primitives {
            primitive.visit_pixels(|pixel| {
                if pixel.palette_index != 0 {
                    pixels.push(pixel);
                }
            });
        }
        pixels
    }
}

#[derive(Debug, Clone)]
pub struct ParticleField {
    particles: [Particle; PARTICLE_CAPACITY],
    random_state: u16,
}

impl Default for ParticleField {
    fn default() -> Self {
        Self::from_state([Particle::default(); PARTICLE_CAPACITY], 0)
    }
}

impl ParticleField {
    pub fn from_state(particles: [Particle; PARTICLE_CAPACITY], random_state: u16) -> Self {
        Self {
            particles,
            random_state,
        }
    }

    pub fn particles(&self) -> &[Particle; PARTICLE_CAPACITY] {
        &self.particles
    }

    pub fn random_state(&self) -> u16 {
        self.random_state
    }

    pub fn initialize_scene(&mut self) {
        self.random_state = PARTICLE_SCENE_RANDOM_SEED;
    }

    /// One completed scene, independent of renderer refresh/interpolation.
    /// Disabled particles neither draw nor age, and retain their pool/seed.
    pub fn capture_scene(
        &mut self,
        enabled: bool,
        entries: &[crate::DrawListEntry],
        camera: [i16; 3],
        rotation: [u16; 3],
    ) -> ParticleFrame {
        let mut frame = ParticleFrame::default();
        if !enabled {
            return frame;
        }
        let matrix = crate::snes_trig::zxy_matrix_q15_fine(rotation[0], rotation[1], rotation[2]);
        for index in crate::draw_order::source_painter_order(entries, camera, rotation) {
            let entry = &entries[index];
            if entry.flags & crate::dl_flags::VISIBLE == 0
                || entry.sflags & PARTICLE_OBJECT_FLAG == 0
            {
                continue;
            }
            let Some(emitter) = entry.particles else {
                continue;
            };
            if let Some(kind) = emitter.kind {
                let work = self.generate(emitter.owner, kind, emitter.amount, emitter.life);
                frame.work.allocation_slots_scanned += work.allocation_slots_scanned;
                frame.work.particles_allocated += work.particles_allocated;
            }
            let origin = crate::draw_order::view_position(entry, camera, matrix);
            let draw = self.draw_owner(emitter.owner, origin);
            frame.work.owner_slots_scanned += PARTICLE_CAPACITY as u32;
            frame.work.particles_visited += u32::from(draw.particles_visited);
            frame.draws.push(draw);
        }
        self.finish_scene();
        frame.work.aging_slots_scanned = PARTICLE_CAPACITY as u32;
        frame
    }

    /// First-free allocation. A requested count of zero wraps after the first
    /// allocation and fills every free slot, including its authored table
    /// look-behind outcomes. Occupied slots do not consume count or randomness.
    pub fn generate(
        &mut self,
        owner: ParticleOwner,
        kind: ParticleKind,
        amount: u8,
        life: u8,
    ) -> ParticleWork {
        let mut work = ParticleWork::default();
        let mut remaining = u16::from(amount);
        for (index, particle) in self.particles.iter_mut().enumerate() {
            work.allocation_slots_scanned += 1;
            if particle.life != 0 {
                continue;
            }
            work.particles_allocated += 1;
            particle.position = [0; 3];
            particle.owner = Some(owner);
            particle.life = life;
            match kind {
                ParticleKind::LargeGeneratedExplosion | ParticleKind::SmallGeneratedExplosion => {
                    // Both generator bodies are disabled by `ifeq 1` in the
                    // original. Their common allocation still occurs and
                    // retains the recycled velocity, flags and color.
                }
                ParticleKind::FireUp | ParticleKind::FireDown => {
                    let upward = kind == ParticleKind::FireUp;
                    particle.flags = if upward { PARTICLE_GRAVITY } else { 0 };
                    particle.velocity[0] = (next_random(&mut self.random_state) & FIRE_LATERAL_MASK)
                        as i8
                        - FIRE_LATERAL_BIAS;
                    particle.velocity[1] = (next_random(&mut self.random_state)
                        & FIRE_VERTICAL_MASK) as i8
                        + if upward {
                            FIRE_UP_VELOCITY
                        } else {
                            FIRE_DOWN_VELOCITY
                        };
                    particle.velocity[2] = (next_random(&mut self.random_state) & FIRE_LATERAL_MASK)
                        as i8
                        - FIRE_LATERAL_BIAS;
                    let color = (next_random(&mut self.random_state) & 3) as u8;
                    particle.palette_index = color.max(1) + 1;
                }
                ParticleKind::LargeTableExplosion
                | ParticleKind::SmallTableExplosion
                | ParticleKind::FastTableExplosion => {
                    particle.life = life
                        .wrapping_add((remaining & LIFE_VARIATION_MASK) as u8)
                        .wrapping_sub(LIFE_VARIATION_BIAS);
                    particle.flags = PARTICLE_FADE_OUT;
                    let table_index = i32::from(remaining as i16) - 1;
                    let vector = if table_index >= 0 {
                        PARTICLE_CIRCLE[table_index as usize]
                    } else {
                        PARTICLE_COUNT_UNDERFLOW[(PARTICLE_CAPACITY as i32 + table_index) as usize]
                    };
                    let small = kind == ParticleKind::SmallTableExplosion;
                    particle.velocity[0] = if small {
                        half_vector(vector[0])
                    } else {
                        vector[0]
                    };
                    particle.velocity[1] = if small {
                        half_vector(vector[1])
                    } else {
                        vector[1]
                    };
                    let (mask, bias) = if small {
                        (SMALL_DEPTH_MASK, SMALL_DEPTH_BIAS)
                    } else {
                        (LARGE_DEPTH_MASK, LARGE_DEPTH_BIAS)
                    };
                    particle.velocity[2] =
                        (next_random(&mut self.random_state) & mask) as i8 - bias;
                    particle.position = particle.velocity.map(i16::from);
                    particle.palette_index = EXPLOSION_COLORS[(PARTICLE_CAPACITY - index) & 1];
                }
            }
            remaining = remaining.wrapping_sub(1);
            if remaining == 0 {
                break;
            }
        }
        work
    }

    /// Advance a visible owner's offsets, then project/draw them. Life zero
    /// is not a filter here: a newly expired but still-owned slot draws once
    /// more unless allocation reused it before this owner was visited.
    pub fn draw_owner(&mut self, owner: ParticleOwner, origin: [i16; 3]) -> ParticleDraw {
        let mut draw = ParticleDraw {
            owner,
            primitives: Vec::new(),
            particles_visited: 0,
        };
        for particle in &mut self.particles {
            if particle.owner != Some(owner) {
                continue;
            }
            draw.particles_visited += 1;
            let previous = particle.position;
            for axis in 0..3 {
                particle.position[axis] =
                    particle.position[axis].wrapping_add(i16::from(particle.velocity[axis]));
            }
            particle.flags |= PARTICLE_OWNER_SEEN;
            if particle.flags & PARTICLE_GRAVITY != 0 {
                particle.velocity[1] = particle.velocity[1].wrapping_add(GRAVITY_STEP);
            }
            let position =
                std::array::from_fn(|axis| particle.position[axis].wrapping_add(origin[axis]));
            if position[2] < 0 {
                continue;
            }
            let Some(start) = project(position) else {
                continue;
            };
            let palette_index = particle.palette_index & PALETTE_MASK;
            if particle.flags & PARTICLE_LINE == 0 {
                draw.primitives.push(ParticlePrimitive::Dot {
                    corner: start,
                    palette_index,
                });
            } else {
                let old_position =
                    std::array::from_fn(|axis| previous[axis].wrapping_add(origin[axis]));
                // The original line branch rejects nonnegative OLD depth;
                // preserve that surprising branch, including wraparound.
                if old_position[2] >= 0 {
                    continue;
                }
                if let Some(end) = project(old_position) {
                    draw.primitives.push(ParticlePrimitive::Line {
                        start,
                        end,
                        palette_index,
                    });
                }
            }
        }
        draw
    }

    /// End-of-scene aging, after all owners and the cockpit HUD were drawn.
    pub fn finish_scene(&mut self) {
        for particle in &mut self.particles {
            if particle.life == 0 {
                particle.owner = None;
                continue;
            }
            particle.life -= 1;
            if particle.flags & PARTICLE_FADE_OUT != 0 && particle.life < FADE_LIFE_LIMIT {
                particle.palette_index = PARTICLE_FADE[usize::from(particle.life)];
            }
            if particle.flags & PARTICLE_OWNER_SEEN == 0 {
                particle.life = 0;
                particle.owner = None;
            } else {
                particle.flags &= !PARTICLE_OWNER_SEEN;
            }
        }
    }
}

fn half_vector(value: i8) -> i8 {
    // Source DIV2 differs from Rust's division: only -1 rounds toward zero;
    // other negative odd values retain the arithmetic-shift rounding.
    if value == -1 {
        0
    } else {
        value >> 1
    }
}

fn next_random(state: &mut u16) -> u16 {
    // Every active call site has cleared the incoming rotation carry while
    // storing its preceding particle field. Only the first sum's overflow
    // contributes to the second sum; unlike the dust stream there is no +1.
    let rotated = state.swap_bytes() >> 1;
    let (sum, overflow) = rotated.overflowing_add(*state);
    *state = sum.wrapping_add(*state).wrapping_add(u16::from(overflow));
    *state
}

fn project([x, y, mut depth]: [i16; 3]) -> Option<[u8; 2]> {
    // Wrapped signed comparisons also matter for the line branch's old
    // negative depth. Do not replace these with saturating/clamped math.
    if PROJECTION_MAX.wrapping_sub(depth) < 0 {
        depth = PROJECTION_MAX - 1;
    }
    if PROJECTION_NEAR.wrapping_sub(depth) > 0 {
        return None;
    }
    let reciprocal = (PROJECTION_NUMERATOR / i32::from((depth as u16) & !1)) as i16;
    let projected = [x, y].map(|value| gsu_fmult_q15(value, reciprocal));
    let mut result = [0; 2];
    for axis in 0..2 {
        let value = projected[axis].wrapping_add(PROJECTION_CENTER[axis]) as u16;
        if value >= PROJECTION_LIMIT[axis] {
            return None;
        }
        result[axis] = value as u8;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dl_flags, DrawListEntry};

    fn emitter(owner: u16, kind: Option<ParticleKind>) -> DrawListEntry {
        DrawListEntry {
            obj_id: owner,
            flags: dl_flags::VISIBLE,
            sflags: PARTICLE_OBJECT_FLAG,
            z: 1024 << 16,
            particles: Some(ParticleEmitter {
                owner: ParticleOwner::new(owner).unwrap(),
                kind,
                amount: 1,
                life: 20,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn disabled_scenes_freeze_the_complete_pool_and_scene_init_only_reseeds_it() {
        let mut field = ParticleField::default();
        field.initialize_scene();
        field.generate(ParticleOwner::new(1).unwrap(), ParticleKind::FireUp, 3, 19);
        let before = *field.particles();
        let random = field.random_state();
        let frame = field.capture_scene(
            false,
            &[emitter(2, Some(ParticleKind::FireDown))],
            [0; 3],
            [0; 3],
        );
        assert_eq!(frame, ParticleFrame::default());
        assert_eq!(field.particles(), &before);
        assert_eq!(field.random_state(), random);
        field.initialize_scene();
        assert_eq!(field.particles(), &before);
        assert_eq!(field.random_state(), PARTICLE_SCENE_RANDOM_SEED);
    }

    #[test]
    fn expired_owned_slots_draw_one_last_time_and_then_retire() {
        let owner = ParticleOwner::new(1).unwrap();
        let mut particles = [Particle::default(); PARTICLE_CAPACITY];
        particles[0] = Particle {
            life: 1,
            palette_index: 4,
            velocity: [3, -5, 7],
            owner: Some(owner),
            ..Default::default()
        };
        let mut field = ParticleField::from_state(particles, 17);
        let command = [emitter(1, None)];
        for visit in 1..=2 {
            let frame = field.capture_scene(true, &command, [0; 3], [0; 3]);
            assert_eq!(
                frame.work,
                ParticleWork {
                    owner_slots_scanned: 300,
                    particles_visited: 1,
                    aging_slots_scanned: 300,
                    ..Default::default()
                }
            );
            assert_eq!(frame.draws[0].primitives.len(), 1);
            assert_eq!(
                field.particles()[0].position,
                [3 * visit, -5 * visit, 7 * visit]
            );
            assert_eq!(field.particles()[0].life, 0);
            assert_eq!(
                field.particles()[0].owner,
                if visit == 1 { Some(owner) } else { None }
            );
        }
        let frame = field.capture_scene(true, &command, [0; 3], [0; 3]);
        assert_eq!(frame.work.particles_visited, 0);
        assert!(frame.draws[0].primitives.is_empty());
    }

    #[test]
    fn only_visible_particle_commands_run_and_hidden_owners_expire() {
        let mut field = ParticleField::default();
        field.generate(
            ParticleOwner::new(2).unwrap(),
            ParticleKind::FireDown,
            1,
            20,
        );
        let mut entries = [
            emitter(1, Some(ParticleKind::FireDown)),
            emitter(2, None),
            emitter(3, Some(ParticleKind::FireUp)),
        ];
        entries[1].flags = 0;
        entries[2].sflags = 0;
        let frame = field.capture_scene(true, &entries, [0; 3], [0; 3]);
        assert_eq!(frame.draws.len(), 1);
        assert_eq!(frame.draws[0].owner.object_id(), 1);
        assert_eq!(
            frame.work,
            ParticleWork {
                allocation_slots_scanned: 2,
                particles_allocated: 1,
                owner_slots_scanned: 300,
                particles_visited: 1,
                aging_slots_scanned: 300,
            }
        );
        assert_eq!(field.particles()[0].owner, None);
        assert_eq!(field.particles()[0].life, 0);
        assert_eq!(field.particles()[1].owner, ParticleOwner::new(1));
        assert_eq!(field.particles()[1].life, 19);
    }
}
