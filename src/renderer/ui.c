// State-aware 2D UI pass.
//
// TITLE:         blinking "PRESS START" prompt (only when the composed
//                title texture is unavailable — the real layer already
//                contains the baked-in "PUSH START" text).
// PLANET_SELECT: PLANETS.ASM route map — real planet/sector bitmaps
//                (SuperFX msprites tex_0/tex_1.CGX), the Arwing cursor and
//                dotted course line (MAP-OBJ.CGX OAM tiles), and the
//                LEVEL1/2/3 route label + score (MAP.CGX chars), all at
//                the literal PLANETS.ASM table positions.
//
// Also owns the full-screen fade overlay pass (WINDOWS.ASM state).

#include "ui.h"
#include "gl_backend.h"
#include "font.h"
#include "bg2d.h"
#include "../game/boot.h"
#include "../game/game_vars.h"
#include <glad/glad.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Read-only accessor in src/game/planets.c: PATH_ID_* sequence of a route
// (used to index the stagepaths line-geometry tables below).
extern int Planets_GetRoutePathIds(uint8 route, uint16 *paths_out, int max_nodes);

static GLuint s_vao = 0;
static GLuint s_vbo = 0;
static uint32 s_frame = 0;    // render-frame counter for blink effects

// Planet-select graphics atlas (built lazily on first use)
#define PS_AW 256
#define PS_AH 96
static GLuint s_ps_tex = 0;
static bool   s_ps_tried = false;

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

// Screen mapping state for the current frame (SNES 256x224, y-up,
// square pixels scaled by screen height — matches Font_DrawString).
static float s_scale = 3.0f;
static int   s_ox = 0;        // centering offset in SNES units
static int   s_scr_w = 800, s_scr_h = 600;

// ---------------------------------------------------------------------------
// PLANETS.ASM literal tables
// ---------------------------------------------------------------------------

// planetpos: top-left of each planet's 32x32 square, SNES pixels (y down).
// (planetscrnpos operands * 8.)
static const uint8 kPlanetPos[16][2] = {
    {  16, 176 }, {  64, 176 }, {  56, 136 }, {  16, 128 },
    { 112, 144 }, {  40,  64 }, { 128,  96 }, { 184, 144 },
    { 168,  96 }, { 112,  32 }, {  80,  80 }, { 208,  96 },
    { 192,  40 }, { 192,  40 }, { 136, 176 }, { 192,  40 },
};

// startplanetpos: course-line/ship anchor per planet (planetpathpos * 8).
static const uint8 kStartPos[16][2] = {
    {  16, 176 }, {  64, 176 }, {  64, 136 }, {  16, 128 },
    { 112, 144 }, {  48,  64 }, { 128, 104 }, { 184, 144 },
    { 176,  96 }, { 112,  32 }, {  80,  80 }, { 208,  96 },
    { 192,  40 }, { 192,  40 }, { 136, 176 }, { 192,  40 },
};

// planetsprs: which SuperFX msprites texture cell each planet uses.
// sheet 0 = tex_0.CGX, 1 = tex_1.CGX; cell = 32x32 cell index (4 per row
// in the 128px-wide CGX sheet); sphere = dpsphere (64x32 wrap texture).
typedef struct { uint8 sheet, cell, sphere; } PlanetSpr;
static const PlanetSpr kPlanetSprs[16] = {
    { 1, 24, 1 },   //  0 Corneria      (playerplanet)
    { 1, 18, 0 },   //  1 Asteroid 1    (space2)
    { 1, 18, 0 },   //  2 Asteroid 3    (space2)
    { 1, 17, 0 },   //  3 Sector X      (space1)
    { 1, 28, 1 },   //  4 Fortuna       (planetb)
    { 1, 26, 1 },   //  5 Titania       (planeta)
    { 1, 21, 0 },   //  6 Space Armada  (bigships)
    { 1, 19, 0 },   //  7 Sector Z      (cluster)
    { 1, 16, 0 },   //  8 Meteor        (bigmeteo)
    { 0, 20, 0 },   //  9 Sector Y      (space4)
    { 1, 20, 0 },   // 10 Black Hole    (blackhole)
    { 1, 22, 1 },   // 11 Macbeth       (planetc)
    { 0,  7, 0 },   // 12 unused        (space3)
    { 0, 20, 0 },   // 13 unused        (space4)
    { 0, 14, 0 },   // 14 OOTD          (starwars3)
    { 1, 30, 1 },   // 15 Venom         (enemyplanet)
};

// stagepaths line geometry: 8px steps with MAP-OBJ.CGX segment tiles.
// OAM char = CGX tile + 2 (upchar=4 -> tile 2, rightchar=3 -> tile 1,
// uprightchar=2 -> tile 0). Hidden (_H) steps move without drawing.
enum {
    SEG_HIDDEN = 0,
    SEG_DIAG_UP,    // tile 0        '/'  (PATHUPRIGHT)
    SEG_DIAG_DN,    // tile 0 vflip  '\'  (PATHDOWNRIGHT)
    SEG_HORIZ,      // tile 1             (PATHRIGHT / PATHHIGHRIGHT)
    SEG_VERT,       // tile 2 hflip       (PATHUP / PATHDOWN)
};

typedef struct { int8 dx, dy; uint8 seg; } PathStep;

#define P_UP   {  0, -8, SEG_VERT    }
#define P_UR   {  8, -8, SEG_DIAG_UP }
#define P_R    {  8,  0, SEG_HORIZ   }
#define P_UPH  {  0, -8, SEG_HIDDEN  }
#define P_URH  {  8, -8, SEG_HIDDEN  }
#define P_DRH  {  8,  8, SEG_HIDDEN  }
#define P_RH   {  8,  0, SEG_HIDDEN  }

static const PathStep kSteps1[]  = { P_UP };
static const PathStep kSteps2[]  = { P_UP, P_UP, P_UR };
static const PathStep kSteps3[]  = { P_UR, P_UR, P_UR, P_R, P_R };
static const PathStep kSteps4[]  = { P_R, P_R, P_R, P_R };
static const PathStep kSteps6[]  = { P_UR };
static const PathStep kSteps7[]  = { P_UR, P_UR, P_UR, P_R, P_R };
static const PathStep kSteps8[]  = { P_R };
static const PathStep kSteps9[]  = { P_UP };
static const PathStep kSteps11[] = { P_R, P_R };
static const PathStep kSteps12[] = { P_UR, P_UR };
static const PathStep kSteps13[] = { P_R, P_R, P_R, P_R, P_R };
static const PathStep kSteps14[] = { P_UR, P_UP };
static const PathStep kSteps15[] = { P_UP };
static const PathStep kSteps17[] = { P_URH, P_URH, P_URH };
static const PathStep kSteps18[] = { P_URH, P_URH };
static const PathStep kSteps19[] = { P_RH, P_RH, P_URH, P_URH,
                                     P_RH, P_RH, P_RH, P_RH, P_RH };
static const PathStep kSteps20[] = { P_DRH, P_DRH, P_DRH, P_DRH,
                                     P_RH, P_RH, P_RH, P_RH,
                                     P_RH, P_RH, P_RH, P_RH };
static const PathStep kSteps21[] = { P_UPH, P_UPH, P_URH };
static const PathStep kSteps22[] = { P_RH, P_RH, P_RH, P_RH, P_RH, P_RH };

typedef struct {
    uint8 sx, sy;               // PATHSTART x,y (8px units)
    const PathStep *steps;
    uint8 n;
} PathGeo;

#define GEO(x, y, arr) { x, y, arr, (uint8)(sizeof(arr) / sizeof((arr)[0])) }
#define GEO_NONE       { 0, 0, NULL, 0 }

// Indexed by planets.c PATH_ID_* (1..22, then END3/END2/END1/OTHEREND).
#define UI_PATH_ID_COUNT 27
static const PathGeo kPathGeo[UI_PATH_ID_COUNT] = {
    GEO_NONE,                   //  0 invalid
    GEO( 4, 20, kSteps1),       //  1 Corneria 2
    GEO( 4, 14, kSteps2),       //  2 Sector X
    GEO( 9,  8, kSteps3),       //  3 Titania
    GEO(18,  5, kSteps4),       //  4 Sector Y
    GEO_NONE,                   //  5 Venom 2 orbital
    GEO( 6, 21, kSteps6),       //  6 Corneria 1
    GEO(11, 17, kSteps7),       //  7 Asteroid Belt 1
    GEO(20, 14, kSteps8),       //  8 Space Armada
    GEO(24, 11, kSteps9),       //  9 Meteor
    GEO_NONE,                   // 10 Venom 1 orbital
    GEO( 6, 23, kSteps11),      // 11 Corneria 3
    GEO(12, 21, kSteps12),      // 12 Asteroid Belt 3
    GEO(18, 19, kSteps13),      // 13 Fortuna
    GEO(27, 19, kSteps14),      // 14 Sector Z
    GEO(28, 11, kSteps15),      // 15 Macbeth
    GEO_NONE,                   // 16 Venom 3 orbital
    GEO( 6, 15, kSteps17),      // 17 Sector X  (black hole entry)
    GEO(12,  9, kSteps18),      // 18 Black Hole -> Sector Y
    GEO(13, 11, kSteps19),      // 19 Black Hole -> Venom 1
    GEO(11, 14, kSteps20),      // 20 Black Hole -> Sector Z
    GEO( 9, 16, kSteps21),      // 21 Asteroid 1 (black hole entry)
    GEO(12, 25, kSteps22),      // 22 Asteroid 3 (OOTD entry)
    GEO_NONE,                   // 23 end3
    GEO_NONE,                   // 24 end2
    GEO_NONE,                   // 25 end1
    GEO_NONE,                   // 26 otherend
};

// ---------------------------------------------------------------------------
// Init / shutdown
// ---------------------------------------------------------------------------
void Ui_Init(void) {
    glGenVertexArrays(1, &s_vao);
    glGenBuffers(1, &s_vbo);
    glBindVertexArray(s_vao);
    glBindBuffer(GL_ARRAY_BUFFER, s_vbo);
    glBufferData(GL_ARRAY_BUFFER, 4 * 4 * sizeof(float), NULL, GL_DYNAMIC_DRAW);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void *)0);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void *)(2 * sizeof(float)));
    glEnableVertexAttribArray(1);
    glBindVertexArray(0);
}

void Ui_Shutdown(void) {
    if (s_vao) { glDeleteVertexArrays(1, &s_vao); s_vao = 0; }
    if (s_vbo) { glDeleteBuffers(1, &s_vbo); s_vbo = 0; }
    if (s_ps_tex) { glDeleteTextures(1, &s_ps_tex); s_ps_tex = 0; }
    s_ps_tried = false;
}

// ---------------------------------------------------------------------------
// GL helpers
// ---------------------------------------------------------------------------
static void begin_2d(int w, int h) {
    s_scr_w = w;
    s_scr_h = h;
    s_scale = (float)h / 224.0f;
    s_ox = (int)(((float)w / s_scale - 256.0f) * 0.5f);

    glDisable(GL_DEPTH_TEST);
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

    glUseProgram(g_hud_shader);

    float ortho[16];
    memset(ortho, 0, sizeof(ortho));
    ortho[0]  =  2.0f / (float)w;
    ortho[5]  =  2.0f / (float)h;
    ortho[10] = -1.0f;
    ortho[12] = -1.0f;
    ortho[13] = -1.0f;
    ortho[15] =  1.0f;
    GlBackend_SetMat4(g_hud_shader, "uProj", ortho);

    float model[16];
    memset(model, 0, sizeof(model));
    model[0] = model[5] = model[10] = model[15] = 1.0f;
    GlBackend_SetMat4(g_hud_shader, "uModel", model);

    GLint loc = glGetUniformLocation(g_hud_shader, "uUseTexture");
    if (loc >= 0) glUniform1i(loc, 0);

    glBindVertexArray(s_vao);
}

static void end_2d(void) {
    glBindVertexArray(0);
    glDisable(GL_BLEND);
    glEnable(GL_DEPTH_TEST);
}

// Draw a quad given raw screen-pixel corners
static void quad_px(float x0, float y0, float x1, float y1,
                    float x2, float y2, float x3, float y3) {
    float verts[] = {
        x0, y0, 0.0f, 0.0f,
        x1, y1, 1.0f, 0.0f,
        x2, y2, 1.0f, 1.0f,
        x3, y3, 0.0f, 1.0f,
    };
    glBindBuffer(GL_ARRAY_BUFFER, s_vbo);
    glBufferSubData(GL_ARRAY_BUFFER, 0, sizeof(verts), verts);
    glDrawArrays(GL_TRIANGLE_FAN, 0, 4);
}

// Font wrapper that applies the same centering offset
static void text_snes(int x, int y, const char *str, float r, float g, float b) {
    Font_SetScreenSize(s_scr_w, s_scr_h);
    Font_DrawString(x + s_ox, y, str, r, g, b);
    // Font_DrawString leaves uUseTexture=0 but rebinds program state; ensure
    // our vertex array is current again for subsequent quads.
    glUseProgram(g_hud_shader);
    glBindVertexArray(s_vao);
}

static void text_centered(int cx, int y, const char *str, float r, float g, float b) {
    int len = (int)strlen(str);
    text_snes(cx - len * 4, y, str, r, g, b);
}

// ---------------------------------------------------------------------------
// Title screen UI
// ---------------------------------------------------------------------------
static void render_title(void) {
    // The composed SNES title layer already contains "PUSH START".
    // Only draw our own blinking prompt when running on the fallback backdrop.
    if (!Bg2d_HasTitle()) {
        text_centered(128, 170, "STAR FOX", 1.0f, 0.85f, 0.3f);
        if ((s_frame / 32) & 1) {
            text_centered(128, 96, "PRESS START", 1.0f, 1.0f, 1.0f);
        }
    }
}

// ---------------------------------------------------------------------------
// Planet select graphics atlas
//
// One RGBA texture holding everything the map screen composites on top of
// the MAP.CGX backdrop:
//   y  0..63 : 16 planet icons, 32x32, slot p at (32*(p&7), 32*(p>>3))
//   y 64..79 : Arwing cursor, 3 angles, 16x16 at (a*16, 64);
//              course-line segment tiles at (48+i*8, 64) i=0 '/', 1 '-', 2 '|'
//   y 80..87 : LEVEL2/LEVEL1/LEVEL3 route labels, 48x8 at (n*48, 80);
//              score '0' digit tile at (144, 80)
// ---------------------------------------------------------------------------
static uint8 *ps_load(const char *path, long *size_out) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    uint8 *buf = (uint8 *)malloc((size_t)sz);
    if (!buf) { fclose(f); return NULL; }
    size_t rd = fread(buf, 1, (size_t)sz, f);
    fclose(f);
    if ((long)rd != sz) { free(buf); return NULL; }
    if (size_out) *size_out = sz;
    return buf;
}

// SNES 4bpp planar tile -> 64 linear palette indices
static void ps_decode4(const uint8 *src, uint8 *dst64) {
    for (int row = 0; row < 8; row++) {
        uint8 p0 = src[row * 2 + 0];
        uint8 p1 = src[row * 2 + 1];
        uint8 p2 = src[16 + row * 2 + 0];
        uint8 p3 = src[16 + row * 2 + 1];
        for (int bit = 7; bit >= 0; bit--) {
            dst64[(7 - bit) + row * 8] =
                (uint8)(((p0 >> bit) & 1)
                      | (((p1 >> bit) & 1) << 1)
                      | (((p2 >> bit) & 1) << 2)
                      | (((p3 >> bit) & 1) << 3));
        }
    }
}

// One 16-color BGR555 palette row from a .COL file
static void ps_pal_row(const uint8 *col, int row, uint8 pal[16][4]) {
    for (int c = 0; c < 16; c++) {
        uint16 w = (uint16)(col[row * 32 + c * 2] | (col[row * 32 + c * 2 + 1] << 8));
        pal[c][0] = (uint8)(((w >>  0) & 0x1F) * 255 / 31);
        pal[c][1] = (uint8)(((w >>  5) & 0x1F) * 255 / 31);
        pal[c][2] = (uint8)(((w >> 10) & 0x1F) * 255 / 31);
        pal[c][3] = 255;
    }
}

// Pixel of a CGX viewed as a 128px-wide sheet (16 tiles per row)
static uint8 ps_sheet_px(const uint8 *tiles64, int ntiles, int x, int y) {
    int t = (y >> 3) * 16 + (x >> 3);
    if (t < 0 || t >= ntiles) return 0;
    return tiles64[t * 64 + (y & 7) * 8 + (x & 7)];
}

static void ps_put(uint8 *atlas, int x, int y, const uint8 rgba[4]) {
    uint8 *px = atlas + ((size_t)y * PS_AW + (size_t)x) * 4;
    px[0] = rgba[0]; px[1] = rgba[1]; px[2] = rgba[2]; px[3] = rgba[3];
}

// Flat 32x32 msprites cell -> atlas (color 0 transparent)
static void ps_blit_flat(uint8 *atlas, int ax, int ay,
                         const uint8 *sheet, int ntiles, int cell,
                         const uint8 pal[16][4]) {
    int bx = (cell & 3) * 32, by = (cell >> 2) * 32;
    for (int y = 0; y < 32; y++) {
        for (int x = 0; x < 32; x++) {
            uint8 v = ps_sheet_px(sheet, ntiles, bx + x, by + y);
            if (!v) continue;
            ps_put(atlas, ax + x, ay + y, pal[v]);
        }
    }
}

// dpsphere planet: bake the 64x32 wrap texture onto a shaded 32x32 disc
// (static stand-in for the SuperFX mdrawtsphere renderer; radius 15 as in
// drawplanetsprites' m_radius).
static void ps_blit_sphere(uint8 *atlas, int ax, int ay,
                           const uint8 *sheet, int ntiles, int cell,
                           const uint8 pal[16][4]) {
    int bx = (cell & 3) * 32, by = (cell >> 2) * 32;   // doub: cells c, c+1
    const float R = 15.0f;
    for (int y = 0; y < 32; y++) {
        for (int x = 0; x < 32; x++) {
            float dx = ((float)x - 15.5f) / R;
            float dy = ((float)y - 15.5f) / R;
            float d2 = dx * dx + dy * dy;
            if (d2 > 1.0f) continue;
            float nz = sqrtf(1.0f - d2);
            float lon = asinf(dx);
            float lat = asinf(dy);
            int u = (int)(32.0f + lon * (64.0f / (2.0f * (float)M_PI)));
            int v = (int)(16.0f + lat * (32.0f / (float)M_PI));
            if (u < 0) u = 0;
            if (u > 63) u = 63;
            if (v < 0) v = 0;
            if (v > 31) v = 31;
            uint8 t = ps_sheet_px(sheet, ntiles, bx + u, by + v);
            float shade = 0.30f + 0.70f * nz;          // limb darkening
            uint8 rgba[4];
            rgba[0] = (uint8)((float)pal[t][0] * shade);
            rgba[1] = (uint8)((float)pal[t][1] * shade);
            rgba[2] = (uint8)((float)pal[t][2] * shade);
            rgba[3] = 255;
            ps_put(atlas, ax + x, ay + y, rgba);
        }
    }
}

// 8x8 4bpp tile -> atlas; opaque_bg0: color 0 drawn as pal[0] (BG chars)
static void ps_blit_tile(uint8 *atlas, int ax, int ay,
                         const uint8 *tile64, const uint8 pal[16][4],
                         bool opaque_bg0) {
    for (int y = 0; y < 8; y++) {
        for (int x = 0; x < 8; x++) {
            uint8 v = tile64[y * 8 + x];
            if (!v && !opaque_bg0) continue;
            ps_put(atlas, ax + x, ay + y, pal[v]);
        }
    }
}

static bool ps_ensure_atlas(void) {
    if (s_ps_tex) return true;
    if (s_ps_tried) return false;
    s_ps_tried = true;

    long tex0_sz = 0, tex1_sz = 0, mo_sz = 0, mocol_sz = 0;
    long mcgx_sz = 0, mcol_sz = 0;
    uint8 *tex0  = ps_load("data/map/tex_0.CGX", &tex0_sz);
    uint8 *tex1  = ps_load("data/map/tex_1.CGX", &tex1_sz);
    uint8 *mo    = ps_load("data/map/MAP-OBJ.CGX", &mo_sz);
    uint8 *mocol = ps_load("data/map/MAP-OBJ.COL", &mocol_sz);
    uint8 *mcgx  = ps_load("data/bg/MAP.CGX", &mcgx_sz);
    uint8 *mcol  = ps_load("data/bg/MAP_C.COL", &mcol_sz);

    if (!tex0 || !tex1 || !mo || !mocol || !mcgx || !mcol ||
        tex0_sz < 0x4000 || tex1_sz < 0x4000 || mo_sz < 30 * 32 ||
        mocol_sz < 32 || mcgx_sz < 160 * 32 || mcol_sz < 7 * 32) {
        fprintf(stderr, "Ui: planet select assets missing (data/map, data/bg)\n");
        free(tex0); free(tex1); free(mo); free(mocol); free(mcgx); free(mcol);
        return false;
    }

    // Decode tile data. msprites CGX: only the first 0x4000 bytes are
    // texture data (512 tiles = 128x256 sheet), same limit as CGX2FX.
    uint8 *t0 = (uint8 *)malloc(512 * 64);
    uint8 *t1 = (uint8 *)malloc(512 * 64);
    uint8 *tm = (uint8 *)malloc(30 * 64);
    uint8 *tg = (uint8 *)malloc((size_t)(mcgx_sz / 32) * 64);
    int n_tg = (int)(mcgx_sz / 32);
    for (int t = 0; t < 512; t++) {
        ps_decode4(tex0 + t * 32, t0 + t * 64);
        ps_decode4(tex1 + t * 32, t1 + t * 64);
    }
    for (int t = 0; t < 30; t++)   ps_decode4(mo + t * 32, tm + t * 64);
    for (int t = 0; t < n_tg; t++) ps_decode4(mcgx + t * 32, tg + t * 64);

    // Palettes: planet textures use CGRAM row 0 (planselpal = MAP_C.COL
    // rows 0..14); OBJ cursor/line use MAP-OBJ.COL row 0 (CGRAM row 15);
    // route label/score chars use BG palette 6.
    uint8 pal_map[16][4], pal_obj[16][4], pal_txt[16][4];
    ps_pal_row(mcol, 0, pal_map);
    ps_pal_row(mocol, 0, pal_obj);
    ps_pal_row(mcol, 6, pal_txt);
    // BG color 0 behind the dynamic text chars = CGRAM color 0
    pal_txt[0][0] = pal_map[0][0];
    pal_txt[0][1] = pal_map[0][1];
    pal_txt[0][2] = pal_map[0][2];

    uint8 *atlas = (uint8 *)calloc((size_t)PS_AW * PS_AH * 4, 1);

    // Planet icons
    for (int p = 0; p < 16; p++) {
        const PlanetSpr *ps = &kPlanetSprs[p];
        const uint8 *sheet = ps->sheet ? t1 : t0;
        int ax = (p & 7) * 32, ay = (p >> 3) * 32;
        if (ps->sphere) {
            ps_blit_sphere(atlas, ax, ay, sheet, 512, ps->cell, pal_map);
        } else {
            ps_blit_flat(atlas, ax, ay, sheet, 512, ps->cell, pal_map);
        }
    }

    // Arwing cursor: OAM chars {9,5,13}+0..3 = CGX tiles {7,3,11}+0..3,
    // 2x2 of 8x8 (TL, TR, BL, BR). Order: angle 0 = up, 1 = diagonal,
    // 2 = right (IRQ.ASM ship draw).
    static const uint8 kShipBase[3] = { 7, 3, 11 };
    for (int a = 0; a < 3; a++) {
        int ax = a * 16;
        ps_blit_tile(atlas, ax,     64, tm + (kShipBase[a] + 0) * 64, pal_obj, false);
        ps_blit_tile(atlas, ax + 8, 64, tm + (kShipBase[a] + 1) * 64, pal_obj, false);
        ps_blit_tile(atlas, ax,     72, tm + (kShipBase[a] + 2) * 64, pal_obj, false);
        ps_blit_tile(atlas, ax + 8, 72, tm + (kShipBase[a] + 3) * 64, pal_obj, false);
    }

    // Course line segment tiles 0 '/', 1 '-', 2 '|'
    for (int i = 0; i < 3; i++) {
        ps_blit_tile(atlas, 48 + i * 8, 64, tm + i * 64, pal_obj, false);
    }

    // Route labels (drawroutename): 6 chars each, CGX tile = char - 32.
    // whichroute 0 -> $74 (LEVEL2), 1 -> $6e (LEVEL1), 2 -> $7a (LEVEL3).
    static const uint8 kNameBase[3] = { 0x74, 0x6e, 0x7a };
    for (int n = 0; n < 3; n++) {
        for (int c = 0; c < 6; c++) {
            int tile = kNameBase[n] + c;
            if (tile >= n_tg) continue;
            ps_blit_tile(atlas, n * 48 + c * 8, 80, tg + tile * 64, pal_txt, true);
        }
    }
    // Score digit '0' ($88)
    if (0x88 < n_tg) {
        ps_blit_tile(atlas, 144, 80, tg + 0x88 * 64, pal_txt, true);
    }

    glGenTextures(1, &s_ps_tex);
    glBindTexture(GL_TEXTURE_2D, s_ps_tex);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, PS_AW, PS_AH, 0,
                 GL_RGBA, GL_UNSIGNED_BYTE, atlas);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glBindTexture(GL_TEXTURE_2D, 0);

    free(atlas);
    free(t0); free(t1); free(tm); free(tg);
    free(tex0); free(tex1); free(mo); free(mocol); free(mcgx); free(mcol);

    printf("Ui: planet select atlas built (%dx%d)\n", PS_AW, PS_AH);
    return true;
}

// Draw an atlas rect at SNES coords (x, y top-left, y down), stretched with
// the same full-screen 256x224 mapping as the Bg2d backdrop.
static void ps_draw(int x, int y, int w, int h,
                    int ax, int ay, bool hflip, bool vflip) {
    float sx = (float)s_scr_w / 256.0f;
    float sy = (float)s_scr_h / 224.0f;

    float x0 = (float)x * sx;
    float x1 = (float)(x + w) * sx;
    float ytop = (float)(224 - y) * sy;
    float ybot = (float)(224 - y - h) * sy;

    float u0 = (float)ax / (float)PS_AW;
    float u1 = (float)(ax + w) / (float)PS_AW;
    float v0 = (float)ay / (float)PS_AH;         // atlas top
    float v1 = (float)(ay + h) / (float)PS_AH;   // atlas bottom

    if (hflip) { float t = u0; u0 = u1; u1 = t; }
    if (vflip) { float t = v0; v0 = v1; v1 = t; }

    float verts[] = {
        x0, ybot, u0, v1,
        x1, ybot, u1, v1,
        x1, ytop, u1, v0,
        x0, ytop, u0, v0,
    };
    glBindBuffer(GL_ARRAY_BUFFER, s_vbo);
    glBufferSubData(GL_ARRAY_BUFFER, 0, sizeof(verts), verts);
    glDrawArrays(GL_TRIANGLE_FAN, 0, 4);
}

// ---------------------------------------------------------------------------
// Planet select route map (PLANETS.ASM planetseq)
// ---------------------------------------------------------------------------
static void render_planet_select(void) {
    if (!ps_ensure_atlas()) return;

    GLint loc_tex = glGetUniformLocation(g_hud_shader, "uUseTexture");
    if (loc_tex >= 0) glUniform1i(loc_tex, 1);
    GLint samp = glGetUniformLocation(g_hud_shader, "uTexture");
    if (samp >= 0) glUniform1i(samp, 0);
    GlBackend_SetVec4(g_hud_shader, "uColor", 1.0f, 1.0f, 1.0f, 1.0f);
    glActiveTexture(GL_TEXTURE0);
    glBindTexture(GL_TEXTURE_2D, s_ps_tex);

    // Planet/sector bitmaps (drawplanetsprites): all 16 slots; slot 14
    // (Out Of This Dimension) only once the nebula route is open.
    for (int p = 0; p < 16; p++) {
        if (p == 14 && !g_nebula_on) continue;
        ps_draw(kPlanetPos[p][0], kPlanetPos[p][1], 32, 32,
                (p & 7) * 32, (p >> 3) * 32, false, false);
    }

    // Course line (drawplanetlines_l): dotted 8px steps along the selected
    // route's stagepaths, blinking like the select loop (gameframe bit 1).
    bool line_on = (g_stage != 0) || ((s_frame & 2) != 0);
    if (line_on) {
        uint16 path_ids[16];
        int n_nodes = Planets_GetRoutePathIds(g_whichroute, path_ids, 16);
        for (int i = 0; i < n_nodes; i++) {
            if (path_ids[i] >= UI_PATH_ID_COUNT) continue;
            const PathGeo *geo = &kPathGeo[path_ids[i]];
            int px = geo->sx * 8, py = geo->sy * 8;
            for (int s = 0; s < geo->n; s++) {
                const PathStep *st = &geo->steps[s];
                switch (st->seg) {
                case SEG_DIAG_UP: ps_draw(px, py, 8, 8, 48, 64, false, false); break;
                case SEG_DIAG_DN: ps_draw(px, py, 8, 8, 48, 64, false, true);  break;
                case SEG_HORIZ:   ps_draw(px, py, 8, 8, 56, 64, false, false); break;
                case SEG_VERT:    ps_draw(px, py, 8, 8, 64, 64, true,  false); break;
                default: break;   // hidden step: move only
                }
                px += st->dx;
                py += st->dy;
            }
        }
    }

    // Arwing cursor: 16x16 at startplanetpos + 8, initial shipangle = 1
    // (diagonal), solid during selection.
    if (g_currentplanet >= 0 && g_currentplanet < 16) {
        ps_draw(kStartPos[g_currentplanet][0] + 8,
                kStartPos[g_currentplanet][1] + 8,
                16, 16, 1 * 16, 64, false, false);
    }

    // Route label (drawroutename): 6 chars at BG2 tilemap (24, 25) and the
    // score line "00000" at (24, 24).
    {
        int n = (g_whichroute < 3) ? g_whichroute : 0;
        ps_draw(192, 200, 48, 8, n * 48, 80, false, false);
        for (int d = 0; d < 5; d++) {
            ps_draw(192 + d * 8, 192, 8, 8, 144, 80, false, false);
        }
    }

    glBindTexture(GL_TEXTURE_2D, 0);
    if (loc_tex >= 0) glUniform1i(loc_tex, 0);
}

// ---------------------------------------------------------------------------
// Public passes
// ---------------------------------------------------------------------------
void Ui_Render(int screen_width, int screen_height) {
    s_frame++;

    if (g_game_state != GAME_STATE_TITLE &&
        g_game_state != GAME_STATE_PLANET_SELECT) {
        return;
    }

    begin_2d(screen_width, screen_height);

    switch (g_game_state) {
    case GAME_STATE_TITLE:         render_title();         break;
    case GAME_STATE_PLANET_SELECT: render_planet_select(); break;
    default: break;
    }

    // Debug overlay (SF_UI_DEBUG=1): font sample + planet map preview,
    // usable from any state for headless verification.
    if (getenv("SF_UI_DEBUG")) {
        text_snes(-60, 200, "THE QUICK BROWN FOX 0123456789", 1.0f, 1.0f, 1.0f);
        text_snes(-60, 188, "JUMPS OVER THE LAZY DOG - ! ?", 0.5f, 1.0f, 0.6f);
        if (g_game_state != GAME_STATE_PLANET_SELECT) {
            render_planet_select();
        }
    }

    end_2d();
}

// Fade overlay: wm_val 0..30 (black/mapfade) or 0..31 (white) maps to alpha.
// wm_val == 0 means fully visible; max value means fully covered.
void Ui_RenderFade(int screen_width, int screen_height) {
    float black_a = 0.0f;
    float white_a = 0.0f;

    for (int i = 0; i < WINDOWARRAY_SIZE; i++) {
        if ((g_windowmode & (1u << i)) == 0) continue;
        const WindowState *w = &g_windowarray[i];
        switch (w->mode) {
        case WINDOW_MODE_BLACK:
        case WINDOW_MODE_MAPFADE: {
            float a = (float)w->wm_val / 30.0f;
            if (a > 1.0f) a = 1.0f;
            if (a > black_a) black_a = a;
            break;
        }
        case WINDOW_MODE_WHITEFADE:
        case WINDOW_MODE_WHITE2NORM: {
            float a = (float)w->wm_val / 31.0f;
            if (a > 1.0f) a = 1.0f;
            if (a > white_a) white_a = a;
            break;
        }
        default:
            break;
        }
    }

    if (black_a < 0.004f && white_a < 0.004f) return;

    begin_2d(screen_width, screen_height);

    if (black_a >= 0.004f) {
        GlBackend_SetVec4(g_hud_shader, "uColor", 0.0f, 0.0f, 0.0f, black_a);
        quad_px(0, 0, (float)screen_width, 0,
                (float)screen_width, (float)screen_height, 0, (float)screen_height);
    }
    if (white_a >= 0.004f) {
        GlBackend_SetVec4(g_hud_shader, "uColor", 1.0f, 1.0f, 1.0f, white_a);
        quad_px(0, 0, (float)screen_width, 0,
                (float)screen_width, (float)screen_height, 0, (float)screen_height);
    }

    end_2d();
}
