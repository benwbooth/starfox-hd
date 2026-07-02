#ifndef STARFOX_VARIABLES_H
#define STARFOX_VARIABLES_H

// Star Fox RAM variable definitions
// Extracted from UltraStarFox INC/VARS.INC, GILESALC.INC, STRUCTS.INC
//
// The original game allocates variables sequentially in SNES RAM.
// We use proper C globals (in game_vars.h) instead of g_ram[] offsets.
// g_ram[] is retained only for SNES hardware emulation in sf_rtl.c.

#include "sf_rtl.h"

// --- Helper macros for raw g_ram access (used sparingly) ---
#define RAM8(off)    (g_ram[(off)])
#define RAM16(off)   (*(uint16 *)&g_ram[(off)])
#define RAM16S(off)  (*(int16 *)&g_ram[(off)])

// ============================================================
// Angle Constants (from VARS.INC)
// SNES uses 0-255 = 0-360 degrees
// ============================================================
#define DEG360   256
#define DEG180   128
#define DEG90    64
#define DEG45    32
#define DEG22    16
#define DEG11    8
#define DEG5     4
#define DEG270   192
#define DEG0     0
#define DEG120   85
#define DEG60    42
#define DEG240   170
#define DEG300   212
#define DEG135   96
#define DEG225   160
#define DEG315   224
#define DEG72    51
#define DEG144   102
#define DEG216   153
#define DEG288   204

// ============================================================
// Game Constants (from VARS.INC)
// ============================================================
#define WALLHGT           120
#define WALLDIST           70
#define NUMBER_AL_MAX      70
#define NUMBER_ZB_MAX      70
#define NUM_ROUTES_MAX     4
#define SPACE_MODE         1
#define WATER_MODE         2

// Screen dimensions
#define NUM_COL            28
#define NUM_ROW            24

// Depth/LOD
#define MAXDEPTHDIST       0x0E00

// Explosion sizes
#define EXPSIZE_SMALL      64
#define EXPSIZE_MEDIUM     128
#define EXPSIZE_LARGE      256

// ============================================================
// Alien Flags — al_flags field (from VARS.INC)
// ============================================================
#define AFEXP             1    // Exploding
#define AF_INRNG_PL       2    // In range of player
#define AF_LEFT_PL        4    // Left of player
#define AF_FRONT_PL       8    // In front of player
#define AF_INVIEW_PL     16    // In view of player
#define AFHIT            32    // Was hit
#define AFONFIRE         64    // On fire

// ============================================================
// Alien Type Flags — al_type field (from VARS.INC)
// ============================================================
#define ATGND             1    // Ground object
#define ATMISSILE         2    // Is missile
#define ATLASER           4    // Is laser bolt
#define ATZREMOVE         8    // Remove when behind
#define ATNUKED          16    // Hit by nuke explosion

// ============================================================
// Game Flags — gameflags (from GILESALC.INC)
// ============================================================
#define GF_NOZREMOVE      1
#define GF_PLAYERDYING    2
#define GF_BOSSDEAD       4
#define GF_STRATDONE1     8
#define GF_STRATDONE2    16
#define GF_VIEWROT       32
#define GF_PLAYERDEAD    64
#define GF_STAGEDONE    128

// ============================================================
// Game Flags 2 — gameflags2 (from GILESALC.INC)
// ============================================================
#define GF2_STRATFLAG1    1
#define GF2_STRATFLAG2    2
#define GF2_VIEWCLOSE     4
#define GF2_INGAME        8

// ============================================================
// Strategy Flags — stratflags (from GILESALC.INC)
// ============================================================
#define SF_NOFIRING       1

// ============================================================
// Boss Flags — bossflags (from GILESALC.INC)
// ============================================================
#define BF_FLAG1          1
#define BF_FLAG2          2
#define BF_FLAG3          4
#define BF_EASYMODE       8
#define BF_DYING         16

// ============================================================
// Player Ship Flags — pshipflags (from GILESALC.INC)
// ============================================================
#define PSF_BODYCOLL      1
#define PSF_LWINGCOLL     2
#define PSF_RWINGCOLL     4
#define PSF_BRKLWING      8
#define PSF_BRKRWING     16
#define PSF_NOCTRL       32
#define PSF_NOFIRE       64
#define PSF_NOYCTRL     128

// ============================================================
// Player Ship Flags 2 — pshipflags2 (from GILESALC.INC)
// ============================================================
#define PSF2_DOUBLASER    1
#define PSF2_WIRESHIP     2
#define PSF2_NOSPARK      4
#define PSF2_TURN180      8
#define PSF2_FORCEBOOST  16
#define PSF2_BOOSTING    32
#define PSF2_BRAKING     64
#define PSF2_PLAYERHP0  128

// ============================================================
// Player Ship Flags 3 — pshipflags3 (from GILESALC.INC)
// ============================================================
#define PSF3_INTUNNEL     1
#define PSF3_ENGINESND    2
#define PSF3_FORCEBRAKE   4
#define PSF3_NOCOLLISIONS 8
#define PSF3_BEAMBALL    16
#define PSF3_NOVIEWCHANGE 32
#define PSF3_KEEPPSTRAT  64

// ============================================================
// Player Fly Mode — playerflymode (from GILESALC.INC)
// ============================================================
#define PFM_DIEFALL       1
#define PFM_DIEYROT       2
#define PFM_WATER         4
#define PFM_SHADOWS       8
#define PFM_WOBBLE       16

// ============================================================
// Space Player Fly Mode — splayerflymode (from GILESALC.INC)
// ============================================================
#define SPFM_NORM         0
#define SPFM_CLOSE        1
#define SPFM_TOINSIDE     2
#define SPFM_INSIDE       3
#define SPFM_TONORM       4
#define SPFM_MAXMODE      5

// ============================================================
// Player Strategy Flags — pstratflags (from GILESALC.INC)
// ============================================================
#define PSTF_NOVDISTC     1
#define PSTF_FLAG1        2
#define PSTF_NOVIEWMOVE   4
#define PSTF_INSEQ        8
#define PSTF_FIRSTFRAMELCOL 16
#define PSTF_NOTDIE      32

// ============================================================
// Player Move Limits — pmovelimit (from GILESALC.INC)
// ============================================================
#define PML_LWLEFT        1
#define PML_RWRIGHT       2
#define PML_LWTOP         4
#define PML_LWBOTTOM      8
#define PML_RWTOP        16
#define PML_RWBOTTOM     32
#define PML_BTOP         64
#define PML_BBOTTOM     128
#define PML_ALL          (PML_LWTOP|PML_RWTOP|PML_LWBOTTOM|PML_RWBOTTOM|PML_LWLEFT|PML_RWRIGHT|PML_BTOP|PML_BBOTTOM)

// ============================================================
// Missile Boundary Flags — missboundflags (from GILESALC.INC)
// ============================================================
#define MB_LEFT           1
#define MB_RIGHT          2
#define MB_TOP            4
#define MB_BOTTOM         8
#define MB_LBOTTOM       16
#define MB_LTOP          32
#define MB_RTOP          64

// ============================================================
// Key Flags — keyflags (from GILESALC.INC)
// ============================================================
#define KF_LKEYDOWN       1
#define KF_LRKEYDOWN      2

// ============================================================
// Screen Arrow Flags — arrows (from KALCS.INC)
// ============================================================
#define SPRAR_UP          1
#define SPRAR_DOWN        2
#define SPRAR_LEFT        4
#define SPRAR_RIGHT       8

// ============================================================
// View Type (from STRATEQU.INC)
// ============================================================
#define VIEWTYPE_NORM     0
#define VIEWTYPE_TOOBJ    1
#define VIEWTYPE_FPOS     2

// ============================================================
// Weapon Flags (from STRATEQU.INC)
// ============================================================
#define WP_FIRE           1
#define WP_FIXPOS         2
#define WP_SPEEDCHG       4

// ============================================================
// Player Ship Numbers (from STRATEQU.INC)
// ============================================================
#define PSHIPNUM_NORM     0
#define PSHIPNUM_WIRE     1
#define PSHIPNUM_NULL     2
#define PSHIPNUM_COCKSHIP 3
#define PSHIPNUM_TUNNEL   4
#define PSHIPNUM_BLACK    5
#define PSHIPNUM_ZOOM     6

// ============================================================
// Character IDs (from VARS.INC)
// ============================================================
#define FRIEND_FOX        0
#define FRIEND_RABBIT     1
#define FRIEND_FALCON     2
#define FRIEND_FROG       3
#define FRIEND_PEPPER     4
#define FRIEND_ANDROSS    5
#define FRIEND_ANYONE     6

// ============================================================
// Machine Flags (from VARS.INC)
// ============================================================
#define MACFEXISTS        1
#define MACFRELAXIS       2

// ============================================================
// Draw List Structure Offsets (from STRUCTS.INC)
// Layout at $700000 (SRAM), 30 bytes per entry
// ============================================================
#define DL_SIZEOF         30
#define DL_NEXT_OFF       0    // 2 bytes
#define DL_SORTZ_OFF      2    // 2 bytes
#define DL_ROTX_OFF       4    // 1 byte
#define DL_ROTY_OFF       5    // 1 byte
#define DL_ROTZ_OFF       6    // 1 byte
#define DL_SFLAGS_OFF     7    // 1 byte
#define DL_SHAPE_OFF      8    // 2 bytes
#define DL_SHADY_OFF     10    // 2 bytes
#define DL_SHADX_OFF     12    // 2 bytes
#define DL_SHADZ_OFF     14    // 2 bytes
#define DL_Y_OFF         16    // 2 bytes
#define DL_X_OFF         18    // 2 bytes
#define DL_Z_OFF         20    // 2 bytes
#define DL_COLTAB_OFF    22    // 2 bytes
#define DL_EXPCNT_OFF    24    // 1 byte
#define DL_ANIMFRAME_OFF 25    // 1 byte
#define DL_COLFRAME_OFF  26    // 1 byte
#define DL_DEPTH_OFF     27    // 1 byte
#define DL_TSCROLLX_OFF  28    // 1 byte
#define DL_TSCROLLY_OFF  29    // 1 byte

// ============================================================
// Collision List Structure (from STRUCTS.INC)
// ============================================================
#define CL_SIZEOF         12   // cl_size - collisttemp
#define CL_ALIEN_OFF      0    // 2 bytes
#define CL_COLBOX_OFF     2    // 2 bytes
#define CL_XMAX_OFF       4    // 2 bytes
#define CL_YMAX_OFF       6    // 2 bytes
#define CL_ZMAX_OFF       8    // 2 bytes

// ============================================================
// Game Configuration Defaults (from CONFIG/GAME.INC)
// ============================================================
#define DEFAULT_LIVES     3
#define BARREL_ROLL_DELAY 3

// Gameplay constants from STRATEQU.INC
#define CROSSHAIRDIST     500
#define INVIEWDIST        60
#define OUTVIEWDIST       120
#define FRAMESPERAP       10
#define DEFAULT_NUKE_COUNT 3
#define SPECIAL_DELAY_FRMS 50
#define PLAYER_FIRESPEED   2

#endif // STARFOX_VARIABLES_H
