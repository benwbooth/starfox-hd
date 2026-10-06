//! Compare the complete unmodified linked protection routine with its live
//! scene adapter. All source writes outside the bootstrap/stack are checked.

use super::*;
use sf2_game::collision_surface::SurfaceMode;
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_invocation::InvocationWorld;
use sf2_game::path_program::ActionGate;
use sf2_game::path_protection::{self, DeflectionProtection};
use sf2_game::path_scene_state::EncounterObjectiveCounts;
use sf2_game::player_action::PlayerServiceFlags;
use sf2_game::scene_path_world::PlayerPathRecords;

const STORAGE: u16 = 0x0200;
const ENTRY: u32 = 0x07F58D;
const FIRST_STATE: u32 = 0x033F;

#[test]
fn missing_configuration_keeps_exactly_the_original_spin_prefix() {
    let rom = rom();
    let mut source = Source::new(&rom, 0xA7);
    for byte in 0..=u8::MAX {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let native = objects.get_mut(owner).unwrap();
        native.extension.relative_rotation.pitch = Angle::from_units(byte);
        native.extension.relative_rotation.roll = Angle::from_units(!byte);
        native.extension.path_state.motion_phase = 0xABCD;
        for (offset, value) in [
            (0x1CD5, byte),
            (0x1CD7, !byte),
            (0x1CE2, 0xCD),
            (0x1CE3, 0xAB),
        ] {
            source.bus.write8(WRAM + u32::from(OWNER) + offset, value);
        }
        let before = objects.clone();
        source.run(ENTRY, Some(0x07F59F), 0, OWNER, true);
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut input = world
            .path_world(&objects, owner, PlayerTarget::Primary)
            .unwrap();
        assert_eq!(
            path_protection::update_effect(
                &mut objects,
                owner,
                input.protection.as_mut().unwrap(),
                input.surface_mode
            ),
            Err(path_protection::ProtectionError::MissingPlayerConfiguration)
        );
        let native = objects.get(owner).unwrap();
        assert_eq!(
            native.extension.relative_rotation.pitch.units(),
            source.bus.read8(WRAM + u32::from(OWNER) + 0x1CD5)
        );
        assert_eq!(
            native.extension.relative_rotation.roll.units(),
            source.bus.read8(WRAM + u32::from(OWNER) + 0x1CD7)
        );
        assert_eq!(
            native.extension.path_state.motion_phase,
            source.bus.read16(WRAM + u32::from(OWNER) + 0x1CE2)
        );
        let mut expected = before;
        expected
            .get_mut(owner)
            .unwrap()
            .extension
            .relative_rotation
            .pitch = native.extension.relative_rotation.pitch;
        expected
            .get_mut(owner)
            .unwrap()
            .extension
            .relative_rotation
            .roll = native.extension.relative_rotation.roll;
        assert_eq!(objects, expected);
    }
}

#[test]
fn protection_matches_original_for_every_control_byte_and_scene_gate_combination() {
    let rom = rom();
    let mut source = Source::new(&rom, 0xA7);
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let linked = actor(&mut objects);
    let selected = actor(&mut objects);
    objects.get_mut(owner).unwrap().base.attachment = Some(linked);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.primary_player = Some(selected);
    world
        .bind_player(&objects, linked, PlayerPathRecords::default())
        .unwrap();
    source.bus.write16(WRAM + u32::from(OWNER) + 6, OTHER);
    source.bus.write16(WRAM + u32::from(OTHER) + 0x2B, STORAGE);
    // Snapshot the full data region once, updating only seeded observations
    // and the four possible writes for each case. This catches stray source
    // writes, not merely agreement on an extracted final flag.
    let mut expected: Vec<u8> = (FIRST_STATE..0x10000)
        .map(|address| source.bus.read8(WRAM + address))
        .collect();
    let put = |source: &mut Source, expected: &mut [u8], address: u32, value: u8| {
        source.bus.write8(WRAM + address, value);
        expected[(address - FIRST_STATE) as usize] = value;
    };
    for control in 0..=u8::MAX {
        for flags in 0..16 {
            for surface in [0, 1, 7, 8, 128, 255] {
                let configuration = if flags & 1 != 0 {
                    9
                } else {
                    control.wrapping_add(10).max(10)
                };
                let blocked = if flags & 2 != 0 { control.max(1) } else { 0 };
                let service = (control & 0xFE) | u8::from(flags & 4 != 0);
                let contacts = if flags & 8 != 0 { control.max(1) } else { 0 };
                for (address, value) in [
                    (0x1DE2, configuration),
                    (0x1B4D, surface),
                    (0x1D72, blocked),
                    (0x1E0D, service),
                    (0xD7F4, contacts),
                    (0xD7F5, !control),
                    (u32::from(STORAGE) + 0x6C02, control),
                    (u32::from(OWNER) + 0x1CD5, control),
                    (u32::from(OWNER) + 0x1CD6, control ^ 0x91),
                    (u32::from(OWNER) + 0x1CD7, control.rotate_left(3)),
                    (u32::from(OWNER) + 0x1CE2, control ^ 0xAF),
                    (u32::from(OWNER) + 0x1CE3, control ^ 0xB7),
                ] {
                    put(&mut source, &mut expected, address, value);
                }
                let native = objects.get_mut(owner).unwrap();
                native.extension.relative_rotation = sf2_game::Rotation {
                    pitch: Angle::from_units(control),
                    yaw: Angle::from_units(control ^ 0x91),
                    roll: Angle::from_units(control.rotate_left(3)),
                };
                native.extension.path_state.motion_phase =
                    u16::from_le_bytes([control ^ 0xAF, control ^ 0xB7]);
                let mut expected_objects = objects.clone();
                world.scene.player_configuration = Some(configuration);
                world.surface_mode = Some(SurfaceMode { flags: surface });
                world.action_gate = Some(ActionGate { code: blocked });
                world.player_service_flags = Some(PlayerServiceFlags::from_bits(service));
                world.contacts_enabled = Some(contacts == 0); // deliberately stale
                world.objective_counts = Some(EncounterObjectiveCounts {
                    remaining_word: u16::from_le_bytes([contacts, !control]),
                    ..Default::default()
                });
                world.player_mut(&objects, linked).unwrap().protection =
                    Some(DeflectionProtection::from_control(control));
                source.run(ENTRY, None, 0, OWNER, true);
                let mut input = world
                    .path_world(&objects, owner, PlayerTarget::Primary)
                    .unwrap();
                let ordinary = path_protection::update_effect(
                    &mut objects,
                    owner,
                    input.protection.as_mut().unwrap(),
                    input.surface_mode,
                )
                .unwrap();
                drop(input);
                let native = objects.get(owner).unwrap();
                for (address, value) in [
                    (
                        u32::from(OWNER) + 0x1CD5,
                        native.extension.relative_rotation.pitch.units(),
                    ),
                    (
                        u32::from(OWNER) + 0x1CD7,
                        native.extension.relative_rotation.roll.units(),
                    ),
                    (
                        u32::from(OWNER) + 0x1CE2,
                        native.extension.path_state.motion_phase as u8,
                    ),
                    (
                        u32::from(STORAGE) + 0x6C02,
                        world
                            .player(&objects, linked)
                            .unwrap()
                            .protection
                            .unwrap()
                            .control(),
                    ),
                ] {
                    expected[(address - FIRST_STATE) as usize] = value;
                }
                for address in FIRST_STATE..0x10000 {
                    assert_eq!(
                        source.bus.read8(WRAM + address),
                        expected[(address - FIRST_STATE) as usize],
                        "control={control} flags={flags} surface={surface} address={address:04x}"
                    );
                }
                let state = expected_objects.get_mut(owner).unwrap();
                state.extension.relative_rotation.pitch =
                    Angle::from_units(source.bus.read8(WRAM + u32::from(OWNER) + 0x1CD5));
                state.extension.relative_rotation.roll =
                    Angle::from_units(source.bus.read8(WRAM + u32::from(OWNER) + 0x1CD7));
                state.extension.path_state.motion_phase =
                    source.bus.read16(WRAM + u32::from(OWNER) + 0x1CE2);
                assert_eq!(objects, expected_objects);
                assert_eq!(
                    ordinary,
                    source.bus.read8(WRAM + u32::from(OWNER) + 0x1CE2) & 0xFE != 0
                );
            }
        }
    }
}
