//! Authored opening-scene timing and palette effects, independent of display
//! frame rate. This controller contains game-domain actions, not machine code.

use super::intro_camera::OpeningCameraCue;

pub const INTRO_PALETTE_COLORS: usize = 128;
const EFFECT_PALETTE_START: usize = 64;
const PALETTE_ROW_COLORS: usize = 16;
const COLOR_CHANNEL_BITS: u32 = 5;
const COLOR_CHANNEL_MAX: u8 = 31;
const FLASH_RED_STEP: u8 = 3;
const FLASH_OTHER_LIMIT: u8 = 28;
const LOGO_RAMP_START: usize = 113;
const LOGO_RAMP: [IntroColor; 4] = [
    IntroColor::new(31, 27, 5),
    IntroColor::new(31, 23, 3),
    IntroColor::new(30, 19, 2),
    IntroColor::new(27, 16, 1),
];

/// Original artwork colors retain five-bit components. Conversion to the
/// renderer's color space happens at presentation, not during game updates.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IntroColor {
    red: u8,
    green: u8,
    blue: u8,
}

impl IntroColor {
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        assert!(
            red <= COLOR_CHANNEL_MAX && green <= COLOR_CHANNEL_MAX && blue <= COLOR_CHANNEL_MAX
        );
        Self { red, green, blue }
    }

    pub const fn from_bgr555(packed: u16) -> Self {
        Self {
            red: packed as u8 & COLOR_CHANNEL_MAX,
            green: (packed >> COLOR_CHANNEL_BITS) as u8 & COLOR_CHANNEL_MAX,
            blue: (packed >> (COLOR_CHANNEL_BITS * 2)) as u8 & COLOR_CHANNEL_MAX,
        }
    }

    pub const fn bgr555(self) -> u16 {
        self.red as u16
            | ((self.green as u16) << COLOR_CHANNEL_BITS)
            | ((self.blue as u16) << (COLOR_CHANNEL_BITS * 2))
    }

    fn flash(self) -> Self {
        Self {
            red: (self.red + FLASH_RED_STEP).min(COLOR_CHANNEL_MAX),
            green: (self.green + 1).min(FLASH_OTHER_LIMIT),
            blue: (self.blue + 1).min(FLASH_OTHER_LIMIT),
        }
    }

    fn restore_toward(self, target: Self) -> Self {
        fn step(value: u8, target: u8) -> u8 {
            match value.cmp(&target) {
                std::cmp::Ordering::Less => value + 1,
                std::cmp::Ordering::Equal => value,
                std::cmp::Ordering::Greater => value - 1,
            }
        }
        Self {
            red: step(self.red, target.red),
            green: step(self.green, target.green),
            blue: step(self.blue, target.blue),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IntroPaletteEffectState {
    pub restoring: bool,
    pub persistent_highlight: bool,
    pub highlighted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningScenePalette {
    pub colors: [IntroColor; INTRO_PALETTE_COLORS],
    pub saved_colors: [IntroColor; INTRO_PALETTE_COLORS],
    pub effects: IntroPaletteEffectState,
    pub refresh_requested: bool,
}

impl OpeningScenePalette {
    pub fn new(colors: [IntroColor; INTRO_PALETTE_COLORS]) -> Self {
        Self {
            colors,
            saved_colors: [IntroColor::default(); INTRO_PALETTE_COLORS],
            effects: IntroPaletteEffectState::default(),
            refresh_requested: false,
        }
    }

    /// Commit the background loader job without changing foreground, polygon,
    /// saved colors or effect policy. The scheduler owns the completion time.
    pub fn install_background(
        &mut self,
        artwork: &sf2_data::opening_artwork::OpeningArtworkPalettes,
    ) {
        for (target, packed) in self.colors.iter_mut().zip(&artwork.background) {
            *target = IntroColor::from_bgr555(*packed);
        }
    }

    /// Commit the separate foreground loader job. Sprite colors remain a
    /// separate asset block; colors 112..127 belong to the polygon/effect ramp.
    pub fn install_foreground(
        &mut self,
        artwork: &sf2_data::opening_artwork::OpeningArtworkPalettes,
        id: sf2_data::opening_artwork::ForegroundPaletteId,
    ) {
        let start = sf2_data::opening_artwork::BACKGROUND_COLORS;
        for (target, packed) in self.colors[start..].iter_mut().zip(artwork.foreground(id)) {
            *target = IntroColor::from_bgr555(*packed);
        }
    }

    /// Install the polygon ramp independently of the artwork palette blocks.
    pub fn install_polygon_palette(&mut self, id: sf2_data::palettes::PolygonPaletteId) {
        let start = INTRO_PALETTE_COLORS - sf2_data::palettes::COLORS_PER_POLYGON_PALETTE;
        for (target, packed) in self.colors[start..].iter_mut().zip(id.colors()) {
            *target = IntroColor::from_bgr555(*packed);
        }
    }

    pub fn prepare_logo(&mut self) {
        self.saved_colors = self.colors;
        self.colors[LOGO_RAMP_START..LOGO_RAMP_START + LOGO_RAMP.len()].copy_from_slice(&LOGO_RAMP);
    }

    pub fn flash(&mut self) {
        self.refresh_requested = true;
        for (index, color) in self
            .colors
            .iter_mut()
            .enumerate()
            .skip(EFFECT_PALETTE_START)
        {
            // The first color in each artwork palette row is unaffected by
            // the flash. Restoration below deliberately includes those colors.
            if index % PALETTE_ROW_COLORS != 0 {
                *color = color.flash();
            }
        }
    }

    pub fn restore_step(&mut self) {
        self.refresh_requested = true;
        let mut changed = false;
        for (current, target) in self.colors[EFFECT_PALETTE_START..]
            .iter_mut()
            .zip(&self.saved_colors[EFFECT_PALETTE_START..])
        {
            changed |= *current != *target;
            *current = current.restore_toward(*target);
        }
        // Reaching the target on this step is not completion yet. The source
        // clears its effect policy only on a step that changed no colors.
        if !changed {
            self.effects.restoring = false;
            if !self.effects.persistent_highlight {
                self.effects.highlighted = false;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningEventTiming {
    At(u16),
    Interval { start: u16, end: u16 },
}

impl OpeningEventTiming {
    pub fn applies(self, update: u16) -> bool {
        match self {
            Self::At(at) => update == at,
            Self::Interval { start, end } => (start..end).contains(&update),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSceneAction {
    PrepareLogoPalette,
    RestorePalette { steps: u8 },
    AdvanceCameraCue,
    RequestNextScene,
    FlashPalette,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningTimedAction {
    pub timing: OpeningEventTiming,
    pub action: OpeningSceneAction,
}

/// The original controller revisits its authored action list each update.
/// Preserve list order: it is not sorted by start time.
pub const OPENING_SCENE_ACTIONS: [OpeningTimedAction; 14] = {
    use OpeningEventTiming::{At, Interval};
    use OpeningSceneAction::*;
    [
        OpeningTimedAction {
            timing: At(14),
            action: PrepareLogoPalette,
        },
        OpeningTimedAction {
            timing: Interval {
                start: 107,
                end: 139,
            },
            action: RestorePalette { steps: 2 },
        },
        OpeningTimedAction {
            timing: At(182),
            action: AdvanceCameraCue,
        },
        OpeningTimedAction {
            timing: At(249),
            action: AdvanceCameraCue,
        },
        OpeningTimedAction {
            timing: At(293),
            action: AdvanceCameraCue,
        },
        OpeningTimedAction {
            timing: At(327),
            action: AdvanceCameraCue,
        },
        OpeningTimedAction {
            timing: At(416),
            action: AdvanceCameraCue,
        },
        OpeningTimedAction {
            timing: At(441),
            action: RequestNextScene,
        },
        OpeningTimedAction {
            timing: Interval {
                start: 169,
                end: 185,
            },
            action: FlashPalette,
        },
        OpeningTimedAction {
            timing: Interval {
                start: 185,
                end: 217,
            },
            action: RestorePalette { steps: 1 },
        },
        OpeningTimedAction {
            timing: Interval {
                start: 314,
                end: 318,
            },
            action: FlashPalette,
        },
        OpeningTimedAction {
            timing: Interval {
                start: 324,
                end: 356,
            },
            action: RestorePalette { steps: 1 },
        },
        OpeningTimedAction {
            timing: Interval {
                start: 409,
                end: 413,
            },
            action: FlashPalette,
        },
        OpeningTimedAction {
            timing: Interval {
                start: 417,
                end: 449,
            },
            action: RestorePalette { steps: 1 },
        },
    ]
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningSceneController {
    elapsed_updates: u16,
    updates_since_cut: u16,
    cue: OpeningCameraCue,
    pub transition_requested: bool,
}

impl OpeningSceneController {
    pub fn elapsed_updates(&self) -> u16 {
        self.elapsed_updates
    }
    pub fn updates_since_cut(&self) -> u16 {
        self.updates_since_cut
    }
    pub fn cue(&self) -> OpeningCameraCue {
        self.cue
    }

    /// Advance only when the scene controller runs, not on each display frame.
    /// Pausing presentation or skipping this controller must not advance time.
    pub fn tick(&mut self, palette: &mut OpeningScenePalette) {
        for event in OPENING_SCENE_ACTIONS {
            if !event.timing.applies(self.elapsed_updates) {
                continue;
            }
            match event.action {
                OpeningSceneAction::PrepareLogoPalette => palette.prepare_logo(),
                OpeningSceneAction::RestorePalette { steps } => {
                    for _ in 0..steps {
                        palette.restore_step();
                    }
                }
                OpeningSceneAction::FlashPalette => palette.flash(),
                OpeningSceneAction::RequestNextScene => self.transition_requested = true,
                OpeningSceneAction::AdvanceCameraCue => {
                    self.cue = match self.cue {
                        OpeningCameraCue::Opening => OpeningCameraCue::FirstCut,
                        OpeningCameraCue::FirstCut => OpeningCameraCue::SecondCut,
                        OpeningCameraCue::SecondCut => OpeningCameraCue::ThirdCut,
                        OpeningCameraCue::ThirdCut => OpeningCameraCue::FourthCut,
                        OpeningCameraCue::FourthCut => OpeningCameraCue::FinalCut,
                        OpeningCameraCue::FinalCut => unreachable!("opening has only five cuts"),
                    };
                    self.updates_since_cut = 0;
                }
            }
        }
        if self.elapsed_updates != u16::MAX {
            self.elapsed_updates += 1;
            self.updates_since_cut = self.updates_since_cut.wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_artwork_installs_preserve_other_layers_and_policy() {
        use sf2_data::opening_artwork::{ForegroundPaletteId, OpeningArtworkPalettes};
        let mut bytes = vec![0; 0x24C0];
        for (start, count, color) in [
            (0x80, 64, 0x1234u16),
            (0x400, 48, 0x0101),
            (0x7C0, 48, 0x0202),
            (0x760, 48, 0x0303),
        ] {
            for word in bytes[start..start + count * 2].chunks_exact_mut(2) {
                word.copy_from_slice(&color.to_le_bytes());
            }
        }
        let artwork = OpeningArtworkPalettes::from_decoded(&bytes).unwrap();
        for (variant, expected) in [
            (ForegroundPaletteId::Standard, 0x0101),
            (ForegroundPaletteId::CatalogOne, 0x0202),
            (ForegroundPaletteId::CatalogTwo, 0x0303),
        ] {
            let mut palette =
                OpeningScenePalette::new([IntroColor::new(1, 2, 3); INTRO_PALETTE_COLORS]);
            palette.saved_colors = [IntroColor::new(4, 5, 6); INTRO_PALETTE_COLORS];
            palette.effects = IntroPaletteEffectState {
                restoring: true,
                persistent_highlight: true,
                highlighted: true,
            };
            palette.refresh_requested = true;
            let before = palette.clone();
            palette.install_background(&artwork);
            assert!(palette.colors[..64].iter().all(|c| c.bgr555() == 0x1234));
            assert_eq!(palette.colors[64..], before.colors[64..]);
            let after_background = palette.clone();
            palette.install_foreground(&artwork, variant);
            assert_eq!(palette.colors[..64], after_background.colors[..64]);
            assert!(palette.colors[64..112]
                .iter()
                .all(|c| c.bgr555() == expected));
            assert_eq!(palette.colors[112..], before.colors[112..]);
            assert_eq!(palette.saved_colors, before.saved_colors);
            assert_eq!(palette.effects, before.effects);
            assert_eq!(palette.refresh_requested, before.refresh_requested);
        }
    }

    #[test]
    fn restoration_finishes_on_the_first_unchanged_step() {
        let mut palette =
            OpeningScenePalette::new([IntroColor::new(1, 1, 1); INTRO_PALETTE_COLORS]);
        palette.effects.restoring = true;
        palette.effects.highlighted = true;
        palette.restore_step();
        assert!(palette.effects.restoring);
        palette.restore_step();
        assert!(!palette.effects.restoring);
        assert!(!palette.effects.highlighted);
    }

    #[test]
    fn flash_has_warm_channel_limits_and_preserves_row_origins() {
        let original = IntroColor::new(30, 31, 31);
        let mut palette = OpeningScenePalette::new([original; INTRO_PALETTE_COLORS]);
        palette.flash();
        assert_eq!(palette.colors[EFFECT_PALETTE_START], original);
        assert_eq!(
            palette.colors[EFFECT_PALETTE_START + 1],
            IntroColor::new(31, 28, 28)
        );
    }
}
