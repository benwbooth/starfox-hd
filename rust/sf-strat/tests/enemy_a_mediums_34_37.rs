//! Tick 157: AUDIT_ENEMY_A Mediums #34–#37 — hard90yr no enemy1 colltype;
//! delayexplode s_decbpl (count_down); pillar3explode 8-child chain + silent;
//! init→strat same-frame fall-through.

use sf_game::alien::{ExplosionSize, ObjectVisualKind, ASF2_COLLDISABLE, ASF3_REALOBJ};
use sf_game::game::{Game, Hooks};
use sf_strat::enemy_a::{
    delayexplode_strat, strat_hard180yr_init, strat_hard90yr_init, strat_houdai_init,
    strat_pillar3_init, strat_skillfly_init, strat_spacebarshoot_init, strat_zaco1l_init,
    strat_zacos_init, wm, ASF4_NOPOLYEXP, COLLTYPE_ENEMY1,
};
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

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

/// Medium #34: hard90YR has no COLLTYPE_ENEMY1 (unlike hard180YR).
#[test]
fn hard90yr_has_no_enemy1_colltype() {
    let mut g = Game::new();
    let a = spawn(&mut g);
    let b = spawn(&mut g);
    strat_hard90yr_init(&mut g, a);
    strat_hard180yr_init(&mut g, b);
    assert_eq!(
        g.objs.aliens[a as usize].collflags & COLLTYPE_ENEMY1,
        0,
        "hard90YR must not set enemy1"
    );
    assert_ne!(
        g.objs.aliens[b as usize].collflags & COLLTYPE_ENEMY1,
        0,
        "hard180YR does set enemy1"
    );
    assert_eq!(g.objs.aliens[a as usize].roty, 128); // DEG180
}

/// Medium #35: delayexplode uses s_decbpl — count=1 survives first tick.
#[test]
fn delayexplode_count_one_survives_first_tick() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].count = 1;
    g.objs.aliens[idx as usize].stratptr = Some(g.world.register_strategy(delayexplode_strat));
    g.objs.aldead = 0;
    delayexplode_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 0, "entry count 1 must survive");
    assert_eq!(g.objs.aliens[idx as usize].count, 0);
    // Second expiry applies ASM s_kill_obj (STRATMAC.INC:2643): colldisable +
    // HP:=0 as a death SIGNAL (the inline expstrat morphs nopolyexp-free
    // corpses into live polygon meshes) — not the removal flag.
    delayexplode_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 0, "expiry signals death, does not remove");
    assert_eq!(g.objs.aliens[idx as usize].hp, 0);
    assert_ne!(
        g.objs.aliens[idx as usize].sflags2 & ASF2_COLLDISABLE,
        0,
        "colldisable set"
    );
}

/// Medium #36: pillar3explode spawns 8 nopolyexp med children, no $10 SE, lifecnt 7.
#[test]
fn pillar3explode_spawns_eight_silent_children() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 100;
    g.objs.aliens[idx as usize].worldy = -50;
    g.objs.aliens[idx as usize].worldz = 800;
    g.objs.aliens[idx as usize].rotz = 0;
    g.objs.aliens[idx as usize].sword2 = 0;
    strat_pillar3_init(&mut g, idx);
    let exp = g.objs.aliens[idx as usize].expstratptr.expect("exp");
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    g.call_strat(exp, idx);

    assert!(
        log.borrow().is_empty(),
        "pillarexplode plays no direct SE, got {:?}",
        *log.borrow()
    );
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert_eq!(after, before + 8, "8 medium-exp children");

    let children: Vec<_> = g
        .objs
        .aliens
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            a.active
                && *i as u16 != 0
                && *i as u16 != idx
                && a.visual_kind == ObjectVisualKind::ExplosionEnvelope(ExplosionSize::Medium)
        })
        .collect();
    assert_eq!(children.len(), 8);
    for (i, (_slot, al)) in children.iter().enumerate() {
        assert_eq!(al.shape, 0, "child {i} uses the non-mesh envelope");
        assert_ne!(al.sflags4 & ASF4_NOPOLYEXP, 0, "child {i} nopolyexp");
        assert_eq!(al.count, i as u8, "staggered lifecnt 0..7");
    }
    assert_eq!(
        g.objs.aliens[idx as usize].count, 6,
        "pillar lifecnt 7 minus the inline first delayremove decrement"
    );
    assert!(g.objs.aliens[idx as usize].stratptr.is_some()); // delayremove
}

/// Medium #37: skillfly_Istrat falls into strat — +1000 then behind-remove.
#[test]
fn skillfly_init_runs_strat_same_frame() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 5000); // ahead → .rem after +1000
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 100;
    strat_skillfly_init(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].worldz, 1100,
        "skillfly_strat body must run on init frame (+1000 kept on .rem)"
    );
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(g.vars.read_ext8(wm::SKILLFLY), 1);
}

/// Medium #37: spacebarshoot_Istrat falls into strat — count 80→79.
#[test]
fn spacebarshoot_init_runs_strat_same_frame() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sbyte1 = 0;
    g.objs.aliens[idx as usize].sword1 = 0;
    g.objs.aliens[idx as usize].sword2 = 0;
    strat_spacebarshoot_init(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].count, 79,
        "body must dec lifecnt on spawn frame"
    );
}

/// Medium #37: houdai_Istrat falls into strat — fires on cadence when far.
#[test]
fn houdai_init_runs_strat_same_frame() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    assert_eq!(idx, 1);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = 0;
    g.objs.aliens[idx as usize].worldz = 2000; // dist_xz >= 800 → may fire
                                               // (gameframe+phase(idx))&0x0F==0 so cadence allows
                                               // fire on this tick; phase(1)=seed54+step54=108.
    g.vars.gameframe = 4; // (4+108)&0x0F == 0
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    strat_houdai_init(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    for (i, a) in g.objs.aliens.iter().enumerate() {
        if a.active {
            eprintln!("[dbgh] slot={} sh={} ty={:02X} p=({},{},{})", i, a.shape, a.type_, a.worldx, a.worldy, a.worldz);
        }
    }
    eprintln!("[dbgh] before={} after={}", before, after);
    assert!(
        after > before,
        "houdai_strat body must fire plasma on spawn frame"
    );
}

/// Medium #37: zacos_Istrat falls into phase0 — pitch dec when in fire band.
#[test]
fn zacos_init_runs_phase0_same_frame() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, 0, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    // worldy >= player_posy-800 → pitch/fire block runs; init sets rotx=DEG90≠0
    // so body does rotx-=2 (not fire).
    g.objs.aliens[idx as usize].worldy = 0;
    g.objs.aliens[idx as usize].worldz = 1000;
    strat_zacos_init(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].rotx,
        62, // DEG90(64) - 2
        "zacos_phase0 body must run on spawn frame"
    );
}

/// Medium #37: zaco1_Istrat falls into phase0 — swpz1 = player_posz+1500.
#[test]
fn zaco1_init_runs_phase0_same_frame() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 200);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 0;
    strat_zaco1l_init(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].swpz1, 1700,
        "zaco1_phase0 sets WP z = player_posz+1500"
    );
}
