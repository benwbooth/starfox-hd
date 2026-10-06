//! Original particle installers, allocation, attachment, rotation, formatting
//! and lifetime strategies. Source code and expected results are unmodified.

use super::{rom, Source, WRAM};
use sf2_game::path_appearance::AnimationControl;
use sf2_game::player_surface_particle::{self, ParticleError, ParticleInputs, SurfaceParticle};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::{
    Angle, Behavior, Object, ObjectId, ObjectKind, ObjectSpawnDefaults, ObjectStore, RandomState,
    ShapeId, Vector3,
};

const OWNER: u16 = 0x03BD;
const SLOT: u32 = 64;

fn address(id: Option<ObjectId>) -> u16 {
    id.map_or(0, |id| OWNER + id.index() as u16 * 0x3F)
}
fn installer(kind: SurfaceParticle) -> u32 {
    match kind {
        SurfaceParticle::Short => 0x07C6E8,
        SurfaceParticle::Long => 0x07C738,
    }
}
fn strategy(kind: SurfaceParticle) -> u32 {
    match kind {
        SurfaceParticle::Short => 0x07C714,
        SurfaceParticle::Long => 0x07C827,
    }
}

struct Native {
    objects: ObjectStore,
    world: ScenePathWorld,
    owner: ObjectId,
}
impl Native {
    fn new(source: &mut Source, count: usize, roll: u8, yaw: u8, seed: u16) -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        for _ in 1..count {
            objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap();
        }
        let position = Vector3 {
            x: seed as i16,
            y: seed.rotate_left(5) as i16,
            z: seed.wrapping_neg() as i16,
        };
        let movement = Vector3 {
            x: seed.rotate_left(3) as i16,
            y: seed as i16,
            z: seed.rotate_right(3) as i16,
        };
        let actor = objects.get_mut(owner).unwrap();
        actor.base.position = position;
        actor.base.roll = Angle::from_units(roll);
        actor.base.yaw = Angle::from_units(yaw);
        actor.base.pitch = Angle::from_units(seed as u8);
        actor.extension.texture_scroll_x = 99;
        if count > 1 {
            let pressured = objects.active_ids()[count - 2];
            objects
                .get_mut(pressured)
                .unwrap()
                .base
                .flags
                .reclaim_on_pool_pressure = true;
        }
        for index in 0..60 {
            let base = u32::from(OWNER) + index * 0x3F;
            for offset in 0..0x3F {
                source.bus.write8(WRAM + base + offset, 0);
                source.bus.write8(WRAM + base + 0x1CC1 + offset, 0);
            }
            if index as usize >= count {
                source
                    .bus
                    .write16(base, if index == 59 { 0 } else { base as u16 + 0x3F });
            }
        }
        for (id, actor) in objects.active_objects() {
            let base = u32::from(address(Some(id)));
            source.bus.write16(base, address(actor.base.next));
            source.bus.write16(base + 2, address(actor.base.previous));
            source.bus.write16(base + 4, 0xBC9C);
            source.bus.write8(
                base + 0x20,
                if actor.base.flags.reclaim_on_pool_pressure {
                    0x10
                } else {
                    0
                },
            );
        }
        source
            .bus
            .write16(0x12A8, address(objects.active_ids().first().copied()));
        source.bus.write16(
            0x12AA,
            if count == 60 {
                0
            } else {
                OWNER + count as u16 * 0x3F
            },
        );
        source.bus.write16(u32::from(OWNER) + 0x2B, SLOT as u16);
        source.bus.write16(0x1B84, 0);
        source.bus.write8(0x190E, 42);
        source.bus.write16(0x1651, OWNER);
        for (offset, value) in [(12, position.x), (14, position.y), (16, position.z)] {
            source.bus.write16(u32::from(OWNER) + offset, value as u16);
        }
        for (offset, value) in [(0x12, seed as u8), (0x14, yaw), (0x16, roll)] {
            source.bus.write8(u32::from(OWNER) + offset, value);
        }
        source.bus.write8(WRAM + u32::from(OWNER) + 0x1CDA, 99);
        for (offset, value) in [
            (0x6B0B, movement.x),
            (0x6B0D, movement.y),
            (0x6B0F, movement.z),
        ] {
            source.bus.write16(WRAM + SLOT + offset, value as u16);
        }
        let mut world = ScenePathWorld::new(RandomState::default());
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 42,
            run_when_paused: false,
        });
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    flight_displacement: Some(movement),
                    ..Default::default()
                },
            )
            .unwrap();
        Self {
            objects,
            world,
            owner,
        }
    }

    fn spawn(
        &mut self,
        source: &mut Source,
        kind: SurfaceParticle,
        input: ParticleInputs,
    ) -> Option<ObjectId> {
        source.bus.write8(2, input.lateral as u8);
        source.bus.write8(0x97, input.forward as u8);
        source.bus.write8(0x0A, input.size);
        source.bus.write8(8, input.frame);
        let full = self.objects.len() == 60;
        source.run(installer(kind), full.then_some(0x008032), 0, OWNER, true);
        let result =
            player_surface_particle::spawn(&mut self.objects, &self.world, self.owner, kind, input);
        if full {
            assert_eq!(result, Err(ParticleError::ObjectPoolExhausted));
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
        self.compare_pool(source);
        result.ok()
    }

    fn compare_pool(&self, source: &Source) {
        assert_eq!(
            source.bus.read16(0x12A8),
            address(self.objects.active_ids().first().copied())
        );
        let mut free = source.bus.read16(0x12AA);
        let mut free_count = 0;
        while free != 0 {
            free_count += 1;
            assert!(free_count <= 60);
            free = source.bus.read16(u32::from(free));
        }
        assert_eq!(free_count, 60 - self.objects.len());
        for (id, actor) in self.objects.active_objects() {
            let base = u32::from(address(Some(id)));
            assert_eq!(source.bus.read16(base), address(actor.base.next));
            assert_eq!(source.bus.read16(base + 2), address(actor.base.previous));
            assert_eq!(source.bus.read16(base + 6), address(actor.base.attachment));
            assert_eq!(
                source.bus.read16(base + 0x29),
                address(actor.base.first_child.or(actor.base.next_sibling))
            );
            assert_eq!(
                source.bus.read8(base + 0x25) & 8 != 0,
                actor.base.flags.remove_after_tick
            );
            assert_eq!(
                source.bus.read8(WRAM + base + 0x1CDA),
                actor.extension.texture_scroll_x
            );
            assert_eq!(
                source.bus.read8(base + 0x23) & 0x10 != 0,
                actor.extension.path_state.motion.refresh_child_chain
            );
            let Behavior::SurfaceParticle(kind) = actor.base.behavior else {
                continue;
            };
            assert_eq!(
                source.bus.read16(base + 4),
                0xBC9C + actor.base.shape.catalog_index() as u16 * 28
            );
            assert_eq!(source.bus.read16(base + 0x19), strategy(kind) as u16);
            assert_eq!(source.bus.read8(base + 0x1B), 7);
            assert_eq!(source.bus.read8(base + 0x13), actor.base.child_number);
            assert_eq!(
                source.bus.read16(WRAM + base + 0x1CD8),
                address(actor.extension.parent)
            );
            for (offset, value) in [
                (12, actor.base.position.x),
                (14, actor.base.position.y),
                (16, actor.base.position.z),
                (0x32, actor.base.velocity.x),
                (0x34, actor.base.velocity.y),
                (0x36, actor.base.velocity.z),
            ] {
                assert_eq!(
                    source.bus.read16(base + offset) as i16,
                    value,
                    "field {offset:X}"
                );
            }
            for (offset, value) in [
                (0x12, actor.base.pitch.units()),
                (0x14, actor.base.yaw.units()),
                (0x16, actor.base.roll.units()),
                (0x2D, actor.base.hit_points),
                (0x2E, actor.base.attack_power),
            ] {
                assert_eq!(source.bus.read8(base + offset), value);
            }
            assert_eq!(
                source.bus.read8(WRAM + base + 0x1CCA),
                actor.extension.path_state.animation.shape.packed()
            );
            assert_eq!(
                source.bus.read16(WRAM + base + 0x1CC8),
                actor.extension.depth_offset
            );
            assert_eq!(
                source.bus.read8(WRAM + base + 0x1CF0),
                actor.extension.spawn_group
            );
            for (offset, mask, flag) in [
                (0x20, 0x20, actor.base.flags.scaled_sprite),
                (
                    0x20,
                    8,
                    actor.extension.path_state.needs_path_initialization,
                ),
                (0x21, 1, actor.base.flags.collision_disabled),
                (0x22, 4, actor.base.flags.general_search_eligible),
                (
                    0x23,
                    4,
                    actor.extension.path_state.motion.attached_coordinates,
                ),
                (
                    0x24,
                    4,
                    actor.base.flags.exclude_from_shape_footprint_search,
                ),
                (0x25, 1, actor.base.flags.remove_with_parent),
                (0x26, 8, actor.base.contacts.run_when_paused),
            ] {
                assert_eq!(
                    source.bus.read8(base + offset) & mask != 0,
                    flag,
                    "flag {offset:X}:{mask:X}"
                );
            }
        }
    }
}

fn step(source: &mut Source, actor: &mut Object, kind: SurfaceParticle, base: u16) {
    let mut expected = actor.clone();
    source.run(strategy(kind), None, 0, base, true);
    let base = u32::from(base);
    expected.extension.path_state.animation.shape =
        AnimationControl::from_packed(source.bus.read8(WRAM + base + 0x1CCA));
    expected.base.flags.remove_after_tick = source.bus.read8(base + 0x25) & 8 != 0;
    expected.base.position = Vector3 {
        x: source.bus.read16(base + 12) as i16,
        y: source.bus.read16(base + 14) as i16,
        z: source.bus.read16(base + 16) as i16,
    };
    expected.base.velocity.x = source.bus.read16(base + 0x32) as i16;
    expected.base.velocity.z = source.bus.read16(base + 0x36) as i16;
    player_surface_particle::step(actor).unwrap();
    assert_eq!(*actor, expected);
    assert_eq!(source.bus.read16(base + 0x34) as i16, actor.base.velocity.y);
}

#[test]
fn particle_installers_match_original_rotation_parameters_and_retained_movement() {
    let mut source = Source::new(&rom(), 0);
    for kind in [SurfaceParticle::Short, SurfaceParticle::Long] {
        for roll in 0..=u8::MAX {
            for yaw in [0, 1, 32, 64, 65, 127, 128, 192, 255] {
                for lateral in [-128, -20, 20, 127] {
                    let seed = u16::from(roll) | u16::from(yaw) << 8;
                    let mut native = Native::new(&mut source, 1, roll, yaw, seed);
                    native.spawn(
                        &mut source,
                        kind,
                        ParticleInputs {
                            lateral,
                            forward: (roll as i8).wrapping_add(lateral),
                            size: roll,
                            frame: yaw,
                        },
                    );
                }
            }
        }
        for value in 0..=u8::MAX {
            let mut native = Native::new(
                &mut source,
                1,
                value,
                value.rotate_left(3),
                u16::from(value) * 257,
            );
            native.spawn(
                &mut source,
                kind,
                ParticleInputs {
                    lateral: value as i8,
                    forward: value.wrapping_neg() as i8,
                    size: value,
                    frame: value,
                },
            );
        }
    }
}

#[test]
fn particle_full_pool_fatal_partial_pool_pressure_and_duplicate_siblings_match_original() {
    let mut source = Source::new(&rom(), 0);
    for kind in [SurfaceParticle::Short, SurfaceParticle::Long] {
        for count in [1, 2, 57, 58, 59, 60] {
            let mut native = Native::new(&mut source, count, 99, 157, 0xAFFF);
            for _ in 0..3 {
                native.spawn(
                    &mut source,
                    kind,
                    ParticleInputs {
                        lateral: -20,
                        forward: 0,
                        size: 2,
                        frame: 0,
                    },
                );
            }
        }
    }
}

#[test]
fn particle_every_animation_byte_and_velocity_word_matches_original_step() {
    let mut source = Source::new(&rom(), 0);
    for kind in [SurfaceParticle::Short, SurfaceParticle::Long] {
        let mut actor = Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::SurfaceParticle(kind),
        );
        for word in 0..=u16::MAX {
            // First sweep covers every animation byte; second fixes a live
            // frame so every signed velocity is actually integrated/decayed.
            for frame in [word as u8, 0x80] {
                actor.extension.path_state.animation.shape = AnimationControl::from_packed(frame);
                actor.base.position = Vector3 {
                    x: i16::MAX,
                    y: i16::MIN,
                    z: -1,
                };
                actor.base.velocity = Vector3 {
                    x: word as i16,
                    y: word.rotate_left(3) as i16,
                    z: word.wrapping_neg() as i16,
                };
                actor.base.flags.remove_after_tick = word & 0x8000 != 0;
                let base = u32::from(OWNER);
                source.bus.write8(WRAM + base + 0x1CCA, frame);
                source.bus.write8(
                    base + 0x25,
                    if actor.base.flags.remove_after_tick {
                        0xA8
                    } else {
                        0xA0
                    },
                );
                for (offset, value) in [
                    (12, actor.base.position.x),
                    (14, actor.base.position.y),
                    (16, actor.base.position.z),
                    (0x32, actor.base.velocity.x),
                    (0x34, actor.base.velocity.y),
                    (0x36, actor.base.velocity.z),
                ] {
                    source.bus.write16(base + offset, value as u16);
                }
                step(&mut source, &mut actor, kind, OWNER);
                assert_eq!(source.bus.read8(base + 0x25) & !8, 0xA0);
            }
        }
    }
}

#[test]
fn particle_installed_lifetimes_match_original_without_feeding_native_state_back() {
    let mut source = Source::new(&rom(), 0);
    for kind in [SurfaceParticle::Short, SurfaceParticle::Long] {
        for frame in 0..=u8::MAX {
            let mut native = Native::new(&mut source, 1, frame, frame.rotate_left(3), 0xE765);
            let child = native
                .spawn(
                    &mut source,
                    kind,
                    ParticleInputs {
                        lateral: 20,
                        forward: -37,
                        size: 2,
                        frame,
                    },
                )
                .unwrap();
            for _ in 0..10 {
                let actor = native.objects.get_mut(child).unwrap();
                step(&mut source, actor, kind, address(Some(child)));
                if actor.base.flags.remove_after_tick {
                    break;
                }
            }
            assert!(
                native
                    .objects
                    .get(child)
                    .unwrap()
                    .base
                    .flags
                    .remove_after_tick
            );
        }
    }
}
