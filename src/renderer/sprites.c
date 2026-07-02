// SNES OBJ sprite decoder and renderer.
//
// Decodes 4bpp tile data from OBJ-1.CGX and palettes from OBJ-1.COL,
// builds an indexed texture atlas, and renders sprites as textured quads
// using the HUD shader's palette-lookup mode (uUseTexture == 2).

#include "sprites.h"
#include "gl_backend.h"
#include <glad/glad.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// ---------------------------------------------------------------------------
// Atlas layout: 16 tiles wide, ceil(134/16) = 9 rows = 128 x 72 pixels
// ---------------------------------------------------------------------------
#define ATLAS_COLS      16
#define ATLAS_ROWS      9
#define TILE_W          8
#define TILE_H          8
#define ATLAS_W         (ATLAS_COLS * TILE_W)   // 128
#define ATLAS_H         (ATLAS_ROWS * TILE_H)   // 72
#define MAX_TILES       (ATLAS_COLS * ATLAS_ROWS) // 144
#define NUM_PALETTES    8
#define COLORS_PER_PAL  16

// Max sprites queued per frame
#define MAX_SPRITE_QUEUE 256

// ---------------------------------------------------------------------------
// Radio portrait faces (FACE.CGX)
//
// CONTINUE.ASM `.displayface` + MDSPRITE.MC `mcopyface`: each portrait frame
// is 640 bytes — a 32x40 pixel 4bpp image stored as 4 columns of 5 SNES
// tiles (column-major, 160 bytes per column).  The Super FX copies it raw
// into the game bitmap, so the pixels index the in-game bitmap palette
// (CGRAM row 0 = NIGHT.COL row 0, the same palette shapes.c uses for
// polygon colors).  18 frames: 0-4 window iris opening animation,
// 5-16 per-character talk frames (2 each), 17 "anyone" silhouette.
// ---------------------------------------------------------------------------
#define FACE_FRAMES     18
#define FACE_W          32
#define FACE_H          40
#define FACE_FRAME_SIZE 640
#define FACE_ATLAS_W    (FACE_FRAMES * FACE_W)   // 576
#define FACE_ATLAS_H    FACE_H

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
static GLuint s_atlas_tex = 0;
static GLuint s_quad_vao  = 0;
static GLuint s_quad_vbo  = 0;
static int    s_num_tiles = 0;
static int    s_screen_w  = 1280;
static int    s_screen_h  = 720;

// Palette data: 8 palettes of 16 RGBA colors (floats)
static float s_palettes[NUM_PALETTES][COLORS_PER_PAL][4];

// In-game bitmap palette (NIGHT.COL row 0) — polygon/bitmap-layer colors.
// Used by the face portraits and the HUD meter boxes (mdrawbox colors).
static float  s_bitmap_palette[COLORS_PER_PAL][4];
static GLuint s_face_tex = 0;

// Draw queue
typedef struct {
    int   tile;
    int   x, y;       // SNES coords (256x224)
    int   palette;
    int   flags;
} SpriteCmd;

static SpriteCmd s_queue[MAX_SPRITE_QUEUE];
static int       s_queue_count = 0;

// ---------------------------------------------------------------------------
// SNES 4bpp tile decoder
// ---------------------------------------------------------------------------
static void decode_4bpp_tile(const uint8 *src, uint8 *dst8x8) {
    for (int row = 0; row < 8; row++) {
        uint8 bp0 = src[row * 2 + 0];
        uint8 bp1 = src[row * 2 + 1];
        uint8 bp2 = src[16 + row * 2 + 0];
        uint8 bp3 = src[16 + row * 2 + 1];
        for (int bit = 7; bit >= 0; bit--) {
            uint8 p = ((bp0 >> bit) & 1)
                    | (((bp1 >> bit) & 1) << 1)
                    | (((bp2 >> bit) & 1) << 2)
                    | (((bp3 >> bit) & 1) << 3);
            dst8x8[(7 - bit) + row * 8] = p;
        }
    }
}

// ---------------------------------------------------------------------------
// BGR555 palette decoder
// ---------------------------------------------------------------------------
static void decode_bgr555(const uint8 *src, float rgba[4]) {
    uint16 word = (uint16)(src[0] | (src[1] << 8));
    rgba[0] = (float)((word >>  0) & 0x1F) / 31.0f;
    rgba[1] = (float)((word >>  5) & 0x1F) / 31.0f;
    rgba[2] = (float)((word >> 10) & 0x1F) / 31.0f;
    rgba[3] = 1.0f;
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------
void Sprites_Init(void) {
    // ---- Load tile data from OBJ-1.CGX (raw 4bpp, no header) ----
    FILE *f = fopen("data/sprites/OBJ-1.CGX", "rb");
    if (!f) {
        fprintf(stderr, "Sprites: cannot open data/sprites/OBJ-1.CGX\n");
        return;
    }
    fseek(f, 0, SEEK_END);
    long tile_file_size = ftell(f);
    fseek(f, 0, SEEK_SET);

    s_num_tiles = (int)(tile_file_size / 32);
    if (s_num_tiles > MAX_TILES) s_num_tiles = MAX_TILES;

    uint8 *tile_rom = (uint8 *)malloc(tile_file_size);
    size_t rd = fread(tile_rom, 1, tile_file_size, f);
    (void)rd;
    fclose(f);

    // Build the atlas: each pixel stores 0-15 palette index.
    // Upload as GL_R8 so the shader reads it as float in [0, 1].
    // We scale index 0-15 → byte 0-255 in steps of 17 (255/15 = 17).
    uint8 *atlas = (uint8 *)calloc(ATLAS_W * ATLAS_H, 1);
    uint8 tile8x8[64];

    for (int t = 0; t < s_num_tiles; t++) {
        decode_4bpp_tile(tile_rom + t * 32, tile8x8);
        int col = t % ATLAS_COLS;
        int row = t / ATLAS_COLS;
        for (int py = 0; py < 8; py++) {
            for (int px = 0; px < 8; px++) {
                int ax = col * TILE_W + px;
                int ay = row * TILE_H + py;
                uint8 idx = tile8x8[px + py * 8];
                // Store index so shader can reconstruct: int(r * 255 + 0.5)
                atlas[ax + ay * ATLAS_W] = idx;
            }
        }
    }
    free(tile_rom);

    // Upload atlas as GL_R8
    glGenTextures(1, &s_atlas_tex);
    glBindTexture(GL_TEXTURE_2D, s_atlas_tex);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_R8, ATLAS_W, ATLAS_H, 0,
                 GL_RED, GL_UNSIGNED_BYTE, atlas);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glBindTexture(GL_TEXTURE_2D, 0);
    free(atlas);

    // ---- Load palettes from OBJ-1.COL ----
    // Sprite palettes are in rows 8-15 (bytes 256-511).
    f = fopen("data/sprites/OBJ-1.COL", "rb");
    if (!f) {
        fprintf(stderr, "Sprites: cannot open data/sprites/OBJ-1.COL\n");
    } else {
        uint8 col_data[1024];
        size_t rd2 = fread(col_data, 1, sizeof(col_data), f);
        (void)rd2;
        fclose(f);

        for (int pal = 0; pal < NUM_PALETTES; pal++) {
            int row_off = (8 + pal) * 32;  // sprite palettes at rows 8-15
            for (int c = 0; c < COLORS_PER_PAL; c++) {
                decode_bgr555(col_data + row_off + c * 2, s_palettes[pal][c]);
            }
            // Color 0 of every palette is transparent
            s_palettes[pal][0][3] = 0.0f;
        }
    }

    // ---- Load in-game bitmap palette (NIGHT.COL row 0) ----
    // Fallback: leave palette black if the file is missing.
    memset(s_bitmap_palette, 0, sizeof(s_bitmap_palette));
    f = fopen("data/sprites/NIGHT.COL", "rb");
    if (!f) {
        fprintf(stderr, "Sprites: cannot open data/sprites/NIGHT.COL\n");
    } else {
        uint8 row0[32];
        size_t rd3 = fread(row0, 1, sizeof(row0), f);
        (void)rd3;
        fclose(f);
        for (int c = 0; c < COLORS_PER_PAL; c++) {
            decode_bgr555(row0 + c * 2, s_bitmap_palette[c]);
        }
    }

    // ---- Load portrait frames (FACE.CGX) into an RGBA atlas ----
    f = fopen("data/sprites/FACE.CGX", "rb");
    if (!f) {
        fprintf(stderr, "Sprites: cannot open data/sprites/FACE.CGX\n");
    } else {
        uint8 face_rom[FACE_FRAMES * FACE_FRAME_SIZE];
        size_t rd4 = fread(face_rom, 1, sizeof(face_rom), f);
        fclose(f);
        if (rd4 < sizeof(face_rom)) {
            fprintf(stderr, "Sprites: FACE.CGX short read (%zu bytes)\n", rd4);
        } else {
            uint8 *face_atlas =
                (uint8 *)calloc((size_t)FACE_ATLAS_W * FACE_ATLAS_H * 4, 1);
            uint8 tile8x8b[64];
            for (int fr = 0; fr < FACE_FRAMES; fr++) {
                // Frame layout: 4 columns of 5 tiles, column-major
                // (mcopyface copies 160 bytes per 8px-wide column).
                for (int col4 = 0; col4 < 4; col4++) {
                    for (int trow = 0; trow < 5; trow++) {
                        const uint8 *src = face_rom + fr * FACE_FRAME_SIZE +
                                           col4 * 160 + trow * 32;
                        decode_4bpp_tile(src, tile8x8b);
                        for (int py = 0; py < 8; py++) {
                            for (int px = 0; px < 8; px++) {
                                uint8 idx = tile8x8b[px + py * 8];
                                int ax = fr * FACE_W + col4 * 8 + px;
                                int ay = trow * 8 + py;
                                uint8 *dst =
                                    face_atlas +
                                    ((size_t)ay * FACE_ATLAS_W + ax) * 4;
                                // Color 0 = transparent (3D shows through)
                                dst[0] = (uint8)(s_bitmap_palette[idx][0] * 255.0f);
                                dst[1] = (uint8)(s_bitmap_palette[idx][1] * 255.0f);
                                dst[2] = (uint8)(s_bitmap_palette[idx][2] * 255.0f);
                                dst[3] = idx ? 255 : 0;
                            }
                        }
                    }
                }
            }
            glGenTextures(1, &s_face_tex);
            glBindTexture(GL_TEXTURE_2D, s_face_tex);
            glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, FACE_ATLAS_W, FACE_ATLAS_H,
                         0, GL_RGBA, GL_UNSIGNED_BYTE, face_atlas);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            glBindTexture(GL_TEXTURE_2D, 0);
            free(face_atlas);
        }
    }

    // ---- Create unit quad VAO (shared with HUD) ----
    float quad[] = {
        0.0f, 0.0f,  0.0f, 0.0f,
        1.0f, 0.0f,  1.0f, 0.0f,
        1.0f, 1.0f,  1.0f, 1.0f,
        0.0f, 1.0f,  0.0f, 1.0f,
    };
    glGenVertexArrays(1, &s_quad_vao);
    glGenBuffers(1, &s_quad_vbo);
    glBindVertexArray(s_quad_vao);
    glBindBuffer(GL_ARRAY_BUFFER, s_quad_vbo);
    glBufferData(GL_ARRAY_BUFFER, sizeof(quad), quad, GL_DYNAMIC_DRAW);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void *)0);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void *)(2 * sizeof(float)));
    glEnableVertexAttribArray(1);
    glBindVertexArray(0);

    printf("Sprites: loaded %d tiles, atlas %dx%d\n", s_num_tiles, ATLAS_W, ATLAS_H);
}

void Sprites_Shutdown(void) {
    if (s_atlas_tex) { glDeleteTextures(1, &s_atlas_tex); s_atlas_tex = 0; }
    if (s_face_tex) { glDeleteTextures(1, &s_face_tex); s_face_tex = 0; }
    if (s_quad_vao) { glDeleteVertexArrays(1, &s_quad_vao); s_quad_vao = 0; }
    if (s_quad_vbo) { glDeleteBuffers(1, &s_quad_vbo); s_quad_vbo = 0; }
}

void Sprites_SetScreenSize(int w, int h) {
    s_screen_w = w;
    s_screen_h = h;
}

// ---------------------------------------------------------------------------
// Queue an 8x8 sprite for rendering this frame.
// Coordinates are in SNES screen space (256x224).
// ---------------------------------------------------------------------------
void Sprites_Draw8(int tile, int x, int y, int palette, int flags) {
    if (s_queue_count >= MAX_SPRITE_QUEUE) return;
    if (tile < 0 || tile >= s_num_tiles) return;
    SpriteCmd *cmd = &s_queue[s_queue_count++];
    cmd->tile = tile;
    cmd->x = x;
    cmd->y = y;
    cmd->palette = palette;
    cmd->flags = flags;
}

// ---------------------------------------------------------------------------
// Flush the sprite queue — render all queued sprites.
// Called once per frame from Hud_Render.
// ---------------------------------------------------------------------------
void Sprites_RenderHUD(void) {
    if (s_queue_count == 0 || !s_atlas_tex) {
        s_queue_count = 0;
        return;
    }

    glUseProgram(g_hud_shader);

    // Orthographic projection: origin bottom-left, size = screen pixels
    float sw = (float)s_screen_w;
    float sh = (float)s_screen_h;
    float ortho[16];
    memset(ortho, 0, sizeof(ortho));
    ortho[0]  =  2.0f / sw;
    ortho[5]  =  2.0f / sh;
    ortho[10] = -1.0f;
    ortho[12] = -1.0f;
    ortho[13] = -1.0f;
    ortho[15] =  1.0f;
    GlBackend_SetMat4(g_hud_shader, "uProj", ortho);

    // Vertices are in screen space; uModel must be identity (other HUD
    // draws leave scale matrices behind).
    float model[16];
    memset(model, 0, sizeof(model));
    model[0] = model[5] = model[10] = model[15] = 1.0f;
    GlBackend_SetMat4(g_hud_shader, "uModel", model);

    // Scale from SNES 256x224 to actual screen
    float sx = sw / 256.0f;
    float sy = sh / 224.0f;

    // Palette-indexed texture mode
    GLint loc_tex = glGetUniformLocation(g_hud_shader, "uUseTexture");
    glUniform1i(loc_tex, 2);

    glActiveTexture(GL_TEXTURE0);
    glBindTexture(GL_TEXTURE_2D, s_atlas_tex);
    glUniform1i(glGetUniformLocation(g_hud_shader, "uTexture"), 0);

    glBindVertexArray(s_quad_vao);

    int cur_pal = -1;

    for (int i = 0; i < s_queue_count; i++) {
        SpriteCmd *cmd = &s_queue[i];

        // Upload palette if changed
        if (cmd->palette != cur_pal) {
            cur_pal = cmd->palette;
            int pi = cur_pal;
            if (pi < 0 || pi >= NUM_PALETTES) pi = 0;
            GLint pal_loc = glGetUniformLocation(g_hud_shader, "uPalette");
            if (pal_loc >= 0) {
                glUniform4fv(pal_loc, COLORS_PER_PAL, &s_palettes[pi][0][0]);
            }
        }

        // Compute tile UV in atlas
        int acol = cmd->tile % ATLAS_COLS;
        int arow = cmd->tile / ATLAS_COLS;
        float u0 = (float)(acol * TILE_W) / (float)ATLAS_W;
        float v0 = (float)(arow * TILE_H) / (float)ATLAS_H;
        float u1 = (float)(acol * TILE_W + TILE_W) / (float)ATLAS_W;
        float v1 = (float)(arow * TILE_H + TILE_H) / (float)ATLAS_H;

        // Apply flips
        if (cmd->flags & SPR_HFLIP) { float t = u0; u0 = u1; u1 = t; }
        if (cmd->flags & SPR_VFLIP) { float t = v0; v0 = v1; v1 = t; }

        // Build screen-space quad (SNES Y=0 at top → OpenGL Y=0 at bottom)
        float px = (float)cmd->x * sx;
        float py = sh - (float)(cmd->y + 8) * sy;  // flip Y, offset by tile height
        float pw = 8.0f * sx;
        float ph = 8.0f * sy;

        // Update quad VBO with UVs for this tile
        float verts[] = {
            px,      py,       u0, v1,
            px + pw, py,       u1, v1,
            px + pw, py + ph,  u1, v0,
            px,      py + ph,  u0, v0,
        };
        glBindBuffer(GL_ARRAY_BUFFER, s_quad_vbo);
        glBufferSubData(GL_ARRAY_BUFFER, 0, sizeof(verts), verts);

        glDrawArrays(GL_TRIANGLE_FAN, 0, 4);
    }

    glBindVertexArray(0);
    glBindTexture(GL_TEXTURE_2D, 0);

    // Reset for next frame
    s_queue_count = 0;
}

// ---------------------------------------------------------------------------
// In-game bitmap palette lookup (NIGHT.COL row 0) for HUD meter colors.
// ---------------------------------------------------------------------------
void Sprites_GetBitmapColor(int index, float out[4]) {
    if (index < 0 || index >= COLORS_PER_PAL) index = 0;
    out[0] = s_bitmap_palette[index][0];
    out[1] = s_bitmap_palette[index][1];
    out[2] = s_bitmap_palette[index][2];
    out[3] = 1.0f;
}

// ---------------------------------------------------------------------------
// Draw a radio portrait frame (0-17) at SNES screen coordinates.
// Immediate draw — caller (Hud_Render) has blending enabled.
// ---------------------------------------------------------------------------
void Sprites_DrawFace(int frame, int x, int y) {
    if (!s_face_tex || frame < 0 || frame >= FACE_FRAMES) return;

    glUseProgram(g_hud_shader);

    float sw = (float)s_screen_w;
    float sh = (float)s_screen_h;
    float ortho[16];
    memset(ortho, 0, sizeof(ortho));
    ortho[0]  =  2.0f / sw;
    ortho[5]  =  2.0f / sh;
    ortho[10] = -1.0f;
    ortho[12] = -1.0f;
    ortho[13] = -1.0f;
    ortho[15] =  1.0f;
    GlBackend_SetMat4(g_hud_shader, "uProj", ortho);

    float model[16];
    memset(model, 0, sizeof(model));
    model[0] = model[5] = model[10] = model[15] = 1.0f;
    GlBackend_SetMat4(g_hud_shader, "uModel", model);

    GlBackend_SetVec4(g_hud_shader, "uColor", 1.0f, 1.0f, 1.0f, 1.0f);
    glUniform1i(glGetUniformLocation(g_hud_shader, "uUseTexture"), 1);
    glActiveTexture(GL_TEXTURE0);
    glBindTexture(GL_TEXTURE_2D, s_face_tex);
    glUniform1i(glGetUniformLocation(g_hud_shader, "uTexture"), 0);

    // SNES 256x224 -> screen
    float sx = sw / 256.0f;
    float sy = sh / 224.0f;
    float px = (float)x * sx;
    float py = sh - (float)(y + FACE_H) * sy;
    float pw = (float)FACE_W * sx;
    float ph = (float)FACE_H * sy;

    // Atlas row 0 = portrait top (uploaded at GL v=0), so top edge samples v0.
    float u0 = (float)(frame * FACE_W) / (float)FACE_ATLAS_W;
    float u1 = (float)((frame + 1) * FACE_W) / (float)FACE_ATLAS_W;

    float verts[] = {
        px,      py,       u0, 1.0f,
        px + pw, py,       u1, 1.0f,
        px + pw, py + ph,  u1, 0.0f,
        px,      py + ph,  u0, 0.0f,
    };
    glBindVertexArray(s_quad_vao);
    glBindBuffer(GL_ARRAY_BUFFER, s_quad_vbo);
    glBufferSubData(GL_ARRAY_BUFFER, 0, sizeof(verts), verts);
    glDrawArrays(GL_TRIANGLE_FAN, 0, 4);

    glBindVertexArray(0);
    glBindTexture(GL_TEXTURE_2D, 0);
    glUniform1i(glGetUniformLocation(g_hud_shader, "uUseTexture"), 0);
}
