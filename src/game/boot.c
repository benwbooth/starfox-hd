// BOOTNMI.ASM → C conversion
// Boot sequence, main loop, RAM initialization

#include "boot.h"
#include "nmi.h"
#include "obj.h"
#include "game.h"
#include "sound.h"
#include "windows.h"
#include "bgs.h"
#include "../sf_rtl.h"
#include <string.h>
#include <stdio.h>

GameState g_game_state = GAME_STATE_BOOT;

// Draw list shared between game logic and renderer
static DrawListEntry s_draw_list[MAX_DRAW_LIST];
static int s_draw_list_count = 0;

void Game_Init(void) {
    printf("Game_Init: initializing...\n");

    // Initialize RAM (from initialise_ram in BOOTNMI.ASM)
    // Clear WRAM working area
    memset(&g_ram[0], 0, 0x2000);

    // Initialize subsystems
    Obj_Init();
    GameCamera_Init();
    Sound_Init();
    Windows_Init();
    Bgs_Init();

    g_game_state = GAME_STATE_TITLE;
    printf("Game_Init: ready, entering title state\n");
}

void Game_Tick(void) {
    // Main game tick — called at fixed 20 FPS rate
    // Mirrors the NMI handler flow from NMI.ASM

    switch (g_game_state) {
    case GAME_STATE_BOOT:
        Game_Init();
        break;

    case GAME_STATE_TITLE:
        // TODO: Title screen logic
        // For now, pressing Start goes to planet select
        if (g_pad1_new & PAD_START) {
            g_game_state = GAME_STATE_PLAYING;
            printf("Game: entering gameplay\n");
        }
        break;

    case GAME_STATE_BRIEFING:
        // TODO: Briefing/controls screen
        break;

    case GAME_STATE_PLANET_SELECT:
        // TODO: Planet/route selection
        break;

    case GAME_STATE_PLAYING:
        // Core gameplay tick
        Nmi_GameTick();
        break;

    case GAME_STATE_CONTINUE:
        // TODO: Continue screen
        break;

    case GAME_STATE_ENDING:
        // TODO: Ending sequence
        break;
    }

    // Update windows/fade effects every tick
    Windows_Update();
}

int Game_GetDrawList(DrawListEntry *out, int max_entries) {
    int count = s_draw_list_count;
    if (count > max_entries) count = max_entries;
    memcpy(out, s_draw_list, sizeof(DrawListEntry) * count);
    return count;
}

// Called by the object system to submit entries to the draw list
void Game_SubmitDrawEntry(const DrawListEntry *entry) {
    if (s_draw_list_count < MAX_DRAW_LIST) {
        s_draw_list[s_draw_list_count++] = *entry;
    }
}

void Game_ClearDrawList(void) {
    s_draw_list_count = 0;
}
