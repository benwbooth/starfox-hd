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
//! The path rows are bridged through `path_adapter`, which keeps sf-path's
//! interpreter state synchronized with the sf-game object lane.

use crate::{bosses, bossf_heli, enemy_a, enemy_b, ground, player};
use sf_game::game::Game;
use sf_game::world::ISTRAT_CAPACITY;

// ============================================================
// ISTRATS.ASM def_Istrat indices (C src/strat/strat_table.c:31-94).
// Only the player/ground/enemy_a-owned rows are needed here; enemy_b and
// bosses carry their own index constants and self-register.
// ============================================================
const IS_PLAYER: usize = 0;
const IS_PBODY: usize = 1;
const IS_PLWING: usize = 2;
const IS_PRWING: usize = 3;
const IS_EXITLIGHT3: usize = 4;
const IS_EXITLIGHT4: usize = 5;
const IS_EXITLIGHT5: usize = 6;
const IS_EXITLIGHT6: usize = 7;
const IS_STAYREL: usize = 8;
const IS_STAYDIST: usize = 9;
const IS_NOCOLL: usize = 10;
const IS_GND: usize = 11;
const IS_EXITOPEN: usize = 13;
const IS_EXITOPENSND: usize = 14;
const IS_CLSHIPGNDA: usize = 15;
const IS_CLSHIPGNDB: usize = 16;
const IS_CLSHIPGNDC: usize = 17;
const IS_CLSHIPWARPA: usize = 18;
const IS_CLSHIPWARPB: usize = 19;
const IS_CLSHIPWARPC: usize = 20;
const IS_CLSHIPSHIPA: usize = 21;
const IS_CLSHIPSHIPB: usize = 22;
const IS_CLSHIPSHIPC: usize = 23;
const IS_CLSHIPEARTHA: usize = 24;
const IS_CLSHIPEARTHB: usize = 25;
const IS_CLSHIPEARTHC: usize = 26;
const IS_CLSHIPTURNA: usize = 27;
const IS_CLSHIPTURNB: usize = 28;
const IS_CLSHIPTURNC: usize = 29;
const IS_CLSHIPBRIDGEA: usize = 30;
const IS_CLSHIPBRIDGEB: usize = 31;
const IS_CLSHIPBRIDGEC: usize = 32;
const IS_CLSHIPCHASEA: usize = 33;
const IS_CLSHIPCHASEB: usize = 34;
const IS_CLSHIPCHASEC: usize = 35;
const IS_CLSHIPDIVEA: usize = 36;
const IS_CLSHIPDIVEB: usize = 37;
const IS_CLSHIPDIVEC: usize = 38;
const IS_CLSHIPUNDERA: usize = 39;
const IS_CLSHIPUNDERB: usize = 40;
const IS_CLSHIPUNDERC: usize = 41;
const IS_CLSHIP1: usize = 42;
const IS_CLSHIP2: usize = 43;
const IS_CLSHIP3: usize = 44;
const IS_PLAYERWARP: usize = 45;
const IS_FASTFIGHTER1: usize = 46;
const IS_KAMI: usize = 54;
const IS_SOKUTEN: usize = 55;
const IS_LARGEPLASMA: usize = 56;
const IS_WORMHEAD: usize = 51;
const IS_GATE: usize = 52;
const IS_SHARK: usize = 59;
const IS_WORM: usize = 60;
const IS_WORM2: usize = 61;
const IS_CAMELEON: usize = 62;
const IS_CAMELEON2: usize = 63;
const IS_BEE1: usize = 64;
const IS_FIGHTER: usize = 65;
const IS_BOSS1: usize = 68;
const IS_PILLAR3: usize = 78;
const IS_BOMWING: usize = 88;
const IS_UP1MAN: usize = 89;
const IS_RADER0: usize = 91;
const IS_RADER1: usize = 92;
const IS_ZACOS: usize = 93;
const IS_ZACO1L: usize = 94;
const IS_ZACO1R: usize = 95;
const IS_HOUDAI: usize = 96;
const IS_HOUDAINS: usize = 97;
const IS_ZACO3: usize = 99;
const IS_TOWER0: usize = 100;
const IS_ZACO0: usize = 101;
const IS_ZACO4: usize = 102;
const IS_HARDENEMY1: usize = 103;
const IS_HARD180YR: usize = 104;
const IS_HARD180YRNZR: usize = 105;
const IS_PARA: usize = 106;
const IS_FZACO: usize = 112;
const IS_FRIEND1: usize = 108;
const IS_FRIEND2: usize = 109;
const IS_FRIEND0: usize = 113;
const IS_INTRO1PFALL: usize = 121;
const IS_DOOR1: usize = 124;
const IS_HARD90YR: usize = 126;
const IS_SZACO2: usize = 128;
const IS_STAYRELHARD180YR: usize = 136;
const IS_CARRIER: usize = 138;
const IS_KICHI2: usize = 140;
const IS_SHIPS: usize = 132;
const IS_RIGHTWALL: usize = 148;
const IS_DUCT: usize = 150;
const IS_TZACO7CAT: usize = 162;
const IS_DRAGONFLY: usize = 164;
const IS_FRIENDEXITBASE: usize = 151;
const IS_PATH: usize = 156;
const IS_SPACEBARWALKER: usize = 172;
const IS_SPACEBARSHOOT: usize = 173;
const IS_WALKER2: usize = 179;
const IS_ITEM5: usize = 174;
const IS_ITEM7: usize = 176;
const IS_HOUDAI5: usize = 188;
const IS_PILLAR3F: usize = 189;
const IS_PARTICLEFIRE: usize = 190;
const IS_HARD180YRFOG: usize = 180; // alias: Strat_Hard180yr_Init
const IS_HARD90YRFOG: usize = 182;
const IS_AIRCAR1: usize = 198;
const IS_AIRCAR2: usize = 199;
const IS_AIRCAR3: usize = 200;
const IS_AIRCAR4: usize = 201;
const IS_AIRCAR5: usize = 202;
const IS_TRUCK1: usize = 213;
const IS_TRUCK2: usize = 214;
const IS_MONOLITH: usize = 215;
const IS_LSEQDOOR1: usize = 216;
const IS_LSEQDOOR2: usize = 217;
const IS_PSHIPOUTOFLB1: usize = 218;
const IS_VIEWOUTOFLB1: usize = 219;
const IS_GATE2: usize = 207;
const IS_TREE3: usize = 205;
const IS_MINE2: usize = 206;
const IS_SFISH: usize = 208;
const IS_HARDROT: usize = 209;
const IS_NOCOLLANIM0: usize = 220; // alias: Strat_NoColl_Init
const IS_PSHIPOUTOFLB3: usize = 221;
const IS_VIEWOUTOFLB3: usize = 222;
const IS_SHIPOUTOFLB3: usize = 223;
const IS_HARD: usize = 225;
const IS_HELPBALL: usize = 224;
const IS_TADPOLE: usize = 227;
const IS_PATHT: usize = 228;
const IS_BASE1: usize = 181;
const IS_SHIPINTRO: usize = 238;
const IS_SHIP0CDOWN: usize = 236;
const IS_BOSS7INTRO: usize = 239;
const IS_SKILLFLY: usize = 240;
const IS_PATHDHA: usize = 242;

// ============================================================
// Non-istrat symbol addresses (C strat_table.h:10-16). SPACEPILON/TIT/BOSSF
// are registered by enemy_b::register; the boss2/sea/8 synthetic addresses
// by bosses::register. Only these two are owned here.
// ============================================================
const STRAT_ADDR_TOW0EXPLODE: u32 = 0x030001;
const STRAT_ADDR_GATE3: u32 = 0x030002;
const STRAT_ADDR_SHIP0CDOWN: u32 = 0x030007;
const STRAT_ADDR_SHIP1A: u32 = 0x05000B;
const STRAT_ADDR_SHIP2: u32 = 0x05000C;
const STRAT_ADDR_SDOOR1: u32 = 0x05000D;
const STRAT_ADDR_SDOOR2: u32 = 0x05000E;
const STRAT_ADDR_CRUISER2: u32 = 0x05000F;
const STRAT_ADDR_CRUISER2FIRE: u32 = 0x050010;
const STRAT_ADDR_CRUISER1: u32 = 0x050025;
const STRAT_ADDR_CRUISER1F: u32 = 0x050026;
const STRAT_ADDR_SHIP3A: u32 = 0x050027;
const STRAT_ADDR_SHIP3: u32 = 0x050028;
const STRAT_ADDR_EXITOPENSND2: u32 = 0x050029;
const STRAT_ADDR_MONOLITH: u32 = 0x050015;
const STRAT_ADDR_PILLAR2: u32 = 0x09_97B3;

// Map callback keys (`sf-map::consts::cb::SET_PLAYER_*`).  The game-core
// callback dispatcher uses the same 24-bit key to find the Rust init routine
// without depending directly on sf-strat.
const CB_SET_PLAYER_EXITBASE: u32 = 0x010301;
const CB_SET_PLAYER_ONPLANET: u32 = 0x010302;
const CB_SET_PLAYER_CLEARDEMO: u32 = 0x010303;
const CB_SET_PLAYER_WARP: u32 = 0x010304;
const CB_SET_PLAYER_CLEAR_EARTH: u32 = 0x010305;
const CB_SET_PLAYER_CLEAR_CHASE: u32 = 0x010306;
const CB_SET_PLAYER_CLEAR_SHIP2: u32 = 0x010307;
const CB_SET_PLAYER_CLEAR_UNDER: u32 = 0x010308;
const CB_SET_PLAYER_DIVE: u32 = 0x010309;
const CB_SET_PLAYER_CLEAR_BRIDGE: u32 = 0x01030A;
const CB_SET_PLAYER_CLEAR_TURN: u32 = 0x01030B;
const CB_SET_PLAYER_WARPOUT: u32 = 0x01030C;
const CB_SET_PLAYER_ONWATER: u32 = 0x01030D;
const CB_SET_PLAYER_TOCSLOW: u32 = 0x01030E;
const CB_SET_PLAYER_INMTEXIT: u32 = 0x01030F;
const CB_SET_PLAYER_INLTEXIT: u32 = 0x010310;
const CB_SET_PLAYER_INSPACE: u32 = 0x010311;
const CB_SET_PLAYER_INTOLB1: u32 = 0x010312;
const CB_SET_PLAYER_OUTOFLB2A: u32 = 0x010313;
const CB_SET_PLAYER_ESCAPENUCLEUS: u32 = 0x010314;
const CB_SET_PLAYER_WASHENT: u32 = 0x010315;
const CB_SET_PLAYER_CLEAR_COLONY: u32 = 0x010316;

// C address-map encoding bases (strat_table.c:26-27).
const STRAT_ADDR_FLAT_ID_BASE: u32 = 0x000000;
const STRAT_ADDR_SYNTH_BASE: u32 = 0x020000;

/// C `Strat_RegisterAll` + `Strat_RegisterAddressMap` (strat_table.c).
///
/// Note vs C: C first copies `g_istrat_shape_defaults` into the mutable
/// strategy-shape table and clears the strategy callbacks. Rust seeds the
/// generated shape defaults here, but does not clear callbacks because
/// `World::init` has already installed the builtin space-bar rows (166-169).
pub fn register_all(g: &mut Game) {
    debug_assert!(crate::istrat_shapes::ISTRAT_DEFAULT_COUNT <= ISTRAT_CAPACITY);
    g.world
        .istrat_shapes
        .copy_from_slice(&crate::istrat_shapes::ISTRAT_SHAPE_DEFAULTS);
    // ---- Lanes that hand back handles (player / ground / enemy_a) ----
    let p = player::install(g);
    // Publish the player collision-proxy box strat handles so the game-core
    // per-level setup (`Game::pcbox_attach_player`, ROM GSTRATS player setup)
    // can build the boxes that route enemy hits onto the ship. Runs on every
    // World::init (each level load), keeping the handles valid for the freshly
    // rebuilt registry.
    g.coldet.pcbox_strats = Some((p.pcbox_body, p.pcbox_wing, p.pcbox_coll));
    let gr = ground::install(g);
    let ea = enemy_a::install(g);
    let monolith = enemy_a::sid(g, enemy_a::monolith_istrat);
    let lseqdoor1 = enemy_a::sid(g, crate::enemies_ground::lseqdoor1_istrat);
    let lseqdoor2 = enemy_a::sid(g, crate::enemies_ground::lseqdoor2_istrat);
    let pshipoutoflb1 = enemy_a::sid(g, player::pshipoutoflb1_istrat);
    let viewoutoflb1 = enemy_a::sid(g, player::viewoutoflb1_istrat);
    let pshipoutoflb3 = enemy_a::sid(g, player::pshipoutoflb3_istrat);
    let viewoutoflb3 = enemy_a::sid(g, player::viewoutoflb3_istrat);
    let shipoutoflb3 = enemy_a::sid(g, enemy_a::shipoutoflb3_istrat);
    // Path-following adapter: registers the path-lane strategies, installs the
    // PathWorld on `Game`, and hands back the three IS_PATH init handles.
    // Done before the `is` borrow below (it needs `&mut g`).
    let path = crate::path_adapter::register(g);

    // ---- IS_XXX index placement (C Strat_RegisterAll body) ----
    let is = &mut g.world.istrats;

    is[IS_PLAYER] = Some(p.player);
    is[IS_PBODY] = Some(p.pcbox_body);
    is[IS_PLWING] = Some(p.pcbox_wing);
    is[IS_PRWING] = Some(p.pcbox_wing);
    is[IS_EXITLIGHT3] = Some(ea.exitlight3);
    is[IS_EXITLIGHT4] = Some(ea.exitlight4);
    is[IS_EXITLIGHT5] = Some(ea.exitlight5);
    is[IS_EXITLIGHT6] = Some(ea.exitlight6);

    // Ground / static objects
    is[IS_STAYREL] = Some(gr.stayrel);
    is[IS_STAYDIST] = Some(gr.staydist);
    is[IS_STAYRELHARD180YR] = Some(gr.stayrelhard180yr);
    is[IS_GND] = Some(gr.gnd);
    is[IS_EXITOPEN] = Some(ea.exitopen);
    is[IS_EXITOPENSND] = Some(ea.exitopensnd);

    // Decorative (no collision)
    is[IS_NOCOLL] = Some(ea.nocoll);
    is[IS_NOCOLLANIM0] = Some(ea.nocoll); // alias -> Strat_NoColl_Init
    is[IS_MONOLITH] = Some(monolith);
    is[IS_LSEQDOOR1] = Some(lseqdoor1);
    is[IS_LSEQDOOR2] = Some(lseqdoor2);
    is[IS_PSHIPOUTOFLB1] = Some(pshipoutoflb1);
    is[IS_VIEWOUTOFLB1] = Some(viewoutoflb1);
    is[IS_PSHIPOUTOFLB3] = Some(pshipoutoflb3);
    is[IS_VIEWOUTOFLB3] = Some(viewoutoflb3);
    is[IS_SHIPOUTOFLB3] = Some(shipoutoflb3);
    is[IS_GATE] = Some(ea.gate);
    is[IS_GATE2] = Some(ea.gate2);
    is[IS_WORMHEAD] = Some(ea.wormhead);
    is[IS_WORM] = Some(ea.worm);
    is[IS_WORM2] = Some(ea.worm2);
    is[IS_SHARK] = Some(ea.shark);

    // Indestructible buildings
    is[IS_HARD] = Some(ea.hard);
    is[IS_HARDENEMY1] = Some(ea.hardenemy1);
    is[IS_HARD180YR] = Some(ea.hard180yr);
    is[IS_HARD180YRNZR] = Some(ea.hard180yr_nzr);
    is[IS_HARD90YR] = Some(ea.hard90yr);
    is[IS_HARD180YRFOG] = Some(ea.hard180yr); // alias -> Strat_Hard180yr_Init
    is[IS_HARD90YRFOG] = Some(ea.hard90yrfog);
    is[IS_HARDROT] = Some(ea.hardrot);

    // Enemy fighters / structures
    is[IS_BOMWING] = Some(ea.bomwing);
    is[IS_UP1MAN] = Some(ea.up1man);
    is[IS_ZACOS] = Some(ea.zacos);
    is[IS_RADER0] = Some(ea.rader0);
    is[IS_RADER1] = Some(ea.rader1);
    is[IS_BOSS1] = Some(ea.boss1);
    is[IS_CAMELEON] = Some(ea.cameleon);
    is[IS_CAMELEON2] = Some(ea.cameleon2);
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
    is[IS_FZACO] = Some(ea.fzaco);
    is[IS_AIRCAR1] = Some(ea.aircar1);
    is[IS_AIRCAR2] = Some(ea.aircar2);
    is[IS_AIRCAR3] = Some(ea.aircar3);
    is[IS_AIRCAR4] = Some(ea.aircar4);
    is[IS_AIRCAR5] = Some(ea.aircar5);
    is[IS_TRUCK1] = Some(ea.truck1);
    is[IS_TRUCK2] = Some(ea.truck2);
    is[IS_CARRIER] = Some(ea.carrier);
    is[IS_BASE1] = Some(ea.base1);
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
    is[IS_CLSHIPSHIPA] = Some(ea.clship_shipa);
    is[IS_CLSHIPSHIPB] = Some(ea.clship_shipb);
    is[IS_CLSHIPSHIPC] = Some(ea.clship_shipc);
    is[IS_CLSHIPTURNA] = Some(ea.clship_turna);
    is[IS_CLSHIPTURNB] = Some(ea.clship_turnb);
    is[IS_CLSHIPTURNC] = Some(ea.clship_turnc);
    is[IS_CLSHIPBRIDGEA] = Some(ea.clship_bridgea);
    is[IS_CLSHIPBRIDGEB] = Some(ea.clship_bridgeb);
    is[IS_CLSHIPBRIDGEC] = Some(ea.clship_bridgec);
    is[IS_CLSHIPDIVEA] = Some(ea.clship_divea);
    is[IS_CLSHIPDIVEB] = Some(ea.clship_diveb);
    is[IS_CLSHIPDIVEC] = Some(ea.clship_divec);
    is[IS_CLSHIPUNDERA] = Some(ea.clship_undera);
    is[IS_CLSHIPUNDERB] = Some(ea.clship_underb);
    is[IS_CLSHIPUNDERC] = Some(ea.clship_underc);
    is[IS_SHIPINTRO] = Some(p.ship_intro_init);
    is[IS_SPACEBARWALKER] = Some(ea.spacebarwalker);
    is[IS_SPACEBARSHOOT] = Some(ea.spacebarshoot);
    is[IS_ITEM5] = Some(ea.item5);
    is[IS_ITEM7] = Some(ea.item7);

    // Path-following enemies/wingmen (C sets IS_PATH/IS_PATHT/IS_PATHDHA to
    // sf-path's path istrats), driven through the Game<->PathWorld adapter
    // registered above.
    is[IS_PATH] = Some(path.path_init);
    is[IS_PATHT] = Some(path.patht_init);
    is[IS_PATHDHA] = Some(path.pathdha_init);

    // ---- Self-registering lanes (enemy_b bosses + stage bosses) ----
    // (C: bossA/boss7/bossF rows + StratBoss2/Sea/8_Register.) These place
    // their own istrat rows and synthetic addresses (SPACEPILON 0x030004,
    // TIT 0x050020, BOSSF 0x060010, BOSSSEAMON/BOSSG 0x030005/06,
    // BOSS8/launcher/pillar 0x060014/15/16).
    enemy_b::register(&mut g.world);
    bosses::register(&mut g.world);
    crate::damyscr::register(&mut g.world);
    bossf_heli::register(&mut g.world);
    // Ground-artillery family (tank1a/tank2/tank3 + bazookaL/R). Self-populates
    // its g_istrats rows exactly like bosses::register.
    crate::enemies_ground::register(&mut g.world);
    // Kichi2 is a source-table jump alias for NoColl, rather than an
    // independently registered strategy. Keep the same semantic handle even
    // when an optimized build emits different addresses for the same Rust
    // function referenced from separate code-generation units.
    g.world.istrats[IS_KICHI2] = Some(ea.nocoll);
    // Andross final boss (bossB face IS 114 + bossBrob robot IS 117). Self-
    // populates its istrat rows and the MAP1_5 synthetic address (0x06000F).
    crate::bossb::register(&mut g.world);
    // bossH — the "gggy" legged spider (D3STRATS.ASM). No ISTRATS.ASM row;
    // placed by direct address in MAP1_4.ASM:217. Registers STRAT_ADDR_BOSSH
    // (0x060011), which sf-map level1_4 uses for the live encounter.
    crate::bossh::register(&mut g.world);
    // Mother system (ASM/MOTHER.ASM + D2STRATS mother1/2 + the MOTHERS.ASM
    // child strategies): registers STRAT_ADDR_MOTHER1/2 and the
    // meteor/slowmeteor/searchmeteor/clasteroid child strats.
    crate::mother::register(g);

    // Complete the canonical ISTRATS table with the already-native entry
    // points that are not owned by one of the smaller lane registrars above.
    // Keeping every source row populated is required for DOBJ/QOBJ2 map
    // spawns, which carry only a strategy row and derive their shape from the
    // generated strategy-to-shape table.
    let canonical_entries: &[(usize, sf_game::game::StrategyFn)] = &[
        (IS_CLSHIP1, enemy_a::clship1_istrat),
        (IS_CLSHIP2, enemy_a::clship2_istrat),
        (IS_CLSHIP3, enemy_a::clship3_istrat),
        (IS_PLAYERWARP, player::player_warp_istrat),
        (IS_FASTFIGHTER1, enemy_a::fastfighter1_istrat),
        (IS_KAMI, enemy_a::kami_istrat),
        (IS_SOKUTEN, crate::enemies_ground::sokuten_istrat),
        (IS_LARGEPLASMA, enemy_a::largeplasma_istrat),
        (IS_BEE1, enemy_a::bee1_istrat),
        (IS_FIGHTER, enemy_a::fighter_istrat),
        (IS_FRIEND1, enemy_a::friend1_istrat),
        (IS_FRIEND2, enemy_a::friend2_istrat),
        (IS_FRIEND0, enemy_a::friend0_istrat),
        (IS_INTRO1PFALL, crate::enemies_ground::intro1pfall_istrat),
        (IS_DOOR1, crate::enemies_ground::door1_istrat),
        (IS_SHIPS, crate::enemies_ground::ships_istrat),
        (IS_RIGHTWALL, crate::enemies_ground::rightwall_istrat),
        (IS_DUCT, crate::enemies_ground::duct_istrat),
        (IS_TZACO7CAT, enemy_a::tzaco7cat_istrat),
        (IS_DRAGONFLY, enemy_a::dragonfly_istrat),
        (IS_WALKER2, crate::enemies_ground::walker2_istrat),
        (IS_HOUDAI5, crate::enemies_ground::houdai5_istrat),
        (IS_PILLAR3F, crate::enemies_ground::pillar3f_istrat),
        (IS_PARTICLEFIRE, enemy_a::particlefire_istrat),
        (IS_TREE3, crate::enemies_ground::tree3_istrat),
        (IS_MINE2, crate::enemies_ground::mine2_istrat),
        (IS_SFISH, crate::enemies_ground::sfish_istrat),
        (IS_HELPBALL, enemy_a::helpball_istrat),
        (IS_SHIP0CDOWN, enemy_a::ship0cdown_istrat),
        (IS_BOSS7INTRO, enemy_b::boss7intro_istrat),
    ];
    for &(row, strategy) in canonical_entries {
        let id = enemy_a::sid(g, strategy);
        g.world.istrats[row] = Some(id);
    }

    // ---- Address map (C Strat_RegisterAddressMap, strat_table.c:104) ----
    // Flat-id + synthetic `0x02:xxxx` forms for every non-null istrat...
    for i in 0..ISTRAT_CAPACITY {
        if let Some(id) = g.world.istrats[i] {
            g.world
                .register_strategy_address(STRAT_ADDR_FLAT_ID_BASE | i as u32, id);
            g.world
                .register_strategy_address(STRAT_ADDR_SYNTH_BASE | i as u32, id);
            // P_SETSTRAT operands in the byte-exact assembled path catalog
            // are real 24-bit ROM symbols, not the old literal builder's flat
            // ids. Register both address spaces against the same Rust handle.
            let rom_addr = sf_path::rom_catalog_data::ROM_ISTRAT_ADDRS[i];
            if rom_addr != 0 {
                g.world.register_strategy_address(rom_addr, id);
            }
        }
    }
    // ...then the explicit non-istrat symbols owned here (enemy_a handles).
    g.world
        .register_strategy_address(STRAT_ADDR_TOW0EXPLODE, ea.tow0_explode);
    g.world.register_strategy_address(
        sf_path::rom_catalog_data::ROM_TOW0EXPLODE_ISTRAT_ADDR,
        ea.tow0_explode,
    );
    g.world
        .register_strategy_address(STRAT_ADDR_GATE3, ea.gate3);
    g.world
        .register_strategy_address(sf_path::rom_catalog_data::ROM_GATE3_ISTRAT_ADDR, ea.gate3);
    // Direct strategy symbols used by the Space Armada maps.  These are not
    // ISTRAT table rows in the rebuilt map ABI, so the flat/synthetic loop
    // above cannot discover them.
    for (addr, strat) in [
        (
            STRAT_ADDR_SHIP0CDOWN,
            enemy_a::ship0cdown_istrat as sf_game::game::StrategyFn,
        ),
        (STRAT_ADDR_SHIP1A, enemy_a::ship1a_istrat),
        (STRAT_ADDR_SHIP2, enemy_a::ship2_istrat),
        (STRAT_ADDR_SDOOR1, crate::enemies_ground::sdoor1_istrat),
        (STRAT_ADDR_SDOOR2, crate::enemies_ground::sdoor2_istrat),
        (STRAT_ADDR_CRUISER2, crate::enemies_ground::cruiser2_istrat),
        (
            STRAT_ADDR_CRUISER2FIRE,
            crate::enemies_ground::cruiser2fire_istrat,
        ),
        (STRAT_ADDR_CRUISER1, crate::enemies_ground::cruiser1_istrat),
        (
            STRAT_ADDR_CRUISER1F,
            crate::enemies_ground::cruiser1f_istrat,
        ),
        (STRAT_ADDR_SHIP3A, enemy_a::ship3a_istrat),
        (STRAT_ADDR_SHIP3, enemy_a::ship3_istrat),
        (STRAT_ADDR_EXITOPENSND2, enemy_a::exitopensnd2_istrat),
        (STRAT_ADDR_MONOLITH, enemy_a::monolith_istrat),
        (STRAT_ADDR_PILLAR2, enemy_a::pillar2_istrat),
    ] {
        let id = enemy_a::sid(g, strat);
        g.world.register_strategy_address(addr, id);
    }
    for (strategy, strat) in [
        (
            sf_map::consts::DirectStrategy::HalfDPillar,
            enemy_a::halfd_istrat as sf_game::game::StrategyFn,
        ),
        (sf_map::consts::DirectStrategy::Pole0, enemy_a::pole0_istrat),
        (
            sf_map::consts::DirectStrategy::GroundPilon,
            enemy_a::groundpilon_istrat,
        ),
    ] {
        let id = enemy_a::sid(g, strat);
        g.world.register_direct_strategy(strategy, id);
    }
    // C `world_cb_set_player_exitbase_l` (world.c:595) calls Strat_PlayerExitBase
    // directly; the Rust map-VM builtin routes through this registered address.
    g.world
        .register_strategy_address(sf_game::world::STRAT_ADDR_PLAYER_EXITBASE, p.exit_base_init);
    let player_callbacks: &[(u32, sf_game::game::StrategyFn)] = &[
        (CB_SET_PLAYER_EXITBASE, player::strat_player_exit_base),
        (CB_SET_PLAYER_ONPLANET, player::set_player_on_planet),
        (CB_SET_PLAYER_CLEARDEMO, player::player_clear_demo_istrat),
        (CB_SET_PLAYER_WARP, player::player_warp_istrat),
        (
            CB_SET_PLAYER_CLEAR_EARTH,
            player::player_clear_earth2_istrat,
        ),
        (CB_SET_PLAYER_CLEAR_CHASE, player::player_clear_chase_istrat),
        (CB_SET_PLAYER_CLEAR_SHIP2, player::player_clear_ship2_istrat),
        (CB_SET_PLAYER_CLEAR_UNDER, player::player_clear_under_istrat),
        (CB_SET_PLAYER_DIVE, player::player_dive_istrat),
        (
            CB_SET_PLAYER_CLEAR_BRIDGE,
            player::strat_player_clear_bridge_init,
        ),
        (CB_SET_PLAYER_CLEAR_TURN, player::player_clear_turn_istrat),
        (CB_SET_PLAYER_WARPOUT, player::player_warp_out_istrat),
        (CB_SET_PLAYER_ONWATER, player::set_player_on_water),
        (CB_SET_PLAYER_TOCSLOW, player::set_player_to_cslow),
        (CB_SET_PLAYER_INMTEXIT, player::set_player_in_mtexit),
        (CB_SET_PLAYER_INLTEXIT, player::set_player_in_ltexit),
        (CB_SET_PLAYER_INSPACE, player::set_player_in_space),
        (CB_SET_PLAYER_INTOLB1, player::set_player_into_lb1),
        (CB_SET_PLAYER_OUTOFLB2A, player::set_player_out_of_lb2a),
        (
            CB_SET_PLAYER_ESCAPENUCLEUS,
            player::set_player_escape_nucleus,
        ),
        (CB_SET_PLAYER_WASHENT, player::player_washent_istrat),
        (
            CB_SET_PLAYER_CLEAR_COLONY,
            player::player_clear_colony_istrat,
        ),
    ];
    for &(addr, strat) in player_callbacks {
        let id = enemy_a::sid(g, strat);
        g.world.register_strategy_address(addr, id);
    }
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
        assert!(
            g.world.istrats[IS_STAYRELHARD180YR].is_some(),
            "ground stayrelhard180yr"
        );
        assert!(g.world.istrats[IS_HARD].is_some(), "enemy_a hard");
        assert!(
            g.world.istrats[IS_HARDENEMY1].is_some(),
            "enemy_a hardenemy1"
        );
        assert!(
            g.world.istrats[IS_HARD90YRFOG].is_some(),
            "enemy_a hard90yrfog"
        );
        assert!(g.world.istrats[IS_SHARK].is_some(), "enemy_a shark");
        assert!(g.world.istrats[IS_FZACO].is_some(), "enemy_a fzaco");
        assert!(g.world.istrats[IS_ZACOS].is_some(), "enemy_a zacos");
        assert!(g.world.istrats[IS_SHIPINTRO].is_some(), "player shipintro");
        assert_eq!(
            g.world.istrat_shapes[IS_TADPOLE], 227,
            "tadpole strategy resolves its visible catalog shape"
        );
        assert_eq!(
            g.world.istrat_shapes[IS_SHIPINTRO], 2,
            "ship-intro strategy resolves the player craft shape"
        );
        assert_eq!(
            g.world.istrat_shapes[239], 55,
            "boss-7 intro strategy resolves the boss shell shape"
        );
        for (row, name) in [
            (IS_MONOLITH, "monolith"),
            (IS_LSEQDOOR1, "lseqdoor1"),
            (IS_LSEQDOOR2, "lseqdoor2"),
            (IS_PSHIPOUTOFLB1, "pshipoutoflb1"),
            (IS_VIEWOUTOFLB1, "viewoutoflb1"),
            (IS_PSHIPOUTOFLB3, "pshipoutoflb3"),
            (IS_VIEWOUTOFLB3, "viewoutoflb3"),
            (IS_SHIPOUTOFLB3, "shipoutoflb3"),
        ] {
            assert!(g.world.istrats[row].is_some(), "{name}");
        }
        assert_eq!(
            g.world.istrats[IS_NOCOLL], g.world.istrats[IS_NOCOLLANIM0],
            "NoColl alias"
        );
        assert_eq!(
            g.world.istrats[IS_NOCOLL], g.world.istrats[IS_KICHI2],
            "kichi2 jumps to nocoll_istrat"
        );
        assert_eq!(
            g.world.istrats[IS_HARD180YR], g.world.istrats[IS_HARD180YRFOG],
            "Hard180yr fog alias"
        );
        assert!(g.world.istrats[84].is_some(), "enemy_b bossA (IS_BOSSA=84)");
        assert!(g.world.istrats[98].is_some(), "enemy_b boss7 (IS_BOSS7=98)");
        assert!(
            g.world.istrats[107].is_some(),
            "bosses boss2 (IS_BOSS2=107)"
        );
        assert!(g.world.istrats[83].is_some(), "bosses boss8 (IS_BOSS8=83)");

        let missing: Vec<usize> = g.world.istrats[..crate::istrat_shapes::ISTRAT_DEFAULT_COUNT]
            .iter()
            .enumerate()
            .filter_map(|(row, strategy)| strategy.is_none().then_some(row))
            .collect();
        assert!(
            missing.is_empty(),
            "native strategy registry has missing ISTRATS.ASM rows: {missing:?}"
        );

        // Path rows are now wired through the Game<->PathWorld adapter.
        assert!(g.world.istrats[IS_PATH].is_some(), "IS_PATH wired");
        assert!(g.world.istrats[IS_PATHT].is_some(), "IS_PATHT wired");
        assert!(g.world.istrats[IS_PATHDHA].is_some(), "IS_PATHDHA wired");
        // The adapter installs a PathWorld on the game.
        assert!(g.path.is_some(), "PathWorld installed");

        // Address map: flat + synthetic for a known istrat, plus explicit
        // symbols. IS_PLAYER=0 -> flat 0x000000 and synth 0x020000.
        assert!(
            g.world.find_strategy_address(0x000000).is_some(),
            "flat player"
        );
        assert!(
            g.world.find_strategy_address(0x020000).is_some(),
            "synth player"
        );
        assert_eq!(
            g.world.find_strategy_address(sf_map::consts::is::TIT),
            g.world.istrats[sf_map::consts::is::TIT as usize],
            "TIT compact strategy row"
        );
        assert!(
            g.world.find_strategy_address(STRAT_ADDR_PILLAR2).is_some(),
            "pillar2 direct symbol"
        );
        assert!(
            g.world
                .find_strategy_address(STRAT_ADDR_TOW0EXPLODE)
                .is_some(),
            "TOW0EXPLODE"
        );
        assert!(
            g.world.find_strategy_address(STRAT_ADDR_GATE3).is_some(),
            "GATE3"
        );
        assert_eq!(
            g.world.find_strategy_address(sf_map::consts::is::BOSSF),
            g.world.istrats[sf_map::consts::is::BOSSF as usize],
            "BOSSF compact strategy row"
        );
        let bossseamon = enemy_a::sid(&mut g, bosses::strat_bossseamon_init);
        assert_eq!(
            g.world
                .find_direct_strategy(sf_map::consts::DirectStrategy::BossSeamon),
            Some(bossseamon),
            "typed BossSeamon key must resolve to the intended strategy"
        );
        let direct: &[(sf_map::consts::DirectStrategy, sf_game::game::StrategyFn)] = &[
            (
                sf_map::consts::DirectStrategy::BossSeamon,
                bosses::strat_bossseamon_init,
            ),
            (
                sf_map::consts::DirectStrategy::Mine0,
                crate::enemies_ground::mine0_istrat,
            ),
            (
                sf_map::consts::DirectStrategy::Mother1,
                crate::mother::strat_mother1_init,
            ),
            (
                sf_map::consts::DirectStrategy::Mother2,
                crate::mother::strat_mother2_init,
            ),
            (
                sf_map::consts::DirectStrategy::Meteor,
                crate::mother::strat_meteor_init,
            ),
            (
                sf_map::consts::DirectStrategy::SlowMeteor,
                crate::mother::strat_slowmeteor_init,
            ),
            (
                sf_map::consts::DirectStrategy::SearchMeteor,
                crate::mother::strat_searchmeteor_init,
            ),
            (
                sf_map::consts::DirectStrategy::Clasteroid,
                crate::mother::strat_clasteroid_init,
            ),
            (
                sf_map::consts::DirectStrategy::SeaDragon,
                bosses::sd_seadragon_init,
            ),
            (
                sf_map::consts::DirectStrategy::Damyscr,
                crate::damyscr::damyscr_init,
            ),
            (
                sf_map::consts::DirectStrategy::SpacePilon,
                enemy_b::strat_spacepilon_init,
            ),
            (
                sf_map::consts::DirectStrategy::BossH,
                crate::bossh::bossh_init,
            ),
            (
                sf_map::consts::DirectStrategy::HalfDPillar,
                enemy_a::halfd_istrat,
            ),
            (sf_map::consts::DirectStrategy::Pole0, enemy_a::pole0_istrat),
            (
                sf_map::consts::DirectStrategy::GroundPilon,
                enemy_a::groundpilon_istrat,
            ),
        ];
        assert_eq!(direct.len(), sf_map::consts::DirectStrategy::ALL.len());
        for &(key, strategy) in direct {
            let expected = enemy_a::sid(&mut g, strategy);
            assert_eq!(
                g.world.find_direct_strategy(key),
                Some(expected),
                "{key:?} strategy key was overwritten or misregistered"
            );
        }
        assert_eq!(
            g.world.find_strategy_address(sf_map::consts::is::BOSS8),
            g.world.istrats[sf_map::consts::is::BOSS8 as usize],
            "BOSS8 compact strategy row"
        );
        for (addr, name) in [
            (STRAT_ADDR_SHIP0CDOWN, "SHIP0CDOWN"),
            (STRAT_ADDR_SHIP1A, "SHIP1A"),
            (STRAT_ADDR_SHIP2, "SHIP2"),
            (STRAT_ADDR_SDOOR1, "SDOOR1"),
            (STRAT_ADDR_SDOOR2, "SDOOR2"),
            (STRAT_ADDR_CRUISER2, "CRUISER2"),
            (STRAT_ADDR_CRUISER2FIRE, "CRUISER2FIRE"),
            (STRAT_ADDR_CRUISER1, "CRUISER1"),
            (STRAT_ADDR_CRUISER1F, "CRUISER1F"),
            (STRAT_ADDR_SHIP3A, "SHIP3A"),
            (STRAT_ADDR_SHIP3, "SHIP3"),
            (STRAT_ADDR_EXITOPENSND2, "EXITOPENSND2"),
        ] {
            assert!(g.world.find_strategy_address(addr).is_some(), "{name}");
        }
        for &(addr, _) in player_callbacks_for_test() {
            assert!(
                g.world.find_strategy_address(addr).is_some(),
                "player callback {addr:#08x}"
            );
        }
    }

    fn player_callbacks_for_test() -> &'static [(u32, &'static str)] {
        &[
            (CB_SET_PLAYER_EXITBASE, "exitbase"),
            (CB_SET_PLAYER_ONPLANET, "onplanet"),
            (CB_SET_PLAYER_CLEARDEMO, "cleardemo"),
            (CB_SET_PLAYER_WARP, "warp"),
            (CB_SET_PLAYER_CLEAR_EARTH, "earth"),
            (CB_SET_PLAYER_CLEAR_CHASE, "chase"),
            (CB_SET_PLAYER_CLEAR_SHIP2, "ship2"),
            (CB_SET_PLAYER_CLEAR_UNDER, "under"),
            (CB_SET_PLAYER_DIVE, "dive"),
            (CB_SET_PLAYER_CLEAR_BRIDGE, "bridge"),
            (CB_SET_PLAYER_CLEAR_TURN, "turn"),
            (CB_SET_PLAYER_WARPOUT, "warpout"),
            (CB_SET_PLAYER_ONWATER, "onwater"),
            (CB_SET_PLAYER_TOCSLOW, "tocslow"),
            (CB_SET_PLAYER_INMTEXIT, "inmtexit"),
            (CB_SET_PLAYER_INLTEXIT, "inltexit"),
            (CB_SET_PLAYER_INSPACE, "inspace"),
            (CB_SET_PLAYER_INTOLB1, "intolb1"),
            (CB_SET_PLAYER_OUTOFLB2A, "outoflb2a"),
            (CB_SET_PLAYER_ESCAPENUCLEUS, "escapenucleus"),
        ]
    }
}
