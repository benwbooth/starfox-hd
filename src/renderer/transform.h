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

// Set camera from game state
void Transform_SetCamera(int32 cx, int32 cy, int32 cz,
                          int16 crx, int16 cry, int16 crz);

// Matrix helpers
void Transform_Identity(float *m);
void Transform_Multiply(float *out, const float *a, const float *b);

#endif // STARFOX_TRANSFORM_H
