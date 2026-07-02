// TRANS.ASM → C conversion
// Per-frame game update — decompiled from transfer_l (TRANS.ASM:36).
//
// On SNES, this ran in the transfer loop between NMI (VBlank) interrupts.
// The flow is:
//   transfer_l:
//     wait for trans_flag == 0
//     set trans_flag = 2 (start screen transfer)
//     do_circle_explosion
//     calcbgscroll_l
//     calcbg2voffsets
//     do_hpositions / dmahpos
//     find_window_pri_l
//     [check freezestrats flag]
//     dostrats              ← core game tick
//     store lastcont0/lastcontl0
//     getview_l             ← camera computation
//     dosounds_l            ← sound processing
//     palgoto_l / fadepalto_l ← palette transitions
//     showview_l            ← build draw list from aliens
//     build_drawlist_l      ← (no-op in shipped game)
//     do_sprites_l          ← 2D sprite processing
//     calcmeters            ← HUD gauge updates
//     generate_collist_l    ← collision list generation
//     [wait for Super FX, do_3d_display_l]
//
// In our HD remaster, SNES-specific steps (BG scroll, DMA, palette,
// sprites, Super FX sync) are replaced by the OpenGL renderer.
// We keep the game logic flow intact.

#include "nmi.h"
#include "obj.h"
#include "game.h"
#include "game_vars.h"
#include "coldet.h"
#include "draw.h"
#include "sound.h"
#include "boot.h"
#include "windows.h"
#include "../sf_rtl.h"
#include "../variables.h"
#include "../renderer/particles.h"
#include "../renderer/hud.h"

// Player body hit points — mirrors PLAYER_BODY_HP in strat_player.c
// (Strat_SpawnPlayer sets player->HP to this value).
#define NMI_PLAYER_MAX_HP 40

void Nmi_GameTick(void) {
    // --- transfer_l flow (TRANS.ASM:36-210) ---

    // --- dostrats (TRANS.ASM:312-334) ---
    // TRANS.ASM:
    //   lda freezestrats
    //   bit #1
    //   bne stratsfrozen
    //   jsr dostrats
    Game_ClearDrawList();
    if ((g_freezestrats & 0x01) == 0) {
        Obj_RunStrategies();
    }

    // Store last frame's controller state (TRANS.ASM:158-161)
    // Used by strategies to detect new key presses vs held keys
    g_lastcont0 = (uint8)(g_pad1 >> 8);
    g_lastcontl0 = (uint8)(g_pad1 & 0xFF);

    // --- getview_l (GAME.ASM) ---
    // Compute camera position and rotation from player state + modifiers
    GameCamera_Update();

    // --- dosounds_l ---
    Sound_Update();

    // --- calcmeters (TRANS.ASM:602) ---
    // Feed the HUD state from game variables once per tick.
    {
        Alien *player = Obj_GetPlayer();
        if (player && player->active) {
            Hud_SetShield(player->HP, NMI_PLAYER_MAX_HP);
        }
        Hud_SetLives(g_lives);
        Hud_SetBombs((int)g_specwepcnt);
        // Boss meter: g_bossmaxhp mirrors m_bossmaxHP (boss active when
        // nonzero; boss strats zero it when the boss dies).
        // TODO: current boss HP (m_bossHP / s_add_bossHP per-frame
        // accumulation) is not ported yet, so show a full bar while a
        // boss is alive.
        Hud_SetBossHP((int)g_bossmaxhp, (int)g_bossmaxhp);
    }

    // --- showview_l (MAIN.ASM:2001) ---
    // Build draw list from active alien linked list.
    // Copies each alien's world position, rotation, shape, sflags,
    // explosion state, animation frames, color table into DrawListEntry array.
    Draw_BuildList();

    // Update particle system (20 FPS → dt = 0.05)
    Particles_Update(0.05f);

    // --- generate_collist_l (STRATROU.ASM) ---
    // Build collision detection list from active aliens.
    Coldet_GenerateList();

    // --- Collision detection ---
    // Run AABB collision checks between entries in the collision list.
    Coldet_Run();
}
