//! Unmodified pilot appearance, damage-particle allocation/formatting, surface
//! depth arbitration and final depth publication. No original routine patches.

use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::player_appearance::{self, AppearanceError, PlayerAppearance};
use sf2_game::{authored_paths, Behavior, ObjectId, ShapeId};

fn fixture(source: &mut Source, count: usize, value: u16) -> Native {
    let mut native = Native::new(source, count, value as u8, (value >> 8) as u8, value);
    let actor = native.objects.get_mut(native.owner).unwrap();
    actor.base.shape = ShapeId::from_catalog_index(2);
    actor.base.hit_points = 1;
    actor.extension.material_set = None;
    actor.extension.depth_offset = 0xACED;
    let record = native
        .world
        .player_mut(&native.objects, native.owner)
        .unwrap();
    record.appearance = Some(PlayerAppearance {
        depth_control: value as u8,
    });
    record.visit = Some(Default::default());
    record.contact = Some(Default::default());
    record.charge = Some(Default::default());
    native.world.player_carry_mode = Some(1);
    native.world.strategy_clock = value;
    native.world.random = sf2_game::RandomState::new([
        value as u8,
        (value >> 8) as u8,
        (value as u8).wrapping_add(127),
        (value >> 8) as u8 ^ 0xD3,
    ]);
    seed(source, &native, 0, 0);
    native
}

fn seed(source: &mut Source, native: &Native, contact_other: u8, charge_other: u8) {
    let actor = native.objects.get(native.owner).unwrap();
    source.bus.write16(
        u32::from(OWNER) + 4,
        0xBC9C + actor.base.shape.catalog_index() as u16 * 28,
    );
    source.bus.write8(
        u32::from(OWNER) + 0x21,
        if actor.extension.path_state.motion.carry_selected_player {
            0x20
        } else {
            0
        },
    );
    source.bus.write8(
        u32::from(OWNER) + 0x23,
        if actor.base.flags.visible { 0 } else { 2 },
    );
    source
        .bus
        .write8(u32::from(OWNER) + 0x2D, actor.base.hit_points);
    source.bus.write16(
        WRAM + u32::from(OWNER) + 0x1CCD,
        actor
            .extension
            .material_set
            .map_or(0, |m| m.catalog_token()),
    );
    source.bus.write16(
        WRAM + u32::from(OWNER) + 0x1CC8,
        actor.extension.depth_offset,
    );
    let record = native.world.player(&native.objects, native.owner).unwrap();
    source.bus.write8(
        WRAM + SLOT + 0x6AA2,
        record.appearance.unwrap().depth_control,
    );
    source
        .bus
        .write8(WRAM + SLOT + 0x6BFF, record.visit.unwrap().pilot_code);
    let contact = record.contact.unwrap();
    source
        .bus
        .write8(WRAM + SLOT + 0x6C00, contact.hit.reserve_shield);
    source.bus.write8(
        WRAM + SLOT + 0x6A72,
        (contact_other & !0x10) | if contact.ignores_contacts { 0x10 } else { 0 },
    );
    let charge = record.charge.unwrap();
    source.bus.write8(
        WRAM + SLOT + 0x6B63,
        (charge_other & 0x3F)
            | if charge.linked_mode { 0x80 } else { 0 }
            | if charge.linked_muzzle_disabled {
                0x40
            } else {
                0
            },
    );
    source.bus.write16(0xC4, native.world.strategy_clock);
    source
        .bus
        .write8(0x1E13, native.world.player_carry_mode.unwrap());
    for (index, byte) in native.world.random.bytes().into_iter().enumerate() {
        source.bus.write8(0xE0 + index as u32, byte);
    }
}

fn compare(source: &Source, native: &Native) {
    native.compare_pool(source);
    let actor = native.objects.get(native.owner).unwrap();
    assert_eq!(
        source.bus.read16(WRAM + u32::from(OWNER) + 0x1CCD),
        actor
            .extension
            .material_set
            .map_or(0, |m| m.catalog_token())
    );
    assert_eq!(
        source.bus.read16(WRAM + u32::from(OWNER) + 0x1CC8),
        actor.extension.depth_offset
    );
    assert_eq!(
        source.bus.read8(WRAM + SLOT + 0x6AA2),
        native
            .world
            .player(&native.objects, native.owner)
            .unwrap()
            .appearance
            .unwrap()
            .depth_control
    );
    for (index, byte) in native.world.random.bytes().into_iter().enumerate() {
        assert_eq!(source.bus.read8(0xE0 + index as u32), byte);
    }
    for (id, actor) in native.objects.active_objects() {
        if actor.base.behavior != Behavior::FollowPath {
            continue;
        }
        let base = u32::from(address(Some(id)));
        assert_eq!(
            source.bus.read16(base + 4),
            0xBC9C + actor.base.shape.catalog_index() as u16 * 28
        );
        assert_eq!(source.bus.read16(base + 0x19), 0x7E1E);
        assert_eq!(source.bus.read8(base + 0x1B), 0x7F);
        assert!(actor.extension.path_state.needs_path_initialization);
        assert_eq!(source.bus.read16(base + 0x2B), 0xF521);
        assert_eq!(actor.base.path, Some(authored_paths::LOCAL_JITTER_SPRITE));
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
            assert_eq!(source.bus.read16(base + offset) as i16, value);
        }
        for (offset, value) in [
            (0x12, actor.base.pitch.units()),
            (0x14, actor.base.yaw.units()),
            (0x16, actor.base.roll.units()),
            (0x13, actor.base.child_number),
            (0x2D, actor.base.hit_points),
            (0x2E, actor.base.attack_power),
        ] {
            assert_eq!(source.bus.read8(base + offset), value);
        }
        assert_eq!(
            source.bus.read8(WRAM + base + 0x1CF0),
            actor.extension.spawn_group
        );
        for (offset, mask, flag) in [
            (0x20, 8, actor.base.flags.casts_shadow),
            (0x21, 1, actor.base.flags.collision_disabled),
            (0x22, 4, actor.base.flags.general_search_eligible),
            (0x23, 2, !actor.base.flags.visible),
            (0x26, 8, actor.base.contacts.run_when_paused),
        ] {
            assert_eq!(
                source.bus.read8(base + offset) & mask != 0,
                flag,
                "flags {offset:X}:{mask:X}"
            );
        }
        assert_eq!(source.bus.read8(base + 0x26) & 0x10, 0);
    }
}

fn update(source: &mut Source, native: &mut Native, child_number: u8) -> Option<ObjectId> {
    source.bus.write8(0, child_number);
    source.run(0x06AA1A, None, 0, OWNER, true);
    let child = player_appearance::update(
        &mut native.objects,
        &mut native.world,
        native.owner,
        child_number,
    )
    .unwrap();
    compare(source, native);
    child
}

fn publish(source: &mut Source, native: &mut Native) {
    source.run_with_y(0x069F21, Some(0x069F36), 0, OWNER, true, Some(SLOT as u16));
    player_appearance::publish_depth(&mut native.objects, &native.world, native.owner).unwrap();
    compare(source, native);
}

fn surface(source: &mut Source, native: &mut Native) {
    source.run_with_y(0x07C32F, Some(0x07C355), 0, OWNER, true, Some(SLOT as u16));
    player_appearance::update_surface_depth(&native.objects, &mut native.world, native.owner)
        .unwrap();
    compare(source, native);
}

#[test]
fn original_pilot_material_and_reserve_shield_appearance_cover_every_byte_pair() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = fixture(&mut source, 1, 0);
    native.objects.get_mut(native.owner).unwrap().base.shape = ShapeId::EMPTY;
    for pilot in 0..=255_u8 {
        for shield in 0..=255_u8 {
            let record = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap();
            record.visit.as_mut().unwrap().pilot_code = pilot;
            record.contact.as_mut().unwrap().hit.reserve_shield = shield;
            record.appearance.as_mut().unwrap().depth_control =
                pilot.rotate_left(3).wrapping_add(shield);
            native.world.strategy_clock =
                (u16::from(pilot) << 8) | u16::from(shield.wrapping_add(pilot));
            seed(&mut source, &native, 0xED, 0x3F);
            assert_eq!(update(&mut source, &mut native, pilot ^ shield), None);
            publish(&mut source, &mut native);
        }
    }
}

#[test]
fn original_surface_depth_arbitrates_all_controls_modes_and_carry_flags() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = fixture(&mut source, 1, 0);
    for carried in [false, true] {
        native
            .objects
            .get_mut(native.owner)
            .unwrap()
            .extension
            .path_state
            .motion
            .carry_selected_player = carried;
        for mode in 0..=255_u8 {
            native.world.player_carry_mode = Some(mode);
            for control in 0..=255_u8 {
                native
                    .world
                    .player_mut(&native.objects, native.owner)
                    .unwrap()
                    .appearance
                    .as_mut()
                    .unwrap()
                    .depth_control = control;
                native
                    .objects
                    .get_mut(native.owner)
                    .unwrap()
                    .extension
                    .depth_offset = 0xFFFF;
                seed(&mut source, &native, 0, 0);
                surface(&mut source, &mut native);
                publish(&mut source, &mut native);
            }
        }
    }
}

#[test]
fn original_damage_particle_gates_cover_every_control_byte_and_preserve_allocation_order() {
    let mut source = Source::new(&rom(), 0xA5);
    for field in 0..8 {
        for value in 0..=255_u8 {
            let mut native = fixture(
                &mut source,
                if value & 1 == 0 { 1 } else { 59 },
                u16::from(value) * 257,
            );
            native.world.strategy_clock = 0;
            let record = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap();
            let actor = native.objects.get_mut(native.owner).unwrap();
            match field {
                0 => {
                    actor.base.shape = if value & 1 == 0 {
                        ShapeId::EMPTY
                    } else {
                        ShapeId::from_catalog_index(2)
                    }
                }
                1 => actor.base.flags.visible = value & 2 == 0,
                2 => actor.base.hit_points = value,
                3 => record.contact.as_mut().unwrap().ignores_contacts = value & 0x10 != 0,
                4 => {
                    let charge = record.charge.as_mut().unwrap();
                    charge.linked_mode = value & 0x80 != 0;
                    charge.linked_muzzle_disabled = value & 0x40 != 0;
                }
                5 => native.world.strategy_clock = u16::from(value) * 257,
                // All gates together, including both allowed linked-mode branches.
                6 => {
                    actor.base.flags.visible = value & 1 == 0;
                    actor.base.hit_points = value & 2;
                    actor.base.shape = if value & 4 == 0 {
                        ShapeId::EMPTY
                    } else {
                        ShapeId::from_catalog_index(2)
                    };
                    record.contact.as_mut().unwrap().ignores_contacts = value & 8 != 0;
                    record.charge.as_mut().unwrap().linked_mode = value & 0x10 != 0;
                    record.charge.as_mut().unwrap().linked_muzzle_disabled = value & 0x20 != 0;
                    native.world.strategy_clock = u16::from(value >> 6);
                }
                _ => record.visit.as_mut().unwrap().pilot_code = value,
            }
            seed(&mut source, &native, value, value);
            source.bus.write8(0, value);
            source.run(0x07CFB1, None, 0, OWNER, true);
            player_appearance::emit_damage_particle(
                &mut native.objects,
                &native.world,
                native.owner,
                value,
            )
            .unwrap();
            compare(&source, &native);
        }
    }
}

#[test]
fn original_retained_surface_flash_and_recovery_sequence_shares_one_appearance_owner() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = fixture(&mut source, 1, 0xFEDC);
    native.objects.get_mut(native.owner).unwrap().base.shape = ShapeId::EMPTY;
    seed(&mut source, &native, 0, 0);
    for visit in 0..4096_u16 {
        let record = native
            .world
            .player_mut(&native.objects, native.owner)
            .unwrap();
        record.contact.as_mut().unwrap().hit.reserve_shield = (visit % 29) as u8;
        record.visit.as_mut().unwrap().pilot_code = (visit >> 3) as u8;
        source.bus.write8(
            WRAM + SLOT + 0x6C00,
            record.contact.unwrap().hit.reserve_shield,
        );
        source
            .bus
            .write8(WRAM + SLOT + 0x6BFF, record.visit.unwrap().pilot_code);
        native.world.strategy_clock = visit.wrapping_add(65401);
        source.bus.write16(0xC4, native.world.strategy_clock);
        let mode = ((visit >> 2) % 3) as u8;
        native.world.player_carry_mode = Some(mode);
        source.bus.write8(0x1E13, mode);
        let carried = visit & 2 != 0;
        native
            .objects
            .get_mut(native.owner)
            .unwrap()
            .extension
            .path_state
            .motion
            .carry_selected_player = carried;
        source
            .bus
            .write8(u32::from(OWNER) + 0x21, if carried { 0x20 } else { 0 });
        if visit & 1 == 0 {
            surface(&mut source, &mut native);
            update(&mut source, &mut native, visit as u8);
        } else {
            update(&mut source, &mut native, visit as u8);
            surface(&mut source, &mut native);
        }
        publish(&mut source, &mut native);
    }
}

#[test]
fn original_retained_damage_children_exceed_five_and_full_pool_fault_precedes_flash_write() {
    let mut source = Source::new(&rom(), 0xA5);
    let mut native = fixture(&mut source, 1, 0);
    for _ in 1..60 {
        assert!(update(&mut source, &mut native, 254).is_some());
    }
    native
        .world
        .player_mut(&native.objects, native.owner)
        .unwrap()
        .visit
        .as_mut()
        .unwrap()
        .pilot_code = 1;
    source.bus.write8(WRAM + SLOT + 0x6BFF, 1);
    native
        .world
        .player_mut(&native.objects, native.owner)
        .unwrap()
        .appearance
        .as_mut()
        .unwrap()
        .depth_control = 0x79;
    source.bus.write8(WRAM + SLOT + 0x6AA2, 0x79);
    source.bus.write8(0, 254);
    source.run(0x06AA1A, Some(0x008032), 0, OWNER, true);
    assert_eq!(
        player_appearance::update(&mut native.objects, &mut native.world, native.owner, 254),
        Err(AppearanceError::ObjectPoolExhausted)
    );
    compare(&source, &native);
}

struct Callbacks;
impl sf2_game::scene_strategy::SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<sf2_game::strategy_schedule::StrategyCompletion, Self::Error> {
        panic!("damage particle must use its real authored path")
    }
    fn death_override(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<sf2_game::strategy_schedule::StrategyCompletion>, Self::Error> {
        panic!("unexpected death override")
    }
    fn resume_map_on_death(
        _: &mut sf2_game::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<(), Self::Error> {
        panic!("unexpected map continuation")
    }
}

#[test]
fn original_damage_particle_birth_real_path_jitter_animation_and_retirement_compose() {
    use sf2_game::scene_strategy::{SceneActors, SceneExecution};
    use sf2_game::strategy_schedule::StrategyHost;
    use sf_oracle::{call_near, Entry};
    let rom = rom();
    let catalog = authored_paths::catalog();
    for value in [0, 1, 127, 128, 255, 256, 32767, 32768, 65535] {
        let mut source = Source::new(&rom, 0);
        source.bus.enable_gsu();
        for (index, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
            source.bus.write8(0x7F7E00 + index as u32, byte);
        }
        source.run(0x7F1737, None, 0, OWNER, true);
        let mut native = fixture(&mut source, 1, value);
        native.world.strategy_clock = 0;
        source.bus.write16(0xC4, 0);
        let child = update(&mut source, &mut native, value as u8).unwrap();
        let base = u32::from(address(Some(child)));
        let mut execution = SceneExecution::default();
        let mut callbacks = Callbacks;
        let mut host = SceneActors {
            objects: &mut native.objects,
            world: &mut native.world,
            execution: &mut execution,
            catalog: &catalog,
            callbacks: &mut callbacks,
            statement_budget: 256,
        };
        for visit in 0..3 {
            let clock = value.wrapping_add(visit);
            source.bus.write16(0xC4, clock);
            let strategy = u32::from(source.bus.read16(base + 0x19))
                | u32::from(source.bus.read8(base + 0x1B)) << 16;
            source.run(strategy, None, 0, base as u16, true);
            host.run_strategy(child, clock).unwrap();
            let actor = host.objects.get(child).unwrap();
            for (offset, actual) in [
                (12, actor.base.position.x),
                (14, actor.base.position.y),
                (16, actor.base.position.z),
                (0x1CCF, actor.extension.relative_position.x),
                (0x1CD1, actor.extension.relative_position.y),
                (0x1CD3, actor.extension.relative_position.z),
            ] {
                assert_eq!(
                    source.bus.read16(WRAM + base + offset) as i16,
                    actual,
                    "seed {value} visit {visit} field {offset:X}"
                );
            }
            assert_eq!(
                source.bus.read16(WRAM + base + 0x1CE2),
                actor.extension.path_state.motion_phase
            );
            assert_eq!(
                source.bus.read8(WRAM + base + 0x1CDA),
                actor.extension.texture_scroll_x
            );
            assert_eq!(
                source.bus.read8(WRAM + base + 0x1CCA),
                actor.extension.path_state.animation.color.packed()
            );
            assert_eq!(
                source.bus.read8(base + 0x25) & 8 != 0,
                actor.base.flags.remove_after_tick
            );
            assert_eq!(actor.base.flags.remove_after_tick, visit == 2);
            for (index, byte) in host.world.random.bytes().into_iter().enumerate() {
                assert_eq!(source.bus.read8(0xE0 + index as u32), byte);
            }
        }
        let result = call_near(
            &mut source.bus,
            0x7F335A,
            &Entry {
                x: base as u16,
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            },
        );
        assert!(result.returned);
        host.retire_object(child).unwrap();
        assert!(host.objects.get(child).is_none());
        assert!(host
            .objects
            .get(native.owner)
            .unwrap()
            .base
            .first_child
            .is_none());
        assert_eq!(host.execution.paths.runtime.resources.owner_count(child), 0);
        native.compare_pool(&source);
    }
}
