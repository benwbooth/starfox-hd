//! Per-tick level trace regression, corrected against the ROM oracle.
//!
//! Fixtures were dumped by a scratchpad C harness (`gc_main.c`) that
//! compiled the REAL `src/game/world.c` + `map_exec.c` + `obj.c` +
//! `coldet.c` + `game_vars.c` + `alien_compat.c` + `map/levels.c` with an
//! empty strategy table and stubbed sound/windows/strings/shapes/paths
//! (matching the sf-game `Hooks` defaults).
//!
//! Scenario: load MAP_ID_1_1, spawn a fake player in slot 0
//! (shape 2, ASF4_PLAYEROBJ, HP 40, internalPLAYPT=0), then 500 ticks of
//! `worldz += MEDPSPEED(65)` followed by the Nmi_GameTick subset.
//! - s0: baseline (inert intro loop).
//! - s1: from tick 20 on, GF_STRATDONE1|GF_BOSSDEAD forced pre-tick so the
//!   script runs deep into the level (RTS, CODE65816 inline callbacks,
//!   clearmap/exitbase natives, specials, loops, setbg...).
//!
//! Each tick emits one `T` line (map VM + ported globals) and one `O` line
//! per active alien in active-list order.  The format originated in gc_main.c,
//! but fixture fields that differ from the old compatibility C are maintained
//! from ROM-backed Rust behavior. Examples include COLDET's per-frame
//! `collcount=1` initialization from `init_strats_ram_l`, the player colour
//! cycle from `init_strats_l`, and Corneria's literal `meters_on trans`,
//! `wipein`, and `mapendwipe` command streams. Map-pointer values are offsets
//! into that current, byte-accurate serialized stream rather than source ROM
//! addresses.

use sf_game::alien::{ASF4_PLAYEROBJ, NUMBER_AL};
use sf_game::obj::strat_init_obj_vars;
use sf_game::vars::{GF_BOSSDEAD, GF_STRATDONE1};
use sf_game::Game;
use sf_map::catalog::{self, map_id};
use std::fmt::Write as _;

const MEDPSPEED: i16 = 65;
const NUM_TICKS: u32 = 500;

fn dump_tick(g: &Game, tick: u32, out: &mut String) {
    let v = &g.vars;
    let w = &g.world;
    writeln!(
        out,
        "T {} gf={} mp={} mc={} lzc={} fin={} spc={} lmo={} stg={} bg={} \
         bgf={} dots={} oth={} dma={} bts={} gm={} gfl={} psf={} psf2={} \
         psf3={} pstf={} pfm={} spfm={} pw1={} pw2={} pw3={} pw4={} mpy={} \
         vd={} met={} bmh={} ca={} ow={} r304={} r305={} r306={} r307={}",
        tick,
        v.gameframe,
        v.mapptr,
        v.mapcnt,
        w.lastzchange,
        w.levelfinished,
        w.specialobjtotal,
        w.lastmapobj,
        v.stagecnt,
        v.currentbg,
        v.bgflags,
        v.dotsflag,
        v.othmusic,
        v.bg_dmalist,
        v.bgtransspeed,
        v.game_mode,
        v.gameflags,
        v.pshipflags,
        v.pshipflags2,
        v.pshipflags3,
        v.pstratflags,
        v.playerflymode,
        v.player_view_mode as u8,
        v.psvar_word1,
        v.psvar_word2,
        v.psvar_word3,
        v.psvar_word4,
        v.minpmove_y,
        v.viewdist,
        v.meters,
        v.bossmaxhp,
        v.circleanim,
        v.oncewipe,
        v.map.skill_fly,
        v.map.stage_clear,
        v.map.clear_background_two,
        v.map.level_finished,
    )
    .unwrap();

    for idx in g.objs.active_indices() {
        let al = &g.objs.aliens[idx as usize];
        writeln!(
            out,
            "O {} sh={} x={} y={} z={} rx={} ry={} rz={} v={} p={} ty={} \
             fl={} sf={} s2={} s3={} s4={} b1={} b2={} b3={} b4={} w1={} \
             w2={} hp={} ap={} cf={} cc={} do={} af={} cfr={} st={} im={} \
             co={} c0={} c1={} ss={} pw={}",
            idx,
            al.shape,
            al.worldx,
            al.worldy,
            al.worldz,
            al.rotx,
            al.roty,
            al.rotz,
            al.vel,
            al.ptr,
            al.type_,
            al.flags,
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
            al.collcount,
            al.depthoffset,
            al.animframe,
            al.colframe,
            if al.stratptr.is_some() { 1 } else { 0 },
            al.immuneptr,
            al.collobjptr,
            al.count,
            al.count1,
            al.stratstate,
            al.pword1,
        )
        .unwrap();
    }
}

fn run_scenario(scenario: u32) -> String {
    let level = catalog::get_map_data(map_id::M1_1).expect("level1_1 built");
    let mut g = Game::new();
    g.load_level(level);

    // Fake player, mirroring gc_main.c exactly.
    let idx = g.objs.alloc().expect("player slot");
    assert_eq!(idx, 0, "player must land in slot 0");
    strat_init_obj_vars(&mut g.objs.aliens[0]);
    g.objs.aliens[0].shape = 2; // SHAPE_MYSHIP_4
    g.objs.aliens[0].sflags4 |= ASF4_PLAYEROBJ;
    g.objs.aliens[0].hp = 40;
    g.vars.internal_playpt = 0;

    let mut out = String::new();
    for tick in 1..=NUM_TICKS {
        g.objs.aliens[0].worldz = g.objs.aliens[0].worldz.wrapping_add(MEDPSPEED);
        if scenario == 1 && tick >= 20 {
            g.vars.gameflags |= GF_STRATDONE1 | GF_BOSSDEAD;
        }
        g.tick();
        dump_tick(&g, tick, &mut out);
    }
    out
}

fn assert_trace_matches(scenario: u32, fixture_name: &str) {
    let path = format!(
        "{}/tests/fixtures/{fixture_name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let produced = run_scenario(scenario);
    // Bless mode: the C dump harness was deleted in the RIIR, so these fixtures
    // can't be regenerated from C. SF_BLESS_FIXTURES=1 rewrites them from the
    // current (ROM-verified) Rust trace. The spawn-time init fields these pin
    // (type_/flags/sflags3/animframe/colframe) are proven ROM-correct by
    // sf-oracle tests/audit_boss.rs + audit_coldet.rs. Corneria's colour,
    // meters, wipe, engine-sound, and map-stream-offset updates are additionally
    // covered by the strict title/weapon oracles and sf-map's binary fixture.
    // This remains a regression guard, not an independent C-parity proof.
    if std::env::var_os("SF_BLESS_FIXTURES").is_some() {
        std::fs::write(&path, &produced).unwrap_or_else(|e| panic!("write {path}: {e}"));
        return;
    }
    let fixture = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));

    let expected: Vec<&str> = fixture.lines().filter(|l| !l.starts_with('#')).collect();
    let got: Vec<&str> = produced.lines().collect();

    let mut ticks_matched = 0u32;
    for (i, (e, g)) in expected.iter().zip(got.iter()).enumerate() {
        if e != g {
            panic!(
                "scenario {scenario}: trace diverges at line {} \
                 (after {ticks_matched} fully matched ticks):\n  fixture: {e}\n  current: {g}",
                i + 1
            );
        }
        if e.starts_with("T ") {
            ticks_matched += 1;
        }
    }
    assert_eq!(
        expected.len(),
        got.len(),
        "scenario {scenario}: line count (fixture {} vs current {})",
        expected.len(),
        got.len()
    );
    assert_eq!(ticks_matched, NUM_TICKS);
}

#[test]
fn level1_1_trace_matches_rom_corrected_baseline() {
    assert_trace_matches(0, "level1_1_trace_s0.txt");
}

#[test]
fn level1_1_trace_matches_rom_corrected_stratdone() {
    assert_trace_matches(1, "level1_1_trace_s1.txt");
}

#[test]
fn player_slot_zero_invariant() {
    // Obj_Init builds the free list 69..0 push-front, so the first alloc
    // must be slot 0 (the player convention obj.c relies on).
    let mut g = Game::new();
    assert_eq!(g.objs.alloc(), Some(0));
    assert_eq!(g.objs.alloc(), Some(1));
    let _ = NUMBER_AL;
}
