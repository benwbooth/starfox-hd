#ifndef STARFOX_DRAW_LIST_H
#define STARFOX_DRAW_LIST_H

#include "../types.h"

void DrawList_Init(void);
void DrawList_Shutdown(void);

// Render the draw list with interpolation between previous and current state
void DrawList_Render(const DrawListEntry *prev, int prev_count,
                     const DrawListEntry *curr, int curr_count,
                     float alpha);

#endif // STARFOX_DRAW_LIST_H
