//! The retail attract loop (indexed scenes 6, 7, 6), composed natively and
//! compared with the complete retail machine at every strategy epoch. The
//! native loop starts from its own boot world, spawns and initializes each
//! scene player, applies the stage hand-over and carries its world between
//! scenes. The only retail input is each scene's palette, which the scene
//! loader uploads (not ported), plus the render-timed entropy-refresh points.
use super::motion_reset_tests::Reader;
use super::{Source, WRAM};
use sf2_game::authored_paths;
use sf2_game::player_action::ScenePalette;
use sf2_game::scene_runner::{EntropyRefresh, SceneRunner};
use sf2_game::scene_strategy::{SceneActors, SceneCallbacks};
use sf2_game::strategy_schedule::{StrategyCompletion, StrategySchedule};
use sf2_game::{
    Angle, Behavior, Object, ObjectId, ObjectKind, ObjectSpawnDefaults, ObjectStore, ShapeId,
    Vector3,
};
use sf_oracle::RetailMachine;

const RETAIL: u32 = 0x7E0000;
const VIEW: u16 = 0x033F;
const POOL: u16 = 0x03BD;
const STRIDE: u16 = 0x3F;
const EPOCH: u32 = 0x7F34E7;
const INITIALIZER: u32 = 0x0682F9;
const INITIALIZER_RETURN: u32 = 0x06832B;
const REFRESH: u32 = 0x7F058F;
const RANDOM_DRAW: u32 = 0x7F7BD4;
const RANDOM_RETURN: u32 = 0x7F7BE7;
/// The scene paths' inline reseed ($44:B13C, executed through its CPU banks).
const RESEEDS: [u32; 4] = [0x09B13D, 0x44B13D, 0x89B13D, 0xC4B13D];

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


/// Advance the retail machine to the first epoch of the next scene player.
/// Stop as the map-spawned scene player's initializer is entered. The map
/// hand-over runs that first visit itself, outside a frame epoch.
fn advance_to_initializer(machine: &mut RetailMachine) {
    assert!(
        machine.tick_until_cpu_execution(0, INITIALIZER, 6000).unwrap(),
        "attract scene player never spawned"
    );
}

#[test]
fn attract_scenes_six_and_seven_run_natively_like_the_retail_machine() {
    let mut m = RetailMachine::new(super::rom());
    // Each scene's player is re-spawned by the map and initialized; every
    // scene run ends with the retail machine at the next initializer.
    advance_to_initializer(&mut m);
    let mut carried = None;
    for selection in [6u8, 7, 6] {
        let (epochs, world) = run_scene(&mut m, selection, carried.take());
        carried = Some(world);
        eprintln!("scene {selection} matched for {epochs} epochs");
        // Both scenes run to their hand-over; scene six requests its exit.
        assert!(epochs > if selection == 6 { 440 } else { 100 });
    }
}

/// Read the starting state once, then run both engines independently until
/// the retail scene hands over. Returns the number of matched epochs.
fn run_scene(
    m: &mut RetailMachine,
    selection: u8,
    carried: Option<(sf2_game::scene_path_world::ScenePathWorld, Poses)>,
) -> (u32, (sf2_game::scene_path_world::ScenePathWorld, Poses)) {
    let (carried, carried_poses) = match carried {
        Some((world, poses)) => (Some(world), Some(poses)),
        None => (None, None),
    };
    assert_eq!(retail_list(m), vec![POOL, POOL + STRIDE]);

    // The map spawns the scene player into an empty list; the excluded
    // proxy follows it. The fixed view lives outside the source pool, so it
    // is listed last natively and never scheduled.
    let mut objects = ObjectStore::new();
    let defaults = ObjectSpawnDefaults { group: 0xFF, run_when_paused: false };
    let player = sf2_game::attract_stage::spawn_scene_player(&mut objects, defaults).unwrap();
    assert_eq!(vector(m, POOL), objects.get(player).unwrap().base.position);
    let idle = sf2_game::attract_stage::excluded_proxy(carried_poses.as_ref().map(|poses| poses.1));
    let idle = objects.allocate_after(Some(player), idle).unwrap();
    let mut view = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Unassigned);
    if let Some((view_pose, _)) = &carried_poses {
        view.base.position = view_pose.0;
        [view.base.pitch, view.base.yaw, view.base.roll] = view_pose.1;
    }
    view.base.flags.strategy_suspended = true;
    if carried_poses.is_some() && selection == 6 {
        sf2_game::attract_stage::reset_view(&mut view);
    }
    let view = objects.allocate_after(Some(idle), view).unwrap();
    assert_eq!(objects.active_ids(), &[player, idle, view]);

    // The first scene reads the boot state; later scenes continue the
    // previous scene's world, as the source's RAM does.
    let mut world = match carried {
        Some(world) => world,
        None => sf2_game::attract_stage::boot_world(),
    };
    world.primary_player = Some(player);
    world.fixed_players[0] = Some(view);

    let mut runner = SceneRunner::new(objects, world, Callbacks);
    runner.schedule = StrategySchedule::resume(runner.world.strategy_clock);
    // The strategy pass skips the shared excluded actor (14D6).
    assert_eq!(word(m, 0x14D6), POOL + STRIDE);
    runner.execution.controls.excluded_actor = Some(idle);
    let catalog = authored_paths::catalog();
    // The initializer's visit runs inside the map hand-over. The map then
    // publishes the scene's selection and gate and the scene loader its
    // palette and background before the first epoch: those producers are not
    // ported, so their outputs are read once more at that epoch. Everything
    // the initializer itself produces is compared instead.
    sf2_game::player_scene_init::initialize(
        &mut runner.objects,
        &mut runner.world,
        &mut runner.execution.paths.runtime,
        player,
    )
    .unwrap();
    // Compare as the original initializer returns, before other services.
    assert!(m.tick_until_cpu_execution(0, INITIALIZER_RETURN, 1).unwrap());
    compare_initialized_player(m, &runner, player, idle);
    assert!(m.tick_until_cpu_execution(0, EPOCH, 60).unwrap());
    assert_eq!(byte(m, 0x1D73), selection);
    let world = &mut runner.world;
    {
        // Native stage hand-over and the map's scene selection.
        sf2_game::attract_stage::hand_over(world).unwrap();
        sf2_game::attract_stage::select_scene(world, selection).unwrap();
        sf2_game::attract_stage::start_frame_loop(world);
        let mode = world.view_transition_mode.unwrap();
        let group = world.spawn_defaults.unwrap().group;
        world.spawn_defaults = Some(mode.spawn_defaults(ObjectSpawnDefaults {
            group,
            run_when_paused: false,
        }));
    }
    world.palette = Some(ScenePalette {
        colors: std::array::from_fn(|i| word(m, 0xEFE5 + i as u16 * 2)),
        saved_colors: std::array::from_fn(|i| word(m, 0xF2E5 + i as u16 * 2)),
    });
    let clock = runner.world.strategy_clock;
    runner.schedule = StrategySchedule::resume(clock);
    runner.prepare_frame(&catalog).unwrap();
    compare(m, &runner, view, 0);
    let mut ended = None;
    for epoch in 0..2000u32 {
        // The retail epoch runs first so its render-timed entropy refresh can
        // be placed at the same actor-visit boundary natively.
        // Draw entry/return markers alternate, so consecutive draws are not
        // collapsed into one recorded entry.
        let mut watched = vec![REFRESH, RANDOM_DRAW, RANDOM_RETURN];
        watched.extend(RESEEDS);
        m.watch_cpu_execution(&watched);
        assert!(m.tick_until_cpu_execution(0, EPOCH + 1, 60).unwrap());
        match m.tick_until_cpu_execution_any(0, &[EPOCH, INITIALIZER], 120).unwrap() {
            Some(EPOCH) => {}
            // The next scene's player is being initialized: stop here.
            Some(_) => {
                ended = Some(epoch);
                break;
            }
            None => panic!("epoch {epoch}: the retail frame loop stalled"),
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
                // A reseed is an ordered RNG event like a draw.
                hit if RESEEDS.contains(&hit) => draws += 1,
                _ => {}
            }
        }
        let refresh = EntropyRefresh::BeforeDraws(&refreshes);
        // Retail now stands at the next epoch, after its pre-epoch services.
        runner
            .run_epoch(&catalog, refresh)
            .and_then(|()| runner.finish_frame(&catalog))
            .and_then(|()| runner.prepare_frame(&catalog))
            .unwrap_or_else(|error| panic!("epoch {epoch}: {error:?}"));
        compare(m, &runner, view, epoch);
        if word(m, POOL + 0x19) != 0x84A2 {
            ended = Some(epoch);
            break;
        }
    }
    let ended = ended.expect("the scene ends within the test bound");
    assert!(ended > 100);
    let view_pose = runner.objects.get(view).unwrap();
    let view_pose = (
        view_pose.base.position,
        [view_pose.base.pitch, view_pose.base.yaw, view_pose.base.roll],
    );
    let idle_pose = runner.objects.get(idle).unwrap().base.position;
    (ended, (runner.world, (view_pose, idle_pose)))
}

/// The fixed view's and the excluded proxy's poses, which persist across
/// scenes.
type Poses = ((Vector3, [Angle; 3]), Vector3);

/// The initializer's records, decoded from the retail allocation.
fn compare_initialized_player(
    m: &RetailMachine,
    runner: &SceneRunner<Callbacks>,
    player: ObjectId,
    idle: ObjectId,
) {
    let mut source = Source::new(&super::rom(), 0);
    for offset in 0..0x20000u32 {
        source.bus.write8(WRAM + offset, m.peek8(RETAIL + offset));
    }
    let slot = u32::from(word(m, POOL + 0x2B));
    let reader = Reader { source: &source, slot, other: idle };
    let native = runner.world.player(&runner.objects, player).unwrap();
    let mut retail = reader.records();
    // The shared reader fixes this record to its own test convention; it
    // is not decoded from the allocation.
    retail.carried = native.carried;
    if native != &retail {
        let (n, r) = (format!("{native:#?}"), format!("{retail:#?}"));
        let diff: Vec<String> = n
            .lines()
            .zip(r.lines())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| format!("line {i}: native {a} retail {b}"))
            .collect();
        panic!("initialized player records differ:\n{}", diff.join("\n"));
    }
    assert_eq!(
        sf2_game::player_storage::get(&runner.objects, &runner.execution.paths.runtime.resources, player)
            .unwrap(),
        &reader.storage(),
        "initialized player storage"
    );
    let actor = runner.objects.get(player).unwrap();
    assert_eq!(actor.base.hit_points, byte(m, POOL + 0x2D));
    assert_eq!(actor.base.attack_power, byte(m, POOL + 0x2E));
    assert_eq!(actor.extension.texture_scroll_x, byte(m, POOL + 0x1CDA));
    assert_eq!(runner.world.scene.player_configuration, Some(byte(m, 0x1DE2)));
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
