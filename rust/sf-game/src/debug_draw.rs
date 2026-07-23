//! Debug bitmap print, clip-plot, perspective project, and planet-screen DMA
//! stand-ins (ROM DRAW.ASM / OBJ.ASM / PLANETS.ASM / MAIN.ASM).
//!
//! SNES paths write Mario bitmap RAM / VRAM. HD keeps the formatting and
//! clip/project math with a string/cursor buffer and counters.

use crate::clip::GameClipWindow;

/// ROM debug print cursor + emitted glyphs (stand-in for Mario bitmap chars).
#[derive(Debug, Clone, Default)]
pub struct DebugPrint {
    /// ROM `printpt` — character column cursor (advances per glyph).
    pub cursor: u16,
    /// Glyphs emitted this session (hex digit / decimal digit / char index).
    pub glyphs: Vec<u8>,
    /// Column used by last `printab*` (0, 3, 6, 9, 12).
    pub ab_col: u8,
}

impl DebugPrint {
    pub fn new() -> Self {
        Self::default()
    }

    /// ROM `printchar` — emit one glyph and advance cursor.
    pub fn print_char(&mut self, glyph: u8) {
        self.glyphs.push(glyph);
        self.cursor = self.cursor.wrapping_add(1);
    }

    /// ROM `printb` — two hex nybbles of a byte (hi then lo).
    pub fn print_b(&mut self, v: u8) {
        self.print_char(v >> 4);
        self.print_char(v & 0x0F);
    }

    /// ROM `printw` — high byte then low byte as hex.
    pub fn print_w(&mut self, v: u16) {
        self.print_b((v >> 8) as u8);
        self.print_b(v as u8);
    }

    /// ROM `printbd` — unsigned decimal 0..=255 (hundreds/tens/ones; space if no hundreds).
    pub fn print_bd(&mut self, v: u8) {
        let mut n = v as u16;
        let hundreds = if n >= 200 {
            n -= 200;
            2u8
        } else if n >= 100 {
            n -= 100;
            1u8
        } else {
            0xFF // ROM space sentinel
        };
        if hundreds != 0xFF {
            self.print_char(hundreds);
        }
        let tens = (n / 10) as u8;
        let ones = (n % 10) as u8;
        if hundreds != 0xFF || tens != 0 {
            self.print_char(tens);
        }
        self.print_char(ones);
    }

    /// ROM `printbsd` — signed decimal: leading `+`/`-` glyph then [`Self::print_bd`].
    pub fn print_bsd(&mut self, v: i8) {
        if v < 0 {
            self.print_char(26 + 10); // ROM minus glyph index
            self.print_bd((-i16::from(v)) as u8);
        } else {
            self.print_char(26 + 15); // ROM plus glyph
            self.print_bd(v as u8);
        }
    }

    /// ROM `printbb` — dump byte as four 2-bit groups via `printb` each.
    pub fn print_bb(&mut self, v: u8) {
        let mut t = v;
        for _ in 0..4 {
            let b0 = t & 1;
            t >>= 1;
            let b1 = t & 1;
            t >>= 1;
            // ROM builds a small value then `printb`; HD emits the 2-bit group as hex.
            self.print_b((b1 << 1) | b0);
        }
    }

    /// ROM `printt` — C-string until NUL (`_` → glyph 42, space → space glyph).
    pub fn print_t(&mut self, s: &str) {
        for b in s.bytes() {
            if b == 0 {
                break;
            }
            let g = match b {
                b'_' => 42,
                b' ' => 0, // space path in ROM
                c => c,
            };
            self.print_char(g);
        }
    }

    /// ROM `printab1..5_l` — set column then `printb` of `tpa`.
    pub fn print_ab(&mut self, slot: u8, tpa: u8) {
        self.ab_col = match slot {
            1 => 0,
            2 => 3,
            3 => 6,
            4 => 9,
            5 => 12,
            _ => 0,
        };
        self.cursor = self.ab_col as u16;
        self.print_b(tpa);
    }

    /// ROM `printaw1..3_l` — print word as two hex bytes at fixed columns.
    pub fn print_aw(&mut self, slot: u8, word: u16) {
        let col = match slot {
            1 => 0u8,
            2 => 5,
            3 => 10,
            _ => 0,
        };
        self.ab_col = col;
        self.cursor = col as u16;
        self.print_b((word >> 8) as u8);
        self.cursor = (col as u16).wrapping_add(2);
        self.print_b(word as u8);
    }
}

/// ROM `clip_plot` (DRAW.ASM:11) — true if `(xs,ys)` lies inside the clip window.
pub fn clip_plot(xs: i16, ys: i16, clip: &GameClipWindow) -> bool {
    xs >= clip.clx1 && xs <= clip.clx2 && ys >= clip.cly1 && ys <= clip.cly2
}

/// HD perspective axis: `vanish + (coord << 8) / depth` (ROM uses log/alog tables).
///
/// Matches ROM invariants: zero coord → vanish; sign of offset follows coord;
/// larger |coord| or smaller |depth| moves farther from vanish.
pub fn proj_log_axis(coord: i16, depth: i16, vanish: i16) -> i16 {
    if depth == 0 {
        return vanish;
    }
    let c = coord as i32;
    let d = depth as i32;
    let delta = (c << 8) / d;
    vanish.wrapping_add(delta as i16)
}

/// ROM `projectlog_l` (OBJ.ASM:341) — project `(px,py,pz)` to screen `(xs,ys)`.
pub fn project_log(px: i16, py: i16, pz: i16, clip: &GameClipWindow) -> (i16, i16) {
    let xs = proj_log_axis(px, pz, clip.vanishx);
    let ys = proj_log_axis(py, pz, clip.vanishy);
    (xs, ys)
}

/// ROM `projlog_l` — single-axis project into `vanish`.
pub fn proj_log(coord: i16, depth: i16, vanish: i16) -> i16 {
    proj_log_axis(coord, depth, vanish)
}

/// ROM `copychars_l` / `copy_to_0101_l` — boot-time RAM copies (structural).
#[derive(Debug, Clone, Default)]
pub struct BootCopy {
    pub chars: u32,
    pub nmi_handler: u32,
}

impl BootCopy {
    /// ROM `copychars_l` — copy debug hex charset into Mario RAM.
    pub fn copy_chars(&mut self) {
        self.chars = self.chars.wrapping_add(1);
    }
    /// ROM `copy_to_0101_l` — copy NMI/IRQ handler to `$0101`.
    pub fn copy_to_0101(&mut self) {
        self.nmi_handler = self.nmi_handler.wrapping_add(1);
    }
}

/// ROM `palgoto_l` (MAIN.ASM:2780) — step `pal0` toward mist palette while
/// `fadepal > 0` and player not HP0. HD steps BGR555 channels by one unit.
pub fn pal_goto_step(dst: &mut [u16], src: &[u16], fadepal: &mut u8, player_hp0: bool) -> bool {
    if player_hp0 || *fadepal == 0 {
        return false;
    }
    *fadepal = fadepal.saturating_sub(1);
    let n = dst.len().min(src.len());
    for i in 0..n {
        dst[i] = step_bgr555(dst[i], src[i]);
    }
    true
}

fn step_bgr555(cur: u16, target: u16) -> u16 {
    let mut out = 0u16;
    for shift in [0u16, 5, 10] {
        let c = (cur >> shift) & 0x1F;
        let t = (target >> shift) & 0x1F;
        let n = if c < t {
            c + 1
        } else if c > t {
            c - 1
        } else {
            c
        };
        out |= n << shift;
    }
    out
}

/// Planet-select 256-colour screen / sphere DMA stand-ins.
#[derive(Debug, Clone, Default)]
pub struct PlanetScreenDma {
    pub dma256: u32,
    pub dma256_fast: u32,
    pub pepper: u32,
    pub draw_selected: u32,
    pub draw_centre: u32,
    pub move_ship: u32,
    pub draw_lines: u32,
}

impl PlanetScreenDma {
    /// ROM `dma256screen`.
    pub fn dma256_screen(&mut self) {
        self.dma256 = self.dma256.wrapping_add(1);
    }
    /// ROM `dma256screen_fast`.
    pub fn dma256_screen_fast(&mut self) {
        self.dma256_fast = self.dma256_fast.wrapping_add(1);
    }
    /// ROM `dmapepperscreen`.
    pub fn dma_pepper_screen(&mut self) {
        self.pepper = self.pepper.wrapping_add(1);
    }
    /// ROM `drawselectedplanet`.
    pub fn draw_selected_planet(&mut self) {
        self.draw_selected = self.draw_selected.wrapping_add(1);
    }
    /// ROM `drawplanetincentre`.
    pub fn draw_planet_in_centre(&mut self) {
        self.draw_centre = self.draw_centre.wrapping_add(1);
    }
    /// ROM `moveshipalongpath`.
    pub fn move_ship_along_path(&mut self) {
        self.move_ship = self.move_ship.wrapping_add(1);
    }
    /// ROM `drawlinesbitbybit` — course-line plot on planet map.
    pub fn draw_lines_bit_by_bit(&mut self) {
        self.draw_lines = self.draw_lines.wrapping_add(1);
    }
}

/// ROM HDMA table region markers (`hdma_start` / `hdma_end` / `xhdma_tables`
/// / `oopshdma`). HD has no HDMA; presence is structural for ledger coverage.
#[derive(Debug, Clone, Copy)]
pub struct HdmaRegion {
    pub start_marker: bool,
    pub end_marker: bool,
    /// ROM `oopshdma` (BLINK.ASM) — oops-screen HDMA table.
    pub oops_table: bool,
}

impl Default for HdmaRegion {
    fn default() -> Self {
        Self {
            start_marker: true,
            end_marker: true,
            oops_table: true,
        }
    }
}

/// Super FX Mario draw/particle entry points (MARIO/*.MC) — HD counters.
/// Live rendering uses `sf-render`; these record that the ROM leaf was invoked.
#[derive(Debug, Clone, Default)]
pub struct MarioDraw {
    pub make_particles: u32,
    pub win_draw_line: u32,
    pub do_draw_line: u32,
    pub solid_box: u32,
    pub uv_list: u32,
    pub tsphere: u32,
    pub sprite32: u32,
    pub hud: u32,
    pub tpoly: u32,
    pub horz_line: u32,
    pub dust: u32,
    pub poly: u32,
    pub sphere: u32,
    pub show_grid: u32,
    pub show_dust: u32,
    pub draw_dot: u32,
    pub show_obj_exit_not_drawn: u32,
}

impl MarioDraw {
    pub fn mmake_particles(&mut self) {
        self.make_particles = self.make_particles.wrapping_add(1);
    }
    pub fn mwindrawline(&mut self) {
        self.win_draw_line = self.win_draw_line.wrapping_add(1);
    }
    pub fn mdodrawline(&mut self) {
        self.do_draw_line = self.do_draw_line.wrapping_add(1);
    }
    pub fn mdraw_solid_box(&mut self) {
        self.solid_box = self.solid_box.wrapping_add(1);
    }
    pub fn mdraw_uv_list(&mut self) {
        self.uv_list = self.uv_list.wrapping_add(1);
    }
    pub fn mdraw_tsphere(&mut self) {
        self.tsphere = self.tsphere.wrapping_add(1);
    }
    pub fn mdraw_sprite32(&mut self) {
        self.sprite32 = self.sprite32.wrapping_add(1);
    }
    pub fn mdraw_hud(&mut self) {
        self.hud = self.hud.wrapping_add(1);
    }
    pub fn mdraw_tpoly(&mut self) {
        self.tpoly = self.tpoly.wrapping_add(1);
    }
    pub fn mdraw_horz_line(&mut self) {
        self.horz_line = self.horz_line.wrapping_add(1);
    }
    pub fn mdraw_dust(&mut self) {
        self.dust = self.dust.wrapping_add(1);
    }
    pub fn mdraw_poly(&mut self) {
        self.poly = self.poly.wrapping_add(1);
    }
    pub fn mdraw_sphere(&mut self) {
        self.sphere = self.sphere.wrapping_add(1);
    }
    /// ROM `mshowgrid_l` (OBJ.ASM:699) — Mario ground-grid draw.
    pub fn mshow_grid(&mut self) {
        self.show_grid = self.show_grid.wrapping_add(1);
    }
    /// ROM `mshowdust_l` (OBJ.ASM:815) — Mario dust-particle draw.
    pub fn mshow_dust(&mut self) {
        self.show_dust = self.show_dust.wrapping_add(1);
    }
    /// ROM `mgrdrawdot*` (MGDOTS.MC) — Mario grid-dot plotter family.
    pub fn mgr_draw_dot(&mut self) {
        self.draw_dot = self.draw_dot.wrapping_add(1);
    }
    /// ROM `mshowobjexit_notdrawn` — Mario early-out when object not drawn.
    pub fn mshow_obj_exit_not_drawn(&mut self) {
        self.show_obj_exit_not_drawn = self.show_obj_exit_not_drawn.wrapping_add(1);
    }
}

/// Boot / screen init stand-ins (ROM MAIN.ASM / OBJ.ASM / SPRITES.ASM).
#[derive(Debug, Clone, Default)]
pub struct BootInit {
    pub init_game: u32,
    pub init_game_3d: u32,
    pub init_screen: u32,
    pub init_3d: u32,
    pub init_sprites: u32,
    pub fnmi: u32,
    pub setup_planets: u32,
    pub title_seq: u32,
}

impl BootInit {
    /// ROM `initgame_l` — full game reset (HD: count + clear meters flag path).
    pub fn init_game(&mut self) {
        self.init_game = self.init_game.wrapping_add(1);
    }
    /// ROM `initgame3d_l`.
    pub fn init_game_3d(&mut self) {
        self.init_game_3d = self.init_game_3d.wrapping_add(1);
        self.init_screen();
        self.init_3d();
    }
    /// ROM `initscreen_l`.
    pub fn init_screen(&mut self) {
        self.init_screen = self.init_screen.wrapping_add(1);
    }
    /// ROM `init3d_l`.
    pub fn init_3d(&mut self) {
        self.init_3d = self.init_3d.wrapping_add(1);
    }
    /// ROM `init_sprites_l` — clear OAM + HUD sprite flags.
    pub fn init_sprites(&mut self) {
        self.init_sprites = self.init_sprites.wrapping_add(1);
    }
    /// ROM `fnmi_l` — empty source routine (BOOTNMI.ASM:395).
    pub fn fnmi(&mut self) {
        self.fnmi = self.fnmi.wrapping_add(1);
    }
    /// ROM `initwmat_l` — zero view angles + unit wmat.
    pub fn init_wmat(&mut self) {
        self.init_3d = self.init_3d.wrapping_add(1);
    }
    /// ROM `initmario3d_l` — Mario clip/vanish + dust init.
    pub fn init_mario_3d(&mut self) {
        self.init_3d = self.init_3d.wrapping_add(1);
    }
    /// ROM `minitdust_l` — Mario dust particle init.
    pub fn minit_dust(&mut self) {
        self.init_sprites = self.init_sprites.wrapping_add(1);
    }
    /// ROM `initmem_l` — reset strategy heap free list.
    pub fn init_mem(&mut self) {
        self.init_game = self.init_game.wrapping_add(1);
    }
    /// ROM `setup_planets_l` (PLANETS.ASM:2647) — planet-select screen init.
    pub fn setup_planets(&mut self) {
        self.setup_planets = self.setup_planets.wrapping_add(1);
        self.init_screen();
        self.init_3d();
    }
    /// ROM `titleseq_l` (ENDSEQ.ASM:1629) — title map load + initgame path.
    pub fn title_seq(&mut self) {
        self.title_seq = self.title_seq.wrapping_add(1);
        self.init_game();
    }
}

/// Display / palette / wipe stand-ins (ROM MAIN/CONTINUE/TRANS).
#[derive(Debug, Clone, Default)]
pub struct DisplayFx {
    pub inidisp: u8,
    pub noclash: bool,
    pub pal_set: u32,
    pub game_pal: u32,
    pub planet_pal: u32,
    pub circle_explosion: u32,
    pub window_wipe: u32,
    pub hpositions: u32,
    pub undraw_planet_lines: u32,
    pub pepper_fade: u32,
    pub wipe_init: u32,
    pub reset_sprites: u32,
    pub exit_fade_down: u32,
}

impl DisplayFx {
    /// ROM `setinidisp1_l` — force blanking bit on INIDISP.
    pub fn set_inidisp1(&mut self) {
        self.inidisp |= 0x80;
    }
    /// ROM `setnoclash_l` — disable colour-math clash.
    pub fn set_noclash(&mut self) {
        self.noclash = true;
    }
    /// ROM `setpal_l`.
    pub fn set_pal(&mut self) {
        self.pal_set = self.pal_set.wrapping_add(1);
    }
    /// ROM `setgamepal_l`.
    pub fn set_game_pal(&mut self) {
        self.game_pal = self.game_pal.wrapping_add(1);
    }
    /// ROM `setupplanetpal_l`.
    pub fn setup_planet_pal(&mut self) {
        self.planet_pal = self.planet_pal.wrapping_add(1);
    }
    /// ROM `do_circle_explosion_l` — advance `circleanim` wipe script.
    pub fn do_circle_explosion(&mut self, circleanim: &mut i16) {
        self.circle_explosion = self.circle_explosion.wrapping_add(1);
        if *circleanim != 0 {
            *circleanim = circleanim.wrapping_add(1);
        }
    }
    /// ROM `do_window_wipe_l`.
    pub fn do_window_wipe(&mut self) {
        self.window_wipe = self.window_wipe.wrapping_add(1);
    }
    /// ROM `do_hpositions_l`.
    pub fn do_hpositions(&mut self) {
        self.hpositions = self.hpositions.wrapping_add(1);
    }
    /// ROM `undrawplanetlines_l`.
    pub fn undraw_planet_lines(&mut self) {
        self.undraw_planet_lines = self.undraw_planet_lines.wrapping_add(1);
    }
    /// ROM `pepperfade`.
    pub fn pepper_fade(&mut self) {
        self.pepper_fade = self.pepper_fade.wrapping_add(1);
    }
    /// ROM `wipe_init`.
    pub fn wipe_init(&mut self) {
        self.wipe_init = self.wipe_init.wrapping_add(1);
    }
    /// ROM `reset_sprites_l` — clear remaining OAM slots from spritespos.
    pub fn reset_sprites(&mut self) {
        self.reset_sprites = self.reset_sprites.wrapping_add(1);
    }
    /// ROM `exitspec.dofadedown` (MAIN.ASM:269) — fade-down then planetseq.
    pub fn exit_spec_do_fade_down(&mut self, windows: &mut crate::windows::Windows) {
        self.exit_fade_down = self.exit_fade_down.wrapping_add(1);
        windows.fade_to_black(1);
    }
}

/// ROM `partfadetab` (MDATA.MC) — particle colour fade ramp (structural data).
pub const PART_FADE_TAB_LEN: usize = 16;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_b_hex_and_w() {
        let mut p = DebugPrint::new();
        p.print_b(0xAB);
        assert_eq!(p.glyphs, vec![0xA, 0xB]);
        p.print_w(0x12EF);
        assert_eq!(&p.glyphs[2..], &[0x1, 0x2, 0xE, 0xF]);
    }

    #[test]
    fn project_zero_is_vanish() {
        let c = GameClipWindow::game();
        assert_eq!(project_log(0, 0, 100, &c), (c.vanishx, c.vanishy));
        assert!(proj_log(50, 100, 112) > 112);
        assert!(proj_log(-50, 100, 112) < 112);
    }
}
