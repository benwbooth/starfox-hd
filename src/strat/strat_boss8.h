#ifndef STARFOX_STRAT_BOSS8_H
#define STARFOX_STRAT_BOSS8_H

#include "../game/obj.h"
#include "../types.h"

// MAP3_4 wash boss (GB3STRAT.ASM boss8_Istrat).
void Strat_Boss8_Init(Alien *self);

// Registers boss8 + washmap support strategies (nucleuslauncher,
// nucleuspillar, nucleusbeamL, boss8shrap) in g_istrats[] and the
// synthetic strategy addresses 0x060014..0x060016 used by levels.c.
void StratBoss8_Register(void);

#endif // STARFOX_STRAT_BOSS8_H
