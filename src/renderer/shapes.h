#ifndef STARFOX_SHAPES_H
#define STARFOX_SHAPES_H

#include "../types.h"

// Shape data loaded from extracted ROM assets
// Star Fox shapes are flat-shaded polygon meshes with:
// - Vertices (3D points)
// - Faces (polygon indices + color)
// - LOD chains (multiple detail levels)
// - Collision boxes

typedef struct {
    float x, y, z;
} ShapeVertex;

typedef struct {
    uint16 vertex_indices[8]; // Up to 8-sided polygon
    uint8 num_verts;
    uint8 color_index;        // Index into color table
} ShapeFace;

typedef struct {
    ShapeVertex *vertices;
    int num_vertices;
    ShapeFace *faces;
    int num_faces;
    // GPU resources
    uint32 vao;
    uint32 vbo;
    uint32 ebo;
    bool gpu_ready;
} Shape;

void Shapes_Init(void);
void Shapes_Shutdown(void);

// Load a shape from extracted data file
bool Shapes_Load(uint16 shape_id, const char *path);

// Render a shape (must have flat shader bound)
void Shapes_Render(uint16 shape_id, uint16 color_table, uint8 explosion_state);

// Get shape for direct access
const Shape *Shapes_Get(uint16 shape_id);

#endif // STARFOX_SHAPES_H
