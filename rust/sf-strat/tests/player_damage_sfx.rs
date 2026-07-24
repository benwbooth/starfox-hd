//! Tick 131: player body/wing damage SE + death BGM + nova $30 verify.
//!
//! ROM PSTRATS.ASM pcolB/LW/RW + PLWbrk/PRWbrk + playerdead_Istrat;
//! GSTRATS.ASM nukeexp_Istrat trigse $30.

use sf_core::player_view::PlayerViewMode;
use sf_game::alien::{ACF_COLLTYPE1, ASF_COLLDISABLE};
use sf_game::vars::{GameVars, GF_PLAYERDEAD, GF_PLAYERDYING, PSF2_PLAYERHP0, SPACE_MODE};
use sf_game::{Game, Hooks};
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::{nukeexp_istrat, ASF2_SFLAG1, ASF2_SFLAG2};
use sf_strat::player::{pcbox_attach, strat_spawn_player, COCKPIT_EXIT_FRAMES};
use std::cell::RefCell;
use std::rc::Rc;

const PSF2_WIRESHIP: u8 = 2;
const PSF_BRKLWING: u8 = 8;
const PSF_BRKRWING: u8 = 16;

const SE_BODY_HARD: u8 = 0x04;
const SE_WING_DESTRUCT_L: u8 = 0x05;
const SE_WING_DESTRUCT_R: u8 = 0x06;
const SE_WING_HIT_L: u8 = 0x07;
const SE_WING_HIT_R: u8 = 0x08;
const SE_WIRE_SCRAPE: u8 = 0x14;
const SE_BODY_SOFT: u8 = 0x19;
const SE_SHIELD_Q: u8 = 0x1b;
const SE_SHIELD_E: u8 = 0x1c;
const SE_PLAYER_DOWN: u8 = 0x03;
const BGM_PLAYER_DOWN: u8 = 0x11;
const SE_NOVA_DET: u8 = 0x30;
const LETHAL_BODY_HEALTH: u8 = 3;
const LETHAL_SHOT_POWER: u8 = 3;
const DEATH_DISPATCH_TICK_LIMIT: usize = 8;

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

fn new_game(log: Rc<RefCell<Vec<Ev>>>) -> Game {
    let mut g = Game::with_hooks(Box::new(Rec(log)));
    g.vars = GameVars::default();
    g.vars.game_mode = SPACE_MODE;
    g.vars.minpmove_y = -210;
    g.vars.set_sv_i16(sv::MINPMOVEX, -240);
    g.vars.set_sv_i16(sv::MAXPMOVEX, 240);
    g.vars.set_sv_i16(sv::MAXPMOVEY, -20);
    g.vars.set_sv_u8(sv::LIVES, 3);
    g.vars.set_sv_u16(sv::RNDVAL, 0x1234);
    g
}

fn spawn_with_boxes(log: Rc<RefCell<Vec<Ev>>>) -> (Game, u16) {
    let mut g = new_game(log);
    let p = strat_spawn_player(&mut g).unwrap();
    g.objs.aliens[p as usize].worldx = 0;
    g.objs.aliens[p as usize].worldy = 0;
    g.objs.aliens[p as usize].worldz = 0;
    g.objs.aliens[p as usize].rotz = 0;
    g.objs.aliens[p as usize].stratptr = None;
    assert!(pcbox_attach(&mut g, p));
    (g, p)
}

fn spawn_shot(g: &mut Game, x: i16, y: i16, z: i16, ap: u8) -> u16 {
    let s = g.objs.alloc().unwrap();
    let al = &mut g.objs.aliens[s as usize];
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    al.hp = 5;
    al.ap = ap;
    al.collflags = ACF_COLLTYPE1;
    al.shape = 511;
    s
}

#[test]
fn body_soft_hit_plays_se_19() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (mut g, _) = spawn_with_boxes(log.clone());
    let body = g.coldet.pcbox.body.unwrap();
    g.tick();
    let (bx, by, bz) = {
        let a = &g.objs.aliens[body as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    spawn_shot(&mut g, bx, by, bz, 3);
    log.borrow_mut().clear();
    g.tick(); // detect
    g.tick(); // route → soft body SE
    assert!(
        log.borrow().contains(&Ev::Se(SE_BODY_SOFT)),
        "expected $19 soft body hit, got {:?}",
        log.borrow()
    );
    assert!(!log.borrow().contains(&Ev::Se(SE_BODY_HARD)));
}

#[test]
fn body_hard_hit_plays_se_04() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (mut g, _) = spawn_with_boxes(log.clone());
    let body = g.coldet.pcbox.body.unwrap();
    g.tick();
    let (bx, by, bz) = {
        let a = &g.objs.aliens[body as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    spawn_shot(&mut g, bx, by, bz, 8);
    log.borrow_mut().clear();
    g.tick();
    g.tick();
    assert!(
        log.borrow().contains(&Ev::Se(SE_BODY_HARD)),
        "expected $04 hard body hit, got {:?}",
        log.borrow()
    );
}

#[test]
fn body_shield_warn_quarter_and_eighth() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (mut g, _) = spawn_with_boxes(log.clone());
    let body = g.coldet.pcbox.body.unwrap();
    g.tick();
    let (bx, by, bz) = {
        let a = &g.objs.aliens[body as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    // Drop through /4 (10) then /8 (5).
    g.objs.aliens[body as usize].hp = 10;
    spawn_shot(&mut g, bx, by, bz, 1);
    log.borrow_mut().clear();
    g.tick();
    g.tick();
    assert!(
        log.borrow().contains(&Ev::Se(SE_SHIELD_Q)),
        "expected $1b at ≤10 HP, got {:?}",
        log.borrow()
    );
    assert!(g.objs.aliens[body as usize].sflags2 & ASF2_SFLAG1 != 0);

    // Further damage into eighth band.
    g.objs.aliens[body as usize].hp = 5;
    g.objs.aliens[body as usize].sflags2 &= !ASF2_SFLAG2;
    // Force a fresh collide entry: clear LCOLLIDE path by ensuring next hit routes.
    spawn_shot(&mut g, bx, by, bz, 1);
    log.borrow_mut().clear();
    g.tick();
    g.tick();
    assert!(
        log.borrow().contains(&Ev::Se(SE_SHIELD_E)),
        "expected $1c at ≤5 HP, got {:?}",
        log.borrow()
    );
}

#[test]
fn left_wing_break_plays_destruct_05() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (mut g, _) = spawn_with_boxes(log.clone());
    let lwing = g.coldet.pcbox.lwing.unwrap();
    g.objs.aliens[lwing as usize].hp = 1;
    g.tick();
    let (wx, wy, wz) = {
        let a = &g.objs.aliens[lwing as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    let shot = spawn_shot(&mut g, wx, wy, wz, 3);
    log.borrow_mut().clear();
    g.tick(); // detect HF2
    g.tick(); // fixed one-point wing damage
    g.objs.aliens[shot as usize].sflags |= ASF_COLLDISABLE;
    for _ in 0..6 {
        g.tick();
        if g.vars.pshipflags & PSF_BRKLWING != 0 {
            break;
        }
    }
    assert!(g.vars.pshipflags & PSF_BRKLWING != 0);
    assert!(
        log.borrow().contains(&Ev::Se(SE_WING_DESTRUCT_L)),
        "expected $05 left wing destruct, got {:?}",
        log.borrow()
    );
}

#[test]
fn right_wing_break_plays_destruct_06() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (mut g, _) = spawn_with_boxes(log.clone());
    let rwing = g.coldet.pcbox.rwing.unwrap();
    g.objs.aliens[rwing as usize].hp = 1;
    g.tick();
    let (wx, wy, wz) = {
        let a = &g.objs.aliens[rwing as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    let shot = spawn_shot(&mut g, wx, wy, wz, 3);
    log.borrow_mut().clear();
    g.tick();
    g.tick();
    g.objs.aliens[shot as usize].sflags |= ASF_COLLDISABLE;
    for _ in 0..6 {
        g.tick();
        if g.vars.pshipflags & PSF_BRKRWING != 0 {
            break;
        }
    }
    assert!(g.vars.pshipflags & PSF_BRKRWING != 0);
    assert!(
        log.borrow().contains(&Ev::Se(SE_WING_DESTRUCT_R)),
        "expected $06 right wing destruct, got {:?}",
        log.borrow()
    );
}

#[test]
fn left_wing_soft_hit_plays_07() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (mut g, _) = spawn_with_boxes(log.clone());
    let lwing = g.coldet.pcbox.lwing.unwrap();
    g.tick();
    let (wx, wy, wz) = {
        let a = &g.objs.aliens[lwing as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    spawn_shot(&mut g, wx, wy, wz, 3);
    log.borrow_mut().clear();
    g.tick();
    g.tick();
    assert!(
        log.borrow().contains(&Ev::Se(SE_WING_HIT_L)),
        "expected $07 left wing hit, got {:?}",
        log.borrow()
    );
}

#[test]
fn wire_wing_scrape_plays_14() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (mut g, _) = spawn_with_boxes(log.clone());
    g.vars.pshipflags2 |= PSF2_WIRESHIP;
    let lwing = g.coldet.pcbox.lwing.unwrap();
    g.tick();
    let (wx, wy, wz) = {
        let a = &g.objs.aliens[lwing as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    spawn_shot(&mut g, wx, wy, wz, 3);
    log.borrow_mut().clear();
    g.tick();
    g.tick();
    assert!(
        log.borrow().contains(&Ev::Se(SE_WIRE_SCRAPE)),
        "expected $14 wire scrape, got {:?}",
        log.borrow()
    );
}

#[test]
fn player_death_plays_se03_and_bgm11() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (mut g, _) = spawn_with_boxes(log.clone());
    let body = g.coldet.pcbox.body.unwrap();
    g.tick();
    let (bx, by, bz) = {
        let a = &g.objs.aliens[body as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    g.objs.aliens[body as usize].hp = 3;
    let shot = spawn_shot(&mut g, bx, by, bz, 3);
    log.borrow_mut().clear();
    g.tick(); // detect HF1
    g.tick(); // body hp -> 0
    g.objs.aliens[shot as usize].sflags |= ASF_COLLDISABLE;
    for _ in 0..8 {
        g.tick();
        if log.borrow().contains(&Ev::Se(SE_PLAYER_DOWN)) {
            break;
        }
    }
    assert!(
        log.borrow().contains(&Ev::Se(SE_PLAYER_DOWN)),
        "expected se_playerdown $03, got {:?}",
        log.borrow()
    );
    assert!(
        log.borrow().contains(&Ev::Music(BGM_PLAYER_DOWN)),
        "expected death BGM $11, got {:?}",
        log.borrow()
    );
}

#[test]
fn cockpit_death_finishes_the_authored_ejection_before_crashing() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (mut g, player) = spawn_with_boxes(log.clone());
    g.vars.player_view_mode = PlayerViewMode::Cockpit;
    let body = g.coldet.pcbox.body.expect("body box");
    g.tick();
    let (body_x, body_y, body_z) = {
        let object = &g.objs.aliens[body as usize];
        (object.worldx, object.worldy, object.worldz)
    };
    g.objs.aliens[body as usize].hp = LETHAL_BODY_HEALTH;
    let shot = spawn_shot(&mut g, body_x, body_y, body_z, LETHAL_SHOT_POWER);

    log.borrow_mut().clear();
    g.tick();
    g.tick();
    g.objs.aliens[shot as usize].sflags |= ASF_COLLDISABLE;
    for _ in 0..DEATH_DISPATCH_TICK_LIMIT {
        g.tick();
        if g.vars.player_view_mode == PlayerViewMode::LeavingCockpit {
            break;
        }
    }

    assert_eq!(g.vars.player_view_mode, PlayerViewMode::LeavingCockpit);
    assert_ne!(g.vars.pshipflags2 & PSF2_PLAYERHP0, 0);
    assert_eq!(g.vars.gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD), 0);
    assert_ne!(g.objs.aliens[player as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), COCKPIT_EXIT_FRAMES - 1);
    let transition = g.objs.aliens[player as usize]
        .stratptr
        .expect("cockpit-ejection callback");

    for _ in 0..usize::from(COCKPIT_EXIT_FRAMES - 2) {
        g.call_strat(transition, player);
    }
    assert_eq!(g.vars.sv_u8(sv::PSVAR_BYTE1), 1);
    assert_eq!(g.vars.gameflags & GF_PLAYERDYING, 0);
    assert_eq!(g.objs.aliens[player as usize].stratptr, Some(transition));

    g.call_strat(transition, player);
    assert_eq!(g.vars.player_view_mode, PlayerViewMode::Exterior);
    assert_ne!(g.vars.gameflags & GF_PLAYERDYING, 0);
    assert_eq!(g.vars.gameflags & GF_PLAYERDEAD, 0);
    assert_eq!(g.objs.aliens[player as usize].sflags & ASF_COLLDISABLE, 0);
    assert_ne!(g.objs.aliens[player as usize].stratptr, Some(transition));
    assert!(g.objs.aliens[player as usize].collstratptr.is_some());
    assert!(g.objs.aliens[player as usize].expstratptr.is_some());

    let events = log.borrow();
    assert_eq!(
        events
            .iter()
            .filter(|event| **event == Ev::Se(SE_PLAYER_DOWN))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| **event == Ev::Music(BGM_PLAYER_DOWN))
            .count(),
        1
    );
}

#[test]
fn nukeexp_plays_detonation_30() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = new_game(log.clone());
    let nuke = g.objs.alloc().expect("nuke");
    log.borrow_mut().clear();
    nukeexp_istrat(&mut g, nuke);
    assert!(
        log.borrow().contains(&Ev::Se(SE_NOVA_DET)),
        "expected nova $30, got {:?}",
        log.borrow()
    );
    assert_eq!(g.objs.aliens[nuke as usize].snd2, 0);
}

#[test]
fn right_wing_soft_hit_plays_08() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (mut g, _) = spawn_with_boxes(log.clone());
    let rwing = g.coldet.pcbox.rwing.unwrap();
    g.tick();
    let (wx, wy, wz) = {
        let a = &g.objs.aliens[rwing as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    spawn_shot(&mut g, wx, wy, wz, 3);
    log.borrow_mut().clear();
    g.tick();
    g.tick();
    assert!(
        log.borrow().contains(&Ev::Se(SE_WING_HIT_R)),
        "expected $08 right wing hit, got {:?}",
        log.borrow()
    );
}
