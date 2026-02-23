#include "polygon.h"
#include "gl_backend.h"
#include <glad/glad.h>
#include <string.h>

// Immediate-mode polygon rendering VAO/VBO for debug and fallback
static GLuint s_imm_vao;
static GLuint s_imm_vbo;

void Polygon_Init(void) {
    glGenVertexArrays(1, &s_imm_vao);
    glGenBuffers(1, &s_imm_vbo);

    glBindVertexArray(s_imm_vao);
    glBindBuffer(GL_ARRAY_BUFFER, s_imm_vbo);
    // Pre-allocate for up to 32 vertices
    glBufferData(GL_ARRAY_BUFFER, sizeof(float) * 3 * 32, NULL, GL_DYNAMIC_DRAW);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, 3 * sizeof(float), (void *)0);
    glEnableVertexAttribArray(0);
    glBindVertexArray(0);
}

void Polygon_Shutdown(void) {
    glDeleteVertexArrays(1, &s_imm_vao);
    glDeleteBuffers(1, &s_imm_vbo);
}

void Polygon_DrawImmediate(const float *verts, int num_verts,
                            float r, float g, float b, float a) {
    if (num_verts < 3 || num_verts > 32) return;

    glBindVertexArray(s_imm_vao);
    glBindBuffer(GL_ARRAY_BUFFER, s_imm_vbo);
    glBufferSubData(GL_ARRAY_BUFFER, 0, sizeof(float) * 3 * num_verts, verts);

    GlBackend_SetVec4(g_flat_shader, "uColor", r, g, b, a);
    glDrawArrays(GL_TRIANGLE_FAN, 0, num_verts);

    glBindVertexArray(0);
}
