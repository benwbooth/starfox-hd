//! Audio: SPC-700 playback of the original sound driver + APU port protocol.
//!
//! Ported from the original C reference (`audio/audio.c`,
//! `audio/spc_player.c`, `audio/spc_boot.c` — the IPL ROM upload handshake —
//! and `game/sound.c` — the port 1/2/3 protocol and SFX ring queue).
//!
//! The pure-Rust `sf-spc` engine drives audio; there is no C++ dependency.

pub mod backend;
pub mod boot;
pub mod native;
pub mod player;
pub mod sound;

/// Active backend: `Spc` + `IPL_ROM` resolve to the native pure-Rust engine.
pub mod spc {
    pub use crate::native::{Spc, IPL_ROM};
}
