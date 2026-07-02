//! Audio: SPC-700 playback of the original sound driver + APU port protocol.
//!
//! Ports (C oracle): `src/audio/audio.c`, `src/audio/spc_player.c`,
//! `src/audio/spc_boot.c` (IPL ROM upload handshake), `src/game/sound.c`
//! (port 1/2/3 protocol, SFX ring queue).
//! The snes_spc C++ emulator stays behind FFI initially (same library as the
//! oracle → bit-identical output); a pure-Rust SPC core can come later.

pub mod boot;
pub mod ffi;
pub mod player;
pub mod sound;
pub mod spc;
