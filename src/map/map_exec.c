// Map execution facade.
// Keeps the old map module boundary while delegating to world.c VM runtime.

#include "map_exec.h"
#include "levels.h"
#include "../game/world.h"

static uint32 s_loaded_map_id;

void MapExec_Init(void) {
    s_loaded_map_id = MAP_ID_NONE;
}

void MapExec_LoadLevel(uint32 map_id) {
    const MapLevelData *level = Levels_GetMapData(map_id);
    if (!level || !level->data || level->length == 0) {
        level = Levels_GetMapData(MAP_ID_NONE);
    }

    World_LoadLevel(level->data, level->length);
    s_loaded_map_id = map_id;
}

void MapExec_Tick(void) {
    (void)s_loaded_map_id;
    World_UpdateObjects();
}
