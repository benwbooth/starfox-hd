//! Tick 215: bossB `bossB_cont` image trail (GB3STRAT.ASM:1283-1301) —
//! every-other-frame `bossBent` / `bossBspinend` spawn; sword1 hi slot.

use sf_game::alien::{ASF2_COLLDISABLE, ASF3_REALOBJ};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bossb::{bossb_cont, bossbent_istrat, bossbent_strat, bossbspinend_istrat};
use sf_strat::bosses::flingboss_arm_init;

const ASF2_SFLAG1: u8 = 0x10;
const ASF2_SFLAG2: u8 = 0x20;
const ASF2_SFLAG4: u8 = 0x80;

fn spawn_player(g: &mut Game) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.internal_playpt = 0;
}

fn spawn_boss(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("b");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    al.active = true;
    al.worldx = 10;
    al.worldy = -40;
    al.worldz = 2000;
    al.roty = 32;
    al.hp = 40;
    idx
}

fn count_active(g: &Game) -> usize {
    g.objs.aliens.iter().filter(|a| a.active).count()
}

/// sflag1 + sflag4 toggle: spawn only on the clear-after-toggle frame.
#[test]
fn bossb_cont_spawns_bent_every_other_frame() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let boss = spawn_boss(&mut g);
    g.objs.aliens[boss as usize].sflags2 |= ASF2_SFLAG1;
    g.objs.aliens[boss as usize].sflags2 &= !ASF2_SFLAG4;
    let before = count_active(&g);

    bossb_cont(&mut g, boss); // toggle 0→1 → skip
    assert_eq!(count_active(&g), before);
    assert_ne!(g.objs.aliens[boss as usize].sflags2 & ASF2_SFLAG4, 0);

    bossb_cont(&mut g, boss); // toggle 1→0 → spawn bossBent
    assert_eq!(count_active(&g), before + 1);
    assert_eq!(g.objs.aliens[boss as usize].sflags2 & ASF2_SFLAG4, 0);

    let trail = (0..g.objs.aliens.len())
        .find(|&i| i != 0 && i != boss as usize && g.objs.aliens[i].active)
        .expect("trail");
    assert_eq!(g.objs.aliens[trail].ptr, boss);
    assert_ne!(g.objs.aliens[trail].sflags2 & ASF2_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[trail].worldx, 10);
    assert_eq!(g.objs.aliens[trail].roty, 32);
    assert_eq!(g.objs.aliens[trail].count, 7); // istrat count=8, first strat −1
}

/// sflag2 → bossBspinend trail; mother sword1 hi increments & 3.
#[test]
fn bossb_cont_spinend_trail_advances_sword1_hi() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let boss = spawn_boss(&mut g);
    g.objs.aliens[boss as usize].sflags2 |= ASF2_SFLAG1 | ASF2_SFLAG2;
    g.objs.aliens[boss as usize].sflags2 |= ASF2_SFLAG4; // next toggle clears
    g.objs.aliens[boss as usize].sword1 = (2u16 << 8) as i16;

    bossb_cont(&mut g, boss);
    assert_eq!((g.objs.aliens[boss as usize].sword1 as u16) >> 8, 3);

    let trail = (0..g.objs.aliens.len())
        .find(|&i| i != 0 && i != boss as usize && g.objs.aliens[i].active)
        .expect("spinend trail");
    assert_eq!(g.objs.aliens[trail].ptr, boss);
    assert_eq!((g.objs.aliens[trail].sword1 as u16) >> 8, 2);
}

/// bossBent fades out after count ticks.
#[test]
fn bossbent_fades_after_lifecnt() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_boss(&mut g);
    bossbent_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 7);
    for _ in 0..8 {
        g.objs.aldead = 0;
        bossbent_strat(&mut g, idx);
        if g.objs.aldead != 0 {
            break;
        }
    }
    assert_eq!(g.objs.aldead, 1);
}

/// Direct spinend istrat wires hardHP + strat.
#[test]
fn bossbspinend_istrat_sets_hardhp() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let mother = spawn_boss(&mut g);
    let child = spawn_boss(&mut g);
    g.objs.aliens[child as usize].ptr = mother;
    g.objs.aliens[child as usize].sword1 = (1u16 << 8) as i16;
    bossbspinend_istrat(&mut g, child);
    assert_eq!(g.objs.aliens[child as usize].hp, 0xff);
    assert!(g.objs.aliens[child as usize].stratptr.is_some());
}

/// flingboss arm_init ENEMY1 = ACF_COLLTYPE2 (0x10).
#[test]
fn flingboss_arm_init_sets_enemy1_colltype2() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_boss(&mut g);
    flingboss_arm_init(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].collflags & 0x10, 0);
    assert_eq!(g.objs.aliens[idx as usize].collflags & 0x01, 0);
}
