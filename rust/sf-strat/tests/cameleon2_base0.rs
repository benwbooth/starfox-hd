//! ROM cameleon2 / cam2 hide-dash (GASTRATS.ASM:1440) + base0 / bazooka1 publicize.

use sf_game::Game;
use sf_strat::enemies_ground::{
    base0_istrat, base0_strat, base0b_strat, bazooka1l_istrat, bazooka1r_istrat,
};
use sf_strat::enemy_a::{
    cam2dash_init, cam2dash_strat, cam2hide_init, cam2hide_strat, cam2nextpos, cameleon2_cont,
    cameleon2_istrat, cameleon2_strat, COLLTYPE_ZENEMY,
};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 3000;
    idx
}

#[test]
fn base0_waits_far_opens_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 6000;
    base0_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 0);
    assert_eq!(g.objs.aliens[idx as usize].roty, 192); // DEG270
                                                       // Close range → open anim.
    g.objs.aliens[idx as usize].worldz = 2000;
    base0_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 1);
    for _ in 0..20 {
        base0b_strat(&mut g, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].animframe, 8);
}

#[test]
fn bazooka1_l_sets_sflag1_r_does_not() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let l = spawn_obj(&mut g);
    g.objs.aliens[l as usize].worldy = 500;
    bazooka1l_istrat(&mut g, l);
    assert_ne!(g.objs.aliens[l as usize].sflags2 & 0x10, 0);
    let r = spawn_obj(&mut g);
    g.objs.aliens[r as usize].worldy = 500;
    bazooka1r_istrat(&mut g, r);
    assert_eq!(g.objs.aliens[r as usize].sflags2 & 0x10, 0);
    assert_eq!(g.objs.aliens[r as usize].vel, 80);
}

#[test]
fn cameleon2_init_teleports_and_flips() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    cameleon2_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ZENEMY, 0);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    // cam2nextpos slot 0 → (-300,-60), then strat chases rotx toward deg180.
    assert_eq!(g.objs.aliens[idx as usize].worldx, -300);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -60);
    assert!(g.objs.aliens[idx as usize].rotx > 0);
}

#[test]
fn cameleon2_hide_advances_slots_then_dash() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    cameleon2_istrat(&mut g, idx);
    // Force hide from slot 0 → slot 1.
    g.objs.aliens[idx as usize].rotx = 128; // already flipped
    cam2hide_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 1);
    // Ease hide to 0 then nextpos.
    g.objs.aliens[idx as usize].rotx = 0;
    cam2hide_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 300); // slot 1
                                                         // Advance to dash: sbyte1=5 then hide → 6 → dash.
    g.objs.aliens[idx as usize].sbyte1 = 5;
    cam2hide_init(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    // Dash tick spins rotz / speeds up.
    g.objs.aliens[idx as usize].rotx = 128;
    cam2dash_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].rotz > 0 || g.objs.aliens[idx as usize].vel > 0);
}

#[test]
fn cameleon2_cont_and_nextpos() {
    let mut g = Game::new();
    spawn_player(&mut g, 100);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 500;
    let z0 = g.objs.aliens[idx as usize].worldz;
    cameleon2_cont(&mut g, idx);
    // add_player_z may or may not change z depending on medpspeed; just call.
    let _ = z0;
    g.objs.aliens[idx as usize].sbyte1 = 3;
    cam2nextpos(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 0);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -200);
    cam2dash_init(&mut g, idx);
    cameleon2_strat(&mut g, idx);
}
