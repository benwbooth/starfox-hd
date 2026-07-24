//! In-game 2D overlay (flight HUD + radio messages).
//!
//! Port (C oracle): `src/renderer/hud.c` — OAM sprites from
//! `do_sprites_l` (SPRITES.ASM) and Super FX bitmap draws from
//! MDRAWLIS.MC/MTXTPRT.MC (meters, boss HP, radio text, portrait).
//!
//! Deviations from the C (documented):
//!  - `Sound_PlaySE(0x8A)` on arrow-flash wrap becomes a pending sound-effect
//!    queue the caller drains ([`Hud::take_pending_sounds`]).
//!  - The C clears `g_boostcnt` when the boost gauge drains; inputs here are
//!    read-only, so an internal latch treats the unchanged input value as
//!    cleared until the game writes a new one.
//!  - The C forces `g_meters = 1` on entering gameplay / stage advance
//!    (MAIN.ASM:105 stand-in); mirrored with an internal flag.

use crate::font::Font;
use crate::gpu::{Gpu, Vertex2, WHITE_TEX};
use crate::renderer::{FrameInputs, GameState};
use crate::sprites::{Sprites, SPR_HFLIP, SPR_PAL_CROSS, SPR_PAL_DEFAULT, SPR_VFLIP};

#[inline]
fn ortho(w: f32, h: f32) -> [f32; 16] {
    [
        2.0 / w,
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / h,
        0.0,
        0.0,
        0.0,
        0.0,
        -1.0,
        0.0,
        -1.0,
        -1.0,
        0.0,
        1.0,
    ]
}

/// Unit quad (pos.xy = uv.xy), transformed per draw by the model matrix.
const UNIT_QUAD: [Vertex2; 4] = [
    Vertex2 {
        pos: [0.0, 0.0],
        uv: [0.0, 0.0],
    },
    Vertex2 {
        pos: [1.0, 0.0],
        uv: [1.0, 0.0],
    },
    Vertex2 {
        pos: [1.0, 1.0],
        uv: [1.0, 1.0],
    },
    Vertex2 {
        pos: [0.0, 1.0],
        uv: [0.0, 1.0],
    },
];

/// Vertical offset of the 256x192 Super FX bitmap within the 256x224 screen.
const BITMAP_Y_OFS: i32 = 16;

/// CONTINUE.ASM openingframes
const MSG_OPENING_FRAMES: u8 = 5;

// variables.h flags used by the HUD (mirrors of the C defines).
pub const GF_PLAYERDYING: u8 = 2;
pub const GF_PLAYERDEAD: u8 = 64;
pub const SPRAR_UP: u8 = 1;
pub const SPRAR_DOWN: u8 = 2;
pub const SPRAR_LEFT: u8 = 4;
pub const SPRAR_RIGHT: u8 = 8;
pub const FRIEND_ANDROSS: u8 = 5;

// Radio message text layout (mfprintstr / friendmsgrighttextclip).
const MSG_TEXT_X: i32 = 82;
const MSG_TEXT_Y: i32 = 169 + BITMAP_Y_OFS;
const MSG_LINE_H: i32 = 16;
const MSG_CHARS_LINE: usize = 11; // (174 - 82) / 8px cells
const MSG_MAX_LINES: usize = 3;

// Radar arrow frame layouts — port of artab (SPRITES.ASM:943-1027).
#[derive(Clone, Copy)]
struct ArrowSpr {
    x: u8,
    y: u8,
    tile: u8,
    flags: u8,
}

const AR_END: u8 = 0xFF;

const fn a(x: u8, y: u8, tile: u8, flags: u8) -> ArrowSpr {
    ArrowSpr { x, y, tile, flags }
}
const AZ: ArrowSpr = a(AR_END, 0, 0, 0);

static ARROW_UP: [[ArrowSpr; 4]; 4] = [
    [a(119, 24, 0x3E, 0), a(127, 24, 0x3E, SPR_HFLIP), AZ, AZ],
    [
        a(119, 24, 0x3E, 0),
        a(127, 24, 0x3E, SPR_HFLIP),
        a(119, 33, 0x3E, 0),
        a(127, 33, 0x3E, SPR_HFLIP),
    ],
    [a(119, 33, 0x3E, 0), a(127, 33, 0x3E, SPR_HFLIP), AZ, AZ],
    [AZ, AZ, AZ, AZ],
];
static ARROW_DOWN: [[ArrowSpr; 4]; 4] = [
    [
        a(119, 193, 0x3E, SPR_VFLIP),
        a(127, 193, 0x3E, SPR_VFLIP | SPR_HFLIP),
        AZ,
        AZ,
    ],
    [
        a(119, 193, 0x3E, SPR_VFLIP),
        a(127, 193, 0x3E, SPR_VFLIP | SPR_HFLIP),
        a(119, 184, 0x3E, SPR_VFLIP),
        a(127, 184, 0x3E, SPR_VFLIP | SPR_HFLIP),
    ],
    [
        a(119, 184, 0x3E, SPR_VFLIP),
        a(127, 184, 0x3E, SPR_VFLIP | SPR_HFLIP),
        AZ,
        AZ,
    ],
    [AZ, AZ, AZ, AZ],
];
static ARROW_RIGHT: [[ArrowSpr; 4]; 4] = [
    [a(225, 104, 0x70, 0), a(225, 112, 0x70, SPR_VFLIP), AZ, AZ],
    [
        a(225, 104, 0x70, 0),
        a(225, 112, 0x70, SPR_VFLIP),
        a(216, 104, 0x70, 0),
        a(216, 112, 0x70, SPR_VFLIP),
    ],
    [a(216, 104, 0x70, 0), a(216, 112, 0x70, SPR_VFLIP), AZ, AZ],
    [AZ, AZ, AZ, AZ],
];
static ARROW_LEFT: [[ArrowSpr; 4]; 4] = [
    [
        a(24, 104, 0x70, SPR_HFLIP),
        a(24, 112, 0x70, SPR_HFLIP | SPR_VFLIP),
        AZ,
        AZ,
    ],
    [
        a(24, 104, 0x70, SPR_HFLIP),
        a(24, 112, 0x70, SPR_HFLIP | SPR_VFLIP),
        a(33, 104, 0x70, SPR_HFLIP),
        a(33, 112, 0x70, SPR_HFLIP | SPR_VFLIP),
    ],
    [
        a(33, 104, 0x70, SPR_HFLIP),
        a(33, 112, 0x70, SPR_HFLIP | SPR_VFLIP),
        AZ,
        AZ,
    ],
    [AZ, AZ, AZ, AZ],
];

/// Word-wrap like mfprintstr (right clip at x=174), lines 16px apart.
/// Mirror of `WrapMessage`.
pub fn wrap_message(msg: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let bytes = msg.as_bytes();
    let mut p = 0usize;
    while p < bytes.len() && lines.len() < MSG_MAX_LINES {
        // Measure next word
        let mut wlen = 0usize;
        while p + wlen < bytes.len() && bytes[p + wlen] != b' ' {
            wlen += 1;
        }
        if wlen == 0 {
            p += 1;
            continue; // skip spaces
        }

        let col = cur.len();
        let mut need = wlen + if col > 0 { 1 } else { 0 };
        if col + need > MSG_CHARS_LINE && col > 0 {
            lines.push(std::mem::take(&mut cur));
            if lines.len() >= MSG_MAX_LINES {
                break;
            }
            need = wlen;
        }
        let _ = need;
        if !cur.is_empty() {
            cur.push(' ');
        }
        for i in 0..wlen {
            if cur.len() >= MSG_CHARS_LINE {
                break;
            }
            cur.push(bytes[p + i] as char);
        }
        p += wlen;
    }
    if !cur.is_empty() && lines.len() < MSG_MAX_LINES {
        lines.push(cur);
    }
    lines
}

pub struct Hud {
    // calcmeters/do_sprites per-game-frame animation state
    boostanim: i32,      // m_boostanim (TRANS.ASM:613)
    sprframe: usize,     // arrow flash frame 0-3 (SPRITES.ASM:862)
    face_talk: i32,      // current talk-frame (5..17 or 4)
    last_gameframe: u32, // 0xFFFF_FFFF = unset
    rng: u32,            // local RNG — do not perturb game RNG

    screen_w: f32,
    screen_h: f32,

    // Read-only-input workarounds (see module docs).
    was_playing: bool,
    last_stage: u32,
    force_meters: bool,
    boostcnt_zeroed: bool,
    boostcnt_seen: u8,

    /// SNES sound-effect codes queued by HUD animations (SE $8A arrow beep).
    pub pending_sounds: Vec<u8>,
}

impl Default for Hud {
    fn default() -> Self {
        Hud {
            boostanim: 40,
            sprframe: 0,
            face_talk: 0,
            last_gameframe: 0xFFFF_FFFF,
            rng: 0x1234_5678,
            screen_w: 0.0,
            screen_h: 0.0,
            was_playing: false,
            last_stage: 0xFFFF_FFFF,
            force_meters: false,
            boostcnt_zeroed: false,
            boostcnt_seen: 0,
            pending_sounds: Vec::new(),
        }
    }
}

impl Hud {
    pub fn new(_gpu: &mut Gpu) -> Self {
        Self::default()
    }

    pub fn take_pending_sounds(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending_sounds)
    }

    fn random(&mut self) -> u32 {
        self.rng = self.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        self.rng >> 16
    }

    /// Solid rectangle in SNES screen space (256x224, y down), bitmap
    /// palette color. Mirror of `SnesSolidRect`.
    #[allow(clippy::too_many_arguments)]
    fn snes_solid_rect(
        &self,
        gpu: &mut Gpu,
        sprites: &Sprites,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color_idx: usize,
    ) {
        let sx = self.screen_w / 256.0;
        let sy = self.screen_h / 224.0;
        let rgba = sprites.bitmap_color(color_idx);

        let mut model = [0.0f32; 16];
        model[0] = w * sx;
        model[5] = h * sy;
        model[10] = 1.0;
        model[12] = x * sx;
        model[13] = self.screen_h - (y + h) * sy;
        model[15] = 1.0;

        let proj = ortho(self.screen_w, self.screen_h);
        gpu.push_overlay_fan(
            &UNIT_QUAD,
            &proj,
            &model,
            [rgba[0], rgba[1], rgba[2], 1.0],
            0,
            None,
            WHITE_TEX,
        );
    }

    /// 1px outline box (Super FX mdrawbox equivalent).
    #[allow(clippy::too_many_arguments)]
    fn snes_outline_box(
        &self,
        gpu: &mut Gpu,
        sprites: &Sprites,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color_idx: usize,
    ) {
        self.snes_solid_rect(gpu, sprites, x, y, w, 1.0, color_idx);
        self.snes_solid_rect(gpu, sprites, x, y + h - 1.0, w, 1.0, color_idx);
        self.snes_solid_rect(gpu, sprites, x, y, 1.0, h, color_idx);
        self.snes_solid_rect(gpu, sprites, x + w - 1.0, y, 1.0, h, color_idx);
    }

    /// Per-game-frame animation updates (mirror of `Hud_TickAnimations`).
    pub(crate) fn tick_animations(&mut self, inputs: &FrameInputs) {
        if inputs.gameframe as u32 == self.last_gameframe {
            return;
        }
        let prev = self.last_gameframe;
        self.last_gameframe = inputs.gameframe as u32;
        if prev == 0xFFFF_FFFF {
            return;
        }

        // Effective boostcnt: the C zeroes g_boostcnt when the gauge fully
        // drains; treat the unchanged input value as cleared.
        if inputs.boostcnt != self.boostcnt_seen {
            self.boostcnt_seen = inputs.boostcnt;
            self.boostcnt_zeroed = false;
        }
        let boostcnt = if self.boostcnt_zeroed {
            0
        } else {
            inputs.boostcnt
        };

        // mboostmeter drive (TRANS.ASM calcmeters:613-632).
        if boostcnt != 0 {
            self.boostanim -= 2;
            if self.boostanim < 0 {
                self.boostanim = 0;
                self.boostcnt_zeroed = true; // C: g_boostcnt = 0
            }
        } else if self.boostanim < 40 {
            self.boostanim += 1;
        }

        // Arrow flash frame (SPRITES.ASM do_arrows:861-876): advance every
        // other game frame through 4 frames; SE $8A on wrap while visible.
        if inputs.gameframe & 1 != 0 {
            self.sprframe += 1;
            if self.sprframe >= 4 {
                self.sprframe = 0;
                if inputs.arrows != 0 {
                    self.pending_sounds.push(0x8A);
                }
            }
        }

        // Radio talk-frame reroll (CONTINUE.ASM friends_messages_l "normal").
        if inputs.msg_count1 > 0 && inputs.msg_count2 >= MSG_OPENING_FRAMES {
            let rnd = self.random() & 31;
            if rnd == 0 {
                self.face_talk = 4; // face_frame4
            } else {
                let mut mouth = (rnd & 1) as i32;
                if inputs.msg_count1 < 30 {
                    mouth = 0;
                }
                self.face_talk =
                    MSG_OPENING_FRAMES as i32 + (((inputs.whichfriend & 0x7F) as i32) << 1) + mouth;
            }
        }
    }

    fn draw_arrow_set(&self, sprites: &mut Sprites, set: &[[ArrowSpr; 4]; 4]) {
        let frame = &set[self.sprframe];
        for spr in frame.iter() {
            if spr.x == AR_END && spr.tile == 0 {
                break;
            }
            sprites.draw8(
                spr.tile as i32,
                spr.x as i32,
                spr.y as i32,
                SPR_PAL_DEFAULT,
                spr.flags,
            );
        }
    }

    /// Font wrapper compensating x so text lines up with the portrait in
    /// 256-wide space (mirror of `DrawMsgLine`).
    #[allow(clippy::too_many_arguments)]
    fn draw_msg_line(
        &self,
        gpu: &mut Gpu,
        font: &Font,
        x: i32,
        y_top: i32,
        text: &str,
        r: f32,
        g: f32,
        b: f32,
    ) {
        let fx = x as f32 * (self.screen_w / 256.0) / (self.screen_h / 224.0);
        font.draw_string(gpu, (fx + 0.5) as i32, 224 - y_top - 8, text, r, g, b);
    }

    /// Radio message block: portrait + text (CONTINUE.ASM
    /// friends_messages_l). Mirror of `DrawRadioMessage`.
    fn draw_radio_message(
        &self,
        gpu: &mut Gpu,
        sprites: &Sprites,
        font: &Font,
        inputs: &FrameInputs,
    ) {
        // friends_messages_l bails while the player is dying/dead.
        if inputs.gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD) != 0 {
            return;
        }
        if inputs.msg_count1 == 0 && inputs.msg_count2 == 0 {
            return;
        }

        // --- Portrait (mcopyface at friendmugshotx=6 cols -> x=48, y=152) --
        let frame = if inputs.msg_count1 == 0 || inputs.msg_count2 < MSG_OPENING_FRAMES {
            // Opening/closing window iris: frames 0-4
            (inputs.msg_count2 as i32).min(4)
        } else {
            self.face_talk
        };
        sprites.draw_face(gpu, frame, 48, 152 + BITMAP_Y_OFS);

        // Text only once the window is open and the message is live.
        if inputs.msg_count1 == 0 || inputs.msg_count2 < MSG_OPENING_FRAMES {
            return;
        }

        if let Some(msg) = inputs.message_text {
            // chkmeter (CONTINUE.ASM): long-message variants (bit 7),
            // Andross, or a visible teammate meter push the text up a line.
            let mut ty = MSG_TEXT_Y;
            if inputs.whichfriend & 0x80 != 0
                || inputs.whichfriend & 0x7F == FRIEND_ANDROSS
                || inputs.friends_meter != 0
            {
                ty -= MSG_LINE_H;
            }

            let lines = wrap_message(msg);
            let shadow = sprites.bitmap_color(9); // msprintstr shadow colour 9
            for (i, line) in lines.iter().enumerate() {
                self.draw_msg_line(
                    gpu,
                    font,
                    MSG_TEXT_X + 1,
                    ty + i as i32 * MSG_LINE_H + 1,
                    line,
                    shadow[0],
                    shadow[1],
                    shadow[2],
                );
                self.draw_msg_line(
                    gpu,
                    font,
                    MSG_TEXT_X,
                    ty + i as i32 * MSG_LINE_H,
                    line,
                    1.0,
                    1.0,
                    1.0,
                );
            }
        }
    }

    /// Teammate HP meter (mshowteammate2, MTXTPRT.MC:568).
    fn draw_teammate_meter(&self, gpu: &mut Gpu, sprites: &Sprites, inputs: &FrameInputs) {
        if inputs.gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD) != 0 {
            return;
        }
        if inputs.msg_count1 == 0 || inputs.msg_count2 < MSG_OPENING_FRAMES {
            return;
        }
        if inputs.friends_meter == 0 {
            return;
        }

        let mut hp = (inputs.friends_meter & 0x7F) as i32;
        self.snes_outline_box(
            gpu,
            sprites,
            82.0,
            (177 + BITMAP_Y_OFS) as f32,
            44.0,
            12.0,
            14,
        );
        if hp > 0 {
            hp = hp.min(40);
            self.snes_solid_rect(
                gpu,
                sprites,
                84.0,
                (179 + BITMAP_Y_OFS) as f32,
                hp as f32,
                8.0,
                2,
            );
        }
    }

    /// Mirror of `Hud_Render`. The end-level transfer loop retains the
    /// standard shield/boost/inventory HUD around its dedicated tally bitmap.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        gpu: &mut Gpu,
        sprites: &mut Sprites,
        font: &mut Font,
        inputs: &FrameInputs,
        screen_width: i32,
        screen_height: i32,
    ) {
        let tally = inputs.tally_active;
        if inputs.game_state != GameState::Playing && !tally {
            self.was_playing = false;
            return;
        }

        self.screen_w = screen_width as f32;
        self.screen_h = screen_height as f32;

        // MAIN.ASM:105 m_meters=1 stand-in (see module docs).
        if !self.was_playing || inputs.stage as u32 != self.last_stage {
            self.was_playing = true;
            self.last_stage = inputs.stage as u32;
            self.force_meters = true;
        }

        self.tick_animations(inputs);

        sprites.set_screen_size(screen_width, screen_height);
        font.set_screen_size(screen_width, screen_height);

        // do_sprites_l gate: m_meters set, screen not blacked out.
        let hud_on = (inputs.meters != 0 || self.force_meters) && inputs.stayblack == -1;

        if hud_on {
            // --- Super FX bitmap meters (MDRAWLIS.MC), bitmap y + 16 ---

            // Shield meter (mdamagemeter): 40x8 box colour 13 at (8,176),
            // fill = player HP clamped to 36, 4px tall. Colour 7 while
            // shieldup (wireframe shield), else colour 2 (MDRAWLIS.MC:911-926).
            {
                let fill = inputs.shield_cur.clamp(0, 36);
                let fill_col = shield_meter_fill_color(inputs.shieldup);
                self.snes_outline_box(
                    gpu,
                    sprites,
                    8.0,
                    (176 + BITMAP_Y_OFS) as f32,
                    40.0,
                    8.0,
                    13,
                );
                if fill > 0 {
                    self.snes_solid_rect(
                        gpu,
                        sprites,
                        10.0,
                        (178 + BITMAP_Y_OFS) as f32,
                        fill as f32,
                        4.0,
                        fill_col,
                    );
                }
            }

            // Boost/brake gauge (mboostmeter): mirror box at (176,176),
            // fill = m_boostanim clamped to 36, colour 6.
            {
                let fill = self.boostanim.min(36);
                self.snes_outline_box(
                    gpu,
                    sprites,
                    176.0,
                    (176 + BITMAP_Y_OFS) as f32,
                    40.0,
                    8.0,
                    13,
                );
                if fill > 0 {
                    self.snes_solid_rect(
                        gpu,
                        sprites,
                        178.0,
                        (178 + BITMAP_Y_OFS) as f32,
                        fill as f32,
                        4.0,
                        6,
                    );
                }
            }

            // Boss HP bar (mdrawbossHP, MDRAWLIS.MC:985-1057): right-aligned
            // at x=222, bitmap y=2. Fill = m_bossHP / m_bossmaxHP, both halved
            // when maxHP>=128. ROM does NOT clamp the fill to max — it only
            // resets m_bossHP to 0 when it overflows max+10 (MDRAWLIS.MC:993-999,
            // low-byte compare, before the halving); otherwise the fill is
            // drawn unclamped.
            if !tally && inputs.boss_hp_max > 0 {
                let maxv_raw = inputs.boss_hp_max & 0xFF;
                let mut curv = inputs.boss_hp_cur & 0xFF;
                if curv >= maxv_raw + 10 {
                    curv = 0;
                }
                let mut maxv = maxv_raw;
                if maxv_raw & 0x80 != 0 {
                    maxv >>= 1;
                    curv >>= 1;
                }
                let w = maxv + 4;
                let bx = (222 - w) as f32;
                self.snes_outline_box(
                    gpu,
                    sprites,
                    bx,
                    (2 + BITMAP_Y_OFS) as f32,
                    w as f32,
                    6.0,
                    14,
                );
                if curv > 0 {
                    self.snes_solid_rect(
                        gpu,
                        sprites,
                        bx + 2.0,
                        (4 + BITMAP_Y_OFS) as f32,
                        curv as f32,
                        2.0,
                        2,
                    );
                }
            }

            if !tally {
                self.draw_teammate_meter(gpu, sprites, inputs);

                // Portrait sits on the bitmap layer, under the OAM sprites.
                self.draw_radio_message(gpu, sprites, font, inputs);
            }

            // --- OAM sprites (SPRITES.ASM do_sprites_l order) ---

            // Nova bomb icons (do_spec_weap, SPRITES.ASM:664-717): tile $3C
            // from (225,182) stepping x -= 9. While `specflash` is live, draw
            // bombs-1 solid then blink the newest icon on `gameframe&7 >= 3`.
            let (solid, blink_on) =
                bomb_icon_draw_count(inputs.bombs, inputs.specflash, inputs.gameframe);
            for i in 0..solid {
                sprites.draw8(0x3C, 225 - i * 9, 182, SPR_PAL_DEFAULT, 0);
            }
            if blink_on {
                sprites.draw8(0x3C, 225 - solid * 9, 182, SPR_PAL_DEFAULT, 0);
            }

            // Crosshair (do_crosshair): four tile-$61 corners around
            // (124,100), palette 4, cockpit view only.
            if !tally && inputs.player_view_mode == sf_core::player_view::PlayerViewMode::Cockpit {
                sprites.draw8(0x61, 124 - 8, 100 - 8, SPR_PAL_CROSS, 0);
                sprites.draw8(0x61, 124 + 8, 100 - 8, SPR_PAL_CROSS, SPR_HFLIP);
                sprites.draw8(0x61, 124 - 8, 100 + 8, SPR_PAL_CROSS, SPR_VFLIP);
                sprites.draw8(0x61, 124 + 8, 100 + 8, SPR_PAL_CROSS, SPR_HFLIP | SPR_VFLIP);
            }

            // Lives (do_lives): ship icon, "x", digit at (16/24/32, 17).
            {
                let mut lc = if inputs.lives > 0 { inputs.lives } else { 1 };
                lc -= 1;
                lc = lc.min(9);
                sprites.draw8(0x3D, 16, 17, SPR_PAL_DEFAULT, 0);
                sprites.draw8(0x62, 24, 17, SPR_PAL_DEFAULT, 0);
                sprites.draw8(0x63 + lc, 32, 17, SPR_PAL_DEFAULT, 0);
            }

            // "SHIELD" label (do_shield): tiles $4B-$4E at (24..48, 183).
            sprites.draw8(0x4B, 24, 183, SPR_PAL_DEFAULT, 0);
            sprites.draw8(0x4C, 32, 183, SPR_PAL_DEFAULT, 0);
            sprites.draw8(0x4D, 40, 183, SPR_PAL_DEFAULT, 0);
            sprites.draw8(0x4E, 48, 183, SPR_PAL_DEFAULT, 0);

            // Boss "ENEMY" tiles (do_enemy): $71-$74 at (200-hp, 16).
            if !tally && inputs.boss_hp_max > 0 {
                let mut v = inputs.boss_hp_max & 0xFF;
                if v & 0x80 != 0 {
                    v >>= 1;
                }
                for i in 0..4 {
                    sprites.draw8(0x71 + i, 200 - v + i * 8, 16, SPR_PAL_DEFAULT, 0);
                }
            }

            // Radar arrows (do_arrows), flashing via sprframe.
            if !tally && inputs.arrows & SPRAR_UP != 0 {
                self.draw_arrow_set(sprites, &ARROW_UP);
            }
            if !tally && inputs.arrows & SPRAR_DOWN != 0 {
                self.draw_arrow_set(sprites, &ARROW_DOWN);
            }
            if !tally && inputs.arrows & SPRAR_LEFT != 0 {
                self.draw_arrow_set(sprites, &ARROW_LEFT);
            }
            if !tally && inputs.arrows & SPRAR_RIGHT != 0 {
                self.draw_arrow_set(sprites, &ARROW_RIGHT);
            }
        } else {
            // Radio traffic still runs with the meters hidden during flight.
            if !tally {
                self.draw_teammate_meter(gpu, sprites, inputs);
                self.draw_radio_message(gpu, sprites, font, inputs);
            }
        }

        sprites.render_hud(gpu);
    }
}

/// Shield meter fill color: wireframe shield uses 7, otherwise 2
/// (MDRAWLIS.MC:911-926 / TRANS.ASM m_shieldup).
pub fn shield_meter_fill_color(shieldup: u8) -> usize {
    if shieldup != 0 {
        7
    } else {
        2
    }
}

/// ROM `do_spec_weap` (SPRITES.ASM:664-717) bomb-icon layout:
/// solid icons for `bombs` (or `bombs-1` while flashing), plus the newest
/// icon only when `specflash != 0` and `(gameframe & 7) >= 3`.
pub fn bomb_icon_draw_count(bombs: i32, specflash: u8, gameframe: u16) -> (i32, bool) {
    let bombs = bombs.clamp(0, 9);
    let flashing = specflash > 0 && bombs > 0;
    let solid = if flashing { bombs - 1 } else { bombs };
    let blink_on = flashing && (gameframe & 7) >= 3;
    (solid, blink_on)
}

#[cfg(test)]
mod tests {
    use super::{bomb_icon_draw_count, shield_meter_fill_color, Hud, SPRAR_UP};
    use crate::renderer::FrameInputs;

    #[test]
    fn bomb_flash_hides_newest_until_blink_window() {
        // No flash: all three solid, no blink icon.
        assert_eq!(bomb_icon_draw_count(3, 0, 0), (3, false));
        // Flashing: draw 2 solid; blink off when gameframe&7 < 3.
        assert_eq!(bomb_icon_draw_count(3, 30, 0), (2, false));
        assert_eq!(bomb_icon_draw_count(3, 30, 2), (2, false));
        // Blink on when gameframe&7 >= 3.
        assert_eq!(bomb_icon_draw_count(3, 30, 3), (2, true));
        assert_eq!(bomb_icon_draw_count(3, 30, 7), (2, true));
        // Zero bombs: nothing.
        assert_eq!(bomb_icon_draw_count(0, 30, 7), (0, false));
    }

    #[test]
    fn shield_meter_uses_color_seven_when_shieldup() {
        assert_eq!(shield_meter_fill_color(0), 2);
        assert_eq!(shield_meter_fill_color(1), 7);
        assert_eq!(shield_meter_fill_color(0xFF), 7);
    }

    #[test]
    fn arrow_flash_wrap_queues_se_8a_when_arrows_visible() {
        // do_arrows (SPRITES.ASM:861-876): every other gameframe advances
        // sprframe; on wrap (4→0) with arrows≠0 queue trigse $8a.
        let mut hud = Hud::default();
        let mut inputs = FrameInputs::default();
        inputs.arrows = SPRAR_UP;
        // Prime last_gameframe so subsequent ticks animate.
        inputs.gameframe = 0;
        hud.tick_animations(&inputs);
        assert!(hud.pending_sounds.is_empty());

        // Odd frames advance sprframe: 1,2,3,4→wrap.
        for gf in [1u16, 3, 5, 7] {
            inputs.gameframe = gf;
            hud.tick_animations(&inputs);
        }
        assert_eq!(hud.take_pending_sounds(), vec![0x8A]);

        // Wrap with no arrows: silent.
        inputs.arrows = 0;
        for gf in [9u16, 11, 13, 15] {
            inputs.gameframe = gf;
            hud.tick_animations(&inputs);
        }
        assert!(hud.take_pending_sounds().is_empty());
    }
}
