//! Color window / fade state machine (no SNES register writes).
//!
//! C oracle: `src/game/windows.c` (WINDOWS.ASM -> C conversion) plus the
//! `WindowState` / `WINDOW_MODE_*` / `WINDOWARRAY_SIZE` declarations in
//! `src/game/game_vars.h:255-271`. The C globals `g_windowmode`,
//! `g_windowarray[8]` and `g_fadedir` live in this struct; `g_oncewipe` and
//! `g_circleanim` stay in [`crate::vars::GameVars`] (the map VM's inline
//! callbacks write them, see game.rs `ClGroundWipeout`), so
//! [`Windows::update`] borrows them from the caller.

use crate::shell::WindowSlot;

/// C `WINDOWARRAY_SIZE` (src/game/game_vars.h:255).
pub const WINDOWARRAY_SIZE: usize = 8;

// C WINDOW_MODE_* (src/game/game_vars.h:257-261).
pub const WINDOW_MODE_NONE: u8 = 0;
pub const WINDOW_MODE_BLACK: u8 = 1;
pub const WINDOW_MODE_WHITEFADE: u8 = 2;
pub const WINDOW_MODE_WHITE2NORM: u8 = 3;
pub const WINDOW_MODE_MAPFADE: u8 = 4;
/// ROM `dyingred` / `bossflash_l` color-math flash slot (WINDOWS.ASM:240).
pub const WINDOW_MODE_DYINGRED: u8 = 5;
/// ROM `alloc_window hitflash` (WINDOWS.ASM:104-163) — screen hit tint.
pub const WINDOW_MODE_HITFLASH: u8 = 6;
/// ROM `halffade` window (MAIN.ASM:1470 fadehalf2norm).
pub const WINDOW_MODE_HALFFADE: u8 = 7;

/// Full black intensity and the number of unit-speed fade steps.
pub const BLACK_FADE_MAX: u8 = 30;

/// Authored presentation fade modes. The slow mode is encoded distinctly from
/// its unit intensity delta; it is not a three-level step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
pub(crate) enum MapFadeRate {
    Normal = 1,
    Quick = 2,
    Slow = 3,
}

impl MapFadeRate {
    const fn to_black_direction(self) -> i8 {
        -(self as i8)
    }

    const fn intensity_step(self) -> u8 {
        match self {
            Self::Normal | Self::Slow => 1,
            Self::Quick => 2,
        }
    }
}

const SLOW_FADE_DIRECTION: i8 = MapFadeRate::Slow.to_black_direction();

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum MapFadeTiming {
    #[default]
    PerSimulationTick,
    FixedPresentation {
        elapsed_ticks: u8,
        total_ticks: u8,
        start_intensity: u8,
    },
}

/// `stayblack` color tag for [`WINDOW_MODE_HITFLASH`] (not a black-hold timer).
pub const HITFLASH_TURQ: u8 = 0;
pub const HITFLASH_TURQ2: u8 = 1;
pub const HITFLASH_RED: u8 = 2;

/// C `MSTARWIPE_CIRCLE` (src/game/windows.c:8).
const MSTARWIPE_CIRCLE: i16 = 1;

/// Windows/fade state (C g_windowmode + g_windowarray + g_fadedir).
#[derive(Debug, Clone, Default)]
pub struct Windows {
    /// C `g_windowmode` — bitmask of allocated slots.
    pub windowmode: u8,
    /// C `g_windowarray[WINDOWARRAY_SIZE]`.
    pub slots: [WindowSlot; WINDOWARRAY_SIZE],
    /// C `g_fadedir` (int8; 0 = no map fade active).
    pub fadedir: i8,
    map_fade_timing: MapFadeTiming,
}

impl Windows {
    pub fn new() -> Self {
        Self::default()
    }

    /// C `window_find_mode` (src/game/windows.c:10).
    fn find_mode(&self, mode: u8) -> Option<usize> {
        (0..WINDOWARRAY_SIZE)
            .find(|&i| self.windowmode & (1 << i) != 0 && self.slots[i].mode == mode)
    }

    /// C `window_alloc` (src/game/windows.c:19).
    fn alloc(&mut self, mode: u8) -> Option<usize> {
        for i in 0..WINDOWARRAY_SIZE {
            if self.windowmode & (1 << i) == 0 {
                self.windowmode |= 1 << i;
                self.slots[i] = WindowSlot::default();
                self.slots[i].mode = mode;
                return Some(i);
            }
        }
        None
    }

    /// C `window_get_or_alloc` (src/game/windows.c:31).
    fn get_or_alloc(&mut self, mode: u8) -> Option<usize> {
        self.find_mode(mode).or_else(|| self.alloc(mode))
    }

    /// C `window_dealloc` (src/game/windows.c:37).
    fn dealloc(&mut self, slot: usize) {
        if slot >= WINDOWARRAY_SIZE {
            return;
        }
        self.windowmode &= !(1u8 << slot);
        self.slots[slot] = WindowSlot::default();
    }

    /// C `update_setblack_slot` (src/game/windows.c:43).
    fn update_setblack_slot(&mut self, slot: usize, oncewipe: &mut u8, circleanim: &mut i16) {
        let test = self.slots[slot].stayblack.wrapping_add(1);
        if test == 0 {
            self.dealloc(slot);
            return;
        }

        if self.slots[slot].stayblack != 0 {
            self.slots[slot].stayblack = self.slots[slot].stayblack.wrapping_sub(1);
            return;
        }

        if *oncewipe == 0 {
            *oncewipe = 1;
            *circleanim = MSTARWIPE_CIRCLE;
            self.slots[slot].stayblack = self.slots[slot].stayblack.wrapping_sub(1);
            return;
        }

        if self.slots[slot].wm_val != 0 {
            let w = self.slots[slot].wm_val;
            self.slots[slot].wm_val = if w >= 2 { w - 2 } else { 0 };
            return;
        }

        self.slots[slot].stayblack = self.slots[slot].stayblack.wrapping_sub(1);
    }

    /// C `update_fadewhite_slot` (src/game/windows.c:71).
    fn update_fadewhite_slot(&mut self, slot: usize) {
        if self.slots[slot].wm_val < 31 {
            self.slots[slot].wm_val += 1;
        }
    }

    /// C `update_fadewhite2norm_slot` (src/game/windows.c:78).
    fn update_fadewhite2norm_slot(&mut self, slot: usize) {
        if self.slots[slot].wm_val > 0 {
            self.slots[slot].wm_val -= 1;
        }
        if self.slots[slot].wm_val == 0 {
            self.dealloc(slot);
        }
    }

    /// C `update_mapfade_slot` (src/game/windows.c:90).
    fn update_mapfade_slot(&mut self, slot: usize) {
        if self.fadedir == 0 {
            return;
        }

        if self.fadedir < 0 {
            if let MapFadeTiming::FixedPresentation {
                elapsed_ticks,
                total_ticks,
                start_intensity,
            } = &mut self.map_fade_timing
            {
                *elapsed_ticks = elapsed_ticks.saturating_add(1).min(*total_ticks);
                let intensity_span = BLACK_FADE_MAX.saturating_sub(*start_intensity);
                let completed_span =
                    u16::from(intensity_span) * u16::from(*elapsed_ticks) / u16::from(*total_ticks);
                self.slots[slot].wm_val = start_intensity
                    .saturating_add(completed_span as u8)
                    .min(BLACK_FADE_MAX);
                if *elapsed_ticks >= *total_ticks {
                    self.fadedir = 0;
                    self.map_fade_timing = MapFadeTiming::PerSimulationTick;
                }
                return;
            }

            // ENDSEQ's authored slow title fade uses direction -3 as a mode,
            // not as a three-level intensity delta. Normal and quick fades
            // retain their one- and two-level deltas.
            let step = if self.fadedir == SLOW_FADE_DIRECTION {
                MapFadeRate::Slow.intensity_step()
            } else {
                (-(self.fadedir as i16)) as u8
            };
            let next = self.slots[slot].wm_val as u16 + step as u16;
            self.slots[slot].wm_val = if next >= u16::from(BLACK_FADE_MAX) {
                BLACK_FADE_MAX
            } else {
                next as u8
            };
            if self.slots[slot].wm_val >= BLACK_FADE_MAX {
                self.fadedir = 0;
            }
            return;
        }

        let step = self.fadedir as u8;
        if self.slots[slot].wm_val <= step {
            self.slots[slot].wm_val = 0;
        } else {
            self.slots[slot].wm_val -= step;
        }

        if self.slots[slot].wm_val == 0 {
            self.fadedir = 0;
            self.dealloc(slot);
        }
    }

    /// C `Windows_Init()` (src/game/windows.c:118).
    pub fn init(&mut self) {
        self.windowmode = 0;
        self.fadedir = 0;
        self.map_fade_timing = MapFadeTiming::PerSimulationTick;
        self.slots = [WindowSlot::default(); WINDOWARRAY_SIZE];
    }

    /// C `Windows_Update()` (src/game/windows.c:124). `oncewipe` /
    /// `circleanim` are `GameVars::oncewipe` / `GameVars::circleanim`
    /// (C `g_oncewipe` / `g_circleanim`).
    pub fn update(&mut self, oncewipe: &mut u8, circleanim: &mut i16) {
        for i in 0..WINDOWARRAY_SIZE {
            if self.windowmode & (1 << i) == 0 {
                continue;
            }
            match self.slots[i].mode {
                WINDOW_MODE_BLACK => self.update_setblack_slot(i, oncewipe, circleanim),
                WINDOW_MODE_WHITEFADE => self.update_fadewhite_slot(i),
                WINDOW_MODE_WHITE2NORM => self.update_fadewhite2norm_slot(i),
                WINDOW_MODE_MAPFADE => self.update_mapfade_slot(i),
                WINDOW_MODE_DYINGRED => {
                    // One-shot flash: decay wm_val then free the slot.
                    if self.slots[i].wm_val > 0 {
                        self.slots[i].wm_val -= 1;
                    } else {
                        self.dealloc(i);
                    }
                }
                WINDOW_MODE_HITFLASH => {
                    // Screen hitflash is driven by screenflashcnt in GSTRATS;
                    // keep the slot until hitflash_off / overwrite.
                }
                WINDOW_MODE_HALFFADE => {
                    // Stepped explicitly by fade_half_to_norm.
                }
                _ => {}
            }
        }
    }

    /// C `Windows_StartMapFade()` (src/game/windows.c:147).
    pub fn start_map_fade(&mut self, mut fadedir: i8) {
        self.map_fade_timing = MapFadeTiming::PerSimulationTick;
        if fadedir == 0 {
            self.fadedir = 0;
            return;
        }
        if fadedir > 2 {
            fadedir = 2;
        }
        if fadedir < -2 {
            fadedir = -2;
        }

        let Some(slot) = self.get_or_alloc(WINDOW_MODE_MAPFADE) else {
            self.fadedir = 0;
            return;
        };

        self.slots[slot].mode = WINDOW_MODE_MAPFADE;
        self.slots[slot].stayblack = 0;

        if fadedir < 0 {
            if self.slots[slot].wm_val > BLACK_FADE_MAX {
                self.slots[slot].wm_val = BLACK_FADE_MAX;
            }
        } else if self.slots[slot].wm_val == 0 {
            self.slots[slot].wm_val = BLACK_FADE_MAX;
        }

        self.fadedir = fadedir;
    }

    /// C `Windows_IsMapFadeActive()` (src/game/windows.c:175).
    pub fn is_map_fade_active(&self) -> bool {
        self.fadedir != 0
    }

    /// C `Windows_FadeToBlack()` (src/game/windows.c:179).
    pub fn fade_to_black(&mut self, speed: i32) {
        let step: i8 = if speed >= 2 { 2 } else { 1 };
        self.start_map_fade(-step);
    }

    /// Begin a semantic fade-to-black from an authored starting intensity.
    ///
    /// The attract shell uses this for the two ENDSEQ transitions that set
    /// the fade counter directly before entering their transfer loops. Map
    /// opcodes continue to use [`Self::fade_to_black`], whose normal/quick
    /// speeds are restricted to one or two intensity steps per tick.
    pub(crate) fn fade_to_black_from(&mut self, rate: MapFadeRate, intensity: u8) {
        let Some(slot) = self.get_or_alloc(WINDOW_MODE_MAPFADE) else {
            self.fadedir = 0;
            return;
        };
        self.slots[slot].mode = WINDOW_MODE_MAPFADE;
        self.slots[slot].stayblack = 0;
        self.slots[slot].wm_val = intensity.min(BLACK_FADE_MAX);
        self.fadedir = rate.to_black_direction();
        self.map_fade_timing = MapFadeTiming::PerSimulationTick;
    }

    /// Begin a source-authored fade whose CPU presentation loop runs at a
    /// different cadence from the port's fixed simulation tick. The source
    /// direction remains intact while the visible intensity reaches black on
    /// the measured whole-machine boundary.
    pub(crate) fn fade_to_black_over(&mut self, rate: MapFadeRate, intensity: u8, total_ticks: u8) {
        assert!(
            total_ticks > 0,
            "presentation fade duration must be nonzero"
        );
        self.fade_to_black_from(rate, intensity);
        self.map_fade_timing = MapFadeTiming::FixedPresentation {
            elapsed_ticks: 0,
            total_ticks,
            start_intensity: intensity.min(BLACK_FADE_MAX),
        };
    }

    /// C `Windows_FadeFromBlack()` (src/game/windows.c:184).
    pub fn fade_from_black(&mut self, speed: i32) {
        let step: i8 = if speed >= 2 { 2 } else { 1 };
        self.start_map_fade(step);
    }

    /// C `Windows_FadeToWhite()` (src/game/windows.c:189).
    pub fn fade_to_white(&mut self, _speed: i32) {
        let Some(slot) = self.get_or_alloc(WINDOW_MODE_WHITEFADE) else {
            return;
        };
        self.slots[slot].mode = WINDOW_MODE_WHITEFADE;
        self.slots[slot].stayblack = 0;
    }

    /// C `initblack_l()` (src/game/windows.c:199).
    ///
    /// C calls `setblack_l()` immediately after setting `stayblack = 40`;
    /// with stayblack just forced to 40, `update_setblack_slot`
    /// (windows.c:43-69) unconditionally takes the `stayblack--` branch and
    /// never reads `g_oncewipe`/`g_circleanim`, so that first step is
    /// inlined here (this keeps the map-VM hook path free of GameVars
    /// borrows; see game.rs `INITBLACK_L`).
    pub fn init_black(&mut self) {
        let Some(slot) = self.get_or_alloc(WINDOW_MODE_BLACK) else {
            return;
        };
        self.slots[slot].mode = WINDOW_MODE_BLACK;
        self.slots[slot].stayblack = 40;
        self.slots[slot].wm_val = BLACK_FADE_MAX;
        // setblack_l() first step: stayblack 40 -> 39 (windows.c:51-54).
        self.slots[slot].stayblack -= 1;
    }

    /// C `setblack_l()` (src/game/windows.c:209).
    pub fn set_black(&mut self, oncewipe: &mut u8, circleanim: &mut i16) {
        if let Some(slot) = self.find_mode(WINDOW_MODE_BLACK) {
            self.update_setblack_slot(slot, oncewipe, circleanim);
        }
    }

    /// C `fadewhite()` (src/game/windows.c:215).
    pub fn fade_white(&mut self) {
        if let Some(slot) = self.get_or_alloc(WINDOW_MODE_WHITEFADE) {
            self.update_fadewhite_slot(slot);
        }
    }

    /// C `initfadewhite2norm_l()` (src/game/windows.c:221).
    pub fn init_fade_white2norm(&mut self) {
        let slot = self
            .find_mode(WINDOW_MODE_WHITEFADE)
            .or_else(|| self.get_or_alloc(WINDOW_MODE_WHITE2NORM));
        let Some(slot) = slot else { return };
        self.slots[slot].mode = WINDOW_MODE_WHITE2NORM;
        self.slots[slot].stayblack = 0;
        self.slots[slot].wm_val = 31;
    }

    /// ROM `fadehalf2norm` (MAIN.ASM:1470) — step half-screen color-math
    /// fade toward normal; dealloc when `wm_val` hits 0. Returns `true` while
    /// the fade is still active.
    pub fn fade_half_to_norm(&mut self) -> bool {
        let Some(slot) = self.get_or_alloc(WINDOW_MODE_HALFFADE) else {
            return false;
        };
        self.slots[slot].mode = WINDOW_MODE_HALFFADE;
        if self.slots[slot].wm_val == 0 {
            self.dealloc(slot);
            return false;
        }
        self.slots[slot].wm_val = self.slots[slot].wm_val.wrapping_sub(1);
        if self.slots[slot].wm_val == 0 {
            self.dealloc(slot);
            return false;
        }
        true
    }

    /// Arm a half-fade at full intensity (ROM alloc path before stepping).
    pub fn start_half_fade(&mut self) {
        if let Some(slot) = self.get_or_alloc(WINDOW_MODE_HALFFADE) {
            self.slots[slot].mode = WINDOW_MODE_HALFFADE;
            if self.slots[slot].wm_val == 0 {
                self.slots[slot].wm_val = 31;
            }
        }
    }

    /// ROM `dyingred_l` (WINDOWS.ASM:166) — red death tint (coldata_r = 10).
    pub fn dying_red(&mut self) {
        let Some(slot) = self.get_or_alloc(WINDOW_MODE_DYINGRED) else {
            return;
        };
        self.slots[slot].mode = WINDOW_MODE_DYINGRED;
        self.slots[slot].wm_val = 10; // ROM coldata_r
        self.slots[slot].stayblack = 1; // tag: dying red (vs boss cyan stayblack=0)
    }

    /// ROM `dyingredoff_l` (WINDOWS.ASM:186).
    pub fn dying_red_off(&mut self) {
        if let Some(slot) = self.find_mode(WINDOW_MODE_DYINGRED) {
            self.dealloc(slot);
        }
    }

    /// ROM `find_window_pri` (TRANS.ASM:344) — lowest set bit in `windowmode`
    /// (first allocated slot). Returns `None` if no windows active.
    pub fn find_window_pri(&self) -> Option<usize> {
        if self.windowmode == 0 {
            return None;
        }
        Some(self.windowmode.trailing_zeros() as usize)
    }

    /// ROM `bossflash_l` (WINDOWS.ASM:240) — cyan dyingred color-math flash.
    /// HD stores intensity in `wm_val` (31 = full); renderer may treat
    /// `WINDOW_MODE_DYINGRED` as a one-shot screen tint.
    pub fn boss_flash(&mut self) {
        let Some(slot) = self.get_or_alloc(WINDOW_MODE_DYINGRED) else {
            return;
        };
        self.slots[slot].mode = WINDOW_MODE_DYINGRED;
        self.slots[slot].wm_val = 31; // ROM coldata_g/b = %11111
        self.slots[slot].stayblack = 0;
    }

    /// Shared `alloc_window hitflash` body (WINDOWS.ASM:104-163).
    /// `wm_val` = coldata intensity; `stayblack` = [`HITFLASH_*`] color tag.
    fn hitflash(&mut self, kind: u8, intensity: u8) {
        let Some(slot) = self.get_or_alloc(WINDOW_MODE_HITFLASH) else {
            return;
        };
        self.slots[slot].mode = WINDOW_MODE_HITFLASH;
        self.slots[slot].wm_val = intensity;
        self.slots[slot].stayblack = kind;
    }

    /// ROM `flashturq_l` (WINDOWS.ASM:104) — full cyan hitflash (g/b = 31).
    pub fn flash_turq(&mut self) {
        self.hitflash(HITFLASH_TURQ, 31);
    }

    /// ROM `flashturq2_l` (WINDOWS.ASM:125) — dim cyan hitflash (g/b = 7).
    pub fn flash_turq2(&mut self) {
        self.hitflash(HITFLASH_TURQ2, 7);
    }

    /// ROM `flashred_l` (WINDOWS.ASM:146) — red hitflash (r = 31).
    pub fn flash_red(&mut self) {
        self.hitflash(HITFLASH_RED, 31);
    }

    /// ROM `dealloc_window hitflash` (GSTRATS.ASM screenflash disable).
    pub fn hitflash_off(&mut self) {
        if let Some(slot) = self.find_mode(WINDOW_MODE_HITFLASH) {
            self.dealloc(slot);
        }
    }

    /// C `fadewhite2norm()` (src/game/windows.c:231).
    pub fn fade_white2norm(&mut self) {
        if let Some(slot) = self.find_mode(WINDOW_MODE_WHITE2NORM) {
            self.update_fadewhite2norm_slot(slot);
        }
    }

    /// ROM `fadetonorm_l` body after clearing `circleanim` — arm white→norm.
    /// Strat wrapper `fade_to_norm_l` also zeros `GameVars::circleanim`.
    pub fn fade_to_norm(&mut self) {
        self.init_fade_white2norm();
    }
}

/// ROM `fadered_l` (MAIN.ASM:2858) — boost red in the first 8×16 BGR555
/// colours of `pal0palette`, then clear colour index `6*16+15`.
///
/// Per colour: keep G/B (`c & 0x7FE0`); if R≤3 force R|=3; R = min(R*2, 31).
pub fn fade_red_palette(palette: &mut [u16]) {
    let n = palette.len().min(8 * 16);
    for c in palette.iter_mut().take(n) {
        let gb = *c & 0x7FE0;
        let mut r = *c & 0x1F;
        if r <= 3 {
            r |= 3;
        }
        r = (r << 1).min(31);
        *c = gb | r;
    }
    const CLEAR_IDX: usize = 6 * 16 + 15; // pal0palette+6*32+30
    if palette.len() > CLEAR_IDX {
        palette[CLEAR_IDX] = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(w: &mut Windows, oncewipe: &mut u8, circleanim: &mut i16, n: usize) {
        for _ in 0..n {
            w.update(oncewipe, circleanim);
        }
    }

    /// Hand-computed against windows.c: FadeToBlack(1) ramps wm_val 0->30
    /// one step per update, then releases g_fadedir.
    #[test]
    fn fade_to_black_ramp() {
        let mut w = Windows::new();
        let (mut ow, mut ca) = (1u8, 0i16);
        w.fade_to_black(1);
        assert_eq!(w.fadedir, -1);
        assert!(w.is_map_fade_active());
        assert_eq!(w.slots[0].mode, WINDOW_MODE_MAPFADE);
        assert_eq!(w.slots[0].wm_val, 0);

        tick(&mut w, &mut ow, &mut ca, 1);
        assert_eq!(w.slots[0].wm_val, 1);
        tick(&mut w, &mut ow, &mut ca, 28);
        assert_eq!(w.slots[0].wm_val, 29);
        assert_eq!(w.fadedir, -1);
        tick(&mut w, &mut ow, &mut ca, 1);
        // wm_val hits 30 -> fadedir released, slot stays allocated
        // (windows.c:96-103).
        assert_eq!(w.slots[0].wm_val, 30);
        assert_eq!(w.fadedir, 0);
        assert_eq!(w.windowmode, 1);
    }

    /// Hand-computed: FadeFromBlack(2) from wm_val=30 needs 15 updates
    /// (30->28->...->2->0), then deallocs the slot and clears fadedir.
    #[test]
    fn fade_from_black_quick() {
        let mut w = Windows::new();
        let (mut ow, mut ca) = (1u8, 0i16);
        w.fade_to_black(1);
        tick(&mut w, &mut ow, &mut ca, 30); // reach fully black
        w.fade_from_black(2);
        assert_eq!(w.fadedir, 2);
        assert_eq!(w.slots[0].wm_val, 30); // nonzero wm_val kept (windows.c:169)

        tick(&mut w, &mut ow, &mut ca, 14);
        assert_eq!(w.slots[0].wm_val, 2);
        assert_eq!(w.fadedir, 2);
        tick(&mut w, &mut ow, &mut ca, 1);
        assert_eq!(w.fadedir, 0);
        assert_eq!(w.windowmode, 0); // deallocated (windows.c:112-115)
        assert!(!w.is_map_fade_active());
    }

    /// ENDSEQ direction -3 is the title's slow-fade mode: it advances one
    /// native intensity level per update rather than treating 3 as a delta.
    #[test]
    fn authored_slow_fade_uses_unit_intensity_steps() {
        let mut w = Windows::new();
        let (mut ow, mut ca) = (1u8, 0i16);
        w.fade_to_black_from(MapFadeRate::Slow, 0);

        let penultimate_intensity = BLACK_FADE_MAX - MapFadeRate::Slow.intensity_step();
        tick(&mut w, &mut ow, &mut ca, usize::from(penultimate_intensity));
        assert_eq!(w.slots[0].wm_val, penultimate_intensity);
        assert!(w.is_map_fade_active());
        tick(&mut w, &mut ow, &mut ca, 1);
        assert_eq!(w.slots[0].wm_val, BLACK_FADE_MAX);
        assert!(!w.is_map_fade_active());
    }

    #[test]
    fn fixed_presentation_fade_retains_normal_mode_and_measured_duration() {
        const PRESENTATION_TICKS: u8 = 16;

        let mut w = Windows::new();
        let (mut ow, mut ca) = (1u8, 0i16);
        w.fade_to_black_over(MapFadeRate::Normal, 0, PRESENTATION_TICKS);

        assert_eq!(w.fadedir, MapFadeRate::Normal.to_black_direction());
        tick(
            &mut w,
            &mut ow,
            &mut ca,
            usize::from(PRESENTATION_TICKS - 1),
        );
        assert_eq!(w.slots[0].wm_val, 28);
        assert!(w.is_map_fade_active());
        tick(&mut w, &mut ow, &mut ca, 1);
        assert_eq!(w.slots[0].wm_val, BLACK_FADE_MAX);
        assert!(!w.is_map_fade_active());
    }

    /// Hand-computed initblack_l sequence: stayblack 40 -> 39 immediately
    /// (inlined setblack_l), then 39 updates to 0, then the oncewipe branch
    /// fires (sets circleanim) and stayblack wraps to 0xFF, and the wrap
    /// test deallocs on the following update (windows.c:43-69).
    #[test]
    fn initblack_wipe_handoff() {
        let mut w = Windows::new();
        let (mut ow, mut ca) = (0u8, 0i16);
        w.init_black();
        assert_eq!(w.slots[0].mode, WINDOW_MODE_BLACK);
        assert_eq!(w.slots[0].stayblack, 39);
        assert_eq!(w.slots[0].wm_val, 30);

        tick(&mut w, &mut ow, &mut ca, 39);
        assert_eq!(w.slots[0].stayblack, 0);
        assert_eq!(ow, 0);

        tick(&mut w, &mut ow, &mut ca, 1);
        // oncewipe==0 branch: oncewipe=1, circleanim=MSTARWIPE_CIRCLE,
        // stayblack wraps 0 -> 0xFF.
        assert_eq!(ow, 1);
        assert_eq!(ca, 1);
        assert_eq!(w.slots[0].stayblack, 0xFF);

        tick(&mut w, &mut ow, &mut ca, 1);
        // test = stayblack + 1 == 0 -> dealloc.
        assert_eq!(w.windowmode, 0);
    }

    /// White fade caps at 31, and white2norm walks back down and deallocs.
    #[test]
    fn white_fade_roundtrip() {
        let mut w = Windows::new();
        let (mut ow, mut ca) = (1u8, 0i16);
        w.fade_to_white(1);
        tick(&mut w, &mut ow, &mut ca, 40);
        assert_eq!(w.slots[0].wm_val, 31); // clamped (windows.c:71-76)

        w.init_fade_white2norm();
        assert_eq!(w.slots[0].mode, WINDOW_MODE_WHITE2NORM);
        assert_eq!(w.slots[0].wm_val, 31);
        tick(&mut w, &mut ow, &mut ca, 30);
        assert_eq!(w.slots[0].wm_val, 1);
        assert_eq!(w.windowmode, 1);
        tick(&mut w, &mut ow, &mut ca, 1);
        assert_eq!(w.windowmode, 0); // wm_val hit 0 -> dealloc
    }

    #[test]
    fn hitflash_turq_red_and_off() {
        let mut w = Windows::new();
        w.flash_turq();
        assert_eq!(w.slots[0].mode, WINDOW_MODE_HITFLASH);
        assert_eq!(w.slots[0].wm_val, 31);
        assert_eq!(w.slots[0].stayblack, HITFLASH_TURQ);

        w.flash_turq2(); // same slot overwrite
        assert_eq!(w.slots[0].wm_val, 7);
        assert_eq!(w.slots[0].stayblack, HITFLASH_TURQ2);

        w.flash_red();
        assert_eq!(w.slots[0].wm_val, 31);
        assert_eq!(w.slots[0].stayblack, HITFLASH_RED);

        w.hitflash_off();
        assert_eq!(w.windowmode, 0);
    }

    #[test]
    fn fade_half_to_norm_steps_and_deallocs() {
        let mut w = Windows::new();
        w.start_half_fade();
        assert_eq!(w.slots[0].mode, WINDOW_MODE_HALFFADE);
        assert_eq!(w.slots[0].wm_val, 31);
        assert!(w.fade_half_to_norm());
        assert_eq!(w.slots[0].wm_val, 30);
        // Drain to zero.
        while w.fade_half_to_norm() {}
        assert_eq!(w.windowmode, 0);
    }
}
