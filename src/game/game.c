// GAME.ASM getview_l → C conversion
// Camera system — computes final camera position and rotation.
//
// Decompiled from getview_l (GAME.ASM / GSTRATS.ASM).
//
// In the SNES version, getview_l:
//   1. Computes base position: viewpos = pviewpos + shake + float_bob
//   2. Computes rotation: viewrot = outv - player_turnrot/plrotz
//   3. Builds rotation matrix from viewrot
//   4. Applies rotation to (0, 0, -outdist) to get camera offset
//   5. Final camera = base_position + rotated_offset
//
// This places the camera `outdist` units behind the player, rotated
// by the current view rotation so the camera looks in the player's
// direction.

#include "game.h"
#include "game_vars.h"
#include "obj.h"
#include "../sf_rtl.h"
#include "../variables.h"
#include "../renderer/transform.h"
#include <math.h>

// Final computed camera position (viewposx/y/z in ASM)
int32 g_camera_x, g_camera_y, g_camera_z;
int16 g_camera_rx, g_camera_ry, g_camera_rz;

void GameCamera_Init(void) {
    g_camera_x = FP16_FROM_INT(0);
    g_camera_y = FP16_FROM_INT(0);
    g_camera_z = FP16_FROM_INT(-500);
    g_camera_rx = g_camera_ry = g_camera_rz = 0;
    g_viewdist = OUTVIEWDIST;
}

// Convert radians to the SNES 0-255 angle system.
static int16 angle8_from_rad(float rad) {
    return (int16)lroundf(rad * (128.0f / 3.14159265f));
}

void GameCamera_Update(void) {
    Alien *player = Obj_GetPlayer();
    if (!player || !player->active) {
        // No player (e.g. title screen) — still set the camera transform
        // so 3D objects render with the initial camera position.
        Transform_SetCamera(g_camera_x, g_camera_y, g_camera_z,
                             g_camera_rx, g_camera_ry, g_camera_rz);
        return;
    }

    int16 pos_x, pos_y, pos_z;
    int16 rot_x, rot_y, rot_z = 0;

    if ((g_viewtype & VIEWTYPE_FPOS) != 0) {
        // --- Fixed-position camera (GAME.ASM getview_l .fpos) ---
        // Strategies (viewopening, playerExitBase, clear demos) drive the
        // final camera position g_viewpos directly; shake/bob are skipped.
        pos_x = g_viewposx;
        pos_y = g_viewposy;
        pos_z = g_viewposz;
    } else {
        // --- Step 1: Base position from pviewpos + shake + bob ---
        // (GAME.ASM: viewposx = pviewposx + sign_extend(viewshakeX) + viewfloatX)
        int16 base_x = g_pviewposx + (int8)g_viewshakeX + g_viewfloatX;
        int16 base_y = g_pviewposy + (int8)g_viewshakeY + g_viewfloatY;
        int16 base_z = g_pviewposz + (int8)g_viewshakeZ + g_pviewposzoff;

        // Update view float bob (sinusoidal camera bob during flight)
        if (g_viewfloatptr >= 0 && g_viewfloatptr < VIEWFLOATTAB_LEN) {
            g_viewfloatY = g_viewfloattab[g_viewfloatptr];
            g_viewfloatptr = (g_viewfloatptr + 1) % VIEWFLOATTAB_LEN;
        }

        // --- Step 3-4: Apply -outdist offset along view direction ---
        // In the SNES version, getview_l builds a rotation matrix from
        // viewrot and rotates (0, 0, -outdist) to get the camera offset.
        // For forward-facing flight, this primarily adds a -Z offset (behind).
        //
        // Simplified: since most of Star Fox is forward-scrolling flight,
        // the camera offset is approximately (0, 0, -viewdist) rotated by
        // the player's pitch (rotx).
        int16 dist = g_viewdist > 0 ? g_viewdist : OUTVIEWDIST;

        // Rotate the offset by pitch for vertical tracking
        float pitch_rad = (float)player->rotx * (3.14159265f / 128.0f);
        int16 offset_y = (int16)(sinf(pitch_rad) * (float)dist);
        int16 offset_z = (int16)(-cosf(pitch_rad) * (float)dist);

        pos_x = base_x;
        pos_y = (int16)(base_y - offset_y);
        pos_z = (int16)(base_z + offset_z);

        // Publish the final camera position (ASM viewposx/y/z, stored into
        // the viewblk pseudo-object) so strategies can measure against it.
        g_viewposx = pos_x;
        g_viewposy = pos_y;
        g_viewposz = pos_z;
    }

    if ((g_viewtype & VIEWTYPE_TOOBJ) != 0) {
        // --- Look-at camera (GAME.ASM getview_l viewtype_toobj) ---
        // Aim the camera at the viewtoobj alien (defaults to the player,
        // matching MAIN.ASM's viewtoobj=playpt initialisation).
        Alien *target = NULL;
        if (g_viewtoobj >= 0 && g_viewtoobj < NUMBER_AL &&
            g_aliens[g_viewtoobj].active) {
            target = &g_aliens[g_viewtoobj];
        }
        if (!target || !target->active) target = player;

        float dx = (float)(target->worldx - pos_x);
        float dy = (float)(target->worldy - pos_y);
        float dz = (float)(target->worldz - pos_z);
        float hlen = sqrtf(dx * dx + dz * dz);

        // SNES camera forward vector for angles (rx, ry):
        //   (cos rx * sin ry, -sin rx, cos rx * cos ry)
        // => ry = atan2(dx, dz), rx = atan2(-dy, hlen).
        rot_y = angle8_from_rad(atan2f(dx, dz));
        rot_x = angle8_from_rad(atan2f(-dy, hlen));
        rot_z = 0;
    } else {
        // --- Step 2: Normal view rotation ---
        // Camera looks in the player's direction, offset by turn/roll
        rot_x = (int16)player->rotx;
        rot_y = (int16)(player->roty - (uint8)(g_player_turnrot >> 8));
        rot_z = 0;
    }

    // --- Step 5: Final camera position ---
    g_camera_x = FP16_FROM_INT(pos_x);
    g_camera_y = FP16_FROM_INT(pos_y);
    g_camera_z = FP16_FROM_INT(pos_z);

    g_camera_rx = rot_x;
    g_camera_ry = rot_y;
    g_camera_rz = rot_z;

    // Update renderer's view matrix
    Transform_SetCamera(g_camera_x, g_camera_y, g_camera_z,
                         g_camera_rx, g_camera_ry, g_camera_rz);

    // Camera cut detection: a viewtype change (fpos jumps in the intro,
    // look-at target switches, level transitions) must not be smoothed by
    // the render-frame camera interpolation.
    {
        static uint8 s_last_viewtype;
        if (g_viewtype != s_last_viewtype) {
            s_last_viewtype = g_viewtype;
            Transform_SnapCamera();
        }
    }
}
