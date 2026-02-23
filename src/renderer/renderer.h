#ifndef STARFOX_RENDERER_H
#define STARFOX_RENDERER_H

#include "../types.h"

// Initialize the OpenGL renderer
void Renderer_Init(int width, int height);

// Handle window resize
void Renderer_Resize(int width, int height);

// Frame lifecycle
void Renderer_BeginFrame(void);

// Submit interpolated draw list for rendering
void Renderer_SubmitDrawList(const DrawListEntry *prev, int prev_count,
                             const DrawListEntry *curr, int curr_count,
                             float alpha);

void Renderer_EndFrame(void);

// Cleanup
void Renderer_Shutdown(void);

#endif // STARFOX_RENDERER_H
