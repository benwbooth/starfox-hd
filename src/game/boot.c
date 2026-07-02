// BOOTNMI.ASM -> C conversion
// Boot sequence, main loop, RAM initialization

#include "boot.h"
#include "nmi.h"
#include "obj.h"
#include "game.h"
#include "game_vars.h"
#include "world.h"
#include "sound.h"
#include "windows.h"
#include "bgs.h"
#include "strings.h"
#include "../map/map_exec.h"
#include "../map/levels.h"
#include "../path/paths.h"
#include "draw.h"
#include "../path/path_literals.h"
#include "../strat/strat_player.h"
#include "../strat/strat_table.h"
#include "../sf_rtl.h"
#include "../variables.h"
#include "../renderer/transform.h"
#include "../renderer/shapes.h"
#include <string.h>
#include <stdio.h>

// PLANETS.ASM conversion hooks (local until planets.h exists).
void Planets_Init(void);
void Planets_Update(void);
bool drawplanetlines_l(void);

GameState g_game_state = GAME_STATE_BOOT;

// Progression timers (20 FPS ticks).
// Level clear: mapend only executes after the cl_* clear-demo submap and
// wipe script have run, so only a short settle delay is needed before the
// stage advance (score tally screen is not ported yet).
#define LEVEL_CLEAR_SETTLE_TICKS 60   // 3 s after mapend before next stage
#define DEATH_RESPAWN_TICKS      50   // 2.5 s after GF_PLAYERDEAD before reload

static int s_levelclear_ticks = 0;
static int s_death_ticks = 0;

// Draw list shared between game logic and renderer
static DrawListEntry s_draw_list[MAX_DRAW_LIST];
static int s_draw_list_count = 0;

// Title screen state
static bool s_title_loaded = false;

void Game_BeginGameplayFromPlanetSelect(void) {
    // Reset per-run player/game flags so a reload after death or a level
    // clear starts clean (GameVars_Init only runs once at boot).
    g_gameflags &= (uint8)~(GF_PLAYERDYING | GF_PLAYERDEAD);
    g_pshipflags = 0;
    g_pshipflags2 = 0;
    g_pshipflags3 = 0;
    g_pstratflags = 0;
    g_playerflymode = PFM_SHADOWS;
    g_splayerflymode = SPFM_NORM;
    g_freezestrats = 0;
    g_bossmaxhp = 0;
    g_meters = 0;
    g_maptrigger = 0;
    g_screenflashcnt = 0;
    g_circleanim = 0;
    g_doingwipe = 0;
    g_oncewipe = 0;
    Windows_Init();

    s_levelclear_ticks = 0;
    s_death_ticks = 0;

    // Initialize gameplay subsystems but preserve selected route/stage state.
    Obj_Init();
    GameCamera_Init();
    World_Init();
    Paths_Init();
    {
        const PathLiteralCatalog *catalog = PathLiterals_GetCatalog();
        Paths_LoadData(catalog->data, catalog->length, catalog->offsets, catalog->count);
    }
    MapExec_Init();
    Strat_RegisterAll();
    MapExec_LoadLevel(g_newmap);

    // Spawn the player (first alien allocated)
    Alien *player = Strat_SpawnPlayer();
    if (player) {
        printf("Game: player spawned at alien[%d]\n",
               (int)(player - g_aliens));

        // LEVEL1_1/2_1/3_1.ASM open with `initlevel 1_1i` whose BG block
        // (BGS.ASM bg_1_1i_1) runs `pstrat playeropening`. That opening
        // spawns the viewopening camera which sets GF_STRATDONE1 — the flag
        // MAP1_1A's scramble loop (`mapif chkstratdone1,.fin`) waits on.
        if (g_newmap == MAP_ID_1_1 || g_newmap == MAP_ID_2_1 ||
            g_newmap == MAP_ID_3_1) {
            Strat_PlayerOpening_Init(player);
        }
    }

    g_game_state = GAME_STATE_PLAYING;
    printf("Game: entering gameplay (route=%u stage=%u level=%u)\n",
           (unsigned)g_whichroute, (unsigned)g_stage, (unsigned)g_currentlevel);
}

void Game_Init(void) {
    printf("Game_Init: initializing...\n");

    // Initialize RAM (from initialise_ram in BOOTNMI.ASM)
    // Clear WRAM working area
    memset(&g_ram[0], 0, 0x2000);

    // Initialize game variables (from GILESALC.INC allocations)
    GameVars_Init();

    // Initialize subsystems
    Obj_Init();
    GameCamera_Init();
    World_Init();
    Paths_Init();
    {
        const PathLiteralCatalog *catalog = PathLiterals_GetCatalog();
        Paths_LoadData(catalog->data, catalog->length, catalog->offsets, catalog->count);
    }
    MapExec_Init();
    Strat_RegisterAll();
    Sound_Init();
    Windows_Init();
    Bgs_Init();
    Strings_Init();

    g_game_state = GAME_STATE_TITLE;
    s_title_loaded = false;
    printf("Game_Init: ready, entering title state\n");
}

// Title screen: driven by MAP_ID_TITLE map script
static void TitleScreen_Tick(void) {
    if (!s_title_loaded) {
        Obj_Init();
        GameCamera_Init();
        World_Init();
        Strat_RegisterAll();
        MapExec_LoadLevel(MAP_ID_TITLE);
        s_title_loaded = true;
        printf("Game: title map loaded\n");
    }

    Game_ClearDrawList();
    Obj_RunStrategies();
    GameCamera_Update();
    Draw_BuildList();

    // Check for Start to enter gameplay
    if (g_pad1_new & PAD_START) {
        Sound_PlaySE(0x10);
        Planets_Init();
        g_game_state = GAME_STATE_PLANET_SELECT;
        s_title_loaded = false;
        printf("Game: entering planet select\n");
    }
}

// Progression bridge: level completion and death/respawn/game-over.
// The original ran the score tally + planetscape interlude here (GAME.ASM);
// we advance the route directly via the planet-select stage graph.
static void Gameplay_ProgressTick(void) {
    // --- Death / respawn / game over ---
    // playerdead_strat (strat_player.c) sets GF_PLAYERDEAD and decrements
    // g_lives once the death animation has run its course.
    if ((g_gameflags & GF_PLAYERDEAD) != 0) {
        s_levelclear_ticks = 0;
        s_death_ticks++;
        if (s_death_ticks < DEATH_RESPAWN_TICKS) {
            return;
        }
        if (g_lives > 0) {
            printf("Game: player down — respawning (lives=%u)\n",
                   (unsigned)g_lives);
            // Reload the current map; route/stage/g_newmap are untouched.
            Game_BeginGameplayFromPlanetSelect();
        } else {
            printf("Game: game over — entering continue screen\n");
            s_death_ticks = 0;
            g_game_state = GAME_STATE_CONTINUE;
        }
        return;
    }
    s_death_ticks = 0;

    // --- Level completion ---
    // mapend runs after the clear-demo submap + wipe script, so the level
    // is visually wrapped up by the time g_levelfinished goes nonzero.
    // TODO: LE_ENTERBHOLE/LE_ENTERSPEC warp values currently advance the
    // normal route instead of warping (routechange* map callbacks already
    // rewrote g_routes for the black-hole cases).
    if (g_levelfinished != 0) {
        s_levelclear_ticks++;
        if (s_levelclear_ticks < LEVEL_CLEAR_SETTLE_TICKS) {
            return;
        }
        g_stage++;
        if (drawplanetlines_l()) {
            // Stage graph resolved the next map on this route into
            // g_newmap/g_currentlevel/g_currentplanet.
            printf("Game: level clear — advancing to stage %u\n",
                   (unsigned)g_stage);
            Game_BeginGameplayFromPlanetSelect();
        } else {
            printf("Game: route complete — entering ending\n");
            s_levelclear_ticks = 0;
            g_game_state = GAME_STATE_ENDING;
        }
        return;
    }
    s_levelclear_ticks = 0;
}

void Game_Tick(void) {
    // Main game tick -- called at fixed 20 FPS rate
    // Mirrors the NMI handler flow from NMI.ASM

    switch (g_game_state) {
    case GAME_STATE_BOOT:
        Game_Init();
        break;

    case GAME_STATE_TITLE:
        TitleScreen_Tick();
        break;

    case GAME_STATE_BRIEFING:
        // Use start/A/B to advance into route select.
        if (g_pad1_new & (PAD_START | PAD_A | PAD_B)) {
            Planets_Init();
            g_game_state = GAME_STATE_PLANET_SELECT;
        }
        break;

    case GAME_STATE_PLANET_SELECT:
        // No 3D scene on the map screen; drop stale title/gameplay entries
        // so they don't render behind the 2D map UI.
        Game_ClearDrawList();
        Planets_Update();
        break;

    case GAME_STATE_PLAYING:
        // Core gameplay tick
        Nmi_GameTick();
        // Level clear / death progression bridge
        Gameplay_ProgressTick();
        break;

    case GAME_STATE_CONTINUE:
        // CONTINUE.ASM semantics: continue restarts the current stage with
        // lives refilled; declining returns to the title screen.
        if (g_pad1_new & (PAD_START | PAD_A)) {
            g_lives = DEFAULT_LIVES;
            printf("Game: continue — retrying stage %u\n", (unsigned)g_stage);
            Game_BeginGameplayFromPlanetSelect();
        } else if (g_pad1_new & (PAD_B | PAD_SELECT)) {
            printf("Game: continue declined — returning to title\n");
            g_game_state = GAME_STATE_TITLE;
        }
        break;

    case GAME_STATE_ENDING:
        // Minimal ending flow: return to title on any confirm key.
        if (g_pad1_new & (PAD_START | PAD_A | PAD_B)) {
            g_game_state = GAME_STATE_TITLE;
        }
        break;
    }

    Bgs_Update();

    // Update windows/fade effects every tick
    Windows_Update();

    // Message state machine (send_message_l / friends_messages_l subset)
    Strings_Update();
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
