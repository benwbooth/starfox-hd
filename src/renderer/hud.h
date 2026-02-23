#ifndef STARFOX_HUD_H
#define STARFOX_HUD_H

#include "../types.h"

void Hud_Init(void);
void Hud_Shutdown(void);
void Hud_Render(int screen_width, int screen_height);

// Update HUD state from game variables
void Hud_SetShield(int current, int max);
void Hud_SetBoost(int current, int max);
void Hud_SetBossHP(int current, int max);
void Hud_SetLives(int count);
void Hud_SetBombs(int count);

#endif // STARFOX_HUD_H
