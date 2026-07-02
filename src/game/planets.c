// PLANETS.ASM -> C conversion
// Minimal planet/route selection control flow and stage path parsing.

#include "boot.h"
#include "game_vars.h"
#include "../sf_rtl.h"
#include "../variables.h"

#define PLANET_ROUTE_CHOICES 3
#define PLANET_ROUTE_SLOTS   4

// Gameplay boot hook implemented in boot.c.
extern void Game_BeginGameplayFromPlanetSelect(void);

typedef enum {
    MAP_ID_NONE = 0,
    MAP_ID_1_1,
    MAP_ID_1_2,
    MAP_ID_1_3,
    MAP_ID_1_4,
    MAP_ID_1_5,
    MAP_ID_1_6,
    MAP_ID_2_1,
    MAP_ID_2_2,
    MAP_ID_2_3,
    MAP_ID_2_4,
    MAP_ID_2_5,
    MAP_ID_2_6,
    MAP_ID_3_1,
    MAP_ID_3_2,
    MAP_ID_3_3,
    MAP_ID_3_4,
    MAP_ID_3_5,
    MAP_ID_3_6,
    MAP_ID_3_7,
    MAP_ID_BLACKHOLE,
    MAP_ID_SPECIAL,
} MapId;

typedef enum {
    PATH_ID_INVALID = 0,
    PATH_ID_1,
    PATH_ID_2,
    PATH_ID_3,
    PATH_ID_4,
    PATH_ID_5,
    PATH_ID_6,
    PATH_ID_7,
    PATH_ID_8,
    PATH_ID_9,
    PATH_ID_10,
    PATH_ID_11,
    PATH_ID_12,
    PATH_ID_13,
    PATH_ID_14,
    PATH_ID_15,
    PATH_ID_16,
    PATH_ID_17,
    PATH_ID_18,
    PATH_ID_19,
    PATH_ID_20,
    PATH_ID_21,
    PATH_ID_22,
    PATH_ID_END3,
    PATH_ID_END2,
    PATH_ID_END1,
    PATH_ID_OTHEREND,
    PATH_ID_COUNT
} PlanetPathId;

typedef enum {
    PATH_NEXT_NONE = 0,
    PATH_NEXT_DIRECT,
    PATH_NEXT_ROUTE_CHOICE
} PathNextType;

typedef struct {
    uint8 planet;
    uint32 map_id;
    uint8 peppermsg;
    uint8 currentlevel;
    uint8 next_type;
    uint8 choice_slot;
    uint16 next_path;
} StagePathNode;

static const uint16 kRouteRoots[PLANET_ROUTE_CHOICES] = {
    PATH_ID_1,
    PATH_ID_6,
    PATH_ID_11,
};

static const StagePathNode kStagePaths[PATH_ID_COUNT] = {
    [PATH_ID_1]  = {  0, MAP_ID_2_1,      88, 1, PATH_NEXT_ROUTE_CHOICE, 2, PATH_ID_INVALID },
    [PATH_ID_2]  = {  3, MAP_ID_2_2,      89, 1, PATH_NEXT_DIRECT,       0, PATH_ID_3       },
    [PATH_ID_3]  = {  5, MAP_ID_2_3,     106, 1, PATH_NEXT_DIRECT,       0, PATH_ID_4       },
    [PATH_ID_4]  = {  9, MAP_ID_2_4,     107, 1, PATH_NEXT_DIRECT,       0, PATH_ID_5       },
    [PATH_ID_5]  = { 15, MAP_ID_2_5,     114, 1, PATH_NEXT_DIRECT,       0, PATH_ID_END2    },
    [PATH_ID_6]  = {  0, MAP_ID_1_1,      88, 0, PATH_NEXT_ROUTE_CHOICE, 1, PATH_ID_INVALID },
    [PATH_ID_7]  = {  2, MAP_ID_1_2,      89, 0, PATH_NEXT_DIRECT,       0, PATH_ID_8       },
    [PATH_ID_8]  = {  6, MAP_ID_1_3,      90, 0, PATH_NEXT_DIRECT,       0, PATH_ID_9       },
    [PATH_ID_9]  = {  8, MAP_ID_1_4,      91, 0, PATH_NEXT_DIRECT,       0, PATH_ID_10      },
    [PATH_ID_10] = { 15, MAP_ID_1_5,      92, 0, PATH_NEXT_DIRECT,       0, PATH_ID_END1    },
    [PATH_ID_11] = {  0, MAP_ID_3_1,     108, 2, PATH_NEXT_ROUTE_CHOICE, 0, PATH_ID_INVALID },
    [PATH_ID_12] = {  1, MAP_ID_3_2,     109, 2, PATH_NEXT_DIRECT,       0, PATH_ID_13      },
    [PATH_ID_13] = {  4, MAP_ID_3_3,     110, 2, PATH_NEXT_DIRECT,       0, PATH_ID_14      },
    [PATH_ID_14] = {  7, MAP_ID_3_4,     111, 2, PATH_NEXT_DIRECT,       0, PATH_ID_15      },
    [PATH_ID_15] = { 11, MAP_ID_3_5,     112, 2, PATH_NEXT_DIRECT,       0, PATH_ID_16      },
    [PATH_ID_16] = { 15, MAP_ID_3_6,     116, 2, PATH_NEXT_DIRECT,       0, PATH_ID_END3    },
    [PATH_ID_17] = {  3, MAP_ID_2_2,      89, 1, PATH_NEXT_ROUTE_CHOICE, 3, PATH_ID_INVALID },
    [PATH_ID_18] = { 10, MAP_ID_BLACKHOLE,113, 0, PATH_NEXT_DIRECT,       0, PATH_ID_4       },
    [PATH_ID_19] = { 10, MAP_ID_BLACKHOLE,113, 0, PATH_NEXT_DIRECT,       0, PATH_ID_10      },
    [PATH_ID_20] = { 10, MAP_ID_BLACKHOLE,113, 0, PATH_NEXT_DIRECT,       0, PATH_ID_14      },
    [PATH_ID_21] = {  2, MAP_ID_1_2,      89, 0, PATH_NEXT_ROUTE_CHOICE, 3, PATH_ID_INVALID },
    [PATH_ID_22] = {  1, MAP_ID_3_2,     109, 2, PATH_NEXT_DIRECT,       0, PATH_ID_OTHEREND},
    [PATH_ID_END3] = {15, MAP_ID_3_7,    116, 2, PATH_NEXT_NONE,         0, PATH_ID_INVALID },
    [PATH_ID_END2] = {15, MAP_ID_2_6,    114, 1, PATH_NEXT_NONE,         0, PATH_ID_INVALID },
    [PATH_ID_END1] = {15, MAP_ID_1_6,     92, 0, PATH_NEXT_NONE,         0, PATH_ID_INVALID },
    [PATH_ID_OTHEREND] = {14, MAP_ID_SPECIAL, 115, 0, PATH_NEXT_NONE,    0, PATH_ID_INVALID },
};

static bool s_route_converted;

static uint8 normalize_route_choice(uint8 route) {
    if (route >= PLANET_ROUTE_CHOICES) {
        return (uint8)(route % PLANET_ROUTE_CHOICES);
    }
    return route;
}

static uint16 normalize_path_id(uint16 path_id) {
    if (path_id >= PATH_ID_COUNT) return PATH_ID_INVALID;
    return path_id;
}

void convertroute(void) {
    if (g_whichroute == 0) {
        g_whichroute = 1;
    } else if (g_whichroute == 1) {
        g_whichroute = 0;
    }
}

void routechange1(void) {
    g_routes[0] = PATH_ID_22;
    g_nebula_on = PATH_ID_22;
}

void routechange2(void) {
    g_routes[1] = PATH_ID_21;
}

void routechange3(void) {
    g_routes[2] = PATH_ID_17;
}

void routechangebhole1(void) {
    g_routes[3] = PATH_ID_19;
}

void routechangebhole2(void) {
    g_routes[3] = PATH_ID_18;
}

void routechangebhole3(void) {
    g_routes[3] = PATH_ID_20;
}

void routechange1_l(void) { routechange1(); }
void routechange2_l(void) { routechange2(); }
void routechange3_l(void) { routechange3(); }
void routechangebhole1_l(void) { routechangebhole1(); }
void routechangebhole2_l(void) { routechangebhole2(); }
void routechangebhole3_l(void) { routechangebhole3(); }

bool drawplanetlines_l(void) {
    uint16 path_id;
    uint16 stage_remaining;
    int guard;

    g_whichroute = normalize_route_choice(g_whichroute);
    path_id = kRouteRoots[g_whichroute];
    stage_remaining = (uint8)g_stage;

    for (guard = 0; guard < 64; guard++) {
        const StagePathNode *node;

        path_id = normalize_path_id(path_id);
        if (path_id == PATH_ID_INVALID) break;
        node = &kStagePaths[path_id];

        g_currentplanet = (int16)node->planet;
        g_newmap = node->map_id;
        g_peppermsg = node->peppermsg;
        g_currentlevel = node->currentlevel;

        if (stage_remaining == 0) {
            return true;
        }
        stage_remaining--;

        if (node->next_type == PATH_NEXT_DIRECT) {
            path_id = node->next_path;
            continue;
        }
        if (node->next_type == PATH_NEXT_ROUTE_CHOICE) {
            if (node->choice_slot >= PLANET_ROUTE_SLOTS) break;
            path_id = g_routes[node->choice_slot];
            continue;
        }
        break;
    }

    g_currentplanet = -1;
    g_newmap = MAP_ID_NONE;
    g_peppermsg = 0;
    return false;
}

// ---------------------------------------------------------------------------
// Read-only render accessor (used by src/renderer/ui.c).
// Walks the stage-path graph for `route` exactly the way drawplanetlines_l
// does — following current g_routes[] choice slots — and writes the planet id
// visited at each stage. Does not mutate any game state.
// ---------------------------------------------------------------------------
int Planets_GetRouteNodes(uint8 route, uint8 *planets_out, int max_nodes) {
    uint16 path_id;
    int count = 0;
    int guard;

    if (!planets_out || max_nodes <= 0) return 0;

    route = normalize_route_choice(route);
    path_id = kRouteRoots[route];

    for (guard = 0; guard < 64 && count < max_nodes; guard++) {
        const StagePathNode *node;

        path_id = normalize_path_id(path_id);
        if (path_id == PATH_ID_INVALID) break;
        node = &kStagePaths[path_id];

        planets_out[count++] = node->planet;

        if (node->next_type == PATH_NEXT_DIRECT) {
            path_id = node->next_path;
            continue;
        }
        if (node->next_type == PATH_NEXT_ROUTE_CHOICE) {
            if (node->choice_slot >= PLANET_ROUTE_SLOTS) break;
            path_id = g_routes[node->choice_slot];
            continue;
        }
        break;
    }

    return count;
}

// Same walk, but reports the PATH_ID_* sequence instead of planet ids.
// The renderer uses this to index its literal port of the PLANETS.ASM
// stagepaths line-segment tables (PATHUP/PATHRIGHT/... step lists).
int Planets_GetRoutePathIds(uint8 route, uint16 *paths_out, int max_nodes) {
    uint16 path_id;
    int count = 0;
    int guard;

    if (!paths_out || max_nodes <= 0) return 0;

    route = normalize_route_choice(route);
    path_id = kRouteRoots[route];

    for (guard = 0; guard < 64 && count < max_nodes; guard++) {
        const StagePathNode *node;

        path_id = normalize_path_id(path_id);
        if (path_id == PATH_ID_INVALID) break;
        node = &kStagePaths[path_id];

        paths_out[count++] = path_id;

        if (node->next_type == PATH_NEXT_DIRECT) {
            path_id = node->next_path;
            continue;
        }
        if (node->next_type == PATH_NEXT_ROUTE_CHOICE) {
            if (node->choice_slot >= PLANET_ROUTE_SLOTS) break;
            path_id = g_routes[node->choice_slot];
            continue;
        }
        break;
    }

    return count;
}

void Planets_Init(void) {
    // initplanets_l defaults
    g_lives = DEFAULT_LIVES;
    g_routes[0] = PATH_ID_12;
    g_routes[1] = PATH_ID_7;
    g_routes[2] = PATH_ID_2;
    g_routes[3] = PATH_ID_19;
    g_stage = 0;
    g_mapmode = 0;
    g_bg_dmalist = 0;

    s_route_converted = false;
}

void Planets_Update(void) {
    uint16 press = g_pad1_new;

    if (!s_route_converted) {
        convertroute();
        s_route_converted = true;
    }

    (void)drawplanetlines_l();

    if (press & (PAD_LEFT | PAD_SELECT | PAD_UP)) {
        g_whichroute = normalize_route_choice(g_whichroute);
        g_whichroute = (g_whichroute == 0) ? 2 : (uint8)(g_whichroute - 1);
        (void)drawplanetlines_l();
        return;
    }

    if (press & (PAD_RIGHT | PAD_DOWN)) {
        g_whichroute = normalize_route_choice(g_whichroute);
        g_whichroute = (uint8)(g_whichroute + 1);
        if (g_whichroute >= PLANET_ROUTE_CHOICES) g_whichroute = 0;
        (void)drawplanetlines_l();
        return;
    }

    if (press & (PAD_START | PAD_A | PAD_B)) {
        g_stage = 0;
        g_currentplanet = 0;
        (void)drawplanetlines_l();

        // planetseq_l converts route back before entering gameplay.
        convertroute();
        s_route_converted = false;

        Game_BeginGameplayFromPlanetSelect();
    }
}
