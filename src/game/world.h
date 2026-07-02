// WORLD.ASM → C conversion
// Map script executor — the distance-driven event system that spawns all
// objects in Star Fox levels.
//
// Decompiled from WORLD.ASM: update_objects_l, newobjex, and all map opcodes.
//
// Star Fox levels are bytecode scripts: as the player moves forward (Z axis),
// a countdown (mapcnt) decrements. When it reaches zero, the executor runs
// opcodes: spawn objects, set variables, loop, branch, call subroutines, etc.
//
// On SNES, map scripts were assembled from macros in MAPMACS.INC and lived
// in ROM banks. Here, level data is C byte arrays loaded from extracted assets
// or compiled from level definitions.

#ifndef STARFOX_GAME_WORLD_H
#define STARFOX_GAME_WORLD_H

#include "../types.h"
#include "obj.h"

// --- Map Opcode Constants (from MAPMACS.INC) ---
// These are the bytecode opcode values. Each opcode is a byte value
// that indexes into a dispatch table (value/2 = index).
#define MAP_OP_MAPOBJ       0   // Spawn object: frame(2),x(2),y(2),z(2),shape(1),strat(1)
#define MAP_OP_END          2   // End level script
#define MAP_OP_LOOP         4   // Loop: addr(2),count(2)
#define MAP_OP_DEBUG        6   // Debug breakpoint
#define MAP_OP_NOP          8   // No operation
#define MAP_OP_MOTHER      10   // Spawn mother ship
#define MAP_OP_REMOVE      12   // Remove aliens by shape
#define MAP_OP_SETSTAGE    14   // Set stage timer
#define MAP_OP_SETBG       16   // Set background
#define MAP_OP_WAIT        18   // Wait by distance: distance(2)
#define MAP_OP_SETBGM      20   // Set background music
#define MAP_OP_NODOTS      22   // Disable dots/particles
#define MAP_OP_GNDDOTS     24   // Enable ground dots
#define MAP_OP_SPACEDUST   26   // Enable space dust
#define MAP_OP_SETOTHMUS   28   // Set other music
#define MAP_OP_VOFSON      30   // Vertical offset on
#define MAP_OP_VOFSOFF     32   // Vertical offset off
#define MAP_OP_HOFSON      34   // Horizontal offset on
#define MAP_OP_HOFSOFF     36   // Horizontal offset off
#define MAP_OP_OBJZROT     38   // Object with Z rotation
#define MAP_OP_JSR         40   // Call subroutine: addr(2),bank(1)
#define MAP_OP_RTS         42   // Return from subroutine
#define MAP_OP_IF          44   // Conditional: func(3),else_addr(2)
#define MAP_OP_GOTO        46   // Jump: addr(2),bank(1)
#define MAP_OP_SETXROT     48   // Set X rotation on last object
#define MAP_OP_SETYROT     50   // Set Y rotation
#define MAP_OP_SETZROT     52   // Set Z rotation
#define MAP_OP_SETALVARB   54   // Set alien var byte: offset(2),value(1)
#define MAP_OP_SETALVARW   56   // Set alien var word: offset(2),value(2)
#define MAP_OP_SETALVARL   58   // Set alien var long: offset(2),value(2),high(1)
#define MAP_OP_SETALXVARB  60   // Set alien extended var byte
#define MAP_OP_SETALXVARW  62   // Set alien extended var word
#define MAP_OP_SETALXVARL  64   // Set alien extended var long
#define MAP_OP_FADEUP      66   // Fade to full brightness
#define MAP_OP_FADEDOWN    68   // Fade to black
#define MAP_OP_SETALVARPB  70   // Set alien var via pointer byte
#define MAP_OP_SETALVARPW  72   // Set alien var via pointer word
#define MAP_OP_SETVAROBJ   74   // Store object ptr to external var
#define MAP_OP_WAITFADE    76   // Wait for fade to complete
#define MAP_OP_QFADEUP     78   // Quick fade up
#define MAP_OP_QFADEDOWN   80   // Quick fade down
#define MAP_OP_SCREENOFF   82   // Screen off
#define MAP_OP_SCREENON    84   // Screen on
#define MAP_OP_ZROTOFF     86   // Disable Z rotation
#define MAP_OP_ZROTON      88   // Enable Z rotation
#define MAP_OP_SPECIAL     90   // Mark object as special/boss
#define MAP_OP_SETVARB     92   // Set external var byte
#define MAP_OP_SETVARW     94   // Set external var word
#define MAP_OP_SETVARL     96   // Set external var long
#define MAP_OP_SETBGSLOW   98   // Set background slowly
#define MAP_OP_WAITSETBG  100   // Wait for background set
#define MAP_OP_SETBGINFO  102   // Set background info
#define MAP_OP_ADDALVARPB 104   // Add to alien var via pointer byte
#define MAP_OP_ADDALVARPW 106   // Add to alien var via pointer word
#define MAP_OP_FADETOSEA  108   // Fade to sea palette
#define MAP_OP_FADETOGROUND 110 // Fade to ground palette
#define MAP_OP_QOBJ       112   // Quick object spawn (compact)
#define MAP_OP_OBJ8       114   // Object type 8 (compact coords)
#define MAP_OP_DOBJ       116   // Double object
#define MAP_OP_QOBJ2      118   // Quick object variant (shape from strat)
#define MAP_OP_CODE65816  120   // Execute inline native map callback
#define MAP_OP_CODEJSL    122   // JSL-style native callback
#define MAP_OP_JMPVARLESS 124   // Jump if var < value
#define MAP_OP_JMPVARMORE 126   // Jump if var > value
#define MAP_OP_JMPVAREQ   128   // Jump if var == value
#define MAP_OP_SENDMSG    130   // Send message
#define MAP_OP_CSPECIAL   132   // Mark object as C-special
#define MAP_OP_NORMOBJ    134   // Normal object spawn (full ptrs)
#define MAP_OP_RESERVED   136   // Not implemented
#define MAP_OP_WAIT2      138   // Quick wait (byte << 4)
#define MAP_OP_SETPATH    140   // Set path for object

// Number of opcodes
#define MAP_NUM_OPCODES   71    // (140/2 + 1)

// JSR stack depth
#define MAP_JSR_STACK_SIZE 16

// Loop tracking
#define MAP_MAX_LOOPS      4

// --- Map Script Data ---
// In the HD build, each level's map data is a flat byte array.
// mapptr is an index into this array.
typedef struct {
    const uint8 *data;     // Map script bytecode
    uint32       length;   // Length of bytecode
} MapScript;

// --- Strategy Table Entry ---
// The SNES istrats[] table maps strategy IDs to function pointers.
// In the HD build, we use a C table of StrategyFunc pointers.
typedef struct {
    StrategyFunc func;     // Strategy function pointer
    uint16       shape_id; // Associated shape (for compact formats like dobj/qobj2)
} StratEntry;

// Native map callback used by mapif/mapcodejsl opcodes.
// Return value models 65816 carry (true = carry set).
typedef bool (*WorldNativeFunc)(Alien *last_obj);
typedef void (*WorldInlineMapFunc)(uint16 *mapptr, Alien *last_obj);

// Flat-memory callback keys used for literal mapif/mapcodejsl porting.
// These are HD runtime IDs, not original SNES ROM addresses.
#define MAP_CB_CHKSTAGEDONE         0x010001u
#define MAP_CB_CHKSTRATDONE1        0x010002u
#define MAP_CB_CHKSTRATDONE2        0x010003u
#define MAP_CB_CHKBOSSDEAD          0x010004u
#define MAP_CB_THEENDDEAD           0x010005u

#define MAP_CB_INITBLACK_L          0x010101u
#define MAP_CB_SETCHARMAPFROMMAP_L  0x010102u
#define MAP_CB_INITFADEWHITE2NORM_L 0x010103u
#define MAP_CB_KILL_ROBOT_L         0x010104u
#define MAP_CB_CLEARMAP_L           0x010105u
#define MAP_CB_CLEARREALOBJMAP_L    0x010106u
#define MAP_CB_SETRESTART_L         0x010107u
#define MAP_CB_MARKBOSS_L           0x010108u

#define MAP_CB_FROG_ALIVE           0x010201u
#define MAP_CB_BUNNY_ALIVE          0x010202u
#define MAP_CB_COCK_ALIVE           0x010203u

#define MAP_CB_CLFRIENDMSG_FROG     0x010211u
#define MAP_CB_CLFRIENDMSG_BUNNY    0x010212u
#define MAP_CB_CLFRIENDMSG_COCK     0x010213u

#define MAP_CB_SET_PLAYER_EXITBASE_L 0x010301u
#define MAP_CB_SET_PLAYER_ONPLANET_L 0x010302u
#define MAP_CB_SET_PLAYER_CLEARDEMO_L 0x010303u
#define MAP_CB_SET_PLAYER_WARP_L    0x010304u
#define MAP_CB_SET_PLAYER_CLEAR_EARTH_L 0x010305u
#define MAP_CB_SET_PLAYER_CLEAR_CHASE_L 0x010306u
#define MAP_CB_SET_PLAYER_CLEAR_SHIP2_L 0x010307u
#define MAP_CB_SET_PLAYER_CLEAR_UNDER_L 0x010308u
#define MAP_CB_SET_PLAYER_DIVE_L        0x010309u
#define MAP_CB_SET_PLAYER_CLEAR_BRIDGE_L 0x01030Au
#define MAP_CB_SET_PLAYER_CLEAR_TURN_L  0x01030Bu
#define MAP_CB_SET_PLAYER_WARPOUT_L     0x01030Cu
#define MAP_CB_SET_PLAYER_ONWATER_L     0x01030Du

#define MAP_CB_IS_PLAYER_DEAD       0x010401u
#define MAP_CB_PLAYER_OUTVIEW_L     0x010402u
#define MAP_CB_LEVELFINISHED_ZERO   0x010403u

#define MAP_CB_BG_1_4B_1_L          0x010501u
#define MAP_CB_BLOCKSND_L           0x010502u

// Bounded MAPMACS.INC space-bar runtime ids used by upcoming literal maps.
#define MAP_ISTRAT_SPACEBAR         166u
#define MAP_ISTRAT_SPINSPACEBAR     167u
#define MAP_ISTRAT_SPACEBAR1        168u
#define MAP_ISTRAT_SPACEBAR3        169u

// --- Map Executor State ---
// These are the public API entry points.

// Initialize the map executor (clear state, set default map pointer)
void World_Init(void);

// Load a level's map script data
void World_LoadLevel(const uint8 *map_data, uint32 map_length);

// Called from Obj_RunStrategies (update_objects_l):
// Tracks player Z movement, decrements mapcnt, triggers newobjex when ready.
void World_UpdateObjects(void);

// Register a native callback by 24-bit script address (bank<<16 | addr16).
// Used by MAP_OP_IF and MAP_OP_CODEJSL literal behavior.
void World_RegisterNativeCallback(uint32 addr24, WorldNativeFunc fn);
void World_RegisterInlineMapCode(uint16 script_ptr, WorldInlineMapFunc fn);

// Register strategy function mapping by 24-bit strategy address.
// Used by pointer-style map opcodes like MAP_OP_OBJ8.
void World_RegisterStrategyAddress(uint32 addr24, StrategyFunc fn);
StrategyFunc World_FindStrategyAddress(uint32 addr24);

// Strategy and shape lookup tables
// These are populated during init from extracted ROM data or compiled tables.
extern StrategyFunc g_istrats[];       // Strategy lookup table
extern uint16       g_istrat_shapes[]; // Shape associated with each strategy
extern uint16       g_shapes_table[];  // Shape lookup table (shape_id → internal)

// Map state (accessible from game_vars for save/restore)
extern int16  g_lastplayz;     // Last player Z position
extern int16  g_lastzchange;   // Z delta this frame
extern uint16 g_lastmapobj;    // Index of last spawned map object (actually Alien*)
extern uint8  g_specialobjtotal; // Count of special/boss objects
extern uint8  g_levelfinished; // Level finished flag

// levelfinished states used by SOUND.ASM dosounds_l gating.
#define LE_BHOLE1      11
#define LE_BHOLE2      12
#define LE_BHOLE3      13
#define LE_ENTERBHOLE  15
#define LE_ENTERSPEC   16

// Map special markers stored on al_sflags4 in HD runtime.
#define ASF4_SPECIAL   0x40
#define ASF4_CSPECIAL  0x80

#endif // STARFOX_GAME_WORLD_H
