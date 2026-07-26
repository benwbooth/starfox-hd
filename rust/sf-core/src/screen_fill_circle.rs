//! Typed state for the circular fixed-colour screen effects authored in
//! `TRANS.ASM`.
//!
//! The port keeps the live radius, RGB colour, semantic phase, cadence, and
//! ordinary object identity directly. It does not retain the source command
//! cursor or model source-machine memory.

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
/// Per-frame channel change used by the red and white retail fill effects.
pub const COLOR_LEVEL_STEP: u8 = 1;
/// White-fill radius target after its blue and green channels fade.
pub const WHITE_FADE_RADIUS_TARGET: u16 = 10;
/// White-fill inward radial speed.
pub const WHITE_FADE_RADIUS_SPEED: i16 = 4;
/// Boss and last-stage circle radius at the first phase boundary.
pub const BOSS_FILL_RADIUS: u16 = 200;
/// Boss circle radius target while its blue and green channels fade.
pub const BOSS_FADE_RADIUS_TARGET: u16 = 500;
/// Boss and last-stage radial speed.
pub const BOSS_RADIUS_SPEED: i16 = 4;
/// Initial boss and last-stage channel cadence.
pub const BOSS_COLOR_INTERVAL: u8 = 3;
/// Boss and last-stage channel fade step.
pub const BOSS_FADE_COLOR_STEP: u8 = 2;
/// First smart-bomb flash radius.
pub const SMART_BOMB_FLASH_RADIUS: u16 = 297;
/// First smart-bomb fixed-colour level.
pub const SMART_BOMB_INITIAL_COLOR_LEVEL: u8 = 28;
/// Smart-bomb radius on its second visible record.
pub const SMART_BOMB_IGNITION_RADIUS: u16 = 9;
/// Smart-bomb radius at its inner phase boundary.
pub const SMART_BOMB_INNER_RADIUS: u16 = 135;
/// Smart-bomb radial speed.
pub const SMART_BOMB_RADIUS_SPEED: i16 = 9;
/// Smart-bomb blue brightening cadence.
pub const SMART_BOMB_BLUE_INTERVAL: u8 = 7;
/// Smart-bomb green brightening cadence.
pub const SMART_BOMB_GREEN_INTERVAL: u8 = 15;
/// Smart-bomb red brightening cadence on the ignition record.
pub const SMART_BOMB_RED_INTERVAL: u8 = 3;

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

/// Scene portion affected by the fixed-colour circle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScreenFillCircleScope {
    /// Retail default: the rendered game scene.
    #[default]
    Scene,
    /// Last-stage transition: the background behind game objects.
    Background,
}

/// Semantic phase of a live retail circle effect.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScreenFillCirclePhase {
    #[default]
    Inactive,
    RedExpanding,
    WhiteExpanding,
    WhiteFadingBlueGreen,
    WhiteFadingRed,
    BossExpanding,
    BossFadingBlueGreen,
    BossFadingRed,
    LastStageExpanding,
    LastStageFadingBlueGreen,
    LastStageFadingRed,
    SmartBombFlash,
    SmartBombIgnition,
    SmartBombExpanding,
    SmartBombFading,
}

/// Flat semantic mirror of the live circular fixed-colour presentation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenFillCircleState {
    pub center: ScreenFillCircleCenter,
    pub scope: ScreenFillCircleScope,
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
    pub red_step: u8,
    pub green_step: u8,
    pub blue_step: u8,
    pub red_interval: u8,
    pub green_interval: u8,
    pub blue_interval: u8,
    pub red_frames_remaining: u8,
    pub green_frames_remaining: u8,
    pub blue_frames_remaining: u8,
}

impl ScreenFillCircleState {
    pub const fn inactive() -> Self {
        Self {
            center: ScreenFillCircleCenter::Screen,
            scope: ScreenFillCircleScope::Scene,
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
            red_step: 0,
            green_step: 0,
            blue_step: 0,
            red_interval: 0,
            green_interval: 0,
            blue_interval: 0,
            red_frames_remaining: 0,
            green_frames_remaining: 0,
            blue_frames_remaining: 0,
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
            target_radius: EXPANDING_RADIUS_TARGET,
            radius_speed: EXPANDING_INITIAL_RADIUS_SPEED,
            radius_acceleration: EXPANDING_RADIUS_ACCELERATION,
            red: INITIAL_COLOR_LEVEL,
            target_red: MAX_COLOR_LEVEL,
            red_step: COLOR_LEVEL_STEP,
            red_interval: 1,
            red_frames_remaining: 1,
            ..Self::inactive()
        };
        self.advance();
    }

    /// Start the authored white crash fill and produce its first visible
    /// record.
    pub fn begin_white(&mut self, center: ScreenFillCircleCenter) {
        *self = Self {
            center,
            phase: ScreenFillCirclePhase::WhiteExpanding,
            target_radius: EXPANDING_RADIUS_TARGET,
            radius_speed: EXPANDING_INITIAL_RADIUS_SPEED,
            radius_acceleration: EXPANDING_RADIUS_ACCELERATION,
            red: INITIAL_COLOR_LEVEL,
            green: INITIAL_COLOR_LEVEL,
            blue: INITIAL_COLOR_LEVEL,
            target_red: MAX_COLOR_LEVEL,
            target_green: MAX_COLOR_LEVEL,
            target_blue: MAX_COLOR_LEVEL,
            red_step: COLOR_LEVEL_STEP,
            green_step: COLOR_LEVEL_STEP,
            blue_step: COLOR_LEVEL_STEP,
            red_interval: 1,
            green_interval: 1,
            blue_interval: 1,
            red_frames_remaining: 1,
            green_frames_remaining: 1,
            blue_frames_remaining: 1,
            ..Self::inactive()
        };
        self.advance();
    }

    /// Start the authored boss explosion circle and produce its first visible
    /// record.
    pub fn begin_boss_explosion(&mut self, center: ScreenFillCircleCenter) {
        *self = Self {
            center,
            phase: ScreenFillCirclePhase::BossExpanding,
            target_radius: BOSS_FILL_RADIUS,
            radius_speed: BOSS_RADIUS_SPEED,
            red: INITIAL_COLOR_LEVEL,
            green: INITIAL_COLOR_LEVEL,
            blue: INITIAL_COLOR_LEVEL,
            target_red: MAX_COLOR_LEVEL,
            target_green: MAX_COLOR_LEVEL,
            target_blue: MAX_COLOR_LEVEL,
            red_step: COLOR_LEVEL_STEP,
            green_step: COLOR_LEVEL_STEP,
            blue_step: COLOR_LEVEL_STEP,
            red_interval: BOSS_COLOR_INTERVAL,
            green_interval: BOSS_COLOR_INTERVAL,
            blue_interval: BOSS_COLOR_INTERVAL,
            red_frames_remaining: BOSS_COLOR_INTERVAL,
            green_frames_remaining: BOSS_COLOR_INTERVAL,
            blue_frames_remaining: BOSS_COLOR_INTERVAL,
            ..Self::inactive()
        };
        self.advance();
    }

    /// Start the authored last-stage background circle and produce its first
    /// visible record.
    pub fn begin_last_stage(&mut self, center: ScreenFillCircleCenter) {
        self.begin_boss_explosion(center);
        self.scope = ScreenFillCircleScope::Background;
        self.phase = ScreenFillCirclePhase::LastStageExpanding;
    }

    /// Start the smart-bomb flash. Its first command explicitly presents the
    /// large grey record before the expanding damage-ring presentation.
    pub fn begin_smart_bomb(&mut self, center: ScreenFillCircleCenter) {
        *self = Self {
            center,
            phase: ScreenFillCirclePhase::SmartBombFlash,
            radius: SMART_BOMB_FLASH_RADIUS,
            target_radius: SMART_BOMB_FLASH_RADIUS,
            red: SMART_BOMB_INITIAL_COLOR_LEVEL,
            green: SMART_BOMB_INITIAL_COLOR_LEVEL,
            blue: SMART_BOMB_INITIAL_COLOR_LEVEL,
            target_red: SMART_BOMB_INITIAL_COLOR_LEVEL,
            target_green: SMART_BOMB_INITIAL_COLOR_LEVEL,
            target_blue: SMART_BOMB_INITIAL_COLOR_LEVEL,
            ..Self::inactive()
        };
    }

    pub fn clear(&mut self) {
        *self = Self::inactive();
    }

    /// Advance one retail presentation record. Phase gates are evaluated
    /// before radius and colour motion, matching the source circle semantics
    /// without retaining the source command stream.
    pub fn advance(&mut self) {
        match self.phase {
            ScreenFillCirclePhase::Inactive => return,
            ScreenFillCirclePhase::RedExpanding => {
                // The source waits for radius 300 even though this command
                // decelerates to 171, intentionally leaving the fill live.
            }
            ScreenFillCirclePhase::WhiteExpanding if self.radius == EXPANDING_SETTLED_RADIUS => {
                self.set_blue_transition(0, 1, COLOR_LEVEL_STEP);
                self.set_green_transition(0, 1, COLOR_LEVEL_STEP);
                self.phase = ScreenFillCirclePhase::WhiteFadingBlueGreen;
            }
            ScreenFillCirclePhase::WhiteFadingBlueGreen if self.blue == 0 => {
                self.target_radius = WHITE_FADE_RADIUS_TARGET;
                self.radius_speed = WHITE_FADE_RADIUS_SPEED;
                self.radius_acceleration = 0;
                self.set_red_transition(0, 1, COLOR_LEVEL_STEP);
                self.phase = ScreenFillCirclePhase::WhiteFadingRed;
            }
            ScreenFillCirclePhase::WhiteFadingRed if self.radius == 0 => {
                self.clear();
                return;
            }
            ScreenFillCirclePhase::BossExpanding if self.radius == BOSS_FILL_RADIUS => {
                self.target_radius = BOSS_FADE_RADIUS_TARGET;
                self.set_blue_transition(0, 1, BOSS_FADE_COLOR_STEP);
                self.set_green_transition(0, 1, BOSS_FADE_COLOR_STEP);
                self.phase = ScreenFillCirclePhase::BossFadingBlueGreen;
            }
            ScreenFillCirclePhase::BossFadingBlueGreen if self.blue == 0 => {
                self.set_red_transition(0, 1, BOSS_FADE_COLOR_STEP);
                self.phase = ScreenFillCirclePhase::BossFadingRed;
            }
            ScreenFillCirclePhase::BossFadingRed if self.red == 0 => {
                self.clear();
                return;
            }
            ScreenFillCirclePhase::LastStageExpanding if self.radius == BOSS_FILL_RADIUS => {
                self.set_blue_transition(0, 1, BOSS_FADE_COLOR_STEP);
                self.set_green_transition(0, 1, BOSS_FADE_COLOR_STEP);
                self.phase = ScreenFillCirclePhase::LastStageFadingBlueGreen;
            }
            ScreenFillCirclePhase::LastStageFadingBlueGreen if self.blue == 0 => {
                self.set_red_transition(0, 1, BOSS_FADE_COLOR_STEP);
                self.phase = ScreenFillCirclePhase::LastStageFadingRed;
            }
            ScreenFillCirclePhase::LastStageFadingRed if self.red == 0 => {
                self.clear();
                return;
            }
            ScreenFillCirclePhase::SmartBombFlash => {
                self.radius = 0;
                self.target_radius = SMART_BOMB_IGNITION_RADIUS;
                self.radius_speed = SMART_BOMB_RADIUS_SPEED;
                self.radius_acceleration = 0;
                self.set_blue_transition(
                    MAX_COLOR_LEVEL,
                    SMART_BOMB_BLUE_INTERVAL,
                    COLOR_LEVEL_STEP,
                );
                self.set_green_transition(
                    MAX_COLOR_LEVEL,
                    SMART_BOMB_GREEN_INTERVAL,
                    COLOR_LEVEL_STEP,
                );
                self.set_red_transition(MAX_COLOR_LEVEL, SMART_BOMB_RED_INTERVAL, COLOR_LEVEL_STEP);
                self.phase = ScreenFillCirclePhase::SmartBombIgnition;
            }
            ScreenFillCirclePhase::SmartBombIgnition => {
                self.target_radius = SMART_BOMB_INNER_RADIUS;
                self.set_red_transition(0, 1, BOSS_FADE_COLOR_STEP);
                self.phase = ScreenFillCirclePhase::SmartBombExpanding;
            }
            ScreenFillCirclePhase::SmartBombExpanding if self.radius == SMART_BOMB_INNER_RADIUS => {
                self.target_radius = SMART_BOMB_FLASH_RADIUS;
                self.set_blue_transition(0, 1, COLOR_LEVEL_STEP);
                self.set_green_transition(0, 1, COLOR_LEVEL_STEP);
                self.phase = ScreenFillCirclePhase::SmartBombFading;
            }
            ScreenFillCirclePhase::SmartBombFading if self.green == 0 => {
                self.clear();
                return;
            }
            _ => {}
        }

        self.advance_radius();
        advance_channel(
            &mut self.red,
            self.target_red,
            self.red_step,
            self.red_interval,
            &mut self.red_frames_remaining,
        );
        advance_channel(
            &mut self.green,
            self.target_green,
            self.green_step,
            self.green_interval,
            &mut self.green_frames_remaining,
        );
        advance_channel(
            &mut self.blue,
            self.target_blue,
            self.blue_step,
            self.blue_interval,
            &mut self.blue_frames_remaining,
        );
    }

    fn set_red_transition(&mut self, target: u8, interval: u8, step: u8) {
        self.target_red = target;
        self.red_interval = interval;
        self.red_frames_remaining = interval;
        self.red_step = step;
    }

    fn set_green_transition(&mut self, target: u8, interval: u8, step: u8) {
        self.target_green = target;
        self.green_interval = interval;
        self.green_frames_remaining = interval;
        self.green_step = step;
    }

    fn set_blue_transition(&mut self, target: u8, interval: u8, step: u8) {
        self.target_blue = target;
        self.blue_interval = interval;
        self.blue_frames_remaining = interval;
        self.blue_step = step;
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

fn advance_channel(
    current: &mut u8,
    target: u8,
    step: u8,
    interval: u8,
    frames_remaining: &mut u8,
) {
    if interval == 0 || step == 0 {
        return;
    }
    if *frames_remaining > 1 {
        *frames_remaining -= 1;
        return;
    }
    *frames_remaining = interval;
    *current = if *current < target {
        (*current).saturating_add(step).min(target)
    } else if *current > target {
        (*current).saturating_sub(step).max(target)
    } else {
        *current
    };
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

    #[test]
    fn boss_fill_uses_the_authored_cadence_and_three_phases() {
        const EXPANDING_RECORDS: u16 = BOSS_FILL_RADIUS / BOSS_RADIUS_SPEED as u16;

        let mut state = ScreenFillCircleState::inactive();
        state.begin_boss_explosion(ScreenFillCircleCenter::Object(4));
        assert_eq!(state.radius, BOSS_RADIUS_SPEED as u16);
        assert_eq!(
            [state.red, state.green, state.blue],
            [INITIAL_COLOR_LEVEL; 3]
        );

        for _ in 1..EXPANDING_RECORDS {
            state.advance();
        }
        assert_eq!(state.radius, BOSS_FILL_RADIUS);
        assert_eq!(
            [state.red, state.green, state.blue],
            [MAX_COLOR_LEVEL - COLOR_LEVEL_STEP; 3]
        );

        state.advance();
        assert_eq!(state.phase, ScreenFillCirclePhase::BossFadingBlueGreen);
        assert_eq!(state.radius, BOSS_FILL_RADIUS + BOSS_RADIUS_SPEED as u16);
        assert_eq!(
            [state.red, state.green, state.blue],
            [
                MAX_COLOR_LEVEL,
                MAX_COLOR_LEVEL - BOSS_COLOR_INTERVAL,
                MAX_COLOR_LEVEL - BOSS_COLOR_INTERVAL,
            ]
        );

        while state.blue != 0 {
            state.advance();
        }
        state.advance();
        assert_eq!(state.phase, ScreenFillCirclePhase::BossFadingRed);
        assert_eq!(state.red, MAX_COLOR_LEVEL - BOSS_FADE_COLOR_STEP);
        while state.red != 0 {
            state.advance();
        }
        assert!(state.is_active());
        state.advance();
        assert_eq!(state, ScreenFillCircleState::inactive());
    }

    #[test]
    fn last_stage_fill_keeps_its_radius_and_background_scope() {
        let mut state = ScreenFillCircleState::inactive();
        state.begin_last_stage(ScreenFillCircleCenter::Object(5));
        assert_eq!(state.scope, ScreenFillCircleScope::Background);

        while state.radius != BOSS_FILL_RADIUS {
            state.advance();
        }
        state.advance();
        assert_eq!(state.phase, ScreenFillCirclePhase::LastStageFadingBlueGreen);
        assert_eq!(state.radius, BOSS_FILL_RADIUS);
    }

    #[test]
    fn smart_bomb_preserves_the_two_authored_leading_records() {
        let mut state = ScreenFillCircleState::inactive();
        state.begin_smart_bomb(ScreenFillCircleCenter::Object(6));
        assert_eq!(state.phase, ScreenFillCirclePhase::SmartBombFlash);
        assert_eq!(state.radius, SMART_BOMB_FLASH_RADIUS);
        assert_eq!(
            [state.red, state.green, state.blue],
            [SMART_BOMB_INITIAL_COLOR_LEVEL; 3]
        );

        state.advance();
        assert_eq!(state.phase, ScreenFillCirclePhase::SmartBombIgnition);
        assert_eq!(state.radius, SMART_BOMB_IGNITION_RADIUS);
        assert_eq!(
            [state.red, state.green, state.blue],
            [SMART_BOMB_INITIAL_COLOR_LEVEL; 3]
        );

        state.advance();
        assert_eq!(state.phase, ScreenFillCirclePhase::SmartBombExpanding);
        assert_eq!(
            state.radius,
            SMART_BOMB_IGNITION_RADIUS + SMART_BOMB_RADIUS_SPEED as u16
        );
        assert_eq!(
            [state.red, state.green, state.blue],
            [
                SMART_BOMB_INITIAL_COLOR_LEVEL - BOSS_FADE_COLOR_STEP,
                SMART_BOMB_INITIAL_COLOR_LEVEL,
                SMART_BOMB_INITIAL_COLOR_LEVEL,
            ]
        );

        while state.radius != SMART_BOMB_INNER_RADIUS {
            state.advance();
        }
        state.advance();
        assert_eq!(state.phase, ScreenFillCirclePhase::SmartBombFading);
        while state.green != 0 {
            state.advance();
        }
        state.advance();
        assert_eq!(state, ScreenFillCircleState::inactive());
    }
}
