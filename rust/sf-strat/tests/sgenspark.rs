//! ROM `sgenspark_srou` / `lspark_*` (PSTRATS.ASM:54-88).

use sf_game::alien::{ASF2_COLLDISABLE, ASF_COLLDISABLE, NUMBER_AL};
use sf_game::Game;
use sf_strat::player::{install, sgen_slspark, sgen_spark};

fn active_count(g: &Game) -> usize {
    (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count()
}

#[test]
fn sgen_spark_spawns_when_allowed() {
    let mut g = Game::new();
    let _ids = install(&mut g);

    let at = g.objs.alloc().expect("slot");
    g.objs.aliens[at as usize].worldx = 100;
    g.objs.aliens[at as usize].worldy = 50;
    g.objs.aliens[at as usize].worldz = 200;
    g.vars.pshipflags2 = 0; // sparks allowed

    let before = active_count(&g);
    sgen_spark(&mut g, at);
    assert_eq!(active_count(&g), before + 1);

    let spark = (0..NUMBER_AL as u16)
        .find(|&i| i != at && g.objs.aliens[i as usize].active)
        .expect("spark");
    let al = &g.objs.aliens[spark as usize];
    assert_eq!(al.worldx, 100);
    assert_eq!(al.worldy, 50);
    assert_eq!(al.worldz, 200);
    assert_eq!(al.count, 5);
    assert_eq!(al.vel, 15);
    assert_eq!(al.sflags & ASF_COLLDISABLE, 0);
    assert_ne!(al.sflags2 & ASF2_COLLDISABLE, 0);
    assert!(al.stratptr.is_some());
}

#[test]
fn sgen_spark_skipped_when_nospark() {
    let mut g = Game::new();
    let _ids = install(&mut g);
    let at = g.objs.alloc().expect("slot");
    g.vars.pshipflags2 = 4; // PSF2_NOSPARK
    let before = active_count(&g);
    sgen_spark(&mut g, at);
    assert_eq!(active_count(&g), before);
}

#[test]
fn sgen_slspark_spawns_faster_variant() {
    let mut g = Game::new();
    let _ids = install(&mut g);
    let at = g.objs.alloc().expect("slot");
    g.objs.aliens[at as usize].worldx = 10;
    let before = active_count(&g);
    sgen_slspark(&mut g, at);
    assert_eq!(active_count(&g), before + 1);
    let spark = (0..NUMBER_AL as u16)
        .find(|&i| i != at && g.objs.aliens[i as usize].active)
        .expect("spark");
    assert_eq!(g.objs.aliens[spark as usize].vel, 20);
}
