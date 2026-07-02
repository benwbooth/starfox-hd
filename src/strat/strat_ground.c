// Ground object strategies (static terrain, staying-relative objects)
// Decompiled from GSTRATS.ASM: stayrel, stayrelhard180yr, staydist
//
// These strategies keep objects positioned relative to the player's
// forward motion. Without s_add_playerZ, objects spawned by the map
// executor would fall behind as the player moves forward.

#include "strat_ground.h"
#include "strat_enemy.h"
#include "../game/game_vars.h"
#include "../game/obj.h"
#include "../variables.h"

// ============================================================
// STAYREL — Stay relative to player Z (GSTRATS.ASM:698-703)
// Decorative, no collision, scrolls with the world.
// ============================================================

static void stayrel_strat(Alien *self) {
    // stayrel_strat (GSTRATS.ASM:699-703)
    // s_add_playerZ x — move with world scrolling
    self->worldz += g_pviewvelz;
    // s_set_alsflag x,colldisable
    self->sflags |= ASF_COLLDISABLE;
}

void Strat_StayRel_Init(Alien *self) {
    self->stratptr = stayrel_strat;
}

// ============================================================
// GND — Static ground-plane segment (GASTRATS.ASM:3720-3725)
// Used by the scramble tunnel maps (MAP1_1A) for op_* floor tiles.
// ============================================================

void Strat_Gnd_Init(Alien *self) {
    // s_set_alptrs x,0,0,0
    self->stratptr = NULL;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    // s_set_altype x,gnd
    self->type |= ATGND;
    // s_set_alsflag x,colldisable
    self->sflags |= ASF_COLLDISABLE;
}

// ============================================================
// STAYRELHARD180YR — Indestructible building, stays relative,
// facing 180 degrees (GSTRATS.ASM:688-695)
// ============================================================

static void stayrelhard180yr_strat(Alien *self) {
    // stayrelhard180YR_strat (GSTRATS.ASM:692-695)
    // s_add_playerZ x
    self->worldz += g_pviewvelz;
}

void Strat_StayRelHard180yr_Init(Alien *self) {
    // stayrelhard180yr_Istrat (GSTRATS.ASM:688-691)
    // s_hardvars x
    self->HP = HARD_HP;
    self->AP = HARD_AP;
    // s_set_alvar B,x,al_roty,#deg180
    self->roty = DEG180;
    // s_set_strat x,stayrelhard180YR_strat
    self->stratptr = stayrelhard180yr_strat;
}

// ============================================================
// STAYDIST — Maintain fixed distance from view position
// (GSTRATS.ASM:706-711)
// sword1 stores the desired Z offset from pviewposz.
// ============================================================

void Strat_StayDist_Init(Alien *self) {
    // staydist_Istrat (GSTRATS.ASM:706-711)
    // s_copy_alvar2alvar W,x,al_worldz,x,al_sword1
    // s_add_alvar W,x,al_worldz,pviewposz
    self->worldz = self->sword1 + g_pviewposz;
    // s_set_alsflag x,colldisable
    self->sflags |= ASF_COLLDISABLE;
    // Init-only, no per-frame strategy (doesn't update after spawn)
    self->stratptr = NULL;
}
