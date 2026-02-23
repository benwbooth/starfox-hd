#ifndef STARFOX_GAME_DRAW_H
#define STARFOX_GAME_DRAW_H

// Draw list builder (from DRAW.ASM)
// Sorts visible objects by depth and builds DrawListEntry array
void Draw_Init(void);
void Draw_BuildList(void);

#endif // STARFOX_GAME_DRAW_H
