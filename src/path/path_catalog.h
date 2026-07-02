#ifndef STARFOX_PATH_PATH_CATALOG_H
#define STARFOX_PATH_PATH_CATALOG_H

#include "../types.h"

typedef struct {
    const uint8 *data;
    uint32 length;
    const uint16 *offsets;
    uint16 count;
} PathCatalog;

const PathCatalog *PathCatalog_Get(void);

#endif // STARFOX_PATH_PATH_CATALOG_H
