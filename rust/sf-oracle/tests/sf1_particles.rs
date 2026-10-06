//! Execute unchanged `MPART.MC` allocation, owner draw and scene-aging entries.
use sf_core::particles::{
    Particle, ParticleEmitter, ParticleField, ParticleKind, ParticleOwner, PARTICLE_CAPACITY,
    PARTICLE_OBJECT_FLAG,
};
use sf_core::{dl_flags, DrawListEntry};
use sf_oracle::{load_built_rom, load_symbols, RetailMachine, SnesBus};
use std::collections::HashMap;

const BITMAP: u32 = 0x708000;
const WIDTH: usize = 224;
const HEIGHT: usize = 192;
const RECORD_BYTES: u32 = 14;
const FLUSH_AND_STOP: u16 = 0xAC96;

struct Source {
    bus: SnesBus,
    symbols: HashMap<String, u32>,
}

impl Source {
    fn new() -> Self {
        let mut bus = SnesBus::new(load_built_rom().expect("source-built particle oracle"));
        bus.enable_gsu();
        bus.write8(0x3038, 0x20);
        bus.write8(0x303A, 0x39);
        bus.write8(0x3034, 1);
        Self {
            bus,
            symbols: load_symbols(),
        }
    }

    fn word(&mut self, address: u32, value: u16) {
        let [lo, hi] = value.to_le_bytes();
        self.bus.write8(address, lo);
        self.bus.write8(address + 1, hi);
    }

    fn read_word(&self, address: u32) -> u16 {
        u16::from_le_bytes([self.bus.read8(address), self.bus.read8(address + 1)])
    }

    fn set_particle(&mut self, index: usize, particle: Particle) {
        let address = self.symbols["M_PARTICLES"] + index as u32 * RECORD_BYTES;
        self.bus.write8(address, particle.life);
        self.bus.write8(address + 1, particle.flags);
        self.bus.write8(address + 2, particle.palette_index);
        for axis in 0..3 {
            self.bus
                .write8(address + 3 + axis as u32, particle.velocity[axis] as u8);
            self.word(
                address + 6 + axis as u32 * 2,
                particle.position[axis] as u16,
            );
        }
        self.word(
            address + 12,
            particle.owner.map_or(0, ParticleOwner::object_id),
        );
    }

    fn particle(&self, index: usize) -> Particle {
        let address = self.symbols["M_PARTICLES"] + index as u32 * RECORD_BYTES;
        Particle {
            life: self.bus.read8(address),
            flags: self.bus.read8(address + 1),
            palette_index: self.bus.read8(address + 2),
            velocity: std::array::from_fn(|axis| self.bus.read8(address + 3 + axis as u32) as i8),
            position: std::array::from_fn(|axis| {
                self.read_word(address + 6 + axis as u32 * 2) as i16
            }),
            owner: ParticleOwner::new(self.read_word(address + 12)),
        }
    }

    fn seed(&mut self, particles: &[Particle; PARTICLE_CAPACITY], random: u16) {
        for (index, particle) in particles.iter().copied().enumerate() {
            self.set_particle(index, particle);
        }
        self.word(self.symbols["M_PARTICLERAND"], random);
    }

    fn run(&mut self, name: &str, values: [u16; 4]) {
        for (index, value) in values.into_iter().enumerate() {
            self.word(0x3000 + index as u32 * 2, value);
        }
        self.word(0x3014, self.symbols["M_STACK"] as u16);
        self.word(0x3016, FLUSH_AND_STOP);
        self.word(0x301E, self.symbols[name] as u16);
        self.bus.tick_gsu(2_000_000);
        assert!(
            !self.bus.gsu_ref().unwrap().is_running(),
            "{name} must return"
        );
    }

    fn compare(&self, field: &ParticleField, context: &str) {
        for (index, particle) in field.particles().iter().enumerate() {
            assert_eq!(
                *particle,
                self.particle(index),
                "{context}: particle {index}"
            );
        }
        assert_eq!(
            field.random_state(),
            self.read_word(self.symbols["M_PARTICLERAND"]),
            "{context}: random"
        );
    }

    fn clear_bitmap(&mut self) {
        for offset in 0..(WIDTH * HEIGHT / 2) as u32 {
            // An existing color-one scene proves transparent particles do not erase it.
            self.bus.write8(
                BITMAP + offset,
                if offset % 32 < 16 && offset % 2 == 0 {
                    255
                } else {
                    0
                },
            );
        }
    }

    fn scene(
        &mut self,
        entries: &[DrawListEntry],
        enabled: u16,
        camera: [i16; 3],
        rotation: [u16; 3],
    ) {
        for (name, angle) in ["M_ROTX", "M_ROTY", "M_ROTZ"].into_iter().zip(rotation) {
            self.word(self.symbols[name], angle);
        }
        self.run("MCROTWMATZXY16", [0; 4]);
        let null_shape = self.read_word(self.symbols["PNULLSHAPES"]);
        let offsets: HashMap<_, _> = [
            "DL_SFLAGS",
            "DL_SHAPE",
            "DL_SORTZ",
            "DL_SHADY",
            "DL_SHADX",
            "DL_SHADZ",
            "DL_X",
            "DL_Y",
            "DL_Z",
        ]
        .into_iter()
        .map(|name| (name, self.symbols[name] & 0xFFFF))
        .collect();
        let mut count = 0;
        for entry in entries
            .iter()
            .filter(|entry| entry.flags & dl_flags::VISIBLE != 0)
        {
            let emitter = entry.particles.unwrap();
            let address = self.symbols["M_DRAWLIST"] + count * self.symbols["DL_SIZEOF"];
            for offset in 0..self.symbols["DL_SIZEOF"] {
                self.bus.write8(address + offset, 0);
            }
            let field = |name: &str| address + offsets[name];
            self.bus.write8(field("DL_SFLAGS"), entry.sflags);
            self.word(field("DL_SHAPE"), null_shape);
            self.word(field("DL_SORTZ"), entry.sort_z as u16);
            self.word(field("DL_SHADY"), emitter.owner.object_id());
            self.bus.write8(field("DL_SHADX"), emitter.amount);
            self.bus.write8(field("DL_SHADX") + 1, emitter.life);
            self.bus.write8(
                field("DL_SHADZ"),
                emitter.kind.map_or(0, |kind| match kind {
                    ParticleKind::LargeGeneratedExplosion => 1,
                    ParticleKind::FireUp => 2,
                    ParticleKind::FireDown => 3,
                    ParticleKind::LargeTableExplosion => 4,
                    ParticleKind::SmallGeneratedExplosion => 5,
                    ParticleKind::SmallTableExplosion => 6,
                    ParticleKind::FastTableExplosion => 7,
                }),
            );
            for (axis, (name, coordinate)) in ["DL_X", "DL_Y", "DL_Z"]
                .into_iter()
                .zip([entry.x, entry.y, entry.z])
                .enumerate()
            {
                self.word(
                    field(name),
                    ((coordinate >> 16) as i16).wrapping_sub(camera[axis]) as u16,
                );
            }
            count += 1;
        }
        self.word(self.symbols["M_NUMSHAPES"], count as u16);
        self.run("MALLROTZSORT", [0; 4]);
        self.word(self.symbols["M_PARTICLESON"], enabled);
        self.word(self.symbols["M_PFM"], 0);
        self.word(self.symbols["M_METERS"], 0);
        self.word(self.symbols["M_HUDROT"], 0);
        self.run("MSHOWVIEW", [0; 4]);
    }

    fn bitmap(&self) -> Vec<u8> {
        let mut result = vec![0; WIDTH * HEIGHT];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let tile = (x / 8) * (HEIGHT / 8) + y / 8;
                let row = BITMAP + (tile * 32 + (y % 8) * 2) as u32;
                for bit in 0..4 {
                    result[y * WIDTH + x] |=
                        ((self.bus.read8(row + (bit / 2 * 16 + bit % 2) as u32) >> (7 - x % 8))
                            & 1)
                            << bit;
                }
            }
        }
        result
    }
}

#[test]
fn original_scene_sort_draw_and_age_match_native_across_owner_reuse_and_disable() {
    let mut source = Source::new();
    let mut field = ParticleField::default();
    field.initialize_scene();
    source.seed(field.particles(), field.random_state());
    for scene in 0..96usize {
        let camera = [17, -19, 43];
        let rotation = [[0; 3], [127, 47, 217], [0, 32768, 0]][scene % 3];
        let enabled = [1, 256, 257, 0, 65535, 1, 1][scene % 7];
        let mut entries: Vec<_> = (0..5usize)
            .map(|index| {
                let owner = ParticleOwner::new(index as u16 + 1).unwrap();
                DrawListEntry {
                    obj_id: owner.object_id(),
                    interpolation_id: scene as u64 / 3,
                    flags: if (scene + index) % 11 == 0 {
                        0
                    } else {
                        dl_flags::VISIBLE
                    },
                    sflags: PARTICLE_OBJECT_FLAG,
                    x: (index as i32 * 43 - 86) << 16,
                    y: (index as i32 * 13 - 26) << 16,
                    z: (500 + ((index + scene) % 3) as i32 * 256) << 16,
                    sort_z: if index == 0 { 15000 } else { 0 },
                    particles: Some(ParticleEmitter {
                        owner,
                        kind: ParticleKind::from_source(((scene + index) % 8) as u8),
                        amount: [0, 1, 3, 33, 127, 255][(scene + index) % 6],
                        life: [0, 1, 7, 8, 31, 32, 250, 255][(scene / 3 + index) % 8],
                    }),
                    ..Default::default()
                }
            })
            .collect();
        if scene % 2 == 0 {
            entries.reverse();
        }
        source.clear_bitmap();
        source.scene(&entries, enabled, camera, rotation);
        let frame = field.capture_scene(enabled != 0, &entries, camera, rotation);
        source.compare(&field, &format!("scene={scene}"));
        let mut bitmap = vec![1; WIDTH * HEIGHT];
        for draw in frame.draws {
            for pixel in draw.pixels() {
                bitmap[usize::from(pixel.y) * WIDTH + usize::from(pixel.x)] = pixel.palette_index;
            }
        }
        assert_eq!(bitmap, source.bitmap(), "scene={scene}: composed pixels");
    }
}

#[test]
fn original_scene_initialization_reseeds_particles() {
    let symbols = load_symbols();
    let mut source = RetailMachine::new(load_built_rom().expect("source-built boot ROM"));
    assert!(source
        .tick_until_cpu_execution(0, symbols["INITGAME3D_L"], 240)
        .unwrap());
    let pool = symbols["M_PARTICLES"];
    let before: Vec<_> = (0..PARTICLE_CAPACITY as u32 * RECORD_BYTES)
        .map(|offset| source.peek8(pool + offset))
        .collect();
    assert!(source
        .tick_until_cpu_execution(0, symbols["INITSCREEN_L"], 1)
        .unwrap());
    let mut field = ParticleField::default();
    field.initialize_scene();
    assert_eq!(
        source.peek16(symbols["M_PARTICLERAND"]),
        field.random_state()
    );
    let after: Vec<_> = (0..PARTICLE_CAPACITY as u32 * RECORD_BYTES)
        .map(|offset| source.peek8(pool + offset))
        .collect();
    assert_eq!(
        before, after,
        "scene initialization must not clear particle records"
    );
}

fn inherited_particles(layout: usize) -> [Particle; PARTICLE_CAPACITY] {
    std::array::from_fn(|index| Particle {
        life: if layout == 0 || (layout == 1 && index % 3 == 1) {
            0
        } else {
            (index % 255 + 1) as u8
        },
        flags: index.wrapping_mul(37) as u8,
        palette_index: index.wrapping_mul(29) as u8,
        velocity: [(index as u8).wrapping_mul(31) as i8, -17, 93],
        position: [32761, -32760, -192],
        owner: ParticleOwner::new(2),
    })
}

#[test]
fn all_generator_types_counts_and_pool_exhaustion_match_original_records() {
    let mut source = Source::new();
    let owner = ParticleOwner::new(1).unwrap();
    for kind in 1..=7u8 {
        for amount in 0..=u8::MAX {
            for layout in 0..3 {
                let particles = inherited_particles(layout);
                let random =
                    [0, 1, 0x7FFF, 0x8000, 0xFFFF, 0x19F8][(usize::from(amount) + layout) % 6];
                let life = [0, 1, 6, 7, 8, 31, 32, 250, 255][(usize::from(amount) + layout) % 9];
                source.seed(&particles, random);
                let mut field = ParticleField::from_state(particles, random);
                source.run(
                    "MMAKE_PARTICLES",
                    [kind.into(), life.into(), owner.object_id(), amount.into()],
                );
                field.generate(
                    owner,
                    ParticleKind::from_source(kind).unwrap(),
                    amount,
                    life,
                );
                source.compare(
                    &field,
                    &format!("kind={kind} amount={amount} life={life} layout={layout}"),
                );
            }
        }
    }
}

#[test]
fn every_random_seed_matches_the_original_single_allocation() {
    let mut source = Source::new();
    let owner = ParticleOwner::new(1).unwrap();
    let particles = [Particle::default(); PARTICLE_CAPACITY];
    source.seed(&particles, 0);
    for random in 0..=u16::MAX {
        source.set_particle(0, Particle::default());
        source.word(source.symbols["M_PARTICLERAND"], random);
        source.run("MMAKE_PARTICLES", [2, 25, owner.object_id(), 1]);
        let mut field = ParticleField::from_state(particles, random);
        field.generate(owner, ParticleKind::FireUp, 1, 25);
        assert_eq!(field.particles()[0], source.particle(0), "seed={random}");
        assert_eq!(
            field.random_state(),
            source.read_word(source.symbols["M_PARTICLERAND"]),
            "seed={random}"
        );
    }
}

#[test]
fn aging_matches_every_life_flag_pair_and_retains_recycled_fields() {
    let mut source = Source::new();
    for life in 0..=u8::MAX {
        let particles = std::array::from_fn(|index| Particle {
            life,
            flags: index as u8,
            ..inherited_particles(2)[index]
        });
        source.seed(&particles, 0xBAAD);
        let mut field = ParticleField::from_state(particles, 0xBAAD);
        source.run("MUPDATE_PARTICLES", [0; 4]);
        field.finish_scene();
        source.compare(&field, &format!("life={life}"));
    }
}

#[test]
fn owner_motion_projection_and_pixels_match_original_at_signed_boundaries() {
    let mut source = Source::new();
    let owner = ParticleOwner::new(1).unwrap();
    let edges = [
        -32768i16, -32767, -32760, -12289, -513, -256, -1, 0, 1, 255, 256, 257, 511, 512, 513,
        12287, 12288, 12289, 32760, 32767,
    ];
    for case in 0..256usize {
        let particles = std::array::from_fn(|index| Particle {
            life: (index + case) as u8,
            flags: (index + case) as u8,
            palette_index: index as u8,
            velocity: [
                (index + case) as u8 as i8,
                index as u8 as i8,
                (index + case * 17) as u8 as i8,
            ],
            position: [
                edges[(index + case) % edges.len()],
                edges[(index * 3 + case) % edges.len()],
                edges[(index * 7 + case) % edges.len()],
            ],
            owner: ParticleOwner::new(if index % 5 == 0 { 2 } else { 1 }),
        });
        let origin = [
            edges[case % edges.len()],
            edges[(case / 3) % edges.len()],
            edges[(case / 7) % edges.len()],
        ];
        source.seed(&particles, 0xBAAD);
        source.clear_bitmap();
        for (name, coordinate) in ["M_BIGX", "M_BIGY", "M_BIGZ"].into_iter().zip(origin) {
            source.word(source.symbols[name], coordinate as u16);
        }
        let before = source.bus.gsu_plot_count();
        source.run("MSHOW_PARTICLES", [0, 0, owner.object_id(), 0]);
        let mut field = ParticleField::from_state(particles, 0xBAAD);
        let draw = field.draw_owner(owner, origin);
        source.compare(&field, &format!("case={case} origin={origin:?}"));
        let mut samples = 0;
        let mut bitmap = vec![1; WIDTH * HEIGHT];
        for primitive in draw.primitives {
            primitive.visit_pixels(|pixel| {
                samples += 1;
                if pixel.palette_index != 0 {
                    bitmap[usize::from(pixel.y) * WIDTH + usize::from(pixel.x)] =
                        pixel.palette_index;
                }
            });
        }
        assert_eq!(
            samples,
            source.bus.gsu_plot_count() - before,
            "case={case}: plot count"
        );
        assert_eq!(bitmap, source.bitmap(), "case={case}: bitmap");
    }
}
