// In-game message font (MOJI_2.fon).
//
// Format (verified against reference/ultrastarfox/SF/INC/MOJI2.INC and the
// raw data): 80 glyphs, each 16x16 pixels, 1bpp, 2 bytes per row stored
// little-endian with bit 15 = leftmost pixel.  Charmap is 1-based in the
// ASM ('A' = $01); the file itself starts at glyph 'A'.
//   file  0-25 : A-Z
//   file 26    : '-'
//   file 27-36 : 0-9
//   file 37    : '!'
//   file 38    : '?'
//   file 39    : ' '
//   file 40-47 : fancy "STAR FOX" logo letters
//   file 64    : '%'

#include "font.h"
#include "gl_backend.h"
#include <glad/glad.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

#define FONT_GLYPH_PX   16                    /* native glyph size */
#define FONT_NUM_GLYPHS 80
#define FONT_ATLAS_COLS 16
#define FONT_ATLAS_ROWS 5
#define FONT_ATLAS_W (FONT_ATLAS_COLS * FONT_GLYPH_PX)  /* 256 */
#define FONT_ATLAS_H (FONT_ATLAS_ROWS * FONT_GLYPH_PX)  /* 80  */

/* Advance/draw size in SNES units (256x224 reference).  The native glyphs
 * are 16x16 but we draw them in an 8x8 cell to keep HUD layouts compact. */
#define FONT_CELL 8

static GLuint s_font_texture = 0;
static GLuint s_font_vao = 0;
static GLuint s_font_vbo = 0;
static int s_screen_w = 800;
static int s_screen_h = 600;
static bool s_initialized = false;

/* ASCII to glyph index translation (file-relative, 0-based) */
static uint8 s_ascii_to_glyph[256];

#define GLYPH_SPACE 39

static void init_translation_table(void) {
    memset(s_ascii_to_glyph, GLYPH_SPACE, sizeof(s_ascii_to_glyph));
    for (int i = 'A'; i <= 'Z'; i++) s_ascii_to_glyph[i] = (uint8)(i - 'A');
    for (int i = 'a'; i <= 'z'; i++) s_ascii_to_glyph[i] = (uint8)(i - 'a');
    for (int i = '0'; i <= '9'; i++) s_ascii_to_glyph[i] = (uint8)(27 + i - '0');
    s_ascii_to_glyph['-']  = 26;
    s_ascii_to_glyph['!']  = 37;
    s_ascii_to_glyph['?']  = 38;
    s_ascii_to_glyph[' ']  = GLYPH_SPACE;
    s_ascii_to_glyph['%']  = 64;
    /* No period/comma glyphs in this font; '-' is the closest dash mark. */
    s_ascii_to_glyph['.']  = GLYPH_SPACE;
    s_ascii_to_glyph[','] = GLYPH_SPACE;
}

void Font_Init(void) {
    init_translation_table();

    FILE *f = fopen("data/font/MOJI_2.fon", "rb");
    if (!f) {
        fprintf(stderr, "Font: cannot open data/font/MOJI_2.fon\n");
        return;
    }
    uint8 font_data[FONT_NUM_GLYPHS * 32];
    size_t nread = fread(font_data, 1, sizeof(font_data), f);
    fclose(f);
    if (nread < sizeof(font_data)) {
        fprintf(stderr, "Font: short read (%zu bytes)\n", nread);
        return;
    }

    /* Decode 1bpp 16x16 glyphs into an RGBA atlas (white on transparent) */
    uint8 *atlas = calloc((size_t)FONT_ATLAS_W * FONT_ATLAS_H * 4, 1);
    if (!atlas) return;

    for (int g = 0; g < FONT_NUM_GLYPHS; g++) {
        int gx = (g % FONT_ATLAS_COLS) * FONT_GLYPH_PX;
        int gy = (g / FONT_ATLAS_COLS) * FONT_GLYPH_PX;
        const uint8 *glyph = &font_data[g * 32]; /* 16 rows x 2 bytes */

        for (int row = 0; row < FONT_GLYPH_PX; row++) {
            uint16 bits = (uint16)(glyph[row * 2] | (glyph[row * 2 + 1] << 8));
            for (int col = 0; col < FONT_GLYPH_PX; col++) {
                if (!(bits & (uint16)(0x8000u >> col))) continue;
                int idx = ((gy + row) * FONT_ATLAS_W + gx + col) * 4;
                atlas[idx + 0] = 255;
                atlas[idx + 1] = 255;
                atlas[idx + 2] = 255;
                atlas[idx + 3] = 255;
            }
        }
    }

    glGenTextures(1, &s_font_texture);
    glBindTexture(GL_TEXTURE_2D, s_font_texture);
    glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, FONT_ATLAS_W, FONT_ATLAS_H, 0,
                 GL_RGBA, GL_UNSIGNED_BYTE, atlas);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glBindTexture(GL_TEXTURE_2D, 0);
    free(atlas);

    /* Single reusable quad (pos.xy + uv.xy), updated per glyph */
    glGenVertexArrays(1, &s_font_vao);
    glGenBuffers(1, &s_font_vbo);
    glBindVertexArray(s_font_vao);
    glBindBuffer(GL_ARRAY_BUFFER, s_font_vbo);
    glBufferData(GL_ARRAY_BUFFER, 4 * 4 * sizeof(float), NULL, GL_DYNAMIC_DRAW);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void *)0);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void *)(2 * sizeof(float)));
    glEnableVertexAttribArray(1);
    glBindVertexArray(0);

    s_initialized = true;
    printf("Font: loaded %d glyphs (%dx%d atlas, 16x16 1bpp)\n",
           FONT_NUM_GLYPHS, FONT_ATLAS_W, FONT_ATLAS_H);
}

void Font_Shutdown(void) {
    if (s_font_texture) { glDeleteTextures(1, &s_font_texture); s_font_texture = 0; }
    if (s_font_vao) { glDeleteVertexArrays(1, &s_font_vao); s_font_vao = 0; }
    if (s_font_vbo) { glDeleteBuffers(1, &s_font_vbo); s_font_vbo = 0; }
    s_initialized = false;
}

unsigned int Font_DebugGetTexture(void) {
    return s_font_texture;
}

void Font_SetScreenSize(int w, int h) {
    s_screen_w = w;
    s_screen_h = h;
}

void Font_DrawString(int x, int y, const char *text, float r, float g, float b) {
    if (!s_initialized || !text || !s_font_texture) return;

    size_t len = strlen(text);
    if (len == 0) return;

    /* Scale: SNES reference is 256x224; scale by screen height */
    float scale = (float)s_screen_h / 224.0f;
    float gw = FONT_CELL * scale;
    float gh = FONT_CELL * scale;

    glUseProgram(g_hud_shader);

    float ortho[16];
    memset(ortho, 0, sizeof(ortho));
    ortho[0] = 2.0f / (float)s_screen_w;
    ortho[5] = 2.0f / (float)s_screen_h;
    ortho[10] = -1.0f;
    ortho[12] = -1.0f;
    ortho[13] = -1.0f;
    ortho[15] = 1.0f;
    GlBackend_SetMat4(g_hud_shader, "uProj", ortho);

    float model[16];
    memset(model, 0, sizeof(model));
    model[0] = model[5] = model[10] = model[15] = 1.0f;
    GlBackend_SetMat4(g_hud_shader, "uModel", model);

    GlBackend_SetVec4(g_hud_shader, "uColor", r, g, b, 1.0f);

    GLint tex_loc = glGetUniformLocation(g_hud_shader, "uUseTexture");
    if (tex_loc >= 0) glUniform1i(tex_loc, 1);
    GLint samp_loc = glGetUniformLocation(g_hud_shader, "uTexture");
    if (samp_loc >= 0) glUniform1i(samp_loc, 0);

    glActiveTexture(GL_TEXTURE0);
    glBindTexture(GL_TEXTURE_2D, s_font_texture);

    glBindVertexArray(s_font_vao);
    glBindBuffer(GL_ARRAY_BUFFER, s_font_vbo);

    float cx = (float)x * scale;
    float cy = (float)y * scale;

    for (size_t i = 0; i < len; i++) {
        uint8 glyph = s_ascii_to_glyph[(uint8)text[i]];
        if (glyph >= FONT_NUM_GLYPHS) glyph = GLYPH_SPACE;

        if (glyph != GLYPH_SPACE) {
            float u0 = (float)(glyph % FONT_ATLAS_COLS) * FONT_GLYPH_PX / (float)FONT_ATLAS_W;
            float v0 = (float)(glyph / FONT_ATLAS_COLS) * FONT_GLYPH_PX / (float)FONT_ATLAS_H;
            float u1 = u0 + (float)FONT_GLYPH_PX / (float)FONT_ATLAS_W;
            float v1 = v0 + (float)FONT_GLYPH_PX / (float)FONT_ATLAS_H;

            /* Atlas row 0 is the glyph top; it is uploaded at GL v=0
             * (texture bottom), so the quad's TOP edge samples v0. */
            float verts[] = {
                cx,      cy,      u0, v1,   /* bottom-left  */
                cx + gw, cy,      u1, v1,   /* bottom-right */
                cx + gw, cy + gh, u1, v0,   /* top-right    */
                cx,      cy + gh, u0, v0,   /* top-left     */
            };
            glBufferSubData(GL_ARRAY_BUFFER, 0, sizeof(verts), verts);
            glDrawArrays(GL_TRIANGLE_FAN, 0, 4);
        }

        cx += gw; /* fixed-width advance */
    }

    glBindVertexArray(0);
    if (tex_loc >= 0) glUniform1i(tex_loc, 0);
    glBindTexture(GL_TEXTURE_2D, 0);
}

void Font_DrawNumber(int x, int y, int value, int digits) {
    if (!s_initialized) return;
    char buf[16];
    if (digits > 15) digits = 15;
    /* Right-aligned decimal number */
    int len = 0;
    int v = value < 0 ? -value : value;
    do {
        buf[len++] = (char)('0' + (v % 10));
        v /= 10;
    } while (v > 0 && len < 15);
    while (len < digits) buf[len++] = ' ';
    /* Reverse */
    for (int i = 0; i < len / 2; i++) {
        char tmp = buf[i]; buf[i] = buf[len - 1 - i]; buf[len - 1 - i] = tmp;
    }
    buf[len] = '\0';
    Font_DrawString(x, y, buf, 1.0f, 1.0f, 1.0f);
}
