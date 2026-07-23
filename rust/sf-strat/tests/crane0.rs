//! ROM crane0 + tzaco7 go/fall/cat (GA2STRAT.ASM:693-870).

use sf_game::alien::{ACF_COLLTYPE1, ACF_COLLTYPE5, ASF_COLLDISABLE, ASF_SHADOW};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemy_a::{
    crane0_istrat, crane0_strat, crane0col_istrat, tzaco7cat_istrat, tzaco7cat_strat,
    tzaco7fall_istrat, tzaco7go_istrat, tzaco7go_strat, COLLTYPE_ENEMY1, DEG90, MEDPSPEED_I16,
};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 3000;
    g.objs.aliens[idx as usize].worldy = -80;
    idx
}

#[test]
fn crane0_istrat_spawns_carried_zaco7() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    crane0_istrat(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.hp, HARD_HP);
    assert_eq!(al.ap, 2);
    assert_eq!(al.sbyte1, 6);
    assert_eq!(al.stratstate, 0);
    assert_ne!(al.sflags & ASF_SHADOW, 0);
    assert_ne!(al.collflags & COLLTYPE_ENEMY1, 0);
    assert!(al.stratptr.is_some());
    assert!(al.collstratptr.is_some());
    let child = al.ptr;
    assert_ne!(child, 0);
    let c = &g.objs.aliens[child as usize];
    assert!(c.active);
    assert_eq!(c.hp, 6);
    assert_eq!(c.ap, 8);
    assert_ne!(c.sflags & ASF_COLLDISABLE, 0);
    assert_ne!(c.sflags & ASF_SHADOW, 0);
    assert_ne!(c.collflags & COLLTYPE_ENEMY1, 0);
    assert_eq!(c.worldx, al.worldx);
    assert_eq!(c.worldz, al.worldz);
}

#[test]
fn crane0_state0_slides_then_enters_chase() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    crane0_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldz = 1000; // |dz|<1500
    let x0 = g.objs.aliens[idx as usize].worldx;
    crane0_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG90);
    assert_eq!(g.objs.aliens[idx as usize].worldx, x0.wrapping_sub(10));
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 5);
    // Burn remaining timer.
    g.objs.aliens[idx as usize].sbyte1 = 1;
    crane0_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    let den = (MEDPSPEED_I16 as i32 - 10).max(1);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, (6000 / den) as u8);
}

#[test]
fn crane0_releases_child_when_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    crane0_istrat(&mut g, idx);
    let child = g.objs.aliens[idx as usize].ptr;
    g.objs.aliens[idx as usize].worldz = 500; // |dz|<700
    crane0_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].ptr, 0);
    // go_istrat sets vy=5 then runs one strat tick (vy−1 → 4).
    assert_eq!(g.objs.aliens[child as usize].vy, 4);
    assert_eq!(g.objs.aliens[child as usize].vz, -20);
    assert_eq!(g.objs.aliens[child as usize].sflags & ASF_COLLDISABLE, 0);
    assert!(g.objs.aliens[child as usize].stratptr.is_some());
}

#[test]
fn crane0col_hf1_kills_child_hf2_drops() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    crane0_istrat(&mut g, idx);
    let child = g.objs.aliens[idx as usize].ptr;

    // Friend laser partner.
    let laser = g.objs.alloc().expect("laser");
    g.objs.aliens[laser as usize].active = true;
    g.objs.aliens[laser as usize].collflags = ACF_COLLTYPE1 | ACF_COLLTYPE5;
    g.objs.aliens[idx as usize].collobjptr = laser;

    g.objs.aliens[idx as usize].hitflags = 0x01; // HF1
    crane0col_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[child as usize].hp, 0);
    assert_ne!(g.objs.aliens[child as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[idx as usize].hitflags, 0);

    // Fresh crane for HF2 drop.
    let idx2 = spawn_obj(&mut g);
    crane0_istrat(&mut g, idx2);
    let child2 = g.objs.aliens[idx2 as usize].ptr;
    g.objs.aliens[idx2 as usize].collobjptr = laser;
    g.objs.aliens[idx2 as usize].hitflags = 0x02; // HF2
    g.objs.aliens[child2 as usize].worldy = -50;
    crane0col_istrat(&mut g, idx2);
    assert_eq!(g.objs.aliens[idx2 as usize].ptr, 0);
    assert!(g.objs.aliens[child2 as usize].stratptr.is_some());
    // Fall tick: vy += 5.
    assert_eq!(g.objs.aliens[child2 as usize].vy, 5);
}

#[test]
fn tzaco7go_anim_and_chase_y() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    tzaco7go_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vy, 4); // init set 5, first strat tick −1
    assert_eq!(g.objs.aliens[idx as usize].vz, -20);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 1);
    // Drain vy then chase player y.
    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].worldy = -80;
    g.objs.aliens[0].worldy = -40;
    let y0 = g.objs.aliens[idx as usize].worldy;
    tzaco7go_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].worldy > y0);
}

#[test]
fn tzaco7fall_explodes_at_ground() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldy = 0;
    tzaco7fall_istrat(&mut g, idx);
    // explode path: hp cleared / colldisable or aldead.
    let al = &g.objs.aliens[idx as usize];
    assert!(al.hp == 0 || al.sflags & ASF_COLLDISABLE != 0 || g.objs.aldead != 0);
}

#[test]
fn tzaco7cat_animates_when_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 200; // |dz|<500
    tzaco7cat_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].ap, 4);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1, 0);
    // istrat already ran one strat tick → anim 1.
    assert_eq!(g.objs.aliens[idx as usize].animframe, 1);
    tzaco7cat_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 2);
    // Far: no further anim.
    g.objs.aliens[idx as usize].worldz = 2000;
    tzaco7cat_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 2);
}
