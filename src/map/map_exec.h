#ifndef STARFOX_MAP_MAP_EXEC_H
#define STARFOX_MAP_MAP_EXEC_H

#include "../types.h"

void MapExec_Init(void);
void MapExec_LoadLevel(uint32 map_id);
void MapExec_Tick(void);

#endif // STARFOX_MAP_MAP_EXEC_H
