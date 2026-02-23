// GAME.ASM → C conversion
// Camera system

#include "game.h"
#include "obj.h"
#include "../sf_rtl.h"
#include "../renderer/transform.h"

int32 g_camera_x, g_camera_y, g_camera_z;
int16 g_camera_rx, g_camera_ry, g_camera_rz;
uint8 g_game_mode = 1;  // SPACE_MODE

void GameCamera_Init(void) {
    g_camera_x = FP16_FROM_INT(0);
    g_camera_y = FP16_FROM_INT(0);
    g_camera_z = FP16_FROM_INT(-500);
    g_camera_rx = g_camera_ry = g_camera_rz = 0;
}

void GameCamera_Update(void) {
    // Camera follows the player object
    Alien *player = Obj_GetPlayer();
    if (!player || !player->active) return;

    // Position camera behind and above player
    g_camera_x = FP16_FROM_INT(player->worldx);
    g_camera_y = FP16_FROM_INT(player->worldy + 50);
    g_camera_z = FP16_FROM_INT(player->worldz - 200);

    g_camera_rx = (int16)player->rotx;
    g_camera_ry = (int16)player->roty;
    g_camera_rz = 0;

    // Update renderer's view matrix
    Transform_SetCamera(g_camera_x, g_camera_y, g_camera_z,
                         g_camera_rx, g_camera_ry, g_camera_rz);
}
