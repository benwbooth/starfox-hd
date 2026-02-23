#include "hud.h"
#include "gl_backend.h"
#include <glad/glad.h>
#include <string.h>

static GLuint s_quad_vao;
static GLuint s_quad_vbo;

// HUD state
static int s_shield_cur, s_shield_max;
static int s_boost_cur, s_boost_max;
static int s_boss_hp_cur, s_boss_hp_max;
static int s_lives;
static int s_bombs;

void Hud_Init(void) {
    // Create a unit quad for drawing bars
    float quad[] = {
        0.0f, 0.0f,  0.0f, 0.0f,
        1.0f, 0.0f,  1.0f, 0.0f,
        1.0f, 1.0f,  1.0f, 1.0f,
        0.0f, 1.0f,  0.0f, 1.0f,
    };

    glGenVertexArrays(1, &s_quad_vao);
    glGenBuffers(1, &s_quad_vbo);
    glBindVertexArray(s_quad_vao);
    glBindBuffer(GL_ARRAY_BUFFER, s_quad_vbo);
    glBufferData(GL_ARRAY_BUFFER, sizeof(quad), quad, GL_STATIC_DRAW);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void *)0);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void *)(2 * sizeof(float)));
    glEnableVertexAttribArray(1);
    glBindVertexArray(0);

    s_shield_cur = s_shield_max = 100;
    s_boost_cur = s_boost_max = 100;
    s_boss_hp_cur = s_boss_hp_max = 0;
    s_lives = 3;
    s_bombs = 3;
}

void Hud_Shutdown(void) {
    glDeleteVertexArrays(1, &s_quad_vao);
    glDeleteBuffers(1, &s_quad_vbo);
}

// Build orthographic projection for HUD (0,0 = bottom-left)
static void BuildOrtho(float *m, float w, float h) {
    memset(m, 0, sizeof(float) * 16);
    m[0] = 2.0f / w;
    m[5] = 2.0f / h;
    m[10] = -1.0f;
    m[12] = -1.0f;
    m[13] = -1.0f;
    m[15] = 1.0f;
}

static void DrawBar(float x, float y, float w, float h,
                     float fill, float r, float g, float b) {
    // Background (dark)
    float model[16];
    memset(model, 0, sizeof(model));
    model[0] = w;
    model[5] = h;
    model[10] = 1.0f;
    model[12] = x;
    model[13] = y;
    model[15] = 1.0f;

    GlBackend_SetMat4(g_hud_shader, "uModel", model);
    GlBackend_SetVec4(g_hud_shader, "uColor", r * 0.3f, g * 0.3f, b * 0.3f, 0.8f);
    glBindVertexArray(s_quad_vao);
    glDrawArrays(GL_TRIANGLE_FAN, 0, 4);

    // Filled portion
    model[0] = w * fill;
    GlBackend_SetMat4(g_hud_shader, "uModel", model);
    GlBackend_SetVec4(g_hud_shader, "uColor", r, g, b, 0.9f);
    glDrawArrays(GL_TRIANGLE_FAN, 0, 4);

    glBindVertexArray(0);
}

void Hud_Render(int screen_width, int screen_height) {
    float sw = (float)screen_width;
    float sh = (float)screen_height;

    glDisable(GL_DEPTH_TEST);
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

    glUseProgram(g_hud_shader);

    float ortho[16];
    BuildOrtho(ortho, sw, sh);
    GlBackend_SetMat4(g_hud_shader, "uProj", ortho);

    // Shield bar (bottom-left)
    if (s_shield_max > 0) {
        float fill = (float)s_shield_cur / (float)s_shield_max;
        DrawBar(20, 20, 200, 16, fill, 0.0f, 0.8f, 0.2f);
    }

    // Boost bar (bottom-left, above shield)
    if (s_boost_max > 0) {
        float fill = (float)s_boost_cur / (float)s_boost_max;
        DrawBar(20, 44, 200, 12, fill, 0.9f, 0.6f, 0.0f);
    }

    // Boss HP bar (top-center)
    if (s_boss_hp_max > 0) {
        float fill = (float)s_boss_hp_cur / (float)s_boss_hp_max;
        DrawBar(sw / 2 - 150, sh - 36, 300, 16, fill, 0.8f, 0.0f, 0.0f);
    }

    glDisable(GL_BLEND);
    glEnable(GL_DEPTH_TEST);
}

void Hud_SetShield(int current, int max) {
    s_shield_cur = current;
    s_shield_max = max;
}

void Hud_SetBoost(int current, int max) {
    s_boost_cur = current;
    s_boost_max = max;
}

void Hud_SetBossHP(int current, int max) {
    s_boss_hp_cur = current;
    s_boss_hp_max = max;
}

void Hud_SetLives(int count) { s_lives = count; }
void Hud_SetBombs(int count) { s_bombs = count; }
