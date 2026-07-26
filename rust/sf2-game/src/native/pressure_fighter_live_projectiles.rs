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

/// The retail path retains a short post-homing aiming phase. It cannot leave
/// before the second move and bounds the correction at five moves if the
/// moving target never settles.
pub(super) const MINIMUM_AIM_CORRECTION_STEPS: u8 = 2;
pub(super) const MAXIMUM_AIM_CORRECTION_STEPS: u8 = 5;

/// A projectile that misses the player advances fifteen times in free flight
/// before its next wait boundary retires it.
pub(super) const CRUISE_STEPS: u8 = 15;

/// Normalized thirds of a retail frame preserve the observed average
/// cooperative movement cadence without exposing interpreter scheduling.
pub(super) const LOGIC_CREDIT_PER_TICK: u8 = 12;
pub(super) const LOGIC_CREDIT_THRESHOLD: u8 = 10;
pub(super) const INITIAL_LOGIC_CREDIT: u8 = 8;
