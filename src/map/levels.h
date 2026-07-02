#ifndef STARFOX_MAP_LEVELS_H
#define STARFOX_MAP_LEVELS_H

#include "../types.h"

typedef struct {
    const uint8 *data;
    uint32 length;
} MapLevelData;

// Planet map IDs (matches PLANETS.ASM path table values used in planets.c).
#define MAP_ID_NONE      0u
#define MAP_ID_1_1       1u
#define MAP_ID_1_2       2u
#define MAP_ID_1_3       3u
#define MAP_ID_1_4       4u
#define MAP_ID_1_5       5u
#define MAP_ID_1_6       6u
#define MAP_ID_2_1       7u
#define MAP_ID_2_2       8u
#define MAP_ID_2_3       9u
#define MAP_ID_2_4       10u
#define MAP_ID_2_5       11u
#define MAP_ID_2_6       12u
#define MAP_ID_3_1       13u
#define MAP_ID_3_2       14u
#define MAP_ID_3_3       15u
#define MAP_ID_3_4       16u
#define MAP_ID_3_5       17u
#define MAP_ID_3_6       18u
#define MAP_ID_3_7       19u
#define MAP_ID_BLACKHOLE 20u
#define MAP_ID_SPECIAL   21u
#define MAP_ID_FINAL     22u
#define MAP_ID_INTRO     23u
#define MAP_ID_TITLE     24u
#define MAP_ID_CONTINUE  25u
#define MAP_ID_WAIT      26u
#define MAP_ID_PLANET    27u
#define MAP_ID_CREDITS   28u
#define MAP_ID_TRAINING  29u

const MapLevelData *Levels_GetMapData(uint32 map_id);

#endif // STARFOX_MAP_LEVELS_H
