//! Enemy-A regression traces, originally captured from the retired C port.
//!
//! Fixtures under `tests/fixtures/ea_*.txt` were dumped by a scratchpad C
//! harness (`ea_harness.c`) that compiled the REAL `src/strat/strat_enemy.c`
//! + `strat_common.c` + `strat_ground.c` + `src/game/obj.c` + `game_vars.c`
//! with stubbed sound/map/world/rtl symbols.
//!
//! Each scenario: seed RNG 0x1234, spawn a scripted fake player in slot 0,
//! spawn the strategy under test with its Istrat as the initial stratptr,
//! then run 120 ticks of `Obj_RunStrategies` (Rust: `Game::run_strategies`)
//! while scripting the player identically. Every tick emits one `T` line of
//! globals and one `O` line per active alien in active-list order; the Rust
//! replay must match the retained snapshot byte-for-byte. These snapshots have
//! received source corrections and are not independent cartridge evidence.
//!
//! The worm and boss1 fixtures retain the source generic-explosion
//! sprite/polygon lifetimes. The explosion child is inserted after its host and
//! advances once in the same strategy pass. Their post-destruction records were
//! corrected as one boundary because the restored objects, slot reuse, and two
//! explosion random draws intentionally affect later active-list and motion
//! records. The zaco1 fixture also preserves the active bit while advancing the
//! source homing-laser animation.
//! The boss1 post-destruction records also retain the retail circle anchor
//! created by `makebosscircexp_srou`; the retired C translation omitted that
//! presentation object.
//! The boss1 fixture preserves the retail `l_add` schedule as well: its cover
//! and turrets are inserted after their current mother, not pushed ahead of it
//! as they were by the retired C allocation shim.
//! The three corrected fixtures also include the retail runtime random draw at
//! the start of every completed strategy frame. The retired C translation did
//! not run that scheduler-level draw, so its later generated positions and
//! facing values were shifted relative to the cartridge.
//! The gate, radar, and boss traces encode `colldisable` in the second source
//! flag byte. `STRATEQU.INC` assigns it bit 8; the retired C translation placed
//! it in the first byte, where it incorrectly aliased `partobj`.
//!
//! The 2026-10 flag-byte, sprite, lifetime, homing-speed-state and radar-entry
//! corrections have a reversible complete-file audit and source assertions in
//! tools/sf1/test_strategy_source_contracts.py. Native-output blessing is
//! disabled; future fixture changes require independent source evidence.
//! The Houdai projectile rows additionally restore insert-after-firer ordering
//! and their first-visit movement. `test_weapon_entry_source.py` reconstructs
//! the entire previous fixture by reversing only that source-proven change;
//! `sf-oracle/tests/weapon_strategy_entry.rs` executes the original initializer.
//! The boss-circle records additionally retain source insert-after-parent
//! ordering, initializer fallthrough and the real particle payload/lifetime.
//! `test_boss_circle_source.py` reverses only that correction and its forced
//! slot-reuse permutation. The boss fixture's death suffix is now historical:
//! its missing initializer visit and premature corpse removal contradict the
//! original code. Its first 70, pre-death frames remain a regression gate.
//! Death and effect lifetime coverage is replaced by the stronger
//! `sf-oracle/tests/support/boss_explosion_lifecycle.rs` gate: complete original
//! TRANS.dostrats passes versus the shipping scheduler, including every active
//! actor, allocation order and random byte, through the final emitter's death.

mod support;

use sf_game::alien::{ACF_FIRSTFRAME, ASF2_COLLDISABLE, ASF_COLLIDE};
use sf_game::game::{Game, StrategyFn};
use sf_game::obj::strat_init_obj_vars;
use sf_strat::enemy_a::{self, wm};
use std::fmt::Write as _;
use support::trace_visual_identity;

const ASF4_PLAYEROBJ: u8 = 0x01;
const ASF_SPECIAL: u8 = 0x01;

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

fn assign_istrat(g: &mut Game, idx: u16, f: StrategyFn) {
    let sid = g.world.register_strategy(f);
    g.objs.aliens[idx as usize].stratptr = Some(sid);
}

fn clear_attack_power(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].ap = 0;
}

fn script_player(g: &mut Game, t: i32) {
    let al = &mut g.objs.aliens[0];
    al.worldz = (65 * (t + 1)) as i16;
    al.worldx = (((t * 7) % 160) - 80) as i16;
    al.worldy = (-40 + ((t * 3) % 60)) as i16;
    al.vz = 65;
    al.rotx = (t * 2) as u8;
    al.roty = (256 - (t % 16)) as u8;
    al.rotz = t as u8;
}

fn dump_tick(g: &Game, t: i32, out: &mut String) {
    let v = &g.vars;
    writeln!(
        out,
        "T {} gf={} gfl={} bmh={} met={} bf={} gas={} sot={} sd={} ps={} \
         swc={} lives={} rnd={} sk={}",
        t,
        v.gameframe,
        v.gameflags,
        v.bossmaxhp,
        v.meters,
        v.read_ext8(wm::BOSSFLAGS),
        v.read_ext8(wm::GASFLAGS),
        g.world.specialobjtotal,
        v.read_ext8(wm::SPECIALS_DEAD),
        v.read_ext16(wm::PLAYERSCORE),
        v.read_ext16(wm::SPECWEPCNT),
        v.read_ext8(wm::LIVES),
        v.read_ext16(wm::RNDVAL),
        v.read_ext8(0x0304),
    )
    .unwrap();
    for idx in g.objs.active_indices() {
        let al = &g.objs.aliens[idx as usize];
        writeln!(
            out,
            "O {} sh={} fl={} ty={} cn={} cn1={} x={} y={} z={} rx={} ry={} \
             rz={} vel={} sf={} sf2={} sf3={} sf4={} sb1={} sb2={} sb3={} \
             sb4={} sw1={} sw2={} hp={} ap={} cf={} vx={} vy={} vz={} hfl={} \
             ss={} colf={} af={} snd1={} snd2={} imm={} fop={} ptr={} wx={} \
             wy={} wz={}",
            idx,
            trace_visual_identity(al),
            al.flags,
            al.type_,
            al.count,
            al.count1,
            al.worldx,
            al.worldy,
            al.worldz,
            al.rotx,
            al.roty,
            al.rotz,
            al.vel,
            al.sflags,
            al.sflags2,
            al.sflags3,
            al.sflags4,
            al.sbyte1,
            al.sbyte2,
            al.sbyte3,
            al.sbyte4,
            al.sword1,
            al.sword2,
            al.hp,
            al.ap,
            al.collflags,
            al.vx,
            al.vy,
            al.vz,
            al.hitflags,
            al.stratstate,
            al.colframe,
            al.animframe,
            al.snd1,
            al.snd2,
            al.immuneptr,
            al.fireobjptr,
            al.ptr,
            al.swpx1,
            al.swpy1,
            al.swpz1,
        )
        .unwrap();
    }
}

fn base_game() -> Game {
    let mut g = Game::new();
    g.vars.write_ext16(wm::RNDVAL, 0x1234);
    g.vars.pviewvelz = 65;
    g.vars.minpmove_y = -60;
    g.vars.playerflymode = 8; // PFM_SHADOWS
    g.vars.write_ext8(wm::CURRENTLEVEL, 1);
    g.vars.internal_playpt = 0;
    // Player in slot 0.
    let p = g.objs.alloc().unwrap();
    assert_eq!(p, 0);
    strat_init_obj_vars(&mut g.objs.aliens[0]);
    let al = &mut g.objs.aliens[0];
    al.shape = 2;
    al.hp = 40;
    al.sflags4 |= ASF4_PLAYEROBJ;
    al.collflags = ACF_FIRSTFRAME;
    g
}

fn run_scenario(g: Game, events: impl Fn(&mut Game, i32), fixture: &str) {
    run_scenario_frames(g, events, fixture, 120);
}

fn run_scenario_frames(mut g: Game, events: impl Fn(&mut Game, i32), fixture: &str, frames: i32) {
    let mut out = String::new();
    for t in 0..frames {
        script_player(&mut g, t);
        events(&mut g, t);
        g.run_strategies();
        dump_tick(&g, t, &mut out);
    }
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), fixture);
    let expected = std::fs::read_to_string(&path).expect("fixture");
    let boundary = format!("T {frames} ");
    let expected = expected
        .split_once(&boundary)
        .map_or(expected.as_str(), |(prefix, _)| prefix);
    for (i, (got, want)) in out.lines().zip(expected.lines()).enumerate() {
        assert_eq!(got, want, "{} line {} mismatch", fixture, i + 1);
    }
    assert_eq!(
        out.lines().count(),
        expected.lines().count(),
        "{} line count",
        fixture
    );
}

#[test]
fn parity_zaco1() {
    let mut g = base_game();
    let e1 = spawn(&mut g, -200, 150, 2500, 10);
    assign_istrat(&mut g, e1, enemy_a::strat_zaco1l_init);
    run_scenario(g, |_, _| {}, "ea_zaco1.txt");
}

#[test]
fn parity_houdai() {
    let mut g = base_game();
    let e1 = spawn(&mut g, 300, 0, 2800, 54);
    assign_istrat(&mut g, e1, enemy_a::strat_houdai_init);
    let e2 = spawn(&mut g, 100, 0, 2000, 11);
    g.objs.aliens[e2 as usize].collflags |= 0x02; // COLLTYPE_ENEMY2
    g.objs.aliens[e2 as usize].hp = 0xFF;
    run_scenario(g, |_, _| {}, "ea_houdai.txt");
}

#[test]
fn parity_worm() {
    let mut g = base_game();
    let e1 = spawn(&mut g, 0, 50, 2000, 12);
    assign_istrat(&mut g, e1, enemy_a::strat_wormhead_init);
    let e2 = spawn(&mut g, 0, 50, 2100, 13);
    g.objs.aliens[e2 as usize].sword1 = (e1 + 1) as i16;
    assign_istrat(&mut g, e2, enemy_a::strat_worm_init);
    let e3 = spawn(&mut g, 0, 50, 2200, 13);
    g.objs.aliens[e3 as usize].sword1 = (e2 + 1) as i16;
    assign_istrat(&mut g, e3, enemy_a::strat_worm_init);
    run_scenario(
        g,
        move |g, t| {
            if t == 40 && g.objs.aliens[e2 as usize].active {
                g.objs.aliens[e2 as usize].hp = 0;
            }
        },
        "ea_worm.txt",
    );
}

#[test]
fn parity_gate2() {
    let mut g = base_game();
    g.vars.write_ext16(wm::MAXPMOVEX, 200u16);
    g.vars.write_ext16(wm::MINPMOVEX, (-200i16) as u16);
    g.vars.write_ext16(wm::MAXPMOVEY, 100u16);
    g.vars.write_ext16(wm::VIEWCY, (-30i16) as u16);
    let e2 = spawn(&mut g, 0, 0, 0, 0); // player collision box (heal target)
    g.objs.aliens[e2 as usize].hp = 10;
    g.objs.aliens[e2 as usize].sflags2 |= ASF2_COLLDISABLE;
    g.vars.write_ext16(wm::PCBOXOBJ_B, e2);
    let e1 = spawn(&mut g, 0, -20, 1500, 14);
    assign_istrat(&mut g, e1, enemy_a::strat_gate2_init);
    run_scenario(g, |_, _| {}, "ea_gate2.txt");
}

#[test]
fn parity_rader0() {
    let mut g = base_game();
    // The radar runs before the player in active-list order. Restore the fake
    // player's harness value after it has supplied this tick's attack power.
    assign_istrat(&mut g, 0, clear_attack_power);
    let e1 = spawn(&mut g, 100, 0, 1200, 15);
    assign_istrat(&mut g, e1, enemy_a::strat_rader0_init);
    g.objs.aliens[e1 as usize].sflags |= ASF_SPECIAL;
    g.world.specialobjtotal = 1;
    run_scenario(
        g,
        move |g, t| {
            if t % 7 == 3 && g.objs.aliens[e1 as usize].active {
                // Model a fresh one-point collision with the live player. The
                // source hit handler reads attack power through collobjptr;
                // setting only the collide flag was a retired-port shortcut.
                g.objs.aliens[e1 as usize].sflags |= ASF_COLLIDE;
                g.objs.aliens[e1 as usize].collobjptr = 0;
                g.objs.aliens[e1 as usize].collcount = 1;
                g.objs.aliens[0].ap = 1;
            }
        },
        "ea_rader0.txt",
    );
}

#[test]
fn boss1_predeath_legacy_regression() {
    let mut g = base_game();
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);
    let e1 = spawn(&mut g, 0, 150, 3000, 16);
    assign_istrat(&mut g, e1, enemy_a::strat_boss1_init);
    run_scenario_frames(g, |_, _| {}, "ea_boss1.txt", 70);
}
