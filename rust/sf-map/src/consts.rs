//! Constants shared by the map bytecode builders.
//!
//! C oracle: `src/map/levels.c` (top-of-file `#define` blocks) and
//! `src/game/world.h` (MAP_OP_* / MAP_CB_*). Values must stay identical —
//! they are baked into the emitted bytecode.

// ============================================================
// Map opcodes (src/game/world.h, from MAPMACS.INC)
// ============================================================
pub mod op {
    pub const MAPOBJ: u8 = 0;
    pub const END: u8 = 2;
    pub const LOOP: u8 = 4;
    pub const MOTHER: u8 = 10;
    pub const REMOVE: u8 = 12;
    pub const SETBG: u8 = 16;
    pub const WAIT: u8 = 18;
    pub const SETBGM: u8 = 20;
    pub const JSR: u8 = 40;
    pub const RTS: u8 = 42;
    pub const IF: u8 = 44;
    pub const GOTO: u8 = 46;
    pub const SETALVARB: u8 = 54;
    pub const SETALVARW: u8 = 56;
    pub const SETALXVARB: u8 = 60;
    pub const SETALXVARW: u8 = 62;
    pub const FADEUP: u8 = 66;
    pub const FADEDOWN: u8 = 68;
    pub const SETALVARPW: u8 = 72;
    pub const SETVAROBJ: u8 = 74;
    pub const WAITFADE: u8 = 76;
    pub const QFADEUP: u8 = 78;
    pub const QFADEDOWN: u8 = 80;
    pub const SPECIAL: u8 = 90;
    pub const SETVARB: u8 = 92;
    pub const SETVARW: u8 = 94;
    pub const WAITSETBG: u8 = 100;
    pub const SETBGINFO: u8 = 102;
    pub const ADDALVARPW: u8 = 106;
    pub const FADETOSEA: u8 = 108;
    pub const FADETOGROUND: u8 = 110;
    pub const CODE65816: u8 = 120;
    pub const CODEJSL: u8 = 122;
    pub const SENDMSG: u8 = 130;
    pub const CSPECIAL: u8 = 132;
    pub const NORMOBJ: u8 = 134;
    pub const SETPATH: u8 = 140;
}

// ============================================================
// Native map callback ids (src/game/world.h MAP_CB_* +
// levels.c local CL_GND ids). Passed to World_RegisterNativeCallback
// and encoded into mapif/mapcodejsl operands.
// ============================================================
pub mod cb {
    pub const CHKSTAGEDONE: u32 = 0x010001;
    pub const CHKSTRATDONE1: u32 = 0x010002;
    pub const CHKSTRATDONE2: u32 = 0x010003;
    pub const CHKBOSSDEAD: u32 = 0x010004;
    pub const THEENDDEAD: u32 = 0x010005;

    pub const INITBLACK_L: u32 = 0x010101;
    pub const SETCHARMAPFROMMAP_L: u32 = 0x010102;
    pub const INITFADEWHITE2NORM_L: u32 = 0x010103;
    pub const KILL_ROBOT_L: u32 = 0x010104;
    pub const CLEARMAP_L: u32 = 0x010105;
    pub const CLEARREALOBJMAP_L: u32 = 0x010106;
    pub const SETRESTART_L: u32 = 0x010107;
    pub const MARKBOSS_L: u32 = 0x010108;

    pub const FROG_ALIVE: u32 = 0x010201;
    pub const BUNNY_ALIVE: u32 = 0x010202;
    pub const COCK_ALIVE: u32 = 0x010203;

    pub const CLFRIENDMSG_FROG: u32 = 0x010211;
    pub const CLFRIENDMSG_BUNNY: u32 = 0x010212;
    pub const CLFRIENDMSG_COCK: u32 = 0x010213;

    pub const SET_PLAYER_EXITBASE_L: u32 = 0x010301;
    pub const SET_PLAYER_ONPLANET_L: u32 = 0x010302;
    pub const SET_PLAYER_CLEARDEMO_L: u32 = 0x010303;
    pub const SET_PLAYER_WARP_L: u32 = 0x010304;
    pub const SET_PLAYER_CLEAR_EARTH_L: u32 = 0x010305;
    pub const SET_PLAYER_CLEAR_CHASE_L: u32 = 0x010306;
    pub const SET_PLAYER_CLEAR_SHIP2_L: u32 = 0x010307;
    pub const SET_PLAYER_CLEAR_UNDER_L: u32 = 0x010308;
    pub const SET_PLAYER_DIVE_L: u32 = 0x010309;
    pub const SET_PLAYER_CLEAR_BRIDGE_L: u32 = 0x01030A;
    pub const SET_PLAYER_CLEAR_TURN_L: u32 = 0x01030B;
    pub const SET_PLAYER_WARPOUT_L: u32 = 0x01030C;
    pub const SET_PLAYER_ONWATER_L: u32 = 0x01030D;

    pub const IS_PLAYER_DEAD: u32 = 0x010401;
    pub const PLAYER_OUTVIEW_L: u32 = 0x010402;
    pub const LEVELFINISHED_ZERO: u32 = 0x010403;

    pub const BG_1_4B_1_L: u32 = 0x010501;
    pub const BLOCKSND_L: u32 = 0x010502;

    // Local native callback ids used only by the CL_GND / CL_WARP / CL_DIVE
    // literal submaps (levels.c).
    pub const CL_GROUND_PRINTLEVELFIN: u32 = 0x01F101;
    pub const CL_GROUND_WIPEOUT: u32 = 0x01F102;
    pub const CL_WARP_PRINTLEVELFIN: u32 = 0x01F103;
    pub const CL_DIVE_CLEAR_ENGINESND: u32 = 0x01F104;
}

// Bounded MAPMACS.INC space-bar runtime strategy ids (src/game/world.h).
pub const MAP_ISTRAT_SPACEBAR: u32 = 166;
pub const MAP_ISTRAT_SPINSPACEBAR: u32 = 167;
pub const MAP_ISTRAT_SPACEBAR1: u32 = 168;
pub const MAP_ISTRAT_SPACEBAR3: u32 = 169;

// ============================================================
// Rotation units (src/variables.h)
// ============================================================
pub const DEG360: i32 = 256;
pub const DEG180: i32 = 128;
pub const DEG90: i32 = 64;
pub const DEG45: i32 = 32;
pub const DEG22: i32 = 16;
pub const DEG11: i32 = 8;
pub const DEG270: i32 = 192;
pub const DEG0: i32 = 0;

// ============================================================
// LEVEL1_1.ASM constants from STRATEQU.INC / map sources (levels.c)
// ============================================================
pub const MEDPSPEED: i32 = 65;
pub const PEXITBASE_SPEED: i32 = 50;
pub const MYBASE_SCALE: i32 = 3;
pub const BOSS7_SCALE: i32 = 3;
pub const BOSSA_SCALE: i32 = 2;
pub const BGM_BOSS1: i32 = 5;
pub const BGM_FANFARE: i32 = 7;
pub const BGM_FADEOUT: i32 = 0xF1;
pub const BG_3_1C: i32 = 3;
pub const BG_1_1C: i32 = 4;
pub const BG_INTRO: i32 = 40;
pub const BG_TITLE: i32 = 41;
pub const BG_CONT: i32 = 42;
pub const BG_CRED: i32 = 43;
pub const BG_TRAINING: i32 = 44;
pub const CL_GND_FRIENDWAIT: i32 = MEDPSPEED * 30;
pub const CL_WARP_FRIENDWAIT: i32 = CL_GND_FRIENDWAIT;

// ============================================================
// Shape ids ("MACRO-counted" def_shape order; see levels.c SH_* block)
// Only the ids used by the ported levels + shared engine helpers are
// listed; follow-up level ports append their own ids here (keep the
// C names and values).
// ============================================================
pub mod sh {
    pub const NULLSHAPE: u16 = 0;
    pub const MYSHIP_4: u16 = 2; // hand-tuned builtin Arwing
    pub const GATE_0: u16 = 8;
    pub const KAMIKAZE: u16 = 10;
    pub const PILLAR3: u16 = 28;
    pub const BOM_WING: u16 = 49;
    pub const RADER_0: u16 = 51;
    pub const RADER_1: u16 = 52;
    pub const ZACO_6: u16 = 53;
    pub const ZACO_5: u16 = 54;
    pub const BOSS_7_1: u16 = 56;
    pub const TOWER_2: u16 = 59; // wireframe-only: not in the compiled catalog
    pub const BU_0: u16 = 61;
    pub const BU_1: u16 = 62;
    pub const BU_2: u16 = 63;
    pub const BU_3: u16 = 64;
    pub const BU_4: u16 = 65;
    pub const BU_5: u16 = 66;
    pub const BU_6: u16 = 67;
    pub const BU_7: u16 = 68;
    pub const BU_8: u16 = 69;
    pub const R_BU_1: u16 = 97;
    pub const R_BU_4: u16 = 100; // PLANET.ASM r_bu_4
    pub const R_BU_6: u16 = 102;
    pub const R_BU_7: u16 = 103;
    pub const ZACO_8: u16 = 105;
    pub const SHIPS: u16 = 111;
    pub const CARRIER: u16 = 115;
    pub const MYBASE_1: u16 = 125;
    pub const SHIP_S_0: u16 = 126;
    pub const SHIP_S_1: u16 = 127;
    pub const XWIRESPACEBAR: u16 = 137;
    pub const XPWIRESPACEBAR: u16 = 138;
    pub const SXPWIRESPACEBAR: u16 = 139;
    pub const YWIRESPACEBAR: u16 = 140;
    pub const ZWIRESPACEBAR: u16 = 141;
    pub const SXWIRESPACEBAR: u16 = 142;
    pub const SYWIRESPACEBAR: u16 = 143;
    pub const SZWIRESPACEBAR: u16 = 144;
    pub const XSOLIDSPACEBAR: u16 = 145;
    pub const XPSOLIDSPACEBAR: u16 = 146;
    pub const SXPSOLIDSPACEBAR: u16 = 147;
    pub const YSOLIDSPACEBAR: u16 = 148;
    pub const ZSOLIDSPACEBAR: u16 = 149;
    pub const SXSOLIDSPACEBAR: u16 = 150;
    pub const SYSOLIDSPACEBAR: u16 = 151;
    pub const SZSOLIDSPACEBAR: u16 = 152;
    pub const ITEM_5: u16 = 159;
    pub const ITEM_7: u16 = 161;
    pub const WALKER_2: u16 = 165;
    pub const ROBOT_0: u16 = 170; // ro_0
    pub const STALK: u16 = 209;
    pub const S_FISH: u16 = 212;
    pub const ZACO_A: u16 = 218;
    pub const FRIENDSHIP_4: u16 = 219;
    pub const ARCH_0: u16 = 229;
    pub const BIG_GATE: u16 = 234;
    pub const TOW_0: u16 = 248;
    pub const MYBASE_0: u16 = 256; // SHAPE_EXT_MYBASE_0 (extended shape catalog)
    pub const OP_0: u16 = 551;
    pub const OP_1: u16 = 552;
    pub const OP_2: u16 = 553;
    pub const IMYSHIP_4: u16 = 554;

    // TITLE.ASM: `my_demo` is the title screen ship. Proxy through myship_4.
    pub const MY_DEMO_PROXY: u16 = MYSHIP_4;
}

// ============================================================
// Strategy ids (ISTRATS.ASM order; levels.c IS_* block) + synthetic
// strategy addresses. Same append-only policy as `sh`.
// ============================================================
pub mod is {
    pub const NOCOLL: u32 = 10;
    pub const GND: u32 = 15;
    pub const CLSHIPGNDA: u32 = 19;
    pub const CLSHIPGNDB: u32 = 20;
    pub const CLSHIPGNDC: u32 = 21;
    pub const GATE: u32 = 53;
    pub const PILLAR3: u32 = 79;
    pub const BOMWING: u32 = 89;
    pub const RADER0: u32 = 92;
    pub const RADER1: u32 = 93;
    pub const ZACOS: u32 = 94;
    pub const ZACO1L: u32 = 95;
    pub const ZACO1R: u32 = 96;
    pub const BOSS7: u32 = 99;
    pub const TOWER0: u32 = 101;
    pub const ZACO4: u32 = 103;
    pub const HARD180YR: u32 = 105;
    pub const HARD90YR: u32 = 127;
    pub const SZACO2: u32 = 129;
    pub const SHIPS: u32 = 133;
    pub const FRIENDEXITBASE: u32 = 152;
    pub const PATH: u32 = 157;
    pub const ITEM5: u32 = 175;
    pub const ITEM7: u32 = 177;
    pub const LOCHNESSMONSTER: u32 = 198;
    pub const TREE3: u32 = 206;
    pub const SFISH: u32 = 209;
    pub const HARDROT: u32 = 210;
    pub const HARD: u32 = 226;
    pub const SHIPINTRO: u32 = 239;
    pub const SKILLFLY: u32 = 241;
    pub const PATHDHA: u32 = 243;
    pub const SPACEBARSHOOT: u32 = 174;
}

// Synthetic strategy addresses (levels.c STRAT_ADDR_* subset).
// `gate_Istrat` in map data contexts resolves to the flat IS_GATE id
// (levels.c redefines STRAT_ADDR_GATE3 to IS_GATE after including
// strat_table.h, so the value baked into bytecode is 53).
pub const STRAT_ADDR_GATE3: u32 = is::GATE;
// `tit_istrat` is the title screen strategy (TITLE.ASM).
pub const STRAT_ADDR_TIT: u32 = 0x050020;

// Mother-system strategy addresses (registered by sf-strat::mother).
// NOTE: the old values 0x020000/0x020001 collided with the synthetic
// istrat forms of istrats 0/1 (the PLAYER init strat and pbody), so mother
// objects resolved to the player strategy and the mothermap never ran.
// 0x0300xx is the non-istrat symbol space (0x030001-0x030007 are already
// claimed by TOW0EXPLODE/GATE3/PLAYER_EXITBASE/SPACEPILON/BOSSSEAMON/
// BOSSG/SHIP0CDOWN).
pub const STRAT_ADDR_MOTHER1: u32 = 0x030008; // mother1_istrat (D2STRATS.ASM:501)
pub const STRAT_ADDR_MOTHER2: u32 = 0x030009; // mother2_istrat (D2STRATS.ASM:524)
pub const STRAT_ADDR_METEOR: u32 = 0x03000A; // meteor_istrat (DSTRATS.ASM:1215)
// slowmeteor's old id 0x030003 collided with STRAT_ADDR_PLAYER_EXITBASE.
pub const STRAT_ADDR_SLOWMETEOR: u32 = 0x03000B; // slowmeteor_istrat (DSTRATS.ASM:1199)
pub const STRAT_ADDR_SEARCHMETEOR: u32 = 0x03000C; // searchmeteor_istrat (DSTRATS.ASM:1167)
pub const STRAT_ADDR_CLASTEROID: u32 = 0x03000D; // clasteroid_Istrat (GA2STRAT.ASM:3357)
// Reserved (referenced by mothermap data; strategies not yet ported —
// children spawn inert until their lane lands them).
pub const STRAT_ADDR_SEADRAGON: u32 = 0x03000E; // seadragon_istrat (DSTRATS.ASM:1934)
pub const STRAT_ADDR_DAMYSCR: u32 = 0x03000F; // damyscr_istrat (DSTRATS.ASM:8476)

// ============================================================
// Path ids (src/path/path_literals.h PATH_ID_* subset)
// ============================================================
pub mod path {
    pub const E_GATE: u16 = 0;
    pub const PONPON: u16 = 44;
    pub const MATEMSG: u16 = 91;
    pub const FROG1_1: u16 = 144;
    pub const FALCO_LV1: u16 = 148;
    pub const FROG_LV1: u16 = 149;
    pub const TOW_0: u16 = 160;
    pub const ROBOT: u16 = 223;
    pub const ROBOTSWITHLOG: u16 = 224;
    pub const ROBOTWITHLOG2: u16 = 225;
    pub const KORORI: u16 = 228;
    pub const CHASE8_1: u16 = 229;
    pub const CHASE8_2: u16 = 230;
    pub const CHASE8_3: u16 = 231;
    pub const PATROL: u16 = 232;
    pub const ROBOTWITHLOG: u16 = 233;
    pub const CHASE6_1: u16 = 234;
    pub const CHASE6_2: u16 = 235;
    pub const E_UFO: u16 = 240;
}

// ============================================================
// SNES alien-struct field offsets used by MAPMACS setalvar (levels.c)
// ============================================================
pub mod al {
    pub const WORLDX: u16 = 12;
    pub const PTR: u16 = 6;
    pub const ROTX: u16 = 18;
    pub const ROTY: u16 = 19;
    pub const ROTZ: u16 = 20;
    pub const VEL: u16 = 21;
    pub const SBYTE1: u16 = 34;
    pub const SBYTE2: u16 = 35;
    pub const SBYTE3: u16 = 36;
    pub const SWORD1: u16 = 38;
    pub const SWORD2: u16 = 40;
    pub const HP: u16 = 42;
    pub const AP: u16 = 43;
}

pub mod alx {
    pub const SWPX1: u16 = 0;
    pub const SWPY1: u16 = 2;
    pub const DEPTHOFFSET: u16 = 21;
    pub const PWORD1: u16 = 52;
}

// ============================================================
// External vars mirrored into flat WRAM for map opcodes (levels.c WM_*)
// ============================================================
pub mod wm {
    pub const MAPVAR1: u16 = 0x0320;
    pub const SKILLFLY: u16 = 0x0304;
    pub const STAGECLEAR: u16 = 0x0305;
    pub const CLB2: u16 = 0x0306;
    pub const LEVELFINISHED: u16 = 0x0307;
    pub const ONECREDSPR: u16 = 0x0308;
    pub const INFOG: u16 = 0x0309;
    pub const FADEPAL: u16 = 0x030A;
    pub const PALFROM: u16 = 0x030B;
    pub const PALTO: u16 = 0x030C;
    pub const PALLEN: u16 = 0x030D;
    pub const PLAYERPOSX: u16 = 0x030E; // 2 bytes
    pub const GSVAR_BYTE1: u16 = 0x0310;
    pub const MAPTRIGGER: u16 = 0x0311;
    pub const NUMENDOK: u16 = 0x0312;
    pub const NUMPLASERS: u16 = 0x0313;
    pub const HPOSJMP: u16 = 0x0314; // 2 bytes
    pub const BOSSMAXHP: u16 = 0x0316; // 2 bytes
}

// Space-bar layout constants (levels.c).
pub const SPACE_VIEWCY: i32 = -60;
pub const SPACE_MINX: i32 = -240;
pub const SPACE_MAXX: i32 = 240;
pub const SPACEBAR_BASE_DIST: i32 = 3000;
pub const SPACEBAR_UNIT_LEN: i32 = 125;
