//! Tick 195: AUDIT_ENEMY_B Minors verify (already ported).

use sf_game::alien::{ATLASER, ATMISSILE};
use sf_game::game::{Game, Hooks};
use sf_strat::enemy_a::boss_attach_child_to_mother;
use sf_strat::enemy_b::{
    bossa_cup_strat, bossacover_init, bossacover_strat, bossacupopen_srou, bossacupperl_istrat,
    bossaturretm_istrat, bossaup_init, bossfa_istrat, bossfa_strat, bossfc2_strat,
    strat_bossa_init, BOSSA_CUP_STATE_COVER, BOSSA_CUP_STATE_DOWN, BOSSA_CUP_STATE_UP,
};
use sf_strat::snes_trig::strat_roffs_full_scaled;
use std::cell::RefCell;
use std::rc::Rc;

/// ROM parent sflag3 all-cups-dead (`ASF3_SFLAG5`).
const BOSSA_PARENT_FLAG_CUPS_DEAD: u8 = 0x01;
const BOSSA_SCALE: u32 = 2;
const BOSSF_SCALE: u32 = 3;
const DEG180: u8 = 128;

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<u8>>>);
impl Hooks for Rec {
    fn play_se(&mut self, id: u8) {
        self.0.borrow_mut().push(id);
    }
}

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldz = z;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn shots(g: &Game, firer: u16) -> Vec<(i16, i16, i16)> {
    g.objs
        .aliens
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            a.active && *i as u16 != firer && *i != 0 && a.type_ & (ATLASER | ATMISSILE) != 0
        })
        .map(|(_, a)| (a.worldx, a.worldy, a.worldz))
        .collect()
}

/// Minor: bossAup/cover $73/$72 gated on cups-dead sflag3.
#[test]
fn bossa_up_cover_sounds_gated_on_cups_dead() {
    // up $73 when cups alive
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let idx = spawn(&mut g);
    bossaup_init(&mut g, idx);
    assert_eq!(*log.borrow(), vec![0x73]);

    // up silent when cups dead
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sflags3 |= BOSSA_PARENT_FLAG_CUPS_DEAD;
    bossaup_init(&mut g, idx);
    assert!(log.borrow().is_empty());

    // cover $72 when cups alive
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let idx = spawn(&mut g);
    bossacover_init(&mut g, idx);
    assert_eq!(*log.borrow(), vec![0x72]);

    // cover silent when cups dead
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sflags3 |= BOSSA_PARENT_FLAG_CUPS_DEAD;
    bossacover_init(&mut g, idx);
    assert!(log.borrow().is_empty());
}

/// Minor: cover DOWN while sbyte2 >= 20 (not > 20).
#[test]
fn bossacover_down_at_sbyte2_eq_20() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let cup = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, cup, 4));
    bossacupperl_istrat(&mut g, cup);
    bossacover_init(&mut g, mother);

    // pre=21 → post-dec 20 → DOWN (old `>20` would have stayed UP).
    g.objs.aliens[mother as usize].sbyte2 = 21;
    bossacover_strat(&mut g, mother);
    assert_eq!(g.objs.aliens[mother as usize].sbyte2, 20);
    assert_eq!(g.objs.aliens[cup as usize].stratstate, BOSSA_CUP_STATE_DOWN);

    // pre=20 → post-dec 19 → UP.
    g.objs.aliens[mother as usize].sbyte2 = 20;
    bossacover_strat(&mut g, mother);
    assert_eq!(g.objs.aliens[mother as usize].sbyte2, 19);
    assert_eq!(g.objs.aliens[cup as usize].stratstate, BOSSA_CUP_STATE_UP);
}

/// Minor: bossA parent has no collstrat (no hit_flash).
#[test]
fn bossa_parent_collstrat_none() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    strat_bossa_init(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].collstratptr.is_none());
}

/// Minor: turret M Icont overwrites sbyte3 to 0 (not DEG180).
#[test]
fn bossaturretm_icont_sbyte3_zero() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let tur = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, tur, 2));
    g.objs.aliens[tur as usize].sbyte3 = DEG180; // would-be Istrat value
    bossaturretm_istrat(&mut g, tur);
    assert_eq!(g.objs.aliens[tur as usize].sbyte3, 0);
}

/// Minor: cup home Z = turret.z - (2<<bossA_scale); open anim caps at 6.
#[test]
fn bossa_cup_home_z_and_open_anim_cap6() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let turret = spawn(&mut g);
    let cup = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, turret, 1));
    assert!(boss_attach_child_to_mother(&mut g, mother, cup, 4));
    bossacupperl_istrat(&mut g, cup);
    g.objs.aliens[turret as usize].worldx = 100;
    g.objs.aliens[turret as usize].worldy = -50;
    g.objs.aliens[turret as usize].worldz = 2000;
    g.objs.aliens[cup as usize].stratstate = BOSSA_CUP_STATE_COVER;
    g.objs.aliens[cup as usize].sbyte2 = 1; // linked turret child_num

    bossa_cup_strat(&mut g, cup);
    assert_eq!(g.objs.aliens[cup as usize].worldx, 100);
    assert_eq!(
        g.objs.aliens[cup as usize].worldy,
        (-50i16).wrapping_sub(15i16 << BOSSA_SCALE)
    );
    assert_eq!(
        g.objs.aliens[cup as usize].worldz,
        2000i16.wrapping_sub(2i16 << BOSSA_SCALE)
    );

    g.objs.aliens[cup as usize].animframe = 6;
    g.vars.gameframe = 0;
    bossacupopen_srou(&mut g, cup);
    assert_eq!(g.objs.aliens[cup as usize].animframe, 6);
    g.objs.aliens[cup as usize].animframe = 5;
    bossacupopen_srou(&mut g, cup);
    assert_eq!(g.objs.aliens[cup as usize].animframe, 6);
}

/// Minor: bossFC2 / bossFA muzzle bytes are rotated before weapon_scale.
#[test]
fn bossf_muzzle_offsets_effective_scale() {
    let muzzle = 20i16 << BOSSF_SCALE; // 160
    assert_eq!(muzzle, 160);
    assert_ne!(muzzle, 20i16 >> 2); // old 4x-too-small class of bug

    // bossFA twin shots
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn(&mut g);
    let fa = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, fa, 1));
    bossfa_istrat(&mut g, fa);
    g.objs.aliens[mother as usize].sflags2 &= !0x10; // clear combine
    g.objs.aliens[fa as usize].stratstate = 2;
    g.objs.aliens[fa as usize].roty = DEG180;
    g.objs.aliens[fa as usize].worldx = 0;
    g.objs.aliens[fa as usize].worldy = 0;
    g.objs.aliens[fa as usize].worldz = 1000;
    g.objs.aliens[fa as usize].vel = 0;
    g.vars.gameframe = 0;
    bossfa_strat(&mut g, fa);
    let fa_shots = shots(&g, fa);
    assert_eq!(fa_shots.len(), 2, "FA twin muzzle");
    let pose = g.objs.aliens[fa as usize];
    let mut expected: Vec<_> = [-40, 40]
        .into_iter()
        .map(|x| {
            let (rx, ry, rz) =
                strat_roffs_full_scaled(pose.rotz, pose.rotx, pose.roty, x, -40, 20, 2);
            (rx, ry, 1000i16.wrapping_add(rz))
        })
        .collect();
    expected.sort_unstable();
    let mut got = fa_shots;
    got.sort_unstable();
    assert_eq!(got, expected, "full-rotation twin muzzle positions");

    // bossFC2 Hplasma at gameframe&31==10
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let boss = spawn(&mut g);
    g.objs.aliens[boss as usize].sbyte2 = 3;
    g.objs.aliens[boss as usize].worldx = 0;
    g.objs.aliens[boss as usize].worldy = 0;
    g.objs.aliens[boss as usize].worldz = 2000; // ahead of player, |dz|>=600
    g.objs.aliens[boss as usize].roty = DEG180;
    g.vars.gameframe = 10;
    bossfc2_strat(&mut g, boss);
    let fc_shots = shots(&g, boss);
    assert_eq!(fc_shots.len(), 1, "FC2 left muzzle at frame 10");
    let pose = g.objs.aliens[boss as usize];
    let (rx, ry, rz) = strat_roffs_full_scaled(pose.rotz, pose.rotx, pose.roty, -40, -40, 0, 2);
    assert_eq!(fc_shots[0], (rx, ry, 2000i16.wrapping_add(rz)));
}
