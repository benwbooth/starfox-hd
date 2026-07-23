//! Native audio for the modern Rust port.
//!
//! Shipping playback mixes decoded PCM assets through typed music, engine,
//! ambience, and effect channels. Verification may still run the original
//! sound processor program behind the explicit `oracle-audio` feature, but
//! that machinery is unreachable from `sf-app`.

pub mod catalog;
pub mod native_player;
pub mod sf2_native_player;
pub mod sound;

#[cfg(feature = "oracle-audio")]
pub mod backend;
#[cfg(feature = "oracle-audio")]
pub mod boot;
#[cfg(feature = "oracle-audio")]
pub mod native;
#[cfg(feature = "oracle-audio")]
pub mod player;

/// Verification-only sound-machine surface.
#[cfg(feature = "oracle-audio")]
pub mod spc {
    pub use crate::native::{Spc, IPL_ROM};
}
