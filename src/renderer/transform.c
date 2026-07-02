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

// --- SNES -> OpenGL coordinate convention -------------------------------
// The game simulation runs in original SNES Star Fox coordinates:
//   +X right, +Y DOWN, +Z forward (into the screen) — right-handed.
// The compiled shape data (tools/shape_compiler.py) already negates model
// vertex Y ("Star Fox Y-down -> OpenGL Y-up"), so the renderer world is the
// SNES world mirrored in Y:  GL_world = diag(1,-1,1) * SNES_world.
// Under that mirror, translations negate Y and Euler rotations about X and
// Z flip sign (rotation about Y is unchanged).
// The GL camera additionally looks down -Z while the SNES camera looks down
// +Z, so the view matrix gets a view-space Z-row negation. The composite
// conversion is diag(1,-1,-1) — a pure 180-degree rotation about X, no
// mirroring, so face winding is preserved (and culling is disabled anyway).
// Transform_SetCamera / Transform_BuildModelMatrix take SNES-convention
// inputs and apply this conversion internally.

// Camera state (SNES-convention inputs) kept for per-render-frame
// interpolation: the game tick runs at 20Hz but rendering is uncapped, so
// the view matrix is rebuilt each render frame from lerp(prev, curr, alpha)
// (see Transform_SetViewLerp). Objects already interpolate in draw_list.c;
// without camera interpolation everything visibly steps forward/back as the
// camera snaps once per tick.
typedef struct {
    int32 x, y, z;      // fixed 16.16, SNES convention (+Y down)
    int16 rx, ry, rz;   // SNES 0-255 angle units
} CameraState;

static CameraState s_cam_prev, s_cam_curr;
static int s_cam_valid = 0;

// Camera state actually used for the most recent view-matrix build (the
// render-frame interpolated values, SNES conventions, before the GL-space
// negations). Read-only mirror for consumers that must stay in lockstep
// with the 3D view — the 2D background layer slaves the painted horizon
// to this (SNES calcbgscroll_l coupling).
static CameraState s_cam_render;

static void build_view_matrix(int32 cx, int32 cy, int32 cz,
                              int16 crx, int16 cry, int16 crz) {
    s_cam_render.x = cx;   s_cam_render.y = cy;   s_cam_render.z = cz;
    s_cam_render.rx = crx; s_cam_render.ry = cry; s_cam_render.rz = crz;
    // Build view matrix from camera position and SNES rotation angles.
    // SNES -> GL world: negate Y translation, negate X/Z rotation angles.
    cy = -cy;
    crx = (int16)(-crx);
    crz = (int16)(-crz);
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

    // Facing conversion: the SNES camera looks down +Z (player worldz
    // increases as he flies forward; map objects spawn at
    // player_z + positive offset, ahead of the ship), while the OpenGL
    // camera looks down -Z. Negate the view-space Z row so world-forward
    // (+Z) maps to GL view-forward (-Z).
    s_view[2]  = -s_view[2];
    s_view[6]  = -s_view[6];
    s_view[10] = -s_view[10];

    // Apply translation in rotated space
    s_view[12] = s_view[0] * tx + s_view[4] * ty + s_view[8]  * tz;
    s_view[13] = s_view[1] * tx + s_view[5] * ty + s_view[9]  * tz;
    s_view[14] = s_view[2] * tx + s_view[6] * ty + s_view[10] * tz;
}

void Transform_SetCamera(int32 cx, int32 cy, int32 cz,
                          int16 crx, int16 cry, int16 crz) {
    // Advance the interpolation history: previous tick's camera becomes the
    // lerp start. Called exactly once per game tick (GameCamera_Update).
    if (s_cam_valid) {
        s_cam_prev = s_cam_curr;
    }
    s_cam_curr.x = cx;  s_cam_curr.y = cy;  s_cam_curr.z = cz;
    s_cam_curr.rx = crx; s_cam_curr.ry = cry; s_cam_curr.rz = crz;
    if (!s_cam_valid) {
        s_cam_prev = s_cam_curr;
        s_cam_valid = 1;
    }
    build_view_matrix(cx, cy, cz, crx, cry, crz);
}

void Transform_SnapCamera(void) {
    // Camera cut (viewtype change, fixed-position jump, level load):
    // collapse the history so the cut stays instant instead of smearing.
    s_cam_prev = s_cam_curr;
}

// Shortest-path interpolation in the SNES 0-255 angle system.
static int16 lerp_cam_angle(int16 from, int16 to, float t, int *big_jump) {
    int a8 = from & 0xFF;
    int b8 = to & 0xFF;
    int diff = b8 - a8;
    if (diff > 127) diff -= 256;
    if (diff < -128) diff += 256;
    if (diff > 48 || diff < -48) *big_jump = 1;  // > ~67 deg in one tick: cut
    int out = a8 + (int)((float)diff * t);
    return (int16)(out & 0xFF);
}

void Transform_SetViewLerp(float alpha) {
    if (!s_cam_valid) return;
    if (alpha < 0.0f) alpha = 0.0f;
    if (alpha > 1.0f) alpha = 1.0f;

    int big_jump = 0;
    int16 rx = lerp_cam_angle(s_cam_prev.rx, s_cam_curr.rx, alpha, &big_jump);
    int16 ry = lerp_cam_angle(s_cam_prev.ry, s_cam_curr.ry, alpha, &big_jump);
    int16 rz = lerp_cam_angle(s_cam_prev.rz, s_cam_curr.rz, alpha, &big_jump);

    float dx = fp16_to_float(s_cam_curr.x - s_cam_prev.x);
    float dy = fp16_to_float(s_cam_curr.y - s_cam_prev.y);
    float dz = fp16_to_float(s_cam_curr.z - s_cam_prev.z);
    // A displacement this large in one 20Hz tick is a teleport, not motion.
    if (dx * dx + dy * dy + dz * dz > 600.0f * 600.0f) big_jump = 1;

    if (big_jump) {
        s_cam_prev = s_cam_curr;
        build_view_matrix(s_cam_curr.x, s_cam_curr.y, s_cam_curr.z,
                          s_cam_curr.rx, s_cam_curr.ry, s_cam_curr.rz);
        return;
    }

    int32 x = s_cam_prev.x + (int32)((float)(s_cam_curr.x - s_cam_prev.x) * alpha);
    int32 y = s_cam_prev.y + (int32)((float)(s_cam_curr.y - s_cam_prev.y) * alpha);
    int32 z = s_cam_prev.z + (int32)((float)(s_cam_curr.z - s_cam_prev.z) * alpha);
    build_view_matrix(x, y, z, rx, ry, rz);
}

void Transform_GetRenderCamera(int32 *x, int32 *y, int32 *z,
                               int16 *rx, int16 *ry, int16 *rz) {
    if (x)  *x  = s_cam_render.x;
    if (y)  *y  = s_cam_render.y;
    if (z)  *z  = s_cam_render.z;
    if (rx) *rx = s_cam_render.rx;
    if (ry) *ry = s_cam_render.ry;
    if (rz) *rz = s_cam_render.rz;
}

void Transform_BuildModelMatrix(float *out, int32 x, int32 y, int32 z,
                                 int16 rx, int16 ry, int16 rz) {
    // Inputs are SNES-convention world coordinates/angles; convert to the
    // GL world (see the block comment above Transform_SetCamera).
    y = -y;
    rx = (int16)(-rx);
    rz = (int16)(-rz);
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
