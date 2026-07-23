//! Tick 82: mcore1 body + tunnela HF5 toggle + null_strat.

use sf_game::alien::ASF_NOHITAFFECT;
use sf_game::vars::HARD_AP;
use sf_game::Game;
use sf_strat::common::null_strat;
use sf_strat::enemies_ground::{tunnela2_strat, tunnela_istrat, tunnela_strat};
use sf_strat::enemy_a::wm;
use sf_strat::enemy_a::{
    mcore1_istrat, mcore1_strat, mcore1col_istrat, ASF2_RELEXPLODE, DEG180, DEG45,
};

const HF5: u8 = 1 << 4;
const TUNNEL_HP: u8 = 20;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].worldx = 0;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 5000;
    g.objs.aliens[idx as usize].worldy = -100;
    g.objs.aliens[idx as usize].worldx = 0;
    idx
}

#[test]
fn mcore1_init_wait_zoom() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.write_ext8(wm::CURRENTLEVEL, 1);

    let idx = spawn_obj(&mut g);
    mcore1_istrat(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.hp, 30);
    assert_eq!(al.ap, 40);
    assert_eq!(al.rotx, DEG180);
    assert_eq!(al.roty, DEG45.wrapping_neg());
    assert_ne!(al.sflags2 & ASF2_RELEXPLODE, 0);
    // Fall-through: state 0 → 1, sbyte1=20, nohitaffect
    assert_eq!(al.stratstate, 1);
    assert_eq!(al.sbyte1, 19); // set 20 then same-frame beqdec
    assert_ne!(al.sflags & ASF_NOHITAFFECT, 0);
    assert!(al.collstratptr.is_some());
    assert!(al.expstratptr.is_some());

    // Level 2 HP; fall-through colanim: hp 50 → frame 1
    let mut g2 = Game::new();
    spawn_player(&mut g2, 0);
    g2.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let i2 = spawn_obj(&mut g2);
    mcore1_istrat(&mut g2, i2);
    assert_eq!(g2.objs.aliens[i2 as usize].hp, 50);
    assert_eq!(g2.objs.aliens[i2 as usize].colframe, 1);

    // Wait countdown → state 2
    let mut g3 = Game::new();
    spawn_player(&mut g3, 0);
    g3.vars.write_ext8(wm::CURRENTLEVEL, 1);
    let i3 = spawn_obj(&mut g3);
    mcore1_istrat(&mut g3, i3);
    for _ in 0..20 {
        mcore1_strat(&mut g3, i3);
    }
    assert_eq!(g3.objs.aliens[i3 as usize].stratstate, 2);

    // Zoom-in: close Z → nextstate into zoom-away (vz += 10)
    g3.objs.aliens[i3 as usize].worldz = 1000; // |dz| < 1500 vs player@0
    g3.objs.aliens[i3 as usize].vz = 0;
    mcore1_strat(&mut g3, i3);
    assert_eq!(g3.objs.aliens[i3 as usize].stratstate, 3);
    assert_eq!(g3.objs.aliens[i3 as usize].vz, 10);

    // Col: state 5 → hitflash path (no crash); else defelasercol
    g3.objs.aliens[i3 as usize].stratstate = 5;
    mcore1col_istrat(&mut g3, i3);
}

#[test]
fn tunnela_hf5_toggle_and_null() {
    let mut g = Game::new();
    let idx = spawn_obj(&mut g);
    tunnela_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, TUNNEL_HP);
    assert_eq!(g.objs.aliens[idx as usize].ap, HARD_AP);
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 1); // fall-through dincanim

    // Animate further
    tunnela_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 2);

    // HF5 → tunnela2
    g.objs.aliens[idx as usize].hitflags |= HF5;
    let s_before = g.objs.aliens[idx as usize].stratptr;
    tunnela_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hitflags & HF5, 0);
    assert_ne!(g.objs.aliens[idx as usize].stratptr, s_before);

    // HF5 on tunnela2 → back
    g.objs.aliens[idx as usize].hitflags |= HF5;
    let s2 = g.objs.aliens[idx as usize].stratptr;
    tunnela2_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hitflags & HF5, 0);
    assert_ne!(g.objs.aliens[idx as usize].stratptr, s2);

    // null_strat is a no-op
    null_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].active);
}
