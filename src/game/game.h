#ifndef STARFOX_GAME_GAME_H
#define STARFOX_GAME_GAME_H

#include "../types.h"

// Camera system (from GAME.ASM)
void GameCamera_Init(void);
void GameCamera_Update(void);

// Camera state
extern int32 g_camera_x, g_camera_y, g_camera_z;
extern int16 g_camera_rx, g_camera_ry, g_camera_rz;

// Draw list management (declared in boot.c)
void Game_SubmitDrawEntry(const DrawListEntry *entry);
void Game_ClearDrawList(void);

#endif // STARFOX_GAME_GAME_H
