//! Tick 192: bossA cup GO/IROTATE/return + turret husk revive
//! (AUDIT_ENEMY_B Criticals #3–#9 verify).

use sf_game::alien::{ASF4_INVISIBLE, ASF_NOHITAFFECT};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemy_a::boss_attach_child_to_mother;
use sf_strat::enemy_b::{
    bossa_cup_strat, bossa_turret_exp_init, bossaattack_init, bossaattack_strat,
    bossacupperl_istrat, bossaturretl_istrat, BOSSA_CUP_STATE_DOWN, BOSSA_CUP_STATE_GO,
    BOSSA_CUP_STATE_IROTATE, BOSSA_CUP_STATE_RETURN, BOSSA_CUP_STATE_UP,
};

const BOSSA_SCALE: i16 = 2;
const BOSSA_PARENT_FLAG_ATTACK_DONE: u8 = 0x10;
const BOSSA_TURRET_HP: u8 = 12;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = 0;
    al.worldy = 0;
    al.worldz = z;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn active_count(g: &Game) -> usize {
    g.objs.aliens.iter().filter(|a| a.active).count()
}

/// Critical #3: GO returns home when cup overshoots player (me.z < pl.z, |dz|>=200).
#[test]
fn bossa_cup_go_returns_when_past_player() {
    let mut g = Game::new();
    spawn_player(&mut g, 1500);
    let mother = spawn(&mut g);
    let turret = spawn(&mut g);
    let cup = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, turret, 1));
    assert!(boss_attach_child_to_mother(&mut g, mother, cup, 4));
    bossacupperl_istrat(&mut g, cup);

    g.objs.aliens[cup as usize].stratstate = BOSSA_CUP_STATE_GO;
    g.objs.aliens[cup as usize].sflags &= !ASF_NOHITAFFECT;
    g.objs.aliens[cup as usize].worldz = 1000; // behind player by 500
    g.objs.aliens[cup as usize].vel = 45;

    bossa_cup_strat(&mut g, cup);
    assert_eq!(
        g.objs.aliens[cup as usize].stratstate,
        BOSSA_CUP_STATE_RETURN
    );
    assert_eq!(g.objs.aliens[cup as usize].worldy, -(100i16 << BOSSA_SCALE));
}

/// Critical #3 complement: still ahead of player → stay in GO.
#[test]
fn bossa_cup_go_stays_while_ahead_of_player() {
    let mut g = Game::new();
    spawn_player(&mut g, 1000);
    let mother = spawn(&mut g);
    let turret = spawn(&mut g);
    let cup = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, turret, 1));
    assert!(boss_attach_child_to_mother(&mut g, mother, cup, 4));
    bossacupperl_istrat(&mut g, cup);

    g.objs.aliens[cup as usize].stratstate = BOSSA_CUP_STATE_GO;
    g.objs.aliens[cup as usize].sflags &= !ASF_NOHITAFFECT;
    g.objs.aliens[cup as usize].worldz = 2000; // ahead
    g.objs.aliens[cup as usize].vel = 45;

    bossa_cup_strat(&mut g, cup);
    assert_eq!(g.objs.aliens[cup as usize].stratstate, BOSSA_CUP_STATE_GO);
}

/// Critical #4: GO never fires (bossacupfire* have no ROM callers in GO).
#[test]
fn bossa_cup_go_does_not_fire() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn(&mut g);
    let turret = spawn(&mut g);
    let cup = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, turret, 1));
    assert!(boss_attach_child_to_mother(&mut g, mother, cup, 4));
    bossacupperl_istrat(&mut g, cup);

    g.objs.aliens[cup as usize].stratstate = BOSSA_CUP_STATE_GO;
    g.objs.aliens[cup as usize].sflags &= !ASF_NOHITAFFECT;
    g.objs.aliens[cup as usize].worldz = 2000;
    g.objs.aliens[cup as usize].vel = 45;

    let before = active_count(&g);
    bossa_cup_strat(&mut g, cup);
    assert_eq!(active_count(&g), before, "GO must not spawn weapons");
}

/// Criticals #8–#9: last cup (2 dead) → GO; otherwise IROTATE.
#[test]
fn bossa_attack_go_only_when_last_cup() {
    // One live cup → dead_cups==2 → GO.
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn(&mut g);
    g.objs.aliens[mother as usize].worldx = 300;
    let cup = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, cup, 4));
    bossacupperl_istrat(&mut g, cup);
    g.objs.aliens[cup as usize].stratstate = BOSSA_CUP_STATE_UP;

    bossaattack_init(&mut g, mother);
    assert_ne!(
        g.objs.aliens[mother as usize].sflags4 & BOSSA_PARENT_FLAG_ATTACK_DONE,
        0
    );
    bossaattack_strat(&mut g, mother);
    assert_eq!(g.objs.aliens[cup as usize].stratstate, BOSSA_CUP_STATE_GO);

    // Three live cups → IROTATE (not GO).
    let mut g2 = Game::new();
    spawn_player(&mut g2, 0);
    let mother2 = spawn(&mut g2);
    g2.objs.aliens[mother2 as usize].worldx = 300;
    let c4 = spawn(&mut g2);
    let c5 = spawn(&mut g2);
    let c6 = spawn(&mut g2);
    assert!(boss_attach_child_to_mother(&mut g2, mother2, c4, 4));
    assert!(boss_attach_child_to_mother(&mut g2, mother2, c5, 5));
    assert!(boss_attach_child_to_mother(&mut g2, mother2, c6, 6));
    bossacupperl_istrat(&mut g2, c4);
    // Mark all UP so dispatcher can reassign one.
    for c in [c4, c5, c6] {
        g2.objs.aliens[c as usize].stratstate = BOSSA_CUP_STATE_UP;
        g2.objs.aliens[c as usize].stratptr = g2.objs.aliens[c4 as usize].stratptr;
    }
    bossaattack_init(&mut g2, mother2);
    bossaattack_strat(&mut g2, mother2);
    let states: Vec<u8> = [c4, c5, c6]
        .iter()
        .map(|&c| g2.objs.aliens[c as usize].stratstate)
        .collect();
    assert!(
        states.iter().any(|&s| s == BOSSA_CUP_STATE_IROTATE),
        "with 3 cups, dispatcher must pick IROTATE, got {states:?}"
    );
    assert!(
        !states.iter().any(|&s| s == BOSSA_CUP_STATE_GO),
        "GO only when last cup"
    );
}

/// Critical #7: turret death → invisible husk + mother.sbyte3++; DOWN revives.
#[test]
fn bossa_turret_husk_and_down_revive() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn(&mut g);
    let turret = spawn(&mut g);
    let cup = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, turret, 1));
    assert!(boss_attach_child_to_mother(&mut g, mother, cup, 4));
    bossaturretl_istrat(&mut g, turret);
    bossacupperl_istrat(&mut g, cup);

    assert_eq!(g.objs.aliens[mother as usize].sbyte3, 0);
    bossa_turret_exp_init(&mut g, turret);
    assert!(
        g.objs.aliens[turret as usize].active,
        "husk stays allocated"
    );
    assert_ne!(g.objs.aliens[turret as usize].sflags4 & ASF4_INVISIBLE, 0);
    assert_eq!(g.objs.aliens[turret as usize].hp, HARD_HP);
    assert_eq!(g.objs.aliens[mother as usize].sbyte3, 1);

    // DOWN revive: cup at/above turret.y - 20<<scale.
    let ty = g.objs.aliens[turret as usize].worldy;
    g.objs.aliens[cup as usize].stratstate = BOSSA_CUP_STATE_DOWN;
    g.objs.aliens[cup as usize].worldy = ty; // >= thresh
    g.objs.aliens[cup as usize].worldx = g.objs.aliens[turret as usize].worldx;
    g.objs.aliens[cup as usize].worldz = g.objs.aliens[turret as usize].worldz;

    bossa_cup_strat(&mut g, cup);
    assert_eq!(g.objs.aliens[turret as usize].sflags4 & ASF4_INVISIBLE, 0);
    assert_eq!(g.objs.aliens[turret as usize].hp, BOSSA_TURRET_HP);
    assert_eq!(g.objs.aliens[mother as usize].sbyte3, 0);
}
