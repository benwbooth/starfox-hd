#include "transform.h"
#include <math.h>
#include <string.h>

static float s_projection[16];
static float s_view[16];

// SNES sin/cos table (256 entries, matching the 0-255 angle system)
// Pre-computed for exact SNES behavior
static float s_sin_table[256];
static float s_cos_table[256];

void Transform_Identity(float *m) {
    memset(m, 0, sizeof(float) * 16);
    m[0] = m[5] = m[10] = m[15] = 1.0f;
}

void Transform_Multiply(float *out, const float *a, const float *b) {
    float tmp[16];
    for (int i = 0; i < 4; i++) {
        for (int j = 0; j < 4; j++) {
            tmp[j * 4 + i] = 0;
            for (int k = 0; k < 4; k++) {
                tmp[j * 4 + i] += a[k * 4 + i] * b[j * 4 + k];
            }
        }
    }
    memcpy(out, tmp, sizeof(float) * 16);
}

void Transform_Init(void) {
    // Build sin/cos lookup table for SNES 256-degree angle system
    for (int i = 0; i < 256; i++) {
        float rad = (float)i * (2.0f * 3.14159265f / 256.0f);
        s_sin_table[i] = sinf(rad);
        s_cos_table[i] = cosf(rad);
    }

    Transform_Identity(s_projection);
    Transform_Identity(s_view);
}

void Transform_SetProjection(int width, int height) {
    // Perspective projection
    float aspect = (float)width / (float)height;
    float fov = 60.0f * (3.14159265f / 180.0f);  // 60 degree FOV
    float near = 1.0f;
    float far = 10000.0f;

    float f = 1.0f / tanf(fov / 2.0f);

    memset(s_projection, 0, sizeof(s_projection));
    s_projection[0] = f / aspect;
    s_projection[5] = f;
    s_projection[10] = (far + near) / (near - far);
    s_projection[11] = -1.0f;
    s_projection[14] = (2.0f * far * near) / (near - far);
}

const float *Transform_GetProjection(void) {
    return s_projection;
}

const float *Transform_GetView(void) {
    return s_view;
}

// Convert fixed 16.16 to float
static float fp16_to_float(int32 val) {
    return (float)val / 65536.0f;
}

void Transform_SetCamera(int32 cx, int32 cy, int32 cz,
                          int16 crx, int16 cry, int16 crz) {
    // Build view matrix from camera position and SNES rotation angles
    uint8 ax = (uint8)(crx & 0xFF);
    uint8 ay = (uint8)(cry & 0xFF);
    uint8 az = (uint8)(crz & 0xFF);

    float sx = s_sin_table[ax], cx_ = s_cos_table[ax];
    float sy = s_sin_table[ay], cy_ = s_cos_table[ay];
    float sz = s_sin_table[az], cz_ = s_cos_table[az];

    // ZYX rotation order (matching SNES)
    float rotation[16];
    Transform_Identity(rotation);
    rotation[0] = cy_ * cz_;
    rotation[1] = cy_ * sz;
    rotation[2] = -sy;
    rotation[4] = sx * sy * cz_ - cx_ * sz;
    rotation[5] = sx * sy * sz + cx_ * cz_;
    rotation[6] = sx * cy_;
    rotation[8] = cx_ * sy * cz_ + sx * sz;
    rotation[9] = cx_ * sy * sz - sx * cz_;
    rotation[10] = cx_ * cy_;

    // Translation (negate for view matrix)
    float tx = -fp16_to_float(cx);
    float ty = -fp16_to_float(cy);
    float tz = -fp16_to_float(cz);

    // View = Rotation^T * Translation
    Transform_Identity(s_view);
    // Transpose the rotation part
    for (int i = 0; i < 3; i++)
        for (int j = 0; j < 3; j++)
            s_view[j * 4 + i] = rotation[i * 4 + j];

    // Apply translation in rotated space
    s_view[12] = s_view[0] * tx + s_view[4] * ty + s_view[8]  * tz;
    s_view[13] = s_view[1] * tx + s_view[5] * ty + s_view[9]  * tz;
    s_view[14] = s_view[2] * tx + s_view[6] * ty + s_view[10] * tz;
}

void Transform_BuildModelMatrix(float *out, int32 x, int32 y, int32 z,
                                 int16 rx, int16 ry, int16 rz) {
    uint8 ax = (uint8)(rx & 0xFF);
    uint8 ay = (uint8)(ry & 0xFF);
    uint8 az = (uint8)(rz & 0xFF);

    float sx = s_sin_table[ax], cx_ = s_cos_table[ax];
    float sy = s_sin_table[ay], cy_ = s_cos_table[ay];
    float sz = s_sin_table[az], cz_ = s_cos_table[az];

    Transform_Identity(out);

    // ZYX rotation (matching SNES rotation order)
    out[0] = cy_ * cz_;
    out[1] = cy_ * sz;
    out[2] = -sy;
    out[4] = sx * sy * cz_ - cx_ * sz;
    out[5] = sx * sy * sz + cx_ * cz_;
    out[6] = sx * cy_;
    out[8] = cx_ * sy * cz_ + sx * sz;
    out[9] = cx_ * sy * sz - sx * cz_;
    out[10] = cx_ * cy_;

    // Translation
    out[12] = fp16_to_float(x);
    out[13] = fp16_to_float(y);
    out[14] = fp16_to_float(z);
}

void Transform_Lerp(float *out, const float *a, const float *b, float alpha) {
    for (int i = 0; i < 16; i++) {
        out[i] = a[i] + (b[i] - a[i]) * alpha;
    }
}
