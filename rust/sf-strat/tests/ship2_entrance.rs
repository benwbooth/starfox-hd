//! Tick 81: ship2 entrance family (into / outside / cont / fire_cont).

use sf_game::alien::ATGND;
use sf_game::vars::{
    GF_STRATDONE1, GF_STRATDONE2, HARD_HP, PSF2_PLAYERHP0, PSF_NOCTRL, PSF_NOFIRE, PSTF_NOTDIE,
};
use sf_game::Game;
use sf_strat::enemy_a::{
    ship2_cont, ship2_istrat, ship2_strat, ship2fire_cont, ship2into_init, ship2into_strat,
    ship2outside_init, ship2outside_strat, AF_LEFT_PL, DEG180, MEDPSPEED_I16,
};

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
fn ship2_init_cont_outside() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.gameflags |= GF_STRATDONE1 | GF_STRATDONE2;

    let s = spawn_obj(&mut g);
    ship2_istrat(&mut g, s);
    assert_eq!(g.objs.aliens[s as usize].hp, HARD_HP);
    assert_eq!(g.objs.aliens[s as usize].ap, 16);
    assert_eq!(g.objs.aliens[s as usize].roty, DEG180);
    assert_eq!(g.objs.aliens[s as usize].vz, -40);
    assert_eq!(g.objs.aliens[s as usize].sbyte1, 39);
    assert_ne!(g.objs.aliens[s as usize].type_ & ATGND, 0);
    assert_eq!(g.vars.gameflags & (GF_STRATDONE1 | GF_STRATDONE2), 0);

    // Far + on-axis → fire_cont (= cont): vz applied
    let z0 = g.objs.aliens[s as usize].worldz;
    ship2_strat(&mut g, s);
    assert_eq!(
        g.objs.aliens[s as usize].worldz,
        z0.wrapping_add(-40) // vz only; pviewvelz=0
    );

    // Cont alias
    ship2fire_cont(&mut g, s);
    ship2_cont(&mut g, s);

    // Outside peel: right of view (leftpl clear) → vx += 5
    let o = spawn_obj(&mut g);
    g.objs.aliens[o as usize].flags &= !AF_LEFT_PL;
    g.objs.aliens[o as usize].vx = 0;
    ship2outside_init(&mut g, o);
    assert_ne!(g.vars.gameflags & GF_STRATDONE2, 0);
    ship2outside_strat(&mut g, o);
    assert_eq!(g.objs.aliens[o as usize].vx, 5);

    // Left of view → vx -= 5
    let o2 = spawn_obj(&mut g);
    g.objs.aliens[o2 as usize].flags |= AF_LEFT_PL;
    g.objs.aliens[o2 as usize].vx = 0;
    ship2outside_strat(&mut g, o2);
    assert_eq!(g.objs.aliens[o2 as usize].vx, -5);
}

#[test]
fn ship2_into_and_branch() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    // Into init latches notdie
    let s = spawn_obj(&mut g);
    ship2into_init(&mut g, s);
    assert_ne!(g.vars.pstratflags & PSTF_NOTDIE, 0);

    // Into strat: far enough to guide (dz=5000) → lock ctrl + medpspeed
    g.objs.aliens[s as usize].worldz = 1500;
    g.objs.aliens[0].worldz = 0; // dz=1500 > 300
    ship2into_strat(&mut g, s);
    assert_eq!(g.vars.pviewvelz, MEDPSPEED_I16);
    assert_ne!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);

    // Close → doneguide: stratdone1, ctrl on, vz=-40
    g.objs.aliens[s as usize].worldz = 200;
    g.objs.aliens[0].worldz = 0;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.gameflags &= !GF_STRATDONE1;
    ship2into_strat(&mut g, s);
    assert_ne!(g.vars.gameflags & GF_STRATDONE1, 0);
    assert_eq!(g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE), 0);
    assert_eq!(g.objs.aliens[s as usize].vz, -40);

    // Player HP0 during into → outside
    let s2 = spawn_obj(&mut g);
    ship2into_init(&mut g, s2);
    g.vars.pshipflags2 |= PSF2_PLAYERHP0;
    g.vars.gameflags &= !GF_STRATDONE2;
    ship2into_strat(&mut g, s2);
    assert_ne!(g.vars.gameflags & GF_STRATDONE2, 0);

    // Main strat: on-axis + close → into
    g.vars.pshipflags2 = 0;
    let s3 = spawn_obj(&mut g);
    ship2_istrat(&mut g, s3);
    g.objs.aliens[s3 as usize].worldx = 0;
    g.objs.aliens[s3 as usize].worldz = 1000; // |dz|<2000, |dx|<120
    g.objs.aliens[0].worldz = 0;
    g.objs.aliens[0].worldx = 0;
    ship2_strat(&mut g, s3);
    assert_ne!(g.vars.pstratflags & PSTF_NOTDIE, 0);

    // Off-axis + close → outside
    let s4 = spawn_obj(&mut g);
    ship2_istrat(&mut g, s4);
    g.objs.aliens[s4 as usize].worldx = 200; // |dx|>=120
    g.objs.aliens[s4 as usize].worldz = 1000;
    g.vars.gameflags &= !GF_STRATDONE2;
    ship2_strat(&mut g, s4);
    assert_ne!(g.vars.gameflags & GF_STRATDONE2, 0);
}
