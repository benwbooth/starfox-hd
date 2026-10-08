//! Indexed scene entry (`$06:A8D2..A987`) against the original routine:
//! sentinels, saved-scene substitution, allocation, pool pressure and the
//! table-driven action/path record. Unsupported entries must fault after
//! the same allocation the source performs.
use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_control::PlayerTarget;
use sf2_game::path_program::ActionGate;
use sf2_game::path_sound::AuthoredCue;
use sf2_game::player_action::{AuthoredSceneAction, PlayerAction, PlayerActionState};
use sf2_game::player_scene_entry::{self, EntryOutcome, SceneEntryError, SceneEntryPhase};
use sf2_game::scene_install::{self, SceneInstallError};
use sf2_game::Behavior;

/// Installed scenes: (selection, path root, action script, action, companion seed).
const SCENES: [(u8, u16, u16, AuthoredSceneAction, u16); 8] = [
    (6, 0xFA11, 0xBEDF, AuthoredSceneAction::Scene6, 0x0000),
    (9, 0xD40E, 0xC191, AuthoredSceneAction::Scene9, 0x1000),
    (3, 0xD9D6, 0xC4F3, AuthoredSceneAction::Scene3, 0x0000),
    (5, 0xB65B, 0xBED3, AuthoredSceneAction::Scene5, 0x0000),
    (7, 0xB65B, 0xBEC2, AuthoredSceneAction::Scene7, 0x0000),
    (4, 0xD48C, 0xBEB4, AuthoredSceneAction::Scene4, 0x0000),
    (25, 0xD490, 0xBEBB, AuthoredSceneAction::Scene25, 0x0000),
    (29, 0xB8D2, 0xBDF9, AuthoredSceneAction::Scene29, 0x0000),
];

/// The entry continuation ($06:846C..84A1) runs the installer only behind
/// the open action gate and the 255 sentinel, then installs the wait.
#[derive(Clone, Copy, PartialEq)]
enum Entry {
    Installer,
    Strategy,
}

fn run_case(count: usize, selection: u8, saved: u8, seed: u16, entry: Entry) {
    let bytes = rom();
    let mut source = Source::new(&bytes, 0);
    let mut native = Native::new(&mut source, count, seed as u8, (seed >> 8) as u8, seed);
    let player = native.owner;
    let prior = PlayerActionState {
        action: None,
        elapsed: seed.rotate_left(3),
        auxiliary_counter: seed.rotate_left(7),
        total_updates: seed ^ 0x5A5A,
    };
    let records = native.world.player_mut(&native.objects, player).unwrap();
    records.action = Some(prior);
    records.saved_scene_selection = Some(saved);
    native.world.scene_selection = Some(selection);
    source.bus.write8(0x1D73, selection);
    source.bus.write8(WRAM + SLOT + 0x6AE6, saved);
    for (offset, value) in [
        (0x6C13, 0u16),
        (0x6C16, prior.elapsed),
        (0x6C18, prior.auxiliary_counter),
        (0x6C1A, prior.total_updates),
    ] {
        source.bus.write16(WRAM + SLOT + offset, value);
    }
    source.bus.write8(WRAM + SLOT + 0x6C15, 0);

    let effective = if selection == 22 { saved } else { selection };
    let skipped = matches!(selection, 0xFE | 0xFF);
    let exhausted = !skipped && count == 60;
    let before = native.objects.active_ids().to_vec();
    let result = if entry == Entry::Installer {
        source.run(0x06A8D2, exhausted.then_some(0x008032), 0, OWNER, true);
        scene_install::install(&mut native.objects, &mut native.world, player)
    } else {
        let cue = AuthoredCue::new(seed as u8, (seed >> 8) as u8 & 0x7F, PlayerTarget::Primary);
        native.world.audio.publish_scene_cue(cue);
        source.bus.write16(0x1CE1, seed & 0x7FFF);
        source.bus.write8(0x1CE4, 0xA5);
        native.world.action_gate = Some(ActionGate { code: (seed as u8) | 1 });
        source.bus.write8(0x1D72, (seed as u8) | 1);
        native.objects.get_mut(player).unwrap().base.behavior =
            Behavior::PlayerSceneEntry(SceneEntryPhase::Enter);
        source.bus.write16(u32::from(OWNER) + 0x19, 0x83F1);
        source.bus.write8(u32::from(OWNER) + 0x1B, 6);
        // Stop where the entry falls through into its first wait visit.
        let stop = match (exhausted, selection) {
            (true, _) => Some(0x008032),
            (false, 0xFF) => None,
            (false, _) => Some(0x0684A2),
        };
        source.run(0x06846C, stop, 0, OWNER, true);
        let outcome = player_scene_entry::begin(&mut native.objects, &mut native.world, player);
        let (strategy, restore) = if selection == 0xFF || exhausted {
            (0x83F1, 0)
        } else {
            (0x84A2, seed as u8)
        };
        if !exhausted {
            assert_eq!(source.bus.read16(u32::from(OWNER) + 0x19), strategy);
            assert_eq!(source.bus.read8(0x1CE3), restore);
            assert_eq!(source.bus.read8(0x1CE4), 0xA5);
        }
        match outcome {
            Ok(EntryOutcome::Retry) => {
                assert_eq!(selection, 0xFF);
                assert_eq!(native.world.audio.restore_cue_id(), None);
                assert_eq!(native.objects.active_ids(), before.as_slice());
                assert_eq!(
                    native.objects.get(player).unwrap().base.behavior,
                    Behavior::PlayerSceneEntry(SceneEntryPhase::Enter)
                );
                return;
            }
            Ok(EntryOutcome::Waiting(created)) => {
                assert_eq!(native.world.audio.restore_cue_id(), Some(seed as u8));
                assert_eq!(
                    native.objects.get(player).unwrap().base.behavior,
                    Behavior::PlayerSceneEntry(SceneEntryPhase::Wait)
                );
                Ok(created)
            }
            Ok(EntryOutcome::GateClosed) => panic!("the action gate is open in this fixture"),
            Err(SceneEntryError::Install(error)) => Err(error),
            Err(error) => panic!("{error:?}"),
        }
    };
    let context = format!("count {count} selection {selection} saved {saved} seed {seed:04X}");

    assert_eq!(
        source.bus.read8(0x1D73),
        native.world.scene_selection.unwrap(),
        "{context}"
    );
    assert_eq!(
        native.world.scene_selection,
        Some(if skipped { selection } else { effective })
    );
    if exhausted {
        assert_eq!(
            result,
            Err(SceneInstallError::ObjectPoolExhausted),
            "{context}"
        );
        assert_eq!(native.objects.active_ids(), before.as_slice());
        return;
    }
    native.compare_pool(&source);
    let created = match result {
        Ok(None) => {
            assert!(skipped, "{context}");
            assert_eq!(native.objects.active_ids(), before.as_slice());
            return;
        }
        Ok(Some(created)) => {
            assert!(SCENES.iter().any(|scene| scene.0 == effective), "{context}");
            created
        }
        Err(SceneInstallError::UnsupportedScene {
            selection: s,
            table_index,
            actor,
        }) => {
            assert_eq!(s, effective, "{context}");
            assert_eq!(table_index, if effective < 30 { effective } else { 0 });
            assert!(SCENES.iter().all(|scene| scene.0 != table_index));
            // Ownership stays with the faulted scene; nothing was assumed.
            assert_eq!(
                native.world.player(&native.objects, player).unwrap().action,
                Some(prior)
            );
            assert!(native.objects.get(actor).is_some());
            return;
        }
        Err(error) => panic!("{context}: {error:?}"),
    };
    let base = u32::from(address(Some(created)));
    let actor = native.objects.get(created).unwrap();
    assert_eq!(actor.base.behavior, Behavior::FollowPath);
    assert_eq!(source.bus.read16(base + 0x19), 0x7E1E);
    assert_eq!(source.bus.read8(base + 0x1B), 0x7F);
    let (_, path_root, script, scene_action, seed) =
        *SCENES.iter().find(|scene| scene.0 == effective).unwrap();
    assert_eq!(source.bus.read16(base + 0x2B), path_root);
    assert_eq!(
        actor.base.path,
        Some(match effective {
            9 => sf2_game::authored_paths::SCENE_NINE,
            3 => sf2_game::authored_paths::SCENE_THREE,
            4 => sf2_game::authored_paths::SCENE_FOUR,
            25 => sf2_game::authored_paths::SCENE_TWENTY_FIVE,
            5 | 7 => sf2_game::authored_paths::SCENE_FIVE,
            6 => sf2_game::authored_paths::SCENE_SIX,
            29 => sf2_game::authored_paths::SCENE_TWENTY_NINE,
            _ => unreachable!(),
        })
    );
    super::special_exit_tests::compare_actor(&source, &native, created, 0);
    let action = native
        .world
        .player(&native.objects, player)
        .unwrap()
        .action
        .unwrap();
    assert_eq!(
        (
            source.bus.read16(WRAM + SLOT + 0x6C13),
            source.bus.read8(WRAM + SLOT + 0x6C15)
        ),
        (script, 0x0D)
    );
    assert_eq!(action.action, Some(PlayerAction::Scene(scene_action)));
    assert_eq!(source.bus.read16(WRAM + SLOT + 0x6C16), action.elapsed);
    assert_eq!(
        source.bus.read16(WRAM + SLOT + 0x6C18),
        action.auxiliary_counter
    );
    assert_eq!(action.auxiliary_counter, seed);
    assert_eq!(
        source.bus.read16(WRAM + SLOT + 0x6C1A),
        action.total_updates
    );
    assert_eq!(action.total_updates, prior.total_updates);
}

#[test]
fn indexed_scene_entry_matches_original_selections_substitution_and_pool_pressure() {
    let mut selections: Vec<(u8, u8)> = (0..=40).map(|s| (s, 0)).collect();
    selections.extend([(0xFE, 9), (0xFF, 9), (0x80, 9)]);
    // Selector 22 substitutes the saved scene without re-testing sentinels.
    selections.extend([9, 3, 22, 0xFE, 0xFF, 40, 0].map(|saved| (22, saved)));
    for count in [3, 30, 59, 60] {
        for &(selection, saved) in &selections {
            for seed in [0x0000, 0xEF73, 0x1234] {
                run_case(count, selection, saved, seed, Entry::Installer);
                run_case(count, selection, saved, seed, Entry::Strategy);
            }
        }
    }
}

#[test]
fn closed_gate_entry_marks_the_fixed_view_before_its_unported_exit() {
    let bytes = rom();
    for prior in [0x00, 0x5F, 0xDF] {
        let mut source = Source::new(&bytes, 0);
        let mut native = Native::new(&mut source, 3, 7, 9, 0x2468);
        let view = native.objects.active_ids()[1];
        native.world.fixed_players[0] = Some(view);
        native
            .objects
            .get_mut(view)
            .unwrap()
            .extension
            .path_state
            .motion
            .carry_selected_player = prior & 0x20 != 0;
        source.bus.write8(0x033F + 0x21, prior);
        native.world.action_gate = Some(ActionGate { code: 0 });
        source.bus.write8(0x1D72, 0);
        native.world.scene_selection = Some(9);
        source.bus.write8(0x1D73, 9);
        let before = native.objects.active_ids().to_vec();
        source.run(0x06846C, Some(0x068525), 0, OWNER, true);
        assert_eq!(
            player_scene_entry::begin(&mut native.objects, &mut native.world, native.owner),
            Ok(EntryOutcome::GateClosed)
        );
        assert_eq!(source.bus.read8(0x033F + 0x21), prior | 0x20);
        let motion = native.objects.get(view).unwrap().extension.path_state.motion;
        assert!(motion.carry_selected_player);
        assert_eq!(native.objects.active_ids(), before.as_slice());
    }
}
