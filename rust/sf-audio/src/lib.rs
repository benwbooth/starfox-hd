//! Audio: SPC-700 playback of the original sound driver + APU port protocol.
//!
//! Ports (C oracle): `src/audio/audio.c`, `src/audio/spc_player.c`,
//! `src/audio/spc_boot.c` (IPL ROM upload handshake), `src/game/sound.c`
//! (port 1/2/3 protocol, SFX ring queue).
//!
//! Backend selection: by default the pure-Rust `sf-spc` engine drives audio
//! (no C++ dependency). Enabling the `ffi-oracle` feature swaps in the bundled
//! snes_spc C++ emulator behind FFI (same library the C build links, so its
//! output is the bit-exact oracle) — retained for cross-check parity tests.

pub mod backend;
pub mod boot;
pub mod native;
pub mod player;
pub mod sound;

// C++ FFI oracle backend (opt-in).
#[cfg(feature = "ffi-oracle")]
pub mod ffi;
#[cfg(feature = "ffi-oracle")]
pub mod ffi_spc;

/// Active backend: `Spc` + `IPL_ROM` resolve to the native engine by default,
/// or the FFI oracle when the `ffi-oracle` feature is enabled.
pub mod spc {
    #[cfg(not(feature = "ffi-oracle"))]
    pub use crate::native::{Spc, IPL_ROM};
    #[cfg(feature = "ffi-oracle")]
    pub use crate::ffi_spc::{Spc, IPL_ROM};
}
