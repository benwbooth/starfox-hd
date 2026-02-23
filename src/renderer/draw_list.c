#include "draw_list.h"
#include "gl_backend.h"
#include "transform.h"
#include "shapes.h"
#include "polygon.h"
#include <glad/glad.h>
#include <string.h>

void DrawList_Init(void) {
    Polygon_Init();
    Shapes_Init();
}

void DrawList_Shutdown(void) {
    Shapes_Shutdown();
    Polygon_Shutdown();
}

// Interpolate between two draw list entries
static void InterpolateEntry(DrawListEntry *out,
                             const DrawListEntry *a,
                             const DrawListEntry *b,
                             float alpha) {
    out->x = (int32)((float)a->x + ((float)b->x - (float)a->x) * alpha);
    out->y = (int32)((float)a->y + ((float)b->y - (float)a->y) * alpha);
    out->z = (int32)((float)a->z + ((float)b->z - (float)a->z) * alpha);
    // For angles, use shortest path interpolation
    out->rx = b->rx;  // TODO: proper angle lerp
    out->ry = b->ry;
    out->rz = b->rz;
    out->shape_id = b->shape_id;
    out->color_table = b->color_table;
    out->lod_depth = b->lod_depth;
    out->flags = b->flags;
    out->explosion_state = b->explosion_state;
}

void DrawList_Render(const DrawListEntry *prev, int prev_count,
                     const DrawListEntry *curr, int curr_count,
                     float alpha) {
    if (curr_count == 0) return;

    glUseProgram(g_flat_shader);
    GlBackend_SetMat4(g_flat_shader, "uView", Transform_GetView());
    GlBackend_SetMat4(g_flat_shader, "uProj", Transform_GetProjection());

    for (int i = 0; i < curr_count; i++) {
        const DrawListEntry *entry = &curr[i];

        if (!(entry->flags & DL_FLAG_VISIBLE)) continue;

        // Interpolate if we have a matching previous entry
        DrawListEntry interp;
        if (i < prev_count && prev[i].shape_id == entry->shape_id) {
            InterpolateEntry(&interp, &prev[i], entry, alpha);
        } else {
            interp = *entry;
        }

        // Build model matrix
        float model[16];
        Transform_BuildModelMatrix(model, interp.x, interp.y, interp.z,
                                    interp.rx, interp.ry, interp.rz);
        GlBackend_SetMat4(g_flat_shader, "uModel", model);

        // Render the shape
        Shapes_Render(interp.shape_id, interp.color_table, interp.explosion_state);
    }
}
