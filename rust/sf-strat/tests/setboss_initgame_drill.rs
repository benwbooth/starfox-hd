//! Tick 100: setboss / initgame_strats / drill.launchweb.

use sf_game::vars::BossEncounter;
use sf_game::Game;
use sf_strat::bosses::drill_launchweb;
use sf_strat::common::{initgame_strats_l, setboss_l, sv, StratRam};
use sf_strat::enemy_a::{bossflags, wm};

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn setboss_appends_and_skips_duplicate() {
    let mut g = Game::new();
    setboss_l(&mut g, BossEncounter::Route1Stage1);
    setboss_l(&mut g, BossEncounter::Route1Stage1); // duplicate of last → no-op
    setboss_l(&mut g, BossEncounter::Route1Stage2);
    assert_eq!(g.vars.boss_seq_len, 2);
    assert_eq!(g.vars.boss_seq[0], Some(BossEncounter::Route1Stage1));
    assert_eq!(g.vars.boss_seq[1], Some(BossEncounter::Route1Stage2));
}

#[test]
fn initgame_strats_clears_view_and_boss() {
    let mut g = Game::new();
    g.vars.set_sv_i16(sv::OUTVX, 99);
    g.vars.set_sv_i16(sv::OUTVY, 88);
    g.vars.set_sv_i16(sv::OUTVZ, 77);
    g.vars.bossmaxhp = 40;
    g.vars.bosshp = 12;
    g.vars.gameflags = 0xFF;
    g.vars.write_ext8(wm::BOSSFLAGS, 0x55);
    g.vars.shared.strategy_flags = 0xAA;

    initgame_strats_l(&mut g);
    assert_eq!(g.vars.sv_i16(sv::OUTVX), 0);
    assert_eq!(g.vars.sv_i16(sv::OUTVY), 0);
    assert_eq!(g.vars.sv_i16(sv::OUTVZ), 0);
    assert_eq!(g.vars.bossmaxhp, 0);
    assert_eq!(g.vars.bosshp, 0);
    assert_eq!(g.vars.gameflags, 0);
    assert_eq!(bossflags(&g), 0);
    assert_eq!(g.vars.shared.strategy_flags, 0);
    assert_eq!(g.vars.read_ext8(0x155C), 1); // gf2_ingame
}

#[test]
fn drill_launchweb_spawns_web() {
    let mut g = Game::new();
    let drill = spawn(&mut g);
    g.objs.aliens[drill as usize].worldx = 10;
    g.objs.aliens[drill as usize].worldy = 20;
    g.objs.aliens[drill as usize].worldz = 30;
    g.objs.aliens[drill as usize].roty = 40;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    drill_launchweb(&mut g, drill);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert_eq!(after, before + 1);
    let web = g
        .objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| a.active && *i as u16 != drill)
        .expect("web");
    assert_eq!(web.1.worldx, 10);
    assert_eq!(web.1.worldy, 20);
    assert_eq!(web.1.worldz, 30);
    assert_eq!(web.1.roty, 40);
    assert!(web.1.stratptr.is_some());
}
