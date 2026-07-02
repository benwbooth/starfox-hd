#include "draw_list.h"
#include "gl_backend.h"
#include "transform.h"
#include "shapes.h"
#include "polygon.h"
#include <glad/glad.h>
#include <string.h>

// --- Wireframe entries (DL_FLAG_WIREFRAME) ---------------------------------
// Scratch buffer for edge extraction: 3 floats per vertex, 2 per segment.
#define DL_MAX_LINE_VERTS 4096

static float s_line_scratch[DL_MAX_LINE_VERTS * 3];

// Render a draw-list entry flagged DL_FLAG_WIREFRAME as a single-color edge
// wireframe. Face2 line faces contribute their segment; solid faces
// contribute their closed edge loop. The SNES wire variants (wiremydemo_*,
// *wirespacebar) are effectively single-color overlays, so a flat light grey
// stands in until the per-face material line path lands in shapes.c.
static void RenderWireframeEntry(uint16 shape_id, const float *model) {
    const Shape *shape = Shapes_Get(shape_id);
    int vert_count = 0;

    if (!shape || !shape->vertices || !shape->faces) return;

    for (int i = 0; i < shape->num_faces; i++) {
        const ShapeFace *face = &shape->faces[i];
        int edge_count = (face->num_verts == 2) ? 1 : face->num_verts;

        if (face->num_verts < 2) continue;

        for (int e = 0; e < edge_count; e++) {
            int a = face->vertex_indices[e];
            int b = face->vertex_indices[(e + 1) % face->num_verts];

            if (a >= shape->num_vertices || b >= shape->num_vertices) continue;
            if (vert_count + 2 > DL_MAX_LINE_VERTS) break;

            s_line_scratch[vert_count * 3 + 0] = shape->vertices[a].x;
            s_line_scratch[vert_count * 3 + 1] = shape->vertices[a].y;
            s_line_scratch[vert_count * 3 + 2] = shape->vertices[a].z;
            s_line_scratch[vert_count * 3 + 3] = shape->vertices[b].x;
            s_line_scratch[vert_count * 3 + 4] = shape->vertices[b].y;
            s_line_scratch[vert_count * 3 + 5] = shape->vertices[b].z;
            vert_count += 2;
        }
    }

    if (vert_count == 0) return;

    GlBackend_SetMat4(g_flat_shader, "uModel", model);
    GlBackend_SetVec4(g_flat_shader, "uColor", 0.75f, 0.78f, 0.82f, 1.0f);
    GlBackend_DrawLines(s_line_scratch, vert_count);
}

static int16 lerp_angle8(int16 from, int16 to, float t) {
    int16 a8 = (int16)(from & 0xFF);
    int16 b8 = (int16)(to & 0xFF);
    int16 diff = (int16)(b8 - a8);
    if (diff > 127) diff -= 256;
    if (diff < -128) diff += 256;
    int16 out8 = (int16)(a8 + (int16)(diff * t));
    return (int16)(out8 & 0xFF);
}

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
    out->rx = lerp_angle8(a->rx, b->rx, alpha);
    out->ry = lerp_angle8(a->ry, b->ry, alpha);
    out->rz = lerp_angle8(a->rz, b->rz, alpha);
    out->shape_id = b->shape_id;
    out->color_table = b->color_table;
    out->flags = b->flags;
    out->explosion_cnt = b->explosion_cnt;
    out->sflags = b->sflags;
    out->anim_frame = b->anim_frame;
    out->col_frame = b->col_frame;
    out->sort_z = b->sort_z;
    out->depth_offset = b->depth_offset;
}

// --- Drop shadows (MARIO/MDRAWLIS.MC shadow pass) --------------------------
// The SNES draws a dedicated shadow pass when playerflymode has pfm_shadows:
// every object with asf_shadow gets its shape re-drawn in `truecolourshadow`
// at ground height (BGS.ASM `shadowheight`, 0 for all planet levels). The
// original substitutes the header's sh_shadow (a flat outline mesh); here we
// project the object's own mesh onto the ground plane by flattening the
// world-space Y row of the model matrix, which yields the same silhouette.
static void RenderShadow(const DrawListEntry *e) {
    const Shape *shape = Shapes_Get(e->shape_id);
    if (!shape || !shape->gpu_ready || shape->num_triangles <= 0) return;
    if (e->y > 0) return;  // below ground (SNES +Y is down): no shadow

    float model[16];
    // Ground height 0 (BGS.ASM shadowheight for planet levels).
    Transform_BuildModelMatrix(model, e->x, 0, e->z, e->rx, e->ry, e->rz);
    // Flatten world-space Y (row 1 of the column-major matrix) so all
    // vertices land on the ground plane; lift slightly to avoid coplanar
    // artifacts with future ground geometry.
    model[1] = 0.0f; model[5] = 0.0f; model[9] = 0.0f;
    model[13] = 0.5f;

    GlBackend_SetMat4(g_flat_shader, "uModel", model);
    GlBackend_SetVec4(g_flat_shader, "uColor", 0.0f, 0.0f, 0.0f, 0.40f);
    glBindVertexArray(shape->vao);
    glDrawArrays(GL_TRIANGLES, 0, shape->num_triangles * 3);
    glBindVertexArray(0);
}

void DrawList_Render(const DrawListEntry *prev, int prev_count,
                     const DrawListEntry *curr, int curr_count,
                     float alpha) {
    if (curr_count == 0) return;

    // The game tick is 20Hz but rendering is uncapped: rebuild the view
    // matrix from the interpolated camera so the camera glides with the
    // interpolated objects instead of stepping once per tick.
    Transform_SetViewLerp(alpha);

    glUseProgram(g_flat_shader);
    GlBackend_SetMat4(g_flat_shader, "uView", Transform_GetView());
    GlBackend_SetMat4(g_flat_shader, "uProj", Transform_GetProjection());

    // Interpolated entries that want a drop shadow (collected during the
    // main pass, drawn afterwards as a translucent overlay).
    static DrawListEntry shadow_list[MAX_DRAW_LIST];
    int shadow_count = 0;

    // Pair current entries with the previous frame's entries by stable
    // object id (alien index), not by list position: spawns/removals
    // reshuffle the draw list order between game ticks, and index-based
    // pairing interpolates against the wrong object (visible jitter).
    // MAX_OBJECTS bounds the id space (obj_id = alien index + 1).
    static int16 prev_by_id[MAX_OBJECTS + 1];
    for (int i = 0; i <= MAX_OBJECTS; i++) prev_by_id[i] = -1;
    for (int i = 0; i < prev_count; i++) {
        if (prev[i].obj_id <= MAX_OBJECTS) {
            prev_by_id[prev[i].obj_id] = (int16)i;
        }
    }

    for (int i = 0; i < curr_count; i++) {
        const DrawListEntry *entry = &curr[i];

        if (!(entry->flags & DL_FLAG_VISIBLE)) continue;

        // Interpolate if we have a matching previous entry
        DrawListEntry interp;
        int prev_idx = (entry->obj_id != 0 && entry->obj_id <= MAX_OBJECTS)
                           ? prev_by_id[entry->obj_id] : -1;
        if (prev_idx >= 0 && prev[prev_idx].shape_id == entry->shape_id) {
            InterpolateEntry(&interp, &prev[prev_idx], entry, alpha);
        } else {
            interp = *entry;
        }

        // Build model matrix
        float model[16];
        Transform_BuildModelMatrix(model, interp.x, interp.y, interp.z,
                                    interp.rx, interp.ry, interp.rz);

        // Render the shape. Wireframe-flagged entries draw as edge lines
        // instead of filled triangles.
        if (interp.flags & DL_FLAG_WIREFRAME) {
            RenderWireframeEntry(interp.shape_id, model);
        } else {
            Shapes_Render(interp.shape_id, interp.anim_frame, interp.col_frame,
                          interp.color_table, interp.explosion_cnt, model);
        }

        // Queue the drop shadow (skip exploding objects — the SNES stops
        // shadowing once the shape breaks apart into particles).
        if ((interp.flags & DL_FLAG_SHADOW) && interp.explosion_cnt == 0 &&
            shadow_count < MAX_DRAW_LIST) {
            shadow_list[shadow_count++] = interp;
        }
    }

    // --- Shadow pass (after the opaque pass so depth testing hides
    // shadow fragments behind solid geometry) ---
    if (shadow_count > 0) {
        glEnable(GL_BLEND);
        glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        glDepthMask(GL_FALSE);
        for (int i = 0; i < shadow_count; i++) {
            RenderShadow(&shadow_list[i]);
        }
        glDepthMask(GL_TRUE);
        glDisable(GL_BLEND);
    }
}
