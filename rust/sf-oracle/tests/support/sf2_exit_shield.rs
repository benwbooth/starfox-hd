//! The unmodified original installer, formatter, attachment and shield
//! strategy run with independently retained native state and real pools.
use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::exit_shield::{self, ExitShieldError};
use sf2_game::path_protection::{DeflectionProtection, LinkedEffectActivity};
use sf2_game::{Angle, Behavior, ObjectId, ShapeId, Vector3};

fn fixture(source: &mut Source, count: usize, bits: u8) -> (Native, ObjectId) {
    let mut native = Native::new(
        source,
        count,
        bits.rotate_left(3),
        bits.rotate_right(3),
        u16::from(bits).wrapping_mul(257),
    );
    let owner = if count == 1 {
        native.owner
    } else {
        native.objects.active_ids()[count - 2]
    };
    native.world.primary_player = Some(native.owner);
    native
        .world
        .player_mut(&native.objects, native.owner)
        .unwrap()
        .protection = Some(DeflectionProtection::from_control(bits));
    native.world.linked_effect_activity = Some(LinkedEffectActivity {
        recent_spawn: bits ^ 0xAA,
    });
    source.bus.write16(0x12C3, OWNER);
    source.bus.write8(WRAM + SLOT + 0x6C02, bits);
    source.bus.write8(0x1DDF, bits ^ 0xAA);
    let base = u32::from(address(Some(owner)));
    let actor = native.objects.get_mut(owner).unwrap();
    actor.base.shape = ShapeId::from_catalog_index(62);
    source.bus.write16(base + 4, 0xC364);
    actor.base.position = Vector3 {
        x: i16::from(bits).wrapping_mul(131),
        y: -17001,
        z: 32767,
    };
    actor.base.pitch = Angle::from_units(bits.wrapping_add(17));
    actor.base.yaw = Angle::from_units(bits.wrapping_add(63));
    actor.base.roll = Angle::from_units(bits.wrapping_neg());
    for (offset, value) in [
        (12, actor.base.position.x),
        (14, actor.base.position.y),
        (16, actor.base.position.z),
    ] {
        source.bus.write16(base + offset, value as u16);
    }
    for (offset, value) in [
        (18, actor.base.pitch.units()),
        (20, actor.base.yaw.units()),
        (22, actor.base.roll.units()),
    ] {
        source.bus.write8(base + offset, value);
    }
    (native, owner)
}

fn install(source: &mut Source, native: &mut Native, owner: ObjectId) -> Option<ObjectId> {
    let protection = native
        .world
        .player(&native.objects, native.owner)
        .unwrap()
        .protection;
    let full = native.objects.len() == 60 && protection.unwrap().remaining() != 0;
    source.run(
        0x06F953,
        full.then_some(0x008032),
        0,
        address(Some(owner)),
        true,
    );
    let result = exit_shield::install(
        &mut native.objects,
        owner,
        protection,
        native.world.spawn_defaults,
        native.world.linked_effect_activity.as_mut(),
    );
    if full {
        assert_eq!(result, Err(ExitShieldError::ObjectPoolExhausted));
    } else {
        assert!(result.is_ok(), "{result:?}");
    }
    compare(source, native);
    result.ok().flatten()
}

fn compare(source: &Source, native: &Native) {
    native.compare_pool(source);
    assert_eq!(
        source.bus.read8(0x1DDF),
        native.world.linked_effect_activity.unwrap().recent_spawn
    );
    assert_eq!(
        source.bus.read8(WRAM + SLOT + 0x6C02),
        native
            .world
            .player(&native.objects, native.owner)
            .unwrap()
            .protection
            .unwrap()
            .control()
    );
    for (id, actor) in native
        .objects
        .active_objects()
        .filter(|(_, a)| a.base.behavior == Behavior::ExitShield)
    {
        let base = u32::from(address(Some(id)));
        assert_eq!(source.bus.read16(base + 4), 0xC5CC);
        assert_eq!(source.bus.read16(base + 0x19), 0xF9BA);
        assert_eq!(source.bus.read8(base + 0x1B), 6);
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
            (0x1CCF, actor.extension.relative_position.x),
            (0x1CD1, actor.extension.relative_position.y),
            (0x1CD3, actor.extension.relative_position.z),
        ] {
            assert_eq!(
                source.bus.read16(WRAM + base + offset) as i16,
                value,
                "shield word {offset:X}"
            );
        }
        for (offset, value) in [
            (18, actor.base.pitch.units()),
            (20, actor.base.yaw.units()),
            (22, actor.base.roll.units()),
            (0x13, actor.base.child_number),
            (0x2D, actor.base.hit_points),
            (0x2E, actor.base.attack_power),
            (0x1CD5, actor.extension.relative_rotation.pitch.units()),
            (0x1CD6, actor.extension.relative_rotation.yaw.units()),
            (0x1CD7, actor.extension.relative_rotation.roll.units()),
            (0x1CCB, actor.extension.path_state.animation.shape.packed()),
            (0x1CF0, actor.extension.spawn_group),
        ] {
            assert_eq!(
                source.bus.read8(WRAM + base + offset),
                value,
                "shield byte {offset:X}"
            );
        }
        for (offset, mask, value) in [
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
            (0x25, 1, actor.base.flags.remove_with_parent),
            (0x26, 8, actor.base.contacts.run_when_paused),
            (0x26, 16, actor.base.flags.maximum_draw_distance),
        ] {
            assert_eq!(
                source.bus.read8(base + offset) & mask != 0,
                value,
                "shield flag {offset:X}:{mask:X}"
            );
        }
    }
}

#[test]
fn shield_installation_matches_original_all_primary_protection_bytes_and_pool_boundaries() {
    let mut source = Source::new(&rom(), 0xA5);
    for count in [1, 2, 58, 59, 60] {
        for control in 0..=255u8 {
            let (mut native, owner) = fixture(&mut source, count, control);
            let first = install(&mut source, &mut native, owner);
            assert_eq!(first.is_some(), control & 31 != 0 && count < 60);
            // Existing, even removal-pending shields are never reused.
            if let Some(child) = first {
                native
                    .objects
                    .get_mut(child)
                    .unwrap()
                    .base
                    .flags
                    .remove_after_tick = true;
                source.bus.write8(u32::from(address(Some(child))) + 0x25, 9);
                let second = install(&mut source, &mut native, owner);
                assert_ne!(second, first);
            }
        }
    }
}

#[test]
fn shield_strategy_matches_original_all_gate_bytes_parent_branches_and_wrapped_spins() {
    let mut source = Source::new(&rom(), 0);
    for gate in 0..=255u8 {
        for state in 0..4 {
            let (mut native, owner) = fixture(&mut source, 2, gate | 1);
            let child = install(&mut source, &mut native, owner).unwrap();
            let base = u32::from(address(Some(child)));
            let parent = u32::from(address(Some(owner)));
            native.objects.get_mut(owner).unwrap().base.flags.visible = state & 1 == 0;
            source
                .bus
                .write8(parent + 0x23, if state & 1 == 0 { 16 } else { 18 });
            if state & 2 != 0 {
                native.objects.get_mut(owner).unwrap().base.shape = ShapeId::EMPTY;
                source.bus.write16(parent + 4, 0xBC9C);
            }
            let actor = native.objects.get_mut(child).unwrap();
            actor.extension.relative_rotation.pitch = Angle::from_units(gate);
            actor.extension.relative_rotation.yaw = Angle::from_units(gate.rotate_left(3));
            actor.extension.relative_rotation.roll = Angle::from_units(gate.wrapping_neg());
            actor.extension.path_state.animation.shape =
                sf2_game::path_appearance::AnimationControl::from_packed(gate ^ 0xB3);
            for (offset, value) in [
                (0x1CD5, gate),
                (0x1CD6, gate.rotate_left(3)),
                (0x1CD7, gate.wrapping_neg()),
                (0x1CCB, gate ^ 0xB3),
            ] {
                source.bus.write8(WRAM + base + offset, value);
            }
            source.bus.write8(0x1D72, gate);
            source.run(0x06F9BA, None, 0, base as u16, true);
            exit_shield::step(&mut native.objects, child, Some(gate)).unwrap();
            compare(&source, &native);
        }
    }
}
