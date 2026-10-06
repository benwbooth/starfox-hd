//! ROM cutscene phase-2 inits (PCSTRATS / PSTRATS).

use sf_game::vars::PSF3_ENGINESND;
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::player::{
    player_chase2_init, player_clear_demo2_init, player_clear_demo2_strat, player_clear_turn2_init,
    player_dive2_init, player_move_init, player_start_init, player_sv as sv, player_under2_init,
    player_warp1_init, player_warp2_init,
};

#[test]
fn player_start_and_move_init() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.vars.pshipflags = 0xff;
    player_start_init(&mut g);
    assert_eq!(g.vars.pshipflags, 0);
    assert_eq!(g.vars.sv_u16(sv::SPECWEPCNT), 3);
    assert_eq!(g.vars.sv_u16(sv::PLAYERSHAPE), 2);

    player_move_init(&mut g, p);
    assert_eq!(g.vars.viewdist, 120);
    assert_eq!(g.vars.sv_i16(sv::OUTDIST), 120);
    assert_eq!(g.vars.internal_playpt, p as i16);
}

#[test]
fn checkpoint_player_handoff_clears_stale_motion_without_resetting_inventory() {
    use sf_game::alien::{ASF_SHADOW, ATGND, ATZREMOVE};
    use sf_strat::player::{prepare_checkpoint_restart_player, strat_spawn_player};

    let mut g = Game::new();
    let p = strat_spawn_player(&mut g).unwrap();
    let body = g.objs.alloc().unwrap();
    g.coldet.pcbox.body = Some(body);
    g.objs.aliens[body as usize].hp = 37;
    g.objs.aliens[p as usize].hp = 10;
    g.objs.aliens[p as usize].ap = 0;
    g.objs.aliens[p as usize].type_ = ATGND | ATZREMOVE;
    g.objs.aliens[p as usize].sflags = 0;
    g.vars.shared.slime_count = 29;
    g.vars.strategy.player_roll_velocity = -32;
    g.vars.strategy.player_roll_offset = 67;
    g.vars.strategy.player_depth_shake = -913;
    g.vars.strategy.player_depth_strategy_offset = 84;
    g.vars.strategy.player_hit_count = 3;
    g.vars.strategy.player_laser_count = 2;
    g.vars.strategy.special_delay = 50;
    g.vars.strategy.special_weapon_count = 7;
    g.vars.strategy.player_roll_delay = 2;
    g.vars.strategy.player_control_delay = 19;
    g.vars.strategy.fire_count = 6;
    g.vars.strategy.fire_delay = 8;

    prepare_checkpoint_restart_player(&mut g, p);

    assert_eq!(g.vars.shared.slime_count, 0);
    assert_eq!(g.vars.strategy.player_roll_velocity, 0);
    assert_eq!(g.vars.strategy.player_roll_offset, 0);
    assert_eq!(g.vars.strategy.player_depth_shake, 0);
    assert_eq!(g.vars.strategy.player_depth_strategy_offset, 0);
    assert_eq!(g.vars.strategy.player_hit_count, 0);
    assert_eq!(g.vars.strategy.player_laser_count, 0);
    assert_eq!(g.vars.strategy.special_delay, 1);
    assert_eq!(g.vars.strategy.special_weapon_count, 7);
    // The initializer retains both timers; playercred's immediate movement
    // body then consumes one visit, as in the source fall-through path.
    assert_eq!(g.vars.strategy.player_roll_delay, 1);
    assert_eq!(g.vars.strategy.player_control_delay, 18);
    assert_eq!(g.vars.strategy.fire_count, 6);
    assert_eq!(g.vars.strategy.fire_delay, 8);
    assert_eq!(g.objs.aliens[p as usize].hp, 255);
    assert_eq!(g.objs.aliens[p as usize].ap, 8);
    assert_eq!(g.objs.aliens[p as usize].type_, ATGND);
    assert_ne!(g.objs.aliens[p as usize].sflags & ASF_SHADOW, 0);
    assert_eq!(g.objs.aliens[body as usize].hp, 37);
}

#[test]
fn phase2_dup_inits() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    g.objs.aliens[p as usize].shape = 2;
    g.vars.pshipflags3 |= PSF3_ENGINESND;

    let d = player_chase2_init(&mut g, p).expect("dup");
    assert_ne!(d, p);
    assert_eq!(g.vars.pshipflags3 & PSF3_ENGINESND, 0);

    let p2 = g.objs.alloc().expect("p2");
    let d2 = player_clear_turn2_init(&mut g, p2).expect("dup2");
    assert_eq!(g.objs.aliens[d2 as usize].sbyte1, 46);
    assert_eq!(g.vars.sv_i16(sv::OUTVY), 0);

    let p3 = g.objs.alloc().expect("p3");
    let _ = player_under2_init(&mut g, p3).expect("under");
    let p4 = g.objs.alloc().expect("p4");
    let d4 = player_warp1_init(&mut g, p4).expect("warp1");
    // `s_set_strat ... clshipboostnosnd_Istrat` falls through to its strategy
    // in the same frame, so the ROM's initial 19 is already decremented to 18.
    assert_eq!(g.objs.aliens[d4 as usize].sbyte2, 18);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 20);

    player_warp2_init(&mut g, p4);
    let p5 = g.objs.alloc().expect("p5");
    let _ = player_dive2_init(&mut g, p5).expect("dive2");
}

#[test]
fn clear_demo2_init_and_strat() {
    let mut g = Game::new();
    let p = g.objs.alloc().expect("slot");
    player_clear_demo2_init(&mut g, p);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 0);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE2), 250);

    g.vars.set_sv_i16(sv::OUTVY, 0);
    player_clear_demo2_strat(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::OUTVY), -32);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE2), 249);
}
