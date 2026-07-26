//! Typed continuation rules for lasers fired by a surviving recurring fighter.
//!
//! The finite opening tracks remain in `pressure_fighter_projectiles`; these
//! constants describe the retail behavior after that certified presentation
//! ends. Source object slots and interpreter state stay in oracle tooling.
//! Regenerate the test evidence with
//! `uv run python tools/sf2/generate_pressure_fighter_live_projectiles.py`.

/// Retail exposes four reusable hostile-laser allocations during this fight.
pub(super) const MAXIMUM_ACTIVE_PROJECTILES: usize = 4;

/// A fighter-created laser inherits the fighter transform at this launch speed.
pub(super) const INITIAL_SPEED: u8 = 30;

/// The shared weapon service declines hostile shots outside this wrapped
/// horizontal player radius.
pub(super) const MAXIMUM_LAUNCH_DISTANCE: u16 = 12_000;

/// The launch initializer promotes the projectile to its flight speed.
pub(super) const CRUISE_SPEED: u8 = 63;

/// Homing ends once the projectile is inside this horizontal player radius.
pub(super) const HOMING_RADIUS: u16 = 1_024;

/// Each distant homing step contracts the projectile toward the player three
/// times before it faces and advances.
pub(super) const HOMING_CONTRACTIONS_PER_STEP: u8 = 3;

/// The retained oracle begins immediately after strategy frame 142. Keeping
/// this semantic clock with the typed fighter state reproduces trigger phase
/// without carrying source-machine scheduling state into the port.
pub(super) const HANDOFF_STRATEGY_FRAME: u8 = 142;

/// The post-homing loop permits forty advances. On even strategy frames its
/// aiming trigger may apply one smooth correction when the player is inside
/// the wrapped yaw arc; contact or crossing the target ends the loop early.
pub(super) const MAXIMUM_AIM_CORRECTION_STEPS: u8 = 40;
pub(super) const SMOOTH_AIM_TRIGGER_PERIOD: u8 = 2;
pub(super) const SMOOTH_AIM_YAW_RADIUS: u8 = 32;

/// A projectile that misses the player advances fifteen times in free flight
/// before its next wait boundary retires it.
pub(super) const CRUISE_STEPS: u8 = 15;

/// Normalized thirds of a retail frame preserve the observed average
/// cooperative movement cadence without exposing interpreter scheduling.
pub(super) const LOGIC_CREDIT_PER_TICK: u8 = 12;
pub(super) const LOGIC_CREDIT_THRESHOLD: u8 = 10;
pub(super) const INITIAL_LOGIC_CREDIT: u8 = 8;
