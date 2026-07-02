//! Pure-Rust SNES SPC-700 + S-DSP audio engine.
//!
//! A fresh, hardware-informed Rust implementation whose observable behavior is
//! bit-for-bit identical to the bundled snes_spc 0.9.0 C++ emulator (the FFI
//! oracle in `sf-audio`). Structure:
//!
//! - [`cpu`] — SPC-700 CPU core, three timers, the $F0-$FF SMP register page,
//!   IPL ROM mapping, and the sample-carry output buffering.
//! - [`dsp`] — 8-voice S-DSP: BRR decode, gaussian interpolation, ADSR/GAIN,
//!   KON/KOFF, ENDX, noise LFSR, pitch modulation, and the 8-tap echo FIR.
//! - [`filter`] — the SNES output low/high-pass + gain filter (`SPC_Filter`).
//!
//! The public surface mirrors `sf-audio::spc` so a backend swap is transparent.

mod cpu;
mod dsp;
pub mod filter;

pub use cpu::SnesSpc;
pub use filter::{Filter, BASS_MAX, BASS_NONE, BASS_NORM, GAIN_UNIT};

/// SNES IPL boot ROM (byte-identical to `s_ipl_rom` in the C oracle).
pub const IPL_ROM: [u8; 0x40] = [
    0xCD, 0xEF, 0xBD, 0xE8, 0x00, 0xC6, 0x1D, 0xD0, 0xFC, 0x8F, 0xAA, 0xF4, 0x8F, 0xBB, 0xF5, 0x78,
    0xCC, 0xF4, 0xD0, 0xFB, 0x2F, 0x19, 0xEB, 0xF4, 0xD0, 0xFC, 0x7E, 0xF4, 0xD0, 0x0B, 0xE4, 0xF5,
    0xCB, 0xF4, 0xD7, 0x00, 0xFC, 0xD0, 0xF3, 0xAB, 0x01, 0x10, 0xEF, 0x7E, 0xF4, 0x10, 0xEB, 0xBA,
    0xF6, 0xDA, 0x00, 0xBA, 0xF4, 0xC4, 0xF4, 0xDD, 0x5D, 0xD0, 0xDB, 0x1F, 0x00, 0x00, 0xC0, 0xFF,
];

pub const ROM_SIZE: usize = 0x40;
pub const RAM_SIZE: usize = 0x10000;
