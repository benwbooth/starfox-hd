// NMI.ASM → C conversion
// Per-frame game update (VBlank handler equivalent)

#include "nmi.h"
#include "obj.h"
#include "game.h"
#include "coldet.h"
#include "draw.h"
#include "sound.h"
#include "boot.h"
#include "../sf_rtl.h"

void Nmi_GameTick(void) {
    // This mirrors the NMI handler's game-mode processing:
    // 1. Clear draw list for new frame
    Game_ClearDrawList();

    // 2. Process player input
    // (handled by strat/strat_player.c)

    // 3. Update all objects (strategies)
    Obj_UpdateAll();

    // 4. Run collision detection
    Coldet_Run();

    // 5. Update camera
    GameCamera_Update();

    // 6. Build draw list from visible objects
    Draw_BuildList();

    // 7. Process sound commands
    Sound_Update();

    // 8. Advance PRNG
    SfRtl_Random();

    // 9. Increment frame counter
    g_frame_count++;
}
