#ifndef STARFOX_GAME_WINDOWS_H
#define STARFOX_GAME_WINDOWS_H

#include "../types.h"

// Color window/fade effects (from WINDOWS.ASM)
void Windows_Init(void);
void Windows_Update(void);

// Trigger fade effects
void Windows_FadeToBlack(int speed);
void Windows_FadeFromBlack(int speed);
void Windows_FadeToWhite(int speed);

// WORLD.ASM fade opcode helpers
void Windows_StartMapFade(int8 fadedir);
bool Windows_IsMapFadeActive(void);

// Literal-ported WINDOWS.ASM core routines
void initblack_l(void);
void setblack_l(void);
void fadewhite(void);
void initfadewhite2norm_l(void);
void fadewhite2norm(void);

#endif // STARFOX_GAME_WINDOWS_H
