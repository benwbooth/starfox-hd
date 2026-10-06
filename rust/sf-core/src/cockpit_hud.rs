//! Source-authored cockpit reticle (`MHUD.MC`), in local playfield pixels.

use crate::point_field::{PointIdentity, PointPixel};
use crate::snes_trig::{COSTAB, SINTAB};

const CENTER: [i16; 2] = [112, 96];
const ROTATION_SHIFT: u32 = 9;
const PRODUCT_BYTE_SHIFT: u32 = 8;
const PRODUCT_WORD_SHIFT: u32 = 16;
const DAMAGED_WING_COLOR: u8 = 2;
const AUTHORED_LINES: [([i16; 2], [i16; 2]); 4] = [
    ([0, 200], [0, 265]),
    ([250, 0], [315, -32]),
    ([315, -32], [315, 32]),
    ([250, 0], [315, 32]),
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CockpitHudState {
    pub enabled: bool,
    pub roll: u8,
    pub palette_index: u8,
    pub left_wing_broken: bool,
    pub right_wing_broken: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CockpitHudLine {
    pub start: [u8; 2],
    pub end: [u8; 2],
    pub palette_index: u8,
}

impl CockpitHudState {
    /// Source draw order: central pair, then three mirrored arrow pairs.
    /// Damage recolors only the arrows. The opposite side negates BOTH axes
    /// after rotation; negating the authored point first rounds differently.
    pub fn lines(self) -> Option<[CockpitHudLine; 8]> {
        self.enabled.then(|| {
            let sine = i32::from(SINTAB[usize::from(self.roll)]);
            let cosine = i32::from(COSTAB[usize::from(self.roll)]);
            std::array::from_fn(|index| {
                let line = index / 2;
                let mirrored = index % 2 != 0;
                let project = |[x, y]: [i16; 2]| {
                    // Coordinates enter `mrotpnty` in authored (y, x) order.
                    // Its second component subtracts the LOW product halves
                    // in reverse order before subtracting the HIGH halves.
                    // Preserve that source borrow, which differs from a
                    // single widened (y*cos - x*sin) at fractional endpoints.
                    let x = i32::from(x);
                    let y = i32::from(y);
                    let along = (y * cosine) << PRODUCT_BYTE_SHIFT;
                    let cross = (x * sine) << PRODUCT_BYTE_SHIFT;
                    let reverse_borrow = i16::from((cross as u16) < (along as u16));
                    let second = ((along >> PRODUCT_WORD_SHIFT) as i16)
                        .wrapping_sub((cross >> PRODUCT_WORD_SHIFT) as i16)
                        .wrapping_sub(reverse_borrow)
                        >> 1;
                    let rotated = [((y * sine + x * cosine) >> ROTATION_SHIFT) as i16, second];
                    std::array::from_fn(|axis| {
                        let value = if mirrored {
                            -rotated[axis]
                        } else {
                            rotated[axis]
                        };
                        // All authored endpoints lie inside the playfield at
                        // every byte angle; this routine has no clip stage.
                        u8::try_from(CENTER[axis] + value).expect("authored HUD endpoint")
                    })
                };
                let broken = if mirrored {
                    self.left_wing_broken
                } else {
                    self.right_wing_broken
                };
                CockpitHudLine {
                    start: project(AUTHORED_LINES[line].0),
                    end: project(AUTHORED_LINES[line].1),
                    palette_index: if line != 0 && broken {
                        DAMAGED_WING_COLOR
                    } else {
                        self.palette_index & 0x0F
                    },
                }
            })
        })
    }

    /// Discrete reticle pixels for HD presentation; no simulation interpolation.
    pub fn pixels(self) -> Vec<PointPixel> {
        let mut pixels = Vec::new();
        for line in self.lines().into_iter().flatten() {
            if line.palette_index == 0 {
                continue;
            }
            line.visit_pixels(|x, y| {
                pixels.push(PointPixel {
                    x,
                    y,
                    palette_index: line.palette_index,
                    identity: PointIdentity::Untracked,
                })
            });
        }
        pixels
    }
}

impl CockpitHudLine {
    /// `mdodrawline`: test the previous subtraction's carry before taking
    /// each minor-axis step, including both endpoints.
    pub fn visit_pixels(self, mut plot: impl FnMut(u8, u8)) {
        let [mut x, mut y] = self.start.map(i32::from);
        let [end_x, end_y] = self.end.map(i32::from);
        let dx = (end_x - x).abs();
        let dy = (end_y - y).abs();
        let step_x = (end_x - x).signum();
        let step_y = (end_y - y).signum();
        if dx >= dy {
            let mut error = (dx - dy) / 2 - dy;
            for _ in 0..=dx {
                plot(x as u8, y as u8);
                if error < 0 {
                    y += step_y;
                    error += dx;
                }
                x += step_x;
                error -= dy;
            }
        } else {
            let mut error = (dy - dx) / 2 - dx;
            for _ in 0..=dy {
                plot(x as u8, y as u8);
                if error < 0 {
                    x += step_x;
                    error += dy;
                }
                y += step_y;
                error -= dx;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_and_transparent_reticles_emit_no_pixels() {
        assert!(CockpitHudState::default().lines().is_none());
        assert!(CockpitHudState {
            enabled: true,
            ..Default::default()
        }
        .pixels()
        .is_empty());
    }

    #[test]
    fn damage_changes_only_the_corresponding_three_arrow_lines() {
        for roll in 0..=u8::MAX {
            let clean = CockpitHudState {
                enabled: true,
                roll,
                palette_index: 15,
                ..Default::default()
            };
            let intact = clean.lines().unwrap();
            for left in [false, true] {
                for right in [false, true] {
                    let damaged = CockpitHudState {
                        left_wing_broken: left,
                        right_wing_broken: right,
                        ..clean
                    };
                    for (i, line) in damaged.lines().unwrap().into_iter().enumerate() {
                        assert_eq!(line.start, intact[i].start);
                        assert_eq!(line.end, intact[i].end);
                        assert_eq!(
                            line.palette_index,
                            if i >= 2 && if i % 2 == 0 { right } else { left } {
                                2
                            } else {
                                15
                            }
                        );
                        line.visit_pixels(|x, y| assert!(x < 224 && y < 192));
                    }
                }
            }
        }
    }
}
