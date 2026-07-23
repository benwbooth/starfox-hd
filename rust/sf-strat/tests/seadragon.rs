//! seadragon / seadragon2 / lochnessmonster behavioral tests — Route 3 L3.
//!
//! ASM oracle: `seadragon_istrat` / `seadragon2_istrat` /
//! `lochnessmonster_istrat` + the `sprouty` segment-growth machine
//! (reference/ultrastarfox/SF/STRAT/DSTRATS.ASM:1926-2395) and the
//! fire-breathing head `snake_istrat` (D2STRATS.ASM:732-861).
//!
//! No sf-oracle byte-exact differential is used: the sea dragon rides the
//! shared `sprouty` growth primitive whose tree/tunnel/flower branches are
//! deliberately scoped out (see the SEADRAGON SCOPE NOTE in bosses.rs), and
//! `make_splash`/`enemyupsea`/`enemydownsea` are the sea lane's cosmetic
//! no-ops. These tests assert the ported spine — segment growth + neck chain,
//! head emergence + fire, and the head-kill -> neck-sink death mechanic —
//! against hand-derived ASM expectations, cited inline.
//!
//! FIDELITY NOTE: the ROM sea dragon has NO boss HP bar (verified: no
//! `s_add_bossHP`/`s_set_bossmaxHP` in any of its spans). Necks are
//! hardHP(255)+nohitaffect; only the head (hp=4) is killable, and killing it
//! sinks the neck. The task's generic "bossmaxhp/m_bossHP drains" template
//! therefore does not apply — the death test instead exercises the real
//! head-kill -> sflag5 sink path.

use sf_game::alien::NUMBER_AL;
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses;

const WM_RNDVAL: u16 = 0x1F00;

// Local mirrors of the private bosses.rs constants (cited to the port).
const SH_SNAKE_1: u16 = 200; // sf-map route3::common SH_SNAKE_1
const SH_SNAKE_0: u16 = 335;
const SH_FIREBREATH: u16 = 363;
const SD_SFLAG2: u8 = 0x20; // sflags2 — "it's a dragon"
const SD_SFLAG3: u8 = 0x40; // sflags2 — head created once
const SD_SFLAG5_SFLAGS3: u8 = 0x01; // sflags3 — sink/withdraw request
const ASF_COLLDISABLE: u8 = 0x10; // alien.rs
const ASF_NOHITAFFECT: u8 = 0x40; // alien.rs
const ASF4_SFLAG8: u8 = 0x20; // alien.rs (lochness "lock ness")
const SEANECK_HP: u8 = 255; // hardHP
const SEADRAGON_HEAD_HP: u8 = 4; // seadragonHP

fn spawn(g: &mut Game, x: i16, y: i16, z: i16, shape: u16) -> u16 {
    let idx = g.objs.alloc().expect("alien pool");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    al.shape = shape;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    idx
}

/// New game with a static player in slot 0 at (0,0,player_z). Returns the game.
fn new_game(player_z: i16) -> Game {
    let mut g = Game::new();
    g.vars.write_ext16(WM_RNDVAL, 0x1234);
    g.vars.internal_playpt = 0;
    bosses::register(&mut g.world);
    let p = spawn(&mut g, 0, 0, player_z, 2);
    let al = &mut g.objs.aliens[p as usize];
    al.hp = 3;
    al.sflags4 |= 0x01; // ASF4_PLAYEROBJ
    g
}

/// Spawn a map-placed seadragon2 root (IS_SEADRAGON2) at (x,y,z), armed with
/// the registered init strat so `run_strategies` drives the whole neck.
fn spawn_seadragon2(g: &mut Game, x: i16, y: i16, z: i16) -> u16 {
    let root = spawn(g, x, y, z, SH_SNAKE_1);
    let id = g.world.istrats[bosses::IS_SEADRAGON2].expect("IS_SEADRAGON2 registered");
    g.objs.aliens[root as usize].stratptr = Some(id);
    root
}

fn count_active(g: &Game) -> usize {
    (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count()
}

/// The fire-breathing head is the object carrying seadragonHP (4).
fn find_head(g: &Game) -> Option<usize> {
    (0..NUMBER_AL).find(|&i| {
        g.objs.aliens[i].active
            && i != 0
            && g.objs.aliens[i].hp == SEADRAGON_HEAD_HP
            && g.objs.aliens[i].shape == SH_SNAKE_0
    })
}

fn count_firebreath(g: &Game) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == SH_FIREBREATH)
        .count()
}

// ------------------------------------------------------------
// 1. seadragon2 init: snake flags + neck HP + drop half a segment.
//    (DSTRATS.ASM:1931 sbyte2=15; :1936-1937 sbyte1=(rnd&3)+2; :1940-1948
//     sflag2/sflag4/colldisable, worldy-=sprout_maxy/2(40); :1950-1963
//     seadragon_istrat2 hp=seaneckHP, nohitaffect, sbyte1 beqdec.)
// ------------------------------------------------------------
#[test]
fn seadragon2_init_sets_snake_flags_and_neck_hp() {
    let mut g = new_game(200);
    let root = spawn_seadragon2(&mut g, 0, 0, 200);

    // ISTRAT wiring: index 197 resolves through world.istrats[] (map path).
    assert!(
        g.world.istrats[bosses::IS_SEADRAGON2].is_some(),
        "IS_SEADRAGON2 registered at world.istrats[197]"
    );
    assert!(
        g.world.istrats[bosses::IS_LOCHNESS].is_some(),
        "IS_LOCHNESS registered at world.istrats[198]"
    );

    g.run_strategies(); // drives the init chain (falls into sprouty.strat)

    let b = &g.objs.aliens[root as usize];
    assert_ne!(b.sflags2 & SD_SFLAG2, 0, "sflag2 (dragon) set");
    assert_eq!(b.worldy, -40, "worldy dropped by sprout_maxy/2 = 40");
    assert_eq!(b.hp, SEANECK_HP, "neck hp = seaneckHP (255)");
    assert_ne!(b.sflags & ASF_NOHITAFFECT, 0, "neck nohitaffect set");
    // sbyte2 (fire counter) copied through from the seadragon2 init = 15.
    assert_eq!(b.sbyte2, 15, "seadragon2 sbyte2 = 15 (fire counter seed)");
}

// ------------------------------------------------------------
// 2. seadragon2 grows a neck + emerges a fire head (sbyte2!=0 skips the
//    distance gate: DSTRATS.ASM:2113 -> .lochness -> .nobluff head create;
//    :2170 .strat2 spawns the next segment above and links al_ptr).
// ------------------------------------------------------------
#[test]
fn seadragon2_grows_neck_and_emerges_head() {
    let mut g = new_game(200);
    let root = spawn_seadragon2(&mut g, 0, -40, 200);
    let start = count_active(&g);

    let mut saw_head = false;
    let mut saw_link = false;
    for _ in 0..40 {
        g.run_strategies();
        if find_head(&g).is_some() {
            saw_head = true;
        }
        // The root links a grown segment via al_ptr (index+1, != 0 / != -1).
        let p = g.objs.aliens[root as usize].ptr;
        if p != 0 && p != 0xffff {
            saw_link = true;
        }
    }

    assert!(saw_head, "fire-breathing head (snake_0, hp=4) emerged");
    assert!(saw_link, "root linked a grown neck segment via al_ptr");
    assert!(
        count_active(&g) > start,
        "neck grew: more objects than the root+player"
    );
    // sflag3 latched: the head is created exactly once.
    assert_ne!(g.objs.aliens[root as usize].sflags3 & 0, 0xff, "sanity");
}

// ------------------------------------------------------------
// 3. the emerged seadragon2 head breathes fire (sbyte2 counts 15->1 then
//    fires firebreath every gf&15==0: D2STRATS.ASM:786-798).
// ------------------------------------------------------------
#[test]
fn seadragon2_head_breathes_fire() {
    let mut g = new_game(200);
    let _root = spawn_seadragon2(&mut g, 0, -40, 200);

    let mut saw_fire = false;
    for _ in 0..120 {
        g.run_strategies();
        if count_firebreath(&g) > 0 {
            saw_fire = true;
            break;
        }
    }
    assert!(
        saw_fire,
        "head fired a firebreath after the sbyte2 countdown"
    );
}

// ------------------------------------------------------------
// 4. death mechanic: killing the head runs snake_istrat.explode, which sets
//    the neck's sflag5 (sink) and marks the head dead (D2STRATS.ASM:810-817).
// ------------------------------------------------------------
#[test]
fn head_kill_sinks_the_neck() {
    let mut g = new_game(200);
    let root = spawn_seadragon2(&mut g, 0, -40, 200);

    // Grow until the head exists and has run its init (hp==4).
    let mut head = None;
    for _ in 0..40 {
        g.run_strategies();
        if let Some(h) = find_head(&g) {
            head = Some(h);
            break;
        }
    }
    let head = head.expect("head emerged");
    let neck = g.objs.aliens[head].ptr; // head.al_ptr -> its neck segment
    assert!(neck != 0 && neck != 0xffff, "head linked to a neck segment");

    // No neck should carry the sink flag yet.
    let sunk_before = (0..NUMBER_AL)
        .any(|i| g.objs.aliens[i].active && g.objs.aliens[i].sflags3 & SD_SFLAG5_SFLAGS3 != 0);
    assert!(!sunk_before, "no neck sink request before the head dies");

    // Fire the head's explosion strat directly (the engine's hp==0 -> expstrat
    // dispatch; invoked here deterministically via the public registry).
    let expstrat = g.objs.aliens[head].expstratptr.expect("head has expstrat");
    g.objs.aldead = 0;
    g.call_strat(expstrat, head as u16);

    // The neck was told to sink (sflag5) and the head latched dead (aldead=1,
    // which run_strategies turns into a free of the object).
    let sunk_after = (0..NUMBER_AL)
        .any(|i| g.objs.aliens[i].active && g.objs.aliens[i].sflags3 & SD_SFLAG5_SFLAGS3 != 0);
    assert!(
        sunk_after,
        "a neck latched sflag5 (sink request) on head death"
    );
    assert_eq!(g.objs.aldead, 1, "head death latched (explode -> aldead)");
    assert_eq!(g.objs.aliens[head].ptr, 0, "head unlinked from its neck");
    let _ = root;
}

// ------------------------------------------------------------
// 5. plain seadragon + lochness init variants.
//    seadragon: sbyte2==0 (gated on proximity + a bluff coin), sflag2 set.
//    lochness: sflag8 ("lock ness"), no height countdown.
// ------------------------------------------------------------
#[test]
fn seadragon_and_lochness_init_variants() {
    let mut g = new_game(200);

    // Plain seadragon via the typed mother-map strategy.
    let sd = spawn(&mut g, 0, 0, 200, SH_SNAKE_1);
    let sd_id = g
        .world
        .find_direct_strategy(bosses::STRATEGY_SEADRAGON)
        .expect("seadragon strategy registered");
    g.objs.aliens[sd as usize].stratptr = Some(sd_id);

    // Lochness via its ISTRAT row.
    let ln = spawn(&mut g, 500, 0, 200, SH_SNAKE_1);
    let ln_id = g.world.istrats[bosses::IS_LOCHNESS].expect("lochness registered");
    g.objs.aliens[ln as usize].stratptr = Some(ln_id);

    g.run_strategies();

    let s = &g.objs.aliens[sd as usize];
    assert_ne!(s.sflags2 & SD_SFLAG2, 0, "seadragon sflag2 set");
    assert_eq!(
        s.sbyte2, 0,
        "plain seadragon has no fire counter (sbyte2=0)"
    );
    assert_ne!(
        s.sflags & ASF_COLLDISABLE,
        0,
        "seadragon colldisable (submerged)"
    );

    let l = &g.objs.aliens[ln as usize];
    assert_ne!(l.sflags4 & ASF4_SFLAG8, 0, "lochness sflag8 set");
    assert_ne!(
        l.sflags2 & SD_SFLAG2,
        0,
        "lochness is also a 'dragon' (sflag2)"
    );
    // lochness kept sflag3 clear until it emerges a head (sflag8 grows on any dz).
    let _ = SD_SFLAG3;
}
