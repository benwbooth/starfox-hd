// In-game 2D overlay (flight HUD + radio messages).
//
// Faithful port of the original's two overlay paths:
//  - OAM sprites built by do_sprites_l (SPRITES.ASM:50): lives, SHIELD
//    label, bomb icons, crosshair, radar arrows, boss "ENEMY" tiles.
//  - Super FX bitmap draws from MDRAWLIS.MC/MTXTPRT.MC: shield meter
//    (mdamagemeter), boost gauge (mboostmeter), boss HP bar (mdrawbossHP),
//    teammate HP meter (mshowteammate2), radio text (mfprintstr) and the
//    portrait (mcopyface, CONTINUE.ASM friends_messages_l).
//
// Coordinates: OAM elements use SNES screen space (256x224, y down).
// Bitmap elements use Super FX bitmap space (256x192) which the SNES
// displays centered -> bitmap y + 16 = screen y (BITMAP_Y_OFS).
// Bitmap colors are palette indices into NIGHT.COL row 0
// (Sprites_GetBitmapColor), matching the polygon palette in shapes.c.

#include "hud.h"
#include "gl_backend.h"
#include "font.h"
#include "sprites.h"
#include "../game/boot.h"
#include "../game/game_vars.h"
#include "../game/sound.h"
#include "../game/strings.h"
#include "../variables.h"
#include <glad/glad.h>
#include <string.h>

// Vertical offset of the 256x192 Super FX bitmap within the 256x224 screen.
#define BITMAP_Y_OFS 16

// CONTINUE.ASM openingframes
#define MSG_OPENING_FRAMES 5

static GLuint s_quad_vao;
static GLuint s_quad_vbo;

// HUD state fed from the game tick (nmi.c calcmeters port)
static int s_shield_cur, s_shield_max;
static int s_boost_cur, s_boost_max;
static int s_boss_hp_cur, s_boss_hp_max;
static int s_lives;
static int s_bombs;

// calcmeters/do_sprites per-game-frame animation state
static int    s_boostanim = 40;      // m_boostanim (TRANS.ASM:613)
static int    s_sprframe = 0;        // arrow flash frame 0-3 (SPRITES.ASM:862)
static int    s_face_talk = 0;       // current talk-frame (5..17 or 4)
static uint16 s_last_gameframe = 0xFFFFu;
static uint32 s_rng = 0x12345678u;   // local RNG — do not perturb game RNG

static float s_screen_w, s_screen_h;

void Hud_Init(void) {
    // Create a unit quad for drawing bars
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
    glBufferData(GL_ARRAY_BUFFER, sizeof(quad), quad, GL_STATIC_DRAW);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void *)0);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void *)(2 * sizeof(float)));
    glEnableVertexAttribArray(1);
    glBindVertexArray(0);

    s_shield_cur = s_shield_max = 40;
    s_boost_cur = s_boost_max = 40;
    s_boss_hp_cur = s_boss_hp_max = 0;
    s_lives = 3;
    s_bombs = 3;
    s_boostanim = 40;
    s_sprframe = 0;
    s_last_gameframe = 0xFFFFu;
}

void Hud_Shutdown(void) {
    glDeleteVertexArrays(1, &s_quad_vao);
    glDeleteBuffers(1, &s_quad_vbo);
}

// Build orthographic projection for HUD (0,0 = bottom-left)
static void BuildOrtho(float *m, float w, float h) {
    memset(m, 0, sizeof(float) * 16);
    m[0] = 2.0f / w;
    m[5] = 2.0f / h;
    m[10] = -1.0f;
    m[12] = -1.0f;
    m[13] = -1.0f;
    m[15] = 1.0f;
}

// Solid rectangle in SNES screen space (256x224, y down), bitmap palette color.
static void SnesSolidRect(float x, float y, float w, float h, int color_idx) {
    float sx = s_screen_w / 256.0f;
    float sy = s_screen_h / 224.0f;
    float rgba[4];
    Sprites_GetBitmapColor(color_idx, rgba);

    float model[16];
    memset(model, 0, sizeof(model));
    model[0] = w * sx;
    model[5] = h * sy;
    model[10] = 1.0f;
    model[12] = x * sx;
    model[13] = s_screen_h - (y + h) * sy;
    model[15] = 1.0f;

    GlBackend_SetMat4(g_hud_shader, "uModel", model);
    GlBackend_SetVec4(g_hud_shader, "uColor", rgba[0], rgba[1], rgba[2], 1.0f);
    glBindVertexArray(s_quad_vao);
    glDrawArrays(GL_TRIANGLE_FAN, 0, 4);
    glBindVertexArray(0);
}

// 1px outline box (Super FX mdrawbox equivalent).
static void SnesOutlineBox(float x, float y, float w, float h, int color_idx) {
    SnesSolidRect(x, y, w, 1, color_idx);
    SnesSolidRect(x, y + h - 1, w, 1, color_idx);
    SnesSolidRect(x, y, 1, h, color_idx);
    SnesSolidRect(x + w - 1, y, 1, h, color_idx);
}

static uint32 HudRandom(void) {
    s_rng = s_rng * 1664525u + 1013904223u;
    return s_rng >> 16;
}

// Per-game-frame animation updates (original ran these once per game frame
// in calcmeters / do_arrows; the render loop runs faster, so gate on
// g_gameframe changes).
static void Hud_TickAnimations(void) {
    if (g_gameframe == s_last_gameframe) return;
    uint16 prev = s_last_gameframe;
    s_last_gameframe = g_gameframe;
    if (prev == 0xFFFFu) return;

    // mboostmeter drive (TRANS.ASM calcmeters:613-632): while boosting or
    // braking drain by 2 to 0, then clear boostcnt; refill by 1 up to 40.
    if (g_boostcnt != 0) {
        s_boostanim -= 2;
        if (s_boostanim < 0) {
            s_boostanim = 0;
            g_boostcnt = 0;
        }
    } else if (s_boostanim < 40) {
        s_boostanim++;
    }

    // Arrow flash frame (SPRITES.ASM do_arrows:861-876): advance every
    // other game frame through 4 frames; SE $8A on wrap while visible.
    if (g_gameframe & 1u) {
        s_sprframe++;
        if (s_sprframe >= 4) {
            s_sprframe = 0;
            if (g_arrows != 0) Sound_PlaySE(0x8A);
        }
    }

    // Radio talk-frame reroll (CONTINUE.ASM friends_messages_l "normal"):
    // rnd&31==0 -> frame 4 (blink), else mouth = rnd&1; mouth forced shut
    // for the last 30 frames of the message.
    if (g_msg_count1 > 0 && g_msg_count2 >= MSG_OPENING_FRAMES) {
        uint32 rnd = HudRandom() & 31u;
        if (rnd == 0u) {
            s_face_talk = 4;   // face_frame4
        } else {
            int mouth = (int)(rnd & 1u);
            if (g_msg_count1 < 30) mouth = 0;
            s_face_talk = MSG_OPENING_FRAMES +
                          ((g_whichfriend & 0x7F) << 1) + mouth;
        }
    }
}

// ---------------------------------------------------------------------------
// Radar arrows — port of artab (SPRITES.ASM:943-1027).
// Each direction has 4 flash frames of up to 4 sprites.
// ---------------------------------------------------------------------------
typedef struct {
    uint8 x, y, tile, flags;
} ArrowSpr;

// Frame layouts (positions decoded from $YYXX words).
#define AR_END 0xFF
static const ArrowSpr s_arrow_up[4][4] = {
    { {119,24,0x3E,0}, {127,24,0x3E,SPR_HFLIP}, {AR_END,0,0,0} },
    { {119,24,0x3E,0}, {127,24,0x3E,SPR_HFLIP},
      {119,33,0x3E,0}, {127,33,0x3E,SPR_HFLIP} },
    { {119,33,0x3E,0}, {127,33,0x3E,SPR_HFLIP}, {AR_END,0,0,0} },
    { {AR_END,0,0,0} },
};
static const ArrowSpr s_arrow_down[4][4] = {
    { {119,193,0x3E,SPR_VFLIP}, {127,193,0x3E,SPR_VFLIP|SPR_HFLIP}, {AR_END,0,0,0} },
    { {119,193,0x3E,SPR_VFLIP}, {127,193,0x3E,SPR_VFLIP|SPR_HFLIP},
      {119,184,0x3E,SPR_VFLIP}, {127,184,0x3E,SPR_VFLIP|SPR_HFLIP} },
    { {119,184,0x3E,SPR_VFLIP}, {127,184,0x3E,SPR_VFLIP|SPR_HFLIP}, {AR_END,0,0,0} },
    { {AR_END,0,0,0} },
};
static const ArrowSpr s_arrow_right[4][4] = {
    { {225,104,0x70,0}, {225,112,0x70,SPR_VFLIP}, {AR_END,0,0,0} },
    { {225,104,0x70,0}, {225,112,0x70,SPR_VFLIP},
      {216,104,0x70,0}, {216,112,0x70,SPR_VFLIP} },
    { {216,104,0x70,0}, {216,112,0x70,SPR_VFLIP}, {AR_END,0,0,0} },
    { {AR_END,0,0,0} },
};
static const ArrowSpr s_arrow_left[4][4] = {
    { {24,104,0x70,SPR_HFLIP}, {24,112,0x70,SPR_HFLIP|SPR_VFLIP}, {AR_END,0,0,0} },
    { {24,104,0x70,SPR_HFLIP}, {24,112,0x70,SPR_HFLIP|SPR_VFLIP},
      {33,104,0x70,SPR_HFLIP}, {33,112,0x70,SPR_HFLIP|SPR_VFLIP} },
    { {33,104,0x70,SPR_HFLIP}, {33,112,0x70,SPR_HFLIP|SPR_VFLIP}, {AR_END,0,0,0} },
    { {AR_END,0,0,0} },
};

static void DrawArrowSet(const ArrowSpr set[4][4]) {
    const ArrowSpr *frame = set[s_sprframe];
    for (int i = 0; i < 4; i++) {
        if (frame[i].x == AR_END && frame[i].tile == 0) break;
        Sprites_Draw8(frame[i].tile, frame[i].x, frame[i].y,
                      SPR_PAL_DEFAULT, frame[i].flags);
    }
}

// ---------------------------------------------------------------------------
// Radio message text: word-wrapped like mfprintstr (right clip at x=174,
// GRAPHICS.INC friendmsgrighttextclip), lines 16px apart.
// ---------------------------------------------------------------------------
#define MSG_TEXT_X       82
#define MSG_TEXT_Y       (169 + BITMAP_Y_OFS)
#define MSG_LINE_H       16
#define MSG_CHARS_LINE   11   // (174 - 82) / 8px cells
#define MSG_MAX_LINES    3

static int WrapMessage(const char *msg, char lines[MSG_MAX_LINES][MSG_CHARS_LINE + 1]) {
    int nlines = 0, col = 0;
    memset(lines, 0, MSG_MAX_LINES * (MSG_CHARS_LINE + 1));
    const char *p = msg;
    while (*p && nlines < MSG_MAX_LINES) {
        // Measure next word
        const char *w = p;
        int wlen = 0;
        while (w[wlen] && w[wlen] != ' ') wlen++;
        if (wlen == 0) { p++; continue; }  // skip spaces

        int need = wlen + (col > 0 ? 1 : 0);
        if (col + need > MSG_CHARS_LINE && col > 0) {
            nlines++;
            col = 0;
            if (nlines >= MSG_MAX_LINES) break;
            need = wlen;
        }
        if (col > 0) lines[nlines][col++] = ' ';
        for (int i = 0; i < wlen && col < MSG_CHARS_LINE; i++) {
            lines[nlines][col++] = w[i];
        }
        p = w + wlen;
    }
    if (col > 0) nlines++;
    return nlines;
}

// Font_DrawString uses a bottom-left origin scaled by screen HEIGHT on both
// axes (glyphs stay square); sprites/bitmap boxes scale x by screen WIDTH.
// Compensate x so the text lines up with the portrait in 256-wide space.
static void DrawMsgLine(int x, int y_top, const char *text, float r, float g, float b) {
    float fx = (float)x * (s_screen_w / 256.0f) / (s_screen_h / 224.0f);
    Font_DrawString((int)(fx + 0.5f), 224 - y_top - 8, text, r, g, b);
}

// ---------------------------------------------------------------------------
// Radio message block: portrait + text + teammate HP meter
// (CONTINUE.ASM friends_messages_l).  Not gated by g_meters.
// ---------------------------------------------------------------------------
static void DrawRadioMessage(void) {
    // friends_messages_l bails while the player is dying/dead.
    if (g_gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD)) return;
    if (g_msg_count1 == 0 && g_msg_count2 == 0) return;

    // --- Portrait (mcopyface at friendmugshotx=6 cols -> x=48, y=152) ---
    int frame;
    if (g_msg_count1 == 0 || g_msg_count2 < MSG_OPENING_FRAMES) {
        // Opening/closing window iris: frames 0-4
        frame = g_msg_count2;
        if (frame > 4) frame = 4;
    } else {
        frame = s_face_talk;
    }
    Sprites_DrawFace(frame, 48, 152 + BITMAP_Y_OFS);

    // Text + meter only once the window is open and the message is live.
    if (g_msg_count1 == 0 || g_msg_count2 < MSG_OPENING_FRAMES) return;

    const char *msg = Strings_GetActiveMessageText();
    if (msg) {
        // chkmeter (CONTINUE.ASM): long-message variants (bit 7), Andross,
        // or a visible teammate meter push the text up one line.
        int ty = MSG_TEXT_Y;
        if ((g_whichfriend & 0x80u) != 0u ||
            (g_whichfriend & 0x7Fu) == FRIEND_ANDROSS ||
            g_friends_meter != 0u) {
            ty -= MSG_LINE_H;
        }

        char lines[MSG_MAX_LINES][MSG_CHARS_LINE + 1];
        int nlines = WrapMessage(msg, lines);
        float shadow[4];
        Sprites_GetBitmapColor(9, shadow);  // msprintstr shadow colour 9
        for (int i = 0; i < nlines; i++) {
            DrawMsgLine(MSG_TEXT_X + 1, ty + i * MSG_LINE_H + 1, lines[i],
                        shadow[0], shadow[1], shadow[2]);
            DrawMsgLine(MSG_TEXT_X, ty + i * MSG_LINE_H, lines[i],
                        1.0f, 1.0f, 1.0f);
        }
    }
}

// Teammate HP meter (mshowteammate2, MTXTPRT.MC:568): 44x12 box colour 14
// at friendhpx/friendhpy = (82,177), fill = HP, height 8, colour 2.
static void DrawTeammateMeter(void) {
    if (g_gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD)) return;
    if (g_msg_count1 == 0 || g_msg_count2 < MSG_OPENING_FRAMES) return;
    if (g_friends_meter == 0u) return;

    int hp = (int)(g_friends_meter & 0x7Fu);
    SnesOutlineBox(82, 177 + BITMAP_Y_OFS, 44, 12, 14);
    if (hp > 0) {
        if (hp > 40) hp = 40;
        SnesSolidRect(84, 179 + BITMAP_Y_OFS, (float)hp, 8, 2);
    }
}

void Hud_Render(int screen_width, int screen_height) {
    static int s_was_playing = 0;

    // Gameplay HUD only applies while actually playing — not on
    // title/menu/planet-select screens.
    if (g_game_state != GAME_STATE_PLAYING) {
        s_was_playing = 0;
        return;
    }

    s_screen_w = (float)screen_width;
    s_screen_h = (float)screen_height;

    // MAIN.ASM:105 sets m_meters=1 in the stage-start sequence; that code
    // path is not ported yet (game.c), so mirror it here on entering
    // gameplay and on each stage advance.  Remove once the game side owns
    // this.
    {
        static uint16 s_last_stage = 0xFFFFu;
        if (!s_was_playing || g_stage != s_last_stage) {
            s_was_playing = 1;
            s_last_stage = g_stage;
            g_meters = 1u;
        }
    }

    Hud_TickAnimations();

    glDisable(GL_DEPTH_TEST);
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

    glUseProgram(g_hud_shader);

    float ortho[16];
    BuildOrtho(ortho, s_screen_w, s_screen_h);
    GlBackend_SetMat4(g_hud_shader, "uProj", ortho);
    glUniform1i(glGetUniformLocation(g_hud_shader, "uUseTexture"), 0);

    Sprites_SetScreenSize(screen_width, screen_height);
    Font_SetScreenSize(screen_width, screen_height);

    // do_sprites_l gate: m_meters set, screen not blacked out.
    bool hud_on = (g_meters != 0u) && (g_stayblack == -1);

    if (hud_on) {
        // --- Super FX bitmap meters (MDRAWLIS.MC), bitmap y + 16 ---

        // Shield meter (mdamagemeter): 40x8 box colour 13 at (8,176),
        // fill = player HP clamped to 36, 4px tall, colour 2.
        // (Colour 7 wireframe-shield variant needs `shieldup`, not ported.)
        {
            int fill = s_shield_cur;
            if (fill < 0) fill = 0;
            if (fill > 36) fill = 36;
            SnesOutlineBox(8, 176 + BITMAP_Y_OFS, 40, 8, 13);
            if (fill > 0) SnesSolidRect(10, 178 + BITMAP_Y_OFS, (float)fill, 4, 2);
        }

        // Boost/brake gauge (mboostmeter): mirror box at (176,176),
        // fill = m_boostanim clamped to 36, colour 6.
        {
            int fill = s_boostanim;
            if (fill > 36) fill = 36;
            SnesOutlineBox(176, 176 + BITMAP_Y_OFS, 40, 8, 13);
            if (fill > 0) SnesSolidRect(178, 178 + BITMAP_Y_OFS, (float)fill, 4, 6);
        }

        // Boss HP bar (mdrawbossHP): right-aligned at x=222 (gameNum_col*8-2),
        // bitmap y=2; frame colour 14, fill colour 2, fill height 2.
        if (s_boss_hp_max > 0) {
            int maxv = s_boss_hp_max & 0xFF;
            int curv = s_boss_hp_cur;
            if (maxv & 0x80) { maxv >>= 1; curv >>= 1; }
            int w = maxv + 4;
            float bx = (float)(222 - w);
            SnesOutlineBox(bx, 2 + BITMAP_Y_OFS, (float)w, 6, 14);
            if (curv > 0) {
                if (curv > maxv) curv = maxv;
                SnesSolidRect(bx + 2, 4 + BITMAP_Y_OFS, (float)curv, 2, 2);
            }
        }

        DrawTeammateMeter();

        // --- Portrait sits on the bitmap layer, under the OAM sprites ---
        DrawRadioMessage();

        // --- OAM sprites (SPRITES.ASM do_sprites_l order) ---

        // Nova bomb icons (do_spec_weap): tile $3C from (225,182)
        // stepping x -= 9 per bomb (bombIconPos/bombIconSpacing).
        for (int i = 0; i < s_bombs && i < 9; i++) {
            Sprites_Draw8(0x3C, 225 - i * 9, 182, SPR_PAL_DEFAULT, 0);
        }

        // Crosshair (do_crosshair): four tile-$61 corners around
        // (chx,chy)=(124,100), palette 4, cockpit view only.
        // (Aim offset arsebandX/Y not ported yet — reticle stays centered.)
        if (g_splayerflymode == SPFM_INSIDE) {
            Sprites_Draw8(0x61, 124 - 8, 100 - 8, SPR_PAL_CROSS, 0);
            Sprites_Draw8(0x61, 124 + 8, 100 - 8, SPR_PAL_CROSS, SPR_HFLIP);
            Sprites_Draw8(0x61, 124 - 8, 100 + 8, SPR_PAL_CROSS, SPR_VFLIP);
            Sprites_Draw8(0x61, 124 + 8, 100 + 8, SPR_PAL_CROSS,
                          SPR_HFLIP | SPR_VFLIP);
        }

        // Lives (do_lives): ship icon, "x", digit at (16/24/32, 17).
        {
            int lc = s_lives > 0 ? s_lives : 1;
            lc -= 1;
            if (lc > 9) lc = 9;
            Sprites_Draw8(0x3D, 16, 17, SPR_PAL_DEFAULT, 0);
            Sprites_Draw8(0x62, 24, 17, SPR_PAL_DEFAULT, 0);
            Sprites_Draw8(0x63 + lc, 32, 17, SPR_PAL_DEFAULT, 0);
        }

        // "SHIELD" label (do_shield): tiles $4B-$4E at (24..48, 183).
        Sprites_Draw8(0x4B, 24, 183, SPR_PAL_DEFAULT, 0);
        Sprites_Draw8(0x4C, 32, 183, SPR_PAL_DEFAULT, 0);
        Sprites_Draw8(0x4D, 40, 183, SPR_PAL_DEFAULT, 0);
        Sprites_Draw8(0x4E, 48, 183, SPR_PAL_DEFAULT, 0);

        // Boss "ENEMY" tiles (do_enemy): $71-$74 at (200-hp, 16).
        if (s_boss_hp_max > 0) {
            int v = s_boss_hp_max & 0xFF;
            if (v & 0x80) v >>= 1;
            for (int i = 0; i < 4; i++) {
                Sprites_Draw8(0x71 + i, 200 - v + i * 8, 16, SPR_PAL_DEFAULT, 0);
            }
        }

        // Radar arrows (do_arrows), flashing via s_sprframe.
        if (g_arrows & SPRAR_UP)    DrawArrowSet(s_arrow_up);
        if (g_arrows & SPRAR_DOWN)  DrawArrowSet(s_arrow_down);
        if (g_arrows & SPRAR_LEFT)  DrawArrowSet(s_arrow_left);
        if (g_arrows & SPRAR_RIGHT) DrawArrowSet(s_arrow_right);
    } else {
        // Radio traffic still runs with the meters hidden (friends_messages_l
        // is called from the main loop regardless of m_meters).
        DrawTeammateMeter();
        DrawRadioMessage();
    }

    Sprites_RenderHUD();

    glDisable(GL_BLEND);
    glEnable(GL_DEPTH_TEST);
}

void Hud_SetShield(int current, int max) {
    s_shield_cur = current;
    s_shield_max = max;
}

void Hud_SetBoost(int current, int max) {
    s_boost_cur = current;
    s_boost_max = max;
}

void Hud_SetBossHP(int current, int max) {
    s_boss_hp_cur = current;
    s_boss_hp_max = max;
}

void Hud_SetLives(int count) { s_lives = count; }
void Hud_SetBombs(int count) { s_bombs = count; }
