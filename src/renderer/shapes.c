#include "shapes.h"
#include "gl_backend.h"
#include <glad/glad.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#define MAX_SHAPES 512

static Shape s_shapes[MAX_SHAPES];
static int s_shape_count = 0;

void Shapes_Init(void) {
    memset(s_shapes, 0, sizeof(s_shapes));
    s_shape_count = 0;
}

void Shapes_Shutdown(void) {
    for (int i = 0; i < MAX_SHAPES; i++) {
        if (s_shapes[i].gpu_ready) {
            glDeleteVertexArrays(1, &s_shapes[i].vao);
            glDeleteBuffers(1, &s_shapes[i].vbo);
            glDeleteBuffers(1, &s_shapes[i].ebo);
        }
        free(s_shapes[i].vertices);
        free(s_shapes[i].faces);
    }
    memset(s_shapes, 0, sizeof(s_shapes));
}

bool Shapes_Load(uint16 shape_id, const char *path) {
    if (shape_id >= MAX_SHAPES) return false;

    // TODO: Parse shape data from extracted ROM assets
    // For now, this is a stub that will be filled in by shape_tool.py output
    (void)path;

    return false;
}

// Star Fox color palette — SNES RGB555 to float RGBA
// These are the standard game colors used by the flat shader
static const float s_color_palette[][4] = {
    { 0.0f, 0.0f, 0.0f, 1.0f },       // 0: black
    { 1.0f, 1.0f, 1.0f, 1.0f },       // 1: white
    { 0.8f, 0.8f, 0.8f, 1.0f },       // 2: light gray
    { 0.5f, 0.5f, 0.5f, 1.0f },       // 3: gray
    { 0.3f, 0.3f, 0.3f, 1.0f },       // 4: dark gray
    { 0.0f, 0.0f, 0.8f, 1.0f },       // 5: blue
    { 0.0f, 0.0f, 0.5f, 1.0f },       // 6: dark blue
    { 0.8f, 0.0f, 0.0f, 1.0f },       // 7: red
    { 0.5f, 0.0f, 0.0f, 1.0f },       // 8: dark red
    { 0.0f, 0.8f, 0.0f, 1.0f },       // 9: green
    { 0.0f, 0.5f, 0.0f, 1.0f },       // 10: dark green
    { 0.8f, 0.8f, 0.0f, 1.0f },       // 11: yellow
    { 0.8f, 0.5f, 0.0f, 1.0f },       // 12: orange
    { 0.5f, 0.0f, 0.8f, 1.0f },       // 13: purple
    { 0.0f, 0.8f, 0.8f, 1.0f },       // 14: cyan
    { 0.8f, 0.4f, 0.4f, 1.0f },       // 15: pink
};
#define NUM_PALETTE_COLORS (sizeof(s_color_palette) / sizeof(s_color_palette[0]))

void Shapes_Render(uint16 shape_id, uint16 color_table, uint8 explosion_state) {
    if (shape_id >= MAX_SHAPES) return;
    const Shape *shape = &s_shapes[shape_id];
    if (!shape->gpu_ready || !shape->faces) return;

    (void)color_table;
    (void)explosion_state;

    // Render each face as a triangle fan
    glBindVertexArray(shape->vao);
    for (int i = 0; i < shape->num_faces; i++) {
        const ShapeFace *face = &shape->faces[i];

        // Set face color
        int ci = face->color_index % NUM_PALETTE_COLORS;
        GlBackend_SetVec4(g_flat_shader, "uColor",
            s_color_palette[ci][0], s_color_palette[ci][1],
            s_color_palette[ci][2], s_color_palette[ci][3]);

        // Draw as triangle fan (first vertex + pairs of remaining)
        glDrawElements(GL_TRIANGLE_FAN, face->num_verts, GL_UNSIGNED_SHORT,
                       (void *)(uintptr_t)(i * 8 * sizeof(uint16)));
    }
    glBindVertexArray(0);
}

const Shape *Shapes_Get(uint16 shape_id) {
    if (shape_id >= MAX_SHAPES) return NULL;
    return &s_shapes[shape_id];
}
