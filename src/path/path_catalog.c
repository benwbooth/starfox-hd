#include "path_catalog.h"
#include "path_literals.h"

const PathCatalog *PathCatalog_Get(void) {
    static PathCatalog catalog;
    const PathLiteralCatalog *literal = PathLiterals_GetCatalog();

    catalog.data = literal->data;
    catalog.length = literal->length;
    catalog.offsets = literal->offsets;
    catalog.count = literal->count;
    return &catalog;
}
