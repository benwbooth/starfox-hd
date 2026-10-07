//! Original mission admission, real allocation and deferred path birth.
use super::special_exit_tests::compare_actor;
use super::surface_particle_tests::{address, Native, OWNER};
use super::{rom, Source, WRAM};
use sf2_game::path_scene_state::EncounterObjectiveCounts;
use sf2_game::player_node_exit::{self, NodeExitError, NodeExitState};
use sf2_game::view_transition::ViewTransitionMode;
use sf2_game::ObjectId;

#[test]
fn layout_advance_matches_original_every_handoff_flag_and_layout_byte() {
    use sf2_game::path_scene_state::EncounterHandoff;
    use sf2_game::scene_path_world::ScenePathWorld;
    let mut source = Source::new(&rom(), 0);
    let mut world = ScenePathWorld::new(Default::default());
    for flags in 0..=u8::MAX {
        for layout in 0..=u8::MAX {
            world.handoff = Some(EncounterHandoff {
                player_flags: flags,
                ..Default::default()
            });
            world.scene.encounter_layout = Some(layout);
            source.bus.write16(0x1D74, 0xA700 | u16::from(flags));
            source.bus.write16(0x1BA5, 0xC700 | u16::from(layout));
            source.run(0x06A0F9, Some(0x06A108), 0, OWNER, true);
            sf2_game::player_mission::consume_layout_advance(&mut world).unwrap();
            assert_eq!(
                source.bus.read16(0x1D74),
                0xA700 | u16::from(world.handoff.unwrap().player_flags)
            );
            assert_eq!(
                source.bus.read16(0x1BA5),
                0xC700 | u16::from(world.scene.encounter_layout.unwrap())
            );
        }
    }
}

fn prepare(source: &mut Source, count: usize, objectives: u16, flags: u8, code: u8) -> Native {
    let mut native = Native::new(source, count, flags, code, objectives);
    native.world.objective_counts = Some(EncounterObjectiveCounts {
        remaining_word: objectives,
        node_record: 173,
        recorded_completions: 0xCDEF,
        signaled_completions: 0xFEDC,
    });
    native.world.node_exit = NodeExitState {
        presentation_flags: Some(flags),
        completion_code: Some(code),
    };
    let mode = u16::from(flags).rotate_left(3) ^ 0xA755;
    native.world.view_transition_mode = Some(ViewTransitionMode { flags: mode });
    native.world.spawn_defaults.as_mut().unwrap().group = code;
    source.bus.write16(0x1B84, mode);
    source.bus.write8(0x190E, code);
    source.bus.write16(WRAM + 0xD7F4, objectives);
    source.bus.write8(0x1E08, flags);
    source.bus.write8(0x1E17, code);
    for (location, value) in [(0x1E07, 71), (0x1E09, 97), (0x1E16, 139), (0x1E18, 211)] {
        source.bus.write8(location, value);
    }
    // These are not authored-path spawns: retain the independent mailbox.
    source.bus.write16(WRAM + 0xD771, 0xBEEF);
    native
}

fn visit(source: &mut Source, native: &mut Native) -> Result<Option<ObjectId>, NodeExitError> {
    let count = native.objects.len();
    let objectives = native.world.objective_counts.unwrap().remaining_word;
    let flags = native.world.node_exit.presentation_flags.unwrap();
    let request = objectives as u8 != 0 && flags & 0xC0 == 0x80;
    let full = count == 60 && request;
    source.run(
        0x06A045,
        Some(if full { 0x008032 } else { 0x06A0A5 }),
        0,
        OWNER,
        true,
    );
    let result = player_node_exit::advance(&mut native.objects, &mut native.world);
    if full {
        assert_eq!(result, Err(NodeExitError::ObjectPoolExhausted));
    } else {
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(result.unwrap().is_some(), request);
    }
    assert_eq!(
        source.bus.read16(WRAM + 0xD7F4),
        native.world.objective_counts.unwrap().remaining_word
    );
    assert_eq!(
        source.bus.read8(0x1E08),
        native.world.node_exit.presentation_flags.unwrap()
    );
    assert_eq!(
        source.bus.read8(0x1E17),
        native.world.node_exit.completion_code.unwrap()
    );
    assert_eq!(source.bus.read16(WRAM + 0xD771), 0xBEEF);
    for (location, expected) in [(0x1E07, 71), (0x1E09, 97), (0x1E16, 139), (0x1E18, 211)] {
        assert_eq!(source.bus.read8(location), expected);
    }
    native.compare_pool(source);
    if let Ok(Some(id)) = result {
        compare_actor(source, native, id, 0);
        let base = u32::from(address(Some(id)));
        let actor = native.objects.get(id).unwrap();
        assert_eq!(source.bus.read16(base + 0x19), 0x7E1E);
        assert_eq!(source.bus.read8(base + 0x1B), 0x7F);
        assert_eq!(source.bus.read16(base + 0x2B), 0xB8C5);
        assert_eq!(
            source.bus.read8(WRAM + base + 0x1CF0),
            actor.extension.spawn_group
        );
        assert!(actor.extension.path_state.needs_path_initialization);
        assert_eq!(
            source.bus.read8(base + 0x25) & 1 != 0,
            actor.base.flags.remove_with_parent
        );
        assert_eq!(
            source.bus.read8(base + 0x23) & 4 != 0,
            actor.extension.path_state.motion.attached_coordinates
        );
        assert_eq!(
            actor.base.path,
            Some(sf2_game::authored_paths::NODE_EXIT_PRESENTATION)
        );
    }
    result
}

#[test]
fn node_exit_admission_matches_original_every_request_and_completion_byte() {
    let mut source = Source::new(&rom(), 0);
    for flags in 0..=u8::MAX {
        for code in 0..=u8::MAX {
            let mut native = prepare(&mut source, 3, 0xAB31, flags, code);
            visit(&mut source, &mut native).unwrap();
            // Retain state for another real visit: the creation flag latches,
            // while the completion selector itself is never consumed.
            assert_eq!(visit(&mut source, &mut native), Ok(None));
        }
    }
}

#[test]
fn node_exit_admission_matches_original_all_objective_words_and_pool_boundaries() {
    let mut source = Source::new(&rom(), 0);
    for objectives in 0..=u16::MAX {
        let code = if objectives & 1 == 0 { 1 } else { 255 };
        let mut native = prepare(&mut source, 3, objectives, 0xAF, code);
        visit(&mut source, &mut native).unwrap();
    }
    for count in [1, 2, 4, 59, 60] {
        for flags in [0, 0x3F, 0x7F, 0x80, 0xBF, 0xFF] {
            for code in [0, 1, 255] {
                let mut native = prepare(&mut source, count, 0xCAFE, flags, code);
                let _ = visit(&mut source, &mut native);
            }
        }
    }
}
