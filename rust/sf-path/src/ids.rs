//! Canonical path ids for the C-authored literal catalog.
//!
//! C origin: `src/path/path_literals.h` (anonymous `PATH_ID_*` enum).
//! Original compiled ids are preserved where already established; new ids for
//! newly ported literal slices are appended above the legacy range.
//! Path ids correspond to PATHDATA.ASM / KPATHDAT.ASM / DPATHDAT.ASM labels
//! (see the per-constant comments carried over from the C header).


pub const PATH_ID_E_GATE: u16 = 0;
pub const PATH_ID_E_FLOWER: u16 = 1;
pub const PATH_ID_E_FLOPEN: u16 = 2;
pub const PATH_ID_BIRD_METEOR: u16 = 9;
pub const PATH_ID_DAMY_EXP: u16 = 10;
pub const PATH_ID_DAMY_EXP2: u16 = 11;
pub const PATH_ID_E_EGG: u16 = 12;
pub const PATH_ID_E_BIG_BIRD: u16 = 13;
pub const PATH_ID_CHIBIR_2: u16 = 14;
pub const PATH_ID_CHIBIR_1: u16 = 15;
pub const PATH_ID_PINITA_B: u16 = 16;
pub const PATH_ID_PINITA_A: u16 = 17;
pub const PATH_ID_E_PILL: u16 = 18;
pub const PATH_ID_ITACHI_B: u16 = 19;
pub const PATH_ID_ITACHI_A: u16 = 20;
pub const PATH_ID_PONPON: u16 = 44;
pub const PATH_ID_MATEMSG: u16 = 91;
pub const PATH_ID_FROG1_1: u16 = 144;
pub const PATH_ID_FALCO_LV1: u16 = 148;
pub const PATH_ID_FROG_LV1: u16 = 149;
pub const PATH_ID_TOW_0: u16 = 160;
pub const PATH_ID_TOW_1: u16 = 161;
pub const PATH_ID_DSMOKE2: u16 = 162;
pub const PATH_ID_DSMOKE3: u16 = 163;
pub const PATH_ID_DSMOKE: u16 = 164;
pub const PATH_ID_ROBOT: u16 = 223;
pub const PATH_ID_ROBOTSWITHLOG: u16 = 224;
pub const PATH_ID_ROBOTWITHLOG2: u16 = 225;
pub const PATH_ID_CARRIEDLOG: u16 = 226;
pub const PATH_ID_ROBEXPLODE: u16 = 227;
pub const PATH_ID_KORORI: u16 = 228;
pub const PATH_ID_CHASE8_1: u16 = 229;
pub const PATH_ID_CHASE8_2: u16 = 230;
pub const PATH_ID_CHASE8_3: u16 = 231;
pub const PATH_ID_PATROL: u16 = 232;
pub const PATH_ID_ROBOTWITHLOG: u16 = 233;
pub const PATH_ID_CHASE6_1: u16 = 234;
pub const PATH_ID_CHASE6_2: u16 = 235;
pub const PATH_ID_E_RABBIT: u16 = 236;
pub const PATH_ID_E_FROG: u16 = 237;
pub const PATH_ID_E_FALCON: u16 = 238;
pub const PATH_ID_DUMMY: u16 = 239;
pub const PATH_ID_E_UFO: u16 = 240;
pub const PATH_ID_ASTEMSG: u16 = 241;
pub const PATH_ID_MES_MESSAGE: u16 = 242;
pub const PATH_ID_CHASE7_1: u16 = 243;
pub const PATH_ID_CHASE7_2: u16 = 244;
pub const PATH_ID_MY_BIRD: u16 = 245;
pub const PATH_ID_RING: u16 = 246;
pub const PATH_ID_CHASE1_1: u16 = 247;
pub const PATH_ID_CHASE1_2: u16 = 248;
pub const PATH_ID_E_ASTE: u16 = 249;
pub const PATH_ID_PYONTA: u16 = 250;
pub const PATH_ID_CHASE4_1: u16 = 251;
pub const PATH_ID_CHASE4_2: u16 = 252;
pub const PATH_ID_CHASE4_3: u16 = 253;
pub const PATH_ID_E_ASTE_B: u16 = 254;
pub const PATH_ID_E_BREASTE: u16 = 255;
pub const PATH_ID_INSEKIKUN: u16 = 256;
pub const PATH_ID_SCREW: u16 = 257;
pub const PATH_ID_DAMYSCR: u16 = 258;
pub const PATH_ID_PATRET_IRAB: u16 = 259;
pub const PATH_ID_PATRET_IFRO: u16 = 260;
pub const PATH_ID_PATRET_IFAL: u16 = 261;
pub const PATH_ID_SEPTER_RAB: u16 = 262;
pub const PATH_ID_SEPTER_FRO: u16 = 263;
pub const PATH_ID_SEPTER_FAL: u16 = 264;
pub const PATH_ID_FALCON3_1: u16 = 265;
pub const PATH_ID_CHECK: u16 = 266;
pub const PATH_ID_AT_HBEAM: u16 = 267;
pub const PATH_ID_EGU6: u16 = 268;
pub const PATH_ID_CHASE2_1: u16 = 269;
pub const PATH_ID_CHASE2_2: u16 = 270;
pub const PATH_ID_CHASE3_1: u16 = 271;
pub const PATH_ID_CHASE3_2: u16 = 272;
pub const PATH_ID_E_SHIELDR: u16 = 273;
pub const PATH_ID_EGU6_IRAB: u16 = 274;
pub const PATH_ID_EGU6_IFRO: u16 = 275;
pub const PATH_ID_EGU6_IFAL: u16 = 276;
// KPATHDAT.ASM: ending/transition camera paths
pub const PATH_ID_GAMEOVER: u16 = 277;
pub const PATH_ID_THEENDT: u16 = 278;
pub const PATH_ID_THEENDH: u16 = 279;
pub const PATH_ID_THEENDE: u16 = 280;
pub const PATH_ID_THEENDE2: u16 = 281;
pub const PATH_ID_THEENDN: u16 = 282;
pub const PATH_ID_THEENDD: u16 = 283;
pub const PATH_ID_FADEINTOTAL: u16 = 284;
pub const PATH_ID_TOTAL: u16 = 285;
pub const PATH_ID_TOTALN: u16 = 286;
pub const PATH_ID_AVE: u16 = 287;
pub const PATH_ID_AVEN: u16 = 288;
pub const PATH_ID_STAGE1: u16 = 289;
pub const PATH_ID_STAGE2: u16 = 290;
pub const PATH_ID_STAGE3: u16 = 291;
pub const PATH_ID_STAGE4: u16 = 292;
pub const PATH_ID_STAGE5: u16 = 293;
pub const PATH_ID_STAGE6: u16 = 294;
pub const PATH_ID_STAGE7: u16 = 295;
// MAP2_4 (Sector Y) paths — stubs until path data is ported.
pub const PATH_ID_E_WHALE: u16 = 296;
pub const PATH_ID_E_RAY_0: u16 = 297;
pub const PATH_ID_E_RAY_1: u16 = 298;
pub const PATH_ID_IKA_2: u16 = 299;
pub const PATH_ID_E_IKA: u16 = 300;
pub const PATH_ID_EGU1: u16 = 301;
pub const PATH_ID_EGU3: u16 = 302;
pub const PATH_ID_AMEBMSG: u16 = 303;
pub const PATH_ID_BRAYMSG: u16 = 304;
pub const PATH_ID_CHASE5_1: u16 = 305;
pub const PATH_ID_CHASE5_2: u16 = 306;
pub const PATH_ID_CHASE5_3: u16 = 307;
pub const PATH_ID_PATRET: u16 = 308;
pub const PATH_ID_REM_WHALE: u16 = 309;
pub const PATH_ID_HANDMSG: u16 = 310;
pub const PATH_ID_EGU1_IFRO: u16 = 311;
pub const PATH_ID_EGU1_IRAB: u16 = 312;
pub const PATH_ID_EGU1_IFAL: u16 = 313;
// MAP2_3A paths
pub const PATH_ID_L_CLISLA: u16 = 314;
pub const PATH_ID_R_CLISLA: u16 = 315;
pub const PATH_ID_MINI_CLI: u16 = 316;
pub const PATH_ID_E_WALK_1: u16 = 317;
pub const PATH_ID_EGU4: u16 = 318;
pub const PATH_ID_E_HELI: u16 = 319;
pub const PATH_ID_E_TANK: u16 = 320;
pub const PATH_ID_E_KANI_0: u16 = 321;
pub const PATH_ID_TENKI_ON: u16 = 322;
pub const PATH_ID_TENKI_DM: u16 = 323;
pub const PATH_ID_KANIHAHA: u16 = 324;
// MAP2_5 (Venom 2 Orbital) paths
pub const PATH_ID_EGU5: u16 = 325;
pub const PATH_ID_MINICAS2: u16 = 326;
pub const PATH_ID_MINICAS0: u16 = 327;
pub const PATH_ID_KASTMSG: u16 = 328;
// MAP3_2 tail paths
pub const PATH_ID_AMEBMSG2: u16 = 329;
// MAP3_3A (Fortuna Part A) paths
pub const PATH_ID_E_BEE: u16 = 330;
pub const PATH_ID_TOMSET: u16 = 331;
pub const PATH_ID_TOMHAHA: u16 = 332;
pub const PATH_ID_E_FLYFISH: u16 = 333;
pub const PATH_ID_KAMOME: u16 = 334;
pub const PATH_ID_DRAGONMSG: u16 = 335;
// SPECIAL.ASM (Out of This Dimension) paths
pub const PATH_ID_PAPER_1B: u16 = 336;
pub const PATH_ID_SLOTMACHINE: u16 = 337;
// CREDITS.ASM / SPECIAL.ASM cutscene path (stub)
pub const PATH_ID_CUTCREDS: u16 = 338;
// MAP1_3A1 (Space Armada Ship 1) paths
pub const PATH_ID_PATCOM: u16 = 339;
pub const PATH_ID_TOTUMSG: u16 = 340;
// MAP3_4B (Sector Z) paths
pub const PATH_ID_CALL_FOL: u16 = 341;
// FINALMAP.ASM (Andross) paths
pub const PATH_ID_MES_ANDROSS1: u16 = 342;
pub const PATH_ID_MES_ANDROSS2: u16 = 343;
// TRAINING.ASM paths
pub const PATH_ID_TRN_CK: u16 = 344;
pub const PATH_ID_TRN_RING: u16 = 345;
pub const PATH_ID_TRN_RING2: u16 = 346;
pub const PATH_ID_HENTAI_FAL: u16 = 347;
pub const PATH_ID_HENTAI_FRO: u16 = 348;
pub const PATH_ID_HENTAI_RAB: u16 = 349;
// CREDITS.ASM paths
pub const PATH_ID_DSIDESLIP: u16 = 350;
pub const PATH_ID_DSTARFOX: u16 = 351;
pub const PATH_ID_DPRESENTED: u16 = 352;
pub const PATH_ID_DNINTENDO: u16 = 353;
// Note: PATH_ID_THEENDT..THEENDD already defined at 278-283 above
// (KPATHDAT.ASM ending/transition camera paths).
// MAP1_5 (Venom 1 Orbital) paths
pub const PATH_ID_E_SHAWERL: u16 = 354;
pub const PATH_ID_E_SHAWERR: u16 = 355;
// MAP3_7A (Venom 3 Surface) paths
pub const PATH_ID_E_DOSUN: u16 = 356;
pub const PATH_ID_ITADOSUN: u16 = 357;
pub const PATH_ID_E_KURURI: u16 = 358;
pub const PATH_DATA_COUNT_LITERAL: u16 = 359;
