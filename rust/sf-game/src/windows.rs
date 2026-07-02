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
            let step = (-(self.fadedir as i16)) as u8;
            let next = self.slots[slot].wm_val as u16 + step as u16;
            self.slots[slot].wm_val = if next >= 30 { 30 } else { next as u8 };
            if self.slots[slot].wm_val >= 30 {
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
                _ => {}
            }
        }
    }

    /// C `Windows_StartMapFade()` (src/game/windows.c:147).
    pub fn start_map_fade(&mut self, mut fadedir: i8) {
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
            if self.slots[slot].wm_val > 30 {
                self.slots[slot].wm_val = 30;
            }
        } else if self.slots[slot].wm_val == 0 {
            self.slots[slot].wm_val = 30;
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
        self.slots[slot].wm_val = 30;
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

    /// C `fadewhite2norm()` (src/game/windows.c:231).
    pub fn fade_white2norm(&mut self) {
        if let Some(slot) = self.find_mode(WINDOW_MODE_WHITE2NORM) {
            self.update_fadewhite2norm_slot(slot);
        }
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
}
