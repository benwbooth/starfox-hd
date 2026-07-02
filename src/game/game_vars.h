#ifndef STARFOX_GAME_VARS_H
#define STARFOX_GAME_VARS_H

// Game state variables — decompiled from GILESALC.INC allocations
// These were originally sequential RAM allocations via the `alc` macro.
// We use proper C globals instead of raw g_ram[] offsets.

#include "../types.h"

// ============================================================
// Game Flags (single bytes, accessed via bit masks)
// ============================================================
extern uint8 g_gameflags;       // GF_* flags
extern uint8 g_gameflags2;      // GF2_* flags
extern uint8 g_stratflags;      // SF_* flags
extern uint8 g_bossflags;       // BF_* flags
extern uint8 g_sflags;          // Strategy flags (general)
extern uint8 g_svar_flags;      // Strategy variable flags

// ============================================================
// Player Ship Flags
// ============================================================
extern uint8 g_pshipflags;      // PSF_* flags
extern uint8 g_pshipflags2;     // PSF2_* flags
extern uint8 g_pshipflags3;     // PSF3_* flags
extern uint8 g_pstratflags;     // PSTF_* flags
extern uint8 g_playerflymode;   // PFM_* flags
extern uint8 g_splayerflymode;  // SPFM_* mode value
extern uint8 g_splayerflymodeopt;
extern uint8 g_pmovelimit;      // PML_* flags
extern uint8 g_pmovelimitAND;
extern uint8 g_missboundflags;  // MB_* flags
extern uint8 g_keyflags;        // KF_* flags
extern uint8 g_arrows;          // SPRAR_* HUD/limit arrows
extern uint8 g_doingwipe;       // Map/transition wipe in progress
extern int8  g_stayblack;       // -1 means normal gameplay control path
extern uint8 g_specials_dead;

// ============================================================
// Player State
// ============================================================
extern int16  g_playervelZ;
extern int16  g_player_speed;
extern uint8  g_player_tospeed;
extern uint8  g_player_medspeed;
extern int16  g_player_posx;
extern int16  g_player_posy;
extern int16  g_player_posz;
extern int16  g_player_turnrot;
extern int16  g_playerdieYrotspeed;
extern uint8  g_player_rolldelay;
extern uint8  g_player_rollZvel;
extern uint8  g_player_rollZoff;
extern int16  g_player_zshake;
extern int16  g_player_zshakeV;
extern uint8  g_player_Ztilt;
extern uint8  g_player_zstratadd;
extern uint8  g_player_noctrlcnt;
extern uint8  g_curr_ship;       // Current ship shape index (PSHIPNUM_*)

// Player shape IDs (xalc — stored separately for wing damage states)
extern uint16 g_playershape;     // Normal
extern uint16 g_playershapeL;    // Left wing broken
extern uint16 g_playershapeR;    // Right wing broken
extern uint16 g_playershapeLR;   // Both wings broken

// ============================================================
// View / Camera (from GILESALC.INC)
// ============================================================
extern int16  g_pviewposx;      // Player view position X
extern int16  g_pviewposy;      // Player view position Y
extern int16  g_pviewposz;      // Player view position Z
// Final camera position (GAME.ASM viewposx/y/z). In VIEWTYPE_NORM this is
// derived from pviewpos each frame by getview; in VIEWTYPE_FPOS strategies
// (viewopening, playerExitBase, ...) drive it directly.
extern int16  g_viewposx;
extern int16  g_viewposy;
extern int16  g_viewposz;
extern int16  g_pviewvelz;      // View velocity Z
extern int16  g_pviewposzoff;   // View position Z offset
extern int16  g_viewfloatptr;   // Float bob table pointer
extern int16  g_viewfloatX;     // Float bob X
extern int16  g_viewfloatY;     // Float bob Y
extern int16  g_viewCY;         // View center Y
extern int16  g_viewdist;       // View distance
extern int16  g_viewaddx;       // View add X
extern int16  g_viewaddy;       // View add Y
extern int16  g_viewtoobj;      // View-to-object reference
extern int16  g_viewposXoff;    // View position X offset
extern int16  g_viewposYoff;    // View position Y offset
extern uint8  g_viewshakeX;     // View shake X
extern uint8  g_viewshakeY;     // View shake Y
extern uint8  g_viewshakeZ;     // View shake Z

// ============================================================
// Counters / Timing
// ============================================================
extern uint16 g_gameframe;      // Global game frame counter
extern uint8  g_framerate;
extern uint8  g_superspeedcnt;
extern uint8  g_superspeeddelay;

// ============================================================
// Weapon / Combat
// ============================================================
extern uint8  g_numplasers;     // Number of active player lasers
extern uint8  g_firecnt;        // Fire counter
extern uint8  g_firedelay;      // Fire delay
extern uint8  g_specialdelay;   // Special weapon delay
extern uint8  g_screenflashcnt;
extern uint8  g_screenflashtype;
extern uint8  g_pnumhits;       // Player hit count
extern uint16 g_specwepcnt;     // Nova bomb inventory
extern uint8  g_boostcnt;       // Boost/brake SFX latch counter

// Weapon position (for fire origin)
extern int16  g_svar_weapx;
extern int16  g_svar_weapy;
extern int16  g_svar_weapz;
extern uint8  g_svar_weapRy;
extern uint8  g_svar_weapRx;
extern int16  g_svar_weapobj;

// ============================================================
// Strategy Object State
// ============================================================
extern int16  g_stratobj_posx;
extern int16  g_stratobj_posy;
extern int16  g_stratobj_posz;
extern int16  g_playstrat_cnt;

// Strategy variables (shared temporaries used by various strategies)
extern int16  g_svar_word1;
extern int16  g_svar_word2;
extern int16  g_svar_word3;
extern int16  g_svar_dist;
extern uint8  g_svar_byte1;
extern uint8  g_svar_byte2;
extern uint8  g_svar_byte3;
extern uint8  g_svar_byte4;
extern uint8  g_svar_byte5;
extern uint8  g_svar_byte6;

// Map strategy variables
extern int16  g_smvar_byte1;    // 2-byte despite name (from ASM)
extern int16  g_smvar_byte2;
extern int16  g_smvar_byte3;
extern int16  g_smvar_word1;
extern int16  g_smvar_word2;
extern int16  g_smvar_word3;
extern int16  g_smvar_word4;
extern int16  g_swvar_word1;

// Global strategy variables
extern uint8  g_gsvar_byte1;
extern uint8  g_gsvar_byte2;

// Map trigger byte — set by boss/strategy code, read by map inline code
extern uint8  g_maptrigger;

// Player strategy variables
extern uint8  g_psvar_byte1;
extern uint8  g_psvar_byte2;
extern uint8  g_psvar_byte3;
extern int16  g_psvar_word1;
extern int16  g_psvar_word2;
extern int16  g_psvar_word3;
extern int16  g_psvar_word4;

// ============================================================
// Player Movement Bounds
// ============================================================
extern int16  g_minpmoveX;
extern int16  g_maxpmoveX;
extern int16  g_minpmoveY;
extern int16  g_maxpmoveY;
extern int16  g_minMmoveX;      // Mario/view move bounds
extern int16  g_maxMmoveX;
extern int16  g_maxMmoveY;
extern int16  g_minpWmoveY;     // Water mode bounds
extern int16  g_maxpWmoveY;

// ============================================================
// Collision Objects
// ============================================================
extern int16  g_pcollobj_LW;    // Player collision object — left wing
extern int16  g_pcollobj_RW;    // Right wing
extern int16  g_pcollobj_B;     // Body
extern int16  g_pcboxobj_LW;    // Player collision box — left wing
extern int16  g_pcboxobj_RW;
extern int16  g_pcboxobj_B;

// ============================================================
// Missile Bounds
// ============================================================
extern int16  g_missbTOPLEFT;
extern int16  g_missbTOPRIGHT;
extern int16  g_missbBOTLEFT;
extern int16  g_missbBOTRIGHT;

// ============================================================
// Misc
// ============================================================
extern uint8  g_slowstars;
extern int16  g_hudrot;
extern int16  g_fobj;           // Follow object
extern uint8  g_nomaxbg2Yscroll;
extern int16  g_slowstars_viewposz;
extern int16  g_bgsscrollZ;     // Background scroll Z
extern int16  g_boostobj;       // Boost object reference
extern uint8  g_boostZoff;
extern int16  g_pwingdistwall;
extern int16  g_rangexy;
extern uint8  g_wireendflash;
extern uint8  g_gasflags;
extern int16  g_wpposZ;
extern uint8  g_maxstate;
extern uint8  g_slimecount;
extern uint8  g_freezestrats;   // Debug/engine flag: freeze strategy update when bit 0 set
extern int16  g_internalPLAYPT;
extern uint8  g_timeuntilfade;
extern uint8  g_oncewipe;       // WINDOWS.ASM one-shot wipe latch
extern int16  g_circleanim;     // Active circle/wipe animation id
extern uint8  g_floatvar1;
extern uint8  g_floatvar2;
extern int16  g_dummyobj;
extern int16  g_player_Zrotfloatptr;
extern uint8  g_player_Zrotfloat;
extern uint8  g_trotx;
extern uint8  g_troty;
extern uint8  g_lastcont0;
extern uint8  g_lastcontl0;
extern int16  g_svar_collx;
extern int16  g_svar_colly;
extern int16  g_svar_collz;
extern int16  g_tpa;            // Temporary accumulator

// ============================================================
// Camera / View Rotation State (from ALCS.INC / GAME.ASM)
// ============================================================
extern int16  g_outvx;          // View rotation velocity X
extern int16  g_outvy;          // View rotation velocity Y
extern int16  g_outdist;        // View distance (camera behind player)
extern int16  g_bg2scroll;      // BG2 scroll value
extern int16  g_lastrot;        // Last rotation value
extern uint8  g_viewtype;       // View type (VIEWTYPE_*)
extern int16  g_bg2Yscroll;     // BG2 Y scroll offset
extern int16  g_bg2Xscroll;     // BG2 X scroll offset
extern int16  g_plrotx;         // Player rotation X (16-bit, 8.8)
extern int16  g_plroty;         // Player rotation Y (16-bit, 8.8)

// ============================================================
// Window / Fade State (WINDOWS.ASM)
// ============================================================
#define WINDOWARRAY_SIZE        8

#define WINDOW_MODE_NONE        0
#define WINDOW_MODE_BLACK       1
#define WINDOW_MODE_WHITEFADE   2
#define WINDOW_MODE_WHITE2NORM  3
#define WINDOW_MODE_MAPFADE     4

typedef struct WindowState {
    uint8 mode;
    uint8 wm_val;
    uint8 stayblack;
    uint8 reserved;
} WindowState;

extern uint8 g_windowmode;                      // Bitmask of allocated window slots
extern WindowState g_windowarray[WINDOWARRAY_SIZE];
extern int8  g_fadedir;                         // WORLD.ASM fade direction: -2,-1,0,1,2

// ============================================================
// Level / Map State
// ============================================================
extern uint16 g_mapvar1;        // Map execution variables
extern uint16 g_mapvar2;
extern uint16 g_mapvar3;
extern uint16 g_mapvar4;
extern uint16 g_mapcnt;         // Map counter
extern uint16 g_mapptr;         // Map pointer
extern int16  g_stagecnt;       // HUD stage counter (setstage opcode)
extern int16  g_dotsflag;       // Dot/starfield mode (-1,0,1)
extern uint8  g_othmusic;       // Secondary music selector
extern uint16 g_maprestart;     // Restart checkpoint map pointer
extern uint16 g_maprestarttemp; // Pending gate checkpoint map pointer
extern uint16 g_restartbg;      // Restart checkpoint background id
extern uint16 g_restartpalfade; // Restart checkpoint palette fade
extern uint16 g_lastpalfade;    // Last applied palette fade
extern uint8  g_maprestartbank;     // Flat-memory restart bank placeholder
extern uint8  g_maprestartbanktemp; // Flat-memory pending restart bank placeholder
extern uint8  g_eroll1;             // Gate checkpoint latch shared with path scripts
extern uint8  g_ebyte3;             // Path-exported byte 3 (fog loop break latch)

// Boss HUD / meter state mirrored from MARIO work RAM.
extern uint16 g_bossmaxhp;
extern uint16 g_meters;

// ============================================================
// Game Mode / Stage
// ============================================================
extern uint8  g_game_mode;      // SPACE_MODE / WATER_MODE
extern uint16 g_stage;
extern uint8  g_whichroute;
extern uint8  g_lives;
extern uint8  g_currentlevel;

// Planet select / route state (PLANETS.ASM)
extern uint16 g_routes[4];
extern int16  g_currentplanet;
extern uint16 g_mapmode;
extern uint16 g_bg_dmalist;
extern uint16 g_bgtransspeed;
extern uint16 g_currentbg;
extern uint8  g_bgflags;
extern uint32 g_newmap;
extern uint8  g_peppermsg;
extern uint16 g_nebula_on;

// ============================================================
// HP values (from MAIN.ASM)
// ============================================================
extern uint8  g_fox_hp;
extern uint8  g_bunny_hp;
extern uint8  g_falcon_hp;      // "cock" in ASM
extern uint8  g_frog_hp;
extern uint8  g_pepper_hp;
extern uint8  g_andross_hp;

// CONTINUE.ASM message state
extern uint8  g_whichfriend;
extern uint16 g_friends_msg;
extern uint8  g_msg_count1;
extern uint8  g_msg_count2;
extern uint8  g_friends_sound;
extern uint8  g_friends_meter;
extern uint16 g_playerscore;
extern uint8  g_numendok;      // KSTRATS.ASM theenddead state

// ============================================================
// Sound State
// ============================================================
extern uint8  g_lsnd;
extern uint8  g_csnd;
extern uint8  g_rsnd;
extern uint8  g_msnd;
extern uint8  g_fsnd;
extern uint8  g_playersndflag;
extern int16  g_lastplayx;
extern uint8  g_sdspt3;
extern uint8  g_sdgpt3;
extern uint8  g_sdpck3;
extern uint8  g_pausesnd;
extern uint8  g_sdport3[16];
extern uint16 g_lastblock;
extern uint8  g_nosetport3;
extern uint8  g_port1bolox;
extern uint8  g_inatunnel;

// ============================================================
// View Float Bob Table (from GSTRATS.ASM)
// Sinusoidal camera bob during forward flight
// ============================================================
extern const int16 g_viewfloattab[];
#define VIEWFLOATTAB_LEN  40   // 40 entries

// Initialize all game variables to defaults
void GameVars_Init(void);

#endif // STARFOX_GAME_VARS_H
