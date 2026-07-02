// Ground object strategies (static terrain, staying-relative objects)
// Decompiled from GSTRATS.ASM: stayrel, stayrelhard180yr, staydist

#ifndef STARFOX_STRAT_GROUND_H
#define STARFOX_STRAT_GROUND_H

#include "../game/obj.h"

// Static ground object that stays relative to player Z
// (stayrel_Istrat — GSTRATS.ASM:698-703)
void Strat_StayRel_Init(Alien *self);

// Hard building that stays relative to player Z, facing 180 degrees
// (stayrelhard180yr_Istrat — GSTRATS.ASM:688-695)
void Strat_StayRelHard180yr_Init(Alien *self);

// Object that maintains fixed distance from view position
// (staydist_Istrat — GSTRATS.ASM:706-711)
void Strat_StayDist_Init(Alien *self);

// Static ground-plane segment used by the scramble tunnel maps
// (gnd_Istrat — GASTRATS.ASM:3720-3725)
void Strat_Gnd_Init(Alien *self);

#endif // STARFOX_STRAT_GROUND_H
