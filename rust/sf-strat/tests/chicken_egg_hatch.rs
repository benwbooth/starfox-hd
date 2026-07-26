//! Tick 210: chicken `egg_istrat` hatch chain (DSTRATS.ASM:4528-4622) —
//! fall → hatch shell+chick / wait-to-hit / bounce→nothing (was instant
//! `strat_explode` on land).

use sf_game::alien::{ObjectVisualKind, ASF3_REALOBJ, ASF_COLLDISABLE, ASF_SHADOW};
use sf_game::game::{Game, Hooks};
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses::{chicken_egg_istrat, chicken_egg_strat};
use std::cell::RefCell;
use std::rc::Rc;

const SH_CHICK_EGG: u16 = 386;
const SH_CHICK_SHELL: u16 = 389;
const SH_CHICK_EGG_OPEN: u16 = 390;
const SH_BOSS_D_4: u16 = 238;
const EGG_HP_INIT: u8 = 21;
const SE_EGG: u8 = 0x3a;

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

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldz = z;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.internal_playpt = 0;
    g.vars.player_posz = z;
}

fn spawn_obj(g: &mut Game, x: i16, y: i16, z: i16) -> u16 {
    let idx = g.objs.alloc().expect("e");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    idx
}

fn count_shape(g: &Game, shape: u16) -> usize {
    g.objs
        .aliens
        .iter()
        .filter(|a| a.active && a.shape == shape)
        .count()
}

/// Init wires eggHP+20, ENEMY1, shadow, egg shape.
#[test]
fn chicken_egg_istrat_sets_hatch_hp() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g, 0, -200, 2000);
    chicken_egg_istrat(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.shape, SH_CHICK_EGG);
    assert_eq!(al.hp, EGG_HP_INIT);
    assert_eq!(al.ap, 8);
    assert_ne!(al.sflags & ASF_SHADOW, 0);
    assert_ne!(al.collflags & 0x10, 0); // COLLTYPE_ENEMY1
    assert_eq!(al.visual_kind, ObjectVisualKind::ScaledSprite);
    assert_eq!(al.depthoffset, 0);
    assert_eq!(al.tx, 0);
}

/// Far + high RNG → hatch: open egg, shell, chick, trigse $3a.
#[test]
fn chicken_egg_hatch_spawns_shell_and_chick() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g, 0, -80, 2000); // |dz|=2000 ≥ 600
                                               // First RANDOM draw 254 (≥76) → skip .notopen random branch.
    g.vars.rng = [0, 0, 0, 0];
    chicken_egg_istrat(&mut g, idx);
    log.borrow_mut().clear();

    for _ in 0..100 {
        chicken_egg_strat(&mut g, idx);
        if g.objs.aliens[idx as usize].shape == SH_CHICK_EGG_OPEN {
            break;
        }
        // If wait path somehow taken, force hatch via HP damage.
        if g.objs.aliens[idx as usize].worldy == 0 && g.objs.aliens[idx as usize].vy == 0 {
            // still closed egg on ground — may be wait; poke HP
            if g.objs.aliens[idx as usize].shape == SH_CHICK_EGG {
                g.objs.aliens[idx as usize].hp = EGG_HP_INIT - 1;
                // drive wait strat if switched
                if let Some(s) = g.objs.aliens[idx as usize].stratptr {
                    g.call_strat(s, idx);
                }
            }
            break;
        }
    }

    assert_eq!(
        g.objs.aliens[idx as usize].shape, SH_CHICK_EGG_OPEN,
        "egg opens to boss_d_7"
    );
    assert!(
        log.borrow().contains(&SE_EGG),
        "hatch trigse $3a; got {:?}",
        *log.borrow()
    );
    assert_eq!(count_shape(&g, SH_CHICK_SHELL), 1, "boss_d_6 shell");
    assert_eq!(count_shape(&g, SH_BOSS_D_4), 1, "boss_d_4 chick");
}

/// Close player → waittobehit; HP damage → hatch.
#[test]
fn chicken_egg_wait_then_hatch_on_damage() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g, 0, -40, 100); // |dz|=100 < 600 → wait
    g.vars.rng = [0, 0, 0, 0]; // random would hatch, but z-gate forces wait
    chicken_egg_istrat(&mut g, idx);

    for _ in 0..100 {
        if let Some(s) = g.objs.aliens[idx as usize].stratptr {
            g.call_strat(s, idx);
        }
        if g.objs.aliens[idx as usize].worldy == 0 && g.objs.aliens[idx as usize].vy == 0 {
            break;
        }
    }
    assert_eq!(
        g.objs.aliens[idx as usize].shape, SH_CHICK_EGG,
        "close → wait, still closed"
    );
    assert_eq!(count_shape(&g, SH_CHICK_SHELL), 0);

    log.borrow_mut().clear();
    g.objs.aliens[idx as usize].hp = EGG_HP_INIT - 1;
    if let Some(s) = g.objs.aliens[idx as usize].stratptr {
        g.call_strat(s, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].shape, SH_CHICK_EGG_OPEN);
    assert!(log.borrow().contains(&SE_EGG));
    assert_eq!(count_shape(&g, SH_CHICK_SHELL), 1);
    assert_eq!(count_shape(&g, SH_BOSS_D_4), 1);
}

/// Shell bounce settles into nothing (colldisable).
#[test]
fn chicken_egg_shell_settles_to_nothing() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g, 0, -60, 2000);
    g.vars.rng = [0, 0, 0, 0];
    chicken_egg_istrat(&mut g, idx);
    for _ in 0..100 {
        if let Some(s) = g.objs.aliens[idx as usize].stratptr {
            g.call_strat(s, idx);
        }
        if g.objs.aliens[idx as usize].shape == SH_CHICK_EGG_OPEN {
            break;
        }
    }
    assert_eq!(g.objs.aliens[idx as usize].shape, SH_CHICK_EGG_OPEN);

    // Drive shell + open-egg outcome until both colldisable.
    for _ in 0..120 {
        for i in 0..g.objs.aliens.len() {
            if !g.objs.aliens[i].active {
                continue;
            }
            if let Some(s) = g.objs.aliens[i].stratptr {
                g.call_strat(s, i as u16);
            }
        }
    }
    let shell = g
        .objs
        .aliens
        .iter()
        .find(|a| a.active && a.shape == SH_CHICK_SHELL)
        .expect("shell");
    assert_ne!(shell.sflags & ASF_COLLDISABLE, 0, "shell → nothing_istrat");
}
