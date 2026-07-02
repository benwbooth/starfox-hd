// COLDET.ASM + STRATROU.ASM generate_collist_l → C conversion
// Collision detection system.
//
// Decompiled from:
//   generate_collist_l (STRATROU.ASM:30-89) — build collision list
//   chkcoll (COLDET.ASM:225-861) — AABB pair testing
//   do_coll_l (STRATROU.ASM:2143-2178) — apply damage with cooldown
//
// Per-frame flow:
//   1. generate_collist_l builds flat array of collision-eligible aliens
//   2. Clear per-frame collision flags (asf_collide)
//   3. For each pair in list, perform AABB overlap test
//   4. On collision: set flags, store cross-references, call collstratptr
//
// The collision type system prevents same-category collisions:
// - Aliens share no type bits → they CAN collide (e.g. laser & enemy)
// - Aliens share type bits → they CANNOT collide (e.g. enemy & enemy)

#include "coldet.h"
#include "obj.h"
#include "game_vars.h"
#include "../strat/strat_enemy.h"
#include "../renderer/shapes.h"
#include "../variables.h"
#include <math.h>
#include <stdlib.h>

// Collision list entry (from STRUCTS.INC cl_ structure, 10 bytes on SNES)
typedef struct {
    Alien  *alien;      // cl_alien: pointer to alien
    int16  xmax;        // cl_xmax: collision box X half-extent
    int16  ymax;        // cl_ymax: collision box Y half-extent
    int16  zmax;        // cl_zmax: collision box Z half-extent
} ColListEntry;

#define MAX_COLLIST 70
static ColListEntry s_collist[MAX_COLLIST];
static int s_collist_count = 0;

// Default collision extents when shape data isn't loaded yet
#define DEFAULT_COLL_EXTENT 20

static void load_collision_extents(const Alien *al, int16 *xmax, int16 *ymax, int16 *zmax) {
    const Shape *shape = Shapes_Get(al->shape);
    if (!shape || !shape->vertices || shape->num_vertices <= 0) {
        *xmax = DEFAULT_COLL_EXTENT;
        *ymax = DEFAULT_COLL_EXTENT;
        *zmax = DEFAULT_COLL_EXTENT;
        return;
    }

    float max_x = 0.0f;
    float max_y = 0.0f;
    float max_z = 0.0f;

    for (int i = 0; i < shape->num_vertices; i++) {
        const ShapeVertex *v = &shape->vertices[i];
        float ax = fabsf(v->x);
        float ay = fabsf(v->y);
        float az = fabsf(v->z);
        if (ax > max_x) max_x = ax;
        if (ay > max_y) max_y = ay;
        if (az > max_z) max_z = az;
    }

    *xmax = (int16)MAX(6, (int)(max_x + 1.0f));
    *ymax = (int16)MAX(6, (int)(max_y + 1.0f));
    *zmax = (int16)MAX(6, (int)(max_z + 1.0f));
}

void Coldet_Init(void) {
    s_collist_count = 0;
}

void Coldet_GenerateList(void) {
    // Decompiled from generate_collist_l (STRATROU.ASM:30-89).
    // Walk active list, skip ineligible aliens, build collision entries.
    s_collist_count = 0;

    Alien *al = g_active_list;
    while (al && s_collist_count < MAX_COLLIST) {
        // Skip if first frame (acf_firstframe) — just spawned, no collision yet
        if (al->collflags & ACF_FIRSTFRAME) {
            al = al->next;
            continue;
        }

        // Skip if collision disabled (asf_colldisable)
        if (al->sflags & ASF_COLLDISABLE) {
            al = al->next;
            continue;
        }

        // Skip if no HP (can't be hit) — note hardHP (0xFF) is valid
        if (al->HP == 0) {
            al = al->next;
            continue;
        }

        // Skip if exploding (afexp flag)
        if (al->flags & AFEXP) {
            al = al->next;
            continue;
        }

        // Build collision entry
        ColListEntry *ce = &s_collist[s_collist_count];
        ce->alien = al;
        load_collision_extents(al, &ce->xmax, &ce->ymax, &ce->zmax);
        s_collist_count++;

        al = al->next;
    }
}

// ============================================================
// Apply damage to a victim alien
// Decompiled from do_coll_l (STRATROU.ASM:2143-2178)
// ============================================================
static void do_coll(Alien *victim, uint8 attacker_ap) {
    // Check cooldown timer — only apply damage every framesperAP frames
    if (victim->collcount > 0) {
        victim->collcount--;
        return;
    }

    // Indestructible objects (hardHP = 0xFF) don't take damage
    if (victim->HP == 0xFF) {
        // Reset cooldown anyway
        victim->collcount = FRAMESPERAP;
        return;
    }

    uint8 damage = attacker_ap;

    // In tunnel mode, halve hardAP damage
    if ((g_pshipflags3 & PSF3_INTUNNEL) && damage == HARD_AP) {
        damage >>= 1;
    }

    // Subtract damage from HP
    if (victim->HP <= damage) {
        victim->HP = 0;
    } else {
        victim->HP -= damage;
    }

    // Reset cooldown timer
    victim->collcount = FRAMESPERAP;
}

// ============================================================
// AABB overlap test between two entries
// Decompiled from COLDET macro (COLDET.ASM:10-65)
// Returns true if the AABBs overlap on all three axes.
// ============================================================
static bool aabb_overlap(
    int16 x1, int16 y1, int16 z1, int16 ext1_x, int16 ext1_y, int16 ext1_z,
    int16 x2, int16 y2, int16 z2, int16 ext2_x, int16 ext2_y, int16 ext2_z)
{
    // Z-axis test: |z2 - z1| < (ext1_z + ext2_z)
    int16 dz = z2 - z1;
    if (dz < 0) dz = -dz;
    if (dz >= ext1_z + ext2_z) return false;

    // X-axis test: |x2 - x1| < (ext1_x + ext2_x)
    int16 dx = x2 - x1;
    if (dx < 0) dx = -dx;
    if (dx >= ext1_x + ext2_x) return false;

    // Y-axis test: |y2 - y1| < (ext1_y + ext2_y)
    int16 dy = y2 - y1;
    if (dy < 0) dy = -dy;
    if (dy >= ext1_y + ext2_y) return false;

    return true;
}

void Coldet_Run(void) {
    // Decompiled from chkcoll loop (COLDET.ASM:225-861)
    //
    // Step 1: Clear per-frame collision state on all aliens
    // (from init_strats_l — clears asf_collide, al_collobjptr)
    for (int i = 0; i < s_collist_count; i++) {
        Alien *al = s_collist[i].alien;
        // init_strats_ram_l mirrors collide -> Lcollide before clearing collide.
        if (al->sflags & ASF_COLLIDE) {
            al->sflags |= ASF_LCOLLIDE;
        } else {
            al->sflags &= ~ASF_LCOLLIDE;
        }
        al->sflags &= ~ASF_COLLIDE;
        al->sflags &= ~ASF_HITFLASH;  // Clear previous frame's hit flash
        al->collobjptr = 0;
    }

    // Step 2: Test all pairs for collision
    for (int i = 0; i < s_collist_count; i++) {
        Alien *al_a = s_collist[i].alien;

        // Skip if already colliding this frame
        if (al_a->sflags & ASF_COLLIDE) continue;

        for (int j = i + 1; j < s_collist_count; j++) {
            Alien *al_b = s_collist[j].alien;

            // Skip if already colliding
            if (al_b->sflags & ASF_COLLIDE) continue;

            // Collision type filter:
            // Two aliens with the SAME collision type bits can't collide.
            // (al_a->collflags & al_b->collflags) checks if they share
            // any collision type bits. If so, skip.
            // Mask out non-type bits (firstframe, weapon).
            uint8 type_mask = (ACF_COLLTYPE1 | ACF_COLLTYPE2 | ACF_COLLTYPE3 |
                               ACF_COLLTYPE4 | ACF_COLLTYPE5);
            uint8 a_types = al_a->collflags & type_mask;
            uint8 b_types = al_b->collflags & type_mask;
            if (a_types & b_types) continue;  // Same category — don't collide

            // At least one must have collision types set
            if (a_types == 0 && b_types == 0) continue;

            // Immunity check: can't collide with immune object
            if (al_a->immuneptr != 0 &&
                al_a->immuneptr == (uint16)(al_b - g_aliens)) continue;
            if (al_b->immuneptr != 0 &&
                al_b->immuneptr == (uint16)(al_a - g_aliens)) continue;

            // AABB overlap test
            bool hit = aabb_overlap(
                al_a->worldx, al_a->worldy, al_a->worldz,
                s_collist[i].xmax, s_collist[i].ymax, s_collist[i].zmax,
                al_b->worldx, al_b->worldy, al_b->worldz,
                s_collist[j].xmax, s_collist[j].ymax, s_collist[j].zmax
            );

            if (!hit) continue;

            // --- Collision occurred ---

            // Set collision flags on both objects
            al_a->sflags |= ASF_COLLIDE;
            al_b->sflags |= ASF_COLLIDE;

            // Store cross-references (each stores index of what it hit)
            al_a->collobjptr = (uint16)(al_b - g_aliens);
            al_b->collobjptr = (uint16)(al_a - g_aliens);

            // Apply damage: A hits B with A's AP, B hits A with B's AP
            if (al_b->AP > 0 && al_a->HP > 0) {
                do_coll(al_a, al_b->AP);
            }
            if (al_a->AP > 0 && al_b->HP > 0) {
                do_coll(al_b, al_a->AP);
            }

            // Call collision strategy callbacks
            // The collision strategy is called immediately on the colliding alien.
            // It typically does hitflash_Istrat → flash white, check HP, explode.
            if (al_a->collstratptr) {
                al_a->collstratptr(al_a);
            }
            if (al_b->collstratptr) {
                al_b->collstratptr(al_b);
            }

            break;  // Each alien can only collide once per frame
        }
    }
}
