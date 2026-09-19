//! Hostile-laser path control, transcribed from the authored `$44:EE98..EEEC`
//! program and its `$7F` assembly handlers.
//!
//! The three source continuations are explicit Rust states, not a list of
//! operations indexed by a recorded frame. The owning strategy runs the
//! returned operation and then the registered callbacks before integrating
//! position, in source order. Allocation, collision effects and the separate
//! non-homing weapon path are owned by the weapon/object systems.

use super::path_control::{CountedLoop, PathWait, PlayerCrossing, PlayerTarget, TriggerPeriod};

pub const LAUNCH_DISTANCE_EXCLUSIVE: u16 = 12_000;
pub const HOMING_DISTANCE_EXCLUSIVE: u16 = 1_000;
pub const FLIGHT_SPEED: u8 = 63;
pub const TARGET_CONTRACTIONS: u8 = 3;
pub const CONTRACTION_RADIUS_DELTA: i16 = 127;
pub const AIM_YAW_RADIUS: u8 = 32;
const AIM_LOOP_COUNT: u16 = 40;
const FREE_FLIGHT_WAIT: u8 = 15;
const TERRAIN_WATCH_LIFETIME: u16 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Homing,
    AimLoop(CountedLoop),
    CancelAimAndCoast,
    Coasting(PathWait),
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaserPathStep {
    /// The distant branch contracts three times, faces the selected target,
    /// and reaches the yielding GOTO. Initial speed is written only once.
    DistantHoming {
        initialize_speed: bool,
    },
    /// Immediate face and speed write, trigger registration, then the first
    /// yielding NEXT. There is no additional frame wait between these actions.
    EnterAimLoop,
    ContinueAimLoop,
    Coast,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostileLaserControl {
    phase: Phase,
    entered: bool,
    aim_callbacks: bool,
    terrain_age: u16,
    crossing: PlayerCrossing,
}

impl Default for HostileLaserControl {
    fn default() -> Self {
        Self {
            phase: Phase::Homing,
            entered: false,
            aim_callbacks: false,
            terrain_age: 0,
            crossing: PlayerCrossing::default(),
        }
    }
}

impl HostileLaserControl {
    /// Run straight-line path instructions until a movement yield or END.
    pub fn advance(&mut self, selected_distance: u16) -> LaserPathStep {
        match self.phase {
            Phase::Homing if selected_distance >= HOMING_DISTANCE_EXCLUSIVE => {
                let initialize_speed = !self.entered;
                self.entered = true;
                LaserPathStep::DistantHoming { initialize_speed }
            }
            Phase::Homing => {
                self.entered = true;
                self.aim_callbacks = true;
                let mut iterations = CountedLoop::new(AIM_LOOP_COUNT);
                // DOQUEUE pushes forty; the immediately following NEXT
                // decrements to thirty-nine before the first movement.
                let repeats = iterations.repeat();
                debug_assert!(repeats);
                self.phase = Phase::AimLoop(iterations);
                LaserPathStep::EnterAimLoop
            }
            Phase::AimLoop(mut iterations) => {
                if iterations.repeat() {
                    self.phase = Phase::AimLoop(iterations);
                    LaserPathStep::ContinueAimLoop
                } else {
                    self.phase = Phase::End;
                    LaserPathStep::End
                }
            }
            Phase::CancelAimAndCoast => {
                self.aim_callbacks = false;
                let mut wait = PathWait::default();
                let ready = wait.ready(FREE_FLIGHT_WAIT);
                debug_assert!(!ready);
                self.phase = Phase::Coasting(wait);
                LaserPathStep::Coast
            }
            Phase::Coasting(mut wait) => {
                if wait.ready(FREE_FLIGHT_WAIT) {
                    self.phase = Phase::End;
                    LaserPathStep::End
                } else {
                    self.phase = Phase::Coasting(wait);
                    LaserPathStep::Coast
                }
            }
            Phase::End => LaserPathStep::End,
        }
    }

    /// The always callback runs before aim/crossing callbacks. Terrain or the
    /// exact age boundary installs END for the *next* path invocation; the
    /// current movement still finishes. A later callback may replace it.
    /// Returns true when the source also resets the actor's path counter and
    /// auxiliary steering byte (`$17/$15`), not the visible roll (`$16`).
    pub fn watch_terrain(&mut self, occupied: bool) -> bool {
        if !occupied {
            self.terrain_age = self.terrain_age.wrapping_add(1);
        }
        if occupied || self.terrain_age == TERRAIN_WATCH_LIFETIME {
            self.phase = Phase::End;
            true
        } else {
            false
        }
    }

    /// Execute the smooth-face operation only if this gate and the authored
    /// yaw-arc test both pass. Use the shared strategy tick, not spawn age.
    pub fn smooth_aim_due(&self, strategy_tick: u8) -> bool {
        self.aim_callbacks && TriggerPeriod::Two.due(strategy_tick)
    }

    /// Run after the smooth-face callback, with freshly projected player
    /// positions. A crossing changes selected player and resets path count
    /// and auxiliary steering state in the owner. Cancellation happens on the
    /// next path advance.
    pub fn crossed_player(&mut self, projections: [Option<i16>; 2]) -> Option<PlayerTarget> {
        if !self.aim_callbacks {
            return None;
        }
        let target = self.crossing.sample(projections)?;
        self.phase = Phase::CancelAimAndCoast;
        Some(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_homing_threshold_is_the_authored_word_not_a_sampled_radius() {
        for distance in [1_000, 1_001, 1_023, 1_024, u16::MAX] {
            let mut state = HostileLaserControl::default();
            assert_eq!(
                state.advance(distance),
                LaserPathStep::DistantHoming {
                    initialize_speed: true
                }
            );
            assert_eq!(
                state.advance(distance),
                LaserPathStep::DistantHoming {
                    initialize_speed: false
                }
            );
            assert_eq!(state.advance(999), LaserPathStep::EnterAimLoop);
        }
    }

    #[test]
    fn exhausted_aim_loop_ends_without_inventing_a_cruise_phase() {
        let mut state = HostileLaserControl::default();
        assert_eq!(state.advance(0), LaserPathStep::EnterAimLoop);
        for _ in 0..38 {
            assert_eq!(state.advance(0), LaserPathStep::ContinueAimLoop);
        }
        assert_eq!(state.advance(0), LaserPathStep::End);
        assert_eq!(state.advance(0), LaserPathStep::End);
    }

    #[test]
    fn crossing_alone_installs_the_fifteen_movement_wait() {
        let mut state = HostileLaserControl::default();
        assert_eq!(state.advance(0), LaserPathStep::EnterAimLoop);
        assert_eq!(state.crossed_player([Some(1), Some(1)]), None);
        assert_eq!(
            state.crossed_player([Some(1), Some(-1)]),
            Some(PlayerTarget::Secondary)
        );
        // Source callbacks remain registered until the forced continuation
        // actually executes its two CANCEL operations.
        assert!(state.smooth_aim_due(2));
        for _ in 0..15 {
            assert_eq!(state.advance(0), LaserPathStep::Coast);
            assert!(!state.smooth_aim_due(2));
        }
        assert_eq!(state.advance(0), LaserPathStep::End);
    }

    #[test]
    fn terrain_callback_installs_end_and_does_not_skip_current_callbacks() {
        let mut state = HostileLaserControl::default();
        state.advance(0);
        assert_eq!(state.crossed_player([Some(1), None]), None);
        assert!(state.watch_terrain(true));
        assert!(state.smooth_aim_due(2));
        // The later crossing callback wins if both force a continuation.
        assert_eq!(
            state.crossed_player([Some(-1), None]),
            Some(PlayerTarget::Primary)
        );
        assert_eq!(state.advance(0), LaserPathStep::Coast);
    }

    #[test]
    fn terrain_age_is_an_exact_word_counter_boundary() {
        let mut state = HostileLaserControl::default();
        for _ in 0..59 {
            assert!(!state.watch_terrain(false));
        }
        assert!(state.watch_terrain(false));
        assert_eq!(state.advance(u16::MAX), LaserPathStep::End);
    }
}
