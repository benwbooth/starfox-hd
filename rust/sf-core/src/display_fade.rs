//! Transfer-completion display fade from SF1 `IRQ.setinidisp`.
//!
//! The authored fade level is separate from the brightness already displayed:
//! `initblack_l` resets the former without immediately rewriting the latter.

pub const FULL_BRIGHTNESS: u8 = 15;
const QUICK_UP: i8 = 2;
const QUICK_DOWN: i8 = -2;
const ALTERNATE_FRAME_DOWN: i8 = -3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayFade {
    pub level: u8,
    pub brightness: u8,
    pub forced_blank: bool,
}

impl DisplayFade {
    pub const fn forced_black() -> Self {
        Self {
            level: 0,
            brightness: 0,
            forced_blank: true,
        }
    }

    pub fn reset_level(&mut self) {
        self.level = 0;
    }

    /// One completed source transfer, not one video refresh. Direction is
    /// shared with the map/strategy owner; terminal and skipped visits retain
    /// the previously published brightness exactly as the source does.
    pub fn advance(&mut self, direction: &mut i8, game_frame: u16) {
        if *direction == 0 {
            return;
        }
        if *direction < 0 {
            if *direction == ALTERNATE_FRAME_DOWN && game_frame & 1 != 0 {
                return;
            }
            let steps = if *direction == QUICK_DOWN { 2 } else { 1 };
            for _ in 0..steps {
                if self.level <= 1 {
                    *direction = 0;
                    *self = Self::forced_black();
                    return;
                }
                self.level -= 1;
            }
        } else {
            let steps = if *direction == QUICK_UP { 3 } else { 1 };
            for _ in 0..steps {
                if self.level == FULL_BRIGHTNESS {
                    *direction = 0;
                    return;
                }
                self.level += 1;
            }
        }
        self.brightness = self.level;
        self.forced_blank = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_reveal_completes_on_the_visit_after_its_final_publication() {
        let mut fade = DisplayFade::forced_black();
        let mut direction = QUICK_UP;
        for expected in [3, 6, 9, 12, 15] {
            fade.advance(&mut direction, 0);
            assert_eq!(fade.brightness, expected);
            assert!(!fade.forced_blank);
            assert_eq!(direction, QUICK_UP);
        }
        fade.advance(&mut direction, 0);
        assert_eq!(direction, 0);
        assert_eq!(fade.brightness, FULL_BRIGHTNESS);
    }

    #[test]
    fn black_initializer_resets_the_fade_level_not_the_visible_brightness() {
        let mut fade = DisplayFade {
            level: 9,
            brightness: 9,
            forced_blank: false,
        };
        fade.reset_level();
        assert_eq!(fade.level, 0);
        assert_eq!(fade.brightness, 9);
        let mut direction = QUICK_UP;
        fade.advance(&mut direction, 0);
        assert_eq!(fade.brightness, 3);
    }

    #[test]
    fn slow_fade_skips_odd_game_frames_and_quick_terminal_branches_retain_output() {
        let mut fade = DisplayFade {
            level: 13,
            brightness: 6,
            forced_blank: false,
        };
        let mut direction = ALTERNATE_FRAME_DOWN;
        fade.advance(&mut direction, 1);
        assert_eq!(fade.level, 13);
        assert_eq!(fade.brightness, 6);
        fade.advance(&mut direction, 2);
        assert_eq!(fade.level, 12);
        assert_eq!(fade.brightness, 12);

        for level in [13, 14, 15] {
            fade.level = level;
            fade.brightness = 6;
            direction = QUICK_UP;
            fade.advance(&mut direction, 0);
            assert_eq!(fade.level, 15);
            assert_eq!(direction, 0);
            assert_eq!(fade.brightness, 6, "terminal branch does not publish");
        }
    }
}
