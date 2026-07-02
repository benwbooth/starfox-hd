#ifndef STARFOX_STRAT_BOSS2_H
#define STARFOX_STRAT_BOSS2_H

#include "../game/obj.h"
#include "../types.h"

// GBSTRATS.ASM boss2_Istrat — Macbeth "spinning top" boss (MAP1_4), reused
// as the Venom1 boss by MAP3_5.
void Strat_Boss2_Init(Alien *self);

// Registers boss2 in g_istrats (ISTRATS.ASM def_Istrat row 108).
// Called from Strat_RegisterAll.
void StratBoss2_Register(void);

#endif // STARFOX_STRAT_BOSS2_H
