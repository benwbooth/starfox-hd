#include <stdio.h>
#include <stdlib.h>
#include <SDL2/SDL.h>
#include <glad/glad.h>

#include "types.h"
#include "config.h"
#include "sf_rtl.h"
#include "renderer/renderer.h"
#include "audio/audio.h"
#include "game/boot.h"

// Game tick rate: original SNES ran at ~20 FPS for game logic
#define GAME_TICK_MS 50  // 1000/20 = 50ms per game tick

static Config g_config;
static SDL_Window *g_window;
static SDL_GLContext g_gl_context;
static bool g_running = true;

static bool Init(void) {
    Config_Load(&g_config, "starfox.ini");

    if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_AUDIO | SDL_INIT_GAMECONTROLLER) < 0) {
        fprintf(stderr, "SDL_Init failed: %s\n", SDL_GetError());
        return false;
    }

    // Request OpenGL 3.3 Core
    SDL_GL_SetAttribute(SDL_GL_CONTEXT_MAJOR_VERSION, 3);
    SDL_GL_SetAttribute(SDL_GL_CONTEXT_MINOR_VERSION, 3);
    SDL_GL_SetAttribute(SDL_GL_CONTEXT_PROFILE_MASK, SDL_GL_CONTEXT_PROFILE_CORE);
    SDL_GL_SetAttribute(SDL_GL_DOUBLEBUFFER, 1);
    SDL_GL_SetAttribute(SDL_GL_DEPTH_SIZE, 24);

    if (g_config.msaa > 0) {
        SDL_GL_SetAttribute(SDL_GL_MULTISAMPLEBUFFERS, 1);
        SDL_GL_SetAttribute(SDL_GL_MULTISAMPLESAMPLES, g_config.msaa);
    }

    uint32 window_flags = SDL_WINDOW_OPENGL | SDL_WINDOW_RESIZABLE;
    if (g_config.fullscreen == 1) window_flags |= SDL_WINDOW_FULLSCREEN;
    else if (g_config.fullscreen == 2) window_flags |= SDL_WINDOW_FULLSCREEN_DESKTOP;

    g_window = SDL_CreateWindow(
        "Star Fox HD",
        SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED,
        g_config.window_width, g_config.window_height,
        window_flags
    );
    if (!g_window) {
        fprintf(stderr, "SDL_CreateWindow failed: %s\n", SDL_GetError());
        return false;
    }

    g_gl_context = SDL_GL_CreateContext(g_window);
    if (!g_gl_context) {
        fprintf(stderr, "SDL_GL_CreateContext failed: %s\n", SDL_GetError());
        return false;
    }

    if (!gladLoadGLLoader((GLADloadproc)SDL_GL_GetProcAddress)) {
        fprintf(stderr, "gladLoadGLLoader failed\n");
        return false;
    }

    printf("OpenGL %s, GLSL %s\n", glGetString(GL_VERSION), glGetString(GL_SHADING_LANGUAGE_VERSION));
    printf("Renderer: %s\n", glGetString(GL_RENDERER));

    // VSync
    if (g_config.target_fps == 0) {
        SDL_GL_SetSwapInterval(1);
    } else {
        SDL_GL_SetSwapInterval(0);
    }

    // Init subsystems
    Renderer_Init(g_config.window_width, g_config.window_height);
    Audio_Init(&g_config);
    SfRtl_Init();

    return true;
}

static void HandleEvents(void) {
    SDL_Event event;
    while (SDL_PollEvent(&event)) {
        switch (event.type) {
        case SDL_QUIT:
            g_running = false;
            break;
        case SDL_WINDOWEVENT:
            if (event.window.event == SDL_WINDOWEVENT_SIZE_CHANGED) {
                Renderer_Resize(event.window.data1, event.window.data2);
            }
            break;
        case SDL_KEYDOWN:
            if (event.key.keysym.sym == SDLK_ESCAPE) {
                g_running = false;
            }
            if (event.key.keysym.sym == SDLK_F11) {
                uint32 flags = SDL_GetWindowFlags(g_window);
                SDL_SetWindowFullscreen(g_window,
                    (flags & SDL_WINDOW_FULLSCREEN_DESKTOP) ? 0 : SDL_WINDOW_FULLSCREEN_DESKTOP);
            }
            break;
        }
        SfRtl_HandleEvent(&event);
    }
}

static void Shutdown(void) {
    Audio_Shutdown();
    Renderer_Shutdown();
    if (g_gl_context) SDL_GL_DeleteContext(g_gl_context);
    if (g_window) SDL_DestroyWindow(g_window);
    SDL_Quit();
}

int main(int argc, char *argv[]) {
    (void)argc; (void)argv;

    if (!Init()) {
        Shutdown();
        return 1;
    }

    // Fixed timestep game loop with interpolation
    uint64 prev_time = SDL_GetPerformanceCounter();
    uint64 freq = SDL_GetPerformanceFrequency();
    double accumulator = 0.0;
    double tick_duration = GAME_TICK_MS / 1000.0;

    // Game state for interpolation
    DrawListEntry prev_draw_list[MAX_DRAW_LIST];
    DrawListEntry curr_draw_list[MAX_DRAW_LIST];
    int prev_draw_count = 0;
    int curr_draw_count = 0;

    while (g_running) {
        uint64 now = SDL_GetPerformanceCounter();
        double dt = (double)(now - prev_time) / (double)freq;
        prev_time = now;

        // Cap dt to prevent spiral of death
        if (dt > 0.25) dt = 0.25;
        accumulator += dt;

        HandleEvents();

        // Fixed timestep game ticks
        while (accumulator >= tick_duration) {
            // Save previous state for interpolation
            memcpy(prev_draw_list, curr_draw_list, sizeof(DrawListEntry) * curr_draw_count);
            prev_draw_count = curr_draw_count;

            // Run one game tick
            SfRtl_BeginFrame();
            Game_Tick();
            curr_draw_count = Game_GetDrawList(curr_draw_list, MAX_DRAW_LIST);

            accumulator -= tick_duration;
        }

        // Interpolation alpha for smooth rendering
        float alpha = (float)(accumulator / tick_duration);

        // Render interpolated frame
        Renderer_BeginFrame();
        Renderer_SubmitDrawList(prev_draw_list, prev_draw_count,
                                curr_draw_list, curr_draw_count, alpha);
        Renderer_EndFrame();

        SDL_GL_SwapWindow(g_window);
    }

    Shutdown();
    return 0;
}
