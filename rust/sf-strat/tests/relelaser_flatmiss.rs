//! ROM `relelaser` / `relflatmiss` / `flatmiss` + fire_friend/reb/plasma/beamball.

use sf_game::Game;
use sf_strat::enemy_a::{
    fire_beamball, fire_friend_elaser, fire_plasma, fire_reb_elaser, flatmiss_istrat,
    flatmiss_strat, relelaser_istrat, relelaser_strat, relflatmiss_istrat, relflatmiss_strat,
    ASF2_RELEXPLODE, ASF2_SFLAG1,
};

#[test]
fn relelaser_istrat_scales_vecs_and_animates() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("laser");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 66;
        al.sbyte1 = 0;
        al.sbyte2 = 0;
        al.sbyte3 = 20;
        al.count = 10;
        al.sflags2 |= ASF2_SFLAG1;
    }
    relelaser_istrat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 0);
    assert_eq!(g.objs.aliens[idx as usize].vel, 66);

    relelaser_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 2);
    assert_eq!(g.objs.aliens[idx as usize].count, 9);
}

#[test]
fn relelaser_expires_without_numplasers() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("laser");
    g.objs.aliens[idx as usize].vel = 60;
    g.objs.aliens[idx as usize].sbyte3 = 0;
    g.objs.aliens[idx as usize].count = 1;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
    relelaser_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].count = 1;
    relelaser_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn relflatmiss_scrolls_and_kills_on_life() {
    let mut g = Game::new();
    g.vars.pviewvelz = 5;
    let idx = g.objs.alloc().expect("plasma");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 80;
        al.roty = 0;
        al.rotx = 0;
        al.count = 1;
        al.worldz = 100;
        al.sflags2 |= ASF2_SFLAG1;
    }
    relflatmiss_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].snd2, 6);
    relflatmiss_strat(&mut g, idx);
    // scrolled +5 then killed
    assert_eq!(g.objs.aliens[idx as usize].hp, 0);
    assert!(g.objs.aliens[idx as usize].worldz >= 105);
}

#[test]
fn flatmiss_does_not_add_player_z() {
    let mut g = Game::new();
    g.vars.pviewvelz = 12;
    let idx = g.objs.alloc().expect("ball");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 70;
        al.count = 5;
        al.worldz = 200;
        al.sflags2 |= ASF2_SFLAG1;
    }
    flatmiss_istrat(&mut g, idx);
    let z0 = g.objs.aliens[idx as usize].worldz;
    flatmiss_strat(&mut g, idx);
    // Only velocity applied — no +pviewvelz
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        z0.wrapping_add(g.objs.aliens[idx as usize].vz)
    );
    assert_eq!(g.objs.aliens[idx as usize].count, 4);
}

#[test]
fn fire_friend_and_reb_elaser_stats() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("firer");
    g.objs.aliens[firer as usize].vel = 40;

    let friend = fire_friend_elaser(&mut g, firer).expect("friend");
    assert_eq!(g.objs.aliens[friend as usize].ap, 2);
    assert_eq!(g.objs.aliens[friend as usize].vel, 66);
    assert_eq!(g.objs.aliens[friend as usize].count, 10);
    assert!(g.objs.aliens[friend as usize].stratptr.is_some());

    let reb = fire_reb_elaser(&mut g, firer).expect("reb");
    assert_eq!(g.objs.aliens[reb as usize].ap, 2);
    assert_eq!(g.objs.aliens[reb as usize].vel, 60);
    assert_eq!(g.objs.aliens[reb as usize].count, 40);
}

#[test]
fn fire_plasma_and_beamball_stats() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("firer");
    g.objs.aliens[firer as usize].vel = 30;

    let plasma = fire_plasma(&mut g, firer).expect("plasma");
    assert_eq!(g.objs.aliens[plasma as usize].ap, 10);
    assert_eq!(g.objs.aliens[plasma as usize].vel, 80);
    assert_eq!(g.objs.aliens[plasma as usize].count, 100);
    assert_ne!(g.objs.aliens[plasma as usize].sflags2 & ASF2_RELEXPLODE, 0);

    let ball = fire_beamball(&mut g, firer).expect("ball");
    assert_eq!(g.objs.aliens[ball as usize].ap, 8);
    assert_eq!(g.objs.aliens[ball as usize].vel, 70);
}
