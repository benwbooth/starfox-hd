//! ROM halfd door + kami weave/die/go + zacos2/cont aliases (GA2STRAT / GASTRATS).

use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemy_a::{
    halfd_istrat, halfd_strat, kami_cont, kami_istrat, kami_strat, kamidie_istrat, kamigo_init,
    zacos2_init, zacos2_strat, zacos_cont, zacos_istrat, zacos_strat, COLLTYPE_ZENEMY, DEG180,
    DEG90,
};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 3000;
    g.objs.aliens[idx as usize].worldy = -200;
    idx
}

#[test]
fn halfd_opens_when_close_closes_when_far() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    halfd_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, HARD_HP);
    // Far (|dz|>=700): anim reset.
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].animframe = 5;
    halfd_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 0);
    // Close: anim advances.
    g.objs.aliens[idx as usize].worldz = 300;
    halfd_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 1);
    g.objs.aliens[idx as usize].animframe = 9;
    halfd_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 9);
}

#[test]
fn kami_istrat_and_weave() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    kami_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 4);
    assert_eq!(g.objs.aliens[idx as usize].ap, 8);
    assert_eq!(g.objs.aliens[idx as usize].vel, 20);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ZENEMY, 0);
    let s1 = g.objs.aliens[idx as usize].sbyte1;
    kami_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, s1.wrapping_add(2));
    assert_eq!(g.objs.aliens[idx as usize].vz, -14);
    assert_eq!(g.objs.aliens[idx as usize].vy, 1);
    // roty = (vx_lo<<2)+deg180
    let vx_lo = g.objs.aliens[idx as usize].vx as u8;
    assert_eq!(
        g.objs.aliens[idx as usize].roty,
        vx_lo.wrapping_shl(2).wrapping_add(DEG180)
    );
}

#[test]
fn kamidie_dives_then_kamigo() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    kami_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldy = -50; // >= -100 → kamigo
    g.objs.aliens[idx as usize].rotx = 0;
    kamidie_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, HARD_HP);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}

#[test]
fn kamidie_pitches_while_high() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    kami_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldy = -200; // still high
    g.objs.aliens[idx as usize].rotx = 10;
    g.objs.aliens[idx as usize].vel = 20;
    // Call die strat body without going to kamigo.
    let tick = g.objs.aliens[idx as usize].expstratptr;
    let _ = tick;
    // Direct: set expstrat and run die strat via public API after forcing high y.
    // Use kamidie path by calling kamidie_istrat with high y — wait, istrat falls
    // into strat which would kamigo if y>=-100. Keep y=-200.
    g.objs.aliens[idx as usize].worldy = -200;
    kamidie_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 4); // not yet kamigo
    assert_eq!(g.objs.aliens[idx as usize].rotx, 14); // 10+4
}

#[test]
fn kamigo_chases_when_far() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].vel = 40;
    kamigo_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, HARD_HP);
    assert!(g.objs.aliens[idx as usize].vel > 40 || g.objs.aliens[idx as usize].vel == 41);
}

#[test]
fn zacos_aliases_and_phase2() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldy = -40; // at player height → pitch block
    zacos_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].ap, 4);
    assert_eq!(g.objs.aliens[idx as usize].rotx, DEG90.wrapping_sub(2)); // phase0 ran
                                                                         // Cont moves.
    let z0 = g.objs.aliens[idx as usize].worldz;
    zacos_cont(&mut g, idx);
    // After gen+addvecs+playerZ, z may change.
    let _ = z0;
    // Force phase1 via zacos2.
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].worldz = 500; // |dz|<2000
    zacos2_init(&mut g, idx);
    // zacos2 falls through phase1 into zacos3_init and zacos3_strat on this
    // frame, so both source pitch steps have already run.
    assert_eq!(g.objs.aliens[idx as usize].rotx, 248);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    zacos_strat(&mut g, idx); // still callable
    zacos2_strat(&mut g, idx);
}

#[test]
fn zacos_zero_pitch_runs_the_complete_source_fallthrough() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldy = -40;
    g.objs.aliens[idx as usize].worldz = 500;
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].roty = DEG180;
    g.objs.aliens[idx as usize].vel = 40;

    zacos_strat(&mut g, idx);

    assert_eq!(g.objs.aliens[idx as usize].rotx, 248);
    assert_eq!(g.objs.aliens[idx as usize].vy, -7);
    assert_eq!(g.objs.aliens[idx as usize].vz, -37);
}

#[test]
fn zacos_dive_completion_accelerates_on_the_transition_frame() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldy = -40;
    g.objs.aliens[idx as usize].worldz = 500;
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].roty = DEG180;
    g.objs.aliens[idx as usize].vel = 40;
    zacos2_init(&mut g, idx);

    g.objs.aliens[idx as usize].rotx = 0;
    g.vars.gameframe = 1;
    let dive = g.objs.aliens[idx as usize].stratptr.expect("dive strategy");
    g.call_strat(dive, idx);

    assert_eq!(g.objs.aliens[idx as usize].rotx, 252);
    assert_eq!(g.objs.aliens[idx as usize].vel, 41);
}

#[test]
fn zacos_bank_phase_uses_the_source_object_stagger() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    for _ in 0..4 {
        spawn_obj(&mut g);
    }
    let idx = spawn_obj(&mut g);
    assert_eq!(idx, 5);
    g.objs.aliens[idx as usize].worldy = -40;
    g.objs.aliens[idx as usize].worldz = 500;
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].roty = DEG180;
    g.objs.aliens[idx as usize].vel = 40;
    zacos2_init(&mut g, idx);

    g.objs.aliens[idx as usize].rotx = 0;
    g.vars.gameframe = 15;
    let dive = g.objs.aliens[idx as usize].stratptr.expect("dive strategy");
    g.call_strat(dive, idx);

    assert_eq!(g.objs.aliens[idx as usize].rotx, 252);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG180);
}

#[test]
fn kami_cont_applies_velocity() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].vx = 10;
    g.objs.aliens[idx as usize].vy = 1;
    g.objs.aliens[idx as usize].vz = -14;
    let x0 = g.objs.aliens[idx as usize].worldx;
    kami_cont(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, x0.wrapping_add(10));
}
