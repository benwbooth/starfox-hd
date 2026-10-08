//! The retail attract loop's first indexed scene, composed natively by
//! `SceneRunner` and compared with the complete retail machine at every
//! strategy epoch. Only the starting state is read from the retail machine
//! (the moment the scene player is first visited); afterwards both engines
//! run independently and no value is copied from one to the other.
use super::motion_reset_tests::Reader;
use super::{Source, WRAM};
use sf2_game::authored_paths;
use sf2_game::cinematic_exit::CinematicSignals;
use sf2_game::path_program::{ActionGate, EncounterSignals};
use sf2_game::player_action::{PlayerServiceFlags, ScenePalette};
use sf2_game::player_scene_entry::SceneEntryPhase;
use sf2_game::scene_runner::{EntropyRefresh, SceneRunner};
use sf2_game::scene_strategy::{SceneActors, SceneCallbacks};
use sf2_game::strategy_schedule::{StrategyCompletion, StrategySchedule};
use sf2_game::view_transition::ViewTransitionMode;
use sf2_game::{
    Angle, Behavior, Buttons, InputState, Object, ObjectId, ObjectKind, ObjectSpawnDefaults,
    ObjectStore, RandomState, ShapeId, Vector3,
};
use sf_oracle::RetailMachine;

const RETAIL: u32 = 0x7E0000;
const VIEW: u16 = 0x033F;
const POOL: u16 = 0x03BD;
const STRIDE: u16 = 0x3F;
const EPOCH: u32 = 0x7F34E7;
const SCENE_PLAYER_ENTRY: u16 = 0x845C;
const REFRESH: u32 = 0x7F058F;
const RANDOM_DRAW: u32 = 0x7F7BD4;
const RANDOM_RETURN: u32 = 0x7F7BE7;

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, Self::Error> {
        Err("unported assigned strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        Ok(None)
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        Err("unported map continuation")
    }
}

fn byte(m: &RetailMachine, address: u16) -> u8 {
    m.peek8(RETAIL + u32::from(address))
}

fn word(m: &RetailMachine, address: u16) -> u16 {
    m.peek16(RETAIL + u32::from(address))
}

fn vector(m: &RetailMachine, base: u16) -> Vector3 {
    Vector3 {
        x: word(m, base + 0x0C) as i16,
        y: word(m, base + 0x0E) as i16,
        z: word(m, base + 0x10) as i16,
    }
}

fn retail_list(m: &RetailMachine) -> Vec<u16> {
    let mut list = Vec::new();
    let mut cursor = word(m, 0x12A8);
    while cursor != 0 {
        list.push(cursor);
        cursor = word(m, cursor);
        assert!(list.len() <= 60, "retail active list must terminate");
    }
    list
}

fn pose(object: &mut Object, m: &RetailMachine, base: u16) {
    object.base.position = vector(m, base);
    object.base.pitch = Angle::from_units(byte(m, base + 0x12));
    object.base.yaw = Angle::from_units(byte(m, base + 0x14));
    object.base.roll = Angle::from_units(byte(m, base + 0x16));
}

/// Run the retail machine to the first epoch of the attract's scene player.
fn boot_to_scene() -> RetailMachine {
    let mut machine = RetailMachine::new(super::rom());
    for _ in 0..400 {
        machine.tick_video_frames(0, 1).unwrap();
        if word(&machine, POOL + 0x19) == SCENE_PLAYER_ENTRY {
            assert!(machine.tick_until_cpu_execution(0, EPOCH, 60).unwrap());
            return machine;
        }
    }
    panic!("attract scene player never installed");
}

#[test]
fn attract_scene_six_runs_natively_like_the_retail_machine() {
    let mut m = boot_to_scene();
    // Copy WRAM so the existing typed record reader can decode the player.
    let mut source = Source::new(&super::rom(), 0);
    for offset in 0..0x20000u32 {
        source.bus.write8(WRAM + offset, m.peek8(RETAIL + offset));
    }
    assert_eq!(retail_list(&m), vec![POOL, POOL + STRIDE]);

    // Allocation inserts at the list head: build the list back to front.
    // The fixed view lives outside the source pool; it is listed last here
    // so insertion after the list head matches the source order.
    let mut objects = ObjectStore::new();
    let mut view = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Unassigned);
    pose(&mut view, &m, VIEW);
    // Outside the source pool the view is never scheduled.
    view.base.flags.strategy_suspended = true;
    let view = objects.allocate(view).unwrap();
    let mut idle = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Unassigned);
    pose(&mut idle, &m, POOL + STRIDE);
    idle.base.hit_points = byte(&m, POOL + STRIDE + 0x2D);
    idle.base.flags.strategy_suspended = byte(&m, POOL + STRIDE + 0x26) & 0x40 != 0;
    idle.base.contacts.first_strategy_visit = byte(&m, POOL + STRIDE + 0x31) & 4 != 0;
    let idle = objects.allocate(idle).unwrap();
    let mut player = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerSceneEntry(SceneEntryPhase::ClearLaunchCounts));
    pose(&mut player, &m, POOL);
    player.base.hit_points = byte(&m, POOL + 0x2D);
    player.base.attack_power = byte(&m, POOL + 0x2E);
    player.base.flags.collision_disabled = byte(&m, POOL + 0x21) & 1 != 0;
    let player = objects.allocate(player).unwrap();
    assert_eq!(objects.active_ids(), &[player, idle, view]);

    let mut world = sf2_game::scene_path_world::ScenePathWorld::new(RandomState::new([
        byte(&m, 0xE0),
        byte(&m, 0xE1),
        byte(&m, 0xE2),
        byte(&m, 0xE3),
    ]));
    let slot = u32::from(word(&m, POOL + 0x2B));
    let records = Reader { source: &source, slot, other: idle }.records();
    world.bind_player(&objects, player, records).unwrap();
    world.primary_player = Some(player);
    world.fixed_players[0] = Some(view);
    let mode = ViewTransitionMode { flags: word(&m, 0x1B84) };
    world.view_transition_mode = Some(mode);
    world.spawn_defaults = Some(mode.spawn_defaults(ObjectSpawnDefaults {
        group: byte(&m, 0x190E),
        run_when_paused: false,
    }));
    world.scene_selection = Some(byte(&m, 0x1D73));
    world.action_gate = Some(ActionGate { code: byte(&m, 0x1D72) });
    world.campaign_phase = Some(byte(&m, 0x1BE0));
    let flags = word(&m, 0x1B96);
    world.cinematic_signals = Some(CinematicSignals {
        exit_requested: flags & 0x10 != 0,
        skip_ready: flags & 0x20 != 0,
    });
    world.reticle_inhibited = Some(flags & 0x100 != 0);
    world.reflect_all_contacts = Some(byte(&m, 0x1AA6) & 2 != 0);
    world.palette = Some(ScenePalette {
        colors: std::array::from_fn(|i| word(&m, 0xEFE5 + i as u16 * 2)),
        saved_colors: std::array::from_fn(|i| word(&m, 0xF2E5 + i as u16 * 2)),
    });
    world.palette_refresh_requested = Some(byte(&m, 0x1E58) & 0x80 != 0);
    world.player_service_flags = Some(PlayerServiceFlags::from_bits(byte(&m, 0x1E0D)));
    world.scene.player_configuration = Some(byte(&m, 0x1DE2));
    world.controller_inputs = [0u16, 2].map(|side| {
        Some(InputState {
            held: Buttons::from_bits(word(&m, 0x1292 + side)),
            pressed: Buttons::from_bits(word(&m, 0x1296 + side)),
        })
    });
    world.encounter_signals = Some(EncounterSignals { raised: word(&m, 0xD77D) });
    world.camera_height_limits = Some((word(&m, 0x1E32) as i16, word(&m, 0x1E34) as i16));
    world.camera_projection_offset = Some(word(&m, 0x1E52) as i16);
    world.weapons = Some(Default::default());
    world.published_motion = Some(sf2_game::path_motion::PublishedPlayerMotion {
        position: Vector3 {
            x: word(&m, 0xD7EC) as i16,
            y: word(&m, 0xD7EE) as i16,
            z: word(&m, 0xD7F0) as i16,
        },
        delta: Vector3 {
            x: word(&m, 0x1E1C) as i16,
            y: word(&m, 0x1E1E) as i16,
            z: word(&m, 0x1E20) as i16,
        },
    });
    world.engine_sound_control =
        Some(sf2_game::player_engine_sound::EngineSoundControl::from_bits(byte(&m, 0x1CE5)));
    world.linked_effect_activity =
        Some(sf2_game::path_protection::LinkedEffectActivity { recent_spawn: byte(&m, 0x1DDF) });
    assert_eq!(word(&m, 0x1DFF), 0, "no tracked camera actor at scene entry");
    world.camera_tracking = Some(Default::default());
    world.scene.active_pilot = Some(byte(&m, 0x1E14));
    world.scene.wingmate_pilot = Some(byte(&m, 0x1E70));
    world.published_camera_projection = Some(word(&m, 0x1E3C) as i16);
    world.scene.active_shield = Some(byte(&m, 0x1DD1));
    world.scene.encounter_location = Some(word(&m, 0x1BB5));
    world.strategy_clock = word(&m, 0xC4);

    let mut runner = SceneRunner::new(objects, world, Callbacks);
    runner.schedule = StrategySchedule::resume(word(&m, 0xC4));
    // The strategy pass skips the shared excluded actor (14D6).
    assert_eq!(word(&m, 0x14D6), POOL + STRIDE);
    runner.execution.controls.excluded_actor = Some(idle);
    let runtime = &mut runner.execution.paths.runtime;
    runtime.background_horizontal = Some(word(&m, 0x1E4E) as i16);
    // The player's allocation is owned by the scene's program resources;
    // its records bind to that storage identity, so rebind afterwards.
    let storage = Reader { source: &source, slot, other: idle }.storage();
    let resource = runtime
        .resources
        .allocate_owned(player, 472, sf2_game::program_state::ProgramData::PlayerStorage(storage))
        .unwrap();
    let records = runner.world.player(&runner.objects, player).unwrap().clone();
    runner.objects.get_mut(player).unwrap().base.player_storage = Some(resource);
    runner.world.bind_player(&runner.objects, player, records).unwrap();
    let catalog = authored_paths::catalog();
    let mut ended = None;
    for epoch in 0..2000u32 {
        // The retail epoch runs first so its render-timed entropy refresh can
        // be placed at the same actor-visit boundary natively.
        // Draw entry/return markers alternate, so consecutive draws are not
        // collapsed into one recorded entry.
        m.watch_cpu_execution(&[REFRESH, RANDOM_DRAW, RANDOM_RETURN]);
        assert!(m.tick_until_cpu_execution(0, EPOCH + 1, 60).unwrap());
        if !m.tick_until_cpu_execution(0, EPOCH, 60).unwrap() {
            // The scene handed over: no further strategy epoch follows soon.
            ended = Some(epoch);
            break;
        }
        let hits = m.take_cpu_execution_watch_hits();
        // Each refresh's index among the pass's ordinary draws. A refresh's
        // own draw follows its entry and is not an ordinary draw.
        let mut draws = 0u16;
        let mut refreshes = Vec::new();
        let mut in_refresh = false;
        for &hit in &hits {
            match hit {
                REFRESH => {
                    refreshes.push(draws);
                    in_refresh = true;
                }
                RANDOM_DRAW if in_refresh => in_refresh = false,
                RANDOM_DRAW => draws += 1,
                _ => {}
            }
        }
        let refresh = EntropyRefresh::BeforeDraws(&refreshes);
        // Retail now stands at the next epoch, after its pre-epoch services.
        runner
            .run_epoch(&catalog, refresh)
            .and_then(|()| runner.prepare_frame(&catalog))
            .unwrap_or_else(|error| panic!("epoch {epoch}: {error:?}"));
        compare(&m, &runner, view, epoch);
        if word(&m, POOL + 0x19) != 0x84A2 {
            ended = Some(epoch);
            break;
        }
    }
    let ended = ended.expect("the scene ends within the test bound");
    eprintln!("scene six matched for {ended} epochs");
    assert!(runner.world.cinematic_signals.unwrap().exit_requested, "native exit request");
    assert!(ended > 400);
}

fn compare(m: &RetailMachine, runner: &SceneRunner<Callbacks>, view: ObjectId, epoch: u32) {
    let retail = retail_list(m);
    let native: Vec<ObjectId> = runner
        .objects
        .active_ids()
        .iter()
        .copied()
        .filter(|&id| id != view)
        .collect();
    if native.len() != retail.len() {
        let n: Vec<String> = native
            .iter()
            .map(|&id| {
                let a = runner.objects.get(id).unwrap();
                format!("{}:{:?}@{:?} hp{} {:?} rm{}", id.index(), a.base.shape, a.base.path, a.base.hit_points, a.base.behavior, a.base.flags.remove_after_tick)
            })
            .collect();
        let r: Vec<String> = retail
            .iter()
            .map(|&b| format!("{b:04X}:sh{:04X}:p{:04X}:s{:04X} hp{}", word(m, b + 4), word(m, b + 0x2B), word(m, b + 0x19), byte(m, b + 0x2D)))
            .collect();
        panic!("epoch {epoch}: active count\nnative {n:?}\nretail {r:?}");
    }
    for (&id, &base) in native.iter().zip(&retail) {
        let actor = runner.objects.get(id).unwrap();
        let context = format!("epoch {epoch} actor {} retail {base:04X}", id.index());
        if Some(id) == runner.execution.controls.excluded_actor {
            // The source poses this proxy for cue markers ($07:B8CA); the
            // native markers are typed values, so its pose is not modeled.
            continue;
        }
        assert_eq!(actor.base.position, vector(m, base), "{context} position");
        assert_eq!(
            [actor.base.pitch, actor.base.yaw, actor.base.roll].map(|a| a.units()),
            [byte(m, base + 0x12), byte(m, base + 0x14), byte(m, base + 0x16)],
            "{context} rotation"
        );
        assert_eq!(actor.base.hit_points, byte(m, base + 0x2D), "{context} health");
    }
    let camera = runner.objects.get(view).unwrap();
    assert_eq!(camera.base.position, vector(m, VIEW), "epoch {epoch}: view position");
    assert_eq!(
        runner.world.action_gate.unwrap().code,
        byte(m, 0x1D72),
        "epoch {epoch}: action gate"
    );
    // The scene palette is uploaded by the scene loader ($7F:0A76 DMA from
    // the $03:C7E8.. load lists), which is not ported yet, so palette words
    // are not compared here; the action stream's palette services are
    // verified separately against the original routines.
    assert_eq!(
        runner.world.cinematic_signals.unwrap().exit_requested,
        word(m, 0x1B96) & 0x10 != 0,
        "epoch {epoch}: exit request"
    );
    let random = runner.world.random.bytes();
    assert_eq!(
        random,
        [byte(m, 0xE0), byte(m, 0xE1), byte(m, 0xE2), byte(m, 0xE3)],
        "epoch {epoch}: random"
    );
}
