//! ROM boss1turret_nfire / boss1turretfire_end (GBSTRATS.ASM:353-401).

use sf_game::alien::{ASF4_CHILDOBJ, ASF4_MOTHEROBJ, ASF_NOHITAFFECT};
use sf_game::Game;
use sf_strat::enemy_a::{
    boss1turret_nfire, boss1turretfire_end, boss_attach_child_to_mother, bossflags, set_bossflags,
    BF_FLAG1,
};

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn link_mother_child(g: &mut Game, mother: u16, child: u16, child_num: u8) {
    g.objs.aliens[mother as usize].sflags4 |= ASF4_MOTHEROBJ;
    boss_attach_child_to_mother(g, mother, child, child_num);
    assert_ne!(g.objs.aliens[child as usize].sflags4 & ASF4_CHILDOBJ, 0);
}

#[test]
fn nfire_sets_colanim0_and_nohitaffect() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let tur = spawn(&mut g);
    link_mother_child(&mut g, mother, tur, 2);
    g.objs.aliens[tur as usize].colframe = 3;
    g.objs.aliens[tur as usize].sflags &= !ASF_NOHITAFFECT;
    g.objs.aliens[tur as usize].hp = 8;
    let hp0 = g.vars.bosshp;
    boss1turret_nfire(&mut g, tur, mother);
    assert_eq!(g.objs.aliens[tur as usize].colframe, 0);
    assert_ne!(g.objs.aliens[tur as usize].sflags & ASF_NOHITAFFECT, 0);
    assert_eq!(g.vars.bosshp, hp0.wrapping_add(8));
    g.objs.aliens[mother as usize].rotz = 40;
    boss1turret_nfire(&mut g, tur, mother);
    assert_eq!(g.objs.aliens[tur as usize].rotz, 40);
}

#[test]
fn fire_end_clears_nohitaffect_and_animates() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let tur = spawn(&mut g);
    link_mother_child(&mut g, mother, tur, 2);
    g.objs.aliens[tur as usize].colframe = 0;
    g.objs.aliens[tur as usize].sflags |= ASF_NOHITAFFECT;
    g.objs.aliens[tur as usize].hp = 8;
    g.vars.gameframe = 1; // off fire gate
    boss1turretfire_end(&mut g, tur, mother);
    assert_eq!(g.objs.aliens[tur as usize].sflags & ASF_NOHITAFFECT, 0);
    assert_eq!(g.objs.aliens[tur as usize].colframe, 1);
}

#[test]
fn fire_end_fires_on_normal_gate() {
    let mut g = Game::new();
    let _player = spawn(&mut g);
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
    let mother = spawn(&mut g);
    let tur = spawn(&mut g);
    link_mother_child(&mut g, mother, tur, 2);
    g.objs.aliens[tur as usize].hp = 8;
    let phase = tur as u16;
    // (gf+phase)&31==0 also implies &15==0; with bf_flag1 clear we take .normd→.norm.
    g.vars.gameframe = (32u16).wrapping_sub(phase % 32);
    assert_eq!((g.vars.gameframe.wrapping_add(phase)) & 31, 0);
    set_bossflags(&mut g, 0);
    let before = g.objs.active_indices().len();
    boss1turretfire_end(&mut g, tur, mother);
    assert!(
        g.objs.active_indices().len() > before,
        "normal laser should spawn"
    );
}

#[test]
fn fire_end_home_consumes_bf_flag1() {
    let mut g = Game::new();
    let _player = spawn(&mut g);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
    let mother = spawn(&mut g);
    let tur = spawn(&mut g);
    link_mother_child(&mut g, mother, tur, 2);
    g.objs.aliens[tur as usize].hp = 8;
    let phase = tur as u16;
    g.vars.gameframe = (16u16).wrapping_sub(phase % 16);
    assert_eq!((g.vars.gameframe.wrapping_add(phase)) & 15, 0);
    set_bossflags(&mut g, BF_FLAG1);
    let before = g.objs.active_indices().len();
    boss1turretfire_end(&mut g, tur, mother);
    assert_eq!(bossflags(&g) & BF_FLAG1, 0, "bf_flag1 consumed");
    assert!(g.objs.active_indices().len() > before);
}

#[test]
fn nfire_when_turrets_closed() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let tur = spawn(&mut g);
    link_mother_child(&mut g, mother, tur, 2);
    g.objs.aliens[tur as usize].colframe = 2;
    g.objs.aliens[tur as usize].sflags &= !ASF_NOHITAFFECT;
    g.objs.aliens[tur as usize].hp = 8;
    boss1turret_nfire(&mut g, tur, mother);
    assert_eq!(g.objs.aliens[tur as usize].colframe, 0);
    assert_ne!(g.objs.aliens[tur as usize].sflags & ASF_NOHITAFFECT, 0);
}
