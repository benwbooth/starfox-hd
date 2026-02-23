// DRAW.ASM → C conversion
// Build sorted draw list from active objects

#include "draw.h"
#include "obj.h"
#include "game.h"
#include "../types.h"

void Draw_Init(void) {
    // TODO: Initialize z-sort blocks
}

void Draw_BuildList(void) {
    // Iterate all active aliens and submit visible ones to draw list
    for (int i = 0; i < NUMBER_AL; i++) {
        Alien *al = &g_aliens[i];
        if (!al->active) continue;
        if (al->shape == 0) continue;

        // Build draw list entry from alien state
        DrawListEntry entry;
        entry.x = FP16_FROM_INT(al->worldx);
        entry.y = FP16_FROM_INT(al->worldy);
        entry.z = FP16_FROM_INT(al->worldz);
        entry.rx = (int16)al->rotx;
        entry.ry = (int16)al->roty;
        entry.rz = (int16)al->rotz;
        entry.shape_id = al->shape;
        entry.color_table = al->coltab;
        entry.lod_depth = 0;  // TODO: calculate from camera distance
        entry.flags = DL_FLAG_VISIBLE;
        entry.explosion_state = (al->flags & 1) ? al->count : 0;  // afexp

        Game_SubmitDrawEntry(&entry);
    }

    // TODO: Sort draw list by Z depth (painter's algorithm)
}
