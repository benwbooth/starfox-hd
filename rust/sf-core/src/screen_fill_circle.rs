//! Typed state for the circular fixed-colour screen effects authored in
//! `TRANS.ASM`.
//!
//! The port keeps the live radius, RGB colour, semantic phase, and ordinary
//! object identity directly. It does not retain the source command cursor or
//! model source-machine memory.

/// Shared retail radius target configured by `radiusto 197,18,-1`.
pub const EXPANDING_RADIUS_TARGET: u16 = 197;
/// Retail first radial step.
pub const EXPANDING_INITIAL_RADIUS_SPEED: i16 = 18;
/// Retail per-frame radial deceleration.
pub const EXPANDING_RADIUS_ACCELERATION: i16 = -1;
/// Radius at which the decelerating source update reaches zero speed.
pub const EXPANDING_SETTLED_RADIUS: u16 = 171;
/// Highest fixed-colour channel level supported by the source effect.
pub const MAX_COLOR_LEVEL: u8 = 31;
/// Channel level installed before the first visible fill frame.
pub const INITIAL_COLOR_LEVEL: u8 = 14;
/// Per-frame channel change used by both retail fill effects.
pub const COLOR_LEVEL_STEP: u8 = 1;
/// White-fill radius target after its blue and green channels fade.
pub const WHITE_FADE_RADIUS_TARGET: u16 = 10;
/// White-fill inward radial speed.
pub const WHITE_FADE_RADIUS_SPEED: i16 = 4;

/// Center used by the circular overlay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScreenFillCircleCenter {
    /// Source default at the middle of the 256-by-224 playfield.
    #[default]
    Screen,
    /// Stable one-based object identity shared with the draw list.
    Object(u16),
    /// Resolved flat world position supplied at the game/render boundary.
    World { x: i16, y: i16, z: i16 },
}

/// Semantic phase of the two retail fill effects currently used by gameplay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScreenFillCirclePhase {
    #[default]
    Inactive,
    RedExpanding,
    WhiteExpanding,
    WhiteFadingBlueGreen,
    WhiteFadingRed,
}

/// Flat semantic mirror of the live circular fixed-colour presentation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenFillCircleState {
    pub center: ScreenFillCircleCenter,
    pub phase: ScreenFillCirclePhase,
    pub radius: u16,
    pub target_radius: u16,
    pub radius_speed: i16,
    pub radius_acceleration: i16,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub target_red: u8,
    pub target_green: u8,
    pub target_blue: u8,
}

impl ScreenFillCircleState {
    pub const fn inactive() -> Self {
        Self {
            center: ScreenFillCircleCenter::Screen,
            phase: ScreenFillCirclePhase::Inactive,
            radius: 0,
            target_radius: 0,
            radius_speed: 0,
            radius_acceleration: 0,
            red: 0,
            green: 0,
            blue: 0,
            target_red: 0,
            target_green: 0,
            target_blue: 0,
        }
    }

    pub const fn is_active(self) -> bool {
        !matches!(self.phase, ScreenFillCirclePhase::Inactive)
    }

    /// Start the player-death red fill and produce its first visible record.
    pub fn begin_red(&mut self, center: ScreenFillCircleCenter) {
        *self = Self {
            center,
            phase: ScreenFillCirclePhase::RedExpanding,
            radius: 0,
            target_radius: EXPANDING_RADIUS_TARGET,
            radius_speed: EXPANDING_INITIAL_RADIUS_SPEED,
            radius_acceleration: EXPANDING_RADIUS_ACCELERATION,
            red: INITIAL_COLOR_LEVEL,
            green: 0,
            blue: 0,
            target_red: MAX_COLOR_LEVEL,
            target_green: 0,
            target_blue: 0,
        };
        self.advance();
    }

    /// Start the authored white crash fill and produce its first visible
    /// record.
    pub fn begin_white(&mut self, center: ScreenFillCircleCenter) {
        *self = Self {
            center,
            phase: ScreenFillCirclePhase::WhiteExpanding,
            radius: 0,
            target_radius: EXPANDING_RADIUS_TARGET,
            radius_speed: EXPANDING_INITIAL_RADIUS_SPEED,
            radius_acceleration: EXPANDING_RADIUS_ACCELERATION,
            red: INITIAL_COLOR_LEVEL,
            green: INITIAL_COLOR_LEVEL,
            blue: INITIAL_COLOR_LEVEL,
            target_red: MAX_COLOR_LEVEL,
            target_green: MAX_COLOR_LEVEL,
            target_blue: MAX_COLOR_LEVEL,
        };
        self.advance();
    }

    pub fn clear(&mut self) {
        *self = Self::inactive();
    }

    /// Advance one retail presentation record. Phase gates are evaluated
    /// before radius and colour motion, matching the source circle command
    /// loop without retaining that command stream.
    pub fn advance(&mut self) {
        match self.phase {
            ScreenFillCirclePhase::Inactive => return,
            ScreenFillCirclePhase::RedExpanding => {
                // The source waits for radius 300 even though this command
                // decelerates to 171, intentionally leaving the fill live.
            }
            ScreenFillCirclePhase::WhiteExpanding if self.radius == EXPANDING_SETTLED_RADIUS => {
                self.target_blue = 0;
                self.target_green = 0;
                self.phase = ScreenFillCirclePhase::WhiteFadingBlueGreen;
            }
            ScreenFillCirclePhase::WhiteFadingBlueGreen if self.blue == 0 => {
                self.target_radius = WHITE_FADE_RADIUS_TARGET;
                self.radius_speed = WHITE_FADE_RADIUS_SPEED;
                self.radius_acceleration = 0;
                self.target_red = 0;
                self.phase = ScreenFillCirclePhase::WhiteFadingRed;
            }
            ScreenFillCirclePhase::WhiteFadingRed if self.radius == 0 => {
                self.clear();
                return;
            }
            _ => {}
        }

        self.advance_radius();
        self.red = advance_channel(self.red, self.target_red);
        self.green = advance_channel(self.green, self.target_green);
        self.blue = advance_channel(self.blue, self.target_blue);
    }

    fn advance_radius(&mut self) {
        if self.radius < self.target_radius {
            let candidate = self.radius.wrapping_add(self.radius_speed as u16);
            self.radius = candidate.min(self.target_radius);
        } else if self.radius > self.target_radius {
            let candidate = self.radius.wrapping_sub(self.radius_speed as u16);
            self.radius = candidate.max(self.target_radius);
        }
        // TRANS.ASM deliberately skips acceleration once the speed reaches
        // zero, leaving the expanding effects below their nominal target.
        if self.radius_speed != 0 {
            self.radius_speed = self.radius_speed.wrapping_add(self.radius_acceleration);
        }
        // The circle worker substitutes one whenever an update produces zero.
        self.radius = self.radius.max(1);
    }
}

fn advance_channel(current: u8, target: u8) -> u8 {
    if current < target {
        current.saturating_add(COLOR_LEVEL_STEP).min(target)
    } else if current > target {
        current.saturating_sub(COLOR_LEVEL_STEP).max(target)
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn red_fill_begins_with_the_first_object_anchored_record() {
        let mut state = ScreenFillCircleState::inactive();
        state.begin_red(ScreenFillCircleCenter::Object(7));

        assert_eq!(state.center, ScreenFillCircleCenter::Object(7));
        assert_eq!(state.phase, ScreenFillCirclePhase::RedExpanding);
        assert_eq!(state.radius, EXPANDING_INITIAL_RADIUS_SPEED as u16);
        assert_eq!(
            state.radius_speed,
            EXPANDING_INITIAL_RADIUS_SPEED + EXPANDING_RADIUS_ACCELERATION
        );
        assert_eq!(state.red, INITIAL_COLOR_LEVEL + COLOR_LEVEL_STEP);
        assert_eq!([state.green, state.blue], [0, 0]);
    }

    #[test]
    fn red_fill_settles_when_the_authored_speed_reaches_zero() {
        let mut state = ScreenFillCircleState::inactive();
        state.begin_red(ScreenFillCircleCenter::Screen);
        for _ in 1..EXPANDING_INITIAL_RADIUS_SPEED {
            state.advance();
        }

        assert_eq!(state.radius, EXPANDING_SETTLED_RADIUS);
        assert_eq!(state.radius_speed, 0);
        assert_eq!(state.red, MAX_COLOR_LEVEL);
        state.advance();
        assert_eq!(state.radius, EXPANDING_SETTLED_RADIUS);
        assert_eq!(state.phase, ScreenFillCirclePhase::RedExpanding);
    }

    #[test]
    fn white_fill_runs_the_three_authored_colour_and_radius_phases() {
        let mut state = ScreenFillCircleState::inactive();
        state.begin_white(ScreenFillCircleCenter::Object(3));
        for _ in 1..EXPANDING_INITIAL_RADIUS_SPEED {
            state.advance();
        }
        assert_eq!(state.radius, EXPANDING_SETTLED_RADIUS);
        assert_eq!([state.red, state.green, state.blue], [MAX_COLOR_LEVEL; 3]);

        state.advance();
        assert_eq!(state.phase, ScreenFillCirclePhase::WhiteFadingBlueGreen);
        assert_eq!(
            [state.red, state.green, state.blue],
            [MAX_COLOR_LEVEL, MAX_COLOR_LEVEL - 1, MAX_COLOR_LEVEL - 1]
        );
        for _ in 0..MAX_COLOR_LEVEL - 1 {
            state.advance();
        }
        assert_eq!([state.green, state.blue], [0, 0]);

        state.advance();
        assert_eq!(state.phase, ScreenFillCirclePhase::WhiteFadingRed);
        assert_eq!(state.radius, EXPANDING_SETTLED_RADIUS - 4);
        assert_eq!(state.red, MAX_COLOR_LEVEL - 1);
        for _ in 0..MAX_COLOR_LEVEL - 1 {
            state.advance();
        }
        assert_eq!(state.red, 0);

        while state.radius != WHITE_FADE_RADIUS_TARGET {
            state.advance();
        }
        assert!(state.is_active());
        assert_eq!(state.phase, ScreenFillCirclePhase::WhiteFadingRed);
    }

    #[test]
    fn clear_removes_all_live_presentation_state() {
        let mut state = ScreenFillCircleState::inactive();
        state.begin_white(ScreenFillCircleCenter::Object(2));
        state.clear();
        assert_eq!(state, ScreenFillCircleState::inactive());
    }
}
