//! Independent helper execution over every direction byte, yaw and coordinate.
//! The native catalog supplies native operands; the original reads its own ROM.
use super::special_exit_tests::compare_actor;
use super::surface_particle_tests::{address, Native};
use super::{rom, Source, WRAM};
use sf2_game::path_commands::ControlCommand;
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_invocation::InvocationWorld;
use sf2_game::path_program::{PathCatalog, Statement};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::path_scene_state::EncounterHandoff;
use sf2_game::{authored_paths, Angle, ObjectId, PathCursor, PathId, Vector3};

fn cursor(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

fn sync_initial(source: &mut Source, native: &Native, id: ObjectId) {
    let a = native.objects.get(id).unwrap();
    let base = u32::from(address(Some(id)));
    for (offset, value) in [
        (12, a.base.position.x),
        (14, a.base.position.y),
        (16, a.base.position.z),
        (0x32, a.base.velocity.x),
        (0x34, a.base.velocity.y),
        (0x36, a.base.velocity.z),
        (0x1CCF, a.extension.relative_position.x),
        (0x1CD1, a.extension.relative_position.y),
        (0x1CD3, a.extension.relative_position.z),
    ] {
        source.bus.write16(WRAM + base + offset, value as u16);
    }
    for (offset, value) in [
        (18, a.base.pitch.units()),
        (20, a.base.yaw.units()),
        (22, a.base.roll.units()),
        (0x18, a.base.speed),
        (10, a.base.target_speed),
        (0x2E, a.base.attack_power),
        (0x1CD5, a.extension.relative_rotation.pitch.units()),
        (0x1CD6, a.extension.relative_rotation.yaw.units()),
        (0x1CD7, a.extension.relative_rotation.roll.units()),
    ] {
        source.bus.write8(WRAM + base + offset, value);
    }
    source
        .bus
        .write16(WRAM + base + 0x1CE2, a.extension.path_state.motion_phase);
}

fn seed(native: &mut Native, id: ObjectId, word: u16) {
    let actor = native.objects.get_mut(id).unwrap();
    actor.base.position = Vector3 {
        x: word as i16,
        y: word.rotate_left(7) as i16,
        z: word.wrapping_neg() as i16,
    };
    actor.base.velocity = Vector3 {
        x: !word as i16,
        y: word as i16,
        z: word.rotate_left(4) as i16,
    };
    actor.base.pitch = Angle::from_units(word as u8);
    actor.base.yaw = Angle::from_units((word >> 8) as u8);
    actor.base.roll = Angle::from_units((!word) as u8);
    actor.base.attack_power = 231;
    actor.base.speed = 133;
    actor.base.target_speed = 51;
    actor.extension.relative_position = actor.base.velocity;
    actor.extension.relative_rotation.pitch = Angle::from_units(129);
    actor.extension.relative_rotation.yaw = Angle::from_units(77);
    actor.extension.relative_rotation.roll = Angle::from_units(241);
    actor.extension.path_state.motion_phase = word;
}

#[test]
fn ordinary_exit_craft_initializer_matches_original_all_direction_bytes_and_coordinate_words() {
    let bytes = rom();
    let mut source = Source::new(&bytes, 0);
    let mut native = Native::new(&mut source, 3, 0, 0, 0);
    let owner = native.objects.active_ids()[1];
    let generated = authored_paths::catalog();
    let poses = (0..authored_paths::LOWERED_COMMAND_COUNT)
        .find_map(|i| match generated.statement(cursor(i as u16)).unwrap() {
            Statement::InitializeExitCraft { poses, .. } => Some(poses),
            _ => None,
        })
        .unwrap();
    let catalog = PathCatalog::new(vec![vec![
        Statement::InitializeExitCraft {
            poses,
            next: cursor(1),
        },
        Statement::Control(ControlCommand::Hold),
    ]])
    .unwrap();
    let mut runtime = PathRuntime::default();
    for word in 0..=u16::MAX {
        seed(&mut native, owner, word);
        sync_initial(&mut source, &native, owner);
        let heading_word = word.rotate_left(8);
        source.bus.write16(0x1D8E, heading_word);
        native.world.handoff = Some(EncounterHandoff {
            heading_word,
            ..Default::default()
        });
        source.run(0x07F808, None, 0, address(Some(owner)), true);
        native.objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
        let mut world = native
            .world
            .path_world(&native.objects, owner, PlayerTarget::Primary)
            .unwrap();
        let _ = runtime
            .step_program(&catalog, &mut native.objects, owner, &mut world)
            .unwrap();
        compare_actor(&source, &native, owner, word);
    }
}

#[test]
fn ordinary_exit_view_placement_matches_original_all_directions_yaws_and_owner_alias() {
    let bytes = rom();
    let mut source = Source::new(&bytes, 0);
    let mut native = Native::new(&mut source, 3, 0, 0, 0);
    let owner = native.objects.active_ids()[1];
    let target = native.objects.active_ids()[0];
    let generated = authored_paths::catalog();
    let depths = (0..authored_paths::LOWERED_COMMAND_COUNT)
        .find_map(|i| match generated.statement(cursor(i as u16)).unwrap() {
            Statement::PositionExitView { depths, .. } => Some(depths),
            _ => None,
        })
        .unwrap();
    let catalog = PathCatalog::new(vec![vec![
        Statement::PositionExitView {
            depths,
            next: cursor(1),
        },
        Statement::Control(ControlCommand::Hold),
    ]])
    .unwrap();
    let mut runtime = PathRuntime::default();
    for alias in [false, true] {
        for word in 0..=u16::MAX {
            seed(&mut native, owner, word);
            seed(&mut native, target, !word);
            for id in [owner, target] {
                sync_initial(&mut source, &native, id);
            }
            let target = if alias { owner } else { target };
            source.bus.write16(WRAM + 0xD771, address(Some(target)));
            runtime.spawns.last_spawn = Some(target);
            let heading_word = word.rotate_left(8);
            let entry_heading = word.rotate_left(3) as u8;
            source.bus.write16(0x1D8E, heading_word);
            source.bus.write8(0x1BA9, entry_heading);
            native.world.handoff = Some(EncounterHandoff {
                heading_word,
                ..Default::default()
            });
            native.world.scene.entry_heading = Some(entry_heading);
            source.run(0x07F3E1, None, 0, address(Some(owner)), true);
            native.objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
            let mut world = native
                .world
                .path_world(&native.objects, owner, PlayerTarget::Primary)
                .unwrap();
            let _ = runtime
                .step_program(&catalog, &mut native.objects, owner, &mut world)
                .unwrap();
            compare_actor(&source, &native, owner, word);
            compare_actor(&source, &native, target, word);
        }
    }
}
