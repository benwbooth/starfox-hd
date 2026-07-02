#include <math.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>
#include <ctype.h>

#include <SDL2/SDL.h>
#include <glad/glad.h>

#include "renderer/gl_backend.h"
#include "renderer/transform.h"
#include "viewer/shape_catalog_asm.h"

#define MESH_VIEWER_WINDOW_WIDTH 1280
#define MESH_VIEWER_WINDOW_HEIGHT 720
#define MESH_VIEWER_PI 3.14159265358979323846f

typedef struct {
    SDL_Window *window;
    SDL_GLContext gl_context;
    bool running;
    bool gl_loaded;
    bool renderer_ready;
    bool catalog_ready;
    bool mouse_orbit_active;
    bool title_dirty;
    int drawable_width;
    int drawable_height;
    int selected_shape_index;
    float orbit_yaw;
    float orbit_pitch;
    float distance;
    float min_distance;
    float max_distance;
    float mesh_center[3];
    float mesh_radius;
    int anim_frame;
    int color_frame;
    const ViewerShapeInfo *selected_shape;
} MeshViewerApp;

typedef struct {
    int initial_shape_index;
    const char *initial_shape_label;
    const char *find_substring;
    ViewerPaletteKind initial_palette;
    int initial_shade_table;
    bool catalog_stats;
    bool catalog_audit;
    bool list_shapes;
} MeshViewerOptions;

static float ClampFloat(float value, float lo, float hi) {
    if (value < lo) return lo;
    if (value > hi) return hi;
    return value;
}

static float MaxFloat(float a, float b) {
    return (a > b) ? a : b;
}

static int WrapIndex(int value, int count) {
    if (count <= 0) return 0;
    value %= count;
    if (value < 0) value += count;
    return value;
}

static int StepWrappedFrame(int current, int delta, int frame_count) {
    int count = frame_count > 0 ? frame_count : 1;
    int value = (current + delta) % count;
    if (value < 0) value += count;
    return value;
}

static void MarkTitleDirty(MeshViewerApp *app) {
    app->title_dirty = true;
}

static void ClampOrbit(MeshViewerApp *app) {
    const float limit = 0.495f * MESH_VIEWER_PI;
    app->orbit_pitch = ClampFloat(app->orbit_pitch, -limit, limit);
    app->distance = ClampFloat(app->distance, app->min_distance, app->max_distance);
}

static void AdjustZoom(MeshViewerApp *app, float steps) {
    float factor = powf(0.88f, steps);
    app->distance *= factor;
    ClampOrbit(app);
    MarkTitleDirty(app);
}

static void BuildLookAt(float *out,
                        float eye_x, float eye_y, float eye_z,
                        float target_x, float target_y, float target_z) {
    float fx = target_x - eye_x;
    float fy = target_y - eye_y;
    float fz = target_z - eye_z;
    float flen = sqrtf(fx * fx + fy * fy + fz * fz);
    const float up_x = 0.0f;
    const float up_y = 1.0f;
    const float up_z = 0.0f;
    float sx;
    float sy;
    float sz;
    float slen;
    float ux;
    float uy;
    float uz;

    if (flen <= 0.0001f) {
        flen = 1.0f;
        fz = -1.0f;
        fx = 0.0f;
        fy = 0.0f;
    }

    fx /= flen;
    fy /= flen;
    fz /= flen;

    sx = fy * up_z - fz * up_y;
    sy = fz * up_x - fx * up_z;
    sz = fx * up_y - fy * up_x;
    slen = sqrtf(sx * sx + sy * sy + sz * sz);
    if (slen <= 0.0001f) {
        sx = 1.0f;
        sy = 0.0f;
        sz = 0.0f;
        slen = 1.0f;
    }

    sx /= slen;
    sy /= slen;
    sz /= slen;

    ux = sy * fz - sz * fy;
    uy = sz * fx - sx * fz;
    uz = sx * fy - sy * fx;

    Transform_Identity(out);
    out[0] = sx;
    out[1] = sy;
    out[2] = sz;
    out[4] = ux;
    out[5] = uy;
    out[6] = uz;
    out[8] = -fx;
    out[9] = -fy;
    out[10] = -fz;
    out[12] = -(sx * eye_x + sy * eye_y + sz * eye_z);
    out[13] = -(ux * eye_x + uy * eye_y + uz * eye_z);
    out[14] = fx * eye_x + fy * eye_y + fz * eye_z;
}

static void BuildOrbitViewMatrix(const MeshViewerApp *app, float *out_view) {
    float cos_pitch = cosf(app->orbit_pitch);
    float sin_pitch = sinf(app->orbit_pitch);
    float cos_yaw = cosf(app->orbit_yaw);
    float sin_yaw = sinf(app->orbit_yaw);
    float eye_x = sin_yaw * cos_pitch * app->distance;
    float eye_y = sin_pitch * app->distance;
    float eye_z = cos_yaw * cos_pitch * app->distance;

    BuildLookAt(out_view, eye_x, eye_y, eye_z, 0.0f, 0.0f, 0.0f);
}

static void BuildCenteredModelMatrix(const MeshViewerApp *app, float *out_model) {
    Transform_Identity(out_model);
    out_model[12] = -app->mesh_center[0];
    out_model[13] = -app->mesh_center[1];
    out_model[14] = -app->mesh_center[2];
}

static void ResetCamera(MeshViewerApp *app) {
    app->orbit_yaw = 0.65f;
    app->orbit_pitch = 0.25f;
    app->distance = ClampFloat(app->mesh_radius * 3.2f,
                               app->min_distance,
                               app->max_distance);
    MarkTitleDirty(app);
}

static void RefreshWindowTitle(MeshViewerApp *app) {
    char title[320];
    int entry_count = ViewerCatalog_GetShapeCount();
    int anim_count = ViewerCatalog_GetShapeFrameCount(app->selected_shape_index);
    int color_count = ViewerCatalog_GetShapeColorFrameCount(app->selected_shape_index);
    const char *label = (app->selected_shape && app->selected_shape->label)
        ? app->selected_shape->label
        : "unnamed";
    const char *display_name = (app->selected_shape && app->selected_shape->display_name)
        ? app->selected_shape->display_name
        : "";

    snprintf(title, sizeof(title),
             "Star Fox Mesh Viewer | %d/%d | %s%s%s | anim %d/%d | color %d/%d | palette %s | shade %d | zoom %.1f",
             app->selected_shape_index + 1,
             entry_count > 0 ? entry_count : 1,
             label,
             *display_name ? " / " : "",
             display_name,
             app->anim_frame,
             anim_count > 0 ? anim_count - 1 : 0,
             app->color_frame,
             color_count > 0 ? color_count - 1 : 0,
             ViewerCatalog_GetPaletteName(ViewerCatalog_GetPalette()),
             ViewerCatalog_GetShadeTable(),
             app->distance);
    SDL_SetWindowTitle(app->window, title);
    app->title_dirty = false;
}

static void UpdateDrawableSize(MeshViewerApp *app) {
    SDL_GL_GetDrawableSize(app->window, &app->drawable_width, &app->drawable_height);
    if (app->drawable_width < 1) app->drawable_width = 1;
    if (app->drawable_height < 1) app->drawable_height = 1;

    glViewport(0, 0, app->drawable_width, app->drawable_height);
    Transform_SetProjection(app->drawable_width, app->drawable_height);
}

static bool SelectCatalogShape(MeshViewerApp *app, int index) {
    int entry_count = ViewerCatalog_GetShapeCount();

    if (entry_count <= 0) {
        fprintf(stderr, "Mesh viewer: no shapes available in catalog\n");
        return false;
    }

    app->selected_shape_index = WrapIndex(index, entry_count);
    app->selected_shape = ViewerCatalog_GetShapeInfo(app->selected_shape_index);
    if (!app->selected_shape) {
        fprintf(stderr, "Mesh viewer: failed to query shape catalog entry %d\n",
                app->selected_shape_index);
        return false;
    }

    ViewerCatalog_GetShapeBounds(app->selected_shape_index,
                                 app->mesh_center,
                                 &app->mesh_radius);

    app->min_distance = MaxFloat(app->mesh_radius * 1.5f, 6.0f);
    app->max_distance = MaxFloat(app->mesh_radius * 32.0f, 80.0f);
    app->anim_frame = 0;
    app->color_frame = 0;
    ResetCamera(app);
    return true;
}

static bool InitMeshViewer(MeshViewerApp *app, const MeshViewerOptions *options) {
    memset(app, 0, sizeof(*app));

    if (SDL_Init(SDL_INIT_VIDEO) < 0) {
        fprintf(stderr, "SDL_Init failed: %s\n", SDL_GetError());
        return false;
    }

    SDL_GL_SetAttribute(SDL_GL_CONTEXT_MAJOR_VERSION, 3);
    SDL_GL_SetAttribute(SDL_GL_CONTEXT_MINOR_VERSION, 3);
    SDL_GL_SetAttribute(SDL_GL_CONTEXT_PROFILE_MASK, SDL_GL_CONTEXT_PROFILE_CORE);
    SDL_GL_SetAttribute(SDL_GL_DOUBLEBUFFER, 1);
    SDL_GL_SetAttribute(SDL_GL_DEPTH_SIZE, 24);
    SDL_GL_SetAttribute(SDL_GL_MULTISAMPLEBUFFERS, 1);
    SDL_GL_SetAttribute(SDL_GL_MULTISAMPLESAMPLES, 4);

    app->window = SDL_CreateWindow("Star Fox Mesh Viewer",
                                   SDL_WINDOWPOS_CENTERED,
                                   SDL_WINDOWPOS_CENTERED,
                                   MESH_VIEWER_WINDOW_WIDTH,
                                   MESH_VIEWER_WINDOW_HEIGHT,
                                   SDL_WINDOW_OPENGL |
                                   SDL_WINDOW_RESIZABLE |
                                   SDL_WINDOW_ALLOW_HIGHDPI);
    if (!app->window) {
        fprintf(stderr, "SDL_CreateWindow failed: %s\n", SDL_GetError());
        return false;
    }

    app->gl_context = SDL_GL_CreateContext(app->window);
    if (!app->gl_context) {
        fprintf(stderr, "SDL_GL_CreateContext failed: %s\n", SDL_GetError());
        return false;
    }

    if (!gladLoadGLLoader((GLADloadproc)SDL_GL_GetProcAddress)) {
        fprintf(stderr, "gladLoadGLLoader failed\n");
        return false;
    }
    app->gl_loaded = true;

    SDL_GL_SetSwapInterval(1);

    printf("OpenGL %s, GLSL %s\n",
           glGetString(GL_VERSION),
           glGetString(GL_SHADING_LANGUAGE_VERSION));
    printf("Renderer: %s\n", glGetString(GL_RENDERER));

    GlBackend_Init();
    Transform_Init();
    app->renderer_ready = true;

    if (!g_flat_shader) {
        fprintf(stderr, "Mesh viewer: flat shader initialization failed\n");
        return false;
    }

    if (!ViewerCatalog_LoadFromAsm()) {
        fprintf(stderr, "Mesh viewer: shape catalog initialization failed\n");
        return false;
    }
    app->catalog_ready = true;

    ViewerCatalog_SetPalette(options->initial_palette);
    ViewerCatalog_SetShadeTable(options->initial_shade_table);

    glEnable(GL_DEPTH_TEST);
    glDepthFunc(GL_LESS);
    glDisable(GL_CULL_FACE);
    glClearColor(0.02f, 0.02f, 0.06f, 1.0f);

    UpdateDrawableSize(app);

    if (options->initial_shape_label) {
        int shape_index = ViewerCatalog_FindShapeByLabel(options->initial_shape_label);
        if (shape_index < 0) {
            fprintf(stderr, "Mesh viewer: unknown shape label '%s'\n", options->initial_shape_label);
            return false;
        }
        if (!SelectCatalogShape(app, shape_index)) {
            return false;
        }
    } else if (!SelectCatalogShape(app, options->initial_shape_index)) {
        return false;
    }

    app->running = true;
    app->title_dirty = true;

    printf("Controls:\n");
    printf("  ESC quit\n");
    printf("  Left/Right select shape\n");
    printf("  Up/Down step animation frame\n");
    printf("  [ and ] step color frame\n");
    printf("  P cycle palette (night/red/blue)\n");
    printf("  L cycle shade table (0..3)\n");
    printf("  Left mouse drag or W/A/S/D orbit camera\n");
    printf("  Mouse wheel, +/- or Q/E zoom\n");
    printf("  R reset camera\n");

    return true;
}

static void ShutdownMeshViewer(MeshViewerApp *app) {
    if (app->catalog_ready) {
        ViewerCatalog_Unload();
        app->catalog_ready = false;
    }
    if (app->renderer_ready && app->gl_loaded) {
        GlBackend_Shutdown();
        app->renderer_ready = false;
    }

    if (app->gl_context) SDL_GL_DeleteContext(app->gl_context);
    if (app->window) SDL_DestroyWindow(app->window);
    app->gl_context = NULL;
    app->window = NULL;
    SDL_Quit();
}

static void HandleKeyDown(MeshViewerApp *app, SDL_Keycode key) {
    switch (key) {
    case SDLK_ESCAPE:
        app->running = false;
        break;
    case SDLK_LEFT:
        SelectCatalogShape(app, app->selected_shape_index - 1);
        break;
    case SDLK_RIGHT:
        SelectCatalogShape(app, app->selected_shape_index + 1);
        break;
    case SDLK_UP:
        app->anim_frame = StepWrappedFrame(app->anim_frame, 1,
                                           ViewerCatalog_GetShapeFrameCount(app->selected_shape_index));
        MarkTitleDirty(app);
        break;
    case SDLK_DOWN:
        app->anim_frame = StepWrappedFrame(app->anim_frame, -1,
                                           ViewerCatalog_GetShapeFrameCount(app->selected_shape_index));
        MarkTitleDirty(app);
        break;
    case SDLK_LEFTBRACKET:
        app->color_frame = StepWrappedFrame(app->color_frame, -1,
                                            ViewerCatalog_GetShapeColorFrameCount(app->selected_shape_index));
        MarkTitleDirty(app);
        break;
    case SDLK_RIGHTBRACKET:
        app->color_frame = StepWrappedFrame(app->color_frame, 1,
                                            ViewerCatalog_GetShapeColorFrameCount(app->selected_shape_index));
        MarkTitleDirty(app);
        break;
    case SDLK_p:
        ViewerCatalog_NextPalette(1);
        MarkTitleDirty(app);
        break;
    case SDLK_l:
        ViewerCatalog_NextShadeTable(1);
        MarkTitleDirty(app);
        break;
    case SDLK_r:
    case SDLK_HOME:
        ResetCamera(app);
        break;
    default:
        break;
    }
}

static void HandleEvents(MeshViewerApp *app) {
    SDL_Event event;

    while (SDL_PollEvent(&event)) {
        switch (event.type) {
        case SDL_QUIT:
            app->running = false;
            break;
        case SDL_WINDOWEVENT:
            if (event.window.event == SDL_WINDOWEVENT_SIZE_CHANGED) {
                UpdateDrawableSize(app);
            }
            break;
        case SDL_KEYDOWN:
            if (event.key.repeat == 0) {
                HandleKeyDown(app, event.key.keysym.sym);
            }
            break;
        case SDL_MOUSEWHEEL: {
            float wheel_delta = event.wheel.preciseY != 0.0f
                ? event.wheel.preciseY
                : (float)event.wheel.y;
            if (event.wheel.direction == SDL_MOUSEWHEEL_FLIPPED) {
                wheel_delta = -wheel_delta;
            }
            if (wheel_delta != 0.0f) {
                AdjustZoom(app, wheel_delta);
            }
            break;
        }
        case SDL_MOUSEBUTTONDOWN:
            if (event.button.button == SDL_BUTTON_LEFT) {
                app->mouse_orbit_active = true;
                SDL_CaptureMouse(SDL_TRUE);
            }
            break;
        case SDL_MOUSEBUTTONUP:
            if (event.button.button == SDL_BUTTON_LEFT) {
                app->mouse_orbit_active = false;
                SDL_CaptureMouse(SDL_FALSE);
            }
            break;
        case SDL_MOUSEMOTION:
            if (app->mouse_orbit_active) {
                app->orbit_yaw += event.motion.xrel * 0.01f;
                app->orbit_pitch -= event.motion.yrel * 0.01f;
                ClampOrbit(app);
                MarkTitleDirty(app);
            }
            break;
        default:
            break;
        }
    }
}

static void UpdateCameraFromKeyboard(MeshViewerApp *app, float dt) {
    const uint8 *keys = SDL_GetKeyboardState(NULL);
    float orbit_step = 1.8f * dt;
    bool changed = false;

    if (keys[SDL_SCANCODE_A]) {
        app->orbit_yaw -= orbit_step;
        changed = true;
    }
    if (keys[SDL_SCANCODE_D]) {
        app->orbit_yaw += orbit_step;
        changed = true;
    }
    if (keys[SDL_SCANCODE_W]) {
        app->orbit_pitch += orbit_step;
        changed = true;
    }
    if (keys[SDL_SCANCODE_S]) {
        app->orbit_pitch -= orbit_step;
        changed = true;
    }
    if (keys[SDL_SCANCODE_EQUALS] || keys[SDL_SCANCODE_KP_PLUS] ||
        keys[SDL_SCANCODE_Q]) {
        AdjustZoom(app, 4.0f * dt);
        changed = true;
    }
    if (keys[SDL_SCANCODE_MINUS] || keys[SDL_SCANCODE_KP_MINUS] ||
        keys[SDL_SCANCODE_E]) {
        AdjustZoom(app, -4.0f * dt);
        changed = true;
    }

    if (changed) {
        ClampOrbit(app);
        MarkTitleDirty(app);
    }
}

static void RenderMeshViewer(const MeshViewerApp *app) {
    float view[16];
    float model[16];
    const float *proj;

    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

    BuildOrbitViewMatrix(app, view);
    BuildCenteredModelMatrix(app, model);
    proj = Transform_GetProjection();
    ViewerCatalog_RenderShape(app->selected_shape_index,
                              app->anim_frame,
                              app->color_frame,
                              model,
                              view,
                              proj);

    glUseProgram(0);
    SDL_GL_SwapWindow(app->window);
}

static bool ContainsCi(const char *haystack, const char *needle);

static int RunCatalogStats(const MeshViewerOptions *options) {
    int i;
    int start_index = 0;
    int end_index;
    bool matched_any = false;
    int header_count;
    int skipped_count;
    double coverage;
    if (!ViewerCatalog_LoadFromAsm()) {
        fprintf(stderr, "Mesh viewer: failed to load ASM shape catalog\n");
        return 1;
    }
    end_index = ViewerCatalog_GetShapeCount();
    header_count = ViewerCatalog_GetShapeHeaderCount();
    skipped_count = ViewerCatalog_GetSkippedShapeCount();
    coverage = (header_count > 0)
        ? (100.0 * (double)ViewerCatalog_GetShapeCount() / (double)header_count)
        : 0.0;

    printf("shapes=%d headers=%d skipped=%d coverage=%.1f%% palette=%s shade=%d\n",
           ViewerCatalog_GetShapeCount(),
           header_count,
           skipped_count,
           coverage,
           ViewerCatalog_GetPaletteName(ViewerCatalog_GetPalette()),
           ViewerCatalog_GetShadeTable());
    if (options->initial_shape_label) {
        start_index = ViewerCatalog_FindShapeByLabel(options->initial_shape_label);
        if (start_index < 0) {
            fprintf(stderr, "Mesh viewer: unknown shape label '%s'\n", options->initial_shape_label);
            ViewerCatalog_Unload();
            return 1;
        }
        end_index = start_index + 1;
    } else if (options->initial_shape_index > 0) {
        start_index = options->initial_shape_index;
        if (start_index < 0) start_index = 0;
        if (start_index >= ViewerCatalog_GetShapeCount()) {
            start_index = ViewerCatalog_GetShapeCount() - 1;
        }
        end_index = start_index + 1;
    }

    if (options->list_shapes || options->find_substring) {
        for (i = start_index; i < end_index; ++i) {
            const ViewerShapeInfo *shape = ViewerCatalog_GetShapeInfo(i);
            if (!shape) {
                continue;
            }
            if (options->find_substring &&
                !ContainsCi(shape->label, options->find_substring) &&
                !ContainsCi(shape->display_name, options->find_substring)) {
                continue;
            }
            matched_any = true;
            printf("%4d %-24s frames=%d colors=%d verts=%d polys=%d lines=%d\n",
                   i,
                   shape->label ? shape->label : "unnamed",
                   ViewerCatalog_GetShapeFrameCount(i),
                   ViewerCatalog_GetShapeColorFrameCount(i),
                   shape->vertex_count,
                   shape->poly_face_count,
                   shape->line_face_count);
        }
        if (options->find_substring && !matched_any) {
            fprintf(stderr, "Mesh viewer: no shapes matched '%s'\n", options->find_substring);
            ViewerCatalog_Unload();
            return 1;
        }
    }

    if (options->catalog_audit) {
        for (i = 0; i < skipped_count; ++i) {
            const ViewerSkippedShapeInfo *info = ViewerCatalog_GetSkippedShapeInfo(i);
            if (!info) {
                continue;
            }
            printf("skip %s:%d reason=%s text=%s\n",
                   info->file_path ? info->file_path : "",
                   info->line_number,
                   info->reason ? info->reason : "unknown",
                   info->line_text ? info->line_text : "");
        }
    }

    ViewerCatalog_Unload();
    return 0;
}

static bool ParsePaletteArg(const char *value, ViewerPaletteKind *out_palette) {
    if (strcasecmp(value, "night") == 0 || strcasecmp(value, "norm") == 0) {
        *out_palette = VIEWER_PALETTE_NORM;
        return true;
    }
    if (strcasecmp(value, "red") == 0) {
        *out_palette = VIEWER_PALETTE_RED;
        return true;
    }
    if (strcasecmp(value, "blue") == 0) {
        *out_palette = VIEWER_PALETTE_BLUE;
        return true;
    }
    return false;
}

static bool ContainsCi(const char *haystack, const char *needle) {
    size_t needle_len;
    size_t i;
    if (!haystack || !needle) {
        return false;
    }
    needle_len = strlen(needle);
    if (needle_len == 0) {
        return true;
    }
    for (i = 0; haystack[i] != '\0'; ++i) {
        size_t j = 0;
        while (needle[j] != '\0' &&
               haystack[i + j] != '\0' &&
               tolower((unsigned char)haystack[i + j]) == tolower((unsigned char)needle[j])) {
            ++j;
        }
        if (j == needle_len) {
            return true;
        }
    }
    return false;
}

int main(int argc, char **argv) {
    MeshViewerApp app;
    MeshViewerOptions options;
    uint64_t last_counter = 0;
    int i;

    memset(&options, 0, sizeof(options));
    options.initial_palette = VIEWER_PALETTE_NORM;
    options.initial_shade_table = 0;

    for (i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--catalog-stats") == 0) {
            options.catalog_stats = true;
        } else if (strcmp(argv[i], "--catalog-audit") == 0) {
            options.catalog_stats = true;
            options.catalog_audit = true;
        } else if (strcmp(argv[i], "--list-shapes") == 0) {
            options.catalog_stats = true;
            options.list_shapes = true;
        } else if (strcmp(argv[i], "--shape") == 0 && i + 1 < argc) {
            options.initial_shape_label = argv[++i];
        } else if (strcmp(argv[i], "--find") == 0 && i + 1 < argc) {
            options.catalog_stats = true;
            options.list_shapes = true;
            options.find_substring = argv[++i];
        } else if (strcmp(argv[i], "--index") == 0 && i + 1 < argc) {
            options.initial_shape_index = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--palette") == 0 && i + 1 < argc) {
            if (!ParsePaletteArg(argv[++i], &options.initial_palette)) {
                fprintf(stderr, "Unknown palette: %s\n", argv[i]);
                return 1;
            }
        } else if (strcmp(argv[i], "--shade") == 0 && i + 1 < argc) {
            options.initial_shade_table = atoi(argv[++i]);
            if (options.initial_shade_table < 0 || options.initial_shade_table > 3) {
                fprintf(stderr, "Shade table must be 0..3\n");
                return 1;
            }
        } else if (strcmp(argv[i], "--help") == 0) {
            printf("Usage: %s [--catalog-stats] [--catalog-audit] [--list-shapes] [--find TEXT] [--shape LABEL] [--index N] [--palette night|red|blue] [--shade 0..3]\n",
                   argv[0]);
            return 0;
        } else {
            fprintf(stderr, "Unknown argument: %s\n", argv[i]);
            return 1;
        }
    }

    if (options.catalog_stats) {
        ViewerCatalog_SetPalette(options.initial_palette);
        ViewerCatalog_SetShadeTable(options.initial_shade_table);
        return RunCatalogStats(&options);
    }

    if (!InitMeshViewer(&app, &options)) {
        ShutdownMeshViewer(&app);
        return 1;
    }

    last_counter = SDL_GetPerformanceCounter();
    while (app.running) {
        uint64_t now = SDL_GetPerformanceCounter();
        float dt = (float)(now - last_counter) / (float)SDL_GetPerformanceFrequency();
        last_counter = now;

        HandleEvents(&app);
        UpdateCameraFromKeyboard(&app, dt);
        if (app.title_dirty) {
            RefreshWindowTitle(&app);
        }
        RenderMeshViewer(&app);
    }

    ShutdownMeshViewer(&app);
    return 0;
}
