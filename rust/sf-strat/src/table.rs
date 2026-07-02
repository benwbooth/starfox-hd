//! Strategy-table registration — RIIR port of `src/strat/strat_table.c`
//! (`Strat_RegisterAll` + `Strat_RegisterAddressMap`).
//!
//! Populates `world.istrats[IS_XXX]` with each lane's registry handle
//! (ISTRATS.ASM def_Istrat indices) and builds the 24-bit strategy address
//! map (flat-id + synthetic `0x02:xxxx` forms for every non-null istrat,
//! plus the explicit non-istrat symbols and boss synthetic addresses).
//!
//! Lane contract:
//! - `player`/`ground`/`enemy_a` expose `install(g) -> *StratIds`, which
//!   register their fns into `world.strat_registry` and hand back named
//!   handles; this module places each handle at its C `IS_XXX` index.
//! - `enemy_b`/`bosses` expose `register(world)` which self-populate their
//!   istrat rows + synthetic addresses (boss7/A/F, spacepilon, title, and
//!   the boss2/sea/8 cast).
//!
//! GAP — path strategies (IS_PATH/IS_PATHT/IS_PATHDHA): the C
//! `Strat_RegisterAll` sets these to `Strat_Path_Init` / `Strat_PathText_Init`
//! / `Strat_PathDha_Init`. Those live in `sf-path` (`interp::strat_path_init`
//! etc.) but take `&mut sf_path::Alien`, not a `sf_game::StrategyFn`
//! (`fn(&mut Game, u16)`), and no `Game`->`PathWorld`/`PathHost` adapter
//! exists yet. Wiring them requires a path-integration shim across the
//! sf-game/sf-path Alien boundary (a separate lane's work), so these three
//! indices are left `None` here and called out as the next finish-line item.

use crate::{bosses, enemy_a, enemy_b, ground, player};
use sf_game::game::Game;
use sf_game::world::ISTRAT_CAPACITY;

// ============================================================
// ISTRATS.ASM def_Istrat indices (C src/strat/strat_table.c:31-94).
// Only the player/ground/enemy_a-owned rows are needed here; enemy_b and
// bosses carry their own index constants and self-register.
// ============================================================
const IS_PLAYER: usize = 0;
const IS_STAYREL: usize = 8;
const IS_STAYDIST: usize = 9;
const IS_NOCOLL: usize = 10;
const IS_GND: usize = 15;
const IS_CLSHIPGNDA: usize = 19;
const IS_CLSHIPGNDB: usize = 20;
const IS_CLSHIPGNDC: usize = 21;
const IS_CLSHIPWARPA: usize = 22;
const IS_CLSHIPWARPB: usize = 23;
const IS_CLSHIPWARPC: usize = 24;
const IS_CLSHIPEARTHA: usize = 28;
const IS_CLSHIPEARTHB: usize = 29;
const IS_CLSHIPEARTHC: usize = 30;
const IS_CLSHIPCHASEA: usize = 37;
const IS_CLSHIPCHASEB: usize = 38;
const IS_CLSHIPCHASEC: usize = 39;
const IS_WORMHEAD: usize = 52;
const IS_GATE: usize = 53;
const IS_WORM: usize = 61;
const IS_WORM2: usize = 62;
const IS_CAMELEON: usize = 63;
const IS_BOSS1: usize = 69;
const IS_PILLAR3: usize = 79;
const IS_BOMWING: usize = 89;
const IS_UP1MAN: usize = 90;
const IS_RADER0: usize = 92;
const IS_RADER1: usize = 93;
const IS_ZACOS: usize = 94;
const IS_ZACO1L: usize = 95;
const IS_ZACO1R: usize = 96;
const IS_HOUDAI: usize = 97;
const IS_HOUDAINS: usize = 98;
const IS_ZACO3: usize = 100;
const IS_TOWER0: usize = 101;
const IS_ZACO0: usize = 102;
const IS_ZACO4: usize = 103;
const IS_HARD180YR: usize = 105;
const IS_HARD180YRNZR: usize = 106;
const IS_PARA: usize = 107;
const IS_HARD90YR: usize = 127;
const IS_SZACO2: usize = 129;
const IS_STAYRELHARD180YR: usize = 137;
const IS_CARRIER: usize = 139;
const IS_FRIENDEXITBASE: usize = 152;
const IS_PATH: usize = 157;
const IS_SPACEBARWALKER: usize = 173;
const IS_SPACEBARSHOOT: usize = 174;
const IS_ITEM5: usize = 175;
const IS_ITEM7: usize = 177;
const IS_HARD180YRFOG: usize = 181; // alias: Strat_Hard180yr_Init
const IS_GATE2: usize = 208;
const IS_HARDROT: usize = 210;
const IS_NOCOLLANIM0: usize = 221; // alias: Strat_NoColl_Init
const IS_HARD: usize = 226;
const IS_TADPOLE: usize = 228;
const IS_PATHT: usize = 229;
const IS_BASE_1: usize = 230;
const IS_SHIPINTRO: usize = 239;
const IS_SKILLFLY: usize = 241;
const IS_PATHDHA: usize = 243;

// ============================================================
// Non-istrat symbol addresses (C strat_table.h:10-16). SPACEPILON/TIT/BOSSF
// are registered by enemy_b::register; the boss2/sea/8 synthetic addresses
// by bosses::register. Only these two are owned here.
// ============================================================
const STRAT_ADDR_TOW0EXPLODE: u32 = 0x030001;
const STRAT_ADDR_GATE3: u32 = 0x030002;

// C address-map encoding bases (strat_table.c:26-27).
const STRAT_ADDR_FLAT_ID_BASE: u32 = 0x000000;
const STRAT_ADDR_SYNTH_BASE: u32 = 0x020000;

/// C `Strat_RegisterAll` + `Strat_RegisterAddressMap` (strat_table.c).
///
/// Note vs C: C first `memcpy`s `g_istrat_shape_defaults` into
/// `g_istrat_shapes` and clears `g_istrats[]` to NULL. In Rust `World::init`
/// already zero-seeds `istrats` and installs the builtin space-bar rows
/// (166-169), so this function does NOT clear the table (that would wipe the
/// builtins). The `g_istrat_shape_defaults` table is not yet ported to Rust,
/// so `world.istrat_shapes` stays zero — a data-layer gap owned by sf-game,
/// not this wiring.
pub fn register_all(g: &mut Game) {
    // ---- Lanes that hand back handles (player / ground / enemy_a) ----
    let p = player::install(g);
    let gr = ground::install(g);
    let ea = enemy_a::install(g);

    // ---- IS_XXX index placement (C Strat_RegisterAll body) ----
    let is = &mut g.world.istrats;

    is[IS_PLAYER] = Some(p.player);

    // Ground / static objects
    is[IS_STAYREL] = Some(gr.stayrel);
    is[IS_STAYDIST] = Some(gr.staydist);
    is[IS_STAYRELHARD180YR] = Some(gr.stayrelhard180yr);
    is[IS_GND] = Some(gr.gnd);

    // Decorative (no collision)
    is[IS_NOCOLL] = Some(ea.nocoll);
    is[IS_NOCOLLANIM0] = Some(ea.nocoll); // alias -> Strat_NoColl_Init
    is[IS_GATE] = Some(ea.gate);
    is[IS_GATE2] = Some(ea.gate2);
    is[IS_WORMHEAD] = Some(ea.wormhead);
    is[IS_WORM] = Some(ea.worm);
    is[IS_WORM2] = Some(ea.worm2);

    // Indestructible buildings
    is[IS_HARD] = Some(ea.hard);
    is[IS_HARD180YR] = Some(ea.hard180yr);
    is[IS_HARD180YRNZR] = Some(ea.hard180yr_nzr);
    is[IS_HARD90YR] = Some(ea.hard90yr);
    is[IS_HARD180YRFOG] = Some(ea.hard180yr); // alias -> Strat_Hard180yr_Init
    is[IS_HARDROT] = Some(ea.hardrot);

    // Enemy fighters / structures
    is[IS_BOMWING] = Some(ea.bomwing);
    is[IS_UP1MAN] = Some(ea.up1man);
    is[IS_ZACOS] = Some(ea.zacos);
    is[IS_RADER0] = Some(ea.rader0);
    is[IS_RADER1] = Some(ea.rader1);
    is[IS_BOSS1] = Some(ea.boss1);
    is[IS_CAMELEON] = Some(ea.cameleon);
    is[IS_PILLAR3] = Some(ea.pillar3);
    is[IS_ZACO1L] = Some(ea.zaco1l);
    is[IS_ZACO1R] = Some(ea.zaco1r);
    is[IS_HOUDAI] = Some(ea.houdai);
    is[IS_HOUDAINS] = Some(ea.houdai_ns);
    is[IS_ZACO3] = Some(ea.zaco3);
    is[IS_ZACO0] = Some(ea.zaco0);
    is[IS_SZACO2] = Some(ea.szaco2);
    is[IS_TOWER0] = Some(ea.tower0);
    is[IS_TADPOLE] = Some(ea.tadpole);
    is[IS_ZACO4] = Some(ea.zaco4);
    is[IS_PARA] = Some(ea.para);
    is[IS_CARRIER] = Some(ea.carrier);
    is[IS_BASE_1] = Some(ea.base1);
    is[IS_SKILLFLY] = Some(ea.skillfly);

    // Wingman exit / clear-demo ships
    is[IS_FRIENDEXITBASE] = Some(ea.friendexitbase);
    is[IS_CLSHIPGNDA] = Some(ea.clship_gnda);
    is[IS_CLSHIPGNDB] = Some(ea.clship_gndb);
    is[IS_CLSHIPGNDC] = Some(ea.clship_gndc);
    is[IS_CLSHIPWARPA] = Some(ea.clship_warpa);
    is[IS_CLSHIPWARPB] = Some(ea.clship_warpb);
    is[IS_CLSHIPWARPC] = Some(ea.clship_warpc);
    is[IS_CLSHIPEARTHA] = Some(ea.clship_eartha);
    is[IS_CLSHIPEARTHB] = Some(ea.clship_earthb);
    is[IS_CLSHIPEARTHC] = Some(ea.clship_earthc);
    is[IS_CLSHIPCHASEA] = Some(ea.clship_chasea);
    is[IS_CLSHIPCHASEB] = Some(ea.clship_chaseb);
    is[IS_CLSHIPCHASEC] = Some(ea.clship_chasec);
    is[IS_SHIPINTRO] = Some(p.ship_intro_init);
    is[IS_SPACEBARWALKER] = Some(ea.spacebarwalker);
    is[IS_SPACEBARSHOOT] = Some(ea.spacebarshoot);
    is[IS_ITEM5] = Some(ea.item5);
    is[IS_ITEM7] = Some(ea.item7);

    // Path-following enemies/wingmen — GAP (see module header):
    // C sets IS_PATH/IS_PATHT/IS_PATHDHA to sf-path's path istrats. No
    // Game<->PathWorld adapter exists, so they stay unwired here.
    let _ = (IS_PATH, IS_PATHT, IS_PATHDHA);

    // ---- Self-registering lanes (enemy_b bosses + stage bosses) ----
    // (C: bossA/boss7/bossF rows + StratBoss2/Sea/8_Register.) These place
    // their own istrat rows and synthetic addresses (SPACEPILON 0x030004,
    // TIT 0x050020, BOSSF 0x060010, BOSSSEAMON/BOSSG 0x030005/06,
    // BOSS8/launcher/pillar 0x060014/15/16).
    enemy_b::register(&mut g.world);
    bosses::register(&mut g.world);

    // ---- Address map (C Strat_RegisterAddressMap, strat_table.c:104) ----
    // Flat-id + synthetic `0x02:xxxx` forms for every non-null istrat...
    for i in 0..ISTRAT_CAPACITY {
        if let Some(id) = g.world.istrats[i] {
            g.world
                .register_strategy_address(STRAT_ADDR_FLAT_ID_BASE | i as u32, id);
            g.world
                .register_strategy_address(STRAT_ADDR_SYNTH_BASE | i as u32, id);
        }
    }
    // ...then the explicit non-istrat symbols owned here (enemy_a handles).
    g.world
        .register_strategy_address(STRAT_ADDR_TOW0EXPLODE, ea.tow0_explode);
    g.world.register_strategy_address(STRAT_ADDR_GATE3, ea.gate3);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_game::game::Game;

    /// The capstone contract: after `register_all`, every C
    /// `Strat_RegisterAll` istrat row and address-map entry resolves.
    #[test]
    fn register_all_wires_indices_and_addresses() {
        let mut g = Game::new();
        register_all(&mut g);

        // Representative istrat rows across all five lanes.
        assert!(g.world.istrats[IS_PLAYER].is_some(), "player");
        assert!(g.world.istrats[IS_GND].is_some(), "ground gnd");
        assert!(g.world.istrats[IS_STAYRELHARD180YR].is_some(), "ground stayrelhard180yr");
        assert!(g.world.istrats[IS_HARD].is_some(), "enemy_a hard");
        assert!(g.world.istrats[IS_ZACOS].is_some(), "enemy_a zacos");
        assert!(g.world.istrats[IS_SHIPINTRO].is_some(), "player shipintro");
        assert_eq!(
            g.world.istrats[IS_NOCOLL], g.world.istrats[IS_NOCOLLANIM0],
            "NoColl alias"
        );
        assert_eq!(
            g.world.istrats[IS_HARD180YR], g.world.istrats[IS_HARD180YRFOG],
            "Hard180yr fog alias"
        );
        assert!(g.world.istrats[85].is_some(), "enemy_b bossA (IS_BOSSA=85)");
        assert!(g.world.istrats[99].is_some(), "enemy_b boss7 (IS_BOSS7=99)");
        assert!(g.world.istrats[108].is_some(), "bosses boss2 (IS_BOSS2=108)");
        assert!(g.world.istrats[84].is_some(), "bosses boss8 (IS_BOSS8=84)");

        // Path rows remain unwired (documented gap).
        assert!(g.world.istrats[IS_PATH].is_none(), "IS_PATH gap");
        assert!(g.world.istrats[IS_PATHT].is_none(), "IS_PATHT gap");
        assert!(g.world.istrats[IS_PATHDHA].is_none(), "IS_PATHDHA gap");

        // Address map: flat + synthetic for a known istrat, plus explicit
        // symbols. IS_PLAYER=0 -> flat 0x000000 and synth 0x020000.
        assert!(g.world.find_strategy_address(0x000000).is_some(), "flat player");
        assert!(g.world.find_strategy_address(0x020000).is_some(), "synth player");
        // Title screen Arwing (tick-30 spin): TIT 0x050020 must resolve.
        assert!(g.world.find_strategy_address(0x050020).is_some(), "TIT");
        assert!(g.world.find_strategy_address(STRAT_ADDR_TOW0EXPLODE).is_some(), "TOW0EXPLODE");
        assert!(g.world.find_strategy_address(STRAT_ADDR_GATE3).is_some(), "GATE3");
        assert!(g.world.find_strategy_address(0x060010).is_some(), "BOSSF");
        assert!(g.world.find_strategy_address(0x030005).is_some(), "BOSSSEAMON");
        assert!(g.world.find_strategy_address(0x060014).is_some(), "BOSS8");
    }
}
