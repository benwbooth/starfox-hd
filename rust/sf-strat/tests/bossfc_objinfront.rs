//! Tick 191: bossFC2/FC3 objinfront + playerturn180 + bossFtur fire window
//! (AUDIT_ENEMY_B Criticals #1–#2, High #12).

use sf_game::vars::PSF2_PLAYERHP0;
use sf_game::Game;
use sf_strat::enemy_a::boss_attach_child_to_mother;
use sf_strat::enemy_b::{
    bossfb_istrat, bossfb_strat, bossfc2_strat, bossfc3_strat, bossftur1_istrat, bossftur1_strat,
};

const SPACE_VIEWCY: i16 = -60;
const ASF2_SFLAG1: u8 = 0x10;

fn spawn_player(g: &mut Game, z: i16) -> u16 {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = 0;
    al.worldy = 0;
    al.worldz = z;
    p
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn active_count(g: &Game) -> usize {
    g.objs.aliens.iter().filter(|a| a.active).count()
}

#[test]
fn bossfb_drops_the_source_mine_shape() {
    const MINE_SHAPE: u16 = 299;

    let mut g = Game::new();
    let _player = spawn_player(&mut g, 0);
    let mother = spawn(&mut g);
    let boss = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, boss, 1));
    bossfb_istrat(&mut g, boss);
    g.objs.aliens[boss as usize].worldz = 1000;
    g.vars.gameframe = 0;

    bossfb_strat(&mut g, boss);

    assert!(g
        .objs
        .aliens
        .iter()
        .any(|object| object.active && object.shape == MINE_SHAPE));
}

/// FC2: `s_jmp_objinfront x,y` — turn block when me.z < pl.z (boss behind).
#[test]
fn bossfc2_objinfront_arms_playerturn180() {
    let mut g = Game::new();
    let _p = spawn_player(&mut g, 5000);
    let boss = spawn(&mut g);
    g.objs.aliens[boss as usize].worldz = 2500; // behind by 2500
    g.objs.aliens[boss as usize].worldy = 0;
    g.objs.aliens[boss as usize].sflags2 = 0;
    g.objs.aliens[boss as usize].sbyte2 = 0; // < 3 → skip shoot path

    let before = g.objs.aliens[0].stratptr;
    bossfc2_strat(&mut g, boss);

    assert_ne!(
        g.objs.aliens[boss as usize].sflags2 & ASF2_SFLAG1,
        0,
        "sflag1 latched"
    );
    assert_eq!(g.objs.aliens[boss as usize].worldy, SPACE_VIEWCY - 500);
    assert!(g.objs.aliens[0].stratptr.is_some());
    assert_ne!(g.objs.aliens[0].stratptr, before);
}

#[test]
fn bossfc2_objinfront_skips_when_boss_ahead() {
    let mut g = Game::new();
    let _p = spawn_player(&mut g, 2500);
    let boss = spawn(&mut g);
    g.objs.aliens[boss as usize].worldz = 5000; // ahead
    g.objs.aliens[boss as usize].sflags2 = 0;
    g.objs.aliens[boss as usize].sbyte2 = 0;

    bossfc2_strat(&mut g, boss);
    assert_eq!(g.objs.aliens[boss as usize].sflags2 & ASF2_SFLAG1, 0);
    assert!(g.objs.aliens[0].stratptr.is_none());
}

#[test]
fn bossfc2_skips_turn180_when_player_dead() {
    let mut g = Game::new();
    let _p = spawn_player(&mut g, 5000);
    g.vars.pshipflags2 |= PSF2_PLAYERHP0;
    let boss = spawn(&mut g);
    g.objs.aliens[boss as usize].worldz = 2500;
    g.objs.aliens[boss as usize].sbyte2 = 0;

    bossfc2_strat(&mut g, boss);
    assert_ne!(g.objs.aliens[boss as usize].sflags2 & ASF2_SFLAG1, 0);
    assert!(
        g.objs.aliens[0].stratptr.is_none(),
        "psf2_playerHP0 gates turn180"
    );
}

/// FC3: args swapped — turn when pl.z < me.z (player behind after reverse).
#[test]
fn bossfc3_objinfront_swapped_args() {
    let mut g = Game::new();
    let _p = spawn_player(&mut g, 2500);
    let boss = spawn(&mut g);
    g.objs.aliens[boss as usize].worldz = 5000;
    g.objs.aliens[boss as usize].sflags2 = 0;
    g.objs.aliens[boss as usize].sbyte2 = 0;

    bossfc3_strat(&mut g, boss);
    assert_ne!(g.objs.aliens[boss as usize].sflags2 & ASF2_SFLAG1, 0);
    assert!(g.objs.aliens[0].stratptr.is_some());
    assert_eq!(g.objs.aliens[boss as usize].worldy, SPACE_VIEWCY - 500);
}

/// High #12: fire only while sbyte2 <= 15 (open-window first half).
#[test]
fn bossftur_fires_only_when_sbyte2_le_15() {
    let mut g = Game::new();
    let _p = spawn_player(&mut g, 0);
    let mother = spawn(&mut g);
    g.objs.aliens[mother as usize].worldz = 3000;
    let tur = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, tur, 1));
    bossftur1_istrat(&mut g, tur);

    g.objs.aliens[tur as usize].sflags2 |= ASF2_SFLAG1;
    g.objs.aliens[tur as usize].sbyte3 = 10; // <= 20
    g.objs.aliens[tur as usize].sbyte2 = 10; // <= 15 after dec
    g.vars.gameframe = 0; // &7 == 0

    let before = active_count(&g);
    bossftur1_strat(&mut g, tur);
    assert!(active_count(&g) > before, "sbyte2<=15 should spawn laser");

    let mut g2 = Game::new();
    let _p2 = spawn_player(&mut g2, 0);
    let mother2 = spawn(&mut g2);
    g2.objs.aliens[mother2 as usize].worldz = 3000;
    let tur2 = spawn(&mut g2);
    assert!(boss_attach_child_to_mother(&mut g2, mother2, tur2, 1));
    bossftur1_istrat(&mut g2, tur2);
    g2.objs.aliens[tur2 as usize].sflags2 |= ASF2_SFLAG1;
    g2.objs.aliens[tur2 as usize].sbyte3 = 10;
    g2.objs.aliens[tur2 as usize].sbyte2 = 20; // > 15 after dec
    g2.vars.gameframe = 0;
    let before2 = active_count(&g2);
    bossftur1_strat(&mut g2, tur2);
    assert_eq!(active_count(&g2), before2, "sbyte2>15 must skip fire");
}
