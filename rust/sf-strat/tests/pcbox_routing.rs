//! pcbox (player collision-proxy box) routing tests.
//!
//! ROM model (see rust/sf-game/src/coldet.rs pcbox section + STRAT/PSTRATS.ASM):
//! the ship owns the `playerB_col` three-box collider; three colldisable proxy
//! objects (`pcboxobj_B/LW/RW`) carry damage state after hit-flag routing.
//! These tests exercise the routing through the real collision tick
//! (`Game::tick` -> coldet_generate + coldet_run -> box collide-strat).

use sf_game::alien::{ACF_COLLTYPE1, ACF_FIRSTFRAME, ASF_COLLDISABLE, ASF_COLLIDE};
use sf_game::vars::{GameVars, GF_PLAYERDYING, HARD_AP, HARD_HP, SPACE_MODE};
use sf_game::{Game, Hooks};
use sf_strat::common::{sv, StratRam};
use sf_strat::player::{pcbox_attach, strat_spawn_player};

struct NopHooks;
impl Hooks for NopHooks {
    fn shape_extents(&self, shape: u16) -> Option<(i16, i16, i16)> {
        const SHAPE_ENEMY_SHOT: u16 = 511;
        const SHAPE_WIDE_COLLIDER: u16 = 512;
        const SHAPE_DEEP_COLLIDER: u16 = 513;
        const SHAPE_ARCH: u16 = 228;
        match shape {
            SHAPE_ENEMY_SHOT => Some((1, 1, 1)),
            SHAPE_WIDE_COLLIDER => Some((80, 70, 30)),
            SHAPE_DEEP_COLLIDER => Some((72, 74, 60)),
            SHAPE_ARCH => Some((120, 160, 40)),
            _ => None,
        }
    }
}

#[test]
fn arch_collision_uses_its_three_boxes_instead_of_header_bounds() {
    const SHAPE_ARCH: u16 = 228;

    let spawn_arch = |game: &mut Game| {
        let object = game.objs.alloc().expect("arch slot");
        let arch = &mut game.objs.aliens[object as usize];
        arch.shape = SHAPE_ARCH;
        arch.hp = HARD_HP;
        arch.collflags = ACF_COLLTYPE1;
        object
    };

    let (mut clear_opening, player) = spawn_with_boxes();
    clear_opening.objs.aliens[player as usize].worldx = 35;
    clear_opening.objs.aliens[player as usize].worldy = -20;
    clear_opening.objs.aliens[player as usize].worldz = 31;
    clear_opening.objs.aliens[player as usize].collflags &= !ACF_FIRSTFRAME;
    spawn_arch(&mut clear_opening);
    clear_opening.coldet_generate_list();
    clear_opening.coldet_run();
    assert_eq!(clear_opening.objs.aliens[player as usize].hitflags, 0);

    let (mut hit_right_post, player) = spawn_with_boxes();
    hit_right_post.objs.aliens[player as usize].worldx = 100;
    hit_right_post.objs.aliens[player as usize].worldy = -60;
    hit_right_post.objs.aliens[player as usize].collflags &= !ACF_FIRSTFRAME;
    spawn_arch(&mut hit_right_post);
    hit_right_post.coldet_generate_list();
    hit_right_post.coldet_run();
    assert_eq!(hit_right_post.objs.aliens[player as usize].hitflags, 1);
}

#[test]
fn nonplayer_objects_also_use_the_arch_box_list() {
    const SHAPE_ARCH: u16 = 228;
    const SHAPE_DEEP_COLLIDER: u16 = 513;

    let mut game = new_game();
    let arch = game.objs.alloc().expect("arch slot");
    game.objs.aliens[arch as usize].shape = SHAPE_ARCH;
    game.objs.aliens[arch as usize].hp = 1;
    game.objs.aliens[arch as usize].collflags = ACF_COLLTYPE1;

    let other = game.objs.alloc().expect("collider slot");
    let collider = &mut game.objs.aliens[other as usize];
    collider.shape = SHAPE_DEEP_COLLIDER;
    collider.worldx = 24;
    collider.worldy = 15;
    collider.worldz = -84;
    collider.hp = 1;
    collider.collflags = sf_game::alien::ACF_COLLTYPE2;

    game.coldet_generate_list();
    game.coldet_run();

    assert_eq!(game.objs.aliens[arch as usize].sflags & ASF_COLLIDE, 0);
    assert_eq!(game.objs.aliens[other as usize].sflags & ASF_COLLIDE, 0);
}

#[test]
fn player_box_scan_preserves_source_list_order_asymmetry() {
    const SHAPE_WIDE_COLLIDER: u16 = 512;
    const BODY_HIT: u8 = 1;
    const LEFT_WING_HIT: u8 = 2;
    const COLLIDER_POSITION: (i16, i16, i16) = (-85, 33, 9);

    let spawn_wide_collider = |game: &mut Game| {
        let object = game.objs.alloc().expect("wide collider slot");
        let collider = &mut game.objs.aliens[object as usize];
        collider.shape = SHAPE_WIDE_COLLIDER;
        collider.worldx = COLLIDER_POSITION.0;
        collider.worldy = COLLIDER_POSITION.1;
        collider.worldz = COLLIDER_POSITION.2;
        collider.hp = 1;
        collider.collflags = ACF_COLLTYPE1;
        object
    };

    let (mut player_first, player) = spawn_with_boxes();
    player_first.objs.aliens[player as usize].collflags &= !ACF_FIRSTFRAME;
    let collider = spawn_wide_collider(&mut player_first);
    player_first.objs.active_move_after(collider, player);
    player_first.coldet_generate_list();
    player_first.coldet_run();
    assert_eq!(player_first.objs.aliens[player as usize].hitflags, BODY_HIT);

    let (mut collider_first, player) = spawn_with_boxes();
    collider_first.objs.aliens[player as usize].collflags &= !ACF_FIRSTFRAME;
    spawn_wide_collider(&mut collider_first);
    collider_first.coldet_generate_list();
    collider_first.coldet_run();
    assert_eq!(
        collider_first.objs.aliens[player as usize].hitflags,
        BODY_HIT | LEFT_WING_HIT
    );
}

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
                                  // A real enemy shot carries a nonzero laser shape (SHAPE_ELASER2), distinct
                                  // from the shape-0 pcbox collision boxes. Needed since coldet_run now applies
                                  // the ROM chkcoll0 same-shape gate (equal al_shape -> skip); leaving this 0
                                  // would make the shot share the boxes' shape and wrongly skip the hit.
    al.shape = 511; // SHAPE_ELASER2
    s
}

#[test]
fn attach_keeps_ship_live_and_builds_three_colldisable_damage_boxes() {
    let (g, p) = spawn_with_boxes();
    // The ship owns playerB_col; only its state proxies are colldisable.
    assert_eq!(g.objs.aliens[p as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[p as usize].hp, HARD_HP);
    assert_eq!(g.objs.aliens[p as usize].ap, HARD_AP);
    let pc = g.coldet.pcbox;
    assert!(pc.attached());
    assert_eq!(pc.player, Some(p));
    assert!(pc.body.is_some() && pc.lwing.is_some() && pc.rwing.is_some());
    // Body box carries the body HP (40); wings carry 5.
    assert_eq!(g.objs.aliens[pc.body.unwrap() as usize].hp, 40);
    assert_eq!(g.objs.aliens[pc.lwing.unwrap() as usize].hp, 5);
    assert!(g.objs.aliens[pc.body.unwrap() as usize].sflags & ASF_COLLDISABLE != 0);
    assert!(g.objs.aliens[pc.lwing.unwrap() as usize].sflags & ASF_COLLDISABLE != 0);
}

#[test]
fn enemy_shot_at_body_box_routes_hit_to_player() {
    let (mut g, p) = spawn_with_boxes();
    let body = g.coldet.pcbox.body.unwrap();
    g.tick(); // position the boxes
    let (bx, by, bz) = {
        let a = &g.objs.aliens[body as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    let _ = spawn_shot(&mut g, bx, by, bz, 3);

    let hits0 = g.vars.sv_u8(sv::PNUMHITS);

    // Frame 1: coldet tests the ship's exact multi-box list, sets HF1 on the
    // ship, but does not directly damage either ship or proxy.
    g.tick();
    assert_eq!(g.objs.aliens[body as usize].hp, 40);
    assert_eq!(g.objs.aliens[p as usize].hitflags & 1, 1);
    assert!(g.objs.aliens[p as usize].sflags & ASF_COLLIDE != 0);

    // Frame 2: playercoll routes HF1 to the body proxy; pcolB applies AP.
    g.tick();
    assert_eq!(g.objs.aliens[body as usize].hp, 37);
    assert!(
        g.vars.sv_u8(sv::PNUMHITS) > hits0,
        "hit should increment pnumhits"
    );
    // pcolB arms the ship timer and queues the body screen flash. This test
    // deliberately removes the normal player strat, so the next frame's
    // playermove hitflash/nohitaffect toggle is not expected here.
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
    // Prime the body so one hit is lethal and spawn the shot at its centre.
    g.tick();
    let (bx, by, bz) = {
        let a = &g.objs.aliens[body as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    g.objs.aliens[body as usize].hp = 3;
    let shot = spawn_shot(&mut g, bx, by, bz, 3);

    g.tick(); // detect: ship HF1
    assert_eq!(g.objs.aliens[body as usize].hp, 3);
    g.tick(); // route + pcolB do_coll: body hp 3 -> 0
    assert_eq!(g.objs.aliens[body as usize].hp, 0);
    // End the sustained pair. The ROM gives end-collision callbacks priority
    // over an explosion callback while LCOLLIDE drains, so allow those exact
    // handoff frames before pcolBexp kills the player.
    g.objs.aliens[shot as usize].sflags |= ASF_COLLDISABLE;
    for _ in 0..8 {
        g.tick();
        if g.vars.gameflags & GF_PLAYERDYING != 0 {
            break;
        }
    }

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
    // ROM pwingcol applies exactly one damage to the wing per cooldown,
    // irrespective of attacker AP.
    g.objs.aliens[rwing as usize].hp = 1;
    // Read the right-wing box position after one positioning pass, then hit it.
    g.tick(); // positions boxes; no shot yet
    let (wx, wy, wz) = {
        let a = &g.objs.aliens[rwing as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    let shot = spawn_shot(&mut g, wx, wy, wz, 3);
    g.tick(); // detect: ship HF3
    assert_eq!(g.objs.aliens[rwing as usize].hp, 1);
    g.tick(); // route + pwingcol: fixed 1 wing damage -> hp 0
    assert_eq!(g.objs.aliens[rwing as usize].hp, 0);
    g.objs.aliens[shot as usize].sflags |= ASF_COLLDISABLE;
    for _ in 0..6 {
        g.tick();
        if g.vars.pshipflags & 16 != 0 {
            break;
        }
    }

    const PSF_BRKRWING: u8 = 16;
    assert!(
        g.vars.pshipflags & PSF_BRKRWING != 0,
        "right wing should break"
    );
    assert_eq!(g.coldet.pcbox.rwing, Some(rwing));
    assert_eq!(g.objs.aliens[rwing as usize].hp, 0xff);
    assert_eq!(g.objs.aliens[rwing as usize].ap, 0);
    assert!(g.objs.aliens[rwing as usize].sflags & ASF_COLLDISABLE != 0);
    // All proxy slots remain addressable; later HF3 hits follow the broken-wing
    // bounce-to-body path in the ROM.
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

#[test]
fn playerb_col_uses_exact_extents_and_strict_edge() {
    // A one-unit projectile centred at x=10 overlaps the body because
    // |dx|=10 < 10+1; x=11 is the ROM COLDET strict miss boundary.
    let (mut hit, p) = spawn_with_boxes();
    hit.tick(); // clear the player's spawn-time firstframe flag
    spawn_shot(&mut hit, 10, 0, 0, 0);
    hit.coldet_generate_list();
    hit.coldet_run();
    assert_eq!(hit.objs.aliens[p as usize].hitflags, 0x01);

    let (mut miss, p) = spawn_with_boxes();
    miss.tick();
    spawn_shot(&mut miss, 11, 0, 0, 0);
    miss.coldet_generate_list();
    miss.coldet_run();
    assert_eq!(miss.objs.aliens[p as usize].hitflags, 0);
}

#[test]
fn playerb_col_rotates_wing_offsets_by_ship_roll_only() {
    let (mut g, p) = spawn_with_boxes();
    g.objs.aliens[p as usize].rotz = 64; // quarter turn
    g.tick();
    let (dx, dy, _) = sf_core::snes_trig::strat_roffs_roll(64, 33, 13, 0);
    spawn_shot(&mut g, dx, dy, 0, 0);
    g.coldet_generate_list();
    g.coldet_run();
    assert_eq!(g.objs.aliens[p as usize].hitflags, 0x04);
}
