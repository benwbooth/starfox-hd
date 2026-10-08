//! Compare the shared reset's already-native service owners. The original
//! entire routine executes unchanged; this is not certification of its still
//! unbound movement, script-selector, terrain-render or continuous-audio state.

use super::{actor, rom, Source, OWNER, WRAM};
use sf2_game::collision_surface::SurfaceMode;
use sf2_game::hit_response::HitSide;
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_invocation::InvocationWorld;
use sf2_game::path_motion::PublishedPlayerMotion;
use sf2_game::path_program::ActionGate;
use sf2_game::path_radio::{RadioLayout, RadioRequest};
use sf2_game::path_scene_state::{CameraTrackingTarget, EncounterCameraFocus, EncounterHandoff};
use sf2_game::player_action::PlayerServiceFlags;
use sf2_game::player_hit_control::ShieldRecoveryRequest;
use sf2_game::player_scene_reset::{reset_services, PlayerSceneResetError};
use sf2_game::scene_path_world::ScenePathWorld;
use sf2_game::weapon_dispatch::WeaponState;
use sf2_game::{Angle, Buttons, InputState, ObjectId, ObjectStore, RandomState, Vector3};

fn input(held: u16, pressed: u16) -> InputState {
    InputState {
        held: Buttons::from_bits(held),
        pressed: Buttons::from_bits(pressed),
    }
}

fn setup(
    source: &mut Source,
    world: &mut ScenePathWorld,
    objects: &mut ObjectStore,
    owner: ObjectId,
    capacity: u8,
    shield: u8,
) {
    let word = u16::from_le_bytes([capacity, shield]);
    let side = if word & 1 == 0 {
        HitSide::Primary
    } else {
        HitSide::Secondary
    };
    objects.get_mut(owner).unwrap().base.contacts.hit_side = side;
    source.bus.write8(
        WRAM + u32::from(OWNER) + 0x23,
        if side == HitSide::Primary { 0xBF } else { 0xFF },
    );
    world.controller_inputs = [
        Some(input(word, !word)),
        Some(input(word.rotate_left(5), word.rotate_right(7))),
    ];
    for (held, pressed, value) in [
        (0x1292, 0x1296, world.controller_inputs[0].unwrap()),
        (0x1294, 0x1298, world.controller_inputs[1].unwrap()),
    ] {
        source.bus.write16(WRAM + held, value.held.bits());
        source.bus.write16(WRAM + pressed, value.pressed.bits());
    }
    // Seed dirty destinations and neighboring publications afresh each time.
    for address in 0x1D69..0x1E71 {
        source.bus.write8(WRAM + address, shield ^ 0xA7);
    }
    source.bus.write16(WRAM + 0x1936, 0xA55A);
    source.bus.write16(WRAM + 0x1938, 0x5AA5);
    source.bus.write8(WRAM + 0x1DD5, capacity);
    source.bus.write8(WRAM + 0x1DD1, shield);
    source.bus.write8(WRAM + 0x1DE2, 9);
    source.bus.write8(WRAM + 0x1DF2, 171);
    source.bus.write8(WRAM + 0x1B4D, 213);
    source.bus.write8(WRAM + 0x1E2F, 0xEB);
    source.bus.write8(WRAM + 0x1D72, 73);
    source.bus.write8(WRAM + 0x1D74, 181);
    source.bus.write8(WRAM + 0x1E1B, 74);
    source.bus.write8(WRAM + 0x1E0D, 0xF5);
    source.bus.write8(WRAM + 0x1E30, 43);
    source.bus.write8(WRAM + 0x1E31, shield);
    source.bus.write16(WRAM + 0x1D88, 0xCDEF);
    source.bus.write16(WRAM + 0x1D8C, 0x1234);
    source.bus.write16(WRAM + 0x1D8E, 0x89AB);
    source.bus.write16(WRAM + 0x1DFF, OWNER);
    for base in [0x1E01, 0xD7EC, 0x1E1C] {
        for (axis, value) in [197, -257i16, i16::MIN].into_iter().enumerate() {
            source
                .bus
                .write16(WRAM + base + 2 * axis as u32, value as u16);
        }
    }
    source.bus.write16(WRAM + 0x1E0F, 1931);
    source.bus.write16(WRAM + 0x1D6F, OWNER);
    source.bus.write8(WRAM + 0x1D71, shield);
    source.bus.write16(WRAM + 0x1B8A, 0xA5A5);
    world.processed_player_input = Some(input(0x5AA5, 0xA55A));
    world.scene.player_configuration = Some(9);
    world.interception_music_ready = Some(shield ^ 0xA7 != 0);
    world.interception_active = Some(true);
    world.linked_effect_activity = Some(sf2_game::path_protection::LinkedEffectActivity { recent_spawn: shield ^ 0xA7 });
    world.scene.active_shield = Some(shield);
    world.active_shield_capacity = Some(capacity);
    world.weapons = Some(WeaponState {
        published_pitch: Some(Angle::from_units(171)),
        ..Default::default()
    });
    world.surface_mode = Some(SurfaceMode { flags: 213 });
    world.node_exit = sf2_game::player_node_exit::NodeExitState {
        presentation_flags: Some(shield ^ 0xA7),
        completion_code: Some(shield ^ 0xA7),
    };
    world.reticle_enabled = Some(true);
    world.action_gate = Some(ActionGate { code: 73 });
    world.handoff = Some(EncounterHandoff {
        player_flags: 181,
        x: 0xCDEFu16 as i16,
        z: 0x1234,
        heading_word: 0x89AB,
    });
    world.shield_recovery = Some(ShieldRecoveryRequest { amount: 74 });
    world.player_service_flags = Some(PlayerServiceFlags::from_bits(0xF5));
    world.target_reticle.horizontal = Some(43);
    world.target_reticle.vertical = Some(shield);
    world.camera_tracking = Some(CameraTrackingTarget { actor: Some(owner) });
    let position = Vector3 {
        x: 197,
        y: -257,
        z: i16::MIN,
    };
    world.camera_focus = Some(EncounterCameraFocus { position });
    world.published_motion = Some(PublishedPlayerMotion {
        position,
        delta: position,
    });
    world.environment_plane_height = Some(1931);
    world.player_surface_support = Some(sf2_game::player_motion::PlayerSurfaceSupport { object: Some(owner), group: shield });
}

fn compare(source: &mut Source, world: &ScenePathWorld, owner: ObjectId) {
    let processed = world.processed_player_input.unwrap();
    assert_eq!(processed.held.bits(), source.bus.read16(WRAM + 0x1938));
    assert_eq!(processed.pressed.bits(), source.bus.read16(WRAM + 0x1936));
    for (address, value) in [
        (0x1DE2, world.scene.player_configuration.unwrap()),
        (0x1DDE, u8::from(world.interception_music_ready.unwrap())),
        (0x1DDF, world.linked_effect_activity.unwrap().recent_spawn),
        (0x1DD1, world.scene.active_shield.unwrap()),
        (0x1DD5, world.active_shield_capacity.unwrap()),
        (0x1B4D, world.surface_mode.unwrap().flags),
        (0x1E08, world.node_exit.presentation_flags.unwrap()),
        (0x1E17, world.node_exit.completion_code.unwrap()),
        (0x1D72, world.action_gate.unwrap().code),
        (0x1E1B, world.shield_recovery.unwrap().amount),
        (0x1E0D, world.player_service_flags.unwrap().bits()),
        (0x1E30, world.target_reticle.horizontal.unwrap()),
        (0x1E31, world.target_reticle.vertical.unwrap()),
    ] {
        assert_eq!(
            value,
            source.bus.read8(WRAM + address),
            "service {address:04X}"
        );
    }
    assert_eq!(world.interception_active, Some(true));
    assert_eq!(source.bus.read16(WRAM + 0x1B8A), 0xA5A5);
    if let Some(weapons) = world.weapons {
        assert_eq!(
            weapons.published_pitch.unwrap().units(),
            source.bus.read8(WRAM + 0x1DF2)
        );
    }
    if let Some(handoff) = world.handoff {
        assert_eq!(handoff.player_flags, source.bus.read8(WRAM + 0x1D74));
        assert_eq!(handoff.x as u16, source.bus.read16(WRAM + 0x1D88));
        assert_eq!(handoff.z as u16, source.bus.read16(WRAM + 0x1D8C));
        assert_eq!(handoff.heading_word, source.bus.read16(WRAM + 0x1D8E));
    }
    assert_eq!(
        world.reticle_enabled,
        Some(source.bus.read8(WRAM + 0x1E2F) & 0x80 != 0)
    );
    let tracked = source.bus.read16(WRAM + 0x1DFF);
    assert!(tracked == 0 || tracked == OWNER);
    assert_eq!(
        world.camera_tracking.unwrap().actor,
        (tracked == OWNER).then_some(owner)
    );
    for (base, position) in [
        (0x1E01, world.camera_focus.unwrap().position),
        (0xD7EC, world.published_motion.unwrap().position),
        (0x1E1C, world.published_motion.unwrap().delta),
    ] {
        for (axis, value) in [position.x, position.y, position.z].into_iter().enumerate() {
            assert_eq!(
                value as u16,
                source.bus.read16(WRAM + base + 2 * axis as u32)
            );
        }
    }
    assert_eq!(
        world.environment_plane_height.unwrap() as u16,
        source.bus.read16(WRAM + 0x1E0F)
    );
    // Absent when an earlier missing owner stopped the native reset.
    if let Some(offset) = world.camera_projection_offset {
        assert_eq!(offset as u16, source.bus.read16(WRAM + 0x1E52));
    }
    if let Some(projection) = world.published_camera_projection {
        assert_eq!(projection as u16, source.bus.read16(WRAM + 0x1E3C));
    }
    let support = world.player_surface_support.unwrap();
    assert_eq!(support.object.map_or(0, |_| OWNER), source.bus.read16(WRAM + 0x1D6F));
    assert_eq!(support.group, source.bus.read8(WRAM + 0x1D71));
}

#[test]
fn reset_service_publications_match_complete_original_routine_for_every_shield_pair() {
    let mut source = Source::new(&rom(), 0x59);
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    for capacity in 0..=u8::MAX {
        for shield in 0..=u8::MAX {
            setup(
                &mut source,
                &mut world,
                &mut objects,
                owner,
                capacity,
                shield,
            );
            let actor = objects.get(owner).unwrap().clone();
            source.run(0x06958C, None, 0xBCD9, OWNER, true);
            reset_services(&objects, &mut world, owner).unwrap();
            compare(&mut source, &world, owner);
            assert_eq!(objects.get(owner), Some(&actor));
            // The companion/configuration, view controls, homing target and
            // movement history are not reset by this original routine.
            for address in [
                0x1DE0, 0x1DE3, 0x1D90, 0x1D91, 0x1D69, 0x1D6A, 0x1D6B, 0x1D6C, 0x1D6D, 0x1D6E,
            ] {
                assert_eq!(source.bus.read8(WRAM + address), shield ^ 0xA7);
            }
        }
    }
}

#[test]
fn reset_missing_native_owners_match_original_completed_write_boundaries() {
    let mut source = Source::new(&rom(), 0x59);
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    for shield in 0..=u8::MAX {
        for stage in 0..2 {
            setup(&mut source, &mut world, &mut objects, owner, 32, shield);
            let (stop, error) = if stage == 0 {
                world.weapons = None;
                (0x0695DF, PlayerSceneResetError::MissingWeaponState)
            } else {
                world.handoff = None;
                (0x06960E, PlayerSceneResetError::MissingHandoff)
            };
            source.run(0x06958C, Some(stop), 0, OWNER, true);
            assert_eq!(reset_services(&objects, &mut world, owner), Err(error));
            compare(&mut source, &world, owner);
        }
    }
}

#[test]
fn original_reset_selects_controller_before_clearing_and_repeats_only_horizontal_store() {
    let mut source = Source::new(&rom(), 0x59);
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    for capacity in [32, 33] {
        setup(&mut source, &mut world, &mut objects, owner, capacity, 42);
        source.writes = Some(Vec::new());
        source.run(0x06958C, None, 0, OWNER, true);
        let input = world.controller_inputs[usize::from(capacity & 1)].unwrap();
        let selected: Vec<_> = source
            .writes
            .as_ref()
            .unwrap()
            .iter()
            .copied()
            .filter(|(address, _)| (WRAM + 0x1936..WRAM + 0x193A).contains(address))
            .collect();
        assert_eq!(
            selected,
            [
                (WRAM + 0x1936, input.pressed.bits() as u8),
                (WRAM + 0x1937, (input.pressed.bits() >> 8) as u8),
                (WRAM + 0x1938, input.held.bits() as u8),
                (WRAM + 0x1939, (input.held.bits() >> 8) as u8),
                (WRAM + 0x1938, 0),
                (WRAM + 0x1939, 0),
                (WRAM + 0x1936, 0),
                (WRAM + 0x1937, 0),
            ]
        );
        let reticle: Vec<_> = source
            .writes
            .as_ref()
            .unwrap()
            .iter()
            .copied()
            .filter(|(address, _)| *address == WRAM + 0x1E30 || *address == WRAM + 0x1E31)
            .collect();
        assert_eq!(reticle, [(WRAM + 0x1E30, 100); 2]);
        source.writes = None;
    }
}

#[test]
fn original_reset_hidden_reticle_and_radio_share_one_vertical_publication() {
    let mut source = Source::new(&rom(), 0x59);
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    for maximum in [0u8, 1, 128, 255] {
        let compact = maximum != 0;
        for vertical in 0..=u8::MAX {
            setup(&mut source, &mut world, &mut objects, owner, 32, vertical);
            source.bus.write16(WRAM + 0x12C3, OWNER);
            source.bus.write8(WRAM + 0xD775, maximum);
            world.primary_player = Some(owner);
            world.health_display = Some(sf2_game::path_scene_state::EncounterHealthDisplay {
                maximum,
                ..Default::default()
            });
            world.radio = Some((
                RadioRequest::default(),
                RadioLayout {
                    compact_panel: !compact,
                    tracked_screen_y: vertical.wrapping_add(128),
                },
            ));
            source.run(0x06958C, None, 0, OWNER, true);
            reset_services(&objects, &mut world, owner).unwrap();
            for hidden_display in [false, true] {
                if hidden_display {
                    source.run(0x07A418, Some(0x07A505), 0, OWNER, true);
                    sf2_game::player_reticle::position(&mut objects, &mut world, owner).unwrap();
                }
                source.run(0x0ACF04, None, u16::from(vertical), OWNER, true);
                world
                    .path_world(&objects, owner, PlayerTarget::Primary)
                    .unwrap()
                    .radio
                    .unwrap()
                    .request_message(vertical);
                let request = world.radio.as_ref().unwrap().0;
                assert_eq!(
                    request.message.index() as u8,
                    source.bus.read8(WRAM + 0xCF31)
                );
                assert_eq!(request.pending, source.bus.read8(WRAM + 0xCF32) != 0);
                assert_eq!(request.panel_y, source.bus.read16(WRAM + 0xD744));
                assert_eq!(request.top_placement, source.bus.read8(WRAM + 0xD759) != 0);
                assert_eq!(
                    world.target_reticle.vertical,
                    Some(source.bus.read8(WRAM + 0x1E31))
                );
            }
        }
    }
}
