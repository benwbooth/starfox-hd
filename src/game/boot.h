#ifndef STARFOX_GAME_BOOT_H
#define STARFOX_GAME_BOOT_H

#include "../types.h"

// Initialize game state (from BOOTNMI.ASM)
void Game_Init(void);

// Run one game tick (called at 20 FPS)
void Game_Tick(void);

// Get current draw list for renderer
// Returns number of entries written
int Game_GetDrawList(DrawListEntry *out, int max_entries);

// Game states
typedef enum {
    GAME_STATE_BOOT,
    GAME_STATE_TITLE,
    GAME_STATE_BRIEFING,
    GAME_STATE_PLANET_SELECT,
    GAME_STATE_PLAYING,
    GAME_STATE_CONTINUE,
    GAME_STATE_ENDING,
} GameState;

extern GameState g_game_state;

#endif // STARFOX_GAME_BOOT_H
