//! The Star Wolf interception (encounter location 7, layout 0A), entered on
//! the retail strategic map by a scripted controller route, composed
//! natively from the retail stage-start state and compared with the complete
//! retail machine. The retail world is read once, as the stage's scene player
//! is initialized (after the stage loop's own setup, before any scene frame);
//! afterwards both engines run independently.
use super::attract_scene_tests::{
    byte, compare, compare_initialized_player, compare_player_records, retail_list, vector, word, Callbacks, EPOCH,
    INITIALIZER, INITIALIZER_RETURN, POOL, RANDOM_DRAW, RANDOM_RETURN, REFRESH, RESEEDS, STRIDE,
};
use sf2_game::scene_runner::EntropyRefresh;
use sf2_game::scene_path_world::ScenePathWorld;
use sf2_game::scene_runner::SceneRunner;
use sf2_game::strategy_schedule::StrategySchedule;
use sf2_game::{Behavior, Object, ObjectKind, ObjectSpawnDefaults, ObjectStore, ShapeId};
use sf_oracle::RetailMachine;

/// The mission-stage launch ($03:B90E) and the common stage loop.
const MISSION_LAUNCH: u32 = 0x03B90E;
const STAGE_LOOP: u32 = 0x03BE74;
const START: u16 = 0x1000;
/// The strategy pass's per-actor dispatch calls ($7F:3596), diagnostics only.
const ACTOR_VISITS: [u32; 2] = [0x7F3526, 0x7F3572];
/// The radio panel service ($0A:CD66) runs in the render-timed frame
/// handler ($03:819C); its talking-frame draw ($0A:CD92) is a render-timed
/// RNG event like the entropy refresh.
const RADIO_PANEL_DRAW: u32 = 0x0ACD92;
const B: u16 = 0x8000;
const RIGHT: u16 = 0x0100;
const UP: u16 = 0x0800;

/// Title, attract and pilot selection by Start presses; then the map cursor
/// to the cyan planet's lane and B. Leon intercepts the ship on the way.
fn navigate(m: &mut RetailMachine) {
    m.tick_video_frames(0, 600).unwrap();
    for _ in 0..20 {
        m.tick_video_frames(START, 6).unwrap();
        m.tick_video_frames(0, 194).unwrap();
    }
    m.tick_video_frames(0, 60).unwrap();
    m.tick_video_frames(RIGHT, 41).unwrap();
    m.tick_video_frames(UP, 20).unwrap();
    m.tick_video_frames(B, 6).unwrap();
    assert!(m.tick_until_cpu_execution(0, MISSION_LAUNCH, 2000).unwrap(), "no encounter launched");
    assert_eq!((byte(m, 0x1BB5), byte(m, 0x1BA5)), (7, 0x0A), "not the Star Wolf interception");
    assert!(m.tick_until_cpu_execution(0, STAGE_LOOP, 200).unwrap());
    assert!(m.tick_until_cpu_execution(0, INITIALIZER, 600).unwrap());
}

/// The campaign state the stage loop starts from, read from retail RAM.
fn stage_world(m: &RetailMachine) -> ScenePathWorld {
    let mut world = sf2_game::attract_stage::boot_world();
    let mode = sf2_game::view_transition::ViewTransitionMode { flags: word(m, 0x1B84) };
    world.view_transition_mode = Some(mode);
    world.spawn_defaults = Some(mode.spawn_defaults(ObjectSpawnDefaults {
        group: byte(m, 0x190E),
        run_when_paused: false,
    }));
    world.published_score = Some(sf2_game::path_score::PlayerScore::from_parts(
        word(m, 0xD816),
        byte(m, 0xD818),
    ));
    world.active_shield_capacity = Some(byte(m, 0x1DD5));
    world.scene.active_shield = Some(byte(m, 0x1DD1));
    world.scene.active_pilot = Some(byte(m, 0x1E14));
    world.scene.wingmate_pilot = Some(byte(m, 0x1E70));
    world.scene.encounter_location = Some(word(m, 0x1BB5));
    world.scene.encounter_layout = Some(byte(m, 0x1BA5));
    world.scene.node_presentation_variant = Some(byte(m, 0x1E09));
    world.scene.gsu_text_active = Some(byte(m, 0xD757));
    world.scene.map_region = Some(byte(m, 0xDB5B));
    world.campaign_phase = Some(byte(m, 0x1BE0));
    world.reflect_all_contacts = Some(byte(m, 0x1AA6) & 0x02 != 0);
    world.cinematic_signals = Some(sf2_game::cinematic_exit::CinematicSignals {
        exit_requested: word(m, 0x1B96) & 0x10 != 0,
        skip_ready: word(m, 0x1B96) & 0x20 != 0,
    });
    world.reticle_inhibited = Some(word(m, 0x1B96) & 0x0100 != 0);
    world.scene_gate_flags = Some(sf2_game::scene_path_world::SceneGateFlags {
        hud_held: word(m, 0x1B96) & 0x40 != 0,
        hud_ready: word(m, 0x1B96) & 0x04 != 0,
    });
    world.scene_display_flags = Some(word(m, 0x1B9C));
    world.reticle_enabled = Some(byte(m, 0x1E2F) & 0x80 != 0);
    world.scene_events = Some(sf2_game::path_scene_state::SceneEventFlags { bits: word(m, 0x1B88) });
    world.stage_layout = Some(word(m, 0x1916));
    world.scenario_flags = Some(word(m, 0xE087));
    world.map_target_position = Some((word(m, 0x1DF3) as i16, word(m, 0x1DF5) as i16));
    // The map publishes the ship's ground position and heading ($04:B0D8).
    world.map_placement = sf2_game::map_effects::MapPlacement {
        x: Some(word(m, 0x1DE4) as i16),
        y: Some(word(m, 0x1DE6) as i16),
        z: Some(word(m, 0x1DE8) as i16),
        heading: Some(byte(m, 0x1DEA)),
    };
    // Path-shared state: script-owned words and bytes, read whole.
    world.deferred_message = Some(sf2_game::path_radio::DeferredMessage { number: word(m, 0xD790) });
    world.radio_event = Some(sf2_game::path_radio::RadioEvent { number: word(m, 0x1E84) });
    world.guidance = Some(sf2_game::path_program::GuidanceHistory { flags: word(m, 0xD792) });
    world.pickup_history = Some(sf2_game::path_program::PickupHistory { collected_mask: word(m, 0xD78E) });
    world.active_node_flags = Some(sf2_game::path_scene_state::ActiveNodeFlags { bits: word(m, 0xD7F6) });
    world.objective_completion = Some(sf2_game::path_scene_state::ObjectiveCompletion { bits: word(m, 0xD79F) });
    world.encounter_signals = Some(sf2_game::path_program::EncounterSignals { raised: word(m, 0xD77D) });
    world.path_latches = Some(sf2_game::path_scene_state::PathLatches { raised: word(m, 0xCF33) });
    world.sound_bank_request = Some(sf2_game::path_scene_state::SoundBankRequest { selection: byte(m, 0x1BBB) });
    world.countdown = Some(sf2_game::path_countdown::PathCountdown { remaining: byte(m, 0xD786) });
    world.projectile_trigger = Some(sf2_game::path_program::ProjectileTrigger { activation: byte(m, 0x1E59) });
    world.projectile_flight_override = Some(sf2_game::path_shots::ProjectileFlightOverride { code: byte(m, 0xD7D8) });
    world.encounter_result = Some(sf2_game::path_program::EncounterResult { word: word(m, 0xD79D) });
    world.slot_words = Some(sf2_game::path_program::PathSlotWords {
        words: [word(m, 0xD7D9), word(m, 0xD7DB), word(m, 0xD7DD), word(m, 0xD7DF)],
    });
    world.difficulty_tallies = Some(sf2_game::path_program::DifficultyTallies {
        counts: [byte(m, 0xD7E1), byte(m, 0xD7E2), byte(m, 0xD7E3)],
    });
    world.coordination = Some(sf2_game::path_scene_state::EncounterCoordination {
        boundary_corrections: byte(m, 0xD73F),
        progress: byte(m, 0xD787),
        secondary_progress: byte(m, 0xD788),
        completed_parts: byte(m, 0xD789),
        active_messages: byte(m, 0xD78A),
        handshake: byte(m, 0xD78B),
        retired_actors: byte(m, 0xD78D),
        phase: byte(m, 0xD79A),
        transition_ready: byte(m, 0xD7D5),
    });
    {
        use sf2_game::path_countdown::{PathScratchBytes, ScratchCell};
        let mut scratch = PathScratchBytes::default();
        for (cell, address) in [
            (ScratchCell::D766, 0xD766),
            (ScratchCell::D77B, 0xD77B),
            (ScratchCell::D77C, 0xD77C),
            (ScratchCell::D79C, 0xD79C),
            (ScratchCell::D7D2, 0xD7D2),
            (ScratchCell::D7D6, 0xD7D6),
            (ScratchCell::D7EB, 0xD7EB),
            (ScratchCell::RetainedRivalHealth(0), 0xD7E8),
            (ScratchCell::RetainedRivalHealth(1), 0xD7E9),
            (ScratchCell::RetainedRivalHealth(2), 0xD7EA),
        ] {
            scratch.set(cell, byte(m, address));
        }
        world.scratch_bytes = Some(scratch);
    }
    world.campaign = Some(sf2_game::path_program::CampaignPathInputs {
        difficulty: match byte(m, 0xD7F2) {
            0 => sf2_game::Difficulty::Normal,
            1 => sf2_game::Difficulty::Hard,
            2 => sf2_game::Difficulty::Expert,
            other => panic!("difficulty code {other}"),
        },
        encounter_variant: byte(m, 0x1C06),
        secondary_variant: byte(m, 0x1C07),
    });
    world.radio = Some((
        sf2_game::path_radio::RadioRequest {
            message: sf2_game::path_radio::MessageIndex::from_authored_number(
                byte(m, 0xCF31).wrapping_add(1),
            ),
            pending: byte(m, 0xCF32) != 0,
            panel_y: word(m, 0xD744),
            top_placement: byte(m, 0xD759) != 0,
        },
        sf2_game::path_radio::RadioLayout {
            compact_panel: byte(m, 0xD775) != 0,
            tracked_screen_y: byte(m, 0x1E31),
        },
    ));
    world.objective_counts = Some(sf2_game::path_scene_state::EncounterObjectiveCounts {
        remaining_word: word(m, 0xD7F4),
        node_record: byte(m, 0xD7A1),
        recorded_completions: word(m, 0xD7E4),
        signaled_completions: word(m, 0xD7E6),
    });
    world.scene_transition = Some(sf2_game::path_scene_state::SceneTransitionControl { phase_word: word(m, 0x1B78) });
    world.interception_active = Some(word(m, 0x1B8A) & 0x20 != 0);
    world.interception_music_ready = Some(byte(m, 0x1DDE) != 0);
    world.encounter_timer_steps = Some(word(m, 0x1C0A));
    world.handoff.as_mut().unwrap().player_flags = byte(m, 0x1D74);
    world.node_exit = sf2_game::player_node_exit::NodeExitState {
        presentation_flags: Some(byte(m, 0x1E08)),
        completion_code: Some(byte(m, 0x1E17)),
    };
    world.player_service_flags =
        Some(sf2_game::player_action::PlayerServiceFlags::from_bits(byte(m, 0x1E0D)));
    world.scene.player_view_control = Some(byte(m, 0x1DE0));
    world.scene.player_walker_form = Some(byte(m, 0x1DCE) != 0);
    world.player_view_options_enabled = Some(byte(m, 0x1DE1) & 0x80 != 0);
    world.surface_mode = Some(sf2_game::collision_surface::SurfaceMode { flags: byte(m, 0x1B4D) });
    world.player_input_settings = Some(sf2_game::player_input::PlayerInputSettings {
        flight_style: byte(m, 0x1DCF),
        button_layout: byte(m, 0x1DD0),
    });
    // The script-owned boss bar (D773/D775); its label (D777) starts unset.
    assert_eq!(word(m, 0xD777), 0, "health display label at stage start");
    world.health_display = Some(sf2_game::path_scene_state::EncounterHealthDisplay {
        current: byte(m, 0xD773),
        maximum: byte(m, 0xD775),
        label: None,
    });
    // Campaign equipment publications (1DD2..1DD4).
    world.active_consumables = Some(sf2_game::player_visit::PublishedConsumables {
        packed_count: byte(m, 0x1DD2),
        kind: byte(m, 0x1DD3),
    });
    world.scene.active_weapon_level = Some(byte(m, 0x1DD4));
    // The scene cue the campaign last published ($1CE1/2): routing bit 80.
    let route = byte(m, 0x1CE2);
    world.audio.resume_retained_scene_cue(Some(sf2_game::path_sound::AuthoredCue::new(
        byte(m, 0x1CE1),
        route & 0x7F,
        if route & 0x80 == 0 {
            sf2_game::path_control::PlayerTarget::Primary
        } else {
            sf2_game::path_control::PlayerTarget::Secondary
        },
    )));
    world.map_records = Some(sf2_game::map_streaming::MapRecordStore::new());
    world.map_regions = Some(Default::default());
    world.region_groups = Some(sf2_game::map_streaming::RegionGroups {
        current: byte(m, 0x190E),
        previous: byte(m, 0x190F),
    });
    world
}

#[test]
fn star_wolf_interception_runs_natively_like_the_retail_machine() {
    let mut m = RetailMachine::new(super::rom());
    navigate(&mut m);
    let mut world = stage_world(&m);
    let map_catalog = sf2_game::authored_maps::catalog().unwrap();
    let mut presentation = sf2_game::map_effects::MapPresentation {
        display_ready: Some(true),
        load_table_idle: Some(true),
        ..Default::default()
    };

    // The stage's prologue map spawns the scene player into an empty list.
    let mut objects = ObjectStore::new();
    let mut prologue = sf2_game::scene_map::SceneMap::new(
        &map_catalog,
        sf2_game::authored_maps::SCENE_PLAYER_PROLOGUE,
    )
    .unwrap();
    let report = sf2_game::map_effects::visit(
        &mut prologue,
        &map_catalog,
        &mut objects,
        &mut world,
        None,
        &mut presentation,
        64,
    )
    .unwrap();
    assert_eq!(report.stop, sf2_game::scene_map::MapStop::Stopped);
    let player = prologue.current_object().unwrap();
    assert_eq!(retail_list(&m), vec![POOL, POOL + STRIDE]);
    assert_eq!(vector(&m, POOL), objects.get(player).unwrap().base.position);

    let idle = sf2_game::attract_stage::excluded_proxy(Some(vector(&m, POOL + STRIDE)));
    let idle = objects.allocate_after(Some(player), idle).unwrap();
    let mut view = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Unassigned);
    view.base.position = vector(&m, 0x033F);
    view.base.flags.strategy_suspended = true;
    let view = objects.allocate_after(Some(idle), view).unwrap();
    world.primary_player = Some(player);
    world.fixed_players[0] = Some(view);
    world.excluded_actor = Some(idle);
    // The same reserved actor (14D6) is the flight services' motion/aim proxy.
    world.weapons.as_mut().unwrap().fallback = Some(idle);
    let mut runner = SceneRunner::new(objects, world, Callbacks);
    runner.schedule = StrategySchedule::resume(runner.world.strategy_clock);
    assert_eq!(word(&m, 0x14D6), POOL + STRIDE);
    runner.execution.controls.excluded_actor = Some(idle);
    sf2_game::player_scene_init::initialize(
        &mut runner.objects,
        &mut runner.world,
        &mut runner.execution.paths.runtime,
        player,
        sf2_game::hit_response::HitSide::Primary,
    )
    .unwrap();
    assert!(m.tick_until_cpu_execution(0, INITIALIZER_RETURN, 1).unwrap());
    compare_initialized_player(&m, &runner, player, idle);

    // The interception map runs after the initializer's pass, to its park.
    let mut scene_map =
        sf2_game::scene_map::SceneMap::new(&map_catalog, sf2_game::authored_maps::STAR_WOLF_INTERCEPTION)
            .unwrap();
    let report = sf2_game::map_effects::visit(
        &mut scene_map,
        &map_catalog,
        &mut runner.objects,
        &mut runner.world,
        Some(&mut runner.execution.paths.runtime.resources),
        &mut presentation,
        256,
    )
    .unwrap();
    assert_eq!(report.stop, sf2_game::scene_map::MapStop::Yielded(0x1388));
    // Paths redirect and restore the scene's map through its owner.
    runner.world.map = Some(scene_map);
    runner.world.map_presentation = presentation;
    sf2_game::attract_stage::start_frame_loop(&mut runner.world);
    let catalog = sf2_game::authored_paths::catalog();
    assert!(m.tick_until_cpu_execution(0, EPOCH, 60).unwrap());
    let clock = runner.world.strategy_clock;
    runner.schedule = StrategySchedule::resume(clock);
    runner.prepare_frame(&catalog).unwrap();
    compare(&m, &runner, view, 0);
    let mut matched = 0;
    // The scene's action clears the gate at its update 110; the player then
    // leaves for its flight strategy in the same visit and duels the rival
    // until it is shot down in epoch 1052.
    for epoch in 0..1052u32 {
        let mut watched = vec![REFRESH, RANDOM_DRAW, RANDOM_RETURN, ACTOR_VISITS[0], ACTOR_VISITS[1], RADIO_PANEL_DRAW];
        watched.extend(RESEEDS);
        m.watch_cpu_execution(&watched);
        assert!(m.tick_until_cpu_execution(0, EPOCH + 1, 60).unwrap());
        assert!(m.tick_until_cpu_execution(0, EPOCH, 120).unwrap(), "epoch {epoch}: retail stalled");
        let hits = m.take_cpu_execution_watch_hits();
        let mut draws = 0u16;
        let mut refreshes = Vec::new();
        let mut in_refresh = false;
        for &hit in &hits {
            match hit {
                REFRESH | RADIO_PANEL_DRAW => {
                    refreshes.push(draws);
                    in_refresh = true;
                }
                RANDOM_DRAW if in_refresh => in_refresh = false,
                RANDOM_DRAW => draws += 1,
                hit if RESEEDS.contains(&hit) => draws += 1,
                _ => {}
            }
        }
        let strategy = word(&m, POOL + 0x19);
        let random_before = runner.world.random.bytes();
        runner
            .run_epoch(&catalog, EntropyRefresh::BeforeDraws(&refreshes))
            .and_then(|()| runner.finish_frame(&catalog))
            .and_then(|()| runner.prepare_frame(&catalog))
            .unwrap_or_else(|error| {
                let actors: Vec<String> = runner
                    .objects
                    .active_ids()
                    .iter()
                    .map(|&id| {
                        let a = runner.objects.get(id).unwrap();
                        format!(
                            "{}:{:?}@{:?} {:?} parent {:?} next {:?} pos {:?} rot {:?}",
                            id.index(),
                            a.base.shape,
                            a.base.path,
                            a.base.behavior,
                            a.base.attachment,
                            a.base.attachment_next,
                            a.base.position,
                            [a.base.pitch, a.base.yaw, a.base.roll]
                        )
                    })
                    .collect();
                let retail: Vec<String> = retail_list(&m)
                    .iter()
                    .map(|&b| {
                        let words: Vec<String> =
                            (0..0x30u16).step_by(2).map(|o| format!("{:04X}", word(&m, b + o))).collect();
                        format!("{b:04X}: {}", words.join(" "))
                    })
                    .collect();
                panic!("epoch {epoch}: {error:?}\nnative actors {actors:#?}\nretail {retail:#?}")
            });
        let retail_random = [byte(&m, 0xE0), byte(&m, 0xE1), byte(&m, 0xE2), byte(&m, 0xE3)];
        if runner.world.random.bytes() != retail_random {
            // Diagnose: how many draws each side made from the shared start.
            let steps = |target: [u8; 4]| {
                let mut state = sf2_game::RandomState::new(random_before);
                (0..400).find(|_| {
                    let hit = state.bytes() == target;
                    state.next_byte();
                    hit
                })
            };
            let mut visit = 0;
            let mut trace = Vec::new();
            for &hit in &hits {
                if ACTOR_VISITS.contains(&hit) {
                    visit += 1;
                } else if hit == RANDOM_DRAW {
                    trace.push(visit);
                }
            }
            eprintln!("epoch {epoch}: retail draws at actor visits {trace:?} (list {:04X?})", retail_list(&m));
            eprintln!(
                "epoch {epoch}: retail draws {draws} refreshes {refreshes:?}; steps to native {:?}, to retail {:?}",
                steps(runner.world.random.bytes()),
                steps(retail_random)
            );
        }
        compare(&m, &runner, view, epoch);
        compare_player_records(
            &m,
            &runner,
            player,
            idle,
            &format!("epoch {epoch} (retail strategy {strategy:04X})"),
            true,
        );
        matched = epoch + 1;
    }
    eprintln!("star wolf interception matched for {matched} epochs");
    assert_eq!(matched, 1052);
    // The player's registered death routine ($06:F3A4) is the frontier: it
    // faults rather than falling back to common destruction.
    let error = runner
        .run_epoch(&catalog, EntropyRefresh::AfterPass)
        .expect_err("the scene player's death routine is not ported");
    assert!(
        format!("{error:?}").contains("UnportedDeathHandler(ScenePlayer)"),
        "{error:?}"
    );
}
