//! Fast, renderer-free whole-route soak test.
//!
//! The pilot is deliberately invulnerable and every currently vulnerable
//! enemy is dealt a fatal hit. This is not a combat-skill oracle; it verifies
//! that every map can run, spawn its strategies, complete its boss/death
//! protocol, tally, resolve the route graph, and reach the ending without a
//! human at the controls.

use std::collections::BTreeSet;

use sf_core::pad;
use sf_game::alien::{
    StratId, ACF_COLLTYPE1, ACF_COLLTYPE2, ACF_COLLTYPE3, ACF_COLLTYPE4, ACF_COLLTYPE5,
    ACF_COLLTYPE6, ASF3_CHILDOBJ, ASF4_PLAYEROBJ, ASF_COLLDISABLE, ASF_COLLIDE, ASF_INVISIBLE,
    ASF_NOHITAFFECT,
};
use sf_game::shell::{GameState, Shell, INTRO_INPUT_DELAY_TICKS, TITLE_INPUT_DELAY_TICKS};
use sf_game::vars::{
    BossEncounter, GF_PLAYERDEAD, GF_PLAYERDYING, HARD_HP, PSF2_PLAYERHP0, PSF3_NOCOLLISIONS,
    PSTF_NOTDIE,
};
use sf_strat::common::{sv, StratRam};

const ROUTE_TICK_BUDGET: usize = 150_000;
const CREDITS_TICK_BUDGET: usize = 20_000;

const SH_CHICKEN_BODY: u16 = 77;
const SH_CHICKEN_HEAD: u16 = 381;
const SH_CHICKEN_TAIL: u16 = 382;
const SH_FLINGBOSS: u16 = 11;
const SH_FLINGBOSS_GRABBER: u16 = 384;
const SH_BOSS8: u16 = 46;
const SH_BOSS8_BEAM: u16 = 43;
const SH_TENKI_MARKER: u16 = 71;
const SH_ANDROSS_FACE: u16 = 431;
const SH_END_BASE_ESCAPE: u16 = 224;
const SH_END_FORMATION: u16 = 225;

const EASY_ROUTE_REQUIRED_SHAPES: [(u16, &str); 8] = [
    (237, "Asteroid Belt large meteor"),
    (298, "Asteroid Belt space pylon"),
    (19, "Asteroid Belt Barricader boss"),
    (189, "Macbeth gro_6 terrain"),
    (239, "Macbeth base tank"),
    (134, "Macbeth tank_2"),
    (230, "Venom zaco_1 fighter"),
    (76, "Andross robot"),
];

const MEDIUM_ROUTE_REQUIRED_SHAPES: [(u16, &str); 6] = [
    (196, "colony aircar"),
    (216, "highway truck"),
    (319, "colony pipe_6"),
    (29, "large tunnel exit"),
    (320, "route-2 final pillar"),
    (223, "Andross monolith"),
];

fn known_player_strat(shell: &Shell, id: Option<StratId>) -> &'static str {
    let Some(id) = id else { return "none" };
    let Some(actual) = shell.game.world.strat_registry.get(id.0 as usize) else {
        return "invalid";
    };
    let actual = *actual as usize;
    for (name, candidate) in [
        (
            "space",
            sf_strat::player::player_in_space_strat as sf_game::game::StrategyFn,
        ),
        ("tunnel", sf_strat::player::player_in_tunnel_strat),
        ("texit", sf_strat::player::player_in_texit_strat),
        ("colony", sf_strat::player::player_in_colony_strat),
        ("washent", sf_strat::player::pshipwashent_strat),
    ] {
        if actual == candidate as usize {
            return name;
        }
    }
    "other"
}

fn configured_shell(route: u32) -> Shell {
    let mut shell = Shell::new();
    // The real app injects renderer-derived sh_max extents into the game.
    // Build the same table without creating a GPU so the renderer-free soak
    // exercises the ROM's shape-margin culling behavior, not a zero-margin
    // approximation.
    let mut shapes = sf_render::shapes_gl::ShapeStore::new();
    for entry in sf_render::shape_data::SHAPE_DATA {
        assert!(shapes.register(entry.shape_id, entry.vertices, entry.faces));
    }
    shell.set_shape_extents(shapes.all_shape_half_extents());
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    shell.set_spawn_player(Box::new(|game, newmap| {
        let _ = sf_strat::player::strat_spawn_player_for_map(game, newmap);
    }));
    shell.set_ending_score_part(Box::new(sf_strat::endscore::spawn_final_score_part));
    shell.set_ending_boss_replay(Box::new(sf_strat::endseq::spawn_replay_boss));

    shell.tick(0);
    shell.tick(0);
    while shell.game.vars.gameframe < INTRO_INPUT_DELAY_TICKS {
        shell.tick(pad::A);
    }
    while shell.state() != GameState::Title {
        shell.tick(0);
    }
    shell.tick(0);
    shell.game.vars.gameframe = TITLE_INPUT_DELAY_TICKS;
    shell.tick(pad::START);
    while shell.state() == GameState::Title {
        shell.tick(0);
    }
    shell.tick(0);
    for _ in 0..route {
        shell.tick(pad::DOWN);
        shell.tick(0);
    }
    shell.tick(pad::START);
    assert_eq!(shell.state(), GameState::Playing);
    shell
}

fn arm_test_pilot(shell: &mut Shell) {
    let was_dying = shell.game.vars.gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD) != 0
        || shell.game.vars.pshipflags2 & PSF2_PLAYERHP0 != 0;
    shell.game.vars.gameflags &= !(GF_PLAYERDYING | GF_PLAYERDEAD);
    shell.game.vars.pshipflags2 &= !PSF2_PLAYERHP0;
    // This is an engine/route soak, not an input-skill test.  HP restoration
    // alone is insufficient because the ROM's pcbox collision path can begin
    // the scripted death sequence before the next tick restores it.  Use the
    // canonical player no-collision mode so walls cannot restart the map at a
    // checkpoint and disguise later route blockers as an infinite map loop.
    shell.game.vars.pshipflags3 |= PSF3_NOCOLLISIONS;
    shell.game.vars.pstratflags |= PSTF_NOTDIE;
    shell.game.vars.write_ext8(0x0520, 3); // canonical lives store

    // Once playerdead_Istrat has installed the crash strategy, clearing the
    // death flags alone cannot make the ship live again: the crash strategy
    // reasserts them every frame.  Restore the registered player dispatch too
    // so an incidental soak-pilot death cannot silently reload the same map.
    if was_dying {
        let ids = sf_strat::player::install(&mut shell.game);
        let pidx = shell.game.vars.internal_playpt;
        if pidx >= 0 && (pidx as usize) < shell.game.objs.aliens.len() {
            let player = &mut shell.game.objs.aliens[pidx as usize];
            player.stratptr = Some(ids.player);
            player.collstratptr = Some(ids.player_coll);
            player.expstratptr = Some(ids.player_dead);
            player.hp = HARD_HP;
            player.sbyte1 = 0;
        }
    }

    for al in &mut shell.game.objs.aliens {
        if al.active && al.sflags4 & ASF4_PLAYEROBJ != 0 {
            al.hp = HARD_HP;
        }
    }
}

fn defeat_vulnerable_hostiles(shell: &mut Shell) {
    let enemy_types = ACF_COLLTYPE1
        | ACF_COLLTYPE2
        | ACF_COLLTYPE3
        | ACF_COLLTYPE4
        | ACF_COLLTYPE5
        | ACF_COLLTYPE6;

    // Great Commander's feet only accept damage through the opened hatch.
    // Directly zeroing the 80 HP while its animation is closed bypasses
    // bossffeet.hit and jumps the mother to its helicopter phase before the
    // torso/arms exist.  Respect the ROM's `anim >= 5` damage window.
    if let Some(feet) = shell.game.objs.aliens.iter_mut().find(|al| {
        al.active
            && al.shape == sf_strat::bossf_heli::SH_AIRSHIP_FEET
            && al.sflags3 & ASF3_CHILDOBJ != 0
            && al.hp != 0
            && al.hp != HARD_HP
    }) {
        if feet.animframe & 0x7f >= 5 {
            feet.hp = 0;
        }
        return;
    }

    // The chicken's heads/tail use HP values offset by +64: ordinary hits
    // shorten their chains instead of killing the object.  This soak applies
    // a synthetic fatal hit, so targeting a head first bypasses that bespoke
    // collision path and can leave the mother permanently invulnerable.
    // Prefer the body during its real red/vulnerable window.
    if let Some(body) = shell.game.objs.aliens.iter_mut().find(|al| {
        al.active
            && al.shape == SH_CHICKEN_BODY
            && al.collflags & enemy_types != 0
            && al.sflags & (ASF_NOHITAFFECT | ASF_COLLDISABLE | ASF_INVISIBLE) == 0
            && al.hp != 0
            && al.hp != HARD_HP
    }) {
        if std::env::var_os("SF_ROUTE_TRACE").is_some() {
            eprintln!(
                "[chicken-body-shot] hp={} sf={:#x} state={}",
                body.hp, body.sflags, body.stratstate
            );
        }
        body.hp = 0;
        return;
    }

    let chicken_active = shell
        .game
        .objs
        .aliens
        .iter()
        .any(|al| al.active && al.shape == SH_CHICKEN_BODY);
    if chicken_active {
        // Head/tail HP is deliberately biased by +64.  Reduce it one hit at
        // a time so arm_strat observes the <65 threshold and performs the
        // ROM's neck-shortening handoff; assigning zero would incorrectly
        // route through the generic explosion callback.  The soak pilot has
        // three independent guns here so all three termini receive a hit in
        // the same frame, allowing the real one-frame body window to occur
        // even while the mother regrows its chains.
        for tip in shell.game.objs.aliens.iter_mut().filter(|al| {
            al.active
                && (al.shape == SH_CHICKEN_HEAD || al.shape == SH_CHICKEN_TAIL)
                && al.hp > 64
                && al.hp != HARD_HP
        }) {
            tip.hp = 64;
        }
        return;
    }

    // Flingboss phase 1 has a hard/nohitaffect body and hard tentacles.
    // Laser contact on either tentacle runs grabberhit/passiton, which raises
    // the mother's sflag5 and drains its separate 24-point phase reserve.
    // Exercise that collision protocol; direct HP writes cannot reach phase 2.
    let fling_phase1 = shell.game.objs.aliens.iter().any(|al| {
        al.active
            && al.shape == SH_FLINGBOSS
            && al.hp == HARD_HP
            && al.sflags & ASF_NOHITAFFECT != 0
    });
    if fling_phase1 {
        let grabbers: Vec<u16> = shell
            .game
            .objs
            .active_indices()
            .into_iter()
            .filter(|&idx| shell.game.objs.aliens[idx as usize].shape == SH_FLINGBOSS_GRABBER)
            .collect();
        for grabber in grabbers {
            sf_strat::bosses::chicken_arm_passiton(&mut shell.game, grabber);
        }
        return;
    }

    // Boss 8's three `nucleusbeamL` children are hard-HP switches: laser
    // contact runs nucleusbeamcol_Istrat and latches sflag1 (or destroys the
    // switch on level 1), after which boss8wait_strat can open the shell.
    // They share ENEMY2/enemyweap with their mother, so ROM collision correctly
    // prevents boss-family self-contact. Drive one real collide callback per
    // frame, just as the soak pilot's laser would, instead of assigning HP.
    let boss8_active = shell
        .game
        .objs
        .aliens
        .iter()
        .any(|al| al.active && al.shape == SH_BOSS8);
    if boss8_active {
        if let Some(beam) = shell.game.objs.active_indices().into_iter().find(|&idx| {
            let al = &shell.game.objs.aliens[idx as usize];
            al.shape == SH_BOSS8_BEAM
                && al.hp == HARD_HP
                && al.collstratptr.is_some()
                && al.sflags2 & 0x10 == 0 // boss8 sflag1
                && al.sflags & ASF_COLLDISABLE == 0
        }) {
            let al = &mut shell.game.objs.aliens[beam as usize];
            al.sflags |= ASF_COLLIDE;
            al.collobjptr = shell.game.vars.internal_playpt as u16;
            return;
        }
    }

    // Pick in the same active-list order used by the ROM collision pass.
    // Slot-number order is observably wrong for overlapping multipart bosses:
    // Castanet allocates its rear cymbal first and its front cymbal second, so
    // the active-list head makes the front half take the decisive hit.  An
    // ascending slot scan killed the rear half first and created a state that
    // normal weapon collision cannot reach.
    let target = shell.game.objs.active_indices().into_iter().find(|&idx| {
        let al = &shell.game.objs.aliens[idx as usize];
        al.sflags4 & ASF4_PLAYEROBJ == 0
            // Titania's shape-72 `tenki_on` object is a weather/progression
            // marker, not a combat target. It must survive long enough to
            // disable its collisions and run the proximity export.
            && !(al.shape == SH_TENKI_MARKER && al.hp == 10 && al.ap == 10)
            && al.collflags & enemy_types != 0
            && al.sflags & (ASF_NOHITAFFECT | ASF_COLLDISABLE | ASF_INVISIBLE) == 0
            && al.hp != 0
            && al.hp != HARD_HP
    });
    if let Some(idx) = target {
        let al = &mut shell.game.objs.aliens[idx as usize];
        if std::env::var_os("SF_ROUTE_TRACE").is_some()
            && ((al.shape == 0 && al.ap == 40) || al.shape == SH_BOSS8)
        {
            eprintln!(
                "[route-shot] shape={} hp={} state={} sf={:#x} z={} strat={:?} exp={:?}",
                al.shape, al.hp, al.stratstate, al.sflags, al.worldz, al.stratptr, al.expstratptr
            );
        }
        al.hp = 0;
        return;
    }

    // Andross's outer face has hard HP; shots damage the independent left
    // and right eye counters through monolithcol_Istrat instead.  Exercise
    // the same one-hit cadence here when no ordinary target (most importantly
    // the briefly exposed core) is vulnerable.
    if let Some(face) = shell.game.objs.aliens.iter_mut().find(|al| {
        al.active
            && al.shape == SH_ANDROSS_FACE
            && al.hp == HARD_HP
            && al.collflags & enemy_types != 0
            // monolithcol_Istrat deliberately accepts eye hits while the
            // parent retains nohitaffect; only its fire states expose them.
            && al.sflags & (ASF_COLLDISABLE | ASF_INVISIBLE) == 0
            && (al.stratstate == 5 || al.stratstate == 21)
            && (al.sbyte1 != 0 || al.sbyte2 != 0)
    }) {
        if face.sbyte1 != 0 {
            face.sbyte1 -= 1;
        } else {
            face.sbyte2 -= 1;
        }
    }
}

fn fly_pad(shell: &Shell, tick: usize) -> u16 {
    // Titania's `tenki_on` weather marker only raises the fog-exit latch when
    // the player passes within 100 units in X/Y during its short Z window.
    // Track the live marker so this unattended route test exercises the real
    // proximity branch instead of depending on the generic sweep's phase.
    let pidx = shell.game.vars.internal_playpt as usize;
    if pidx < shell.game.objs.aliens.len() && shell.game.objs.aliens[pidx].active {
        let px = shell.game.objs.aliens[pidx].worldx;
        if let Some(marker) = shell
            .game
            .objs
            .aliens
            .iter()
            .find(|al| al.active && al.shape == SH_TENKI_MARKER && al.hp == 10 && al.ap == 10)
        {
            let dx = marker.worldx.wrapping_sub(px);
            let py = shell.game.objs.aliens[pidx].worldy;
            let dy = marker.worldy.wrapping_sub(py);
            let horizontal = if dx > 40 {
                pad::RIGHT
            } else if dx < -40 {
                pad::LEFT
            } else {
                0
            };
            // World Y grows toward the bottom of the screen: DOWN raises Y,
            // UP lowers it.
            let vertical = if dy > 40 {
                pad::DOWN
            } else if dy < -40 {
                pad::UP
            } else {
                0
            };
            return horizontal | vertical;
        }
    }

    // The Space Armada guide ship accepts the player only inside its narrow
    // X corridor (`Xdistmore #30<<2` in GASTRATS.ASM).  Fly toward the live
    // guide instead of relying on the generic sweep to happen to line up.
    if pidx < shell.game.objs.aliens.len() && shell.game.objs.aliens[pidx].active {
        let px = shell.game.objs.aliens[pidx].worldx;
        if let Some(guide) = shell
            .game
            .objs
            .aliens
            .iter()
            .find(|al| al.active && al.hp == HARD_HP && al.ap == 16 && al.snd2 == 5)
        {
            let dx = guide.worldx.wrapping_sub(px);
            if dx > 60 {
                return pad::RIGHT;
            }
            if dx < -60 {
                return pad::LEFT;
            }
            return 0;
        }
    }

    let horizontal = if (tick / 240) & 1 == 0 {
        pad::LEFT
    } else {
        pad::RIGHT
    };
    let vertical = if (tick / 160) & 1 == 0 {
        pad::UP
    } else {
        pad::DOWN
    };
    pad::Y | horizontal | vertical
}

fn nearby_labels(map: u32, ptr: u16) -> Vec<String> {
    let Some(level) = sf_map::catalog::get_map_data(map) else {
        return Vec::new();
    };
    let mut labels: Vec<_> = level
        .labels
        .iter()
        .map(|(name, offset)| (offset.abs_diff(ptr), name, offset))
        .collect();
    labels.sort_by_key(|entry| entry.0);
    labels
        .into_iter()
        .take(4)
        .map(|(_, name, offset)| format!("{name}@{offset}"))
        .collect()
}

#[test]
fn all_three_routes_reach_the_ending_unattended() {
    let route_filter = std::env::var("SF_ROUTE_ONLY")
        .ok()
        .and_then(|value| value.parse::<u32>().ok());
    for route in 0..3u32 {
        if route_filter.is_some_and(|selected| selected != route) {
            continue;
        }
        let mut shell = configured_shell(route);
        let mut maps = BTreeSet::new();
        let mut shapes_seen = BTreeSet::new();
        let mut saw_dm_end_base_escape = false;
        let mut saw_dm_end_formation = false;
        let mut last_guide: Option<(u16, Option<StratId>)> = None;
        let mut last_monolith: Option<(u16, u8, u8, u8, u16)> = None;
        let mut last_trace_ptr: Option<(u16, u32, u16)> = None;
        let mut reported_death = false;

        for tick in 0..ROUTE_TICK_BUDGET {
            let frame = shell.frame();
            maps.insert(frame.newmap);
            shapes_seen.extend(
                shell
                    .game
                    .objs
                    .active_indices()
                    .into_iter()
                    .map(|idx| shell.game.objs.aliens[idx as usize].shape),
            );
            if shell.state() == GameState::Playing && shell.game.vars.currentbg == 19 {
                saw_dm_end_base_escape |= shell
                    .game
                    .objs
                    .aliens
                    .iter()
                    .any(|al| al.active && al.shape == SH_END_BASE_ESCAPE);
            }
            if shell.state() == GameState::Playing && shell.game.vars.currentbg == 20 {
                saw_dm_end_formation |= shell
                    .game
                    .objs
                    .aliens
                    .iter()
                    .any(|al| al.active && al.shape == SH_END_FORMATION);
            }

            if std::env::var_os("SF_ROUTE_TRACE").is_some() {
                if shell.game.vars.gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD) != 0 {
                    let p = &shell.game.objs.aliens[shell.game.vars.internal_playpt as usize];
                    eprintln!(
                        "[pilot-death] route={route} tick={tick} ptr={} gf={:#x} hp={} state={} strat={:?}",
                        shell.game.vars.mapptr,
                        shell.game.vars.gameflags,
                        p.hp,
                        p.stratstate,
                        p.stratptr,
                    );
                    if !reported_death {
                        reported_death = true;
                        let boxes = [
                            ("body", shell.game.coldet.pcbox.body),
                            ("left", shell.game.coldet.pcbox.lwing),
                            ("right", shell.game.coldet.pcbox.rwing),
                        ]
                        .map(|(name, slot)| {
                            let state = slot.map(|slot| {
                                let al = &shell.game.objs.aliens[slot as usize];
                                format!(
                                    "slot={slot} active={} hp={} sf={:#x} sf4={:#x} strat={:?} coll={:?}",
                                    al.active,
                                    al.hp,
                                    al.sflags,
                                    al.sflags4,
                                    al.stratptr,
                                    al.collstratptr,
                                )
                            });
                            format!("{name}={state:?}")
                        });
                        eprintln!(
                            "[pilot-death-detail] flags2={:#x} flags3={:#x} pstrat={:#x} pcplayer={:?} boxes={boxes:?} labels={:?}",
                            shell.game.vars.pshipflags2,
                            shell.game.vars.pshipflags3,
                            shell.game.vars.pstratflags,
                            shell.game.coldet.pcbox.player,
                            nearby_labels(frame.newmap, shell.game.vars.mapptr),
                        );
                    }
                }
                if let Some((last_stage, last_map, last_ptr)) = last_trace_ptr {
                    if last_stage == frame.stage
                        && last_map == frame.newmap
                        && shell.game.vars.mapptr < last_ptr
                    {
                        eprintln!(
                            "[map-back] route={route} tick={tick} stage={} map={} {last_ptr}->{} jsr_top={} loops={}",
                            frame.stage,
                            frame.newmap,
                            shell.game.vars.mapptr,
                            shell.game.world.jsr_top,
                            shell.game.world.num_loops,
                        );
                    }
                }
                last_trace_ptr = Some((frame.stage, frame.newmap, shell.game.vars.mapptr));
                if tick % 10_000 == 0 {
                    let pl = &shell.game.objs.aliens[shell.game.vars.internal_playpt as usize];
                    eprintln!(
                        "[route] route={route} tick={tick} stage={} map={} ptr={} cnt={} jsr_top={} loops={} free={:?} pxyz=({},{},{}) vel={}/({},{},{}) rot=({},{},{}) pstrat={:?}/{} psf={:#x} pstf={:#x} pviewz={} playerposz={} lastdz={}",
                        frame.stage,
                        frame.newmap,
                        shell.game.vars.mapptr,
                        shell.game.vars.mapcnt,
                        shell.game.world.jsr_top,
                        shell.game.world.num_loops,
                        shell.game.objs.free_head,
                        pl.worldx,
                        pl.worldy,
                        pl.worldz,
                        pl.vel,
                        pl.vx,
                        pl.vy,
                        pl.vz,
                        pl.rotx,
                        pl.roty,
                        pl.rotz,
                        pl.stratptr,
                        known_player_strat(&shell, pl.stratptr),
                        shell.game.vars.pshipflags,
                        shell.game.vars.pstratflags,
                        shell.game.vars.sv_i16(sv::PVIEWPOSZ),
                        shell.game.vars.player_posz,
                        shell.game.world.lastzchange,
                    );
                }
                let monolith = shell
                    .game
                    .objs
                    .active_indices()
                    .into_iter()
                    .find(|&idx| shell.game.objs.aliens[idx as usize].shape == SH_ANDROSS_FACE)
                    .map(|idx| {
                        let al = shell.game.objs.aliens[idx as usize];
                        (idx, al.stratstate, al.sbyte1, al.sbyte2, al.ptr)
                    });
                if monolith != last_monolith {
                    if let Some((idx, state, left, right, ptr)) = monolith {
                        let core = ptr.checked_sub(1).and_then(|ci| {
                            shell.game.objs.aliens.get(ci as usize).map(|al| {
                                (ci, al.active, al.hp, al.stratstate, al.worldz, al.stratptr)
                            })
                        });
                        eprintln!(
                            "[monolith] tick={tick} idx={idx} state={state} eyes={left}/{right} ptr={ptr} core={core:?} gf={:#x}",
                            shell.game.vars.gameflags,
                        );
                    } else if last_monolith.is_some() {
                        eprintln!(
                            "[monolith] tick={tick} removed gf={:#x}",
                            shell.game.vars.gameflags
                        );
                    }
                    last_monolith = monolith;
                }
                let guide = shell.game.objs.active_indices().into_iter().find(|&idx| {
                    let al = &shell.game.objs.aliens[idx as usize];
                    al.hp == HARD_HP && al.ap == 16 && al.snd2 == 5
                });
                let current = guide.map(|idx| (idx, shell.game.objs.aliens[idx as usize].stratptr));
                if current != last_guide || (guide.is_some() && tick % 250 == 0) {
                    if let Some(idx) = guide {
                        let al = &shell.game.objs.aliens[idx as usize];
                        let pl = &shell.game.objs.aliens[shell.game.vars.internal_playpt as usize];
                        eprintln!(
                            "[guide] tick={tick} idx={idx} strat={:?} dx={} dz={} xyz=({},{},{}) player=({},{},{}) gf={:#x} psf={:#x} stayblack={} wipe={} noctrlcnt={} pstrat={:?}",
                            al.stratptr,
                            al.worldx.wrapping_sub(pl.worldx),
                            al.worldz.wrapping_sub(pl.worldz),
                            al.worldx,
                            al.worldy,
                            al.worldz,
                            pl.worldx,
                            pl.worldy,
                            pl.worldz,
                            shell.game.vars.gameflags,
                            shell.game.vars.pshipflags,
                            shell.game.vars.sv_i8(sv::STAYBLACK),
                            shell.game.vars.sv_u8(sv::DOINGWIPE),
                            shell.game.vars.sv_u8(sv::PLAYER_NOCTRLCNT),
                            pl.stratptr,
                        );
                    } else if last_guide.is_some() {
                        eprintln!(
                            "[guide] tick={tick} removed gf={:#x}",
                            shell.game.vars.gameflags
                        );
                    }
                    last_guide = current;
                }
                if tick % 100 == 0 {
                    if let Some(idx) =
                        shell.game.objs.active_indices().into_iter().find(|&idx| {
                            shell.game.objs.aliens[idx as usize].shape == SH_CHICKEN_BODY
                        })
                    {
                        let al = &shell.game.objs.aliens[idx as usize];
                        eprintln!(
                            "[chicken] tick={tick} frame={} idx={idx} hp={} sf={:#x}/{:#x}/{:#x}/{:#x} type={:#x} state={} count={} strat={:?} exp={:?} reglen={} pactive={} gf={:#x}",
                            shell.game.vars.gameframe,
                            al.hp,
                            al.sflags,
                            al.sflags2,
                            al.sflags3,
                            al.sflags4,
                            al.type_,
                            al.stratstate,
                            al.count,
                            al.stratptr,
                            al.expstratptr,
                            shell.game.world.strat_registry.len(),
                            shell.game.objs.aliens[0].active,
                            shell.game.vars.gameflags,
                        );
                    }
                }
            }

            match shell.state() {
                GameState::Playing => {
                    arm_test_pilot(&mut shell);
                    defeat_vulnerable_hostiles(&mut shell);
                    let input = fly_pad(&shell, tick);
                    shell.tick(input);
                }
                GameState::Tally => shell.tick(0),
                GameState::PlanetSelect => {
                    shell.tick(pad::START);
                    shell.tick(0);
                }
                GameState::Continue => {
                    shell.tick(0);
                    shell.tick(pad::START);
                }
                GameState::Ending => break,
                state => panic!(
                    "route {route} unexpectedly entered {state:?} at tick {tick}; maps={maps:?}"
                ),
            }
        }

        let frame = shell.frame();
        let labels = nearby_labels(frame.newmap, shell.game.vars.mapptr);
        let map_bytes = sf_map::catalog::get_map_data(frame.newmap)
            .map(|level| {
                let p = shell.game.vars.mapptr as usize;
                if p >= level.data.len() {
                    Vec::new()
                } else {
                    level.data[p.saturating_sub(8)..(p + 24).min(level.data.len())].to_vec()
                }
            })
            .unwrap_or_default();
        let active: Vec<String> = shell
            .game
            .objs
            .active_indices()
            .into_iter()
            .map(|idx| {
                let al = &shell.game.objs.aliens[idx as usize];
                format!(
                    "{idx}:sh{} hp{} coll{:#x} sf{:#x}/sf3{:#x} xyz({},{},{}) state{} count{} sb({},{},{}) strat{:?} exp{:?}",
                    al.shape,
                    al.hp,
                    al.collflags,
                    al.sflags,
                    al.sflags3,
                    al.worldx,
                    al.worldy,
                    al.worldz,
                    al.stratstate,
                    al.count,
                    al.sbyte1,
                    al.sbyte2,
                    al.sbyte3,
                    al.stratptr,
                    al.expstratptr,
                )
            })
            .collect();
        let inactive_slots = shell
            .game
            .objs
            .aliens
            .iter()
            .filter(|al| !al.active)
            .count();
        let mut free_slots = BTreeSet::new();
        let mut free = shell.game.objs.free_head;
        while let Some(idx) = free {
            if !free_slots.insert(idx) {
                break;
            }
            free = shell.game.objs.aliens[idx as usize].next;
        }
        assert_eq!(
            shell.state(),
            GameState::Ending,
            "route {route} stalled after stage {}, map {}, gameframe {}; map_ptr={}, map_count={}, last_z_change={}, player_z={}, player_vz={}, view_vz={}, gameflags={:#x}, map_trigger={:#04x}, global_strategy_byte={}, boss_max_hp={}, inactive_slots={inactive_slots}, free_head={:?}, free_slots={}, labels={labels:?}, bytes={map_bytes:02x?}, visited={maps:?}, active={active:#?}",
            frame.stage,
            frame.newmap,
            frame.gameframe,
            shell.game.vars.mapptr,
            shell.game.vars.mapcnt,
            shell.game.world.lastzchange,
            shell.game.objs.aliens[shell.game.vars.internal_playpt as usize].worldz,
            shell.game.objs.aliens[shell.game.vars.internal_playpt as usize].vz,
            shell.game.vars.pviewvelz,
            shell.game.vars.gameflags,
            shell.game.vars.map.trigger,
            shell.game.vars.map.global_strategy_byte,
            shell.game.vars.bossmaxhp,
            shell.game.objs.free_head,
            free_slots.len(),
        );
        assert!(
            maps.len() >= 6,
            "route {route} visited too few maps: {maps:?}"
        );
        assert!(
            saw_dm_end_base_escape,
            "route {route} reached Ending without the DM_END base-escape ship"
        );
        assert!(
            saw_dm_end_formation,
            "route {route} reached Ending without the DM_END formation flight"
        );
        let recorded_bosses: Vec<BossEncounter> = shell.game.vars.boss_seq
            [..usize::from(shell.game.vars.boss_seq_len)]
            .iter()
            .copied()
            .flatten()
            .collect();
        let expected_bosses: &[BossEncounter] = match route {
            0 => &[
                BossEncounter::Route1Stage1,
                BossEncounter::Route1Stage2,
                BossEncounter::Route1Stage3,
                BossEncounter::Route1Stage4,
                BossEncounter::Route1Stage5,
                BossEncounter::Route1Stage6,
            ],
            1 => &[
                BossEncounter::Route3Stage1,
                BossEncounter::Route3Stage2,
                BossEncounter::Route3Stage3,
                BossEncounter::Route3Stage4,
                BossEncounter::Route3Stage5,
                BossEncounter::Route3Stage6,
                BossEncounter::Route3Stage7,
                BossEncounter::FinalBattle,
            ],
            2 => &[
                BossEncounter::Route2Stage1,
                BossEncounter::Route2Stage2,
                BossEncounter::Route2Stage3,
                BossEncounter::Route2Stage4,
                BossEncounter::Route2Stage5,
                BossEncounter::Route2Stage6,
                BossEncounter::FinalBattle,
            ],
            _ => unreachable!("three-route test only"),
        };
        assert_eq!(
            recorded_bosses, expected_bosses,
            "route {route} did not retain its exact boss replay order"
        );

        // Continue through the typed score parade, every recorded recap, the
        // exact credits map, and the permanent final score. Count presentation
        // ticks so a route cannot silently skip or shorten a source handler.
        let mut replay_order = Vec::new();
        let mut replay_tick_counts: Vec<(BossEncounter, u16)> = Vec::new();
        let mut active_replay: Option<(BossEncounter, u16)> = None;
        for _ in 0..CREDITS_TICK_BUDGET {
            let ending = shell.frame();
            if let Some(replay) = ending.ending_replay {
                match active_replay {
                    Some((encounter, ticks)) if encounter == replay.encounter => {
                        active_replay = Some((encounter, ticks.saturating_add(1)));
                    }
                    Some(previous) => {
                        replay_tick_counts.push(previous);
                        replay_order.push(replay.encounter);
                        active_replay = Some((replay.encounter, 1));
                    }
                    None => {
                        replay_order.push(replay.encounter);
                        active_replay = Some((replay.encounter, 1));
                    }
                }
            } else if let Some(previous) = active_replay.take() {
                replay_tick_counts.push(previous);
            }
            if ending.ending_final_score_complete {
                break;
            }
            shell.tick(0);
        }
        if let Some(previous) = active_replay.take() {
            replay_tick_counts.push(previous);
        }
        assert_eq!(
            replay_order, expected_bosses,
            "route {route} recap presentation order diverged from its recorded encounters"
        );
        for (encounter, ticks) in replay_tick_counts {
            assert_eq!(
                ticks,
                sf_game::shell::ending_replay_spec(encounter).duration_ticks,
                "{encounter:?} recap duration"
            );
        }
        assert!(
            shell.frame().ending_final_score_complete,
            "route {route} staff roll did not reach the permanent final score within {CREDITS_TICK_BUDGET} ticks; map_ptr={} map_count={} level_finished={} active={:?}",
            shell.game.vars.mapptr,
            shell.game.vars.mapcnt,
            shell.game.world.levelfinished,
            shell.game.objs.active_indices(),
        );

        if route == 0 {
            for (shape, name) in EASY_ROUTE_REQUIRED_SHAPES {
                assert!(
                    shapes_seen.contains(&shape),
                    "easy route reached Ending without observing {name} shape {shape}; seen={shapes_seen:?}"
                );
            }

            // The source ending is permanent: verify the final-score object
            // presentation and prove controller input cannot return to title.
            let score_messages: BTreeSet<u16> = shell
                .game
                .objs
                .active_indices()
                .into_iter()
                .filter_map(|idx| {
                    let object = &shell.game.objs.aliens[idx as usize];
                    (object.sflags4 & ASF4_PLAYEROBJ == 0 && object.coltab != 0)
                        .then_some(object.coltab)
                })
                .collect();
            assert!(score_messages.contains(&sf_strat::endscore::MSG_TOTAL_LABEL_TAG));
            assert!(score_messages.contains(&sf_strat::endscore::MSG_AVERAGE_LABEL_TAG));
            assert!(score_messages
                .iter()
                .any(|message| message & sf_strat::endscore::MSG_TAG_MASK
                    == sf_strat::endscore::MSG_PERCENT_TAG));
            shell.tick(pad::START);
            assert_eq!(shell.state(), GameState::Ending);
        }
        // Planet-select DOWN order is easy -> hard -> medium, so the third
        // configured selection (route==2 here) is the Route-2/M2_6 path.
        if route == 2 {
            for (shape, name) in MEDIUM_ROUTE_REQUIRED_SHAPES {
                assert!(
                    shapes_seen.contains(&shape),
                    "medium route reached Ending without observing {name} shape {shape}; seen={shapes_seen:?}"
                );
            }
        }
    }
}
