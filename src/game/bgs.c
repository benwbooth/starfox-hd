// BGS.ASM -> C conversion
// Minimal background request scheduler state.

#include "bgs.h"
#include "game_vars.h"

void Bgs_Init(void) {
    g_bgflags = 0;
    g_bg_dmalist = 0;
    g_currentbg = 0;
    g_bgtransspeed = 0;
}

void Bgs_Update(void) {
    // Minimal modechange_l behavior: process slow background transition steps
    // on masked game frames. Without SNES bglists/DMA this collapses to a
    // single completion event.
    if (g_bg_dmalist != 0) {
        if (((uint16)g_gameframe & g_bgtransspeed) == 0) {
            g_bg_dmalist = 0;
            g_bgflags |= BGF_BG;
        }
    }

    // In the full game this bit triggers the background DMA script runner.
    if (g_bgflags & BGF_BG) {
        g_bgflags &= (uint8)~BGF_BG;
    }
}
