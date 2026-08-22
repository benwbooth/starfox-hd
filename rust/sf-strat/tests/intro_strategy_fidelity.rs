//! Reachability and source-timing regressions for the retail attract intro.

use sf_game::alien::{
    ASF3_REALOBJ, ASF_COLLDISABLE, ASF_HITFLASH, ATMISSILE, ATZREMOVE,
};
use sf_game::Game;
use sf_map::catalog::map_id;
use sf_map::consts::intro_strategy_address;
use sf_strat::intro::{
    player_down_intro_init, player_down_intro_tick, player_down_left_intro_init,
    player_down_right_intro_init, player_fire_intro_init, zaco_intro_init, zaco_intro_tick,
    zaco_leader_intro_init, zaco_leader_intro_tick,
};

// Retail's runtime #smoke word ($ADD5) resolves to flat id 357.
const SMOKE_SHAPE: u16 = 357;
const OLD_TYPE_SHAPE: u16 = 323;
const NULL_SHAPE: u16 = 0;
const CENTER_CRAFT_DAMAGED_PHASE: u8 = 2;
const LEADER_ATTACK_PHASE: u8 = 2;
const INTRO_CRAFT_SPEED: u8 = 120;
const INTRO_CRAFT_HARDNESS: u8 = 8;
const CENTER_CRAFT_FIRST_PHASE_TICKS_AFTER_INIT: usize = 33;
const EXPECTED_SECOND_HIT_DELAY_AFTER_ENTRY: i16 = 19;
const WING_LIFETIME_AFTER_INIT: u8 = 69;
const LASER_LIFETIME: u8 = 60;
const ZACO_LIFETIME_AFTER_INIT: u8 = 59;
const ZACO_TURN_DELAY_AFTER_INIT: u8 = 9;
const ZACO_TARGETING_SPEED_AFTER_ENTRY: u8 = 62;
const INTRO_SMOKE_VELOCITY_X: i16 = 40;
const ANIMATION_FRAME_MASK: u8 = 127;
const INTRO_MAP_RESUME_LIMIT: usize = 8;
const EXPECTED_INTRO_CRAFTS: usize = 3;
const EVERY_FOURTH_FRAME_PERIOD: u16 = 4;
const EXPECTED_PAIRED_LASERS: usize = 2;
const LEADER_ATTACK_DISTANCE: i16 = 600;

fn setup() -> (Game, u16) {
    let mut game = Game::new();
    let player = game.objs.alloc().expect("player");
    assert_eq!(player, 0);
    game.objs.aliens[player as usize].sflags3 |= ASF3_REALOBJ;
    game.vars.internal_playpt = player as i16;
    let object = game.objs.alloc().expect("intro object");
    (game, object)
}

fn objects_with_type(game: &Game, object_type: u8) -> Vec<u16> {
    game.objs
        .active_indices()
        .into_iter()
        .filter(|&object| game.objs.aliens[object as usize].type_ & object_type != 0)
        .collect()
}

#[test]
fn every_intro_map_strategy_address_resolves_to_native_rust() {
    let mut game = Game::new();
    sf_strat::table::register_all(&mut game);

    for (name, address) in [
        ("center craft", intro_strategy_address::PLAYER_DOWN),
        ("left craft", intro_strategy_address::PLAYER_DOWN_LEFT),
        ("right craft", intro_strategy_address::PLAYER_DOWN_RIGHT),
        ("laser controller", intro_strategy_address::PLAYER_FIRE),
        ("fighter wave", intro_strategy_address::ZACO),
        ("fighter leader", intro_strategy_address::ZACO_LEADER),
    ] {
        assert!(
            game.world.find_strategy_address(address).is_some(),
            "{name} address did not resolve"
        );
    }
}

#[test]
fn retail_intro_map_spawns_live_center_wing_and_laser_strategies() {
    let level = sf_map::catalog::get_map_data(map_id::INTRO).expect("intro map");
    let mut game = Game::new();
    sf_strat::table::register_all(&mut game);
    sf_strat::player::strat_spawn_player_for_map(&mut game, map_id::INTRO)
        .expect("passive presentation player");
    game.load_level(level);
    if let Some((native_callbacks, inline_callbacks)) =
        sf_map::catalog::get_map_callback_regs(map_id::INTRO)
    {
        game.world
            .register_named_callbacks(native_callbacks, inline_callbacks, &level.labels);
    }

    game.map_exec();
    for _ in 0..INTRO_MAP_RESUME_LIMIT {
        game.vars.mapcnt = 0;
        game.map_exec();
        let craft_count = game
            .objs
            .aliens
            .iter()
            .filter(|object| object.active && object.shape == OLD_TYPE_SHAPE)
            .count();
        if craft_count == EXPECTED_INTRO_CRAFTS {
            break;
        }
    }
    game.vars.mapcnt = 0;
    game.map_exec();

    let intro_crafts: Vec<u16> = game
        .objs
        .active_indices()
        .into_iter()
        .filter(|&object| game.objs.aliens[object as usize].shape == OLD_TYPE_SHAPE)
        .collect();
    assert_eq!(intro_crafts.len(), EXPECTED_INTRO_CRAFTS);
    assert!(intro_crafts
        .iter()
        .all(|&object| game.objs.aliens[object as usize].stratptr.is_some()));
    assert!(game.objs.aliens.iter().any(|object| {
        object.active
            && object.shape == NULL_SHAPE
            && object.sword1 != 0
            && object.stratptr.is_some()
    }));

    game.tick();
    assert!(intro_crafts
        .iter()
        .all(|&object| game.objs.aliens[object as usize].sflags & ASF_COLLDISABLE != 0));
}

#[test]
fn center_craft_reaches_the_authored_hit_smoke_and_damaged_flight() {
    let (mut game, craft) = setup();
    player_down_intro_init(&mut game, craft);

    {
        let craft_state = game.objs.aliens[craft as usize];
        assert_eq!(craft_state.vel, INTRO_CRAFT_SPEED);
        assert_eq!(craft_state.hp, 1);
        assert_eq!(craft_state.ap, INTRO_CRAFT_HARDNESS);
        assert_ne!(craft_state.sflags & ASF_COLLDISABLE, 0);
        assert_eq!(craft_state.type_ & ATZREMOVE, 0);
    }

    for frame in 1..=CENTER_CRAFT_FIRST_PHASE_TICKS_AFTER_INIT {
        game.vars.gameframe = frame as u16;
        player_down_intro_tick(&mut game, craft);
    }

    let craft_state = game.objs.aliens[craft as usize];
    assert_eq!(craft_state.stratstate, CENTER_CRAFT_DAMAGED_PHASE);
    assert_eq!(craft_state.sword2, EXPECTED_SECOND_HIT_DELAY_AFTER_ENTRY);
    assert_ne!(craft_state.sflags & ASF_HITFLASH, 0);

    let smoke = game
        .objs
        .active_indices()
        .into_iter()
        .find(|&object| game.objs.aliens[object as usize].shape == SMOKE_SHAPE)
        .expect("damaged intro smoke");
    assert_eq!(game.objs.aliens[smoke as usize].vx, INTRO_SMOKE_VELOCITY_X);
}

#[test]
fn wing_crafts_peel_in_opposite_directions_and_keep_retail_lifetime() {
    let (mut game, left) = setup();
    let right = game.objs.alloc().expect("right craft");
    game.vars.gameframe = 0;
    player_down_left_intro_init(&mut game, left);
    player_down_right_intro_init(&mut game, right);

    let left_state = game.objs.aliens[left as usize];
    let right_state = game.objs.aliens[right as usize];
    assert_eq!(left_state.roty, 1);
    assert_eq!(right_state.roty, 1u8.wrapping_neg());
    assert_eq!(left_state.count, WING_LIFETIME_AFTER_INIT);
    assert_eq!(right_state.count, WING_LIFETIME_AFTER_INIT);
    assert_eq!(left_state.hp, INTRO_CRAFT_HARDNESS);
    assert_eq!(right_state.hp, INTRO_CRAFT_HARDNESS);
}

#[test]
fn paired_laser_controller_emits_two_persistent_offset_shots() {
    let (mut game, target) = setup();
    let controller = game.objs.alloc().expect("laser controller");
    game.objs.aliens[target as usize].worldz = 1000;
    game.objs.aliens[controller as usize].sword1 = (target + 1) as i16;
    game.vars.gameframe = (EVERY_FOURTH_FRAME_PERIOD - controller % EVERY_FOURTH_FRAME_PERIOD)
        % EVERY_FOURTH_FRAME_PERIOD;

    player_fire_intro_init(&mut game, controller);

    let lasers = objects_with_type(&game, ATMISSILE);
    assert_eq!(lasers.len(), EXPECTED_PAIRED_LASERS);
    let mut x_positions: Vec<i16> = lasers
        .iter()
        .map(|&laser| {
            let laser = game.objs.aliens[laser as usize];
            assert_eq!(laser.count, LASER_LIFETIME);
            assert_eq!(laser.type_ & ATZREMOVE, 0);
            laser.worldx
        })
        .collect();
    x_positions.sort_unstable();
    assert!(x_positions[0] < 0);
    assert!(x_positions[1] > 0);
}

#[test]
fn fighter_wave_uses_retail_tumble_then_acceleration_timing() {
    let (mut game, fighter) = setup();
    zaco_intro_init(&mut game, fighter);
    assert_eq!(
        game.objs.aliens[fighter as usize].count,
        ZACO_LIFETIME_AFTER_INIT
    );
    assert_eq!(
        game.objs.aliens[fighter as usize].sbyte1,
        ZACO_TURN_DELAY_AFTER_INIT
    );
    assert_eq!(
        game.objs.aliens[fighter as usize].animframe & ANIMATION_FRAME_MASK,
        1
    );

    for frame in 1..=ZACO_TURN_DELAY_AFTER_INIT {
        game.vars.gameframe = frame as u16;
        zaco_intro_tick(&mut game, fighter);
    }
    assert_eq!(
        game.objs.aliens[fighter as usize].vel,
        ZACO_TARGETING_SPEED_AFTER_ENTRY
    );
    assert_eq!(game.objs.aliens[fighter as usize].sbyte1, 1);
}

#[test]
fn lead_fighter_attacks_then_requests_the_intro_exit_after_passing_player() {
    let (mut game, leader) = setup();
    zaco_leader_intro_init(&mut game, leader);
    {
        let leader_state = &mut game.objs.aliens[leader as usize];
        leader_state.stratstate = LEADER_ATTACK_PHASE;
        leader_state.worldz = LEADER_ATTACK_DISTANCE;
        leader_state.rotx = 0;
        leader_state.roty = 0;
        leader_state.rotz = 0;
    }
    game.vars.gameframe = 0;
    zaco_leader_intro_tick(&mut game, leader);
    assert_eq!(objects_with_type(&game, ATMISSILE).len(), 1);
    assert!(!game.vars.strategy.intro_exit_requested);

    {
        let leader_state = &mut game.objs.aliens[leader as usize];
        leader_state.worldz = -100;
        leader_state.vel = 0;
    }
    game.vars.gameframe = 1;
    zaco_leader_intro_tick(&mut game, leader);
    assert!(game.vars.strategy.intro_exit_requested);
}
