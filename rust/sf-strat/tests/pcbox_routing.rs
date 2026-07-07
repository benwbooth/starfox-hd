//! pcbox (player collision-proxy box) routing tests.
//!
//! ROM model (see rust/sf-game/src/coldet.rs pcbox section + STRAT/PSTRATS.ASM):
//! the ship does not take enemy hits on its own body; three proxy boxes
//! (`pcboxobj_B/LW/RW`) carry the collision and route hits back to the ship.
//! These tests exercise the routing through the real collision tick
//! (`Game::tick` -> coldet_generate + coldet_run -> box collide-strat).

use sf_game::alien::{
    ACF_COLLTYPE1, ASF_COLLDISABLE, ASF_COLLIDE, ASF_HITFLASH, ASF_NOHITAFFECT,
};
use sf_game::vars::{GameVars, GF_PLAYERDYING, SPACE_MODE};
use sf_game::{Game, Hooks};
use sf_strat::common::{sv, StratRam};
use sf_strat::player::{pcbox_attach, strat_spawn_player};

struct NopHooks;
impl Hooks for NopHooks {}

fn new_game() -> Game {
    let mut g = Game::with_hooks(Box::new(NopHooks));
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

/// Spawn the ship, attach the pcboxes, and neutralise the heavy per-frame ship
/// strat (we only want the boxes + collision to run).
fn spawn_with_boxes() -> (Game, u16) {
    let mut g = new_game();
    let p = strat_spawn_player(&mut g).unwrap();
    g.objs.aliens[p as usize].worldx = 0;
    g.objs.aliens[p as usize].worldy = 0;
    g.objs.aliens[p as usize].worldz = 0;
    g.objs.aliens[p as usize].rotz = 0;
    g.objs.aliens[p as usize].stratptr = None; // don't run playermove in the test
    assert!(pcbox_attach(&mut g, p), "pcbox_attach failed");
    (g, p)
}

/// Spawn a stationary enemy shot at `(x,y,z)`.
fn spawn_shot(g: &mut Game, x: i16, y: i16, z: i16, ap: u8) -> u16 {
    let s = g.objs.alloc().unwrap();
    let al = &mut g.objs.aliens[s as usize];
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    al.hp = 5;
    al.ap = ap;
    al.collflags = ACF_COLLTYPE1; // enemy weapon type
    s
}

#[test]
fn attach_makes_ship_colldisable_and_builds_three_boxes() {
    let (g, p) = spawn_with_boxes();
    // Ship no longer self-collides.
    assert!(g.objs.aliens[p as usize].sflags & ASF_COLLDISABLE != 0);
    let pc = g.coldet.pcbox;
    assert!(pc.attached());
    assert_eq!(pc.player, Some(p));
    assert!(pc.body.is_some() && pc.lwing.is_some() && pc.rwing.is_some());
    // Body box carries the body HP (40); wings carry 5.
    assert_eq!(g.objs.aliens[pc.body.unwrap() as usize].hp, 40);
    assert_eq!(g.objs.aliens[pc.lwing.unwrap() as usize].hp, 5);
}

#[test]
fn enemy_shot_at_body_box_routes_hit_to_player() {
    let (mut g, p) = spawn_with_boxes();
    let body = g.coldet.pcbox.body.unwrap();
    // Shot overlapping the body box (ship centre).
    spawn_shot(&mut g, 0, 0, 0, 3);

    let hits0 = g.vars.sv_u8(sv::PNUMHITS);

    // Frame 1: coldet detects the overlap and docks the body box HP + sets its
    // collide flag.
    g.tick();
    assert!(
        g.objs.aliens[body as usize].hp < 40,
        "body box should take damage (was {})",
        g.objs.aliens[body as usize].hp
    );
    assert!(g.objs.aliens[body as usize].sflags & ASF_COLLIDE != 0);

    // Frame 2: the box collide-strat routes the hit onto the ship.
    g.tick();
    assert!(
        g.vars.sv_u8(sv::PNUMHITS) > hits0,
        "hit should increment pnumhits"
    );
    // Ship flashes + is briefly invulnerable, screenflash queued (body type 0).
    assert!(g.objs.aliens[p as usize].sflags & ASF_HITFLASH != 0);
    assert!(g.objs.aliens[p as usize].sflags & ASF_NOHITAFFECT != 0);
    assert_eq!(g.objs.aliens[p as usize].sbyte1, 7); // player_hitflashfrms
    assert_eq!(g.vars.sv_u8(sv::SCREENFLASHCNT), 4);
    assert_eq!(g.vars.sv_u8(sv::SCREENFLASHTYPE), 0);
    // Ship survives a non-fatal hit.
    assert_ne!(g.vars.gameflags & GF_PLAYERDYING, GF_PLAYERDYING);
}

#[test]
fn body_box_destroyed_triggers_death_and_detaches_boxes() {
    let (mut g, p) = spawn_with_boxes();
    let body = g.coldet.pcbox.body.unwrap();
    let lwing = g.coldet.pcbox.lwing.unwrap();
    // Prime the body box so one hit is lethal.
    g.objs.aliens[body as usize].hp = 3;
    spawn_shot(&mut g, 0, 0, 0, 3);

    g.tick(); // detect + do_coll: body hp 3 -> 0
    assert_eq!(g.objs.aliens[body as usize].hp, 0);
    g.tick(); // route: body hp==0 -> death + detach

    // Death sequence engaged.
    assert_eq!(g.vars.gameflags & GF_PLAYERDYING, GF_PLAYERDYING);
    // Boxes detached: state cleared, boxes colldisable so they leave the
    // collision list.
    assert!(!g.coldet.pcbox.attached());
    assert!(g.objs.aliens[body as usize].sflags & ASF_COLLDISABLE != 0);
    assert!(g.objs.aliens[lwing as usize].sflags & ASF_COLLDISABLE != 0);

    // A fresh overlapping shot must NOT re-damage the detached boxes nor
    // re-enter the crash (no panic, still dying).
    let dead_frames_before = g.objs.aliens[p as usize].sbyte1;
    spawn_shot(&mut g, 0, 0, 0, 3);
    g.tick();
    g.tick();
    assert_eq!(g.vars.gameflags & GF_PLAYERDYING, GF_PLAYERDYING);
    // The crash strat keeps advancing (sbyte1 counts up), proving it is not
    // frozen by re-collision.
    assert!(g.objs.aliens[p as usize].sbyte1 >= dead_frames_before);
}

#[test]
fn wing_box_destroyed_breaks_wing_and_drops_from_collision() {
    let (mut g, _p) = spawn_with_boxes();
    let rwing = g.coldet.pcbox.rwing.unwrap();
    g.objs.aliens[rwing as usize].hp = 3;
    // Read the right-wing box position after one positioning pass, then hit it.
    g.tick(); // positions boxes; no shot yet
    let (wx, wy, wz) = {
        let a = &g.objs.aliens[rwing as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    spawn_shot(&mut g, wx, wy, wz, 3);
    g.tick(); // detect + do_coll on the wing box -> hp 0
    assert_eq!(g.objs.aliens[rwing as usize].hp, 0);
    g.tick(); // route -> wing break

    const PSF_BRKRWING: u8 = 16;
    assert!(g.vars.pshipflags & PSF_BRKRWING != 0, "right wing should break");
    assert!(g.coldet.pcbox.rwing.is_none(), "broken wing box detached");
    assert!(g.objs.aliens[rwing as usize].sflags & ASF_COLLDISABLE != 0);
    // Body + left wing still attached.
    assert!(g.coldet.pcbox.body.is_some());
    assert!(g.coldet.pcbox.lwing.is_some());
}

/// Regression: with NO pcbox attached the historical direct model is
/// unchanged — the ship itself is a normal collider (not colldisable) and
/// takes hits directly.
#[test]
fn unattached_keeps_direct_model() {
    let mut g = new_game();
    let p = strat_spawn_player(&mut g).unwrap();
    assert!(!g.coldet.pcbox.attached());
    assert_eq!(
        g.objs.aliens[p as usize].sflags & ASF_COLLDISABLE,
        0,
        "ship must remain a direct collider when no boxes are attached"
    );
}
