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
    pub const SETSTAGE: u8 = 14;
    pub const SETBG: u8 = 16;
    pub const WAIT: u8 = 18;
    pub const SETBGM: u8 = 20;
    pub const JSR: u8 = 40;
    pub const RTS: u8 = 42;
    pub const IF: u8 = 44;
    pub const GOTO: u8 = 46;
    pub const OBJZROT: u8 = 38;
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
    pub const QOBJ: u8 = 112;
    pub const DOBJ: u8 = 116;
    pub const QOBJ2: u8 = 118;
    pub const CODE65816: u8 = 120;
    pub const CODEJSL: u8 = 122;
    pub const SENDMSG: u8 = 130;
    pub const CSPECIAL: u8 = 132;
    pub const NORMOBJ: u8 = 134;
    pub const WAIT2: u8 = 138;
    pub const SETPATH: u8 = 140;
    /// Rust-authored object spawn carrying a typed [`DirectStrategy`].
    /// The payload intentionally has the same width as `NORMOBJ`, but its
    /// strategy word is an enum identity rather than a source-machine address.
    pub const DIRECTOBJ: u8 = 142;
    /// Rust-authored mother spawn carrying a typed [`DirectStrategy`].
    pub const DIRECTMOTHER: u8 = 144;
    /// Typed replacement for PLANET.ASM `mapnozremove`. The source embeds a
    /// small native-code block; the Rust map VM carries the gameplay meaning
    /// directly without shipping source-machine instructions.
    pub const PRESERVE_BEHIND_VIEW_OBJECTS: u8 = 146;
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
    pub const SET_PLAYER_TOCSLOW_L: u32 = 0x01030E;
    pub const SET_PLAYER_INMTEXIT_L: u32 = 0x01030F;
    pub const SET_PLAYER_INLTEXIT_L: u32 = 0x010310;
    pub const SET_PLAYER_INSPACE_L: u32 = 0x010311;
    pub const SET_PLAYER_INTOLB1_L: u32 = 0x010312;
    pub const SET_PLAYER_OUTOFLB2A_L: u32 = 0x010313;
    pub const SET_PLAYER_ESCAPENUCLEUS_L: u32 = 0x010314;
    /// Rust bridge for BGS.ASM `bg_1_3da`'s `pstrat playerwashent`.
    /// On the ROM this dispatch is owned by the background script rather than
    /// a MAPMACS `mapplayermode`, so the literal Rust map emits it explicitly.
    pub const SET_PLAYER_WASHENT_L: u32 = 0x010315;
    /// Rust bridge for BGS.ASM `bg_2_6b_1`'s `pstrat playerclearcolony`.
    pub const SET_PLAYER_CLEAR_COLONY_L: u32 = 0x010316;

    pub const IS_PLAYER_DEAD: u32 = 0x010401;
    pub const PLAYER_OUTVIEW_L: u32 = 0x010402;
    pub const LEVELFINISHED_ZERO: u32 = 0x010403;
    pub const PLAYER_CANT_DIE: u32 = 0x010404;

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
pub const MAP_ISTRAT_SPACEBAR: u32 = 165;
pub const MAP_ISTRAT_SPINSPACEBAR: u32 = 166;
pub const MAP_ISTRAT_SPACEBAR1: u32 = 167;
pub const MAP_ISTRAT_SPACEBAR3: u32 = 168;

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
/// `wipein mscramwipe_circle` black-screen travel distance in LEVEL{1,2,3}_1.
pub const SCRAMBLE_WIPE_DISTANCE: i32 = 300;
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
// Shape ids (the original def_shape catalog order).
// Only the ids used by the ported levels + shared engine helpers are
// listed; follow-up level ports append their own ids here (keep the
// C names and values).
// ============================================================
pub mod sh {
    pub const NULLSHAPE: u16 = 0;
    pub const MYSHIP_4: u16 = 2; // hand-tuned builtin Arwing
    pub const GATE_0: u16 = 7;
    pub const KAMIKAZE: u16 = 9;
    pub const PILLAR3: u16 = 27;
    pub const BOM_WING: u16 = 48;
    pub const RADER_0: u16 = 50;
    pub const RADER_1: u16 = 51;
    pub const ZACO_6: u16 = 52;
    pub const ZACO_5: u16 = 53;
    pub const BOSS_7_1: u16 = 55;
    pub const TOWER_2: u16 = 58;
    pub const BU_0: u16 = 60;
    pub const BU_1: u16 = 61;
    pub const BU_2: u16 = 62;
    pub const BU_3: u16 = 63;
    pub const BU_4: u16 = 64;
    pub const BU_5: u16 = 65;
    pub const BU_6: u16 = 66;
    pub const BU_7: u16 = 67;
    pub const BU_8: u16 = 68;
    pub const BOSS_9_5: u16 = 78;
    pub const AIR_1: u16 = 79;
    pub const LINE_2: u16 = 80;
    pub const WALL_4: u16 = 90;
    pub const R_BU_1: u16 = 96;
    pub const R_BU_4: u16 = 99; // PLANET.ASM r_bu_4
    pub const R_BU_6: u16 = 101;
    pub const R_BU_7: u16 = 102;
    pub const ZACO_8: u16 = 104;
    pub const SHIPS: u16 = 110;
    pub const CARRIER: u16 = 114;
    pub const MYBASE_1: u16 = 124;
    pub const SHIP_S_0: u16 = 125;
    pub const SHIP_S_1: u16 = 126;
    pub const XWIRESPACEBAR: u16 = 136;
    pub const XPWIRESPACEBAR: u16 = 137;
    pub const SXPWIRESPACEBAR: u16 = 138;
    pub const YWIRESPACEBAR: u16 = 139;
    pub const ZWIRESPACEBAR: u16 = 140;
    pub const SXWIRESPACEBAR: u16 = 141;
    pub const SYWIRESPACEBAR: u16 = 142;
    pub const SZWIRESPACEBAR: u16 = 143;
    pub const XSOLIDSPACEBAR: u16 = 144;
    pub const XPSOLIDSPACEBAR: u16 = 145;
    pub const SXPSOLIDSPACEBAR: u16 = 146;
    pub const YSOLIDSPACEBAR: u16 = 147;
    pub const ZSOLIDSPACEBAR: u16 = 148;
    pub const SXSOLIDSPACEBAR: u16 = 149;
    pub const SYSOLIDSPACEBAR: u16 = 150;
    pub const SZSOLIDSPACEBAR: u16 = 151;
    pub const ITEM_5: u16 = 158;
    pub const ITEM_7: u16 = 160;
    pub const WALKER_2: u16 = 164;
    pub const RO_0: u16 = 169;
    pub const ROBOT_0: u16 = 420; // SHAPE_EXT_ROBOT_0
    pub const STALK: u16 = 208;
    pub const S_FISH: u16 = 211;
    pub const LAST_B_0: u16 = 212;
    pub const LAST_B_2: u16 = 213;
    pub const LAST_B_3: u16 = 214;
    pub const DOOR_L: u16 = 215;
    pub const ZACO_A: u16 = 217;
    pub const FRIENDSHIP_4: u16 = 218;
    pub const FACE_B: u16 = 223;
    pub const MY_DEMOS: u16 = 224;
    pub const MY_DEMO: u16 = 225;
    pub const ARCH_0: u16 = 228;
    pub const BIG_GATE: u16 = 233;
    pub const TOW_0: u16 = 247;
    pub const MYBASE_0: u16 = 256; // SHAPE_EXT_MYBASE_0 (extended shape catalog)
    pub const WHALE: u16 = 281;
    pub const PIPE_8_0: u16 = 285;
    pub const PIPE_8: u16 = 286;
    pub const BOU_1B: u16 = 287;
    pub const PAPER_1: u16 = 288;
    pub const PAPER_3: u16 = 289;
    pub const POLE_0: u16 = 290;
    pub const SLOT_0: u16 = 291;
    pub const FONT_T2: u16 = 292;
    pub const FONT_H2: u16 = 293;
    pub const FONT_E2: u16 = 294;
    pub const FONT_E3: u16 = 295;
    pub const FONT_N2: u16 = 296;
    pub const FONT_D2: u16 = 297;
    pub const PILON: u16 = 298;
    pub const ITEM_0: u16 = 324;
    pub const R_BUT_2: u16 = 325;
    pub const WALK_4_0: u16 = 326;
    pub const BOOST_SHAPE: u16 = 362;
    pub const LINE_SPARK: u16 = 380;
    pub const BOSS_9_0: u16 = 391;
    pub const BARRIER: u16 = 392;
    pub const BOSS_0_0: u16 = 432;
    pub const BOSS_0_0A: u16 = 433;
    pub const BOSS_0_2: u16 = 434;
    pub const BOSS_0_3: u16 = 435;
    pub const BOSS_1_0: u16 = 436;
    pub const BOSS_1_1: u16 = 437;
    pub const AMOEBA1: u16 = 438;
    pub const RPILLAR3: u16 = 439;
    pub const OP_0: u16 = 551;
    pub const OP_1: u16 = 552;
    pub const OP_2: u16 = 553;
    pub const IMYSHIP_4: u16 = 554;

    // Compatibility alias retained for pre-catalog callers.
    pub const MY_DEMO_PROXY: u16 = MY_DEMO;
}

// ============================================================
// Strategy ids (ISTRATS.ASM order; levels.c IS_* block) + synthetic
// strategy addresses. Same append-only policy as `sh`.
// ============================================================
pub mod is {
    pub const NOCOLL: u32 = 10;
    pub const GND: u32 = 11;
    pub const CLSHIPGNDA: u32 = 15;
    pub const CLSHIPGNDB: u32 = 16;
    pub const CLSHIPGNDC: u32 = 17;
    pub const GATE: u32 = 52;
    pub const PILLAR3: u32 = 78;
    pub const FLYPILLARS: u32 = 73;
    pub const BOMWING: u32 = 88;
    pub const RADER0: u32 = 91;
    pub const RADER1: u32 = 92;
    pub const ZACOS: u32 = 93;
    pub const ZACO1L: u32 = 94;
    pub const ZACO1R: u32 = 95;
    pub const BOSS7: u32 = 98;
    pub const BOSS8: u32 = 83;
    pub const NUCLEUSLAUNCHER: u32 = 85;
    pub const NUCLEUSPILLAR: u32 = 86;
    pub const TOWER0: u32 = 100;
    pub const ZACO4: u32 = 102;
    pub const HARD180YR: u32 = 104;
    pub const HARD90YR: u32 = 126;
    pub const SZACO2: u32 = 128;
    pub const AMOEBA: u32 = 127;
    pub const SHIPS: u32 = 132;
    pub const FRIENDEXITBASE: u32 = 151;
    pub const BOSSB: u32 = 114;
    pub const BOSSF: u32 = 115;
    pub const MADBIKER: u32 = 118;
    pub const MADTRUCKER: u32 = 119;
    pub const ROADLINE: u32 = 120;
    pub const AIRSHIP: u32 = 125;
    pub const PATH: u32 = 156;
    pub const UPERM: u32 = 159;
    pub const SHOU0: u32 = 177;
    pub const SHOU0A: u32 = 178;
    pub const METEO0: u32 = 194;
    pub const MINE2: u32 = 206;
    pub const BIG_METEOR: u32 = 233;
    pub const BREAK_METEOR: u32 = 234;
    pub const ITEM5: u32 = 174;
    pub const ITEM7: u32 = 176;
    pub const LOCHNESSMONSTER: u32 = 197;
    pub const TREE3: u32 = 205;
    pub const SFISH: u32 = 208;
    pub const HARDROT: u32 = 209;
    pub const HARD: u32 = 225;
    pub const LASTB2: u32 = 210;
    pub const LASTB3: u32 = 211;
    pub const LASTB4: u32 = 212;
    pub const MONOLITH: u32 = 215;
    pub const LSEQDOOR1: u32 = 216;
    pub const LSEQDOOR2: u32 = 217;
    pub const PSHIPOUTOFLB1: u32 = 218;
    pub const VIEWOUTOFLB1: u32 = 219;
    pub const NOCOLLANIM0: u32 = 220;
    pub const PSHIPOUTOFLB3: u32 = 221;
    pub const VIEWOUTOFLB3: u32 = 222;
    pub const SHIPOUTOFLB3: u32 = 223;
    pub const SHIPINTRO: u32 = 238;
    pub const BOSS7INTRO: u32 = 239;
    pub const TIT: u32 = 241;
    pub const SHIP0CDOWN: u32 = 236;
    pub const WARP: u32 = 160;
    pub const SKILLFLY: u32 = 240;
    pub const PATHDHA: u32 = 242;
    /// `patht_istrat` — path-following scaled MARIO text object.
    pub const PATHT: u32 = 228;
    pub const SPACEBARSHOOT: u32 = 173;
}

/// Native strategies that are not entries in the game's compact strategy
/// table. Authored maps store this identity directly and resolve it through a
/// separate typed registry; it never shares a numeric namespace with ROM
/// compatibility addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DirectStrategy {
    BossSeamon = 1,
    Mine0,
    Mother1,
    Mother2,
    Meteor,
    SlowMeteor,
    SearchMeteor,
    Clasteroid,
    SeaDragon,
    Damyscr,
    SpacePilon,
    BossH,
    HalfDPillar,
    Pole0,
    GroundPilon,
}

impl DirectStrategy {
    pub const ALL: [Self; 15] = [
        Self::BossSeamon,
        Self::Mine0,
        Self::Mother1,
        Self::Mother2,
        Self::Meteor,
        Self::SlowMeteor,
        Self::SearchMeteor,
        Self::Clasteroid,
        Self::SeaDragon,
        Self::Damyscr,
        Self::SpacePilon,
        Self::BossH,
        Self::HalfDPillar,
        Self::Pole0,
        Self::GroundPilon,
    ];

    pub const COUNT: usize = Self::ALL.len();

    pub const fn id(self) -> u8 {
        self as u8
    }

    pub const fn index(self) -> usize {
        self.id() as usize - 1
    }

    pub const fn from_id(id: u8) -> Option<Self> {
        if id == 0 || id as usize > Self::COUNT {
            None
        } else {
            Some(Self::ALL[id as usize - 1])
        }
    }
}

/// Strategy operand accepted by authored map builders. `Encoded` is retained
/// for compact-table and ROM-oracle compatible records; new native Rust
/// strategies use `Direct` and therefore cannot collide with those values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrategyRef {
    Encoded(u32),
    Direct(DirectStrategy),
}

impl From<u32> for StrategyRef {
    fn from(value: u32) -> Self {
        Self::Encoded(value)
    }
}

impl From<i32> for StrategyRef {
    fn from(value: i32) -> Self {
        assert!(value >= 0, "strategy identity cannot be negative");
        Self::Encoded(value as u32)
    }
}

impl From<DirectStrategy> for StrategyRef {
    fn from(value: DirectStrategy) -> Self {
        Self::Direct(value)
    }
}

// Synthetic strategy addresses (levels.c STRAT_ADDR_* subset).
// `gate_Istrat` in map data contexts resolves to the flat IS_GATE id
// (levels.c redefines STRAT_ADDR_GATE3 to IS_GATE after including
// strat_table.h, so the value baked into bytecode is 52).
pub const STRAT_ADDR_GATE3: u32 = is::GATE;
/// Non-table `mine0_istrat` assembled address (DSTRATS.ASM / retail ROM).
pub const STRATEGY_MINE0: DirectStrategy = DirectStrategy::Mine0;
// `tit_istrat` is the title screen strategy (TITLE.ASM).
pub const STRAT_ADDR_TIT: u32 = is::TIT;

/// Direct strategy symbols used exclusively by the retail attract intro.
///
/// These are the stable native-map keys emitted by `INTRO.ASM`; they are not
/// ISTRATS table rows.
pub mod intro_strategy_address {
    pub const PLAYER_DOWN: u32 = 0x05_0019;
    pub const PLAYER_DOWN_LEFT: u32 = 0x05_001A;
    pub const PLAYER_DOWN_RIGHT: u32 = 0x05_001B;
    pub const PLAYER_FIRE: u32 = 0x05_001C;
    pub const ZACO: u32 = 0x05_001E;
    pub const ZACO_LEADER: u32 = 0x05_001F;
}

// Mother-system strategy addresses (registered by sf-strat::mother).
// NOTE: the old values 0x020000/0x020001 collided with the synthetic
// istrat forms of istrats 0/1 (the PLAYER init strat and pbody), so mother
// objects resolved to the player strategy and the mothermap never ran.
// 0x0300xx is the non-istrat symbol space (0x030001-0x030007 are already
// claimed by TOW0EXPLODE/GATE3/PLAYER_EXITBASE/SPACEPILON/BOSSSEAMON/
// BOSSG/SHIP0CDOWN).
pub const STRATEGY_MOTHER1: DirectStrategy = DirectStrategy::Mother1;
pub const STRATEGY_MOTHER2: DirectStrategy = DirectStrategy::Mother2;
pub const STRATEGY_METEOR: DirectStrategy = DirectStrategy::Meteor;
pub const STRATEGY_SLOWMETEOR: DirectStrategy = DirectStrategy::SlowMeteor;
pub const STRATEGY_SEARCHMETEOR: DirectStrategy = DirectStrategy::SearchMeteor;
pub const STRATEGY_CLASTEROID: DirectStrategy = DirectStrategy::Clasteroid;
pub const STRATEGY_SEADRAGON: DirectStrategy = DirectStrategy::SeaDragon;
pub const STRATEGY_DAMYSCR: DirectStrategy = DirectStrategy::Damyscr;

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
    pub const DINTRO1: u16 = 362;
}

/// Low words of the assembled MARIO `msg_*` records (MDATA.MC).  The ROM
/// `textpath` macro stores these in `al_coltab`; the renderer resolves the
/// same values back to strings for scaled world-space text.
pub mod msg {
    pub const STARFOX: u16 = 0xC8DA;
    pub const NINTENDO: u16 = 0xC8E3;
    pub const PRESENTED: u16 = 0xC8EC;
    pub const PRESENTS: u16 = 0xC8F6;
    pub const DEVELOPED: u16 = 0xC8FF;
    pub const PROGRAMMED: u16 = 0xC90C;
    pub const BY: u16 = 0xC917;
    pub const ARGONAUT: u16 = 0xC91A;
    pub const EXECUTIVE: u16 = 0xC92C;
    pub const YAMAUCHI: u16 = 0xC93F;
    pub const PRODUCER: u16 = 0xC950;
    pub const MIYAMOTO: u16 = 0xC959;
    pub const DIRECTOR: u16 = 0xC96A;
    pub const EGUCHI: u16 = 0xC973;
    pub const ASSISTANTDIRECTOR: u16 = 0xC982;
    pub const YAMADA: u16 = 0xC995;
    pub const DYLAN: u16 = 0xC9A3;
    pub const GILES: u16 = 0xC9B2;
    pub const KRISTER: u16 = 0xC9C0;
    pub const SYSTEM3D: u16 = 0xC9D0;
    pub const PETE: u16 = 0xC9DA;
    pub const CARL: u16 = 0xC9E6;
    pub const GRAPHICDESIGNER: u16 = 0xC9F2;
    pub const IMAMURA: u16 = 0xCA03;
    pub const SHAPEDESIGNER: u16 = 0xCA12;
    pub const WATANABE: u16 = 0xCA21;
    pub const KONDO: u16 = 0xCA33;
    pub const HIRASAWA: u16 = 0xCA3E;
    pub const SUPERFXSTAFF: u16 = 0xCA4E;
    pub const BEN: u16 = 0xCA5D;
    pub const NISHIUMI: u16 = 0xCA68;
    pub const KAKUI: u16 = 0xCA79;
    pub const YAMASHIRO: u16 = 0xCA88;
    pub const KAWAGUCHI: u16 = 0xCA9A;
    pub const JEZ: u16 = 0xCAB7;
    pub const KATO: u16 = 0xCABF;
    pub const EFFECTS: u16 = 0xCACA;
    pub const COMPOSER: u16 = 0xCAD8;
    pub const NISHIDA: u16 = 0xCAE7;
    pub const IAN: u16 = 0xCAF8;
    pub const DAN: u16 = 0xCB05;
    pub const TONY: u16 = 0xCB0F;
    pub const KIMURA: u16 = 0xCB1B;
    pub const SHIMIZU: u16 = 0xCB29;
    pub const YAJIMA: u16 = 0xCB37;
    pub const YAMAMOTO: u16 = 0xCB45;
    pub const ENGLISH: u16 = 0xCB5C;
    pub const JAPANESE: u16 = 0xCB6C;
    pub const SOFTWARE: u16 = 0xCB7D;
    pub const RICK: u16 = 0xCB8E;
    pub const JONDEAN: u16 = 0xCB9D;
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
    pub const COLTAB: u16 = 32;
    pub const TX: u16 = 42;
    pub const PWORD1: u16 = 52;
}

// ============================================================
// External vars mirrored into flat WRAM for map opcodes (levels.c WM_*)
// ============================================================
pub mod wm {
    /// Super FX RAM `m_meters` (`SYMBOLS.TXT`: $70:0200). Unlike the
    /// ordinary WRAM variables below, this target must retain its bank byte
    /// in SETVAR bytecode so the game bridge can distinguish it from WRAM
    /// $00:0200.
    pub const M_METERS: u32 = 0x70_0200;
    /// Native WRAM scroll-request state used by the credits screen.
    pub const BG2VOFSREQ: u16 = 0x1A39;
    pub const BG2HOFSREQ: u16 = 0x1A3B;
    pub const BG2VOFSOVERRIDE: u16 = 0x1AE1;
    pub const BG2YSCROLL: u16 = 0x1721;
    pub const DOSPACESC: u16 = 0x1727;
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
    /// Retail map-program operand for the screen-black hold counter.
    /// Decoded to `GameVars::strategy.stay_black` at the import boundary.
    pub const STAYBLACK: u16 = 0x1962;
    pub const GSVAR_BYTE1: u16 = 0x0310;
    pub const MAPTRIGGER: u16 = 0x0311;
    pub const NUMENDOK: u16 = 0x0312;
    pub const NUMPLASERS: u16 = 0x0313;
    /// Typed map operand for the circular-wipe request.
    pub const CIRCULAR_WIPE: u16 = 0x0317;
    /// Typed import operand for the source one-byte `scramble` countdown.
    pub const SCRAMBLE_COUNT: u16 = 0x0318;
    /// Retail Rev 2 background-swap player-strategy preservation latch.
    pub const PRESERVE_PLAYER_STRATEGY: u16 = 0x1F05;
    pub const HPOSJMP: u16 = 0x1AE6; // 2 bytes, SYMBOLS.TXT
    pub const BOSSMAXHP: u16 = 0x0316; // 2 bytes
}

// Space-bar layout constants (levels.c).
pub const SPACE_VIEWCY: i32 = -60;
pub const SPACE_MINX: i32 = -240;
pub const SPACE_MAXX: i32 = 240;
pub const SPACEBAR_BASE_DIST: i32 = 3000;
pub const SPACEBAR_UNIT_LEN: i32 = 125;
