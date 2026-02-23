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

#endif // STARFOX_GAME_WINDOWS_H
