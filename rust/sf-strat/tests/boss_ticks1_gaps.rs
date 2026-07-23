//! Tick 142: AUDIT_BOSS_TICKS remaining gaps — bossgs flicker/fchase +
//! bossg `.genspark` (D2STRATS.ASM:343-352 / 481-488).

use sf_game::alien::{ASF3_REALOBJ, NUMBER_AL};
use sf_game::Game;
use sf_strat::bosses::{bossg_genspark, bossgs_strat, strat_bossg_init};
use sf_strat::player;

const COLTAB_BLACK_C: u16 = 6;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldz = z;
    al.worldx = 0;
    al.worldy = -40;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn active_count(g: &Game) -> usize {
    (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count()
}

/// Minor #9: odd gameframe → BLACK_C; even → clear.
#[test]
fn bossgs_flickers_black_c_on_odd_frames() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sbyte1 = 40;
    g.objs.aliens[idx as usize].sword1 = 0;
    g.objs.aliens[idx as usize].worldx = 0;

    g.vars.gameframe = 1;
    bossgs_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].coltab, COLTAB_BLACK_C);

    g.vars.gameframe = 2;
    bossgs_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].coltab, 0);
}

/// Minor #10: Fchase_A ±5 with no overshoot clamp (oscillates past target).
#[test]
fn bossgs_x_chase_overshoots_without_clamp() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sbyte1 = 40;
    g.objs.aliens[idx as usize].sword1 = 3; // target within one step
    g.objs.aliens[idx as usize].worldx = 0;
    g.vars.gameframe = 0; // even → clear coltab path

    bossgs_strat(&mut g, idx);
    // ROM: worldx < sword1 → +=5 → lands at 5, not clamped to 3.
    assert_eq!(g.objs.aliens[idx as usize].worldx, 5);

    bossgs_strat(&mut g, idx);
    // worldx > sword1 → -=5 → back to 0 (oscillation).
    assert_eq!(g.objs.aliens[idx as usize].worldx, 0);
}

/// Known gap: `.genspark` spawns lspark at boss pos with worldy−60.
#[test]
fn bossg_genspark_spawns_spark_at_y_minus_60() {
    let mut g = Game::new();
    let _ = player::install(&mut g);
    spawn_player(&mut g, 0);
    g.vars.pshipflags2 = 0; // sparks allowed

    let boss = spawn(&mut g);
    strat_bossg_init(&mut g, boss);
    g.objs.aliens[boss as usize].worldx = 100;
    g.objs.aliens[boss as usize].worldy = -200;
    g.objs.aliens[boss as usize].worldz = 500;

    let before = active_count(&g);
    bossg_genspark(&mut g, boss);
    assert_eq!(active_count(&g), before + 1, "sgenspark spawned");
    // Boss y restored (dummy-copy semantics).
    assert_eq!(g.objs.aliens[boss as usize].worldy, -200);

    let spark = (0..NUMBER_AL as u16)
        .find(|&i| {
            i != boss
                && i != 0
                && g.objs.aliens[i as usize].active
                && g.objs.aliens[i as usize].count == 5
        })
        .expect("lspark");
    let al = &g.objs.aliens[spark as usize];
    assert_eq!(al.worldx, 100);
    assert_eq!(al.worldy, -260); // −200 − 60
    assert_eq!(al.worldz, 500);
    assert_eq!(al.vel, 15);
}

/// nospark flag skips genspark spawn (sgenspark_srou gate).
#[test]
fn bossg_genspark_respects_nospark() {
    let mut g = Game::new();
    let _ = player::install(&mut g);
    spawn_player(&mut g, 0);
    g.vars.pshipflags2 = 4; // PSF2_NOSPARK
    let boss = spawn(&mut g);
    strat_bossg_init(&mut g, boss);
    let before = active_count(&g);
    bossg_genspark(&mut g, boss);
    assert_eq!(active_count(&g), before);
}
