//! Tick 208: pillar3 / pillar3f fall → bouncyball explode flash (AUDIT_ENEMY_A
//! Medium #36 leftover Minor). ROM `pillar3fall_i` (DSTRATS.ASM:804-809) and
//! `pillar3ffall_i` (KSTRATS.ASM:655-658).

use sf_game::alien::{ASF3_REALOBJ, ASF_COLLDISABLE, ASF_NOHITAFFECT, ASF_SHADOW};
use sf_game::game::{Game, Hooks};
use sf_game::obj::strat_init_obj_vars;
use sf_strat::enemies_ground::pillar3f_istrat;
use sf_strat::enemy_a::{strat_pillar3_init, AF_LEFT_PL, SH_BOUNCYBALL};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<u8>>>);
impl Hooks for Rec {
    fn play_se(&mut self, id: u8) {
        self.0.borrow_mut().push(id);
    }
    fn trig_se(&mut self, id: u8) {
        self.0.borrow_mut().push(id);
    }
}

fn spawn_player(g: &mut Game, x: i16, y: i16, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posx = x;
    g.vars.player_posy = y;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn_obj(g: &mut Game, x: i16, y: i16, z: i16) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    idx
}

fn find_killed_child(g: &Game, pillar: u16) -> Option<u16> {
    g.objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| {
            *i as u16 != 0
                && *i as u16 != pillar
                && a.active
                && a.hp == 0
                && a.sflags & ASF_COLLDISABLE != 0
                && a.expstratptr.is_some()
        })
        .map(|(i, _)| i as u16)
}

/// Dist trigger: pillar3fall_i spawns bouncyball at copypos, worldz−10, kill_obj.
#[test]
fn pillar3_fall_spawns_bouncyball_at_z_minus_10() {
    let mut g = Game::new();
    // Player already inside distless #500 so init's same-frame strat falls.
    spawn_player(&mut g, 0, -40, 100);
    let idx = spawn_obj(&mut g, 50, 0, 200);
    g.objs.aliens[idx as usize].hitflags = 0;
    strat_pillar3_init(&mut g, idx);

    let ball = find_killed_child(&g, idx).expect("killed explode child");
    let pillar = &g.objs.aliens[idx as usize];
    let child = &g.objs.aliens[ball as usize];
    assert_eq!(child.worldx, pillar.worldx);
    assert_eq!(child.worldy, pillar.worldy);
    assert_eq!(child.worldz, pillar.worldz.wrapping_sub(10));
    assert_eq!(child.shape, SH_BOUNCYBALL);
    assert_eq!(child.hp, 0);
    assert_ne!(child.sflags & ASF_COLLDISABLE, 0);
    assert!(child.stratptr.is_some());
    assert!(child.collstratptr.is_some());
    assert!(child.expstratptr.is_some());
    assert_eq!(pillar.next, Some(ball));
    assert_eq!(child.prev, Some(idx));
    assert_ne!(pillar.sflags & ASF_NOHITAFFECT, 0);
    assert_ne!(pillar.sflags & ASF_SHADOW, 0);
    assert_eq!(pillar.sbyte2, 16);
}

/// Far enough that scaled `xzdiffs` ≥ pillar3DIST (avoid i16 overflow that
/// makes huge |dz| look near). Full HP + clear HF2 → no fall on init frame.
#[test]
fn pillar3_init_no_fall_when_far() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn_obj(&mut g, 0, 0, 2000); // xzdiffs ≈ 1687 ≥ 500
    g.objs.aliens[idx as usize].hitflags = 0;
    strat_pillar3_init(&mut g, idx);
    assert_eq!(
        g.objs.aliens.iter().filter(|a| a.active).count(),
        2,
        "no bouncyball when upright"
    );
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 0, "still upright");
}

/// Left-of-view → sbyte1 = −4; right keeps +4 (s_rightview_strat).
#[test]
fn pillar3_fall_sbyte1_follows_leftpl() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let left = spawn_obj(&mut g, 0, 0, 100);
    g.objs.aliens[left as usize].flags |= AF_LEFT_PL;
    g.objs.aliens[left as usize].hp = 3; // < fall HP
    strat_pillar3_init(&mut g, left);
    // Init may already fall on same frame via hp gate.
    assert_eq!(g.objs.aliens[left as usize].sbyte1 as i8, -4, "leftpl → −4");

    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let right = spawn_obj(&mut g, 0, 0, 100);
    g.objs.aliens[right as usize].flags &= !AF_LEFT_PL;
    g.objs.aliens[right as usize].hp = 3;
    strat_pillar3_init(&mut g, right);
    assert_eq!(
        g.objs.aliens[right as usize].sbyte1 as i8, 4,
        "right of view → +4"
    );
}

/// After 16 fall frames → pillar3stay_istrat plays trigse $49.
#[test]
fn pillar3_stay_plays_se_49() {
    let shared = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(shared.clone())));
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn_obj(&mut g, 0, 0, 100);
    g.objs.aliens[idx as usize].hp = 3;
    strat_pillar3_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 16);
    shared.borrow_mut().clear();

    for _ in 0..16 {
        let strat = g.objs.aliens[idx as usize].stratptr.expect("strat");
        g.call_strat(strat, idx);
    }
    assert!(
        shared.borrow().contains(&0x49),
        "stay must trigse $49, got {:?}",
        *shared.borrow()
    );
    assert_eq!(
        g.objs.aliens[idx as usize].sflags & (ASF_NOHITAFFECT | ASF_SHADOW),
        0,
        "stay clears nohitaffect+shadow"
    );
}

#[test]
fn pillar3_explosion_children_follow_the_pillar_in_source_order() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let pillar = spawn_obj(&mut g, 0, 0, 2_000);
    strat_pillar3_init(&mut g, pillar);

    let explode = g.objs.aliens[pillar as usize]
        .expstratptr
        .expect("pillar explosion strategy");
    g.call_strat(explode, pillar);

    assert_eq!(
        g.objs.active_indices(),
        vec![pillar, 9, 8, 7, 6, 5, 4, 3, 2, 0],
        "each source s_make_obj inserts after the current pillar"
    );
    for (delay, child) in (2u16..=9).enumerate() {
        assert_eq!(g.objs.aliens[child as usize].count, delay as u8);
    }
}

/// pillar3ffall_i: bouncyball at copypos with NO z−10; leftpl roll sign.
#[test]
fn pillar3f_fall_spawns_bouncyball_no_z_offset() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn_obj(&mut g, 80, -20, 400);
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    pillar3f_istrat(&mut g, idx);
    // Zdistless #500 vs player at z=0 → |400|<500 → same-frame fall.
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert_eq!(after, before + 1);

    let ball = find_killed_child(&g, idx).expect("killed explode child");
    let pillar = &g.objs.aliens[idx as usize];
    let child = &g.objs.aliens[ball as usize];
    assert_eq!(child.worldx, pillar.worldx);
    assert_eq!(child.worldy, pillar.worldy);
    assert_eq!(child.worldz, pillar.worldz, "pillar3f has no worldz−10");
    assert_eq!(child.shape, SH_BOUNCYBALL);
    assert_eq!(pillar.sbyte1 as i8, 4); // right of view default

    // leftpl → −4
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let left = spawn_obj(&mut g, 0, 0, 100);
    g.objs.aliens[left as usize].flags |= AF_LEFT_PL;
    pillar3f_istrat(&mut g, left);
    assert_eq!(g.objs.aliens[left as usize].sbyte1 as i8, -4);
}
