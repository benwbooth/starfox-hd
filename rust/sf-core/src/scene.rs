//! Typed scene-lighting state shared by gameplay and rendering.
//!
//! The original background scripts update four independent concepts: the
//! live polygon palette, the colour-pair family used for distance shading,
//! the distance thresholds, and the ground plane used by projected shadows.
//! Keeping those concepts as enums prevents background identifiers or source
//! addresses from leaking into the renderer.

/// Number of eight-pixel Mode-2 vertical-offset columns in the 256-pixel
/// source display.
pub const BG2_VERTICAL_OFFSET_COLUMNS: usize = 32;
/// Number of independently scrolled background rows in the 256-by-224
/// source display.
pub const BG2_HORIZONTAL_OFFSET_ROWS: usize = 224;

/// Horizontal background transform selected by the active authored scene.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BackgroundHorizontalMode {
    #[default]
    Disabled,
    Rotate,
    Tunnel,
    NoGradient,
    BlackHole,
    Water,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GamePalette {
    #[default]
    Night,
    Red,
    Blue,
}

/// Destination of the scripted Fortuna background-palette walk.
///
/// `fadepalto_l` copies one color per frame from the selected source row into
/// background palette row 4. This is independent of [`GamePalette`], which
/// selects the live polygon palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteFadeTarget {
    Sea,
    Ground,
}

/// Retail `palnum` start: fifteen two-byte palette entries, copied from
/// indices 15 down to 1 while index 0 remains the background backdrop color.
pub const PALETTE_FADE_COUNTER_START: u16 = 30;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DepthColors {
    #[default]
    Night,
    Mist,
    Desert,
    Marine,
    Red,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DepthThresholds {
    #[default]
    Normal,
    Tunnel,
    Mist,
    StageOne,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SceneStyle {
    pub game_palette: GamePalette,
    pub depth_colors: DepthColors,
    pub depth_thresholds: DepthThresholds,
    pub shadow_height: i16,
}
