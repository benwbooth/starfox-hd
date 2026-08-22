//! Route-3 lane-local shared pieces.
//!
//! Everything here mirrors `src/map/levels.c` byte-for-byte:
//! - constants (SH_/IS_/PATH_ID_/... flat C names) — values extracted from
//!   the C oracle by the r3 fixture harness. DUPLICATE: consolidate — a
//!   subset overlaps `crate::consts`; kept lane-local so route lanes never
//!   edit shared files concurrently.
//! - `Route3Ext` — mb_* emitters that exist in C but not yet in the shared
//!   `MapBuilder` (marked for consolidation into builder.rs).
//! - shared submap copies (CL_* clear demos, MAP1_1A, FINALMAP content).

pub(crate) use crate::builder::BarShapeMode;
use crate::builder::MapBuilder;
pub(crate) use crate::consts::op;

// ============================================================
// Constants (values dumped from the C oracle; see r3 fixtures harness)
// ============================================================
pub const ALX_DEPTHOFFSET: u16 = 21;
pub const ALX_PWORD1: u16 = 52;
pub const AL_PTR: u16 = 6;
pub const AL_ROTX: u16 = 18;
pub const AL_ROTY: u16 = 19;
pub const AL_ROTZ: u16 = 20;
pub const AL_SBYTE1: u16 = 34;
pub const AL_SBYTE2: u16 = 35;
pub const AL_SWORD1: u16 = 38;
pub const AL_VEL: u16 = 21;
pub const AL_WORLDX: u16 = 12;
pub const BGM_BOSS1: i32 = 5;
pub const BGM_BOSS_FINAL: i32 = 19;
pub const BGM_FADEOUT: i32 = 241;
pub const BGM_FANFARE: i32 = 7;
pub const BGM_FINAL_CONT: i32 = 18;
pub const BG_1_3B: i32 = 8;
pub const BG_1_3E: i32 = 12;
pub const BG_1_6C: i32 = 17;
pub const BG_2_6C: i32 = 29;
pub const BG_3_1C: i32 = 3;
pub const BG_3_4C: i32 = 34;
pub const BG_3_4D: i32 = 35;
pub const BOSS8_CIRC: i32 = 1680;
pub const BOSS8_SCALE: i32 = 3;
pub const BOSSA_SCALE: i32 = 2;
pub const CL_GND_FRIENDWAIT: i32 = 1950;
pub const DEG135: i32 = 96;
pub const DEG180: i32 = 128;
pub const DEG22: i32 = 16;
pub const DEG270: i32 = 192;
pub const DEG45: i32 = 32;
pub const DEG90: i32 = 64;
pub const DIR_EAST: i32 = 192;
pub const DIR_SOUTH: i32 = 128;
pub const IS_BASE_1: u32 = 229;
pub const IS_BAZOOKAL: u32 = 157;
pub const IS_BAZOOKAR: u32 = 158;
pub const IS_BIG_METEOR: u32 = 233;
pub const IS_BOMWING: u32 = 88;
pub const IS_BOSS2: u32 = 107;
pub const IS_BOSSA: u32 = 84;
pub const IS_BREAK_METEOR: u32 = 234;
pub const IS_BREAK_METEORT: u32 = 237;
pub const IS_CAMELEON: u32 = 62;
pub const IS_CHICKEN: u32 = 116;
pub const IS_CLSHIPCHASEA: u32 = 33;
pub const IS_CLSHIPCHASEB: u32 = 34;
pub const IS_CLSHIPCHASEC: u32 = 35;
pub const IS_CLSHIPDIVEA: u32 = 36;
pub const IS_CLSHIPDIVEB: u32 = 37;
pub const IS_CLSHIPDIVEC: u32 = 38;
pub const IS_CLSHIPGNDA: u32 = 15;
pub const IS_CLSHIPGNDB: u32 = 16;
pub const IS_CLSHIPGNDC: u32 = 17;
pub const IS_CLSHIPSHIPA: u32 = 21;
pub const IS_CLSHIPSHIPB: u32 = 22;
pub const IS_CLSHIPSHIPC: u32 = 23;
pub const IS_CLSHIPUNDERA: u32 = 39;
pub const IS_CLSHIPUNDERB: u32 = 40;
pub const IS_CLSHIPUNDERC: u32 = 41;
pub const IS_COLONY0: u32 = 169;
pub const IS_COLONY1: u32 = 170;
pub const IS_COLONY2: u32 = 171;
pub const IS_FIREPILLAR: u32 = 193;
pub const IS_FLYPILLARS: u32 = 73;
pub const IS_FRIENDEXITBASE: u32 = 151;
pub const IS_GATE: u32 = 52;
pub const IS_GND: u32 = 11;
pub const IS_HARD: u32 = 225;
pub const IS_HARD180YR: u32 = 104;
pub const IS_HOUDAI: u32 = 96;
pub const IS_HOUDAI5F: u32 = 187;
pub const IS_HOUDAINS: u32 = 97;
pub const IS_ITEM5: u32 = 174;
pub const IS_ITEM6: u32 = 175;
pub const IS_ITEM7: u32 = 176;
pub const IS_METEO0: u32 = 194;
pub const IS_MISSPOD: u32 = 67;
pub const IS_NOCOLL: u32 = 10;
pub const IS_PILLAR3: u32 = 78;
pub const IS_RADER0: u32 = 91;
pub const IS_RADER1: u32 = 92;
pub const IS_ROCKHARD: u32 = 192;
pub const IS_SEADRAGON2: u32 = 196;
pub const IS_SEAMON: u32 = 80;
pub const IS_SHIPINTRO: u32 = 238;
pub const IS_SHOU0: u32 = 177;
pub const IS_SHOU0A: u32 = 178;
pub const IS_SPACEBARWALKER: u32 = 172;
pub const IS_TORPEDO: u32 = 79;
pub const IS_TOWER0: u32 = 100;
pub const IS_TRACKCORNER: u32 = 49;
pub const IS_TREE1: u32 = 203;
pub const IS_TREE2: u32 = 204;
pub const IS_TRUCK: u32 = 48;
pub const IS_UP1MAN: u32 = 89;
pub const IS_UPERM: u32 = 159;
pub const IS_VOLCANO: u32 = 191;
pub const IS_WALKING: u32 = 77;
pub const IS_WALLL: u32 = 75; // ISTRATS.ASM def_istrat row 76 (walll)
pub const IS_WALLLEFTRIGHT: u32 = 74; // row 75 (wallleftright)
pub const IS_WALLR: u32 = 76; // row 77 (wallr). Were all 105=hard180yr.
pub const IS_WEBMONSTER: u32 = 122;
pub const IS_WINDMILL: u32 = 66;
pub const IS_WINGLAZERMAN: u32 = 90;
pub const IS_WOODS: u32 = 53;
pub const IS_WORM: u32 = 60;
pub const IS_WORMHEAD: u32 = 51;
pub const IS_ZACO0: u32 = 101;
pub const IS_ZACO1L: u32 = 94;
pub const IS_ZACO1R: u32 = 95;
pub const IS_ZACO4: u32 = 102;
pub const IS_ZACOS: u32 = 93;
pub const MAP37_CLEN: i32 = 125;
pub const MAP_BARSHAPE_SOLID: i32 = 1;
pub const MAP_BARSHAPE_WIRE: i32 = 0;
pub const MAP_CB_BLOCKSND_L: u32 = 66818;
pub const MAP_CB_BUNNY_ALIVE: u32 = 66050;
pub const MAP_CB_CHKBOSSDEAD: u32 = 65540;
pub const MAP_CB_CHKSTAGEDONE: u32 = 65537;
pub const MAP_CB_CHKSTRATDONE1: u32 = 65538;
pub const MAP_CB_CLEARREALOBJMAP_L: u32 = 65798;
pub const MAP_CB_CLFRIENDMSG_BUNNY: u32 = 66066;
pub const MAP_CB_CLFRIENDMSG_COCK: u32 = 66067;
pub const MAP_CB_CLFRIENDMSG_FROG: u32 = 66065;
pub const MAP_CB_CL_DIVE_CLEAR_ENGINESND: u32 = 127236;
pub const MAP_CB_CL_GROUND_PRINTLEVELFIN: u32 = 127233;
pub const MAP_CB_CL_GROUND_WIPEOUT: u32 = 127234;
pub const MAP_CB_COCK_ALIVE: u32 = 66051;
pub const MAP_CB_FROG_ALIVE: u32 = 66049;
pub const MAP_CB_INITBLACK_L: u32 = 65793;
pub const MAP_CB_IS_PLAYER_DEAD: u32 = 66561;
pub const MAP_CB_MARKBOSS_L: u32 = 65800;
pub const MAP_CB_SETRESTART_L: u32 = 65799;
pub const MAP_CB_SET_PLAYER_CLEARDEMO_L: u32 = 66307;
pub const MAP_CB_SET_PLAYER_CLEAR_CHASE_L: u32 = 66310;
pub const MAP_CB_SET_PLAYER_CLEAR_SHIP2_L: u32 = 66311;
pub const MAP_CB_SET_PLAYER_CLEAR_UNDER_L: u32 = 66312;
pub const MAP_CB_SET_PLAYER_DIVE_L: u32 = 66313;
pub const MAP_CB_SET_PLAYER_EXITBASE_L: u32 = 66305;
pub const MAP_CB_SET_PLAYER_ONPLANET_L: u32 = 66306;
pub const MAP_CB_SET_PLAYER_ONWATER_L: u32 = 66317;
pub const MAP_ID_1_5: i32 = 5;
pub const MAP_ID_1_6: i32 = 6;
pub const MAP_ID_3_5: i32 = 17;
pub const MAP_ID_3_7: i32 = 19;
pub const MAP_ID_FINAL: i32 = 22;
pub const MAP_ID_INTRO: i32 = 23;
pub const MAP_ID_SPECIAL: i32 = 21;
pub const MAP_OP_END: i32 = 2;
pub const MEDPSPEED: i32 = 65;
pub const MYBASE_SCALE: i32 = 3;
pub const NUCLEUSHEIGHT: i32 = 100;
pub const PATH_ID_AMEBMSG2: u16 = 329;
pub const PATH_ID_ASTEMSG: u16 = 241;
pub const PATH_ID_BIRD_METEOR: u16 = 9;
pub const PATH_ID_CALL_FOL: u16 = 341;
pub const PATH_ID_CHASE1_1: u16 = 247;
pub const PATH_ID_CHASE1_2: u16 = 248;
pub const PATH_ID_CHASE2_1: u16 = 269;
pub const PATH_ID_CHASE2_2: u16 = 270;
pub const PATH_ID_CHASE3_1: u16 = 271;
pub const PATH_ID_CHASE3_2: u16 = 272;
pub const PATH_ID_CHASE5_1: u16 = 305;
pub const PATH_ID_CHASE5_2: u16 = 306;
pub const PATH_ID_CHASE5_3: u16 = 307;
pub const PATH_ID_CHASE6_1: u16 = 234;
pub const PATH_ID_CHASE6_2: u16 = 235;
pub const PATH_ID_CHASE7_1: u16 = 243;
pub const PATH_ID_CHASE7_2: u16 = 244;
pub const PATH_ID_CHECK: u16 = 266;
pub const PATH_ID_DAMYSCR: u16 = 258;
pub const PATH_ID_DRAGONMSG: u16 = 335;
pub const PATH_ID_E_BEE: u16 = 330;
pub const PATH_ID_E_DOSUN: u16 = 356;
pub const PATH_ID_E_FLOWER: u16 = 1;
pub const PATH_ID_E_FLYFISH: u16 = 333;
pub const PATH_ID_E_GATE: u16 = 0;
pub const PATH_ID_E_KURURI: u16 = 358;
pub const PATH_ID_E_SHIELDR: u16 = 273;
pub const PATH_ID_E_TANK: u16 = 320;
pub const PATH_ID_E_WALK_1: u16 = 317;
pub const PATH_ID_FALCON3_1: u16 = 265;
pub const PATH_ID_FALCO_LV1: u16 = 148;
pub const PATH_ID_FROG_LV1: u16 = 149;
pub const PATH_ID_ITACHI_A: u16 = 20;
pub const PATH_ID_ITACHI_B: u16 = 19;
pub const PATH_ID_ITADOSUN: u16 = 357;
pub const PATH_ID_KAMOME: u16 = 334;
pub const PATH_ID_MATEMSG: u16 = 91;
pub const PATH_ID_MES_ANDROSS1: u16 = 342;
pub const PATH_ID_MES_ANDROSS2: u16 = 343;
pub const PATH_ID_MY_BIRD: u16 = 245;
pub const PATH_ID_PATRET_IFAL: u16 = 261;
pub const PATH_ID_PATROL: u16 = 232;
pub const PATH_ID_PONPON: u16 = 44;
pub const PATH_ID_ROBOT: u16 = 223;
pub const PATH_ID_ROBOTSWITHLOG: u16 = 224;
pub const PATH_ID_ROBOTWITHLOG: u16 = 233;
pub const PATH_ID_SCREW: u16 = 257;
pub const PATH_ID_TOMHAHA: u16 = 332;
pub const PATH_ID_TOMSET: u16 = 331;
pub const PATH_ID_TOW_0: u16 = 160;
pub const PEXITBASE_SPEED: i32 = 50;
pub const ROTNUM_WASH: i32 = 8;
pub const ROTSIZE_WASH: i32 = 32;
pub const SH_ARCH_0: u16 = 228;
pub const SH_ASTEROID1_PROXY: u16 = 275;
pub const SH_ASTEROID2: u16 = 194;
pub const SH_BASE_1: u16 = 232;
pub const SH_BAZOOKA: u16 = 131;
pub const SH_BEEANIM: u16 = 16;
pub const SH_BIG_GATE: u16 = 233;
pub const SH_BIG_M: u16 = 18;
pub const SH_BIG_METEOR_PROXY: u16 = 237;
pub const SH_BOM_WING: u16 = 48;
pub const SH_BOSS_0_1: u16 = 84;
pub const SH_BOSS_2_2_PROXY: u16 = 69;
pub const SH_BOSS_8_0_PROXY: u16 = 46;
pub const SH_BOSS_8_4_PROXY: u16 = 45;
pub const SH_BOSS_A_2: u16 = 57;
pub const SH_BOSS_D_1: u16 = 77;
pub const SH_BOSS_D_4: u16 = 238;
pub const SH_BOSS_F_3_PROXY: u16 = 81;
/// `boss_f_4` (SHAPES3/ISTRATS shape slot 95).  This used to be a nullshape
/// proxy even though the mesh is present in the generated Rust shape table.
pub const SH_BOSS_F_4_PROXY: u16 = 94;
pub const SH_BOU_0_PROXY: u16 = 248;
pub const SH_BOU_1_PROXY: u16 = 284;
pub const SH_BU_0: u16 = 60;
pub const SH_BU_1: u16 = 61;
pub const SH_BU_2: u16 = 62;
pub const SH_BU_3: u16 = 63;
pub const SH_BU_6: u16 = 66;
pub const SH_BU_7: u16 = 67;
pub const SH_BU_8: u16 = 68;
pub const SH_BWARKER_3: u16 = 157;
pub const SH_BZACO_8: u16 = 231;
pub const SH_B_HOU_0: u16 = 163;
pub const SH_CAMELEON: u16 = 15;
pub const SH_COLONY3L: u16 = 155;
pub const SH_COLONY3R: u16 = 156;
pub const SH_COLONY_0: u16 = 152;
pub const SH_COLONY_0_PROXY: u16 = 152;
pub const SH_COLONY_1: u16 = 153;
pub const SH_COLONY_2: u16 = 154;
pub const SH_D_BODY_0: u16 = 14;
pub const SH_D_HEAD_0: u16 = 13;
pub const SH_D_PILAR: u16 = 320;
pub const SH_FACE_B_PROXY: u16 = 223;
pub const SH_FLOWER_1: u16 = 206;
pub const SH_FLOWER_2: u16 = 207;
pub const SH_FRIENDSHIP_4: u16 = 218;
pub const SH_F_FISH_PROXY: u16 = 271;
pub const SH_GATE_0: u16 = 7;
pub const SH_HOUDAI_0: u16 = 54;
pub const SH_HALF_D: u16 = 321;
pub const SH_HOU_4_PROXY: u16 = 44;
pub const SH_HOU_5: u16 = 168;
pub const SH_IMYSHIP_4: u16 = 554;
pub const SH_ITEM_5: u16 = 158;
pub const SH_ITEM_6: u16 = 159;
pub const SH_ITEM_7: u16 = 160;
pub const SH_KAMIKAZE: u16 = 9;
pub const SH_METEO_0: u16 = 192;
pub const SH_MISS_1_1: u16 = 36;
pub const SH_MISS_1_2: u16 = 8;
pub const SH_MOTHER1: u16 = 278;
pub const SH_MYBASE_0: u16 = 256;
pub const SH_MYBASE_1: u16 = 124;
pub const SH_MYSHIP_4: u16 = 2;
pub const SH_MY_BIRD: u16 = 557;
pub const SH_NULLSHAPE: u16 = 0;
pub const SH_OPEN_L_PROXY: u16 = 235;
pub const SH_OP_0: u16 = 551;
pub const SH_OP_1: u16 = 552;
pub const SH_OP_2: u16 = 553;
pub const SH_PILLAR3: u16 = 27;
pub const SH_POLE_0_PROXY: u16 = 290;
pub const SH_RADER_0: u16 = 50;
pub const SH_RADER_1: u16 = 51;
pub const SH_RAIL_0: u16 = 35;
pub const SH_RAIL_4: u16 = 5;
pub const SH_RAW_BOSS_7_0: i32 = 421;
pub const SH_RAW_BOSS_7_1: i32 = 55;
pub const SH_RAW_BOSS_7_3: i32 = 424;
pub const SH_ROBOT_0: u16 = 420;
pub const SH_ROUND_0: u16 = 17;
pub const SH_RO_0_PROXY: u16 = 169;
pub const SH_RO_1_PROXY: u16 = 170;
pub const SH_RO_2_PROXY: u16 = 171;
pub const SH_RO_3_PROXY: u16 = 172;
pub const SH_RO_4_PROXY: u16 = 173;
pub const SH_RO_5_PROXY: u16 = 174;
pub const SH_RO_6_PROXY: u16 = 175;
pub const SH_RPILLAR3_PROXY: u16 = 439;
pub const SH_R_BU_1: u16 = 96;
pub const SH_R_BU_2: u16 = 97;
pub const SH_R_BU_4: u16 = 99;
pub const SH_R_BU_6: u16 = 101;
pub const SH_R_BU_7: u16 = 102;
pub const SH_R_HOU_0: u16 = 161;
pub const SH_SEA_0_0: u16 = 31;
pub const SH_SHARK: u16 = 12;
pub const SH_SHIELDR: u16 = 202;
pub const SH_SNAKE_1: u16 = 200;
pub const SH_SPACEPILON: u16 = 614;
pub const SH_SSHIP_0_C_PROXY: u16 = 23;
pub const SH_STALK: u16 = 208;
pub const SH_SVOLCANO_PROXY: u16 = 191;
pub const SH_S_HOU_0: u16 = 162;
pub const SH_S_TANK_0: u16 = 229;
pub const SH_S_WARK_0: u16 = 219;
pub const SH_TANK_1: u16 = 167;
pub const SH_TOWER_2: u16 = 58;
pub const SH_TOW_0: u16 = 247;
pub const SH_TRUCK: u16 = 4;
pub const SH_TUNNEL_0: u16 = 121;
pub const SH_UPER_M: u16 = 132;
pub const SH_UP_DOOR_PROXY: u16 = 236;
pub const SH_VOLCANO_PROXY: u16 = 190;
pub const SH_WALKER_0: u16 = 26;
pub const SH_WALL_0_PROXY: u16 = 86;
pub const SH_WALL_1_PROXY: u16 = 87;
pub const SH_WALL_2: u16 = 88;
pub const SH_WALL_4_PROXY: u16 = 90;
pub const SH_WARKER_3_PROXY: u16 = 129;
pub const SH_WARP_PROXY: u16 = 133;
pub const SH_W_L: u16 = 49;
pub const SH_ZACO_5: u16 = 53;
pub const SH_ZACO_6: u16 = 52;
pub const SH_ZACO_8: u16 = 104;
pub const SH_ZACO_A: u16 = 217;
pub const SH_ZACO_B: u16 = 201;
pub const SPACEBAR_BASE_DIST: i32 = 3000;
pub const SPACEBAR_UNIT_LEN: i32 = 125;
pub const SPACE_VIEWCY: i32 = -60;
pub const STRAT_ADDR_AIRSHIP: u32 = 125;
pub const STRAT_ADDR_BOSS8: u32 = crate::consts::is::BOSS8;
pub const STRAT_ADDR_BOSSF: u32 = crate::consts::is::BOSSF;
pub const STRAT_ADDR_BOTLEFT1: u32 = 147;
pub const STRAT_ADDR_BOTRIGHT1: u32 = 146;
pub const STRAT_ADDR_GATE3: u32 = crate::consts::is::GATE;
pub const STRAT_ADDR_MONOLITH: u32 = 215;
/// `dpilar_Istrat` and `halfd_Istrat` are the same ROM entry point.
pub const STRATEGY_HALFDPILAR: crate::consts::DirectStrategy =
    crate::consts::DirectStrategy::HalfDPillar;
// (The old 131072/131073 = 131072/131073 values collided with synth
// istrats 0/1 — mothers ran the player strategy.)
pub const STRATEGY_MOTHER1: crate::consts::DirectStrategy = crate::consts::STRATEGY_MOTHER1;
pub const STRATEGY_MOTHER2: crate::consts::DirectStrategy = crate::consts::STRATEGY_MOTHER2;
pub const STRAT_ADDR_NUCLEUSLAUNCHER: u32 = crate::consts::is::NUCLEUSLAUNCHER;
pub const STRAT_ADDR_NUCLEUSPILLAR: u32 = crate::consts::is::NUCLEUSPILLAR;
pub const STRAT_ADDR_OPENLR: u32 = 231;
pub const STRATEGY_POLE0: crate::consts::DirectStrategy = crate::consts::DirectStrategy::Pole0;
pub const STRAT_ADDR_SHIP0CDOWN: u32 = crate::consts::is::SHIP0CDOWN;
// Keep this wired to the shared non-istrat address.  The historical literal
// 196611 (196611) is now STRAT_ADDR_PLAYER_EXITBASE; using it here made
// asteroid1 objects run the player exit-base/death strategy and killed the
// real player when the asteroids were culled behind the camera.
pub const STRATEGY_SLOWMETEOR: crate::consts::DirectStrategy = crate::consts::STRATEGY_SLOWMETEOR;
pub const STRATEGY_SPACEPILON: crate::consts::DirectStrategy =
    crate::consts::DirectStrategy::SpacePilon;
pub const STRAT_ADDR_TOPLEFT1: u32 = 145;
pub const STRAT_ADDR_TOPRIGHT1: u32 = 144;
pub const STRAT_ADDR_TWALL0: u32 = 163;
pub const STRAT_ADDR_UPDOOR: u32 = 232;
pub const STRAT_ADDR_WARKER3: u32 = 154;
pub const STRAT_ADDR_WARP: u32 = crate::consts::is::WARP;
pub const TSIZE: i32 = 200;
pub const WM_BOSSMAXHP: u16 = 790;
pub const WM_CLB2: u16 = 774;
pub const WM_MAPVAR1: u16 = 800;
pub const WM_STAGECLEAR: u16 = 773;

// ============================================================
// mb_* emitters missing from the shared MapBuilder.
// TODO(consolidation): move into `crate::builder::MapBuilder` once the
// builder file is no longer contended between route lanes.
// ============================================================
pub(crate) trait Route3Ext {
    /// C `mb_ttruck`: train truck mapobj at track grid (tx,tz) facing ta.
    fn ttruck(&mut self, tx: i32, tz: i32, ta: i32);
    /// C `mb_thoriz`: horizontal rail at track grid (tx,tz).
    fn thoriz(&mut self, tx: i32, tz: i32);
    /// C `mb_tvert`: vertical rail at track grid (tx,tz).
    fn tvert(&mut self, tx: i32, tz: i32);
    /// C `mb_tcorner`: corner rail at track grid (tx,tz) facing ta, turning dir.
    fn tcorner(&mut self, tx: i32, tz: i32, ta: i32, dir: i32);
}

impl Route3Ext for MapBuilder {
    fn ttruck(&mut self, tx: i32, tz: i32, ta: i32) {
        self.mapobj(
            0,
            -500 + TSIZE * tx,
            0,
            4096 + tz * TSIZE,
            SH_TRUCK,
            IS_TRUCK,
        );
        self.setalvarb(AL_ROTY, ta);
    }

    fn thoriz(&mut self, tx: i32, tz: i32) {
        self.mapobj(
            0,
            -500 + TSIZE * tx,
            0,
            4096 + tz * TSIZE,
            SH_RAIL_0,
            IS_NOCOLL,
        );
    }

    fn tvert(&mut self, tx: i32, tz: i32) {
        self.mapobj(
            0,
            -500 + TSIZE * tx,
            0,
            4096 + tz * TSIZE,
            SH_RAIL_0,
            IS_NOCOLL,
        );
        self.setalvarb(AL_ROTY, DEG90);
    }

    fn tcorner(&mut self, tx: i32, tz: i32, ta: i32, dir: i32) {
        self.mapobj(
            0,
            -500 + TSIZE * tx,
            0,
            4096 + tz * TSIZE,
            SH_RAIL_4,
            IS_TRACKCORNER,
        );
        self.setalvarb(AL_ROTY, ta);
        self.setalvarb(AL_SBYTE1, dir);
    }
}

// ============================================================
// Shared submap copies (DUPLICATE: consolidate)
// ============================================================
/// C `append_cl_chase_submap()`.
/// DUPLICATE: consolidate — literal copy of the shared levels.c submap
/// (other route lanes carry their own copies until a shared home exists).
pub(crate) fn append_cl_chase_submap(b: &mut MapBuilder) {
    // CL_CHASE.ASM shared clear-demo helper for LEVEL3_2.
    b.label("cl_chase");
    // mother_CLasteroids (CL_CHASE.ASM:2 / CL_SHIP.ASM:42).
    b.mapmother(
        0,
        0,
        0,
        3000,
        SH_MOTHER1,
        STRATEGY_MOTHER1,
        crate::mothers::mother_maps().mother_clasteroids,
    );
    b.mapplayeroutview();
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_CLEAR_CHASE_L);
    b.setbgm(BGM_FANFARE);
    b.mapwait(3800);

    b.setvarb(WM_STAGECLEAR, 1);
    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(MAP_CB_FROG_ALIVE, "cl_chase.frog_alive");
    b.mapgoto("cl_chase.nf");
    b.label("cl_chase.frog_alive");
    b.mapobj(
        CL_GND_FRIENDWAIT,
        1000,
        -300,
        50,
        SH_MYSHIP_4,
        IS_CLSHIPCHASEA,
    );
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_FROG);
    b.label("cl_chase.nf");

    b.mapif_builtin(MAP_CB_BUNNY_ALIVE, "cl_chase.bunny_alive");
    b.mapgoto("cl_chase.nb");
    b.label("cl_chase.bunny_alive");
    b.mapobj(
        CL_GND_FRIENDWAIT,
        -2000,
        -300,
        50,
        SH_MYSHIP_4,
        IS_CLSHIPCHASEB,
    );
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_BUNNY);
    b.label("cl_chase.nb");

    b.mapif_builtin(MAP_CB_COCK_ALIVE, "cl_chase.cock_alive");
    b.mapgoto("cl_chase.nc");
    b.label("cl_chase.cock_alive");
    b.mapobj(CL_GND_FRIENDWAIT, 0, 0, -2000, SH_MYSHIP_4, IS_CLSHIPCHASEC);
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_COCK);
    b.label("cl_chase.nc");

    b.mapwait(7000);
    b.setvarb(WM_CLB2, 0);
    b.setvarb(WM_STAGECLEAR, 0);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_PRINTLEVELFIN);
    b.label("cl_chase.eswait");
    b.mapwait(1);
    b.maploop("cl_chase.eswait", 100);
    b.maprts();

    // ----------------------------------------------------------------
    // CL_SHIP.ASM — clear demo for route-1 ship levels (3_4 and 1_3).
    // Two entry points: cl_ship3_4 (colony bg) and cl_ship1_3 (Sship bg).
    // Both jump to shared cl_ship_cont.
    // ----------------------------------------------------------------
}

/// C `append_cl_ship_submap()`.
/// DUPLICATE: consolidate — literal copy of the shared levels.c submap
/// (other route lanes carry their own copies until a shared home exists).
pub(crate) fn append_cl_ship_submap(b: &mut MapBuilder) {
    // cl_ship3_4 entry point
    b.label("cl_ship3_4");
    b.setbg(BG_3_4D);
    b.initbg();
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_CLEAR_SHIP2_L);
    b.setbgm(BGM_FANFARE);
    b.mapobj(
        0,
        0,
        SPACE_VIEWCY,
        0,
        SH_COLONY_0_PROXY,
        STRAT_ADDR_SHIP0CDOWN,
    );
    b.setalvarb(AL_ROTY, DEG180);
    b.mapgoto("cl_ship.cont");

    // cl_ship1_3 entry point
    b.label("cl_ship1_3");
    b.setbg(BG_1_3E);
    b.initbg();
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_CLEAR_SHIP2_L);
    b.setbgm(BGM_FANFARE);
    b.mapobj(
        0,
        0,
        SPACE_VIEWCY,
        0,
        SH_SSHIP_0_C_PROXY,
        STRAT_ADDR_SHIP0CDOWN,
    );

    // cl_ship_cont shared continuation
    b.label("cl_ship.cont");
    b.mapwait(9000 - CL_GND_FRIENDWAIT);
    // mother_CLasteroids (CL_CHASE.ASM:2 / CL_SHIP.ASM:42).
    b.mapmother(
        0,
        0,
        0,
        3000,
        SH_MOTHER1,
        STRATEGY_MOTHER1,
        crate::mothers::mother_maps().mother_clasteroids,
    );

    b.setvarb(WM_STAGECLEAR, 1);
    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(MAP_CB_FROG_ALIVE, "cl_ship.frog_alive");
    b.mapgoto("cl_ship.nf");
    b.label("cl_ship.frog_alive");
    b.mapobj(
        CL_GND_FRIENDWAIT,
        -1000,
        -50,
        50,
        SH_MYSHIP_4,
        IS_CLSHIPSHIPA,
    );
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_FROG);
    b.label("cl_ship.nf");

    b.mapif_builtin(MAP_CB_BUNNY_ALIVE, "cl_ship.bunny_alive");
    b.mapgoto("cl_ship.nb");
    b.label("cl_ship.bunny_alive");
    b.mapobj(
        CL_GND_FRIENDWAIT,
        1000,
        -50,
        50,
        SH_MYSHIP_4,
        IS_CLSHIPSHIPB,
    );
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_BUNNY);
    b.label("cl_ship.nb");

    b.mapif_builtin(MAP_CB_COCK_ALIVE, "cl_ship.cock_alive");
    b.mapgoto("cl_ship.nc");
    b.label("cl_ship.cock_alive");
    b.mapobj(CL_GND_FRIENDWAIT, 0, 200, -500, SH_MYSHIP_4, IS_CLSHIPSHIPC);
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_COCK);
    b.label("cl_ship.nc");

    b.mapwait(3000);
    b.setvarb(WM_CLB2, 0);
    b.setvarb(WM_STAGECLEAR, 0);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_PRINTLEVELFIN);
    b.label("cl_ship.sdloop");
    b.mapif_builtin(MAP_CB_CHKSTAGEDONE, "cl_ship.sdcont");
    b.mapgoto("cl_ship.sdloop");
    b.label("cl_ship.sdcont");
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_WIPEOUT);
    b.setbgm(BGM_FADEOUT);
    b.mapwait(45 * MEDPSPEED * 2);
    b.maprts();

    // ----------------------------------------------------------------
    // CL_UNDER.ASM — clear demo for underwater levels.
    // ----------------------------------------------------------------
}

/// C `append_cl_under_submap()`.
/// DUPLICATE: consolidate — literal copy of the shared levels.c submap
/// (other route lanes carry their own copies until a shared home exists).
pub(crate) fn append_cl_under_submap(b: &mut MapBuilder) {
    b.label("cl_under");
    b.mapplayeroutview();
    b.mapwait(1000);
    b.setbgm(BGM_FADEOUT);
    b.mapwait(2000);
    b.setbgm(BGM_FANFARE);
    b.mapwait(3000);

    b.setvarb(WM_STAGECLEAR, 1);
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_CLEAR_UNDER_L);

    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(MAP_CB_FROG_ALIVE, "cl_under.frog_alive");
    b.mapgoto("cl_under.nf");
    b.label("cl_under.frog_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_FROG);
    b.mapobj(
        CL_GND_FRIENDWAIT,
        1000,
        -300,
        50,
        SH_MYSHIP_4,
        IS_CLSHIPUNDERA,
    );
    b.label("cl_under.nf");

    b.mapif_builtin(MAP_CB_BUNNY_ALIVE, "cl_under.bunny_alive");
    b.mapgoto("cl_under.nb");
    b.label("cl_under.bunny_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_BUNNY);
    b.mapobj(
        CL_GND_FRIENDWAIT,
        -2000,
        -300,
        50,
        SH_MYSHIP_4,
        IS_CLSHIPUNDERB,
    );
    b.label("cl_under.nb");

    b.mapif_builtin(MAP_CB_COCK_ALIVE, "cl_under.cock_alive");
    b.mapgoto("cl_under.nc");
    b.label("cl_under.cock_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_COCK);
    b.mapobj(CL_GND_FRIENDWAIT, 0, 0, -2000, SH_MYSHIP_4, IS_CLSHIPUNDERC);
    b.label("cl_under.nc");

    b.mapwait(3800);
    b.setvarb(WM_CLB2, 0);
    b.setvarb(WM_STAGECLEAR, 0);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_PRINTLEVELFIN);
    b.label("cl_under.eswait");
    b.mapwait(1);
    b.maploop("cl_under.eswait", 100);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_WIPEOUT);
    b.setbgm(BGM_FADEOUT);
    b.mapwait(32 * MEDPSPEED);
    b.maprts();

    // ----------------------------------------------------------------
    // CL_DIVE.ASM — clear demo for dive levels.
    // Has inline 65816 to clear engine sound, handled via callback.
    // ----------------------------------------------------------------
}

/// C `append_cl_dive_submap()`.
/// DUPLICATE: consolidate — literal copy of the shared levels.c submap
/// (other route lanes carry their own copies until a shared home exists).
pub(crate) fn append_cl_dive_submap(b: &mut MapBuilder) {
    b.label("cl_dive");
    b.mapplayeroutview();
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_FANFARE);
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_DIVE_L);
    b.mapwait(2800);

    b.setvarb(WM_STAGECLEAR, 1);
    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(MAP_CB_FROG_ALIVE, "cl_dive.frog_alive");
    b.mapgoto("cl_dive.nf");
    b.label("cl_dive.frog_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_FROG);
    b.mapobj(
        CL_GND_FRIENDWAIT,
        200,
        SPACE_VIEWCY,
        50,
        SH_MYSHIP_4,
        IS_CLSHIPDIVEB,
    );
    b.label("cl_dive.nf");

    b.mapif_builtin(MAP_CB_BUNNY_ALIVE, "cl_dive.bunny_alive");
    b.mapgoto("cl_dive.nb");
    b.label("cl_dive.bunny_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_BUNNY);
    b.mapobj(
        CL_GND_FRIENDWAIT,
        -200,
        SPACE_VIEWCY,
        50,
        SH_MYSHIP_4,
        IS_CLSHIPDIVEA,
    );
    b.label("cl_dive.nb");

    b.mapif_builtin(MAP_CB_COCK_ALIVE, "cl_dive.cock_alive");
    b.mapgoto("cl_dive.nc");
    b.label("cl_dive.cock_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_COCK);
    b.mapobj(
        CL_GND_FRIENDWAIT,
        0,
        SPACE_VIEWCY - 40,
        -50,
        SH_MYSHIP_4,
        IS_CLSHIPDIVEC,
    );
    b.label("cl_dive.nc");

    b.mapwait(5000);
    b.setvarb(WM_CLB2, 0);
    b.setvarb(WM_STAGECLEAR, 0);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_PRINTLEVELFIN);
    b.label("cl_dive.eswait");
    b.mapwait(1);
    b.maploop("cl_dive.eswait", 100);

    // Inline 65816: clear engine sound flag (pshipflags3 &= ~psf3_enginesnd)
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_CL_DIVE_CLEAR_ENGINESND);
    b.qfadedown();
    b.waitfade();
    b.setvarb(WM_CLB2, 1);
    b.maprts();

    // ----------------------------------------------------------------
    // CL_BRIDG.ASM — clear demo for bridge levels.
    // ----------------------------------------------------------------
}

/// C `append_cl_ground_submap()`.
/// DUPLICATE: consolidate — literal copy of the shared levels.c submap
/// (other route lanes carry their own copies until a shared home exists).
pub(crate) fn append_cl_ground_submap(b: &mut MapBuilder) {
    b.label("cl_ground");
    b.setbgm(BGM_FADEOUT);
    b.mapwait(2000);
    b.setbgm(BGM_FANFARE);
    b.mapwait(3000);
    b.setvarb(WM_STAGECLEAR, 1);
    b.mapcodejsl_builtin(MAP_CB_SET_PLAYER_CLEARDEMO_L);

    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(MAP_CB_FROG_ALIVE, "cl_ground.frog_alive");
    b.mapgoto("cl_ground.nf");
    b.label("cl_ground.frog_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_FROG);
    b.mapobj(CL_GND_FRIENDWAIT, 500, -50, 50, SH_MYSHIP_4, IS_CLSHIPGNDB);
    b.label("cl_ground.nf");

    b.mapif_builtin(MAP_CB_BUNNY_ALIVE, "cl_ground.bunny_alive");
    b.mapgoto("cl_ground.nb");
    b.label("cl_ground.bunny_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_BUNNY);
    b.mapobj(CL_GND_FRIENDWAIT, -500, -50, 50, SH_MYSHIP_4, IS_CLSHIPGNDA);
    b.label("cl_ground.nb");

    b.mapif_builtin(MAP_CB_COCK_ALIVE, "cl_ground.cock_alive");
    b.mapgoto("cl_ground.nc");
    b.label("cl_ground.cock_alive");
    b.mapcodejsl_builtin(MAP_CB_CLFRIENDMSG_COCK);
    b.mapobj(CL_GND_FRIENDWAIT, 0, -500, -300, SH_MYSHIP_4, IS_CLSHIPGNDC);
    b.label("cl_ground.nc");

    b.mapwait(3800);
    b.setvarb(WM_CLB2, 0);
    b.setvarb(WM_STAGECLEAR, 0);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_PRINTLEVELFIN);
    b.label("cl_ground.eswait");
    b.mapwait(1);
    b.maploop("cl_ground.eswait", 100);
    b.mapcodejsl_builtin(MAP_CB_CL_GROUND_WIPEOUT);
    b.setbgm(BGM_FADEOUT);
    b.mapwait(32 * MEDPSPEED);
    b.setvarb(WM_CLB2, 1);
    b.maprts();
}

/// C `append_map1_1a_submap()`.
/// DUPLICATE: consolidate — literal copy of the shared levels.c submap
/// (other route lanes carry their own copies until a shared home exists).
pub(crate) fn append_map1_1a_submap(b: &mut MapBuilder) {
    b.label("map1_1a");
    b.mapobj(0, 0, 0, 250, SH_OP_0, IS_GND);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250, SH_OP_1, IS_NOCOLL);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (100 << 3), SH_OP_0, IS_GND);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (100 << 3), SH_OP_1, IS_NOCOLL);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (200 << 3), SH_OP_0, IS_GND);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (200 << 3), SH_OP_1, IS_NOCOLL);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);

    b.mapobj(0, -40, 0, -200, SH_IMYSHIP_4, IS_SHIPINTRO);
    b.setalvarw(AL_SWORD1, -70);
    b.setalvarb(AL_SBYTE1, 60);
    b.mapobj(0, 40, 0, -200, SH_IMYSHIP_4, IS_SHIPINTRO);
    b.setalvarw(AL_SWORD1, -70);
    b.setalvarb(AL_SBYTE1, 50);
    b.mapobj(0, 0, 0, -300, SH_IMYSHIP_4, IS_SHIPINTRO);
    b.setalvarw(AL_SWORD1, -100);
    b.setalvarb(AL_SBYTE1, -1);

    b.label("map1_1a.here2");
    b.mapwait((100 << 3) - MEDPSPEED);
    b.mapobj(0, 0, 0, 250 + (200 << 3), SH_OP_0, IS_GND);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapobj(0, 0, 0, 250 + (200 << 3), SH_OP_1, IS_NOCOLL);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.maploop("map1_1a.here2", 8);

    b.label("map1_1a.here3");
    b.mapwait((100 << 3) - MEDPSPEED);
    b.mapobj(0, 0, 0, 250 + (200 << 3), SH_OP_2, IS_GND);
    b.setalxvarb(ALX_DEPTHOFFSET, 1);
    b.mapif_builtin(MAP_CB_CHKSTRATDONE1, "map1_1a.fin");
    b.mapgoto("map1_1a.here3");
    b.label("map1_1a.fin");
    b.maprts();

    // ----------------------------------------------------------------
    // CL_GND.ASM — clear demo for ground levels.
    // ----------------------------------------------------------------
}

fn final_dpilar_l(b: &mut MapBuilder, wait: i32, y: i32, z: i32) {
    b.mapnobj(0, -60, y, z, SH_D_PILAR, STRATEGY_HALFDPILAR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapwait(wait);
}

fn final_dpilar_r(b: &mut MapBuilder, wait: i32, y: i32, z: i32) {
    b.mapnobj(wait, 60, y, z, SH_D_PILAR, STRATEGY_HALFDPILAR);
}

fn final_halfd_l(b: &mut MapBuilder, wait: i32, z: i32) {
    b.mapnobj(0, -60, -60, z, SH_HALF_D, STRATEGY_HALFDPILAR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapwait(wait);
}

fn final_halfd_r(b: &mut MapBuilder, wait: i32, z: i32) {
    b.mapnobj(wait, 60, -60, z, SH_HALF_D, STRATEGY_HALFDPILAR);
}

/// C `append_finalmap_content()` — FINALMAP.ASM shared tail used by
/// LEVEL3_7 (prefix "level3_7.final") and MAP_ID_FINAL (prefix "final").
/// Returns the (cantdie, cleanup) CODE65816 script ptrs.
/// DUPLICATE: consolidate — also used by the route-1 lane (level1_6).
pub(crate) fn append_finalmap_content(
    b: &mut MapBuilder,
    prefix: &str,
    current_level: u8,
) -> (u16, u16) {
    // incmap DM_LB1.ASM — complete last-base entrance cutscene.
    crate::levels::tunnel::append_dm_lb1(b, &format!("{prefix}.lb1"));

    // final_tunnel entry point
    let label = format!("{prefix}.tunnel");
    b.label(&label);
    b.mapwait(2000);

    // set BG to 2_6c
    b.setbg(BG_2_6C);

    // setrestart finalmap_restart
    b.mapcodejsl_builtin(MAP_CB_SETRESTART_L);

    // finalmap_cont entry point
    let label = format!("{prefix}.cont");
    b.label(&label);
    b.mapplayeroutview();
    b.mapwait(2000);

    // pathobj mes_andross1 message
    b.pathobj(0, 0, 0, 4000, SH_NULLSHAPE, PATH_ID_MES_ANDROSS1, 10, 10);

    // .finalt: tunnel sections (4 iterations)
    let label = format!("{prefix}.finalt");
    b.label(&label);
    b.mapnobj(0, 288, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    b.mapnobj(0, -288, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    b.mapnobj(0, 288, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    b.mapnobj(1536, -288, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);
    b.maploop(&label, 4);

    // wall/gate obstacles
    b.mapobj(0, -144, -60, 4000, SH_WALL_2, IS_HARD180YR);
    b.mapobj(1280, 144, -60, 4000, SH_WALL_2, IS_HARD180YR);
    b.mapnobj(0, 0, -60, 4000, SH_GATE_0, STRAT_ADDR_GATE3);
    b.setalvarb(AL_SBYTE1, 1);
    b.mapwait(1000);

    // `mapdpilarL/R` pairs.
    final_dpilar_l(b, 0, -20, 4000);
    final_dpilar_r(b, 800, -20, 4000);
    final_dpilar_l(b, 0, -100, 4000);
    final_dpilar_r(b, 800, -100, 4000);

    // item_5 + walls + pillars
    b.mapnobj(0, 0, -60, 4000, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.mapobj(0, -144, -60, 4000, SH_WALL_2, IS_HARD180YR);
    b.mapobj(4096, 144, -60, 4000, SH_WALL_2, IS_HARD180YR);
    final_dpilar_l(b, 0, -100, 4000);
    final_dpilar_r(b, 800, -100, 4000);
    final_dpilar_l(b, 0, -20, 4000);
    final_dpilar_r(b, 800, -20, 4000);

    // The source gates these obstacle patterns with
    // `mapgotoifnotlevel`; each embedded final-map caller knows its route.
    if current_level == 3 {
        final_dpilar_r(b, 600, -60, 4000);
        final_dpilar_l(b, 0, -100, 4000);
        final_dpilar_l(b, 600, -20, 4000);
        final_dpilar_r(b, 200, -20, 4000);
        final_dpilar_r(b, 200, -40, 4000);
        final_dpilar_r(b, 600, -60, 4000);
        final_dpilar_l(b, 200, -100, 4000);
        final_dpilar_l(b, 200, -80, 4000);
        final_dpilar_l(b, 600, -60, 4000);
        final_dpilar_r(b, 0, -100, 4000);
        final_dpilar_r(b, 600, -20, 4000);
    }

    if current_level == 2 {
        let level2t = format!("{prefix}.level2t");
        b.label(&level2t);
        final_dpilar_l(b, 0, -100, 4000);
        final_dpilar_r(b, 100, -100, 4000);
        final_dpilar_l(b, 0, -80, 4000);
        final_dpilar_r(b, 800, -80, 4000);
        final_dpilar_l(b, 0, -20, 4000);
        final_dpilar_r(b, 100, -20, 4000);
        final_dpilar_l(b, 0, -40, 4000);
        final_dpilar_r(b, 800, -40, 4000);
        b.maploop(&level2t, 2);
    }

    // common pillar section
    final_dpilar_l(b, 0, -100, 4000);
    final_dpilar_r(b, 0, -100, 4000);
    final_dpilar_l(b, 0, -20, 4000);
    final_dpilar_r(b, 1500, -20, 4000);

    // level 3 half-door section
    if current_level == 3 {
        let label = format!("{prefix}.level3t");
        b.label(&label);
        b.mapobj(0, 100, -60, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
        final_halfd_l(b, 1500, 4000);
        b.mapobj(0, -100, -60, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
        final_halfd_r(b, 1500, 4000);
        b.maploop(&label, 2);
        b.mapwait(500);
    }

    // level 1 half-door section
    if current_level == 1 {
        let label = format!("{prefix}.level1t");
        b.label(&label);
        b.mapobj(0, 110, -60, 4000, SH_WALL_4_PROXY, IS_HARD180YR);
        final_halfd_l(b, 1500, 4000);
        b.mapobj(0, -110, -60, 4000, SH_WALL_4_PROXY, IS_HARD180YR);
        final_halfd_r(b, 1500, 4000);
        b.maploop(&label, 2);
        b.mapwait(500);
    }

    // final corridor: item_7 + wall_4 + halfdL/R
    b.mapnobj(0, 96, -60, 4000, SH_ITEM_7, IS_ITEM7);
    b.setalvarb(AL_SBYTE1, 1);
    b.mapobj(0, 110, -60, 4000, SH_WALL_4_PROXY, IS_HARD180YR);
    final_halfd_l(b, 2000, 4000);
    b.mapobj(0, -110, -60, 4000, SH_WALL_4_PROXY, IS_HARD180YR);
    final_halfd_r(b, 1000, 4000);

    // tunnel exit
    b.mapwait(2000);
    b.pathobj(0, 0, 0, 3000, SH_NULLSHAPE, PATH_ID_MES_ANDROSS2, 10, 10);
    crate::levels::tunnel::append_ltunnel_exit(b, &format!("{prefix}.lexit"));

    // BG transition
    b.mapwait(100);
    b.setbg(BG_1_6C);
    b.initbg();
    // `maptexitwait -200` = mapwait 800.
    b.mapwait(800);
    let after_inspace = format!("{prefix}.after_inspace");
    b.mapif_builtin(crate::consts::cb::IS_PLAYER_DEAD, &after_inspace);
    b.mapcodejsl_builtin(crate::consts::cb::SET_PLAYER_INSPACE_L);
    b.label(&after_inspace);

    // boss final music
    b.setbgm(BGM_BOSS_FINAL);
    b.mapwait(2000);

    // face_b monolith boss
    b.mapnobj(
        4096,
        0,
        SPACE_VIEWCY,
        -200,
        SH_FACE_B_PROXY,
        STRAT_ADDR_MONOLITH,
    );

    // mapwaitboss nosound
    b.mapwait(100);
    let label = format!("{prefix}.bosswait.loop");
    b.label(&label);
    let contlabel = format!("{prefix}.bosswait.cont");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, &contlabel);
    b.mapgoto(&label);
    b.label(&contlabel);
    let cantdie_ptr = b.mapcode65816_inline();
    let cleanup_ptr = b.mapcode65816_inline();

    // markboss bossfinal
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);
    b.mapwait(5000);

    // Full `incmap dm_end` base escape and formation-flight sequence.
    crate::levels::tunnel::append_dm_end(b, &format!("{prefix}.dm_end"));

    // .wait1: infinite wait
    let label = format!("{prefix}.wait1");
    b.label(&label);
    b.mapwait(1000);
    b.mapgoto(&label);

    // finalmap_restart: setbgm $12, goto finalmap_cont
    let label = format!("{prefix}.restart");
    b.label(&label);
    b.mapwait(1000);
    b.setbgm(BGM_FINAL_CONT);
    let contlabel = format!("{prefix}.cont");
    b.mapgoto(&contlabel);
    (cantdie_ptr, cleanup_ptr)
}
