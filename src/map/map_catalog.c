#include "map_catalog.h"

const MapLevelData *MapCatalog_GetLevel(uint32 map_id) {
    return Levels_GetMapData(map_id);
}
