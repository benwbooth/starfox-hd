#ifndef STARFOX_STRAT_BOSS_SEA_H
#define STARFOX_STRAT_BOSS_SEA_H

#include "../types.h"
#include "../game/obj.h"

// Sea Zone (MAP2_3B/MAP2_3C) bosses.
//
// ASM sources:
//   GA2STRAT.ASM:3056-3196  bossseamon_Istrat / bossseamon_strat /
//                           bossseamonexp_Istrat
//   GASTRATS.ASM:2046-2140  seamon_Istrat / seamon_strat (small sea monster,
//                           also used by MAP3_3B via istrat index 81)
//   D2STRATS.ASM:54-495     bossg_istrat / bossgexplode_istrat / bossgs_istrat
//   D3STRATS.ASM:1098-1155  flyingfish_istrat (launched by bossg)

// bossseamon_Istrat (GA2STRAT.ASM:3056)
void Strat_BossSeamon_Init(Alien *self);

// bossg_istrat (D2STRATS.ASM:54)
void Strat_BossG_Init(Alien *self);

// seamon_Istrat (GASTRATS.ASM:2046)
void Strat_Seamon_Init(Alien *self);

void StratBossSea_Register(void);

#endif // STARFOX_STRAT_BOSS_SEA_H
