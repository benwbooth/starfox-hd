//! Boss-lane trace regressions, originally captured from the removed C port
//! and subsequently corrected against the retail ASM/ROM oracle.
//!
//! Fixtures `tests/fixtures/bo_{boss2,bossg,boss8}.txt` were dumped by the
//! scratchpad C harness (`bo_harness/bo_harness.c` + `bo_stubs.c`), which
//! compiles the REAL boss TUs + strat_enemy.c + strat_common.c +
//! strat_ground.c + obj.c + game_vars.c with stubbed sound/map/world/rtl.
//!
//! Each scenario: seed RNG 0x1234, spawn a scripted player in slot 0,
//! spawn the boss with its Istrat as the initial stratptr, run 150 ticks
//! of `Game::run_strategies` (C `Obj_RunStrategies`) while scripting the
//! player identically. Every tick emits one `T` global line plus one `A`
//! line per active alien in SLOT order; the Rust replay must match the blessed
//! ROM-verified trace byte-for-byte.
//!
//! The boss2 fixture from tick 22 onward includes the source `explode_Istrat`
//! sprite/polygon split. `s_make_obj` inserts the sprite after the exploding
//! object and the strategy loop reads that link after the current strategy, so
//! the sprite also runs its first `explode_strat` tick in that same pass.
//! Restoring that omitted lifecycle and timing also restores its object-slot
//! reuse and random draws, so all downstream slot references and
//! random-dependent poses were re-blessed together at that source boundary.
//! The boss2 and boss8 fixtures also preserve `l_add` scheduling from
//! `MACROS.INC`: a newly made child is inserted after the current mother and
//! therefore advances after the mother's completed pose in the same pass.
//! The retired C allocator incorrectly pushed every child at the list head.
//!
//! Regenerate (repo root; harness in the session scratchpad):
//!   gcc -O1 -Isrc -o bo_harness bo_harness.c bo_stubs.c \
//!       src/strat/strat_boss2.c src/strat/strat_boss_sea.c \
//!       src/strat/strat_boss8.c src/strat/strat_enemy.c \
//!       src/strat/strat_common.c src/strat/strat_ground.c \
//!       src/game/obj.c src/game/game_vars.c -lm
//!   for s in boss2 bossg boss8; do ./bo_harness $s \
//!       > rust/sf-strat/tests/fixtures/bo_$s.txt; done

use sf_game::alien::NUMBER_AL;
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses::{install_bosses, BossStratIds};
use std::fmt::Write as _;

// WRAM addresses (C globals mirrored into g_ram; see bosses.rs / enemy_a).
const WM_RNDVAL: u16 = 0x1F00;
const WM_BOSSFLAGS: u16 = 0x1F02;
const WM_STRATFLAGS: u16 = 0x1F05;
const WM_GSVAR_BYTE1: u16 = 0x0310;
const WM_MAPTRIGGER: u16 = 0x0311;

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

fn dump_tick(g: &Game, t: i32, out: &mut String) {
    let v = &g.vars;
    writeln!(
        out,
        "T{} G gf={} gfl={} bf={} sf={} bmax={} met={} gsv={} mtr={} psf={} psf2={} pst={}",
        t,
        v.gameframe,
        v.gameflags,
        v.read_ext8(WM_BOSSFLAGS),
        v.read_ext8(WM_STRATFLAGS),
        v.bossmaxhp,
        v.meters,
        v.read_ext8(WM_GSVAR_BYTE1),
        v.read_ext8(WM_MAPTRIGGER),
        v.pshipflags,
        v.pshipflags2,
        v.pstratflags,
    )
    .unwrap();
    for s in 0..NUMBER_AL {
        let al = &g.objs.aliens[s];
        if !al.active {
            continue;
        }
        writeln!(
            out,
            "T{} A{:02} sh={} f={} t={} c={} c1={} x={} y={} z={} rx={} ry={} rz={} \
             v={} sf={} s2={} s3={} s4={} b1={} b2={} b3={} b4={} b5={} b6={} w1={} \
             w2={} hp={} ap={} cf={} vx={} vy={} vz={} hf={} ss={} colf={} af={} \
             snd2={} im={} fo={} ptr={} sm={}",
            t,
            s,
            al.shape,
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
            al.sbyte5,
            al.sbyte6,
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
            al.snd2,
            al.immuneptr,
            al.fireobjptr,
            al.ptr,
            al.stratmem,
        )
        .unwrap();
    }
}

fn setup(scenario: &str) -> (Game, BossStratIds, u16) {
    let mut g = Game::new();
    // Seed the shared PRNG cell exactly like the C harness (g_rndval=0x1234).
    g.vars.write_ext16(WM_RNDVAL, 0x1234);
    g.vars.internal_playpt = 0;

    let ids = install_bosses(&mut g);

    // Player slot 0 — scripted, no strategy.
    let p = spawn(&mut g, 0, 0, 0, 2);
    {
        let al = &mut g.objs.aliens[p as usize];
        al.hp = 3;
        al.vz = 65;
        al.sflags4 |= 0x01; // ASF4_PLAYEROBJ
    }

    let boss = match scenario {
        "boss2" => spawn(&mut g, 0x2000, -600, 1000, 255),
        "bossg" => spawn(&mut g, 0, -0x60, 3000, 121),
        "boss8" => spawn(&mut g, 0, 0, 210 << 3, 250),
        _ => unreachable!(),
    };
    let handle = match scenario {
        "boss2" => ids.boss2,
        "bossg" => ids.bossg,
        "boss8" => ids.boss8,
        _ => unreachable!(),
    };
    g.objs.aliens[boss as usize].stratptr = Some(handle);
    (g, ids, boss)
}

fn run(scenario: &str) -> String {
    let (mut g, _ids, _boss) = setup(scenario);
    let mut out = String::new();
    for t in 1..=150i32 {
        g.objs.aliens[0].worldz = (65 * t) as i16;
        g.objs.aliens[0].vz = 65;

        match scenario {
            "boss2" => {
                for (tick, slot) in [(20, 7), (30, 8), (40, 9), (50, 10)] {
                    if t == tick && g.objs.aliens[slot].active {
                        g.objs.aliens[slot].hp = 0;
                    }
                }
            }
            "bossg" => {
                if t == 80 {
                    let mt = g.vars.read_ext8(WM_MAPTRIGGER);
                    g.vars.write_ext8(WM_MAPTRIGGER, mt & !1u8);
                }
            }
            _ => {}
        }

        g.run_strategies();
        dump_tick(&g, t, &mut out);
    }
    out
}

fn check(scenario: &str, fixture: &str) {
    let got = run(scenario);
    // Bless mode: the original C dump harness was deleted in the RIIR, so these
    // fixtures can no longer be regenerated from C. Set SF_BLESS_FIXTURES=1 to
    // rewrite them from the current source-verified Rust trace. The ROM ground
    // truth for spawn-time init is proven separately by sf-oracle
    // tests/audit_boss.rs; boss inits were diffed against
    // GBSTRATS/D2STRATS/GB3STRAT, and generic explosion allocation/lifetimes
    // against EXPSTRAT.ASM. This test is a regression guard, not an independent
    // C-parity proof.
    if std::env::var_os("SF_BLESS_FIXTURES").is_some() {
        std::fs::write(fixture, &got).unwrap_or_else(|e| panic!("write {fixture}: {e}"));
        return;
    }
    let want = std::fs::read_to_string(fixture).unwrap_or_else(|e| panic!("read {fixture}: {e}"));
    if got != want {
        // Report the first diverging line for a fast bisect.
        for (i, (a, b)) in got.lines().zip(want.lines()).enumerate() {
            if a != b {
                panic!("{scenario} parity diverged at line {i}:\n  rust: {a}\n     c: {b}");
            }
        }
        panic!(
            "{scenario} parity: length mismatch (rust {} lines, c {} lines)",
            got.lines().count(),
            want.lines().count()
        );
    }
}

#[test]
fn boss2_spawn_through_first_leap_matches_rom_verified_trace() {
    check(
        "boss2",
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bo_boss2.txt"),
    );
}

#[test]
fn bossg_mode_table_walk_matches_rom_verified_trace() {
    check(
        "bossg",
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bo_bossg.txt"),
    );
}

#[test]
fn boss8_open_volley_cycle_matches_rom_verified_trace() {
    check(
        "boss8",
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bo_boss8.txt"),
    );
}
