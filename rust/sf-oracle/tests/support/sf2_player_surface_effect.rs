//! Original protected surface-effect allocation, inheritance, random order,
//! both scheduled strategies and real retirement, without output replay.

use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_relationships;
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::player_surface_effect::{
    self, SurfaceEffectError, SurfaceEffectInputs, SurfaceEffectPhase,
};
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::{Angle, Behavior, ObjectId, RandomState, Vector3};
use sf_oracle::{call_near, Entry};

struct Fixture {
    native: Native,
    origin: ObjectId,
}
impl Fixture {
    fn new(source: &mut Source, count: usize) -> Self {
        let mut native = Native::new(source, count, 0, 0, 0);
        let origin = native
            .objects
            .active_ids()
            .iter()
            .copied()
            .find(|&id| id != native.owner)
            .unwrap();
        player_storage::initialize(
            &mut native.objects,
            &mut native.world,
            &mut PathRuntime::default(),
            native.owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 19,
                score: Default::default(),
            },
        )
        .unwrap();
        native
            .world
            .player_mut(&native.objects, native.owner)
            .unwrap()
            .auxiliary
            .as_mut()
            .unwrap()
            .mode = 0x11;
        Self { native, origin }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.native
            .world
            .player_mut(&self.native.objects, self.native.owner)
            .unwrap()
    }
    fn input(&self, value: u16) -> SurfaceEffectInputs {
        SurfaceEffectInputs {
            origin: Some(self.origin),
            lateral: Some(value as i8),
            vertical: Some((value >> 8) as i8),
            forward: Some(value.rotate_left(3) as i8),
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
                (0x18, actor.base.speed),
                (0x0A, actor.base.target_speed),
            ] {
                source.bus.write8(base + offset, value);
            }
            source.bus.write16(base + 6, address(actor.base.attachment));
            source.bus.write16(
                base + 0x29,
                address(actor.base.attachment_next),
            );
            source.bus.write8(
                base + 0x23,
                u8::from(actor.extension.path_state.motion.refresh_child_chain) * 0x10
                    | u8::from(actor.extension.path_state.motion.attached_coordinates) * 4,
            );
            source.bus.write8(
                base + 0x25,
                u8::from(actor.base.flags.remove_with_parent)
                    | u8::from(actor.base.flags.remove_after_tick) * 8,
            );
            source.bus.write16(
                WRAM + base + 0x1CE2,
                actor.extension.path_state.motion_phase,
            );
            source.bus.write16(
                WRAM + base + 0x1CE4,
                actor.extension.path_state.script_value,
            );
        }
    }
    fn seed_controls(&mut self, source: &mut Source) {
        let record = *self.records();
        source
            .bus
            .write8(WRAM + SLOT + 0x6AA0, record.auxiliary.unwrap().mode);
        source.bus.write8(
            WRAM + SLOT + 0x6B94,
            record.motion.unwrap().walker_contact_control,
        );
        source.bus.write8(
            WRAM + SLOT + 0x6AA4,
            record.motion.unwrap().walker_motion_control,
        );
        source.bus.write8(
            WRAM + SLOT + 0x6B64,
            record.mode_selection.unwrap().surface_control,
        );
        source.bus.write16(
            WRAM + SLOT + 0x6A7D,
            record.surface.unwrap().plane_height as u16,
        );
        source
            .bus
            .write8(0xC4, self.native.world.strategy_clock as u8);
    }
    fn spawn(
        &mut self,
        source: &mut Source,
        input: SurfaceEffectInputs,
        fatal: bool,
    ) -> Option<ObjectId> {
        self.seed_actors(source);
        for (index, value) in self.native.world.random.bytes().into_iter().enumerate() {
            source.bus.write8(0xE0 + index as u32, value);
        }
        self.spawn_retained(source, input, fatal)
    }
    fn spawn_retained(
        &mut self,
        source: &mut Source,
        input: SurfaceEffectInputs,
        fatal: bool,
    ) -> Option<ObjectId> {
        self.seed_controls(source);
        source.bus.write16(4, address(input.origin));
        source.bus.write8(2, input.lateral.unwrap() as u8);
        source.bus.write8(8, input.vertical.unwrap() as u8);
        source.bus.write8(0x97, input.forward.unwrap() as u8);
        source.bus.write16(0x0A, 0x5A93);
        source.run(0x07C475, fatal.then_some(0x008032), 0, OWNER, true);
        let actual = player_surface_effect::spawn(
            &mut self.native.objects,
            &mut self.native.world,
            self.native.owner,
            input,
        );
        if fatal {
            assert_eq!(actual, Err(SurfaceEffectError::ObjectPoolExhausted));
            self.compare(source);
            None
        } else {
            let actual = actual.unwrap();
            assert_eq!(
                actual.map_or(0x5A93, |result| result.steering_response_target),
                source.bus.read16(0x0A)
            );
            self.compare(source);
            actual.map(|result| result.object)
        }
    }
    fn compare(&self, source: &Source) {
        self.native.compare_pool(source);
        for (index, value) in self.native.world.random.bytes().into_iter().enumerate() {
            assert_eq!(source.bus.read8(0xE0 + index as u32), value);
        }
        for (id, actor) in self.native.objects.active_objects() {
            let Behavior::SurfaceEffect(phase) = actor.base.behavior else {
                continue;
            };
            let base = u32::from(address(Some(id)));
            assert_eq!(source.bus.read16(base + 4), 0xC0A8);
            assert_eq!(
                source.bus.read16(base + 0x19),
                match phase {
                    SurfaceEffectPhase::Initialize => 0xC61E,
                    SurfaceEffectPhase::Active => 0xC631,
                }
            );
            assert_eq!(source.bus.read8(base + 0x1B), 7);
            assert_eq!(source.bus.read8(base + 0x13), actor.base.child_number);
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
                    "effect {id:?} field {offset:X}"
                );
            }
            for (offset, value) in [
                (0x12, actor.base.pitch.units()),
                (0x14, actor.base.yaw.units()),
                (0x16, actor.base.roll.units()),
                (0x18, actor.base.speed),
                (0x0A, actor.base.target_speed),
                (0x2D, actor.base.hit_points),
                (0x2E, actor.base.attack_power),
            ] {
                assert_eq!(
                    source.bus.read8(base + offset),
                    value,
                    "effect {id:?} byte {offset:X}"
                );
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
            assert_eq!(
                source.bus.read16(WRAM + base + 0x1CE2),
                actor.extension.path_state.motion_phase
            );
            assert_eq!(
                source.bus.read16(WRAM + base + 0x1CE4),
                actor.extension.path_state.script_value
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
                    "effect flag {offset:X}:{mask:X}"
                );
            }
        }
    }
    fn step(&mut self, source: &mut Source, effect: ObjectId) {
        self.seed_controls(source);
        let Behavior::SurfaceEffect(phase) = self.native.objects.get(effect).unwrap().base.behavior
        else {
            panic!("expected effect")
        };
        let before = *self.records();
        source.run(
            match phase {
                SurfaceEffectPhase::Initialize => 0x07C61E,
                SurfaceEffectPhase::Active => 0x07C631,
            },
            None,
            0,
            address(Some(effect)),
            true,
        );
        player_surface_effect::step(&mut self.native.objects, &self.native.world, effect).unwrap();
        assert_eq!(*self.records(), before);
        self.compare(source);
    }
    fn retire(&mut self, source: &mut Source, effect: ObjectId) {
        // Retirement ends with RTS, unlike the two far-call effect strategies.
        let result = call_near(
            &mut source.bus,
            0x7F335A,
            &Entry {
                x: address(Some(effect)),
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        assert!(result.returned, "original effect retirement must return");
        self.native.objects.remove(effect).unwrap();
        self.compare(source);
    }
}

#[test]
fn surface_effect_all_modes_surface_states_child_limits_and_full_pool_match_original() {
    let mut source = Source::new(&rom(), 0);
    let mut spawned = 0;
    let mut gated = 0;
    for mode in 0..=u8::MAX {
        for contact in 0..8 {
            let mut f = Fixture::new(&mut source, 2);
            f.records().auxiliary.as_mut().unwrap().mode = mode;
            f.records().motion.as_mut().unwrap().walker_contact_control = (mode << 3) | contact;
            let input = f.input(u16::from(mode) * 197);
            if f.spawn(&mut source, input, false).is_some() {
                spawned += 1;
            } else {
                gated += 1;
            }
        }
    }
    assert!(spawned > 500 && gated > 500);
    for count in [7, 59, 60] {
        for children in 0..=5 {
            let mut f = Fixture::new(&mut source, count);
            let ids: Vec<_> = f
                .native
                .objects
                .active_ids()
                .iter()
                .copied()
                .filter(|&id| id != f.native.owner && id != f.origin)
                .collect();
            for &child in ids.iter().take(children) {
                path_relationships::attach_fresh_child(
                    &mut f.native.objects,
                    f.native.owner,
                    child,
                    77,
                )
                .unwrap();
            }
            let input = f.input(0x7139);
            let result = f.spawn(&mut source, input, count == 60 && children < 5);
            assert_eq!(result.is_some(), count < 60 && children < 5);
        }
    }
}

#[test]
fn surface_effect_all_pitch_yaw_pairs_real_origins_and_random_order_match_original() {
    let mut source = Source::new(&rom(), 0);
    for value in 0..=u16::MAX {
        let mut f = Fixture::new(&mut source, 2);
        f.native.world.random = RandomState::new([
            value as u8,
            (value >> 8) as u8,
            value.rotate_left(3) as u8,
            !value as u8,
        ]);
        f.native.world.strategy_clock = value;
        let owner = f.native.objects.get_mut(f.native.owner).unwrap();
        owner.base.pitch = Angle::from_units(value.rotate_left(1) as u8);
        owner.base.yaw = Angle::from_units(value.rotate_left(5) as u8);
        owner.base.roll = Angle::from_units(value.rotate_right(1) as u8);
        let origin = f.native.objects.get_mut(f.origin).unwrap();
        origin.base.pitch = Angle::from_units((value >> 8) as u8);
        origin.base.yaw = Angle::from_units(value as u8);
        origin.base.position = Vector3 {
            x: value.wrapping_mul(193) as i16,
            y: value.rotate_left(7) as i16,
            z: !value as i16,
        };
        f.records().surface.as_mut().unwrap().plane_height = value.rotate_left(3) as i16;
        let input = f.input(value);
        f.spawn(&mut source, input, false).unwrap();
    }
}

#[test]
fn surface_effect_all_height_words_lifetime_bytes_and_carry_chase_branches_match_original() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source, 2);
    let input = f.input(0);
    let effect = f.spawn(&mut source, input, false).unwrap();
    f.step(&mut source, effect);
    for value in 0..=u16::MAX {
        for carried in [false, true] {
            f.native.world.strategy_clock = value;
            let record = f.records();
            record.mode_selection.as_mut().unwrap().surface_control = u8::from(carried) * 2;
            record.motion.as_mut().unwrap().walker_motion_control =
                if value & 1 != 0 { value as u8 } else { 0 };
            record.surface.as_mut().unwrap().plane_height = value.rotate_right(3) as i16;
            f.native.objects.get_mut(f.origin).unwrap().base.position = Vector3 {
                x: value as i16,
                y: 0,
                z: !value as i16,
            };
            let actor = f.native.objects.get_mut(effect).unwrap();
            actor.base.position = Vector3 {
                x: !value as i16,
                y: value as i16,
                z: value.rotate_left(9) as i16,
            };
            actor.base.target_speed = value as u8;
            actor.base.speed = (value >> 8) as u8;
            actor.base.pitch = Angle::from_units((value >> 8) as u8);
            actor.base.yaw = Angle::from_units(value as u8);
            actor.base.flags.remove_after_tick = false;
            actor.extension.path_state.script_value = value.rotate_left(5);
            actor.extension.path_state.motion_phase = value.rotate_right(7);
            f.seed_actors(&mut source);
            f.step(&mut source, effect);
        }
    }
}

#[test]
fn surface_effect_retained_lifetimes_and_original_retirement_match_without_replaying_outputs() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new(&mut source, 2);
    f.seed_actors(&mut source);
    for (index, value) in f.native.world.random.bytes().into_iter().enumerate() {
        source.bus.write8(0xE0 + index as u32, value);
    }
    let mut active: Vec<ObjectId> = Vec::new();
    let mut retired = 0;
    let mut initialized = 0;
    for tick in 0_u16..8192 {
        f.native.world.strategy_clock = tick;
        // These are caller-owned inputs, not effect outputs. Child positions,
        // velocities, counters, flags and random state remain independently live.
        let record = f.records();
        record.mode_selection.as_mut().unwrap().surface_control = u8::from(tick % 5 == 0) * 2;
        record.motion.as_mut().unwrap().walker_motion_control = u8::from(tick % 7 == 0);
        record.surface.as_mut().unwrap().plane_height = (tick % 91) as i16 - 45;
        let input = f.input(tick);
        if tick % 3 == 0 {
            if let Some(effect) = f.spawn_retained(&mut source, input, false) {
                active.push(effect);
                initialized += 1;
            }
        }
        for effect in active.clone() {
            f.step(&mut source, effect);
            if f.native
                .objects
                .get(effect)
                .unwrap()
                .base
                .flags
                .remove_after_tick
            {
                f.retire(&mut source, effect);
                active.retain(|&id| id != effect);
                retired += 1;
            }
        }
    }
    assert!(initialized > 2000 && retired > 2000);
}
