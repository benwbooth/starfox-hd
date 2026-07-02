#ifndef STARFOX_FONT_H
#define STARFOX_FONT_H

#include "../types.h"

void Font_Init(void);
void Font_Shutdown(void);
void Font_DrawString(int x, int y, const char *text, float r, float g, float b);
void Font_DrawNumber(int x, int y, int value, int digits);
void Font_SetScreenSize(int w, int h);

// Debug: raw GL texture handle of the glyph atlas (0 if not loaded)
unsigned int Font_DebugGetTexture(void);

#endif
