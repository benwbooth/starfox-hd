#include "../types.h"
#include "gl_backend.h"
#include "transform.h"
#include <glad/glad.h>
#include <string.h>

typedef struct {
    float x, y, z;
    float vx, vy, vz;
    float life;
    float max_life;
    uint8 type;
    bool active;
} Particle;

#define MAX_PARTICLES 256
static Particle s_particles[MAX_PARTICLES];

/* GL resources for a small quad used to draw each particle */
static GLuint s_quad_vao;
static GLuint s_quad_vbo;

/* Build a translation + uniform-scale model matrix (column-major 4x4) */
static void BuildParticleModel(float *m, float x, float y, float z, float scale) {
    /*  scale  0      0      0
        0      scale  0      0
        0      0      scale  0
        x      y      z      1   */
    memset(m, 0, 16 * sizeof(float));
    m[0]  = scale;
    m[5]  = scale;
    m[10] = scale;
    m[12] = x;
    m[13] = y;
    m[14] = z;
    m[15] = 1.0f;
}

void Particles_Init(void) {
    memset(s_particles, 0, sizeof(s_particles));

    /* Unit quad centred at origin, in XY plane.
       Vertices ordered for GL_TRIANGLE_FAN. */
    static const float quad_verts[] = {
        -0.5f, -0.5f, 0.0f,
         0.5f, -0.5f, 0.0f,
         0.5f,  0.5f, 0.0f,
        -0.5f,  0.5f, 0.0f,
    };

    glGenVertexArrays(1, &s_quad_vao);
    glGenBuffers(1, &s_quad_vbo);

    glBindVertexArray(s_quad_vao);
    glBindBuffer(GL_ARRAY_BUFFER, s_quad_vbo);
    glBufferData(GL_ARRAY_BUFFER, sizeof(quad_verts), quad_verts, GL_STATIC_DRAW);

    /* aPos — location 0 in g_flat_shader */
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, 3 * sizeof(float), (void *)0);

    glBindVertexArray(0);
}

void Particles_Shutdown(void) {
    memset(s_particles, 0, sizeof(s_particles));

    if (s_quad_vbo) { glDeleteBuffers(1, &s_quad_vbo); s_quad_vbo = 0; }
    if (s_quad_vao) { glDeleteVertexArrays(1, &s_quad_vao); s_quad_vao = 0; }
}

void Particles_Spawn(float x, float y, float z,
                     float vx, float vy, float vz,
                     float life, uint8 type) {
    for (int i = 0; i < MAX_PARTICLES; i++) {
        if (!s_particles[i].active) {
            s_particles[i] = (Particle){
                .x = x, .y = y, .z = z,
                .vx = vx, .vy = vy, .vz = vz,
                .life = life, .max_life = life,
                .type = type, .active = true
            };
            return;
        }
    }
}

void Particles_Update(float dt) {
    if (dt <= 0.0f) return;
    for (int i = 0; i < MAX_PARTICLES; i++) {
        Particle *p = &s_particles[i];
        if (!p->active) continue;
        p->life -= dt;
        if (p->life <= 0.0f) {
            p->active = false;
            continue;
        }
        p->x += p->vx * dt;
        p->y += p->vy * dt;
        p->z += p->vz * dt;
    }
}

void Particles_Render(void) {
    if (!g_flat_shader) return;

    /* Count active particles first to avoid GPU state changes when idle */
    int any_active = 0;
    for (int i = 0; i < MAX_PARTICLES; i++) {
        if (s_particles[i].active) { any_active = 1; break; }
    }
    if (!any_active) return;

    const float *view = Transform_GetView();
    const float *proj = Transform_GetProjection();

    glUseProgram(g_flat_shader);
    GlBackend_SetMat4(g_flat_shader, "uView", view);
    GlBackend_SetMat4(g_flat_shader, "uProj", proj);

    /* Particles are additive-blended, depth-tested but not depth-written */
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE);
    glDepthMask(GL_FALSE);

    glBindVertexArray(s_quad_vao);

    for (int i = 0; i < MAX_PARTICLES; i++) {
        const Particle *p = &s_particles[i];
        if (!p->active) continue;

        float alpha = (p->max_life > 0.0f) ? (p->life / p->max_life) : 0.0f;
        if (alpha <= 0.0f) continue;

        /* Colour by type */
        float r, g, b;
        switch (p->type) {
        default: /* fall through */
        case 0: r = 1.0f; g = 0.4f; b = 0.1f; break;  /* fire (orange/red) */
        case 1: r = 1.0f; g = 1.0f; b = 0.2f; break;  /* spark (yellow)    */
        case 2: r = 1.0f; g = 1.0f; b = 1.0f; break;  /* flash (white)     */
        case 3: r = 0.3f; g = 0.5f; b = 1.0f; break;  /* energy (blue)     */
        }

        float model[16];
        BuildParticleModel(model, p->x, p->y, p->z, 2.0f);

        GlBackend_SetMat4(g_flat_shader, "uModel", model);
        GlBackend_SetVec4(g_flat_shader, "uColor", r, g, b, alpha);

        glDrawArrays(GL_TRIANGLE_FAN, 0, 4);
    }

    glBindVertexArray(0);

    /* Restore state */
    glDepthMask(GL_TRUE);
    glDisable(GL_BLEND);
}
