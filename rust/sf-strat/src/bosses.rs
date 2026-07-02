//! Stage bosses ported in their own C translation units (RIIR wave 3,
//! enemy_b lane).
//!
//! C oracle:
//! - `src/strat/strat_boss2.c`     — Boss2 spinning-top (9 children,
//!   6-state machine, plasma orbiters)
//! - `src/strat/strat_boss_sea.c`  — Seamon / BossG / seamon fish /
//!   flyingfish (multi-wave gating via gsvar_byte1 + maptrigger bits)
//! - `src/strat/strat_boss8.c`     — Boss8 washmap cast (shell / cover /
//!   beams / launchers / pillars / shrapnel, GF_BOSSDEAD + GF_STAGEDONE
//!   release)
//!
//! Shared helpers come from `crate::enemy_b::eb_compat` (documented
//! DUPLICATE copies of concurrent-lane code; consolidation after landing).

#![allow(dead_code)]

use crate::enemy_b::eb_compat::*;

// ============================================================
// Registration constants (table lane contract) — preserved verbatim.
// ============================================================

/// C `IS_SEAMON` (strat_boss_sea.c:53, ISTRATS.ASM def_Istrat 81).
pub const IS_SEAMON: usize = 81;
/// C `IS_BOSSG` (strat_boss_sea.c:54, def_Istrat 144).
pub const IS_BOSSG: usize = 144;
/// C `IS_BOSS2` (strat_boss2.c:57, def_Istrat 108).
pub const IS_BOSS2: usize = 108;
/// C `IS_NUCLEUSBEAML` (strat_boss8.c:64, def_Istrat 82).
pub const IS_NUCLEUSBEAML: usize = 82;
/// C `IS_BOSS8SHRAP` (strat_boss8.c:65, def_Istrat 83).
pub const IS_BOSS8SHRAP: usize = 83;
/// C `IS_BOSS8` (strat_boss8.c:66, def_Istrat 84).
pub const IS_BOSS8: usize = 84;
/// C `IS_NUCLEUSLAUNCHER` (strat_boss8.c:67, def_Istrat 86).
pub const IS_NUCLEUSLAUNCHER: usize = 86;
/// C `IS_NUCLEUSPILLAR` (strat_boss8.c:68, def_Istrat 87).
pub const IS_NUCLEUSPILLAR: usize = 87;

/// C `STRAT_ADDR_BOSSSEAMON` (strat_boss_sea.c:45).
pub const STRAT_ADDR_BOSSSEAMON: u32 = 0x030005;
/// C `STRAT_ADDR_BOSSG` (strat_boss_sea.c:46).
pub const STRAT_ADDR_BOSSG: u32 = 0x030006;
/// C `B8_STRAT_ADDR_BOSS8` (strat_boss8.c:71).
pub const B8_STRAT_ADDR_BOSS8: u32 = 0x060014;
/// C `B8_STRAT_ADDR_NUCLEUSLAUNCHER` (strat_boss8.c:72).
pub const B8_STRAT_ADDR_NUCLEUSLAUNCHER: u32 = 0x060015;
/// C `B8_STRAT_ADDR_NUCLEUSPILLAR` (strat_boss8.c:73).
pub const B8_STRAT_ADDR_NUCLEUSPILLAR: u32 = 0x060016;

// ============================================================
// BOSS2_BEGIN (strat_boss2.c)
// BOSS2_END
// ============================================================
// BOSSSEA_BEGIN (strat_boss_sea.c)
// BOSSSEA_END
// ============================================================
// BOSS8_BEGIN (strat_boss8.c)
// BOSS8_END
// ============================================================

/// Table-lane registration entry: C `StratBoss2_Register` +
/// `StratBossSea_Register` + `StratBoss8_Register` (called from
/// `Strat_RegisterAll`, strat_table.c:212-214). Completed as the boss
/// ports land.
pub fn register(_world: &mut sf_game::world::World) {
    // Filled in by the part assembly.
}
