//! Tick 79: CLSHIP1–3 / TURN2 / dive·under boost + floatCLship +
//! up1manchild1–3 + firenormringlaser + boss2spark.

use sf_game::alien::{ASF_COLLDISABLE, ASF_SHADOW, ATZREMOVE};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::bosses::{boss2spark_istrat, boss2spark_srou, boss2spark_strat};
use sf_strat::enemy_a::{
    clship1_istrat, clship1_strat, clship2_istrat, clship3_istrat, clship_dive_cont2_pub,
    clship_diveboost_istrat, clship_turn2_istrat, clship_turn2_strat, clship_underboost_istrat,
    clship_underboost_strat, fire_ringlaser, firenormringlaser, float_clship, float_clship2,
    up1manchild1_istrat, up1manchild2_istrat, up1manchild3_istrat, ASF2_SFLAG1, ASF3_SFLAG6, DEG5,
    DEG90,
};

/// ROM mother/child link is index+1 in `al_ptr` / `al_sword1`.
fn obj_link(idx: u16) -> u16 {
    idx.wrapping_add(1)
}

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
    g.objs.aliens[idx as usize].worldz = 500;
    g.objs.aliens[idx as usize].worldy = -100;
    g.objs.aliens[idx as usize].worldx = 200;
    idx
}

#[test]
fn clship123_turn2_dive_under_float() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let s1 = spawn_obj(&mut g);
    clship1_istrat(&mut g, s1);
    assert_ne!(g.objs.aliens[s1 as usize].sflags & ASF_SHADOW, 0);
    assert_eq!(g.objs.aliens[s1 as usize].rotz, DEG90.wrapping_neg());
    assert_eq!(g.objs.aliens[s1 as usize].sflags2 & ASF2_SFLAG1, 0); // no SHIPa latch
    let x0 = g.objs.aliens[s1 as usize].worldx;
    clship1_strat(&mut g, s1);
    // worldx chases toward -50
    assert!(g.objs.aliens[s1 as usize].worldx != x0 || x0 == -50);

    let s2 = spawn_obj(&mut g);
    clship2_istrat(&mut g, s2);
    assert_eq!(g.objs.aliens[s2 as usize].rotz, DEG90);

    let s3 = spawn_obj(&mut g);
    clship3_istrat(&mut g, s3);
    assert_eq!(g.objs.aliens[s3 as usize].rotz, 0);

    let t2 = spawn_obj(&mut g);
    g.objs.aliens[t2 as usize].vel = 10;
    clship_turn2_istrat(&mut g, t2);
    assert_eq!(g.objs.aliens[t2 as usize].sbyte1, ((128u16 + 42) / 4) as u8);
    assert_ne!(g.objs.aliens[t2 as usize].type_ & ATZREMOVE, 0);
    let rz0 = g.objs.aliens[t2 as usize].rotz;
    clship_turn2_strat(&mut g, t2);
    assert_eq!(g.objs.aliens[t2 as usize].rotz, rz0.wrapping_add(2));

    let dive = spawn_obj(&mut g);
    g.objs.aliens[dive as usize].rotz = 40;
    g.objs.aliens[dive as usize].rotx = 0;
    g.objs.aliens[dive as usize].sbyte2 = 5;
    clship_diveboost_istrat(&mut g, dive);
    assert_eq!(g.objs.aliens[dive as usize].rotz, 0);
    assert_eq!(g.objs.aliens[dive as usize].rotx, DEG5);
    assert_eq!(g.objs.aliens[dive as usize].vel, 120);

    let under = spawn_obj(&mut g);
    g.objs.aliens[under as usize].sbyte1 = 1;
    g.objs.aliens[under as usize].vel = 0;
    clship_underboost_istrat(&mut g, under);
    clship_underboost_strat(&mut g, under);
    assert!(g.objs.aliens[under as usize].vel > 0);

    // dive_cont2 with sword1>0 just decrements (+ optional scroll)
    let d2 = spawn_obj(&mut g);
    g.objs.aliens[d2 as usize].sword1 = 3;
    g.vars.psvar_word2 = 40;
    clship_dive_cont2_pub(&mut g, d2);
    assert_eq!(g.objs.aliens[d2 as usize].sword1, 2);
    assert_eq!(g.objs.aliens[d2 as usize].worldz, 500i16.wrapping_add(40));

    // floatCLship sets rotz/worldy; floatCLship2 adds deltas
    let f = spawn_obj(&mut g);
    g.objs.aliens[f as usize].sbyte3 = 0;
    g.objs.aliens[f as usize].sbyte4 = 0;
    g.objs.aliens[f as usize].rotz = 0;
    g.objs.aliens[f as usize].worldy = 0;
    float_clship(&mut g, f);
    assert_eq!(g.objs.aliens[f as usize].sbyte3, 1);
    assert_eq!(g.objs.aliens[f as usize].rotz, 2); // tab[1]=1 << 1
    assert_eq!(g.objs.aliens[f as usize].worldy, 2); // tab word after <<1 index

    let f2 = spawn_obj(&mut g);
    g.objs.aliens[f2 as usize].sbyte3 = 0;
    g.objs.aliens[f2 as usize].sbyte4 = 0;
    g.objs.aliens[f2 as usize].rotz = 10;
    g.objs.aliens[f2 as usize].worldy = 100;
    float_clship2(&mut g, f2);
    // tab[1]=1 >> 2 = 0; worldy tab>>2 may be 0 early — rotz unchanged or +0
    assert_eq!(g.objs.aliens[f2 as usize].sbyte3, 1);
}

#[test]
fn up1manchild_firenorm_boss2spark() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let mother = spawn_obj(&mut g);
    g.objs.aliens[mother as usize].rotz = 0;
    g.objs.aliens[mother as usize].worldx = 0;
    g.objs.aliens[mother as usize].worldy = 0;
    g.objs.aliens[mother as usize].worldz = 1000;

    let c1 = spawn_obj(&mut g);
    g.objs.aliens[c1 as usize].ptr = obj_link(mother);
    g.objs.aliens[c1 as usize].sflags4 |= sf_game::alien::ASF4_CHILDOBJ;
    up1manchild1_istrat(&mut g, c1);
    assert_eq!(g.objs.aliens[c1 as usize].hp, HARD_HP);
    assert_eq!(g.objs.aliens[c1 as usize].sbyte2 as i8, -80);
    assert_eq!(g.objs.aliens[c1 as usize].sbyte3 as i8, 75);
    // child parked via rotate_8yx(rotz=0): mulslog loses 1 vs exact −80/75
    assert_eq!(g.objs.aliens[c1 as usize].worldx, -79);
    assert_eq!(g.objs.aliens[c1 as usize].worldy, 74);

    let c2 = spawn_obj(&mut g);
    g.objs.aliens[c2 as usize].ptr = obj_link(mother);
    g.objs.aliens[c2 as usize].sflags4 |= sf_game::alien::ASF4_CHILDOBJ;
    up1manchild2_istrat(&mut g, c2);
    assert_eq!(g.objs.aliens[c2 as usize].sbyte2 as i8, 80);

    let c3 = spawn_obj(&mut g);
    g.objs.aliens[c3 as usize].ptr = obj_link(mother);
    g.objs.aliens[c3 as usize].sflags4 |= sf_game::alien::ASF4_CHILDOBJ;
    up1manchild3_istrat(&mut g, c3);
    assert_eq!(g.objs.aliens[c3 as usize].sbyte3 as i8, -90);
    assert_eq!(g.objs.aliens[c3 as usize].sbyte4, 0);

    let firer = spawn_obj(&mut g);
    let slow = fire_ringlaser(&mut g, firer).expect("ring");
    let fast = firenormringlaser(&mut g, firer).expect("normring");
    assert_eq!(g.objs.aliens[slow as usize].vel, 70);
    assert_eq!(g.objs.aliens[fast as usize].vel, 120);

    let host = spawn_obj(&mut g);
    let spark = spawn_obj(&mut g);
    g.objs.aliens[spark as usize].sword1 = obj_link(host) as i16;
    boss2spark_istrat(&mut g, spark);
    assert_ne!(g.objs.aliens[spark as usize].sflags & ASF_COLLDISABLE, 0);
    boss2spark_srou(&mut g, spark); // no-op

    // Advance sbyte1 via delay-3 frames until ≥10, then fire path runs.
    g.vars.gameframe = 0;
    for _ in 0..80 {
        boss2spark_strat(&mut g, spark);
        g.vars.gameframe = g.vars.gameframe.wrapping_add(1);
    }
    assert!(g.objs.aliens[spark as usize].sbyte1 > 0);
    // Host sflag6 → spark removes
    g.objs.aliens[host as usize].sflags3 |= ASF3_SFLAG6;
    g.objs.aldead = 0;
    boss2spark_strat(&mut g, spark);
    assert_eq!(g.objs.aldead, 1);
}
