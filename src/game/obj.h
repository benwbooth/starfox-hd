#ifndef STARFOX_GAME_OBJ_H
#define STARFOX_GAME_OBJ_H

#include "../types.h"

// Forward declaration for strategy function pointer
struct Alien;
typedef void (*StrategyFunc)(struct Alien *self);

// Alien (object) structure — decompiled from STRUCTS.INC al_start + alx_start
// Every 3D entity in the game: enemies, items, player wingmen, obstacles, player.
// On SNES these were contiguous blocks in RAM addressed by X/Y registers.
typedef struct Alien {
    // Linked list pointers (offset 0: _next, offset 2: _prev in ASM)
    struct Alien *next;
    struct Alien *prev;

    // --- al_start fields (STRUCTS.INC) ---
    uint16 shape;         // al_shape: Shape reference
    uint16 ptr;           // al_ptr: Attached alien pointer
    uint8  flags;         // al_flags: AFEXP, AFHIT, AF_FRONT_PL, etc.
    uint8  type;          // al_type: ATGND, ATMISSILE, ATLASER, ATZREMOVE, etc.
    uint8  count;         // al_count: Frame/explosion counter
    uint8  count1;        // al_count1: Secondary counter
    int16  worldx;        // al_worldx: World position X
    int16  worldy;        // al_worldy: World position Y
    int16  worldz;        // al_worldz: World position Z
    uint8  rotx;          // al_rotx: Rotation angle X (0-255)
    uint8  roty;          // al_roty: Rotation angle Y
    uint8  rotz;          // al_rotz: Rotation angle Z
    uint8  vel;           // al_vel: Velocity magnitude

    // --- GILESAL.INC fields (strategy data, part of al_start block) ---
    StrategyFunc stratptr; // al_stratptr: Current strategy routine
    uint16 immuneptr;     // Pointer to immunity object
    uint16 collobjptr;    // Pointer to collision object
    uint8  sflags;        // al_sflags: Strategy/display flags (asf_*)
    uint8  sflags2;       // Strategy flags 2
    uint8  sflags3;       // Strategy flags 3
    uint8  sflags4;       // Strategy flags 4
    uint8  skidy;         // Skid Y
    uint8  sbyte1;        // Temp byte 1 (also particle amount)
    uint8  sbyte2;        // Temp byte 2 (also particle life)
    uint8  sbyte3;        // Temp byte 3 (also particle type)
    uint8  sbyte4;        // Temp byte 4
    int16  sword1;        // Temp word 1
    int16  sword2;        // Temp word 2
    uint8  HP;            // al_hp: Hit points
    uint8  AP;            // al_ap: Armor points
    uint8  weapontype;    // Weapon type
    uint8  collcount;     // Collision time counter
    uint8  collflags;     // al_collflags: Collision flags (acf_*)
    int16  vx;            // Velocity X
    int16  vy;            // Velocity Y
    int16  vz;            // Velocity Z
    uint8  hitflags;      // Hit flags
    uint8  sbyte5;        // Temp byte 5
    uint8  sbyte6;        // Temp byte 6

    // --- Extended alien data (STRUCTS.INC alx_start, GILESALX.INC) ---
    int16  sWPx1, sWPy1, sWPz1;  // Weapon position
    StrategyFunc expstratptr;     // Explosion strategy
    StrategyFunc collstratptr;    // Collision strategy
    StrategyFunc endcollstratptr; // End-collision strategy
    StrategyFunc tempstratptr;    // Temporary/saved strategy
    uint8  stratstate;    // Strategy state machine state
    uint16 fireobjptr;    // Fire object pointer
    int16  depthoffset;   // al_depthoffset: Depth offset for draw list
    uint8  relposx;       // Relative position X
    uint8  relposy;       // Relative position Y
    uint8  relposz;       // Relative position Z
    uint16 debrisshape;   // Debris shape ID

    // --- STRUCTS.INC alx extended fields ---
    uint8  colframe;      // al_colframe: Color animation frame
    uint8  animframe;     // al_animframe: Animation frame
    uint8  snd1, snd2;    // Sound IDs
    uint16 coltab;        // al_coltab: Color table pointer
    uint8  childx, childy, childz;         // Child relative offsets
    uint8  childrotx, childroty, childrotz; // Child rotation
    uint16 childrotobj;   // Child rotation object
    uint8  tx, ty;        // al_tx, al_ty: Texture scroll coordinates
    uint16 memptr;        // Strategy memory pointer
    uint16 stackptr;      // Stack pointer
    uint16 stratmem;      // Strategy memory
    uint8  pbyte1, pbyte2;// Permanent bytes
    uint16 pword1;        // Permanent word

    // Active flag (not in SNES — we use this instead of checking list membership)
    bool active;
} Alien;

// Max aliens (from VARS.INC number_al)
#define NUMBER_AL 70

// Alien strategy flags (al_sflags) — from VARS.INC / STRATEQU.INC
#define ASF_INVISIBLE  0x01  // Don't render this alien
#define ASF_HITFLASH   0x02  // Flash white for one frame on hit
#define ASF_SHADOW     0x04  // Render shadow
#define ASF_PARTOBJ    0x08  // Particle/debris object
#define ASF_COLLDISABLE 0x10 // Collision disabled
#define ASF_COLLIDE    0x20  // Currently colliding (set per-frame)
#define ASF_NOHITAFFECT 0x40 // Don't apply hit effects
#define ASF_LCOLLIDE   0x80  // Was colliding in previous frame

// Additional strategy flags used by SOUND.ASM logic.
// These live in the higher-byte sflags fields on SNES.
#define ASF3_REALOBJ   0x08  // al_sflags3: realobj
#define ASF3_SFLAG5    0x01  // path trigger: hit by player weapon
#define ASF3_SFLAG7    0x04  // path trigger: any collision hit
#define ASF3_CHILDOBJ  0x10  // child-object link member
#define ASF3_MOTHEROBJ 0x20  // mother object with child list
#define ASF3_TEXTOBJ   0x40  // software text object
#define ASF3_SSPRITE   0x80  // software sprite object
#define ASF4_PLAYEROBJ 0x01  // al_sflags4: playerobj
#define ASF4_DONESND   0x02  // al_sflags4: donesnd
#define ASF4_NOPOLYEXP 0x04  // flat-runtime marker for no polygon explosion
#define ASF4_SFLAG8    0x20  // al_sflags4: sflag8 (STRATEQU sflag8)

// Strategy flags byte 2 (sflags2) — explosion-system flags from STRATEQU.INC
#define ASF2_NOEXPSND   0x08  // noexpsnd: suppress explosion sound
#define ASF2_SFLAG1     0x10  // sflag1: general purpose strategy flag
#define ASF2_RELEXPLODE 0x04  // relexplode: add player Z to maintain world-relative pos

// Alien collision flags (al_collflags)
// Alien collision flags (al_collflags)
// These are type bits that control which object categories can collide.
// Two aliens collide only if (a->collflags & b->collflags) == 0
// (i.e. they share no collision type bits — enemies don't collide with enemies).
#define ACF_COLLTYPE6   0x01  // Collision type 6 (Z-enemy)
#define ACF_WEAPON      0x02  // Is a weapon/projectile
#define ACF_FIRSTFRAME  0x04  // First frame after spawn (skip coldet)
#define ACF_COLLTYPE1   0x08  // Collision type 1 (player laser)
#define ACF_COLLTYPE2   0x10  // Collision type 2 (enemy type 1)
#define ACF_COLLTYPE3   0x20  // Collision type 3 (enemy type 2)
#define ACF_COLLTYPE4   0x40  // Collision type 4 (enemy weapon)
#define ACF_COLLTYPE5   0x80  // Collision type 5 (friendly)

// Object system
void Obj_Init(void);

// Per-frame strategy processing (decompiled from dostrats in TRANS.ASM)
void Obj_RunStrategies(void);

// Allocate/free aliens from linked lists
Alien *Obj_Alloc(void);
void Obj_Free(Alien *al);

// Get alien by array index
Alien *Obj_GetByIndex(int idx);

// Alien array and special objects
extern Alien g_aliens[NUMBER_AL];
extern Alien g_view_block;  // Camera viewpoint alien

// Linked list heads
extern Alien *g_active_list;  // allst — head of active alien linked list
extern Alien *g_free_list;    // alfreelst — head of free alien list

// Death flag — set by strategies to mark current alien for removal
extern uint8 g_aldead;

// Player object (always g_aliens[0] when spawned)
Alien *Obj_GetPlayer(void);

// Kill all aliens and reset lists (from kill_list_l in MAIN.ASM)
void Obj_KillAll(void);

#endif // STARFOX_GAME_OBJ_H
