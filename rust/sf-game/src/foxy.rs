//! Fox continue-screen state (ROM CONTINUE.ASM `foxy_continue_l` / `foxytrans_l`).
//!
//! SNES path: load fox BG/OBJ VRAM, charmap, clear bitmap halves, build fox
//! OAM, then spin `foxytrans` (clear + sprites + Mario `mshowobj3`). HD keeps
//! the selectable UI state + transfer counters; no PPU/GSU.

use crate::charmap::{CharMap, CharMapScreen};
use crate::dma::DmaFlush;
use crate::planets::DEFAULT_LIVES;

/// Default demo ship shape for the continue viewer (ROM `my_demo`).
pub const FOXY_SHAPE_MY_DEMO: u16 = 0; // shape id filled by shell/catalog

/// Cursor Y (ROM spr_foxcursor+1 = 179).
pub const FOXY_CURSOR_Y: u8 = 179;
/// Cursor X when option = continue (ROM 45).
pub const FOXY_CURSOR_X_CONTINUE: u8 = 45;
/// Cursor X when option = quit (ROM 93).
pub const FOXY_CURSOR_X_QUIT: u8 = 93;
/// Initial Mario viewer Z (ROM `m_bigz` = 350).
pub const FOXY_BIG_Z_INIT: i16 = 350;

/// ROM continue-screen fox viewer / menu state.
#[derive(Debug, Clone)]
pub struct FoxyContinue {
    /// ROM `foxy_option` — 0 = Continue, nonzero = Quit (SELECT toggles).
    pub option: u8,
    /// ROM `foxy_frame` — arm/eye anim counter (0 = idle).
    pub frame: u8,
    /// ROM `foxy_foot` — foot anim / random walk.
    pub foot: u8,
    /// ROM `foxy_shape` / `m_shapeptr`.
    pub shape: u16,
    /// ROM `foxy_ptr` into shapeslist.
    pub ptr: u16,
    /// ROM `foxy_xa/ya/za` — per-frame rot deltas applied in drawsome3d.
    pub rot_dx: i8,
    pub rot_dy: i8,
    pub rot_dz: i8,
    /// ROM `m_bigz` zoom.
    pub big_z: i16,
    /// Cursor sprite X/Y after last [`Self::fox_sprites`].
    pub cursor_x: u8,
    pub cursor_y: u8,
    /// How many times `clronehalf` / `clrotherhalf` ran (GSU clear stand-in).
    pub half_clears: u32,
    /// `fox_sprites_l` invocations.
    pub sprites_built: u32,
    /// `drawsome3d` invocations (Mario showobj stand-in).
    pub draw3d_frames: u32,
    /// Viewer rot accumulated (low byte of m_rotx/y/z).
    pub rot_x: u8,
    pub rot_y: u8,
    pub rot_z: u8,
}

impl Default for FoxyContinue {
    fn default() -> Self {
        Self {
            option: 0,
            frame: 0,
            foot: 0,
            shape: FOXY_SHAPE_MY_DEMO,
            ptr: 0,
            rot_dx: 0,
            rot_dy: 4,
            rot_dz: 0,
            big_z: FOXY_BIG_Z_INIT,
            cursor_x: FOXY_CURSOR_X_CONTINUE,
            cursor_y: FOXY_CURSOR_Y,
            half_clears: 0,
            sprites_built: 0,
            draw3d_frames: 0,
            rot_x: 0,
            rot_y: 0,
            rot_z: 0,
        }
    }
}

impl FoxyContinue {
    /// ROM `clronehalf_l` / `clrotherhalf_l` — GSU bitmap clear; HD counts.
    pub fn clear_one_half(&mut self) {
        self.half_clears = self.half_clears.wrapping_add(1);
    }

    pub fn clear_other_half(&mut self) {
        self.half_clears = self.half_clears.wrapping_add(1);
    }

    /// ROM `fox_sprites_l` — place cursor from `foxy_option` (+ arm/eye/foot
    /// anim tables omitted; HD only needs the selectable cursor).
    pub fn fox_sprites(&mut self) {
        self.cursor_y = FOXY_CURSOR_Y;
        self.cursor_x = if self.option == 0 {
            FOXY_CURSOR_X_CONTINUE
        } else {
            FOXY_CURSOR_X_QUIT
        };
        self.sprites_built = self.sprites_built.wrapping_add(1);
    }

    /// ROM `drawsome3d` — advance gameframe viewer rot by xa/ya/za.
    pub fn draw_some_3d(&mut self) {
        self.rot_x = self.rot_x.wrapping_add(self.rot_dx as u8);
        self.rot_y = self.rot_y.wrapping_add(self.rot_dy as u8);
        self.rot_z = self.rot_z.wrapping_add(self.rot_dz as u8);
        self.draw3d_frames = self.draw3d_frames.wrapping_add(1);
    }

    /// ROM `foxytrans_l` — wait transfer flags (HD: immediate), clear halves,
    /// rebuild fox sprites, draw 3D fox.
    pub fn foxy_trans(&mut self) {
        self.clear_one_half();
        self.clear_other_half();
        self.fox_sprites();
        self.draw_some_3d();
    }

    /// SELECT toggles continue vs quit (CONTINUE.ASM:208-214).
    pub fn toggle_option(&mut self) {
        self.option ^= 0xFF;
        self.fox_sprites();
    }

    /// True when the player chose Continue (option == 0).
    pub fn chose_continue(&self) -> bool {
        self.option == 0
    }
}

/// Result of entering the fox continue screen.
#[derive(Debug, Clone)]
pub struct FoxyContinueEnter {
    pub foxy: FoxyContinue,
    pub lives: u8,
    pub charmap: CharMapScreen,
    pub dma: DmaFlush,
}

/// ROM `foxy_continue_l` (CONTINUE.ASM:42) — early-out if `credits == 0`;
/// otherwise reset fox state, set fox charmap, clear halves, build sprites,
/// DMA OAM, then run two `foxytrans` frames (fade-in prep).
///
/// Returns `None` when there are no continue credits (ROM `.end2`).
pub fn foxy_continue_enter(credits: u8, charmap: &mut CharMap) -> Option<FoxyContinueEnter> {
    if credits == 0 {
        return None;
    }
    charmap.set_fox();
    let mut foxy = FoxyContinue::default();
    foxy.clear_one_half();
    foxy.clear_other_half();
    foxy.fox_sprites();
    let mut dma = DmaFlush::new();
    dma.dma_sprites();
    // Two foxytrans before the fade loop (CONTINUE.ASM:159-160).
    foxy.foxy_trans();
    foxy.foxy_trans();
    Some(FoxyContinueEnter {
        foxy,
        lives: DEFAULT_LIVES,
        charmap: CharMapScreen::Fox,
        dma,
    })
}

/// ROM `endtrans_l` (ENDSEQ.ASM:392) — wait for transfer slot, clear halves,
/// rebuild draw path. HD records a completed end-transfer tick.
#[derive(Debug, Clone, Default)]
pub struct EndTrans {
    pub ticks: u32,
    pub half_clears: u32,
}

impl EndTrans {
    pub fn run(&mut self) {
        self.half_clears = self.half_clears.wrapping_add(2); // one + other
        self.ticks = self.ticks.wrapping_add(1);
    }
}
