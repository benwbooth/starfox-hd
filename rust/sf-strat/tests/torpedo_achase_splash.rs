//! Tick 201: torpedo Achase + underwater mover glue + splash/upsea
//! (GASTRATS.ASM:2007-2044). Closes TIER2 torpedo full-body blocker.

use sf_core::aim_angle::yanglexy;
use sf_core::snes_trig::achase_angle_8;
use sf_game::alien::ASF_COLLDISABLE;
use sf_game::game::{Game, Hooks, PosSndFamilyId};
use sf_strat::common::gen_vecs_3d;
use sf_strat::enemies_ground::{torpedo_istrat, torpedo_strat, torpedoa_strat};
use std::cell::RefCell;
use std::rc::Rc;

const SH_F_FISH: u16 = 271;
const DEG45: u8 = 32;

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

fn spawn_player(g: &mut Game, x: i16, z: i16) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = x;
    al.worldz = z;
    g.vars.internal_playpt = 0;
    g.vars.pviewvelz = 0;
}

fn spawn_torpedo(g: &mut Game, x: i16, y: i16, z: i16) -> u16 {
    let idx = g.objs.alloc().expect("t");
    let al = &mut g.objs.aliens[idx as usize];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    idx
}

fn count_active(g: &Game) -> usize {
    g.objs.aliens.iter().filter(|a| a.active).count()
}

/// Far: `s_obj2obj_angle` rate-3 = Achase(nega(Yanglexy)); then n3dvecs coast.
#[test]
fn torpedo_yaw_achase_rate3_then_gen3dvecs() {
    let mut g = Game::new();
    spawn_player(&mut g, 800, 0);
    let t = spawn_torpedo(&mut g, 0, -50, 4000); // |dz|=4000 > 800
    let before = count_active(&g);
    torpedo_istrat(&mut g, t);
    assert!(count_active(&g) > before, "makeSsplash each submerged tick");

    let me = &g.objs.aliens[t as usize];
    assert_eq!(me.shape, 0, "still submerged");
    assert_ne!(me.sflags & ASF_COLLDISABLE, 0);
    assert_eq!(me.vel, 30);

    // Expected yaw after one Achase rate-3 from roty=0 at the pre-move pose.
    let target = yanglexy(800i16.wrapping_sub(0), 0i16.wrapping_sub(4000)).wrapping_neg();
    let mut expect = 0u8;
    achase_angle_8(&mut expect, target, 3);

    // Manual aim + n3dvecs (no splash/scroll) must match the same yaw step.
    let mut g2 = Game::new();
    spawn_player(&mut g2, 800, 0);
    let t2 = spawn_torpedo(&mut g2, 0, -50, 4000);
    {
        let al = &mut g2.objs.aliens[t2 as usize];
        al.vel = 30;
        al.roty = 0;
        al.rotx = 0;
    }
    let pl = g2.objs.aliens[0];
    let me0 = g2.objs.aliens[t2 as usize];
    let mut roty = me0.roty;
    achase_angle_8(
        &mut roty,
        yanglexy(
            pl.worldx.wrapping_sub(me0.worldx),
            pl.worldz.wrapping_sub(me0.worldz),
        )
        .wrapping_neg(),
        3,
    );
    g2.objs.aliens[t2 as usize].roty = roty;
    gen_vecs_3d(&mut g2.objs.aliens[t2 as usize]);
    assert_eq!(roty, expect);
    let al = &g2.objs.aliens[t2 as usize];
    assert!(al.vx != 0 || al.vz != 0, "n3dvecs from aimed yaw+speed30");

    // Live strat path: first istrat tick aimed once from roty=0 at same pose.
    assert_eq!(
        g.objs.aliens[t as usize].roty, expect,
        "torpedo_strat Achase rate-3 matches yanglexy_nega"
    );
}

/// Inside 800z: surface splash + enemyupsea + pitch Achase toward 0.
#[test]
fn torpedo_surface_plays_upsea_and_levels_pitch() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0, 0);
    let t = spawn_torpedo(&mut g, 100, -40, 500); // |dz|=500 < 800
    let before = count_active(&g);
    torpedo_istrat(&mut g, t);

    let al = &g.objs.aliens[t as usize];
    assert_eq!(al.shape, SH_F_FISH);
    assert_eq!(al.sflags & ASF_COLLDISABLE, 0);
    assert!(count_active(&g) > before, "makesplash on surface");
    assert!(
        log.borrow()
            .iter()
            .any(|e| matches!(e, SndEvent::MakeSnd(PosSndFamilyId::EnemyUpSea, _, _))),
        "jsl enemyupsea_l → make_snd(EnemyUpSea); got {:?}",
        log.borrow()
    );

    // Pitch was -deg45 then Achase rate-2 toward 0 once in torpedoa_strat.
    let mut pitch = (-(DEG45 as i8)) as u8;
    achase_angle_8(&mut pitch, 0, 2);
    assert_eq!(al.rotx, pitch);

    let p0 = al.rotx;
    torpedoa_strat(&mut g, t);
    let p1 = g.objs.aliens[t as usize].rotx;
    assert!((p1 as i8).unsigned_abs() < (p0 as i8).unsigned_abs() || p1 == 0);
}

#[test]
fn torpedo_public_istrat_wires_ptrs() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0);
    let t = spawn_torpedo(&mut g, 0, 0, 5000);
    torpedo_istrat(&mut g, t);
    let al = &g.objs.aliens[t as usize];
    assert!(al.stratptr.is_some());
    assert!(al.collstratptr.is_some());
    assert!(al.expstratptr.is_some());
    assert_eq!(al.hp, 4);
    assert_eq!(al.ap, 4);
    let yaw0 = al.roty;
    torpedo_strat(&mut g, t);
    assert_eq!(g.objs.aliens[t as usize].shape, 0);
    assert_ne!(g.objs.aliens[t as usize].roty, yaw0);
}
