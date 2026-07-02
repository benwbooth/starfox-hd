#ifndef STARFOX_SPRITES_H
#define STARFOX_SPRITES_H

#include "../types.h"

// Sprite flip flags (match SNES OAM bits 14-15)
#define SPR_HFLIP  0x01
#define SPR_VFLIP  0x02

// OAM palette IDs (0-7, mapped to COL file rows 8-15)
#define SPR_PAL_DEFAULT  0   // White/blue/gray — HUD text, icons
#define SPR_PAL_BLUE     1   // White-to-blue gradient
#define SPR_PAL_FOX      2   // White/orange — fox life icon
#define SPR_PAL_CROSS    4   // Green — crosshair

void Sprites_Init(void);
void Sprites_Shutdown(void);
void Sprites_SetScreenSize(int w, int h);

// Draw an 8x8 sprite at SNES coordinates (256x224 reference)
void Sprites_Draw8(int tile, int x, int y, int palette, int flags);

// Render all queued HUD sprites for the current frame.
// Called from Hud_Render() after bars, before text.
void Sprites_RenderHUD(void);

// Radio portrait (FACE.CGX): frames 0-4 window iris opening animation,
// 5-16 per-character talk frames (fox/peppy/falco/slippy/pepper/andross,
// 2 each), 17 "anyone" silhouette.  x/y in SNES 256x224 coordinates
// (top-left of the 32x40 portrait).
void Sprites_DrawFace(int frame, int x, int y);

// In-game bitmap-layer palette (NIGHT.COL row 0), index 0-15 -> RGBA.
// This is the palette the Super FX bitmap (3D view + HUD meters + faces)
// indexes on the SNES.
void Sprites_GetBitmapColor(int index, float out[4]);

#endif
