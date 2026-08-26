//! Tick 202: shou0 / bazooka / houdai5f full-body fire spawn + weapon SE
//! (closes TIER2 FULL BODY blocker — these aim at the player, not find_nearobj).

use sf_core::aim_angle::{xanglexy, yanglexy};
use sf_game::alien::{ObjectVisualKind, ASF4_INVISIBLE, ATLASER};
use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_strat::enemies_ground::{bazooka1l_istrat, houdai5f_istrat, houdai5f_strat, shou0_istrat};
use sf_strat::enemy_a::SH_BOUNCYBALL;
use std::cell::RefCell;
use std::rc::Rc;

const DEG180: u8 = 0x80;
const PLASMA_SPEED: u8 = 80;
const PLASMA_LIFE: u8 = 100;
const PLASMA_AP: u8 = 10;
const HOUDAI5F_MUZZLE_Y: i16 = -236; // (-59<<2) after weapon_scale ASL back

#[derive(Debug, Clone, PartialEq, Eq)]
enum SndEvent {
    MakeSnd(PosSndFamilyId, i16, i16),
}

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<SndEvent>>>);
impl Hooks for Rec {
    fn make_snd(&mut self, family: PosSndFamilyId, x: i16, z: i16) {
        self.0.borrow_mut().push(SndEvent::MakeSnd(family, x, z));
    }
}

fn spawn_player(g: &mut Game, x: i16, y: i16, z: i16) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    g.vars.internal_playpt = 0;
    g.vars.player_posx = x;
    g.vars.player_posy = y;
    g.vars.player_posz = z;
    g.vars.pviewvelz = 0;
}

fn spawn_enemy(g: &mut Game, x: i16, y: i16, z: i16) -> u16 {
    let idx = g.objs.alloc().expect("e");
    let al = &mut g.objs.aliens[idx as usize];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    idx
}

fn find_laser(g: &Game, skip: u16) -> Option<usize> {
    g.objs.aliens.iter().enumerate().find_map(|(i, a)| {
        if i as u16 != skip && i != 0 && a.active && a.type_ & ATLASER != 0 {
            Some(i)
        } else {
            None
        }
    })
}

/// shou0 in-range /16 gate: plasma spawn aims with raw Yanglexy (not nega) +
/// EnemyBattry (fire_plasma).
#[test]
fn shou0_plasma_spawn_aim_and_battry_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 200, -40, 0);
    let e = spawn_enemy(&mut g, 0, 0, 1000); // |dz|=1000 ∈ [500,2500)
                                             // Force fire gate: (gameframe+idx)&15==0 with idx=1 → gameframe=15.
    g.vars.gameframe = 15;
    shou0_istrat(&mut g, e);

    let shot_i = find_laser(&g, e).expect("PLASMA spawned");
    let me = &g.objs.aliens[e as usize];
    let pl = &g.objs.aliens[0];
    let expect_yaw = yanglexy(
        pl.worldx.wrapping_sub(me.worldx),
        pl.worldz.wrapping_sub(me.worldz),
    );
    let expect_pitch = xanglexy(
        pl.worldy.wrapping_sub(me.worldy),
        pl.worldx.wrapping_sub(me.worldx),
        pl.worldz.wrapping_sub(me.worldz),
    );
    let shot = &g.objs.aliens[shot_i];
    assert_eq!(
        shot.roty, expect_yaw,
        "weapon_rots2obj yaw = Yanglexy (raw)"
    );
    assert_eq!(shot.rotx, expect_pitch);
    assert_eq!(shot.vel, PLASMA_SPEED);
    assert_eq!(shot.count, PLASMA_LIFE);
    assert_eq!(shot.ap, PLASMA_AP);
    assert!(
        log.borrow()
            .iter()
            .any(|e| matches!(e, SndEvent::MakeSnd(PosSndFamilyId::EnemyBattry, _, _))),
        "fire_plasma → enemybattrysound; got {:?}",
        log.borrow()
    );
}

/// bazooka fire state lobs RELSLOWELASER + lasersound.
#[test]
fn bazooka_fire_plays_laser_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0, -40, 0);
    let e = spawn_enemy(&mut g, 0, 0, 2000);
    // Drive into fire: rise/level/aim is long — force state 2 + sbyte1 countdown.
    bazooka1l_istrat(&mut g, e);
    {
        let al = &mut g.objs.aliens[e as usize];
        al.stratstate = 2;
        al.sbyte1 = 3; // beqdec will fire while counting
        al.vel = 0;
        al.vy = 0;
        al.worldy = -40; // near player y so chase is quiet
    }
    // Seed RNG so rndrot is deterministic; call strat via stratptr.
    g.vars.rng = [1, 2, 3, 4];
    let s = g.objs.aliens[e as usize].stratptr.expect("tick");
    g.call_strat(s, e);

    assert!(
        find_laser(&g, e).is_some(),
        "RELSLOWELASER spawned in fire state"
    );
    assert!(
        log.borrow()
            .iter()
            .any(|e| matches!(e, SndEvent::MakeSnd(PosSndFamilyId::Laser, _, _))),
        "fire_relslowElaser → lasersound; got {:?}",
        log.borrow()
    );
}

/// houdai5f far + /32 gate: Hplasma at muzzle y−236, yaw+180, EnemyBattry.
#[test]
fn houdai5f_hplasma_muzzle_and_battry_se() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0, 0, 0);
    let e = spawn_enemy(&mut g, 50, 0, 3000);
    g.objs.aliens[e as usize].roty = 0;
    g.objs.aliens[e as usize].rotx = 10;
    g.vars.gameframe = 0; // &31==0
    houdai5f_istrat(&mut g, e);

    let shot_i = find_laser(&g, e).expect("Hplasma");
    let shot = &g.objs.aliens[shot_i];
    assert_eq!(shot.vel, 100);
    assert_eq!(shot.count, 100);
    assert_eq!(shot.roty, DEG180, "s_weapon_rot #0,#deg180");
    assert_eq!(shot.rotx, 10);
    assert_eq!(shot.shape, SH_BOUNCYBALL);
    assert_eq!(shot.visual_kind, ObjectVisualKind::ScaledSprite);
    assert_eq!(shot.sflags4 & ASF4_INVISIBLE, 0);
    assert_eq!(shot.fireobjptr, 1, "al_ptr = playpt (slot0+1)");
    // Muzzle: firer (50,0,3000) + (0,-236,0) with roty0/rotx10 — full rotate.
    // At least Y should be below firer.
    assert!(
        shot.worldy < g.objs.aliens[e as usize].worldy,
        "muzzle y offset downward (expect ~{HOUDAI5F_MUZZLE_Y})"
    );
    assert!(
        log.borrow()
            .iter()
            .any(|e| matches!(e, SndEvent::MakeSnd(PosSndFamilyId::EnemyBattry, _, _))),
        "enemybattrysound on Hplasma; got {:?}",
        log.borrow()
    );

    // Close hold: no second shot when near.
    log.borrow_mut().clear();
    let e2 = spawn_enemy(&mut g, 0, 0, 200);
    g.vars.gameframe = 0;
    houdai5f_istrat(&mut g, e2);
    let lasers_near = g
        .objs
        .aliens
        .iter()
        .enumerate()
        .filter(|(i, a)| *i != 0 && *i as u16 != e && a.active && a.type_ & ATLASER != 0)
        .count();
    // Only the earlier far shot should exist as laser from e; e2 must not add one.
    assert_eq!(
        lasers_near, 1,
        "Zdistless #400 holds fire (only prior far shot)"
    );
}

#[test]
fn houdai5f_public_strat_anim_wraps() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0, 0);
    let e = spawn_enemy(&mut g, 0, 0, 5000);
    g.vars.gameframe = 1; // no fire
    houdai5f_istrat(&mut g, e);
    let a0 = g.objs.aliens[e as usize].animframe & 0x7f;
    houdai5f_strat(&mut g, e);
    let a1 = g.objs.aliens[e as usize].animframe & 0x7f;
    assert_eq!(a1, a0.wrapping_add(1) % 12);
}
