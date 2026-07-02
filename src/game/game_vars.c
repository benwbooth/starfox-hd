// Game state variables — decompiled from GILESALC.INC
// Storage for all game state that was originally in SNES work RAM.

#include "game_vars.h"
#include "../variables.h"
#include <string.h>

// ============================================================
// Game Flags
// ============================================================
uint8 g_gameflags;
uint8 g_gameflags2;
uint8 g_stratflags;
uint8 g_bossflags;
uint8 g_sflags;
uint8 g_svar_flags;

// ============================================================
// Player Ship Flags
// ============================================================
uint8 g_pshipflags;
uint8 g_pshipflags2;
uint8 g_pshipflags3;
uint8 g_pstratflags;
uint8 g_playerflymode;
uint8 g_splayerflymode;
uint8 g_splayerflymodeopt;
uint8 g_pmovelimit;
uint8 g_pmovelimitAND;
uint8 g_missboundflags;
uint8 g_keyflags;
uint8 g_arrows;
uint8 g_doingwipe;
int8  g_stayblack;
uint8 g_specials_dead;

// ============================================================
// Player State
// ============================================================
int16  g_playervelZ;
int16  g_player_speed;
uint8  g_player_tospeed;
uint8  g_player_medspeed;
int16  g_player_posx;
int16  g_player_posy;
int16  g_player_posz;
int16  g_player_turnrot;
int16  g_playerdieYrotspeed;
uint8  g_player_rolldelay;
uint8  g_player_rollZvel;
uint8  g_player_rollZoff;
int16  g_player_zshake;
int16  g_player_zshakeV;
uint8  g_player_Ztilt;
uint8  g_player_zstratadd;
uint8  g_player_noctrlcnt;
uint8  g_curr_ship;

uint16 g_playershape;
uint16 g_playershapeL;
uint16 g_playershapeR;
uint16 g_playershapeLR;

// ============================================================
// View / Camera
// ============================================================
int16  g_pviewposx;
int16  g_pviewposy;
int16  g_pviewposz;
int16  g_viewposx;
int16  g_viewposy;
int16  g_viewposz;
int16  g_pviewvelz;
int16  g_pviewposzoff;
int16  g_viewfloatptr;
int16  g_viewfloatX;
int16  g_viewfloatY;
int16  g_viewCY;
int16  g_viewdist;
int16  g_viewaddx;
int16  g_viewaddy;
int16  g_viewtoobj;
int16  g_viewposXoff;
int16  g_viewposYoff;
uint8  g_viewshakeX;
uint8  g_viewshakeY;
uint8  g_viewshakeZ;

// ============================================================
// Counters / Timing
// ============================================================
uint16 g_gameframe;
uint8  g_framerate;
uint8  g_superspeedcnt;
uint8  g_superspeeddelay;

// ============================================================
// Weapon / Combat
// ============================================================
uint8  g_numplasers;
uint8  g_firecnt;
uint8  g_firedelay;
uint8  g_specialdelay;
uint8  g_screenflashcnt;
uint8  g_screenflashtype;
uint8  g_pnumhits;
uint16 g_specwepcnt;
uint8  g_boostcnt;

int16  g_svar_weapx;
int16  g_svar_weapy;
int16  g_svar_weapz;
uint8  g_svar_weapRy;
uint8  g_svar_weapRx;
int16  g_svar_weapobj;

// ============================================================
// Strategy Object State
// ============================================================
int16  g_stratobj_posx;
int16  g_stratobj_posy;
int16  g_stratobj_posz;
int16  g_playstrat_cnt;

int16  g_svar_word1;
int16  g_svar_word2;
int16  g_svar_word3;
int16  g_svar_dist;
uint8  g_svar_byte1;
uint8  g_svar_byte2;
uint8  g_svar_byte3;
uint8  g_svar_byte4;
uint8  g_svar_byte5;
uint8  g_svar_byte6;

int16  g_smvar_byte1;
int16  g_smvar_byte2;
int16  g_smvar_byte3;
int16  g_smvar_word1;
int16  g_smvar_word2;
int16  g_smvar_word3;
int16  g_smvar_word4;
int16  g_swvar_word1;

uint8  g_gsvar_byte1;
uint8  g_gsvar_byte2;

uint8  g_maptrigger;

uint8  g_psvar_byte1;
uint8  g_psvar_byte2;
uint8  g_psvar_byte3;
int16  g_psvar_word1;
int16  g_psvar_word2;
int16  g_psvar_word3;
int16  g_psvar_word4;

// ============================================================
// Player Movement Bounds
// ============================================================
int16  g_minpmoveX;
int16  g_maxpmoveX;
int16  g_minpmoveY;
int16  g_maxpmoveY;
int16  g_minMmoveX;
int16  g_maxMmoveX;
int16  g_maxMmoveY;
int16  g_minpWmoveY;
int16  g_maxpWmoveY;

// ============================================================
// Collision Objects
// ============================================================
int16  g_pcollobj_LW;
int16  g_pcollobj_RW;
int16  g_pcollobj_B;
int16  g_pcboxobj_LW;
int16  g_pcboxobj_RW;
int16  g_pcboxobj_B;

// ============================================================
// Missile Bounds
// ============================================================
int16  g_missbTOPLEFT;
int16  g_missbTOPRIGHT;
int16  g_missbBOTLEFT;
int16  g_missbBOTRIGHT;

// ============================================================
// Misc
// ============================================================
uint8  g_slowstars;
int16  g_hudrot;
int16  g_fobj;
uint8  g_nomaxbg2Yscroll;
int16  g_slowstars_viewposz;
int16  g_bgsscrollZ;
int16  g_boostobj;
uint8  g_boostZoff;
int16  g_pwingdistwall;
int16  g_rangexy;
uint8  g_wireendflash;
uint8  g_gasflags;
int16  g_wpposZ;
uint8  g_maxstate;
uint8  g_slimecount;
uint8  g_freezestrats;
int16  g_internalPLAYPT;
uint8  g_timeuntilfade;
uint8  g_oncewipe;
int16  g_circleanim;
uint8  g_floatvar1;
uint8  g_floatvar2;
int16  g_dummyobj;
int16  g_player_Zrotfloatptr;
uint8  g_player_Zrotfloat;
uint8  g_trotx;
uint8  g_troty;
uint8  g_lastcont0;
uint8  g_lastcontl0;
int16  g_svar_collx;
int16  g_svar_colly;
int16  g_svar_collz;
int16  g_tpa;

// Camera / View Rotation State (from ALCS.INC / GAME.ASM)
int16  g_outvx;
int16  g_outvy;
int16  g_outdist;
int16  g_bg2scroll;
int16  g_lastrot;
uint8  g_viewtype;
int16  g_bg2Yscroll;
int16  g_bg2Xscroll;
int16  g_plrotx;
int16  g_plroty;

// ============================================================
// Window / Fade State
// ============================================================
uint8 g_windowmode;
WindowState g_windowarray[WINDOWARRAY_SIZE];
int8  g_fadedir;

// ============================================================
// Level / Map State
// ============================================================
uint16 g_mapvar1;
uint16 g_mapvar2;
uint16 g_mapvar3;
uint16 g_mapvar4;
uint16 g_mapcnt;
uint16 g_mapptr;
int16  g_stagecnt;
int16  g_dotsflag;
uint8  g_othmusic;
uint16 g_maprestart;
uint16 g_maprestarttemp;
uint16 g_restartbg;
uint16 g_restartpalfade;
uint16 g_lastpalfade;
uint8  g_maprestartbank;
uint8  g_maprestartbanktemp;
uint8  g_eroll1;
uint8  g_ebyte3;
uint16 g_bossmaxhp;
uint16 g_meters;

// ============================================================
// Game Mode / Stage
// ============================================================
uint8  g_game_mode;
uint16 g_stage;
uint8  g_whichroute;
uint8  g_lives;
uint8  g_currentlevel;
uint16 g_routes[4];
int16  g_currentplanet;
uint16 g_mapmode;
uint16 g_bg_dmalist;
uint16 g_bgtransspeed;
uint16 g_currentbg;
uint8  g_bgflags;
uint32 g_newmap;
uint8  g_peppermsg;
uint16 g_nebula_on;

// ============================================================
// HP values
// ============================================================
uint8  g_fox_hp;
uint8  g_bunny_hp;
uint8  g_falcon_hp;
uint8  g_frog_hp;
uint8  g_pepper_hp;
uint8  g_andross_hp;

// ============================================================
// CONTINUE / message state
// ============================================================
uint8  g_whichfriend;
uint16 g_friends_msg;
uint8  g_msg_count1;
uint8  g_msg_count2;
uint8  g_friends_sound;
uint8  g_friends_meter;
uint16 g_playerscore;
uint8  g_numendok;

// ============================================================
// Sound State
// ============================================================
uint8  g_lsnd;
uint8  g_csnd;
uint8  g_rsnd;
uint8  g_msnd;
uint8  g_fsnd;
uint8  g_playersndflag;
int16  g_lastplayx;
uint8  g_sdspt3;
uint8  g_sdgpt3;
uint8  g_sdpck3;
uint8  g_pausesnd;
uint8  g_sdport3[16];
uint16 g_lastblock;
uint8  g_nosetport3;
uint8  g_port1bolox;
uint8  g_inatunnel;

// ============================================================
// View Float Bob Table (from GSTRATS.ASM viewfloattab)
// Sinusoidal camera bob — 40 entries cycling through +6 to -6
// ============================================================
const int16 g_viewfloattab[VIEWFLOATTAB_LEN] = {
    0,
    1, 2, 3, 4, 4, 5, 5, 6, 6, 6,
    5, 5, 4, 4, 3, 2, 1,
    0,
    -1, -2, -3, -4, -4, -5, -5, -6, -6, -6,
    -5, -5, -4, -4, -3, -2, -1,
    0, 0, 0, 0  // padding to 40
};

// ============================================================
// Initialize all game variables
// Called from MAIN.ASM initialise_l equivalent
// ============================================================
void GameVars_Init(void) {
    // Zero all flags
    g_gameflags = 0;
    g_gameflags2 = 0;
    g_stratflags = 0;
    g_bossflags = 0;
    g_sflags = 0;
    g_svar_flags = 0;
    g_pshipflags = 0;
    g_pshipflags2 = 0;
    g_pshipflags3 = 0;
    g_pstratflags = 0;
    g_playerflymode = PFM_SHADOWS;  // Shadows on by default
    g_splayerflymode = SPFM_NORM;
    g_splayerflymodeopt = 0;
    g_pmovelimit = 0;
    g_pmovelimitAND = 0;
    g_missboundflags = 0;
    g_keyflags = 0;
    g_arrows = 0;
    g_doingwipe = 0;
    g_stayblack = -1;
    g_specials_dead = 0;

    // Player defaults (from MAIN.ASM initgame_l)
    g_playervelZ = 0;
    g_player_speed = 0;
    g_player_tospeed = 4;
    g_player_medspeed = 4;
    g_player_posx = 0;
    g_player_posy = 0;
    g_player_posz = 0;
    g_player_turnrot = 0;
    g_playerdieYrotspeed = 0;
    g_player_rolldelay = 0;
    g_player_rollZvel = 0;
    g_player_rollZoff = 0;
    g_player_zshake = 0;
    g_player_zshakeV = 0;
    g_player_Ztilt = 0;
    g_player_zstratadd = 0;
    g_player_noctrlcnt = 0;
    g_curr_ship = PSHIPNUM_NORM;

    // View defaults
    g_pviewposx = 0;
    g_pviewposy = 0;
    g_pviewposz = 0;
    g_viewposx = 0;
    g_viewposy = 0;
    g_viewposz = 0;
    g_pviewvelz = 0;
    g_pviewposzoff = 0;
    g_viewfloatptr = 0;
    g_viewfloatX = 0;
    g_viewfloatY = 0;
    g_viewCY = 0;
    g_viewdist = 0;
    g_viewaddx = 0;
    g_viewaddy = 0;
    g_viewtoobj = 0;
    g_viewposXoff = 0;
    g_viewposYoff = 0;
    g_viewshakeX = 0;
    g_viewshakeY = 0;
    g_viewshakeZ = 0;

    // Timing
    g_gameframe = 0;
    g_framerate = 0;
    g_superspeedcnt = 0;
    g_superspeeddelay = 0;

    // Combat
    g_numplasers = 0;
    g_firecnt = 0;
    g_firedelay = 0;
    g_specialdelay = 0;
    g_screenflashcnt = 0;
    g_screenflashtype = 0;
    g_pnumhits = 0;
    g_specwepcnt = DEFAULT_NUKE_COUNT;
    g_boostcnt = 0;

    // Movement bounds (set per-level, defaults for ground mode)
    g_minpmoveX = -120;
    g_maxpmoveX = 120;
    g_minpmoveY = -60;
    g_maxpmoveY = 100;
    g_minMmoveX = -120;
    g_maxMmoveX = 120;
    g_maxMmoveY = 100;
    g_minpWmoveY = -30;
    g_maxpWmoveY = 80;

    // Game state
    g_game_mode = SPACE_MODE;
    g_stage = 0;
    g_whichroute = 0;
    g_lives = DEFAULT_LIVES;
    g_currentlevel = 0;
    memset(g_routes, 0, sizeof(g_routes));
    g_currentplanet = -1;
    g_mapmode = 0;
    g_bg_dmalist = 0;
    g_bgtransspeed = 0;
    g_currentbg = 0;
    g_bgflags = 0;
    g_newmap = 0;
    g_peppermsg = 0;
    g_nebula_on = 0;

    // HP (from MAIN.ASM — initgame_l sets these)
    g_fox_hp = 3;
    g_bunny_hp = 3;
    g_falcon_hp = 3;
    g_frog_hp = 3;
    g_pepper_hp = 3;
    g_andross_hp = 3;

    // CONTINUE/message state defaults
    g_whichfriend = FRIEND_FOX;
    g_friends_msg = 0;
    g_msg_count1 = 0;
    g_msg_count2 = 0;
    g_friends_sound = 2;  // othersnd
    g_friends_meter = 0;
    g_playerscore = 0;
    g_numendok = 0;

    // Sound
    g_lsnd = 0;
    g_csnd = 0;
    g_rsnd = 0;
    g_msnd = 0;
    g_fsnd = 0;
    g_playersndflag = 0;
    g_lastplayx = 0;
    g_sdspt3 = 0;
    g_sdgpt3 = 0;
    g_sdpck3 = 0;
    g_pausesnd = 0;
    memset(g_sdport3, 0, sizeof(g_sdport3));
    g_lastblock = 0;
    g_nosetport3 = 0;
    g_port1bolox = 0;
    g_inatunnel = 0;

    // Camera / view rotation
    g_outvx = 0;
    g_outvy = 0;
    g_outdist = OUTVIEWDIST;
    g_bg2scroll = 0;
    g_lastrot = 0;
    g_viewtype = VIEWTYPE_NORM;
    g_bg2Yscroll = 0;
    g_bg2Xscroll = 0;
    g_plrotx = 0;
    g_plroty = 0;

    // Transfer/debug state
    g_freezestrats = 0;
    g_timeuntilfade = 0;
    g_oncewipe = 1;
    g_circleanim = 0;
    g_windowmode = 0;
    g_fadedir = 0;
    memset(g_windowarray, 0, sizeof(g_windowarray));

    // Map state
    g_mapvar1 = 0;
    g_mapvar2 = 0;
    g_mapvar3 = 0;
    g_mapvar4 = 0;
    g_mapcnt = 0;
    g_mapptr = 0;
    g_stagecnt = 0;
    g_dotsflag = 0;
    g_othmusic = 0;
    g_maprestart = 0;
    g_maprestarttemp = 0;
    g_restartbg = 0;
    g_restartpalfade = 0;
    g_lastpalfade = 0;
    g_maprestartbank = 0;
    g_maprestartbanktemp = 0;
    g_eroll1 = 0;
    g_ebyte3 = 0;
    g_bossmaxhp = 0;
    g_meters = 0;
}
