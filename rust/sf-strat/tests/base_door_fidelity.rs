//! Source-accurate base trigger and positional door-sound boundaries.

use std::cell::RefCell;
use std::rc::Rc;

use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_game::obj::strat_init_obj_vars;
use sf_strat::enemies_ground;

const MASSIVE_BASE_STRATEGY: usize = 142;
const PROXIMITY_DOOR_STRATEGY: usize = 139;
const COLONY_EXIT_STRATEGY: usize = 235;
const IRIS_STRATEGY: usize = 47;
const IRIS_INNER_SHAPE: u16 = 459;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SoundEvent {
    family: PosSndFamilyId,
    world_x: i16,
    world_z: i16,
}

#[derive(Clone, Default)]
struct RecordingHooks(Rc<RefCell<Vec<SoundEvent>>>);

impl Hooks for RecordingHooks {
    fn make_snd(&mut self, family: PosSndFamilyId, world_x: i16, world_z: i16) {
        self.0.borrow_mut().push(SoundEvent {
            family,
            world_x,
            world_z,
        });
    }
}

fn setup(log: Rc<RefCell<Vec<SoundEvent>>>) -> Game {
    let mut game = Game::with_hooks(Box::new(RecordingHooks(log)));
    enemies_ground::register(&mut game.world);

    let player = game.objs.alloc().expect("player slot");
    assert_eq!(player, 0);
    strat_init_obj_vars(&mut game.objs.aliens[player as usize]);
    game.vars.internal_playpt = player as i16;
    game
}

fn place(game: &mut Game, strategy_index: usize, world_x: i16, world_z: i16) -> u16 {
    let object = game.objs.alloc().expect("object slot");
    strat_init_obj_vars(&mut game.objs.aliens[object as usize]);
    let state = &mut game.objs.aliens[object as usize];
    state.worldx = world_x;
    state.worldz = world_z;
    state.stratptr = game.world.istrats[strategy_index];
    object
}

fn tick(game: &mut Game, object: u16) {
    let strategy = game.objs.aliens[object as usize]
        .stratptr
        .expect("registered strategy");
    game.call_strat(strategy, object);
}

#[test]
fn massive_base_initialization_clears_the_map_trigger() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut game = setup(log);
    game.vars.map.trigger = u8::MAX;
    let base = place(&mut game, MASSIVE_BASE_STRATEGY, 0, 20_000);

    tick(&mut game, base);

    assert_eq!(game.vars.map.trigger, 0);
}

#[test]
fn colony_exit_emits_one_open_sound_when_animation_starts() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut game = setup(log.clone());
    game.objs.aliens[0].worldz = 1_000;
    let door = place(&mut game, COLONY_EXIT_STRATEGY, 40, 100);

    tick(&mut game, door);
    tick(&mut game, door);

    assert_eq!(
        *log.borrow(),
        vec![SoundEvent {
            family: PosSndFamilyId::DoorOpen,
            world_x: 40,
            world_z: 100,
        }]
    );
    assert_eq!(game.objs.aliens[door as usize].animframe, 2);
}

#[test]
fn proximity_door_emits_open_and_close_sounds_at_source_frames() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut game = setup(log.clone());
    game.objs.aliens[0].worldz = 0;
    let door = place(&mut game, PROXIMITY_DOOR_STRATEGY, -20, 100);

    tick(&mut game, door);
    assert_eq!(game.objs.aliens[door as usize].animframe & 0x7F, 1);

    game.objs.aliens[door as usize].animframe = 0x87;
    game.objs.aliens[door as usize].worldz = 1_000;
    tick(&mut game, door);

    assert_eq!(
        *log.borrow(),
        vec![
            SoundEvent {
                family: PosSndFamilyId::DoorOpen,
                world_x: -20,
                world_z: 100,
            },
            SoundEvent {
                family: PosSndFamilyId::DoorClose,
                world_x: -20,
                world_z: 1_000,
            },
        ]
    );
    assert_eq!(game.objs.aliens[door as usize].animframe & 0x7F, 6);
}

#[test]
fn generic_door_emits_close_cues_at_both_source_frames() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut game = setup(log.clone());
    game.objs.aliens[0].worldz = 0;
    let door = place(&mut game, PROXIMITY_DOOR_STRATEGY, 30, 2_000);

    enemies_ground::door1_istrat(&mut game, door);
    game.objs.aliens[door as usize].animframe = 9;
    enemies_ground::door1openwait_strat(&mut game, door);
    assert_eq!(game.objs.aliens[door as usize].animframe, 8);

    game.objs.aliens[door as usize].animframe = 1;
    enemies_ground::door1closewait_strat(&mut game, door);

    assert_eq!(
        *log.borrow(),
        vec![
            SoundEvent {
                family: PosSndFamilyId::DoorClose,
                world_x: 30,
                world_z: 2_000,
            },
            SoundEvent {
                family: PosSndFamilyId::DoorClose,
                world_x: 30,
                world_z: 2_000,
            },
        ]
    );
    assert_eq!(game.objs.aliens[door as usize].animframe, 2);
}

#[test]
fn iris_spawns_its_inner_mesh_and_sounds_only_when_opening_starts() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut game = setup(log.clone());
    let iris = place(&mut game, IRIS_STRATEGY, 70, 1_000);
    {
        let outer = &mut game.objs.aliens[iris as usize];
        outer.worldy = -40;
        outer.rotx = 12;
        outer.roty = 34;
        outer.rotz = 56;
    }

    enemies_ground::iris_istrat(&mut game, iris);
    let inner = game
        .objs
        .active_indices()
        .into_iter()
        .find(|&object| object != 0 && object != iris)
        .expect("inner iris");
    let inner_state = game.objs.aliens[inner as usize];
    assert_eq!(inner_state.shape, IRIS_INNER_SHAPE);
    assert_eq!(
        (inner_state.worldx, inner_state.worldy, inner_state.worldz),
        (70, -40, 1_100)
    );
    assert_eq!(
        (inner_state.rotx, inner_state.roty, inner_state.rotz),
        (12, 34, 56)
    );

    game.objs.aliens[iris as usize].hp = 124;
    enemies_ground::iris_strat(&mut game, iris);
    enemies_ground::iris_strat(&mut game, iris);

    assert_eq!(
        *log.borrow(),
        vec![SoundEvent {
            family: PosSndFamilyId::DoorOpen,
            world_x: 70,
            world_z: 1_000,
        }]
    );
    assert_eq!(game.objs.aliens[iris as usize].animframe & 0x7F, 2);
}
