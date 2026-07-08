//! End-to-end regression tests for two playtest bugs, driving the REAL Shell
//! gameplay path (register_strats + spawn hooks, map VM, coldet), not the
//! isolated strat unit harness.
//!
//! BUG A — "collision not working, ship/lasers pass through": the player's
//! collision-proxy boxes (pcbox, ROM GSTRATS player setup) were never attached
//! in live gameplay, so enemy shots/contact could not hit the ship. Fixed by
//! wiring `Game::pcbox_attach_player` into the shell's gameplay-start.
//!
//! BUG B — "the arwing gets stuck and can't move": routes 2/3 (and every
//! route-lane level) stashed their inline CODE65816 callbacks in name-keyed
//! records that never reached the map VM, so it halted forever at
//! `level_scramble_keep_player_strat` — before the exit-base setup that hands
//! control back. Fixed by registering those callbacks at level load.

use sf_core::pad;
use sf_game::alien::{ACF_FIRSTFRAME, ASF4_PLAYEROBJ, ASF_COLLDISABLE};
use sf_game::shell::{GameState, Shell};
use sf_game::vars::{COLLTYPE_ENEMY1, PSF_NOCTRL};
use sf_strat::common::StratRam;
use sf_strat::player::player_sv as sv;

const SHAPE_ELASER2: u16 = 511;

fn make_shell() -> Shell {
    let mut shell = Shell::new();
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    shell.set_spawn_player(Box::new(|game, newmap| {
        if let Some(idx) = sf_strat::player::strat_spawn_player(game) {
            if newmap == sf_map::catalog::map_id::M1_1
                || newmap == sf_map::catalog::map_id::M2_1
                || newmap == sf_map::catalog::map_id::M3_1
            {
                sf_strat::player::strat_player_opening_init(game, idx);
            }
        }
    }));
    shell
}

/// Drive Title -> PlanetSelect -> (DOWN x route) -> gameplay, then tick until
/// the opening sequence returns control (PSF_NOCTRL clears) or `max` frames.
fn drive_to_controllable(route_downs: u32, max: u32) -> Shell {
    let mut sh = make_shell();
    for _ in 0..4 {
        if sh.state() == GameState::Title {
            break;
        }
        sh.tick(0);
    }
    sh.tick(pad::START); // Title -> PlanetSelect
    sh.tick(0);
    for _ in 0..route_downs {
        sh.tick(pad::DOWN);
        sh.tick(0);
    }
    sh.tick(pad::START); // select -> begin gameplay
    for _ in 0..max {
        if sh.state() == GameState::Playing && sh.game.vars.pshipflags & PSF_NOCTRL == 0 {
            break;
        }
        sh.tick(0);
    }
    sh
}

fn player_slot(sh: &Shell) -> u16 {
    let p = sh.game.vars.internal_playpt;
    assert!(p >= 0 && sh.game.objs.aliens[p as usize].active, "no live player");
    p as u16
}

fn player_worldx(sh: &Shell) -> i16 {
    sh.game.objs.aliens[player_slot(sh) as usize].worldx
}

// ============================================================
// BUG B — control returns after the opening on all three routes.
// ============================================================
#[test]
fn every_route_regains_control_and_responds_to_steering() {
    for route in 0..3u32 {
        let mut sh = drive_to_controllable(route, 700);
        assert_eq!(sh.state(), GameState::Playing, "route {route}: not in gameplay");
        assert_eq!(
            sh.game.vars.pshipflags & PSF_NOCTRL,
            0,
            "route {route}: player never regained control after the opening (BUG B)"
        );
        // Hold LEFT: +worldx projects screen-right, so LEFT must DECREASE worldx.
        let x0 = player_worldx(&sh);
        for _ in 0..15 {
            sh.tick(pad::LEFT);
        }
        let x1 = player_worldx(&sh);
        assert!(
            x1 < x0,
            "route {route}: LEFT did not move the ship (x {x0} -> {x1}) — arwing stuck (BUG B)"
        );
    }
}

// ============================================================
// BUG A — the shell attaches the pcbox and enemy fire hits the ship.
// ============================================================
#[test]
fn gameplay_start_attaches_pcbox_and_enemy_shot_damages_player() {
    let mut sh = drive_to_controllable(0, 700);
    let p = player_slot(&sh);

    // The per-level setup ran: the 3 proxy boxes exist and the ship is
    // colldisable (the boxes carry its collision). Without the fix pcbox was
    // never attached and this is false.
    assert!(
        sh.game.coldet.pcbox.attached(),
        "pcbox not attached in the real gameplay path (BUG A)"
    );
    assert!(
        sh.game.objs.aliens[p as usize].sflags & ASF_COLLDISABLE != 0,
        "ship should be colldisable while the boxes carry collision"
    );

    // Freeze the ship so the seeded stationary shot stays overlapping (the boxes
    // still re-park on it via their own strats each frame).
    sh.game.objs.aliens[p as usize].stratptr = None;
    let (px, py, pz) = {
        let a = &sh.game.objs.aliens[p as usize];
        (a.worldx, a.worldy, a.worldz)
    };

    let hits_before = sh.game.vars.sv_u8(sv::PNUMHITS);

    // Seed a stationary enemy shot right on the ship centre (= the body box).
    let s = sh.game.objs.alloc().expect("free slot for enemy shot");
    {
        let a = &mut sh.game.objs.aliens[s as usize];
        a.worldx = px;
        a.worldy = py;
        a.worldz = pz;
        a.hp = 5;
        a.ap = 5;
        a.collflags = COLLTYPE_ENEMY1;
        a.shape = SHAPE_ELASER2; // nonzero, distinct from the shape-0 boxes
        a.sflags4 = 0;
        a.collcount = 1;
        a.collflags &= !ACF_FIRSTFRAME;
    }

    // A couple of real gameplay ticks (run_strategies re-parks the boxes, then
    // coldet_generate_list + coldet_run route the hit into the ship).
    for _ in 0..3 {
        // Keep the shot glued to the (frozen) ship centre each frame.
        let a = &mut sh.game.objs.aliens[s as usize];
        a.worldx = px;
        a.worldy = py;
        a.worldz = pz;
        sh.tick(0);
    }

    let hits_after = sh.game.vars.sv_u8(sv::PNUMHITS);
    assert!(
        hits_after > hits_before,
        "enemy shot did not damage the player through the pcbox (before={hits_before} after={hits_after}) (BUG A)"
    );
}

// ============================================================
// BUG A — the player's fired laser collides with and damages an enemy.
// ============================================================
#[test]
fn player_laser_damages_a_seeded_enemy() {
    let mut sh = drive_to_controllable(0, 700);
    let p = player_slot(&sh);

    // Fire through the real playerfire path (FIREDELAY gates the first frame,
    // so hold Y until a bolt actually spawns).
    let find_bolt = |sh: &Shell| -> Option<u16> {
        let mut cur = sh.game.objs.active_head;
        while let Some(i) = cur {
            let a = &sh.game.objs.aliens[i as usize];
            if i != p && a.shape == SHAPE_ELASER2 && a.sflags4 & ASF4_PLAYEROBJ == 0 {
                return Some(i);
            }
            cur = a.next;
        }
        None
    };
    let mut laser = None;
    for _ in 0..8 {
        sh.tick(pad::Y);
        if let Some(l) = find_bolt(&sh) {
            laser = Some(l);
            break;
        }
    }
    let laser = laser.expect("player Y-fire should spawn an elaser2 bolt");
    // Freeze the bolt's fast forward motion so the seeded overlap is stable
    // across the coldet tick (its own strat would otherwise fly it ~264/frame).
    sh.game.objs.aliens[laser as usize].stratptr = None;
    sh.game.objs.aliens[laser as usize].sflags &= !ASF_COLLDISABLE;

    // Park a fresh enemy exactly on the bolt and freeze the bolt there so the
    // next coldet tick tests them overlapping (isolates the wiring: colltype
    // compatibility + collision-list membership + damage application).
    let (lx, ly, lz) = {
        let a = &sh.game.objs.aliens[laser as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    let enemy = sh.game.objs.alloc().expect("free slot for enemy");
    {
        let a = &mut sh.game.objs.aliens[enemy as usize];
        a.worldx = lx;
        a.worldy = ly;
        a.worldz = lz;
        a.hp = 30;
        a.ap = 1;
        a.collflags = COLLTYPE_ENEMY1; // 0x01 — differs from laser colltype 0x08
        a.shape = 100; // nonzero, distinct from the bolt shape
        a.sflags4 = 0;
        a.sflags = 0;
        a.immuneptr = 0;
        a.collcount = 1;
        a.collflags &= !ACF_FIRSTFRAME;
    }
    let enemy_hp_before = sh.game.objs.aliens[enemy as usize].hp;

    for _ in 0..3 {
        {
            let a = &mut sh.game.objs.aliens[laser as usize];
            a.worldx = lx;
            a.worldy = ly;
            a.worldz = lz;
            a.collflags &= !ACF_FIRSTFRAME;
        }
        {
            let a = &mut sh.game.objs.aliens[enemy as usize];
            a.worldx = lx;
            a.worldy = ly;
            a.worldz = lz;
        }
        sh.tick(0);
        if sh.game.objs.aliens[enemy as usize].hp < enemy_hp_before
            || !sh.game.objs.aliens[enemy as usize].active
        {
            return; // enemy took the hit — wiring works
        }
    }
    panic!(
        "player laser did not damage the seeded enemy (hp {} -> {}, active={}) (BUG A)",
        enemy_hp_before,
        sh.game.objs.aliens[enemy as usize].hp,
        sh.game.objs.aliens[enemy as usize].active
    );
}
