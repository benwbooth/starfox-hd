//! Tick 136: boss1 death chain = bossexplode_Istrat (AUDIT_BOSS_TICKS2 Medium #9).
//!
//! ROM GBSTRATS.ASM:96 expstratptr → EXPSTRAT.ASM:78-140: s_boss_dying ($1e +
//! bgm $f1), 14 staggered SML/MED/L explosions + circdelayexplode, lifecnt 38,
//! bossdelayexplode; rotz spin via pushed boss1exp_Istrat (tempstrat).

use sf_game::alien::ASF3_REALOBJ;
use sf_game::vars::{GameVars, PSTF_NOTDIE};
use sf_game::{Game, Hooks};
use sf_strat::enemy_a::{boss1exp_init, bossflags, strat_boss1_init, wm, BF_DYING, SF_NOFIRING};
use std::cell::RefCell;
use std::rc::Rc;

const SE_BOSS_DYING: u8 = 0x1E;
const BGM_BOSS_DYING: u8 = 0xF1;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Ev {
    Se(u8),
    Music(u8),
}

#[derive(Clone, Default)]
struct Rec(Rc<RefCell<Vec<Ev>>>);
impl Hooks for Rec {
    fn play_se(&mut self, id: u8) {
        self.0.borrow_mut().push(Ev::Se(id));
    }
    fn play_music(&mut self, id: u8) {
        self.0.borrow_mut().push(Ev::Music(id));
    }
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn boss1_death_plays_dying_se_and_bgm() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
    g.vars = GameVars::default();
    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    log.borrow_mut().clear(); // drop appear SE $82

    boss1exp_init(&mut g, boss);

    assert_eq!(
        *log.borrow(),
        vec![Ev::Se(SE_BOSS_DYING), Ev::Music(BGM_BOSS_DYING)],
        "s_boss_dying: trigse $1e + startbgm $f1"
    );
    assert_ne!(bossflags(&g) & BF_DYING, 0);
    assert_ne!(g.vars.pstratflags & PSTF_NOTDIE, 0);
    assert_ne!(g.vars.read_ext8(wm::STRATFLAGS) & SF_NOFIRING, 0);
}

#[test]
fn boss1_death_spawns_barrage_lifecnt38_and_spin() {
    let mut g = Game::new();
    // Player slot so release/explode paths that touch playpt stay safe.
    let p = spawn(&mut g);
    assert_eq!(p, 0);
    g.objs.aliens[0].sflags3 |= ASF3_REALOBJ;

    let boss = spawn(&mut g);
    strat_boss1_init(&mut g, boss);
    let children_before = g.objs.active_indices().len() - 2; // player + boss
    assert!(
        children_before >= 9,
        "cover + 8 turrets, got {children_before}"
    );

    boss1exp_init(&mut g, boss);

    // Children released; 14 timed explosions + 1 circdelayexplode proxy.
    let after = g.objs.active_indices().len();
    // player + boss + 15 barrage = 17
    assert_eq!(
        after, 17,
        "player+boss+14 exp+circdelay; got {after} (children freed then barrage)"
    );
    assert_eq!(g.objs.aliens[boss as usize].count, 38, "lifecnt #58-20");
    assert!(
        g.objs.aliens[boss as usize].tempstratptr.is_some(),
        "boss1exp_Istrat spin armed in tempstrat"
    );
    assert!(
        g.objs.aliens[boss as usize].stratptr.is_some(),
        "bossdelayexplode_strat armed"
    );

    // Spin tick: rotz += deg90/32 each delay-explode frame via tempstrat.
    let rot0 = g.objs.aliens[boss as usize].rotz;
    let temp = g.objs.aliens[boss as usize].tempstratptr.unwrap();
    g.call_strat(temp, boss);
    // ROM deg90 = 64 → +2 per tick (GBSTRATS.ASM:87-90).
    assert_eq!(
        g.objs.aliens[boss as usize].rotz,
        rot0.wrapping_add(64 / 32),
        "boss1exp spin deg90/32"
    );
}
