#ifndef STARFOX_GAME_OBJ_H
#define STARFOX_GAME_OBJ_H

#include "../types.h"

// Alien (object) structure — matches STRUCTS.INC layout
// Every 3D entity in the game (enemies, items, player wingmen, obstacles)
typedef struct Alien {
    struct Alien *next;
    struct Alien *prev;
    // al_start fields
    uint16 shape;         // Shape reference
    uint16 ptr;           // Pointer to attached alien
    uint8  flags;         // Alien flags (afexp, afhit, etc.)
    uint8  type;          // Alien type flags (atgnd, atmissile, etc.)
    uint8  count;         // Frame counter
    uint8  count1;        // Secondary counter
    int16  worldx;        // World X coordinate
    int16  worldy;        // World Y coordinate
    int16  worldz;        // World Z coordinate
    uint8  rotx;          // Rotation angle X
    uint8  roty;          // Rotation angle Y
    uint8  rotz;          // Rotation angle Z
    uint8  vel;           // Velocity
    // Giles rotation data (from GILESal.INC)
    int16  xv, yv, zv;   // Velocity vector
    int16  xa, ya, za;    // Acceleration
    // Extended alien data (from alx_start in STRUCTS.INC)
    uint8  colframe;      // Collision frame
    uint8  animframe;     // Animation frame
    uint8  snd1, snd2;    // Sound IDs
    uint16 coltab;        // Collision table pointer
    uint8  childx, childy, childz;         // Child relative offsets
    uint8  childrotx, childroty, childrotz; // Child rotation
    uint16 childrotobj;   // Child rotation object
    uint8  tx, ty;        // Texture coordinates
    uint16 memptr;        // Strategy memory pointer
    uint16 stackptr;      // Stack pointer
    uint16 stratmem;      // Strategy memory
    uint8  pbyte1, pbyte2;// Permanent bytes
    uint16 pword1;        // Permanent word

    // Strategy function pointer
    void (*strategy)(struct Alien *self);

    // Active flag
    bool active;
} Alien;

#define NUMBER_AL 70  // Max aliens (from VARS.INC)

// Object system
void Obj_Init(void);
void Obj_UpdateAll(void);

// Allocate/free aliens
Alien *Obj_Alloc(void);
void Obj_Free(Alien *al);

// Get alien by index
Alien *Obj_GetByIndex(int idx);

// Active alien list for iteration
extern Alien g_aliens[NUMBER_AL];
extern Alien g_view_block;  // Player viewpoint "alien"

// Player object (always g_aliens[0])
Alien *Obj_GetPlayer(void);

#endif // STARFOX_GAME_OBJ_H
