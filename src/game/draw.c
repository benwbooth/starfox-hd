// MAIN.ASM marioshowview → C conversion
// Build draw list from active alien linked list.
//
// Decompiled from marioshowview (MAIN.ASM:2097-2271).
// Walks the active alien list (allst) and copies each visible alien's
// world position, rotation, shape, flags, animation state, etc.
// into DrawListEntry structs for the renderer to consume.
//
// On SNES, positions were stored as view-relative (worldpos - viewpos)
// because the Super FX needed pre-transformed coordinates.
// In our HD remaster, we store world positions directly and let
// the OpenGL view matrix handle the camera transform.

#include "draw.h"
#include "obj.h"
#include "game.h"
#include "game_vars.h"
#include "../types.h"
#include "../variables.h"
#include <string.h>

void Draw_Init(void) {
    // No initialization needed — draw list is rebuilt every frame
}

void Draw_BuildList(void) {
    // Decompiled from marioshowview (MAIN.ASM:2097-2271).
    //
    // marioshowview:
    //   stz numshapes
    //   ldx #m_Drawlist      ; draw list pointer
    //   ldy allst            ; active alien list head
    // .next:
    //   [clear view flags in al_flags]
    //   [skip if invisible (asf_invisible)]
    //   [set dl_sortz: ground=15000, else 0]
    //   [dl_x = al_worldx - viewposx]  (view-relative on SNES)
    //   [dl_y = al_worldy - viewposy]
    //   [dl_z = al_worldz - viewposz]
    //   [copy dl_shape, dl_rotx/y/z]
    //   [handle explosion: dl_expcnt, particle data]
    //   [copy dl_sflags, mask out hitflash from alien]
    //   [copy dl_animframe, dl_colframe — bit 7 means use gameframe]
    //   [copy dl_depth, dl_tscrollx/y]
    //   [copy dl_coltab]
    //   [inc numshapes, advance dl pointer]
    //   [next alien via _next]

    Alien *al = g_active_list;
    while (al) {
        // --- Clear view placement flags (MAIN.ASM:2118-2121) ---
        // al_flags &= ~(afinrngpl | affrontpl | afinviewpl | afleftpl)
        // al_flags |= affrontpl
        al->flags &= ~(AF_INRNG_PL | AF_FRONT_PL | AF_INVIEW_PL | AF_LEFT_PL);
        al->flags |= AF_FRONT_PL;

        // --- Skip invisible aliens (MAIN.ASM:2123) ---
        if (al->sflags & ASF_INVISIBLE) {
            al = al->next;
            continue;
        }

        // --- Skip if no shape ---
        if (al->shape == 0) {
            al = al->next;
            continue;
        }

        // --- Build DrawListEntry ---
        DrawListEntry entry;
        memset(&entry, 0, sizeof(entry));

        // Sort Z: ground objects sort behind everything (MAIN.ASM:2126-2133)
        entry.sort_z = (al->type & ATGND) ? 15000 : 0;

        // World position as FP16.16 for the OpenGL renderer.
        // On SNES this was view-relative (worldpos - viewpos), but our
        // renderer handles the camera transform via the view matrix.
        entry.x = FP16_FROM_INT(al->worldx);
        entry.y = FP16_FROM_INT(al->worldy);
        entry.z = FP16_FROM_INT(al->worldz);

        // Shape (MAIN.ASM:2150-2152)
        entry.shape_id = al->shape;

        // Rotation angles (MAIN.ASM:2155-2160)
        entry.rx = (int16)al->rotx;
        entry.ry = (int16)al->roty;
        entry.rz = (int16)al->rotz;

        // Explosion state (MAIN.ASM:2164-2192)
        if (al->flags & AFEXP) {
            // `dl_expcnt` drives the generic per-face explosion offset in the
            // renderer, matching the Super FX `mexpfaces*` path.
            entry.explosion_cnt = (al->sflags4 & ASF4_NOPOLYEXP) ? 0u : al->count;

            // Particle objects store extra data in shadow fields
            if (al->sflags & ASF_PARTOBJ) {
                // dl_shady = alien pointer (unique ID)
                entry.shad_y = (int16)(al - g_aliens);  // index as ID
                entry.shad_x = (int16)al->sbyte1;       // particle amount
                // sbyte2 = life, sbyte3 = type
                entry.shad_z = (int16)al->sbyte3;
            }
        } else {
            entry.explosion_cnt = 0;
        }

        // Strategy flags (MAIN.ASM:2196-2201)
        entry.sflags = al->sflags;
        // Mask out hitflash — it's a one-frame effect
        al->sflags &= ~ASF_HITFLASH;

        // Animation frame (MAIN.ASM:2204-2209)
        // Bit 7 clear means follow `gameframe`; bit 7 set keeps the stored frame.
        if ((int8)al->animframe < 0) {
            entry.anim_frame = al->animframe & 127;
        } else {
            entry.anim_frame = (uint8)(g_gameframe & 127);
        }

        // Color animation frame (MAIN.ASM:2211-2216)
        if ((int8)al->colframe < 0) {
            entry.col_frame = al->colframe & 127;
        } else {
            entry.col_frame = (uint8)(g_gameframe & 127);
        }

        // Depth offset (MAIN.ASM:2219-2220)
        entry.depth_offset = (uint8)al->depthoffset;

        // Texture scroll coordinates (MAIN.ASM:2223-2226)
        entry.tscroll_x = al->tx;
        entry.tscroll_y = al->ty;

        // Color table (MAIN.ASM:2243-2246)
        entry.color_table = al->coltab;

        // Mark as visible for the renderer
        entry.flags = DL_FLAG_VISIBLE;

        // Drop shadow (MDRAWLIS.MC shadow pass): drawn for objects with
        // asf_shadow when the level's flymode enables shadows (pfm_shadows,
        // set for planet/exitbase/tunnel modes in STRATEQU.INC).
        if ((al->sflags & ASF_SHADOW) && (g_playerflymode & PFM_SHADOWS)) {
            entry.flags |= DL_FLAG_SHADOW;
        }

        // Stable identity so the renderer can pair this entry with the
        // previous frame's entry for interpolation even when the draw list
        // order changes (spawns/removals reshuffle the active list).
        entry.obj_id = (uint16)((al - g_aliens) + 1);

        // Set view-side flag based on world position relative to camera
        // (MAIN.ASM:2068-2078 in alienflags_l)
        al->flags |= AF_INVIEW_PL;
        if (al->worldx < g_pviewposx) {
            al->flags |= AF_LEFT_PL;
        }

        Game_SubmitDrawEntry(&entry);

        al = al->next;
    }
}
