#ifndef STARFOX_VARIABLES_H
#define STARFOX_VARIABLES_H

// Star Fox RAM variable definitions
// Extracted from UltraStarFox INC/ALCS.INC, DALCS.INC, EALCS.INC, KALCS.INC
//
// The original game allocates variables sequentially using macros:
//   zalc = zero page ($0000-$00FF)
//   alc  = work RAM  ($0300-$1FFF)
//   xalc = extended  ($7E2000+)
//   malc = MARIO RAM ($700200+)
//
// We use the g_ram[] array and offset macros (snesrev pattern).
// These offsets are computed from the assembly allocation order.

#include "sf_rtl.h"

// --- Helper macros for accessing g_ram as typed values ---
#define RAM8(off)    (g_ram[(off)])
#define RAM16(off)   (*(uint16 *)&g_ram[(off)])
#define RAM16S(off)  (*(int16 *)&g_ram[(off)])

// ============================================================
// Zero Page Variables (zalc, base = $00)
// ============================================================

// Rendering pipeline
#define ZP_TRANS_FLAG    0x00
#define ZP_X1_FRACT      0x01
#define ZP_X1             0x02   // 2 bytes
#define ZP_X2             0x04   // 2 bytes
#define ZP_FRAMECNT       0x06
#define ZP_Y1_FRACT       0x07
#define ZP_Y1             0x08   // 2 bytes
#define ZP_Y2             0x0A   // 2 bytes
#define ZP_DX             0x0C   // 2 bytes
#define ZP_DY             0x0E   // 2 bytes

// Clipping
#define ZP_CLX1           0x30   // 2 bytes
#define ZP_CLY1           0x32   // 2 bytes
#define ZP_CLX2           0x34   // 2 bytes
#define ZP_CLY2           0x36   // 2 bytes

// Text/print
#define ZP_PRINTPT        0x44   // 2 bytes
#define ZP_DRAWMAP        0x46   // 2 bytes
#define ZP_SHOWMAP        0x48   // 2 bytes

// Rotation angles (used by 3D engine)
#define ZP_ROTX           0x80
#define ZP_ROTY           0x81
#define ZP_ROTZ           0x82

// View position
#define ZP_VIEWPOSX       0x90   // 2 bytes
#define ZP_VIEWPOSY       0x92   // 2 bytes
#define ZP_VIEWPOSZ       0x94   // 2 bytes
#define ZP_VANISHX        0x96   // 2 bytes
#define ZP_VANISHY        0x98   // 2 bytes

// Object counts
#define ZP_NUMPNTS        0xA0
#define ZP_NUMFACES       0xA1
#define ZP_NUMALS         0xA2
#define ZP_NUMDRAWN       0xA3

// Transform temporaries
#define ZP_BIGX           0xB0   // 2 bytes
#define ZP_BIGY           0xB2   // 2 bytes
#define ZP_BIGZ           0xB4   // 2 bytes

// PRNG (kept between levels)
#define ZP_RAND           0xC0   // 2 or 4 bytes depending on rngmode

// Input
#define ZP_KEYBYTE        0xD0
#define ZP_KEYBYTEL       0xD1

// ============================================================
// Work RAM Variables (alc, base = $300)
// ============================================================
#define WK_BASE           0x300

// Controller state
#define WK_CONT0L        (WK_BASE + 0x00)
#define WK_CONT0         (WK_BASE + 0x01)
#define WK_CONTL0L       (WK_BASE + 0x02)
#define WK_CONTL0        (WK_BASE + 0x03)

// World position
#define WK_WORLDX        (WK_BASE + 0x20)  // 2 bytes
#define WK_WORLDY        (WK_BASE + 0x22)  // 2 bytes
#define WK_WORLDZ        (WK_BASE + 0x24)  // 2 bytes

// Alien management
#define WK_ALLST          (WK_BASE + 0x26)  // 2 bytes - active alien list
#define WK_ALFREELST      (WK_BASE + 0x28)  // 2 bytes - free alien blocks
#define WK_SCENNUM        (WK_BASE + 0x2A)  // 2 bytes

// Player rotation
#define WK_PLROTX        (WK_BASE + 0x40)  // 2 bytes
#define WK_PLROTY        (WK_BASE + 0x42)  // 2 bytes
#define WK_PLROTZ        (WK_BASE + 0x44)  // 2 bytes

// Game status
#define WK_FLAGS          (WK_BASE + 0x50)
#define WK_TYPE           (WK_BASE + 0x51)
#define WK_ALDEAD         (WK_BASE + 0x52)
#define WK_ENEMYS         (WK_BASE + 0x53)
#define WK_ATTACKERS      (WK_BASE + 0x54)

// Mario chip interface
#define WK_MARIOCODE      (WK_BASE + 0x100)
#define WK_MARIO_DRAW_MODE (WK_BASE + 0x101)

// Rotation matrix (3x3, each element is 2 bytes: high.low)
#define WK_MAT11W        (WK_BASE + 0x120)
#define WK_MAT11         (WK_BASE + 0x121)
#define WK_MAT12W        (WK_BASE + 0x122)
#define WK_MAT12         (WK_BASE + 0x123)
#define WK_MAT13W        (WK_BASE + 0x124)
#define WK_MAT13         (WK_BASE + 0x125)
#define WK_MAT21W        (WK_BASE + 0x126)
#define WK_MAT21         (WK_BASE + 0x127)
#define WK_MAT22W        (WK_BASE + 0x128)
#define WK_MAT22         (WK_BASE + 0x129)
#define WK_MAT23W        (WK_BASE + 0x12A)
#define WK_MAT23         (WK_BASE + 0x12B)

// World matrix
#define WK_WMAT11W       (WK_BASE + 0x140)
#define WK_WMAT11        (WK_BASE + 0x141)
// ... (18 bytes total for 3x3 matrix with hi/lo pairs)

// View rotation
#define WK_VIEWROTXW     (WK_BASE + 0x170)
#define WK_VIEWROTX      (WK_BASE + 0x171)
#define WK_VIEWROTYW     (WK_BASE + 0x172)
#define WK_VIEWROTY      (WK_BASE + 0x173)
#define WK_VIEWROTZW     (WK_BASE + 0x174)
#define WK_VIEWROTZ      (WK_BASE + 0x175)

// Frame counters
#define WK_GAMEFRAME     (WK_BASE + 0x180)  // 2 bytes
#define WK_COLFRAME      (WK_BASE + 0x182)
#define WK_ANIMFRAME     (WK_BASE + 0x183)

// Scroll/background
#define WK_BG2YSCROLL    (WK_BASE + 0x200)  // 2 bytes
#define WK_BG2XSCROLL   (WK_BASE + 0x202)  // 2 bytes

// Window/fade state
#define WK_WINDOWPTR     (WK_BASE + 0x210)  // 2 bytes
#define WK_WINDOWMODE    (WK_BASE + 0x212)

// Planet/route
#define WK_ROUTES        (WK_BASE + 0x300)  // 2*4 bytes
#define WK_STAGE         (WK_BASE + 0x308)  // 2 bytes
#define WK_WHICHROUTE    (WK_BASE + 0x30A)
#define WK_LIVES         (WK_BASE + 0x30B)

// Draw list
#define WK_DLPTR         (WK_BASE + 0x400)  // 2 bytes
#define WK_NUMSHAPES     (WK_BASE + 0x402)  // 2 bytes

// Sound
#define WK_LSND          (WK_BASE + 0x500)
#define WK_CSND          (WK_BASE + 0x501)
#define WK_RSND          (WK_BASE + 0x502)
#define WK_MSND          (WK_BASE + 0x503)
#define WK_FSND          (WK_BASE + 0x504)

// Map/level
#define WK_MAPCNT        (WK_BASE + 0x600)  // 2 bytes
#define WK_MAPPTR        (WK_BASE + 0x602)  // 2 bytes
#define WK_MAPVAR1       (WK_BASE + 0x604)  // 2 bytes
#define WK_MAPVAR2       (WK_BASE + 0x606)  // 2 bytes
#define WK_MAPVAR3       (WK_BASE + 0x608)  // 2 bytes
#define WK_MAPVAR4       (WK_BASE + 0x60A)  // 2 bytes

// HP values (from MAIN.ASM)
#define WK_FOX_HP        (WK_BASE + 0x700)
#define WK_BUNNY_HP      (WK_BASE + 0x701)
#define WK_COCK_HP       (WK_BASE + 0x702)  // Falco
#define WK_FROG_HP       (WK_BASE + 0x703)
#define WK_PEPPER_HP     (WK_BASE + 0x704)

// Game mode
#define WK_GAMEMODE      (WK_BASE + 0x710)  // 2 bytes

// Credits/continue
#define WK_CREDITS       (WK_BASE + 0x720)  // 2 bytes
#define WK_WHICHFRIEND   (WK_BASE + 0x722)

// Score
#define WK_CURRENTLEVEL  (WK_BASE + 0x730)

// ============================================================
// Angle Constants (from VARS.INC)
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

// ============================================================
// Alien Flags (from VARS.INC)
// ============================================================
#define AFEXP             1    // Exploding
#define AF_INRNG_PL       2    // In range of player
#define AF_LEFT_PL        4    // Left of player
#define AF_FRONT_PL       8    // In front of player
#define AF_INVIEW_PL     16    // In view of player
#define AFHIT            32    // Was hit
#define AFONFIRE         64    // On fire

// ============================================================
// Alien Type Flags (from VARS.INC)
// ============================================================
#define ATGND             1    // Ground object
#define ATMISSILE         2    // Is missile
#define ATLASER           4    // Is laser bolt
#define ATZREMOVE         8    // Remove when behind
#define ATNUKED          16    // Hit by nuke explosion

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

#endif // STARFOX_VARIABLES_H
