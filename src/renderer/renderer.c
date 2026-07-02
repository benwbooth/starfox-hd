#include "renderer.h"
#include "gl_backend.h"
#include "draw_list.h"
#include "transform.h"
#include "shapes.h"
#include "hud.h"
#include "particles.h"
#include "font.h"
#include "sprites.h"
#include "bg2d.h"
#include "ui.h"
#include "light_data.h"
#include <glad/glad.h>
#include <stdio.h>
#include <stdlib.h>

static int s_width, s_height;

void Renderer_Init(int width, int height) {
    s_width = width;
    s_height = height;

    GlBackend_Init();
    DrawList_Init();
    Transform_Init();
    Hud_Init();
    Particles_Init();
    Font_Init();
    Sprites_Init();
    Bg2d_Init();
    Ui_Init();

    // Set initial projection (also done in Renderer_Resize)
    Transform_SetProjection(width, height);

    // Register built-in shapes (Arwing, etc.)
    Shapes_RegisterBuiltins();

    glEnable(GL_DEPTH_TEST);
    glDepthFunc(GL_LESS);
    // Disable backface culling — Star Fox shape winding is not guaranteed CCW
    glDisable(GL_CULL_FACE);

    glViewport(0, 0, width, height);
    glClearColor(0.0f, 0.0f, 0.05f, 1.0f);  // Deep space blue-black

    printf("Renderer initialized (%dx%d)\n", width, height);
}

void Renderer_Resize(int width, int height) {
    s_width = width;
    s_height = height;
    glViewport(0, 0, width, height);
    Transform_SetProjection(width, height);
    Font_SetScreenSize(width, height);
}

void Renderer_BeginFrame(void) {
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
}

void Renderer_SubmitDrawList(const DrawListEntry *prev, int prev_count,
                             const DrawListEntry *curr, int curr_count,
                             float alpha) {
    // Rebuild the interpolated view matrix first: the BG layer derives the
    // painted-horizon scroll from the render-frame camera, so it must see
    // the same camera the 3D pass uses (DrawList_Render's own
    // Transform_SetViewLerp call with the same alpha is then a no-op).
    Transform_SetViewLerp(alpha);

    // Pass order: BG layer -> 3D -> particles -> HUD -> state UI -> fade
    Bg2d_Render(s_width, s_height);
    DrawList_Render(prev, prev_count, curr, curr_count, alpha);
    Particles_Render();
    Hud_Render(s_width, s_height);
    Ui_Render(s_width, s_height);
    Ui_RenderFade(s_width, s_height);
}

// Debug frame dump: set SF_DUMP_PPM=<path> to write the framebuffer to a
// PPM file every 30 rendered frames (overwrites the same file).
static void MaybeDumpFrame(void) {
    static const char *s_dump_path = NULL;
    static int s_dump_checked = 0;
    static unsigned s_frame = 0;

    if (!s_dump_checked) {
        s_dump_checked = 1;
        s_dump_path = getenv("SF_DUMP_PPM");
    }
    if (!s_dump_path) return;

    unsigned frame = s_frame++;
    if ((frame % 30) != 0) return;

    unsigned char *pixels = malloc((size_t)s_width * s_height * 3);
    if (!pixels) return;
    glPixelStorei(GL_PACK_ALIGNMENT, 1);
    glReadPixels(0, 0, s_width, s_height, GL_RGB, GL_UNSIGNED_BYTE, pixels);

    char path_buf[512];
    const char *out_path = s_dump_path;
    if (getenv("SF_DUMP_SEQ")) {
        snprintf(path_buf, sizeof(path_buf), "%s.%04u.ppm", s_dump_path, frame / 30);
        out_path = path_buf;
    }
    FILE *f = fopen(out_path, "wb");
    if (f) {
        fprintf(f, "P6\n%d %d\n255\n", s_width, s_height);
        // Flip vertically (GL rows are bottom-up)
        for (int y = s_height - 1; y >= 0; y--) {
            fwrite(pixels + (size_t)y * s_width * 3, 1, (size_t)s_width * 3, f);
        }
        fclose(f);
    }
    free(pixels);
}

void Renderer_EndFrame(void) {
    // Post-processing would go here (CRT filter, bloom, etc.)
    MaybeDumpFrame();
}

void Renderer_Shutdown(void) {
    Ui_Shutdown();
    Bg2d_Shutdown();
    Sprites_Shutdown();
    Font_Shutdown();
    Particles_Shutdown();
    Hud_Shutdown();
    DrawList_Shutdown();
    GlBackend_Shutdown();
    printf("Renderer shut down\n");
}
