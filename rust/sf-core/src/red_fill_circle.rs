//! Typed state for the red circular screen fill used by the player-death
//! transition.
//!
//! The retail effect is authored by `redfillscreen_circle` in `TRANS.ASM`.
//! This record keeps the effect's domain values directly; it is not a cursor
//! into the source command table and does not model source-machine memory.

/// Retail radius target configured by `radiusto 197,18,-1`.
pub const RED_FILL_RADIUS_TARGET: u16 = 197;
/// Retail first radial step.
pub const RED_FILL_INITIAL_RADIUS_SPEED: i16 = 18;
/// Retail per-frame radial acceleration.
pub const RED_FILL_RADIUS_ACCELERATION: i16 = -1;
/// Radius at which the source update reaches zero speed and remains.
pub const RED_FILL_SETTLED_RADIUS: u16 = 171;
/// Retail fixed-color red target (five-bit intensity).
pub const RED_FILL_RED_TARGET: u8 = 31;
/// Retail red level installed before the first visible frame.
pub const RED_FILL_INITIAL_RED: u8 = 14;
/// Retail red change per visible frame.
pub const RED_FILL_RED_STEP: u8 = 1;

/// Flat, semantic mirror of the live red-fill circle values.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RedFillCircleState {
    pub radius: u16,
    pub target_radius: u16,
    pub radius_speed: i16,
    pub radius_acceleration: i16,
    pub red: u8,
    pub target_red: u8,
    pub active: bool,
}

impl RedFillCircleState {
    pub const fn inactive() -> Self {
        Self {
            radius: 0,
            target_radius: 0,
            radius_speed: 0,
            radius_acceleration: 0,
            red: 0,
            target_red: 0,
            active: false,
        }
    }

    /// Start the authored effect and produce its first visible record.
    pub fn begin(&mut self) {
        self.radius = 0;
        self.target_radius = RED_FILL_RADIUS_TARGET;
        self.radius_speed = RED_FILL_INITIAL_RADIUS_SPEED;
        self.radius_acceleration = RED_FILL_RADIUS_ACCELERATION;
        self.red = RED_FILL_INITIAL_RED;
        self.target_red = RED_FILL_RED_TARGET;
        self.active = true;
        self.advance();
    }

    pub fn clear(&mut self) {
        *self = Self::inactive();
    }

    /// Advance the live radius and fixed-color intensity by one retail
    /// presentation record. Word wrapping and target clamping match the
    /// original circle update without retaining its command stream.
    pub fn advance(&mut self) {
        if !self.active {
            return;
        }

        if self.radius < self.target_radius {
            let candidate = self.radius.wrapping_add(self.radius_speed as u16);
            self.radius = candidate.min(self.target_radius);
        } else if self.radius > self.target_radius {
            let candidate = self.radius.wrapping_sub(self.radius_speed as u16);
            self.radius = candidate.max(self.target_radius);
        }
        // TRANS.ASM deliberately skips acceleration once `circlespeed` is
        // zero, leaving this particular command settled below its nominal
        // target rather than allowing a negative-speed reversal.
        if self.radius_speed != 0 {
            self.radius_speed = self.radius_speed.wrapping_add(self.radius_acceleration);
        }

        if self.red < self.target_red {
            self.red = self
                .red
                .saturating_add(RED_FILL_RED_STEP)
                .min(self.target_red);
        } else if self.red > self.target_red {
            self.red = self
                .red
                .saturating_sub(RED_FILL_RED_STEP)
                .max(self.target_red);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_produces_the_first_retail_record() {
        let mut state = RedFillCircleState::inactive();
        state.begin();
        assert!(state.active);
        assert_eq!(state.radius, RED_FILL_INITIAL_RADIUS_SPEED as u16);
        assert_eq!(
            state.radius_speed,
            RED_FILL_INITIAL_RADIUS_SPEED + RED_FILL_RADIUS_ACCELERATION
        );
        assert_eq!(state.red, RED_FILL_INITIAL_RED + RED_FILL_RED_STEP);
    }

    #[test]
    fn authored_deceleration_settles_when_the_speed_reaches_zero() {
        let mut state = RedFillCircleState::inactive();
        state.begin();
        for _ in 1..RED_FILL_INITIAL_RADIUS_SPEED {
            state.advance();
        }
        assert_eq!(state.radius, RED_FILL_SETTLED_RADIUS);
        assert_eq!(state.radius_speed, 0);
        assert_eq!(state.red, RED_FILL_RED_TARGET);

        state.advance();
        assert_eq!(state.radius, RED_FILL_SETTLED_RADIUS);
        assert_eq!(state.radius_speed, 0);
    }

    #[test]
    fn clear_removes_all_live_presentation_state() {
        let mut state = RedFillCircleState::inactive();
        state.begin();
        state.clear();
        assert_eq!(state, RedFillCircleState::inactive());
    }
}
