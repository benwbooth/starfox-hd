#ifndef STARFOX_STRAT_PLAYER_H
#define STARFOX_STRAT_PLAYER_H

#include "../game/obj.h"

// Player (Arwing) strategy — from PSTRATS.ASM
void Strat_Player(Alien *self);

// Intro ship flyby used by MAP1_1A (`shipintro_Istrat`).
void Strat_ShipIntro_Init(Alien *self);

// Spawn the player alien at the start of gameplay
Alien *Strat_SpawnPlayer(void);

// --- PCSTRATS.ASM: Player Clear / Escape Strategies ---

// Bridge clear sequence (playerclearbridge_Istrat)
void Strat_PlayerClearBridge_Init(Alien *self);

// Nucleus escape sequence (playerEscapeNucleus_Istrat)
void Strat_PlayerEscapeNucleus_Init(Alien *self);

// --- PISTRATS.ASM: Player Opening (Intro Sequence) Strategies ---

// Level opening intro sequence (playeropening_Istrat)
void Strat_PlayerOpening_Init(Alien *self);

// set_playerExitBase_l (PISTRATS.ASM:621-720) — full hangar launch
// sequence (wait/Go/Follow states + friendstart3 wingman), invoked by the
// MAP_CB_SET_PLAYER_EXITBASE_L map callback after the scramble opening.
void Strat_PlayerExitBase(Alien *player);

#endif // STARFOX_STRAT_PLAYER_H
