// 2D background layer pass.
//
// Composes SNES BG layers CPU-side from the uncompressed dev assets
// (.CGX tiles / .SCR tilemaps / .COL palettes in reference DATA/, copied
// to data/title/ and data/bg/) into 256x224 RGBA textures drawn as a
// screen-space quad before the 3D pass.
//
// Title screen (BGS.ASM bg_title_1):
//   BG3: TI-3-US.CGX + TI-3-US.SCR  (2bpp "STAR FOX" logo layer, vofs 9)
//   BG2: CP.CGX + CP.SCR            (4bpp crew portraits / PUSH START
//                                    backdrop, vscroll 257 -> +1 px)
//   CGRAM: CP-US.COL                (US Rev 1/2 title palette)
//
// Gameplay backgrounds (BGS.ASM bg_* blocks, see s_bg_defs below): each
// block names a 4bpp BG2 CGX+SCR pair, a CGRAM palette, and a bg2Yscroll
// base vertical offset. The 8 KB .SCR files are 64x64-tile maps stored as
// four 32x32 screens in SNES order (BG2SC size bits = 3 in MAIN.ASM); the
// 2 KB ones are a single 32x32 screen. Uncompressed .SCR tile indices are
// 0-based against their .CGX (the ROM's mario-decruncher applies the
// scr_offset/192 VRAM bias at decompression time, BANK2.ASM:44).
//
// Camera coupling (BGS.ASM info von/hon flags): every render frame the
// SNES recomputes the BG2 scroll from the view rotation so the painted
// horizon stays glued to the 3D ground plane (GSTRATS.ASM calcbgscroll_l):
//   bg2scroll     = (bg2Yscroll + clamp(-(outvx>>6 + outvx>>7), -56, 232)) & 511
//   m_scrollxoff  =  bg2Xscroll + ((outvy - player_turnrot) >> 5)
// and the `hofmode rotate` HDMA program adds worldx>>3 strafe parallax
// (TRANS.ASM rotplanet -> MARIO/MHOFS.MC mrotplanet). Defs flagged `sky`
// below (BGS blocks with von/hon) are composed as the FULL wrapping
// tilemap and scrolled per render frame from the interpolated camera;
// voff blocks (tunnels, water, black hole) stay static like the original.
//
// Deviations from hardware (acceptable for this pass):
//   - hofmode rotate/tunnel* per-scanline HDMA (horizon ROLL tilt, tunnel
//     warp) is not applied; the uniform base scroll is (camera rz is 0 in
//     normal flight, so no visible mismatch).
//   - The vertical pitch gain uses the port projection's real focal length
//     (f*tan(pitch)) instead of the SNES linear 6 px/unit, which encoded
//     the SNES projection's ~244 px focal; this keeps the painted horizon
//     exactly on the port's y=0 vanishing line.
//   - Backdrop = CGRAM color 0 (no HDMA color-gradient enhancement).
//
// Note: data/gfx_title.bin (raw ROM slice at 0x010000) turned out to be
// 65816 code, not graphics — the ROM stores its tile data compressed
// ("mario" decrunch in BGS.ASM), so we load the uncompressed dev assets
// instead (same pattern as data/sprites/OBJ-1.CGX).

#include "bg2d.h"
#include "gl_backend.h"
#include "transform.h"
#include "../game/boot.h"
#include "../game/bgs.h"
#include "../game/game_vars.h"
#include "../map/levels.h"      // MAP_ID_* for per-map default backgrounds
#include <glad/glad.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define BG2D_W 256
#define BG2D_H 224

// Background ids from levels.c map bytecode (setbg opcode operand)
#define BG2D_ID_TITLE   41u
// Pseudo-id for the planet-select / briefing map screen (PLANETS.ASM:
// MAP.CGX + MAP.SCR + COL/MAP_C.COL, mode 3 BG2). Not a setbg id.
#define BG2D_ID_MAP     63u
// Pseudo-id for bg_special (SNES id 44 clashes with the port's BG_TRAINING)
#define BG2D_ID_SPECIAL 62u

// BGS.ASM bg_* block -> data files. All BG2 layers are 4bpp; the optional
// overlay (BG3) layer is 2bpp. Ids are the port's levels.c BG_* constants.
typedef struct {
    uint8 id;
    const char *name;          // BGS.ASM block, for logging
    const char *cgx;           // BG2 tiles (4bpp)
    const char *scr;           // BG2 tilemap
    const char *col;           // CGRAM palette
    int vofs;                  // bg2Yscroll base
    const char *cgx3;          // optional BG3 overlay tiles (2bpp)
    const char *scr3;          // optional BG3 overlay tilemap
    int vofs3;                 // setbg3vofs
    uint8 sky;                 // BGS.ASM info von/hon: camera-coupled scroll
} BgDef;

static const BgDef s_bg_defs[] = {
    // Corneria family (ST-P sky + mountain horizon + ground gradient)
    {  4, "bg_1_1c",     "data/bg/ST-P.CGX",   "data/bg/ST-P.SCR",   "data/bg/BG2-D.COL", 232, NULL, NULL, 0, 1 },
    {  3, "bg_3_1c",     "data/bg/ST-P.CGX",   "data/bg/ST-P.SCR",   "data/bg/BG2-G.COL", 232, NULL, NULL, 0, 1 },
    { 44, "bg_training", "data/bg/ST-P.CGX",   "data/bg/ST-P.SCR",   "data/bg/BG2-D.COL", 232, NULL, NULL, 0, 1 },
    // Asteroid-belt space (starfield + cratered moon)
    {  6, "bg_1_3i",     "data/bg/3-4.CGX",    "data/bg/3-4.SCR",    "data/bg/SPACE.COL", 232, NULL, NULL, 0, 1 },
    {  7, "bg_1_3a",     "data/bg/3-4.CGX",    "data/bg/3-4.SCR",    "data/bg/SPACE.COL", 232, NULL, NULL, 0, 1 },
    {  9, "bg_1_3c",     "data/bg/3-4.CGX",    "data/bg/3-4.SCR",    "data/bg/SPACE.COL", 232, NULL, NULL, 0, 1 },
    { 35, "bg_3_4d",     "data/bg/3-4.CGX",    "data/bg/3-4.SCR",    "data/bg/SPACE.COL",   0, NULL, NULL, 0, 1 },
    // Asteroid clear demo (asteroid + planets starfield)
    { 12, "bg_1_3e",     "data/bg/SPACE.CGX",  "data/bg/1-3.SCR",    "data/bg/SPACE.COL", 232, NULL, NULL, 0, 1 },
    // Tunnels (info voff; static base image; original warps it per-scanline)
    {  8, "bg_1_3b",     "data/bg/T-SP.CGX",   "data/bg/T-SP.SCR",   "data/bg/T-M-3.COL",   0, NULL, NULL, 0, 0 },
    { 34, "bg_3_4c",     "data/bg/T-SP.CGX",   "data/bg/T-SP.SCR",   "data/bg/T-M-3.COL",   0, NULL, NULL, 0, 0 },
    { 25, "bg_2_3c",     "data/bg/T-SP.CGX",   "data/bg/T-F-S.SCR",  "data/bg/T-M-3.COL",   0, NULL, NULL, 0, 0 },
    { 29, "bg_2_6c",     "data/bg/T-ST.CGX",   "data/bg/T-ST.SCR",   "data/bg/T-M-3.COL",   0, NULL, NULL, 0, 0 },
    // Venom final approach (glowing spheres; info voff)
    { 17, "bg_1_6c",     "data/bg/B-HOLE.CGX", "data/bg/LAST.SCR",   "data/bg/BG2-F.COL",   0, NULL, NULL, 0, 0 },
    // Fortuna bridge (sky/water backdrop + BG3 water-surface overlay; voff)
    { 24, "bg_2_3b",     "data/bg/B-M.CGX",    "data/bg/2-3B.SCR",   "data/bg/B-M.COL",     0,
                         "data/bg/2-3B.CGX",   "data/bg/2-3H.SCR",   24, 0 },
    // Attract intro (Corneria seen from space; info von,hon)
    { 40, "bg_intro",    "data/bg/DEMO.CGX",   "data/bg/DEMO.SCR",   "data/bg/BG2-B.COL",  24, NULL, NULL, 0, 1 },
    // Continue / controller screen (US CONT-2; info voff)
    { 42, "bg_cont",     "data/bg/CONT-2.CGX", "data/bg/CONT-2.SCR", "data/bg/BG2-E.COL",   0, NULL, NULL, 0, 0 },
    // Credits (nebula starfield; info von,hon)
    { 43, "bg_cred",     "data/bg/2-4.CGX",    "data/bg/2-4.SCR",    "data/bg/BG2-F.COL", 232, NULL, NULL, 0, 1 },
    // Planet select / briefing map screen (pseudo-id)
    { BG2D_ID_MAP, "planets_map", "data/bg/MAP.CGX", "data/bg/MAP.SCR", "data/bg/MAP_C.COL", 0, NULL, NULL, 0, 0 },
    // --- Level-default backgrounds (SNES bglists ids; used via the per-map
    // --- default table until each level's setbg opcodes are ported) ---
    {  5, "bg_1_2",      "data/bg/STARS.CGX",  "data/bg/STARS.SCR",  "data/bg/STARS.COL", 232, NULL, NULL, 0, 1 },
    { 13, "bg_1_4",      "data/bg/1-4.CGX",    "data/bg/1-4.SCR",    "data/bg/LIGHT.COL", 232, NULL, NULL, 0, 1 },
    { 14, "bg_1_5",      "data/bg/LSB.CGX",    "data/bg/LSB.SCR",    "data/bg/BG2-C.COL", 164, NULL, NULL, 0, 1 },
    { 37, "bg_3_6",      "data/bg/LSB.CGX",    "data/bg/LSB.SCR",    "data/bg/BG2-C.COL", 164, NULL, NULL, 0, 1 },
    { 15, "bg_1_6a",     "data/bg/F-1.CGX",    "data/bg/F-1.SCR",    "data/bg/BG2-A.COL", 232, NULL, NULL, 0, 1 },
    { 38, "bg_3_7a",     "data/bg/F-1.CGX",    "data/bg/F-1.SCR",    "data/bg/BG2-A.COL", 232, NULL, NULL, 0, 1 },
    { 22, "bg_2_2",      "data/bg/2-2.CGX",    "data/bg/2-2.SCR",    "data/bg/SPACE.COL", 232, NULL, NULL, 0, 1 },
    { 23, "bg_2_3a",     "data/bg/2-3.CGX",    "data/bg/2-3.SCR",    "data/bg/BG2-A.COL", 232, NULL, NULL, 0, 1 },
    { 26, "bg_2_4",      "data/bg/2-4.CGX",    "data/bg/2-4.SCR",    "data/bg/BG2-F.COL", 232, NULL, NULL, 0, 1 },
    { 27, "bg_2_6a",     "data/bg/C-M.CGX",    "data/bg/T-SS.SCR",   "data/bg/T-M-2.COL",   0,
                         "data/bg/FS-BG3.CGX", "data/bg/FS-NI.SCR",  0, 0 },
    { 30, "bg_3_2",      "data/bg/3-2.CGX",    "data/bg/3-2.SCR",    "data/bg/BG2-B.COL",  24, NULL, NULL, 0, 1 },
    { 31, "bg_3_3a",     "data/bg/3-3.CGX",    "data/bg/3-3.SCR",    "data/bg/BG2-C.COL", 232, NULL, NULL, 0, 1 },
    { 33, "bg_3_4b",     "data/bg/3-4.CGX",    "data/bg/3-4.SCR",    "data/bg/SPACE.COL", 232, NULL, NULL, 0, 1 },
    { 36, "bg_3_5",      "data/bg/HOLE-A.CGX", "data/bg/HOLE-A.SCR", "data/bg/HOLE.COL",  272, NULL, NULL, 0, 1 },
    // bg_hole: info hon,voff -> static vertical (bhole hofs warp not ported)
    { 39, "bg_hole",     "data/bg/B-HOLE.CGX", "data/bg/B-HOLE.SCR", "data/bg/BG2-D.COL",   0, NULL, NULL, 0, 0 },
    { BG2D_ID_SPECIAL, "bg_special", "data/bg/M.CGX", "data/bg/M.SCR", "data/bg/SPACE.COL", 448, NULL, NULL, 0, 1 },
};

// Default bg id per loaded map (from each LEVEL*.ASM's opening setbg /
// BGS.ASM bglists). Most port levels don't run their setbg opcodes yet, so
// g_currentbg keeps a stale value across level entry; this table supplies
// the level's opening background until the map's own setbg changes it.
typedef struct { uint32 map_id; uint8 bg_id; } MapDefaultBg;
static const MapDefaultBg s_map_default_bg[] = {
    { MAP_ID_1_1, 4 },  { MAP_ID_1_2, 5 },  { MAP_ID_1_3, 6 },
    { MAP_ID_1_4, 13 }, { MAP_ID_1_5, 14 }, { MAP_ID_1_6, 15 },
    { MAP_ID_2_1, 4 },  { MAP_ID_2_2, 22 }, { MAP_ID_2_3, 23 },
    { MAP_ID_2_4, 26 }, { MAP_ID_2_5, 14 }, { MAP_ID_2_6, 27 },
    { MAP_ID_3_1, 3 },  { MAP_ID_3_2, 30 }, { MAP_ID_3_3, 31 },
    { MAP_ID_3_4, 33 }, { MAP_ID_3_5, 36 }, { MAP_ID_3_6, 37 },
    { MAP_ID_3_7, 38 },
    { MAP_ID_BLACKHOLE, 39 }, { MAP_ID_SPECIAL, BG2D_ID_SPECIAL },
    { MAP_ID_FINAL, 17 },     { MAP_ID_INTRO, 40 },
    { MAP_ID_TITLE, BG2D_ID_TITLE }, { MAP_ID_CONTINUE, 42 },
    { MAP_ID_CREDITS, 43 },   { MAP_ID_TRAINING, 44 },
};
#define NUM_BG_DEFS ((int)(sizeof(s_bg_defs) / sizeof(s_bg_defs[0])))

static GLuint s_title_tex = 0;
static GLuint s_def_tex[NUM_BG_DEFS];      // 0 until built
static bool s_def_tried[NUM_BG_DEFS];
// Tilemap pixel size for sky (camera-coupled) textures; 0 for the static
// pre-baked 256x224 composites, which are drawn with plain 0..1 UVs.
static int s_def_map_w[NUM_BG_DEFS];
static int s_def_map_h[NUM_BG_DEFS];
static GLuint s_vao = 0;
static GLuint s_vbo = 0;
static uint64 s_warned_bgs = 0;   // one log per unknown bg id (ids 0-63)

// ---------------------------------------------------------------------------
// File / SNES decode helpers
// ---------------------------------------------------------------------------
static uint8 *load_file(const char *path, long *size_out) {
    FILE *f = fopen(path, "rb");
    if (!f) {
        fprintf(stderr, "Bg2d: cannot open %s\n", path);
        return NULL;
    }
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    uint8 *buf = (uint8 *)malloc(size);
    if (!buf) { fclose(f); return NULL; }
    size_t rd = fread(buf, 1, (size_t)size, f);
    fclose(f);
    if ((long)rd != size) { free(buf); return NULL; }
    if (size_out) *size_out = size;
    return buf;
}

static void decode_2bpp_tile(const uint8 *src, uint8 *dst8x8) {
    for (int row = 0; row < 8; row++) {
        uint8 p0 = src[row * 2 + 0];
        uint8 p1 = src[row * 2 + 1];
        for (int bit = 7; bit >= 0; bit--) {
            uint8 v = (uint8)(((p0 >> bit) & 1) | (((p1 >> bit) & 1) << 1));
            dst8x8[(7 - bit) + row * 8] = v;
        }
    }
}

static void decode_4bpp_tile(const uint8 *src, uint8 *dst8x8) {
    for (int row = 0; row < 8; row++) {
        uint8 p0 = src[row * 2 + 0];
        uint8 p1 = src[row * 2 + 1];
        uint8 p2 = src[16 + row * 2 + 0];
        uint8 p3 = src[16 + row * 2 + 1];
        for (int bit = 7; bit >= 0; bit--) {
            uint8 v = (uint8)(((p0 >> bit) & 1)
                            | (((p1 >> bit) & 1) << 1)
                            | (((p2 >> bit) & 1) << 2)
                            | (((p3 >> bit) & 1) << 3));
            dst8x8[(7 - bit) + row * 8] = v;
        }
    }
}

// BGR555 -> RGB888
static void cgram_color(const uint8 *col, int index, uint8 rgb[3]) {
    uint16 w = (uint16)(col[index * 2] | (col[index * 2 + 1] << 8));
    rgb[0] = (uint8)(((w >>  0) & 0x1F) * 255 / 31);
    rgb[1] = (uint8)(((w >>  5) & 0x1F) * 255 / 31);
    rgb[2] = (uint8)(((w >> 10) & 0x1F) * 255 / 31);
}

// Tilemap entry for map pixel (mx, my). 8 KB .SCR = 64x64 tiles stored as
// four 32x32 screens (TL, TR, BL, BR); 2 KB = one 32x32 screen.
static uint16 scr_entry(const uint8 *scr, long scr_sz, int mx, int my) {
    int quads_per_row = (scr_sz >= 8192) ? 2 : 1;
    int quad = (my / 256) * quads_per_row + (mx / 256) % quads_per_row;
    long off = (long)quad * 2048
             + (((my % 256) / 8) * 32 + ((mx % 256) / 8)) * 2;
    if (off + 1 >= scr_sz) return 0;
    return (uint16)(scr[off] | (scr[off + 1] << 8));
}

static GLuint upload_rgba(const uint8 *rgba, int w, int h, GLint wrap) {
    GLuint tex = 0;
    glGenTextures(1, &tex);
    glBindTexture(GL_TEXTURE_2D, tex);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, w, h, 0,
                 GL_RGBA, GL_UNSIGNED_BYTE, rgba);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, wrap);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, wrap);
    glBindTexture(GL_TEXTURE_2D, 0);
    return tex;
}

// ---------------------------------------------------------------------------
// Generic gameplay background composer (BG2 4bpp + optional BG3 2bpp)
// ---------------------------------------------------------------------------
static GLuint build_bg_texture(const BgDef *def, int *map_w_out, int *map_h_out) {
    long cgx_sz = 0, scr_sz = 0, col_sz = 0, cgx3_sz = 0, scr3_sz = 0;
    *map_w_out = 0;
    *map_h_out = 0;
    uint8 *cgx = load_file(def->cgx, &cgx_sz);
    uint8 *scr = load_file(def->scr, &scr_sz);
    uint8 *col = load_file(def->col, &col_sz);
    uint8 *cgx3 = NULL, *scr3 = NULL;
    if (def->cgx3) {
        cgx3 = load_file(def->cgx3, &cgx3_sz);
        scr3 = load_file(def->scr3, &scr3_sz);
    }

    if (!cgx || !scr || !col || scr_sz < 2048 || col_sz < 512) {
        fprintf(stderr, "Bg2d: %s assets missing/short, using fallback backdrop\n",
                def->name);
        free(cgx); free(scr); free(col); free(cgx3); free(scr3);
        return 0;
    }

    int n2 = (int)(cgx_sz / 32);                       // 4bpp: 32 bytes/tile
    uint8 *px2 = (uint8 *)malloc((size_t)n2 * 64);
    for (int t = 0; t < n2; t++) decode_4bpp_tile(cgx + t * 32, px2 + t * 64);

    int n3 = 0;
    uint8 *px3 = NULL;
    if (cgx3 && scr3 && scr3_sz >= 2048) {
        n3 = (int)(cgx3_sz / 16);                      // 2bpp: 16 bytes/tile
        px3 = (uint8 *)malloc((size_t)n3 * 64);
        for (int t = 0; t < n3; t++) decode_2bpp_tile(cgx3 + t * 16, px3 + t * 64);
    }

    int map_h2 = (scr_sz >= 8192) ? 512 : 256;
    int map_h3 = (scr3_sz >= 8192) ? 512 : 256;

    uint8 backdrop[3];
    cgram_color(col, 0, backdrop);                     // SNES backdrop color

    // Sky (von/hon) layers: compose the FULL wrapping BG2 tilemap so the
    // per-frame camera scroll can window into it (the SNES tilemap wraps;
    // GL_REPEAT wraps ours the same way). Static layers keep the pre-baked
    // 256x224 window with the base vofs applied.
    int sky = def->sky != 0;
    int out_w = sky ? ((scr_sz >= 8192) ? 512 : 256) : BG2D_W;
    int out_h = sky ? map_h2 : BG2D_H;

    uint8 *rgba = (uint8 *)calloc((size_t)out_w * out_h * 4, 1);

    for (int y = 0; y < out_h; y++) {
        // Flip vertically so GL row 0 = picture bottom (standard UVs)
        uint8 *out_row = rgba + (size_t)(out_h - 1 - y) * out_w * 4;

        int my2 = (y + (sky ? 0 : def->vofs)) % map_h2;
        if (my2 < 0) my2 += map_h2;
        int my3 = (y + def->vofs3) % map_h3;
        if (my3 < 0) my3 += map_h3;

        for (int x = 0; x < out_w; x++) {
            uint8 rgb[3] = { backdrop[0], backdrop[1], backdrop[2] };

            // --- BG2 (4bpp) ---
            {
                uint16 e = scr_entry(scr, scr_sz, x, my2);
                int tile = e & 0x3FF;
                int pal  = (e >> 10) & 7;
                int r = my2 & 7, c = x & 7;
                if (e & 0x8000) r = 7 - r;
                if (e & 0x4000) c = 7 - c;
                if (tile < n2) {
                    uint8 v = px2[tile * 64 + r * 8 + c];
                    if (v) cgram_color(col, pal * 16 + v, rgb);
                }
            }

            // --- optional BG3 overlay (2bpp; never present on sky defs) ---
            if (px3 && !sky) {
                uint16 e = scr_entry(scr3, scr3_sz, x, my3);
                int tile = e & 0x3FF;
                int pal  = (e >> 10) & 7;
                int r = my3 & 7, c = x & 7;
                if (e & 0x8000) r = 7 - r;
                if (e & 0x4000) c = 7 - c;
                if (tile < n3) {
                    uint8 v = px3[tile * 64 + r * 8 + c];
                    if (v) cgram_color(col, pal * 4 + v, rgb);
                }
            }

            uint8 *px = out_row + (size_t)x * 4;
            px[0] = rgb[0]; px[1] = rgb[1]; px[2] = rgb[2]; px[3] = 255;
        }
    }

    GLuint tex = upload_rgba(rgba, out_w, out_h,
                             sky ? GL_REPEAT : GL_CLAMP_TO_EDGE);
    if (sky) {
        *map_w_out = out_w;
        *map_h_out = out_h;
    }

    free(rgba);
    free(px2); free(px3);
    free(cgx); free(scr); free(col); free(cgx3); free(scr3);

    printf("Bg2d: composed %s (%d tiles, vofs %d%s)\n", def->name, n2,
           def->vofs, sky ? ", sky-coupled" : "");
    return tex;
}

// Lazily build the texture for a bg id; returns the def index or -1.
static int layer_index_for_id(uint8 id) {
    for (int i = 0; i < NUM_BG_DEFS; i++) {
        if (s_bg_defs[i].id != id) continue;
        if (!s_def_tried[i]) {
            s_def_tried[i] = true;
            s_def_tex[i] = build_bg_texture(&s_bg_defs[i],
                                            &s_def_map_w[i], &s_def_map_h[i]);
        }
        return i;
    }
    return -1;
}


// ---------------------------------------------------------------------------
// Compose the title screen (BG2 backdrop + BG3 logo) into an RGBA texture
// ---------------------------------------------------------------------------
static void build_title_texture(void) {
    long ti_cgx_sz, ti_scr_sz, cp_cgx_sz, cp_scr_sz, col_sz;
    uint8 *ti_cgx = load_file("data/title/TI-3-US.CGX", &ti_cgx_sz);
    uint8 *ti_scr = load_file("data/title/TI-3-US.SCR", &ti_scr_sz);
    uint8 *cp_cgx = load_file("data/title/CP.CGX", &cp_cgx_sz);
    uint8 *cp_scr = load_file("data/title/CP.SCR", &cp_scr_sz);
    uint8 *col    = load_file("data/title/CP-US.COL", &col_sz);

    if (!ti_cgx || !ti_scr || !cp_cgx || !cp_scr || !col ||
        ti_scr_sz < 2048 || cp_scr_sz < 2048 || col_sz < 512) {
        fprintf(stderr, "Bg2d: title assets missing/short, using fallback backdrop\n");
        free(ti_cgx); free(ti_scr); free(cp_cgx); free(cp_scr); free(col);
        return;
    }

    // Pre-decode all tiles
    int n_ti = (int)(ti_cgx_sz / 16);   // 2bpp: 16 bytes/tile
    int n_cp = (int)(cp_cgx_sz / 32);   // 4bpp: 32 bytes/tile
    uint8 *ti_px = (uint8 *)malloc((size_t)n_ti * 64);
    uint8 *cp_px = (uint8 *)malloc((size_t)n_cp * 64);
    for (int t = 0; t < n_ti; t++) decode_2bpp_tile(ti_cgx + t * 16, ti_px + t * 64);
    for (int t = 0; t < n_cp; t++) decode_4bpp_tile(cp_cgx + t * 32, cp_px + t * 64);

    uint8 *rgba = (uint8 *)calloc(BG2D_W * BG2D_H * 4, 1);

    for (int y = 0; y < BG2D_H; y++) {
        // Flip vertically so GL row 0 = picture bottom (standard UVs)
        uint8 *out_row = rgba + (size_t)(BG2D_H - 1 - y) * BG2D_W * 4;

        int by2 = (y + 1) % 256;   // BG2: bg2Yscroll 257 -> effective +1
        int by3 = (y + 9) % 256;   // BG3: setbg3vofs 9

        for (int x = 0; x < BG2D_W; x++) {
            uint8 rgb[3] = { 0, 0, 0 };   // pal0palette forced to black

            // --- BG2 (CP backdrop, 4bpp) ---
            {
                int me = ((by2 / 8) * 32 + (x / 8)) * 2;
                uint16 e = (uint16)(cp_scr[me] | (cp_scr[me + 1] << 8));
                int tile = e & 0x3FF;
                int pal  = (e >> 10) & 7;
                int r = by2 & 7, c = x & 7;
                if (e & 0x8000) r = 7 - r;
                if (e & 0x4000) c = 7 - c;
                if (tile < n_cp) {
                    uint8 v = cp_px[tile * 64 + r * 8 + c];
                    if (v) cgram_color(col, pal * 16 + v, rgb);
                }
            }

            // --- BG3 (TI-3 logo, 2bpp) over the top ---
            {
                int me = ((by3 / 8) * 32 + (x / 8)) * 2;
                uint16 e = (uint16)(ti_scr[me] | (ti_scr[me + 1] << 8));
                int tile = e & 0x3FF;
                int pal  = (e >> 10) & 7;
                int r = by3 & 7, c = x & 7;
                if (e & 0x8000) r = 7 - r;
                if (e & 0x4000) c = 7 - c;
                if (tile < n_ti) {
                    uint8 v = ti_px[tile * 64 + r * 8 + c];
                    if (v) cgram_color(col, pal * 4 + v, rgb);
                }
            }

            uint8 *px = out_row + (size_t)x * 4;
            px[0] = rgb[0]; px[1] = rgb[1]; px[2] = rgb[2]; px[3] = 255;
        }
    }

    s_title_tex = upload_rgba(rgba, BG2D_W, BG2D_H, GL_CLAMP_TO_EDGE);

    free(rgba);
    free(ti_px); free(cp_px);
    free(ti_cgx); free(ti_scr); free(cp_cgx); free(cp_scr); free(col);

    printf("Bg2d: title screen composed (%d BG3 tiles, %d BG2 tiles)\n", n_ti, n_cp);
}

// ---------------------------------------------------------------------------
// Init / shutdown
// ---------------------------------------------------------------------------
void Bg2d_Init(void) {
    build_title_texture();
    memset(s_def_tex, 0, sizeof(s_def_tex));
    memset(s_def_tried, 0, sizeof(s_def_tried));
    memset(s_def_map_w, 0, sizeof(s_def_map_w));
    memset(s_def_map_h, 0, sizeof(s_def_map_h));

    // Dynamic quad (pos.xy + uv.xy)
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

void Bg2d_Shutdown(void) {
    if (s_title_tex) { glDeleteTextures(1, &s_title_tex); s_title_tex = 0; }
    for (int i = 0; i < NUM_BG_DEFS; i++) {
        if (s_def_tex[i]) { glDeleteTextures(1, &s_def_tex[i]); s_def_tex[i] = 0; }
        s_def_tried[i] = false;
        s_def_map_w[i] = 0;
        s_def_map_h[i] = 0;
    }
    if (s_vao) { glDeleteVertexArrays(1, &s_vao); s_vao = 0; }
    if (s_vbo) { glDeleteBuffers(1, &s_vbo); s_vbo = 0; }
}

bool Bg2d_HasTitle(void) {
    return s_title_tex != 0;
}

// ---------------------------------------------------------------------------
// Drawing helpers (screen-space, hud shader)
// ---------------------------------------------------------------------------
static void set_ortho(int w, int h) {
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
}

static void draw_quad_uv(float x, float y, float w, float h,
                         float u0, float v0, float u1, float v1) {
    float verts[] = {
        x,     y,     u0, v0,
        x + w, y,     u1, v0,
        x + w, y + h, u1, v1,
        x,     y + h, u0, v1,
    };
    glBindBuffer(GL_ARRAY_BUFFER, s_vbo);
    glBufferSubData(GL_ARRAY_BUFFER, 0, sizeof(verts), verts);
    glDrawArrays(GL_TRIANGLE_FAN, 0, 4);
}

static void draw_quad(float x, float y, float w, float h) {
    draw_quad_uv(x, y, w, h, 0.0f, 0.0f, 1.0f, 1.0f);
}

// Starfield-ish dark gradient fallback (deterministic star placement)
static void draw_fallback_backdrop(int w, int h) {
    GLint loc_tex = glGetUniformLocation(g_hud_shader, "uUseTexture");
    glUniform1i(loc_tex, 0);

    // Vertical gradient: deep space black at top -> dark blue near bottom
    const int bands = 8;
    for (int i = 0; i < bands; i++) {
        float t = (float)i / (float)(bands - 1);   // 0 bottom .. 1 top
        GlBackend_SetVec4(g_hud_shader, "uColor",
                          0.01f + 0.03f * (1.0f - t),
                          0.01f + 0.03f * (1.0f - t),
                          0.05f + 0.10f * (1.0f - t),
                          1.0f);
        float band_h = (float)h / (float)bands;
        draw_quad(0.0f, band_h * (float)i, (float)w, band_h + 1.0f);
    }

    // Stars: deterministic pseudo-random spread
    GlBackend_SetVec4(g_hud_shader, "uColor", 0.85f, 0.85f, 0.95f, 1.0f);
    uint32 seed = 0x1234567u;
    float sx = (float)w / 256.0f;
    float sy = (float)h / 224.0f;
    for (int i = 0; i < 64; i++) {
        seed = seed * 1664525u + 1013904223u;
        int px = (int)((seed >> 8) & 0xFF);
        seed = seed * 1664525u + 1013904223u;
        int py = (int)((seed >> 8) % 224u);
        float size = (i & 7) == 0 ? 2.0f : 1.0f;
        draw_quad((float)px * sx, (float)py * sy, size * sx, size * sy);
    }
}

static void draw_layer_texture(GLuint tex, int w, int h,
                               float u0, float v0, float u1, float v1) {
    if (!tex) {
        draw_fallback_backdrop(w, h);
        return;
    }
    GLint loc_tex = glGetUniformLocation(g_hud_shader, "uUseTexture");
    glUniform1i(loc_tex, 1);
    GLint samp_loc = glGetUniformLocation(g_hud_shader, "uTexture");
    if (samp_loc >= 0) glUniform1i(samp_loc, 0);
    GlBackend_SetVec4(g_hud_shader, "uColor", 1.0f, 1.0f, 1.0f, 1.0f);
    glActiveTexture(GL_TEXTURE0);
    glBindTexture(GL_TEXTURE_2D, tex);
    draw_quad_uv(0.0f, 0.0f, (float)w, (float)h, u0, v0, u1, v1);
    glBindTexture(GL_TEXTURE_2D, 0);
    glUniform1i(loc_tex, 0);
}

// ---------------------------------------------------------------------------
// SNES BG2 scroll coupling (GSTRATS.ASM calcbgscroll_l, run every frame)
// ---------------------------------------------------------------------------
// Computes the UV window into a sky (von/hon) tilemap texture.
// with_camera=false draws the base scroll only (non-gameplay states, e.g.
// the ending credits, where the original zeroes outvx/outvy anyway).
static void sky_uv_window(int idx, bool with_camera,
                          float *u0, float *v0, float *u1, float *v1) {
    const BgDef *def = &s_bg_defs[idx];
    float mw = (float)s_def_map_w[idx];
    float mh = (float)s_def_map_h[idx];
    float vofs = (float)def->vofs;      // bg2Yscroll base (BGS.ASM)
    float hofs = (float)g_bg2Xscroll;

    if (with_camera) {
        int32 camx_fp = 0;
        int16 rx16 = 0, ry16 = 0;
        Transform_GetRenderCamera(&camx_fp, NULL, NULL, &rx16, &ry16, NULL);
        // SNES 0-255 angle units, signed (render camera quantizes to whole
        // units, so BG and 3D view move in lockstep).
        int rx = rx16 & 0xFF; if (rx >= 128) rx -= 256;   // pitch, + = up
        int ry = ry16 & 0xFF; if (ry >= 128) ry -= 256;   // yaw,   + = right

        // Vertical: the SNES adds -(outvx>>6 + outvx>>7) = -6 px per pitch
        // unit to bg2Yscroll each frame, clamped to [-56, 232] unless
        // nomaxbg2Yscroll (launch/landing look-downs). 6 px/unit is the
        // horizon-lock gain of the SNES projection (~244 px focal); use the
        // port projection's real focal length so the painted horizon sits
        // exactly on the 3D ground plane's vanishing line, which is
        // focal*tan(pitch) below screen centre when pitched up.
        float focal = Transform_GetProjection()[5] * ((float)BG2D_H * 0.5f);
        float vdelta = -focal * tanf((float)rx * (3.14159265f / 128.0f));
        if (!g_nomaxbg2Yscroll) {
            if (vdelta > 232.0f) vdelta = 232.0f;
            if (vdelta < -56.0f) vdelta = -56.0f;
        }
        vofs += vdelta;

        // Horizontal: m_scrollxoff = bg2Xscroll + (outvy - player_turnrot)>>5
        // = 8 px per yaw unit (the render camera yaw already folds in
        // player_turnrot), plus the `hofmode rotate` HDMA base of
        // worldx>>3 (TRANS.ASM rotplanet -> MARIO/MHOFS.MC mrotplanet).
        hofs += (float)ry * 8.0f + (float)(camx_fp >> 16) / 8.0f;

        // TEMPORARY (SF_BG_DEBUG=1): quantitative horizon-lock check.
        // Projects a y=0 world point far ahead through the real
        // view/projection and prints its screen row (224-space) next to
        // the BG scroll, so a frame dump can confirm the painted horizon
        // sits on the 3D ground plane's vanishing line.
        {
            static int s_dbg = -1;
            if (s_dbg < 0) s_dbg = getenv("SF_BG_DEBUG") ? 1 : 0;
            if (s_dbg) {
                static unsigned s_n = 0;
                if ((s_n++ % 30) == 0) {
                    int32 cxp, cyp, czp;
                    Transform_GetRenderCamera(&cxp, &cyp, &czp, NULL, NULL, NULL);
                    const float *V = Transform_GetView();
                    const float *P = Transform_GetProjection();
                    // GL-world point on the SNES y=0 ground plane, far ahead
                    float wx = (float)cxp / 65536.0f;
                    float wy = 0.0f;
                    float wz = (float)czp / 65536.0f + 100000.0f;
                    float vx = V[0]*wx + V[4]*wy + V[8]*wz  + V[12];
                    float vy = V[1]*wx + V[5]*wy + V[9]*wz  + V[13];
                    float vz = V[2]*wx + V[6]*wy + V[10]*wz + V[14];
                    float cy = P[1]*vx + P[5]*vy + P[9]*vz;
                    float cw = P[3]*vx + P[7]*vy + P[11]*vz;
                    float row224 = (1.0f - cy / cw) * 0.5f * 224.0f;
                    printf("Bg2dDbg: rx=%d ry=%d vofs=%.1f hofs=%.1f "
                           "y0_vanish_row224=%.1f painted_row224(m)=m-%.1f\n",
                           rx, ry, vofs, hofs, row224, vofs);
                }
            }
        }
    }

    // Texture rows were flipped at compose time (GL row 0 = map bottom), so
    // map row m sits at v = (mh - m)/mh; screen top shows map row vofs and
    // the window wraps via GL_REPEAT like the SNES tilemap.
    *u0 = hofs / mw;
    *u1 = (hofs + (float)BG2D_W) / mw;
    *v1 = (mh - vofs) / mh;                     // quad top
    *v0 = (mh - vofs - (float)BG2D_H) / mh;     // quad bottom
}

// ---------------------------------------------------------------------------
// Per-frame background pass
// ---------------------------------------------------------------------------
void Bg2d_Render(int screen_width, int screen_height) {
    bool bg_active = (g_bgflags & BGF_BG) != 0;
    bool draw = false;
    bool couple = false;    // apply the per-frame camera scroll coupling
    int  idx = -1;          // s_bg_defs index (-1: title/fallback)
    GLuint tex = 0;   // 0 -> fallback backdrop

    if (g_game_state == GAME_STATE_TITLE) {
        // Title map's setbg opcode selects BG_TITLE; draw the logo layer
        // even if the map script hasn't reached setbg yet.
        draw = true;
        tex = s_title_tex;
    } else if (g_game_state == GAME_STATE_PLANET_SELECT ||
               g_game_state == GAME_STATE_BRIEFING) {
        // PLANETS.ASM map screen backdrop (route frames, sector labels).
        draw = true;
        idx = layer_index_for_id(BG2D_ID_MAP);
    } else if (g_game_state == GAME_STATE_CONTINUE) {
        // bg_cont_1 controller screen backdrop.
        draw = true;
        idx = layer_index_for_id(42);
    } else if (g_game_state == GAME_STATE_ENDING) {
        draw = true;
        idx = layer_index_for_id((uint8)(g_currentbg & 63u));
    } else if (bg_active || g_game_state == GAME_STATE_PLAYING) {
        // BGF_BG is transient (cleared by Bgs_Update), so also key off the
        // playing state; g_currentbg holds the last setbg operand. Because
        // most ported levels don't run their setbg opcodes yet, g_currentbg
        // can be stale from the title map: snapshot it at map load and use
        // the per-map default until the map issues its own setbg.
        static uint32 s_prev_map = 0xFFFFFFFFu;
        static uint16 s_bg_at_map_start = 0;
        if (g_newmap != s_prev_map) {
            s_prev_map = g_newmap;
            s_bg_at_map_start = g_currentbg;
        }

        draw = true;
        uint32 id = (uint32)(g_currentbg & 63u);
        if (g_currentbg == s_bg_at_map_start) {
            // No setbg from this map yet -> level's opening background.
            for (int i = 0; i < (int)(sizeof(s_map_default_bg) / sizeof(s_map_default_bg[0])); i++) {
                if (s_map_default_bg[i].map_id == g_newmap) {
                    id = s_map_default_bg[i].bg_id;
                    break;
                }
            }
        }

        if (id == BG2D_ID_TITLE) {
            tex = s_title_tex;
        } else {
            idx = layer_index_for_id((uint8)id);
            couple = true;   // gameplay: slave sky layers to the camera
            if ((idx < 0 || !s_def_tex[idx]) &&
                !(s_warned_bgs & ((uint64)1u << id))) {
                s_warned_bgs |= ((uint64)1u << id);
                printf("Bg2d: no layer data for bg id %u, using fallback backdrop\n",
                       (unsigned)id);
            }
        }
    }

    if (!draw) return;

    if (idx >= 0) tex = s_def_tex[idx];

    // Sky (von/hon) layers are full wrapping tilemaps: window into them.
    float u0 = 0.0f, v0 = 0.0f, u1 = 1.0f, v1 = 1.0f;
    if (idx >= 0 && tex && s_def_map_w[idx] > 0) {
        sky_uv_window(idx, couple, &u0, &v0, &u1, &v1);
    }

    glDisable(GL_DEPTH_TEST);
    glDepthMask(GL_FALSE);
    glDisable(GL_BLEND);

    glUseProgram(g_hud_shader);
    set_ortho(screen_width, screen_height);
    glBindVertexArray(s_vao);

    draw_layer_texture(tex, screen_width, screen_height, u0, v0, u1, v1);

    glBindVertexArray(0);
    glDepthMask(GL_TRUE);
    glEnable(GL_DEPTH_TEST);
}
