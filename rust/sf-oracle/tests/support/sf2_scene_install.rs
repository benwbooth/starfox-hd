//! Indexed scene entry (`$06:A8D2..A987`) against the original routine:
//! sentinels, saved-scene substitution, allocation, pool pressure and the
//! table-driven action/path record. Unsupported entries must fault after
//! the same allocation the source performs.
use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::player_action::{AuthoredSceneAction, PlayerAction, PlayerActionState};
use sf2_game::scene_install::{self, SceneInstallError};
use sf2_game::Behavior;

const SCENE_NINE_PATH: u16 = 0xD40E;
const SCENE_NINE_ACTION: (u16, u8) = (0xC191, 0x0D);

fn run_case(count: usize, selection: u8, saved: u8, seed: u16) {
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
    source.run(0x06A8D2, exhausted.then_some(0x008032), 0, OWNER, true);
    let before = native.objects.active_ids().to_vec();
    let result = scene_install::install(&mut native.objects, &mut native.world, player);
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
            assert_eq!(effective, 9, "{context}");
            created
        }
        Err(SceneInstallError::UnsupportedScene {
            selection: s,
            table_index,
            actor,
        }) => {
            assert_eq!(s, effective, "{context}");
            assert_eq!(table_index, if effective < 30 { effective } else { 0 });
            assert_ne!(table_index, 9);
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
    assert_eq!(source.bus.read16(base + 0x2B), SCENE_NINE_PATH);
    assert_eq!(actor.base.path, Some(sf2_game::authored_paths::SCENE_NINE));
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
        SCENE_NINE_ACTION
    );
    assert_eq!(
        action.action,
        Some(PlayerAction::Scene(AuthoredSceneAction::Scene9))
    );
    assert_eq!(source.bus.read16(WRAM + SLOT + 0x6C16), action.elapsed);
    assert_eq!(
        source.bus.read16(WRAM + SLOT + 0x6C18),
        action.auxiliary_counter
    );
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
                run_case(count, selection, saved, seed);
            }
        }
    }
}
