//! Unmodified splash allocation, formatting, placement and both lifetime
//! entries. Expected values come from the original routines, not fixtures.

use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_appearance::AnimationControl;
use sf2_game::player_surface::PlayerSurface;
use sf2_game::player_surface_splash::{
    self, SplashError, SplashInputs, SplashPhase, SurfaceSplash,
};
use sf2_game::{Angle, Behavior, ObjectId, RandomState, Vector3};
use sf_oracle::{call_near, Entry};

struct Fixture {
    native: Native,
    origin: ObjectId,
}

impl Fixture {
    fn new(source: &mut Source, count: usize, value: u16) -> Self {
        let mut native = Native::new(source, count, value as u8, (value >> 8) as u8, value);
        let origin = native
            .objects
            .active_ids()
            .iter()
            .copied()
            .find(|&id| id != native.owner)
            .unwrap();
        native
            .world
            .player_mut(&native.objects, native.owner)
            .unwrap()
            .surface = Some(PlayerSurface {
            plane_height: value.rotate_left(3) as i16,
            material: 0xA5,
        });
        native.world.random = RandomState::new([value as u8, (value >> 8) as u8, 127, 219]);
        let actor = native.objects.get_mut(origin).unwrap();
        actor.base.position = Vector3 {
            x: -31777,
            y: (value as i16).wrapping_mul(313),
            z: 32509,
        };
        actor.base.roll = Angle::from_units(value as u8);
        actor.base.yaw = Angle::from_units((value >> 8) as u8);
        actor.base.pitch = Angle::from_units(value as u8 ^ 0x77);
        Self { native, origin }
    }

    fn inputs(&self, value: u16) -> SplashInputs {
        SplashInputs {
            origin: self.origin,
            lateral: value as i8,
            forward: value.rotate_left(3) as i8,
            frame: (value >> 8) as u8,
            size: value.rotate_left(4) as u8,
        }
    }

    fn seed_actors(&self, source: &mut Source) {
        for (id, actor) in self.native.objects.active_objects() {
            let base = u32::from(address(Some(id)));
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
            for (offset, value) in [
                (0x12, actor.base.pitch.units()),
                (0x14, actor.base.yaw.units()),
                (0x16, actor.base.roll.units()),
            ] {
                source.bus.write8(base + offset, value);
            }
        }
        let record = self
            .native
            .world
            .player(&self.native.objects, self.native.owner)
            .unwrap();
        source.bus.write16(
            WRAM + SLOT + 0x6A7D,
            record.surface.unwrap().plane_height as u16,
        );
    }

    fn spawn(
        &mut self,
        source: &mut Source,
        kind: SurfaceSplash,
        input: SplashInputs,
    ) -> Option<ObjectId> {
        self.seed_actors(source);
        for (index, byte) in self.native.world.random.bytes().into_iter().enumerate() {
            source.bus.write8(0xE0 + index as u32, byte);
        }
        self.spawn_retained(source, kind, input)
    }

    fn spawn_retained(
        &mut self,
        source: &mut Source,
        kind: SurfaceSplash,
        input: SplashInputs,
    ) -> Option<ObjectId> {
        source.bus.write16(4, address(Some(input.origin)));
        source.bus.write8(2, input.lateral as u8);
        source.bus.write8(0x97, input.forward as u8);
        source.bus.write8(8, input.frame);
        source.bus.write8(0x0A, input.size);
        let full = self.native.objects.len() == 60;
        source.run(
            match kind {
                SurfaceSplash::Short => 0x07CBD3,
                SurfaceSplash::Long => 0x07CBFD,
            },
            full.then_some(0x008032),
            0,
            OWNER,
            true,
        );
        let actual = player_surface_splash::spawn(
            &mut self.native.objects,
            &mut self.native.world,
            self.native.owner,
            kind,
            input,
        );
        if full {
            assert_eq!(actual, Err(SplashError::ObjectPoolExhausted));
        } else {
            assert!(actual.is_ok(), "{actual:?}");
        }
        self.compare(source);
        actual.ok()
    }

    fn compare(&self, source: &Source) {
        self.native.compare_pool(source);
        for (index, byte) in self.native.world.random.bytes().into_iter().enumerate() {
            assert_eq!(
                source.bus.read8(0xE0 + index as u32),
                byte,
                "random {index}"
            );
        }
        for (id, actor) in self.native.objects.active_objects() {
            let Behavior::SurfaceSplash(phase) = actor.base.behavior else {
                continue;
            };
            let base = u32::from(address(Some(id)));
            assert_eq!(
                source.bus.read16(base + 4),
                0xBC9C + actor.base.shape.catalog_index() as u16 * 28
            );
            assert_eq!(
                source.bus.read16(base + 0x19),
                match phase {
                    SplashPhase::InheritParentMotion => 0xCC6E,
                    SplashPhase::Animate => 0xCC95,
                }
            );
            assert_eq!(source.bus.read8(base + 0x1B), 7);
            assert_eq!(
                source.bus.read16(base + 0x2B),
                address(actor.base.effect_origin.map(|origin| origin.slot()))
            );
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
                    "actor {base:X} field {offset:X}"
                );
            }
            for (offset, value) in [
                (0x12, actor.base.pitch.units()),
                (0x14, actor.base.yaw.units()),
                (0x16, actor.base.roll.units()),
                (0x13, actor.base.child_number),
                (0x2D, actor.base.hit_points),
                (0x2E, actor.base.attack_power),
            ] {
                assert_eq!(
                    source.bus.read8(base + offset),
                    value,
                    "actor {base:X} byte {offset:X}"
                );
            }
            for (offset, value) in [
                (0x1CCA, actor.extension.path_state.animation.shape.packed()),
                (0x1CDA, actor.extension.texture_scroll_x),
                (0x1CF0, actor.extension.spawn_group),
            ] {
                assert_eq!(
                    source.bus.read8(WRAM + base + offset),
                    value,
                    "extension {offset:X}"
                );
            }
            assert_eq!(
                source.bus.read16(WRAM + base + 0x1CE2),
                actor.extension.path_state.motion_phase
            );
            assert_eq!(
                source.bus.read16(WRAM + base + 0x1CC8),
                actor.extension.depth_offset
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
                (0x25, 8, actor.base.flags.remove_after_tick),
                (0x26, 8, actor.base.contacts.run_when_paused),
            ] {
                assert_eq!(
                    source.bus.read8(base + offset) & mask != 0,
                    flag,
                    "flags {offset:X}:{mask:X}"
                );
            }
        }
    }

    fn step(&mut self, source: &mut Source, child: ObjectId) {
        let Behavior::SurfaceSplash(phase) = self.native.objects.get(child).unwrap().base.behavior
        else {
            panic!("not a splash")
        };
        source.run(
            match phase {
                SplashPhase::InheritParentMotion => 0x07CC6E,
                SplashPhase::Animate => 0x07CC95,
            },
            None,
            0,
            address(Some(child)),
            true,
        );
        player_surface_splash::step(&mut self.native.objects, child).unwrap();
        self.compare(source);
    }
}

#[test]
fn splash_installers_match_original_for_rotations_origins_clamp_edges_and_inherited_bytes() {
    let mut source = Source::new(&rom(), 0);
    for kind in [SurfaceSplash::Short, SurfaceSplash::Long] {
        for roll in 0..=u8::MAX {
            for yaw in [0_u8, 1, 32, 64, 127, 128, 192, 255] {
                for plane_delta in [-32768_i16, -128, -127, 0, 126, 127, 128, 32767] {
                    let value = u16::from(roll) | u16::from(yaw) << 8;
                    let mut f = Fixture::new(&mut source, 2, value);
                    let height = f
                        .native
                        .objects
                        .get(f.origin)
                        .unwrap()
                        .base
                        .position
                        .y
                        .wrapping_add(plane_delta);
                    f.native
                        .world
                        .player_mut(&f.native.objects, f.native.owner)
                        .unwrap()
                        .surface
                        .as_mut()
                        .unwrap()
                        .plane_height = height;
                    let mut input = f.inputs(value);
                    if roll & 1 != 0 {
                        input.origin = f.native.owner;
                    }
                    f.spawn(&mut source, kind, input).unwrap();
                }
            }
        }
    }
}

#[test]
fn splash_full_animation_bytes_and_frame_limits_match_original_in_both_phases() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source, 2, 0x3581);
    let input = f.inputs(0);
    let child = f.spawn(&mut source, SurfaceSplash::Long, input).unwrap();
    let base = u32::from(address(Some(child)));
    for phase in [SplashPhase::InheritParentMotion, SplashPhase::Animate] {
        for frames in 0..=u8::MAX {
            for packed in 0..=u8::MAX {
                let actor = f.native.objects.get_mut(child).unwrap();
                actor.base.behavior = Behavior::SurfaceSplash(phase);
                actor.base.flags.remove_after_tick = false;
                actor.extension.path_state.motion_phase = u16::from(frames) << 8 | 0x5A;
                actor.extension.path_state.animation.shape = AnimationControl::from_packed(packed);
                source.bus.write16(
                    base + 0x19,
                    if phase == SplashPhase::InheritParentMotion {
                        0xCC6E
                    } else {
                        0xCC95
                    },
                );
                source
                    .bus
                    .write8(base + 0x25, source.bus.read8(base + 0x25) & !8);
                source
                    .bus
                    .write16(WRAM + base + 0x1CE2, u16::from(frames) << 8 | 0x5A);
                source.bus.write8(WRAM + base + 0x1CCA, packed);
                let parent = f.native.objects.get_mut(f.native.owner).unwrap();
                parent.base.velocity = Vector3 {
                    x: i16::from(packed).wrapping_mul(193),
                    y: -701,
                    z: i16::from(packed).wrapping_mul(-151),
                };
                for (offset, value) in [
                    (0x32, parent.base.velocity.x),
                    (0x34, parent.base.velocity.y),
                    (0x36, parent.base.velocity.z),
                ] {
                    source.bus.write16(u32::from(OWNER) + offset, value as u16);
                }
                f.step(&mut source, child);
            }
        }
    }
}

#[test]
fn splash_retained_pool_links_random_draws_and_original_retirement_match() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source, 2, 0x5A7F);
    f.seed_actors(&mut source);
    for (index, byte) in f.native.world.random.bytes().into_iter().enumerate() {
        source.bus.write8(0xE0 + index as u32, byte);
    }
    for tick in 0..128 {
        let mut input = f.inputs(tick);
        input.frame = 0;
        let kind = if tick % 3 == 0 {
            SurfaceSplash::Short
        } else {
            SurfaceSplash::Long
        };
        f.spawn_retained(&mut source, kind, input).unwrap();
        let children: Vec<_> = f
            .native
            .objects
            .active_objects()
            .filter_map(|(id, actor)| {
                matches!(actor.base.behavior, Behavior::SurfaceSplash(_)).then_some(id)
            })
            .collect();
        for child in children {
            f.step(&mut source, child);
        }
        let retiring: Vec<_> = f
            .native
            .objects
            .active_objects()
            .filter_map(|(id, actor)| actor.base.flags.remove_after_tick.then_some(id))
            .collect();
        for child in retiring {
            let result = call_near(
                &mut source.bus,
                0x7F335A,
                &Entry {
                    x: address(Some(child)),
                    dbr: 0x7E,
                    p: 0x20,
                    ..Default::default()
                },
            );
            assert!(result.returned, "original retirement must return");
            f.native.objects.remove(child).unwrap();
        }
        f.compare(&source);
    }
    for kind in [SurfaceSplash::Short, SurfaceSplash::Long] {
        let mut full = Fixture::new(&mut source, 60, 0x95C3);
        let input = full.inputs(0x3919);
        assert_eq!(full.spawn(&mut source, kind, input), None);
    }
}
