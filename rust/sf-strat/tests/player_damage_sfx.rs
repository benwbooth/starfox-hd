//! Tick 131: player body/wing damage SE + death BGM + nova $30 verify.
//!
//! ROM PSTRATS.ASM pcolB/LW/RW + PLWbrk/PRWbrk + playerdead_Istrat;
//! GSTRATS.ASM nukeexp_Istrat trigse $30.

use sf_core::player_view::PlayerViewMode;
use sf_core::screen_fill_circle::{
    ScreenFillCircleCenter, ScreenFillCirclePhase, COLOR_LEVEL_STEP,
    EXPANDING_INITIAL_RADIUS_SPEED, INITIAL_COLOR_LEVEL,
};
use sf_game::alien::{
    ObjectVisualKind, ACF_COLLTYPE1, AFEXP, AFONFIRE, ASF2_COLLDISABLE, ASF3_REALOBJ,
    ASF4_PLAYEROBJ, ASF_SHADOW,
};
use sf_game::draw::AF_INVIEW_PL;
use sf_game::vars::{
    GameVars, GF_PLAYERDEAD, GF_PLAYERDYING, PFM_DIEFALL, PFM_DIEYROT,
    PLAYER_DEATH_FADE_DELAY_TICKS, PSF2_PLAYERHP0, SPACE_MODE,
};
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
const TERMINAL_CRASH_REMAINING_TICKS: usize = 59;
const TERMINAL_SHIP_POSITION: [i16; 3] = [100, -50, 200];
const TERMINAL_SHIP_VELOCITY: [i16; 3] = [7, -3, 11];
const MEDIUM_EXPLOSION_SPRITE_SHAPE: u16 = 462;
const MEDIUM_EXPLOSION_POLYGON_SHAPE: u16 = 466;
const MEDIUM_EXPLOSION_SPRITE_TICKS: u8 = 6;
const POLYGON_EXPLOSION_TICKS: u8 = 12;
const PLAYER_SPRITE_SCALE_ADJUSTMENT: u8 = 253;
const DEFAULT_PLAYER_DEATH_YAW_STEP: i16 = 128;
const SLOW_PLAYER_DEATH_YAW_STEP: i16 = 42;
const SLOW_PLAYER_DEATH_DIFFICULTY: u8 = 2;
const SLOW_PLAYER_DEATH_STAGE: u8 = 1;
const DEATH_SMOKE_SHAPE: u16 = 358;
const FIRST_SMOKE_CRASH_TICK: u16 = 16;
const CRASH_CAMERA_START_Y: i16 = -200;
const CRASH_SHIP_START_Y: i16 = -100;
const CRASH_CAMERA_EXPECTED_Y: i16 = -188;
const CRASH_TEST_PITCH: i16 = 32 * 256;
const PLAYER_GROUND_IMPACT_SPARKS: usize = 4;

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
    // The focused harness bypasses the authored control-entry strategy that
    // normally clears the ship's startup collision-disable flag.
    g.objs.aliens[p as usize].sflags2 &= !ASF2_COLLDISABLE;
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
    g.objs.aliens[shot as usize].sflags2 |= ASF2_COLLDISABLE;
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
    g.objs.aliens[shot as usize].sflags2 |= ASF2_COLLDISABLE;
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
    g.objs.aliens[shot as usize].sflags2 |= ASF2_COLLDISABLE;
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
    g.objs.aliens[shot as usize].sflags2 |= ASF2_COLLDISABLE;
    for _ in 0..DEATH_DISPATCH_TICK_LIMIT {
        g.tick();
        if g.vars.player_view_mode == PlayerViewMode::LeavingCockpit {
            break;
        }
    }

    assert_eq!(g.vars.player_view_mode, PlayerViewMode::LeavingCockpit);
    assert_ne!(g.vars.pshipflags2 & PSF2_PLAYERHP0, 0);
    assert_eq!(g.vars.gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD), 0);
    assert_ne!(g.objs.aliens[player as usize].sflags2 & ASF2_COLLDISABLE, 0);
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
    assert_eq!(g.objs.aliens[player as usize].sflags2 & ASF2_COLLDISABLE, 0);
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
fn crash_speed_uses_the_source_fixed_step_cadence() {
    const INITIAL_SPEED: i16 = 65;

    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = new_game(log);
    let player = strat_spawn_player(&mut g).expect("player");
    g.vars.set_sv_i16(sv::PLAYER_SPEED, INITIAL_SPEED);
    g.vars.gameframe = 1;

    let death_init = g.objs.aliens[player as usize]
        .expstratptr
        .expect("player death initializer");
    g.call_strat(death_init, player);
    let crash = g.objs.aliens[player as usize]
        .stratptr
        .expect("player crash callback");

    for frame in [2, 3] {
        g.vars.gameframe = frame;
        g.call_strat(crash, player);
        assert_eq!(g.vars.sv_i16(sv::PLAYER_SPEED), INITIAL_SPEED);
    }

    g.vars.gameframe = 4;
    g.call_strat(crash, player);
    assert_eq!(g.vars.sv_i16(sv::PLAYER_SPEED), INITIAL_SPEED - 1);
    assert_eq!(g.objs.aliens[player as usize].vel, 64);
}

#[test]
fn rotating_crash_camera_chases_the_pre_movement_ship_height() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = new_game(log);
    let player = strat_spawn_player(&mut g).expect("player");
    g.vars.playerflymode = PFM_DIEYROT;
    g.vars.gameframe = 1;

    let death_init = g.objs.aliens[player as usize]
        .expstratptr
        .expect("player death initializer");
    g.call_strat(death_init, player);
    let crash = g.objs.aliens[player as usize]
        .stratptr
        .expect("player crash callback");

    g.objs.aliens[player as usize].worldy = CRASH_SHIP_START_Y;
    g.vars.set_sv_i16(sv::PVIEWPOSY, CRASH_CAMERA_START_Y);
    g.vars.set_sv_i16(sv::PLROTX, CRASH_TEST_PITCH);
    g.call_strat(crash, player);

    assert_ne!(
        g.objs.aliens[player as usize].worldy, CRASH_SHIP_START_Y,
        "test pitch must move the ship vertically"
    );
    assert_eq!(g.vars.sv_i16(sv::PVIEWPOSY), CRASH_CAMERA_EXPECTED_Y);
}

#[test]
fn falling_crash_emits_four_sparks_once_on_ground_contact() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = new_game(log);
    let player = strat_spawn_player(&mut g).expect("player");
    g.vars.playerflymode = PFM_DIEFALL;
    g.objs.aliens[player as usize].worldy = -20;

    let death_init = g.objs.aliens[player as usize]
        .expstratptr
        .expect("player death initializer");
    g.call_strat(death_init, player);
    let crash = g.objs.aliens[player as usize]
        .stratptr
        .expect("player crash callback");

    g.objs.aliens[player as usize].worldy = 0;
    let before_impact = g.objs.active_indices().len();
    g.call_strat(crash, player);
    assert_eq!(
        g.objs.active_indices().len(),
        before_impact + PLAYER_GROUND_IMPACT_SPARKS
    );

    g.call_strat(crash, player);
    assert_eq!(
        g.objs.active_indices().len(),
        before_impact + PLAYER_GROUND_IMPACT_SPARKS,
        "ground-impact sparks must not respawn while the ship remains grounded"
    );
}

#[test]
fn death_initializer_rearms_a_grounded_ship_impact_burst() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = new_game(log);
    let player = strat_spawn_player(&mut g).expect("player");
    let follower = g.create_player_dummy().expect("player follower");
    g.vars.playerflymode = PFM_DIEFALL;
    g.objs.aliens[player as usize].worldy = 0;
    g.objs.aliens[player as usize].sflags2 |= ASF2_SFLAG1;
    let before_impact = g.objs.active_indices().len();

    let death_init = g.objs.aliens[player as usize]
        .expstratptr
        .expect("player death initializer");
    g.call_strat(death_init, player);

    assert_eq!(g.vars.internal_playpt, player as i16);
    assert_eq!(g.vars.player_object, follower as i16);

    assert_eq!(
        g.objs.active_indices().len(),
        before_impact + PLAYER_GROUND_IMPACT_SPARKS
    );
}

#[test]
fn death_yaw_step_uses_the_authored_stage_specific_speed() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut ordinary = new_game(log.clone());
    let _ordinary_player = strat_spawn_player(&mut ordinary).expect("ordinary player");
    assert_eq!(
        ordinary.vars.strategy.player_death_yaw_step,
        DEFAULT_PLAYER_DEATH_YAW_STEP
    );

    let mut slow = new_game(log);
    slow.vars.shared.difficulty_level = SLOW_PLAYER_DEATH_DIFFICULTY;
    slow.vars.shared.stage = SLOW_PLAYER_DEATH_STAGE;
    let slow_player = strat_spawn_player(&mut slow).expect("slow-turning player");
    let death_init = slow.objs.aliens[slow_player as usize]
        .expstratptr
        .expect("player death initializer");
    slow.call_strat(death_init, slow_player);
    assert_eq!(
        slow.vars.strategy.player_death_yaw_step,
        SLOW_PLAYER_DEATH_YAW_STEP
    );
}

#[test]
fn rotating_crash_waits_for_the_flash_window_before_emitting_smoke() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = new_game(log);
    let player = strat_spawn_player(&mut g).expect("player");
    g.vars.playerflymode = PFM_DIEYROT;
    g.vars.gameframe = 1;

    let death_init = g.objs.aliens[player as usize]
        .expstratptr
        .expect("player death initializer");
    g.call_strat(death_init, player);
    let crash = g.objs.aliens[player as usize]
        .stratptr
        .expect("player crash callback");

    for frame in 2..FIRST_SMOKE_CRASH_TICK {
        g.vars.gameframe = frame;
        g.call_strat(crash, player);
        assert_eq!(
            g.objs
                .active_indices()
                .into_iter()
                .filter(|slot| g.objs.aliens[*slot as usize].shape == DEATH_SMOKE_SHAPE)
                .count(),
            0
        );
    }

    g.vars.gameframe = FIRST_SMOKE_CRASH_TICK;
    g.call_strat(crash, player);
    let smoke = g.objs.aliens[player as usize]
        .next
        .expect("smoke linked immediately after the player");
    assert_eq!(g.objs.aliens[smoke as usize].shape, DEATH_SMOKE_SHAPE);
}

#[test]
fn rotating_crash_does_not_emit_smoke_after_fire_attaches() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = new_game(log);
    let player = strat_spawn_player(&mut g).expect("player");
    g.vars.playerflymode = PFM_DIEYROT;
    g.vars.gameframe = 1;

    let death_init = g.objs.aliens[player as usize]
        .expstratptr
        .expect("player death initializer");
    g.call_strat(death_init, player);
    let crash = g.objs.aliens[player as usize]
        .stratptr
        .expect("player crash callback");
    g.objs.aliens[player as usize].flags |= AFONFIRE;
    g.objs.aliens[player as usize].sbyte1 = (FIRST_SMOKE_CRASH_TICK - 1) as u8;

    g.vars.gameframe = FIRST_SMOKE_CRASH_TICK;
    g.call_strat(crash, player);
    assert!(g.objs.aliens[player as usize].next.is_none());
}

#[test]
fn terminal_explosion_builds_the_retail_anchor_particle_and_fade_state() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut g = new_game(log.clone());
    let player = strat_spawn_player(&mut g).expect("player");
    let death_init = g.objs.aliens[player as usize]
        .expstratptr
        .expect("player death initializer");

    g.call_strat(death_init, player);
    let crash = g.objs.aliens[player as usize]
        .stratptr
        .expect("player crash callback");
    for _ in 1..TERMINAL_CRASH_REMAINING_TICKS {
        g.call_strat(crash, player);
    }

    {
        let ship = &mut g.objs.aliens[player as usize];
        ship.worldx = TERMINAL_SHIP_POSITION[0];
        ship.worldy = TERMINAL_SHIP_POSITION[1];
        ship.worldz = TERMINAL_SHIP_POSITION[2];
        ship.vx = TERMINAL_SHIP_VELOCITY[0];
        ship.vy = TERMINAL_SHIP_VELOCITY[1];
        ship.vz = TERMINAL_SHIP_VELOCITY[2];
        ship.sflags |= ASF_SHADOW;
    }
    log.borrow_mut().clear();
    g.call_strat(crash, player);

    assert_eq!(
        g.vars.gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD),
        GF_PLAYERDYING | GF_PLAYERDEAD
    );
    assert_eq!(g.vars.sv_u8(sv::LIVES), 2);
    assert_eq!(
        g.vars.player_death_fade_delay,
        PLAYER_DEATH_FADE_DELAY_TICKS
    );
    assert!(log.borrow().contains(&Ev::Se(SE_PLAYER_DOWN)));

    let anchor = g.vars.internal_playpt as u16;
    assert_ne!(anchor, player);
    let anchor_object = g.objs.aliens[anchor as usize];
    assert!(anchor_object.active);
    assert_eq!(anchor_object.sflags3 & ASF3_REALOBJ, 0);
    assert_ne!(anchor_object.sflags4 & ASF4_PLAYEROBJ, 0);
    assert_eq!(
        [
            anchor_object.worldx,
            anchor_object.worldy,
            anchor_object.worldz,
        ],
        [
            TERMINAL_SHIP_POSITION[0] + TERMINAL_SHIP_VELOCITY[0],
            TERMINAL_SHIP_POSITION[1] + TERMINAL_SHIP_VELOCITY[1],
            TERMINAL_SHIP_POSITION[2] + TERMINAL_SHIP_VELOCITY[2],
        ]
    );
    assert_eq!(
        [anchor_object.vx, anchor_object.vy, anchor_object.vz],
        TERMINAL_SHIP_VELOCITY
    );
    assert!(anchor_object.stratptr.is_none());
    assert_eq!(g.vars.strategy.view_target_object, anchor as i16);
    assert_eq!(g.vars.strategy.circle_object, anchor as i16);
    assert_eq!(
        g.vars.screen_fill_circle.center,
        ScreenFillCircleCenter::Object(anchor + 1)
    );
    assert_eq!(
        g.vars.screen_fill_circle.phase,
        ScreenFillCirclePhase::RedExpanding
    );
    assert_eq!(
        g.vars.screen_fill_circle.radius,
        EXPANDING_INITIAL_RADIUS_SPEED as u16
    );
    assert_eq!(
        g.vars.screen_fill_circle.red,
        INITIAL_COLOR_LEVEL + COLOR_LEVEL_STEP
    );

    let ship = g.objs.aliens[player as usize];
    assert_eq!(ship.hp, 0);
    assert_eq!([ship.vx, ship.vy, ship.vz], [0, 0, 0]);
    assert_eq!(ship.sflags & ASF_SHADOW, 0);
    assert_eq!(ship.stratptr, ship.collstratptr);
    assert_eq!(ship.stratptr, ship.expstratptr);
    assert!(ship.stratptr.is_some());

    let particle = g
        .objs
        .active_indices()
        .into_iter()
        .find(|slot| *slot != player && *slot != anchor)
        .expect("large particle object");
    let particle_object = g.objs.aliens[particle as usize];
    assert_eq!(
        [
            particle_object.worldx,
            particle_object.worldy,
            particle_object.worldz,
        ],
        TERMINAL_SHIP_POSITION
    );
    assert!(particle_object.stratptr.is_some());
    assert_eq!(
        &g.objs.active_indices()[..3],
        &[player, particle, anchor],
        "source allocations must remain linked immediately after the crashing ship"
    );

    // The retail draw pass marks the visible ship before the following
    // strategy sweep; that gate selects the live mesh/sprite explosion.
    g.objs.aliens[player as usize].flags |= AF_INVIEW_PL;
    g.tick();
    assert!(g.objs.aliens[player as usize].active);
    assert_eq!(
        g.objs.aliens[player as usize].shape,
        MEDIUM_EXPLOSION_POLYGON_SHAPE
    );
    assert_ne!(g.objs.aliens[player as usize].flags & AFEXP, 0);
    assert_eq!(
        g.objs.aliens[player as usize].count1,
        POLYGON_EXPLOSION_TICKS
    );
    assert!(g.objs.aliens[anchor as usize].active);
    assert_ne!(g.objs.aliens[particle as usize].flags & AFEXP, 0);
    let sprite = g
        .objs
        .active_indices()
        .into_iter()
        .find(|slot| g.objs.aliens[*slot as usize].visual_kind == ObjectVisualKind::ScaledSprite)
        .expect("scaled terminal explosion sprite");
    assert_eq!(
        g.objs.aliens[sprite as usize].shape,
        MEDIUM_EXPLOSION_SPRITE_SHAPE
    );
    assert_eq!(
        g.objs.aliens[sprite as usize].tx,
        PLAYER_SPRITE_SCALE_ADJUSTMENT
    );
    assert_eq!(
        g.vars.player_death_fade_delay,
        PLAYER_DEATH_FADE_DELAY_TICKS - 1
    );
    assert_eq!(g.vars.sv_u8(sv::LIVES), 2);

    for _ in 0..MEDIUM_EXPLOSION_SPRITE_TICKS {
        g.tick();
    }
    assert!(!g.objs.aliens[sprite as usize].active);
    assert!(g.objs.aliens[player as usize].active);
    for _ in MEDIUM_EXPLOSION_SPRITE_TICKS..POLYGON_EXPLOSION_TICKS {
        g.tick();
    }
    assert!(!g.objs.aliens[player as usize].active);
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
