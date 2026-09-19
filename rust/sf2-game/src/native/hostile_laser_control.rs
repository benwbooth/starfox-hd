//! Hostile-laser path control, transcribed from the authored `$44:EE98..EF29`
//! program and its `$7F` assembly handlers.
//!
//! The three source continuations are explicit Rust states, not a list of
//! operations indexed by a recorded frame. The owning strategy runs the
//! returned operation, ordinary movement, and then the registered callbacks,
//! in source order. Allocation and collision-world queries are owned by the
//! weapon/object systems; both homing and ballistic continuations live here.

use super::path_control::{CountedLoop, PathWait, PlayerCrossing, PlayerTarget, TriggerPeriod};
use super::{path_control, path_math, path_motion, Object, Rotation, Vector3};

pub const LAUNCH_DISTANCE_EXCLUSIVE: u16 = 12_000;
pub const HOMING_DISTANCE_EXCLUSIVE: u16 = 1_000;
pub const FLIGHT_SPEED: u8 = 63;
pub const TARGET_CONTRACTIONS: u8 = 3;
pub const CONTRACTION_RADIUS_DELTA: i16 = 127;
pub const AIM_YAW_RADIUS: u8 = 32;
const AIM_LOOP_COUNT: u16 = 40;
const FREE_FLIGHT_WAIT: u8 = 15;
const TERRAIN_WATCH_LIFETIME: u16 = 60;
const WORLD_MOVEMENT_SCALE: i16 = 4;

/// Live world inputs, sampled by the object scheduler for one path call.
/// Occupancy is queried separately, after movement, at the new position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaserFlightInput {
    pub selected_position: Vector3,
    pub player_positions: [Option<Vector3>; 2],
    pub strategy_tick: u8,
    pub hit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaserFlightStep {
    End,
    Moved {
        /// FORCE resets the owner's ordinary wait and counted-loop bytes.
        reset_path_counters: bool,
        selected_player: Option<PlayerTarget>,
    },
}

/// The non-homing continuation (`$44:EEFE..EF29`) retains its launch speed
/// and faces neither player. The loop count comes from the authored lifetime
/// byte, widened to the source's word-sized counted-loop stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BallisticLaserControl {
    iterations: CountedLoop,
    check_ground_plane: bool,
    ended: bool,
}

impl BallisticLaserControl {
    pub const fn new(lifetime: u8, check_ground_plane: bool) -> Self {
        Self {
            iterations: CountedLoop::new(lifetime as u16),
            check_ground_plane,
            ended: false,
        }
    }

    /// Query collision-world height only after movement and only if the
    /// optional ground-plane branch did not already force END. The height
    /// comparison is a wrapped signed word subtraction, not a widened one.
    pub fn step(
        &mut self,
        object: &mut Object,
        hit: bool,
        collision_height_at: impl FnOnce(Vector3) -> i16,
    ) -> LaserFlightStep {
        if self.ended || !self.iterations.repeat() {
            self.ended = true;
            return LaserFlightStep::End;
        }
        object.base.velocity = path_motion::direction_velocity(
            object.base.pitch,
            object.base.yaw,
            object.base.speed,
            WORLD_MOVEMENT_SCALE,
        );
        path_motion::integrate(&mut object.base.position, object.base.velocity);
        let surface_hit = (self.check_ground_plane && object.base.position.y >= 0)
            || object
                .base
                .position
                .y
                .wrapping_sub(collision_height_at(object.base.position))
                >= 0;
        self.ended = hit || surface_hit;
        LaserFlightStep::Moved {
            reset_path_counters: self.ended,
            selected_player: None,
        }
    }
}

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
    /// Execute the homing weapon path, movement and registered callbacks.
    /// The occupancy service must apply the selected player's map-test
    /// exemption, exactly as `$7F:B73E` does. This method deliberately does
    /// not invent an empty world, a collision result, or a per-object clock.
    /// Allocation and retirement belong to the calling object scheduler.
    pub fn step(
        &mut self,
        object: &mut Object,
        input: LaserFlightInput,
        occupied_at: impl FnOnce(Vector3) -> bool,
    ) -> LaserFlightStep {
        let distance = path_math::vector_length(Vector3 {
            x: input
                .selected_position
                .x
                .wrapping_sub(object.base.position.x),
            y: 0,
            z: input
                .selected_position
                .z
                .wrapping_sub(object.base.position.z),
        });
        match self.advance(distance) {
            LaserPathStep::End => return LaserFlightStep::End,
            LaserPathStep::DistantHoming { initialize_speed } => {
                if initialize_speed {
                    object.base.speed = FLIGHT_SPEED;
                }
                for _ in 0..TARGET_CONTRACTIONS {
                    object.base.position = path_math::change_radius(
                        object.base.position,
                        input.selected_position,
                        CONTRACTION_RADIUS_DELTA,
                    );
                }
                (object.base.pitch, object.base.yaw) =
                    path_control::target_angles(object.base.position, input.selected_position);
            }
            LaserPathStep::EnterAimLoop => {
                (object.base.pitch, object.base.yaw) =
                    path_control::target_angles(object.base.position, input.selected_position);
                object.base.speed = FLIGHT_SPEED;
            }
            LaserPathStep::ContinueAimLoop | LaserPathStep::Coast => {}
        }

        object.base.velocity = path_motion::direction_velocity(
            object.base.pitch,
            object.base.yaw,
            object.base.speed,
            WORLD_MOVEMENT_SCALE,
        );
        path_motion::integrate(&mut object.base.position, object.base.velocity);

        // Hit was registered first ($EE7B); always-watch follows ($EE98).
        // Neither ends this invocation early. Later FORCE callbacks win.
        let mut reset_path_counters = input.hit;
        if input.hit {
            self.phase = Phase::End;
        }
        reset_path_counters |= self.watch_terrain(occupied_at(object.base.position));
        if self.smooth_aim_due(input.strategy_tick) {
            let (pitch, yaw) =
                path_control::target_angles(object.base.position, input.selected_position);
            if path_control::within_yaw_arc(object.base.yaw, yaw, AIM_YAW_RADIUS) {
                object.base.pitch = path_control::smooth_face_angle(object.base.pitch, pitch);
                object.base.yaw = path_control::smooth_face_angle(object.base.yaw, yaw);
            }
        }
        let rotation = Rotation {
            pitch: object.base.pitch,
            yaw: object.base.yaw,
            roll: object.base.roll,
        };
        let projections = input.player_positions.map(|target| {
            target.map(|target| {
                path_control::forward_plane_projection(object.base.position, rotation, target)
            })
        });
        let selected_player = self.crossed_player(projections);
        reset_path_counters |= selected_player.is_some();
        LaserFlightStep::Moved {
            reset_path_counters,
            selected_player,
        }
    }

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
    use crate::{Behavior, ObjectKind, ShapeId};

    fn laser() -> Object {
        Object::new(
            ObjectKind::Projectile,
            ShapeId::ENEMY_LASER,
            Behavior::Projectile,
        )
    }

    fn flight_input() -> LaserFlightInput {
        LaserFlightInput {
            selected_position: Vector3 { x: 0, y: 0, z: 500 },
            player_positions: [None, None],
            strategy_tick: 1,
            hit: false,
        }
    }

    #[test]
    fn ballistic_loop_keeps_launch_speed_and_has_no_homing_age_cap() {
        let mut state = BallisticLaserControl::new(80, false);
        let mut object = laser();
        object.base.position.y = -100;
        object.base.speed = 30;
        for _ in 0..79 {
            assert_eq!(
                state.step(&mut object, false, |_| 0),
                LaserFlightStep::Moved {
                    reset_path_counters: false,
                    selected_player: None
                }
            );
            assert_eq!(object.base.speed, 30);
        }
        assert_eq!(state.step(&mut object, false, |_| 0), LaserFlightStep::End);
    }

    #[test]
    fn ballistic_ground_branch_short_circuits_collision_height_lookup() {
        let mut state = BallisticLaserControl::new(45, true);
        let mut object = laser();
        assert_eq!(
            state.step(&mut object, false, |_| panic!(
                "ground branch already succeeded"
            )),
            LaserFlightStep::Moved {
                reset_path_counters: true,
                selected_player: None
            }
        );
        assert_eq!(state.step(&mut object, false, |_| 0), LaserFlightStep::End);
    }

    #[test]
    fn ballistic_surface_test_preserves_word_subtraction_overflow() {
        let mut state = BallisticLaserControl::new(45, false);
        let mut object = laser();
        object.base.position.y = i16::MIN;
        // -32768 - 1 wraps positive, so this source predicate fires.
        assert_eq!(
            state.step(&mut object, false, |_| 1),
            LaserFlightStep::Moved {
                reset_path_counters: true,
                selected_player: None
            }
        );
    }

    #[test]
    fn full_aim_path_moves_thirty_nine_times_and_end_does_not_query_world() {
        let mut state = HostileLaserControl::default();
        let mut object = laser();
        for _ in 0..39 {
            assert!(matches!(
                state.step(&mut object, flight_input(), |_| false),
                LaserFlightStep::Moved { .. }
            ));
        }
        let last_position = object.base.position;
        assert_eq!(
            state.step(&mut object, flight_input(), |_| panic!(
                "END has no callbacks"
            )),
            LaserFlightStep::End
        );
        assert_eq!(last_position, object.base.position);
    }

    #[test]
    fn map_query_observes_post_movement_position_and_forces_next_call_to_end() {
        let mut state = HostileLaserControl::default();
        let mut object = laser();
        let mut queried = None;
        assert_eq!(
            state.step(&mut object, flight_input(), |position| {
                queried = Some(position);
                position.z > 200
            }),
            LaserFlightStep::Moved {
                reset_path_counters: true,
                selected_player: None
            }
        );
        assert_eq!(queried, Some(object.base.position));
        assert_eq!(state.advance(0), LaserPathStep::End);
    }

    #[test]
    fn later_crossing_callback_overrides_hit_termination_in_same_movement() {
        let mut state = HostileLaserControl::default();
        let mut object = laser();
        let mut input = flight_input();
        input.player_positions[0] = Some(input.selected_position);
        state.step(&mut object, input, |_| false);
        input.hit = true;
        input.player_positions[0] = Some(Vector3::default());
        assert_eq!(
            state.step(&mut object, input, |_| false),
            LaserFlightStep::Moved {
                reset_path_counters: true,
                selected_player: Some(PlayerTarget::Primary),
            }
        );
        assert_eq!(state.advance(0), LaserPathStep::Coast);
    }

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
