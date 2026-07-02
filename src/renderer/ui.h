#ifndef STARFOX_UI_H
#define STARFOX_UI_H

#include "../types.h"

// State-aware 2D UI pass (title prompt, planet-select route map)
// plus the full-screen fade overlay driven by the WINDOWS.ASM state.

void Ui_Init(void);
void Ui_Shutdown(void);

// Draw state-dependent UI (after 3D + HUD, before the fade overlay).
void Ui_Render(int screen_width, int screen_height);

// Draw the full-screen fade overlay (black/white) from g_windowarray state.
// Must be the last 2D pass of the frame.
void Ui_RenderFade(int screen_width, int screen_height);

#endif // STARFOX_UI_H
