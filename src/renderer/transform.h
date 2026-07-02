#ifndef STARFOX_TRANSFORM_H
#define STARFOX_TRANSFORM_H

#include "../types.h"

void Transform_Init(void);

// Set projection matrix from window dimensions
void Transform_SetProjection(int width, int height);

// Build a 4x4 model matrix from SNES-style position and rotation
// pos: x,y,z in fixed 16.16; rot: rx,ry,rz in SNES angle units
void Transform_BuildModelMatrix(float *out, int32 x, int32 y, int32 z,
                                 int16 rx, int16 ry, int16 rz);

// Interpolate between two model matrices
void Transform_Lerp(float *out, const float *a, const float *b, float alpha);

// Get current projection matrix (column-major 4x4)
const float *Transform_GetProjection(void);

// Get current view matrix (column-major 4x4)
const float *Transform_GetView(void);

// Set camera from game state (once per game tick; SNES-convention inputs)
void Transform_SetCamera(int32 cx, int32 cy, int32 cz,
                          int16 crx, int16 cry, int16 crz);

// Rebuild the view matrix from the interpolated camera state
// (lerp between the previous and current tick's Transform_SetCamera inputs).
// Called once per RENDER frame with the fixed-timestep alpha.
void Transform_SetViewLerp(float alpha);

// Collapse camera interpolation history after a cut (viewtype change,
// fixed-position jump, level load) so the cut stays instant.
void Transform_SnapCamera(void);

// Camera state used for the most recent view-matrix build (render-frame
// interpolated, SNES conventions: position fixed 16.16 with +Y down,
// rotation in SNES 0-255 angle units). Any output pointer may be NULL.
// Consumers that must track the 3D view exactly (e.g. the 2D background
// horizon) read this instead of the per-tick game camera.
void Transform_GetRenderCamera(int32 *x, int32 *y, int32 *z,
                               int16 *rx, int16 *ry, int16 *rz);

// Matrix helpers
void Transform_Identity(float *m);
void Transform_Multiply(float *out, const float *a, const float *b);

#endif // STARFOX_TRANSFORM_H
