#ifndef STARFOX_BG2D_H
#define STARFOX_BG2D_H

#include "../types.h"

// 2D background layer pass (SNES BG2/BG3 tilemap layers).
// Draws before the 3D pass, depth test off.

void Bg2d_Init(void);
void Bg2d_Shutdown(void);

// Draw the active background layer (title screen, planet select backdrop,
// or a starfield-ish gradient fallback for unported bg ids).
void Bg2d_Render(int screen_width, int screen_height);

// True if the composed title-screen texture (STAR FOX logo layer) loaded.
bool Bg2d_HasTitle(void);

#endif // STARFOX_BG2D_H
