#include "renderer.h"
#include "gl_backend.h"
#include "draw_list.h"
#include "transform.h"
#include "hud.h"
#include <glad/glad.h>
#include <stdio.h>

static int s_width, s_height;

void Renderer_Init(int width, int height) {
    s_width = width;
    s_height = height;

    GlBackend_Init();
    DrawList_Init();
    Transform_Init();
    Hud_Init();

    glEnable(GL_DEPTH_TEST);
    glDepthFunc(GL_LESS);
    glEnable(GL_CULL_FACE);
    glCullFace(GL_BACK);

    glViewport(0, 0, width, height);
    glClearColor(0.0f, 0.0f, 0.05f, 1.0f);  // Deep space blue-black

    printf("Renderer initialized (%dx%d)\n", width, height);
}

void Renderer_Resize(int width, int height) {
    s_width = width;
    s_height = height;
    glViewport(0, 0, width, height);
    Transform_SetProjection(width, height);
}

void Renderer_BeginFrame(void) {
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
}

void Renderer_SubmitDrawList(const DrawListEntry *prev, int prev_count,
                             const DrawListEntry *curr, int curr_count,
                             float alpha) {
    DrawList_Render(prev, prev_count, curr, curr_count, alpha);
    Hud_Render(s_width, s_height);
}

void Renderer_EndFrame(void) {
    // Post-processing would go here (CRT filter, bloom, etc.)
}

void Renderer_Shutdown(void) {
    Hud_Shutdown();
    DrawList_Shutdown();
    GlBackend_Shutdown();
    printf("Renderer shut down\n");
}
