#ifndef STARFOX_SHAPES_H
#define STARFOX_SHAPES_H

#include "../types.h"

// Shape data loaded from extracted ROM assets / assembly source
// Star Fox shapes are flat-shaded polygon meshes with:
// - Vertices (3D points)
// - Faces (polygon indices + color) — up to 12-sided
// - LOD chains (multiple detail levels)

typedef struct {
    float x, y, z;
} ShapeVertex;

typedef struct {
    uint16 vertex_indices[12]; // Up to 12-sided polygon
    uint8 num_verts;
    uint8 color_index;         // Index into color table
} ShapeFace;

typedef struct {
    // Source data
    ShapeVertex *vertices;
    int num_vertices;
    ShapeFace *faces;
    int num_faces;
    float *face_normals;         // Derived flat normals, 3 floats per face
    // GPU resources (expanded triangle VBO)
    uint32 vao;
    uint32 vbo;
    int num_triangles;            // Total triangles after fan triangulation
    int *face_tri_start;          // Start triangle index per face
    int *face_tri_count;          // Number of triangles per face
    int num_line_verts;           // GL_LINES vertices appended after the tris
    int *face_line_first;         // Per-face first line vertex in VBO, -1=none
    bool gpu_ready;
} Shape;

// Well-known active shape IDs. Keep the canonical runtime ids aligned with the
// live `def_shape` order where that order is already used by current slices.
#define SHAPE_MYSHIP_4     2   // def_shape myship_4
#define SHAPE_ARWING       SHAPE_MYSHIP_4
#define SHAPE_ARWING_L     SHAPE_MYSHIP_4
#define SHAPE_ARWING_R     SHAPE_MYSHIP_4
#define SHAPE_ARWING_B     SHAPE_MYSHIP_4
#define SHAPE_BOSS7_1     56   // def_shape boss_7_1 closed body
#define SHAPE_BOSS_7_1    SHAPE_BOSS7_1

// Active raw shape-word ids that still remain live at runtime.
#define SHAPE_BOSS7_0    240   // boss_7_0 hatch part
#define SHAPE_BOSS7_1O   242   // boss_7_1o opened body shell
#define SHAPE_BOSS7_2    243   // boss_7_2 shield
#define SHAPE_BOSS7_3    244   // boss_7_3 upper launcher
#define SHAPE_BOSS7_4    245   // boss_7_4 lower launcher
#define SHAPE_BOSS_7_0     240 // boss7 hatch, frame 0 mesh
#define SHAPE_BOSS_7_1O    242 // boss7 open body shell
#define SHAPE_BOSS_7_2     243 // boss7 shield
#define SHAPE_BOSS_7_3     244 // boss7 upper launcher, frame 0 mesh
#define SHAPE_BOSS_7_4     245 // boss7 lower launcher, frame 0 mesh

// Temporary flat slots for the small set of live raw 16-bit shape words still
// emitted by current literal slices.
#define SHAPE_ALIAS_MOTHER1     278
#define SHAPE_ALIAS_OP_0        508
#define SHAPE_ALIAS_OP_1        509
#define SHAPE_ALIAS_OP_2        510

void Shapes_Init(void);
void Shapes_Shutdown(void);

// Register built-in shapes (Arwing, etc.) from hardcoded data
void Shapes_RegisterBuiltins(void);

// Canonicalize the live raw/16-bit shape words used by current literal slices
// into bounded flat runtime ids. Unknown words are returned unchanged.
uint16 Shapes_ResolveShapeWord(uint16 shape_id);

// Register a shape from vertex/face arrays (copies data, uploads to GPU)
bool Shapes_Register(uint16 shape_id, const ShapeVertex *verts, int nverts,
                     const ShapeFace *faces, int nfaces);

// Render a shape (must have flat shader bound, model/view/proj set)
void Shapes_Render(uint16 shape_id, uint8 anim_frame, uint8 col_frame,
                   uint16 color_table, uint8 explosion_state,
                   const float *model_matrix);

// Depth-band threshold records from ASM/COLTABS.ASM `depthtables`, selected
// per background segment like the `set_zdepthtable` map macro. Controls both
// the COLDEPTH color bank and the COLLITE shade group by object distance.
enum {
    SHAPES_DEPTHZ_NORMAL = 0,
    SHAPES_DEPTHZ_TUNNEL = 1,
    SHAPES_DEPTHZ_MIST = 2,
    SHAPES_DEPTHZ_STAGE1 = 3,
    SHAPES_DEPTHZ_COUNT = 4,
};
void Shapes_SetDepthTable(uint8 table);

// Get shape for direct access
const Shape *Shapes_Get(uint16 shape_id);

#endif // STARFOX_SHAPES_H
