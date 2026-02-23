#ifndef STARFOX_POLYGON_H
#define STARFOX_POLYGON_H

#include "../types.h"

void Polygon_Init(void);
void Polygon_Shutdown(void);

// Draw an immediate-mode flat-shaded polygon (for debug/fallback)
void Polygon_DrawImmediate(const float *verts, int num_verts,
                            float r, float g, float b, float a);

#endif // STARFOX_POLYGON_H
