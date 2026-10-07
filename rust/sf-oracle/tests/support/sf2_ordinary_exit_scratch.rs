//! Original-only retained execution proves whether a null-child scratch store is live.
use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_program::{ActionGate, SelectedAuxiliaryState};
use sf2_game::path_protection::{DeflectionProtection, LinkedEffectActivity};
use sf2_game::path_scene_state::{CameraTrackingTarget, EncounterHandoff};
use sf2_game::player_engine_sound::EngineSoundControl;
use sf2_game::{authored_paths, Behavior, RandomState};
use sf_oracle::{call_near, Entry};

#[test]
fn original_ordinary_exit_null_child_retirement_has_no_live_scratch_read_in_complete_continuation()
{
    let rom = rom();
    for pilot in 0..6u8 {
        for protection in [0u8, 0xFF] {
            for direction in 0..8u8 {
                let mode = if pilot & 1 == 0 { 0x10 } else { 0x30 };
                let mut source = Source::new(&rom, 0);
                source.bus.enable_gsu();
                for (i, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
                    source.bus.write8(0x7F7E00 + i as u32, byte);
                }
                source.run(0x7F1737, None, 0, OWNER, true);
                let mut native = Native::new(&mut source, 3, 87, 193, 0xEF73);
                let player = native.owner;
                let owner = native.objects.active_ids()[1];
                let view = native.objects.active_ids()[0];
                let base = u32::from(address(Some(owner)));
                let root = native.objects.get_mut(owner).unwrap();
                root.base.behavior = Behavior::FollowPath;
                root.base.path = Some(authored_paths::ORDINARY_SCENE_EXIT);
                root.base.hit_points = 1;
                root.base.attack_power = 1;
                root.base.flags.reclaim_on_pool_pressure = false;
                root.extension.path_state.needs_path_initialization = true;
                source.bus.write16(base + 0x2B, 0xCF18);
                source.bus.write16(base + 0x19, 0x7E1E);
                source.bus.write8(base + 0x1B, 0x7F);
                source.bus.write8(base + 0x20, 8);
                source.bus.write8(base + 0x2D, 1);
                source.bus.write8(base + 0x2E, 1);
                native.world.primary_player = Some(player);
                native.world.fixed_players[0] = Some(view);
                native.world.action_gate = Some(ActionGate { code: 0 });
                native.world.camera_tracking = Some(CameraTrackingTarget::default());
                native.world.engine_sound_control = Some(EngineSoundControl::from_bits(0xED));
                native.world.linked_effect_activity =
                    Some(LinkedEffectActivity { recent_spawn: 0xAD });
                native.world.handoff = Some(EncounterHandoff {
                    player_flags: 0xC0,
                    x: 1717,
                    z: -9123,
                    heading_word: u16::from_le_bytes([0x73, direction]),
                });
                native.world.scene.active_pilot = Some(pilot);
                native.world.scene.active_shield = Some(if pilot & 1 == 0 { 80 } else { 7 });
                source
                    .bus
                    .write8(0x1DD1, native.world.scene.active_shield.unwrap());
                native.world.scene.encounter_node_mode =
                    Some(if pilot & 2 == 0 { 0 } else { 0x80 });
                source.bus.write8(
                    WRAM + 0xD79B,
                    native.world.scene.encounter_node_mode.unwrap(),
                );
                native.world.scene.entry_heading = Some(pilot.wrapping_mul(43).wrapping_add(17));
                source
                    .bus
                    .write8(0x1BA9, native.world.scene.entry_heading.unwrap());
                native.world.scene.player_configuration = Some(if pilot & 1 == 0 { 9 } else { 3 });
                native.world.scene.encounter_location = Some(if pilot & 2 == 0 { 2 } else { 5 });
                native.world.random = RandomState::new([
                    pilot.wrapping_add(1),
                    protection.wrapping_add(31),
                    mode,
                    0xAB,
                ]);
                let player_state = native.world.player_mut(&native.objects, player).unwrap();
                player_state.protection = Some(DeflectionProtection::from_control(protection));
                player_state.auxiliary = Some(SelectedAuxiliaryState {
                    mode,
                    action_flags: 0,
                    stored_world_position: Default::default(),
                    stored_rotation: Default::default(),
                });
                player_state.target_control = Some(Default::default());
                player_state.target_control.as_mut().unwrap().mode =
                    if pilot & 1 == 0 { 8 } else { 7 };
                player_state.contact = Some(Default::default());
                player_state.contact.as_mut().unwrap().hit.reserve_shield =
                    if pilot & 2 == 0 { 80 } else { 0 };
                player_state.contact.as_mut().unwrap().hit.feedback_duration = 37;
                player_state.contact.as_mut().unwrap().hit.feedback_flags = 0x80;
                source.bus.write16(0x12C3, address(Some(player)));
                source.bus.write8(WRAM + SLOT + 0x6C02, protection);
                source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
                source
                    .bus
                    .write16(WRAM + SLOT + 0x6C1C, if pilot & 1 == 0 { 8 } else { 7 });
                source
                    .bus
                    .write8(WRAM + SLOT + 0x6C00, if pilot & 2 == 0 { 80 } else { 0 });
                source.bus.write8(WRAM + SLOT + 0x6C11, 37);
                source.bus.write8(WRAM + SLOT + 0x6C12, 0x80);
                source.bus.write8(0x1CE5, 0xED);
                source.bus.write8(0x1DDF, 0xAD);
                source.bus.write8(0x1D74, 0xC0);
                source.bus.write16(0x1D88, 1717);
                source.bus.write16(0x1D8C, (-9123i16) as u16);
                source
                    .bus
                    .write16(0x1D8E, u16::from_le_bytes([0x73, direction]));
                source.bus.write8(0x1E14, pilot);
                source
                    .bus
                    .write8(0x1DE2, native.world.scene.player_configuration.unwrap());
                source
                    .bus
                    .write16(0x1BB5, native.world.scene.encounter_location.unwrap());
                for (i, byte) in native.world.random.bytes().into_iter().enumerate() {
                    source.bus.write8(0xE0 + i as u32, byte);
                }

                for visit in 0..240u16 {
                    source.bus.write16(0xC4, visit);
                    if visit == 70 {
                        source.bus.write8(0x1D72, 3);
                    }
                    if visit == 170 {
                        source.bus.write8(0x1D72, 0);
                    }
                    let mut pending = source.bus.read16(0x12A8);
                    while pending != 0 {
                        let base = u32::from(pending);
                        if pending != address(Some(player)) && pending != address(Some(view)) {
                            if visit == 70 && pending == address(Some(owner)) {
                                assert_eq!(source.bus.read16(base + 6), 0);
                                assert_eq!(source.bus.read16(0x29), 0);
                                assert_eq!(source.bus.read8(base + 0x23) & 0x10, 0);
                                source.byte_accesses = Some((0x25, Vec::new()));
                            }
                            source.run(0x7F3565, Some(0x7F357B), 0, pending, true);
                        }
                        pending = source.bus.read16(base);
                    }
                    let mut pending = source.bus.read16(0x12A8);
                    while pending != 0 {
                        let next = source.bus.read16(u32::from(pending));
                        if source.bus.read8(u32::from(pending) + 0x25) & 8 != 0 {
                            assert!(
                                call_near(
                                    &mut source.bus,
                                    0x7F335A,
                                    &Entry {
                                        x: pending,
                                        dbr: 0x7E,
                                        p: 0x20,
                                        ..Default::default()
                                    }
                                )
                                .returned
                            );
                        }
                        pending = next;
                    }
                }
                let (_, accesses) = source.byte_accesses.as_ref().unwrap();
                let store = accesses
                    .iter()
                    .position(|(write, _)| *write)
                    .expect("original scratch store");
                assert_ne!(accesses[store].1 & 8, 0);
                assert!(
                    accesses[store + 1..].iter().all(|(write, _)| *write),
                    "pilot {pilot} direction {direction} protection {protection}: {accesses:?}"
                );
                assert_eq!(source.bus.read8(0x1D74) & 13, 13);
            }
        }
    }
}
