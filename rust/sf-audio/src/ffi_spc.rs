//! Safe wrapper over the snes_spc emulator (C oracle:
//! `src/audio/spc_player.c` usage of the spc.h API).

use crate::ffi;

/// Owned SPC-700 emulator instance + output filter, mirroring the pairing
/// the C oracle creates in `SpcPlayer_Init`.
pub struct Spc {
    spc: *mut ffi::SnesSpc,
    filter: *mut ffi::SpcFilter,
}

// The raw pointers are uniquely owned; the emulator has no thread affinity.
unsafe impl Send for Spc {}

impl Spc {
    pub fn new() -> Self {
        let spc = unsafe { ffi::spc_new() };
        let filter = unsafe { ffi::spc_filter_new() };
        assert!(!spc.is_null() && !filter.is_null(), "snes_spc alloc");
        unsafe {
            ffi::spc_filter_set_gain(filter, ffi::SPC_FILTER_GAIN_UNIT);
            ffi::spc_filter_set_bass(filter, ffi::SPC_FILTER_BASS_NORM);
        }
        Spc { spc, filter }
    }

    /// Install the 64-byte IPL boot ROM (C `SpcBoot_Init`).
    pub fn init_rom(&mut self, rom: &[u8; ffi::SPC_ROM_SIZE]) {
        unsafe { ffi::spc_init_rom(self.spc, rom.as_ptr()) }
    }

    pub fn reset(&mut self) {
        unsafe { ffi::spc_reset(self.spc) }
    }

    /// Direct APU RAM access (C `spc_get_ram` uses in the boot path).
    pub fn ram_mut(&mut self) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(ffi::spc_get_ram(self.spc), ffi::SPC_RAM_SIZE)
        }
    }

    pub fn read_port(&mut self, time: i32, port: i32) -> i32 {
        unsafe { ffi::spc_read_port(self.spc, time, port) }
    }

    pub fn write_port(&mut self, time: i32, port: i32, data: i32) {
        unsafe { ffi::spc_write_port(self.spc, time, port, data) }
    }

    pub fn end_frame(&mut self, end_time: i32) {
        unsafe { ffi::spc_end_frame(self.spc, end_time) }
    }

    /// Samples generated since the output buffer was last armed
    /// (C `spc_sample_count`, used by `spc_advance` in spc_boot.c).
    pub fn sample_count(&self) -> i32 {
        unsafe { ffi::spc_sample_count(self.spc) }
    }

    /// Arm the emulator's output buffer (C `spc_set_output`).
    ///
    /// # Safety
    /// `out` must stay valid (not moved or freed) until the buffer is
    /// re-armed, detached with [`detach_output`](Self::detach_output), or
    /// replaced by a `play_filtered`/`skip` call (spc_play/spc_skip re-arm
    /// the output internally).
    pub unsafe fn set_output(&mut self, out: *mut i16, out_size: i32) {
        ffi::spc_set_output(self.spc, out, out_size)
    }

    /// Point the emulator at its internal dummy buffer so no external
    /// pointer is retained (spc_set_output(NULL, 0)).
    pub fn detach_output(&mut self) {
        unsafe { ffi::spc_set_output(self.spc, std::ptr::null_mut(), 0) }
    }

    /// Emulate without keeping samples (C `spc_skip`).
    pub fn skip(&mut self, count: i32) -> Result<(), &'static str> {
        let err = unsafe { ffi::spc_skip(self.spc, count) };
        if err.is_null() {
            Ok(())
        } else {
            Err("spc_skip failed")
        }
    }

    /// Generate interleaved stereo samples and run the SNES output filter,
    /// like the C `SpcPlayer_Generate` inner loop.
    /// Generate interleaved stereo samples WITHOUT the output filter (raw
    /// oracle DSP/CPU output) — used by pre-filter parity cross-checks.
    pub fn play_raw(&mut self, out: &mut [i16]) -> Result<(), &'static str> {
        assert!(out.len() % 2 == 0, "stereo pair count");
        let err = unsafe { ffi::spc_play(self.spc, out.len() as i32, out.as_mut_ptr()) };
        if !err.is_null() {
            return Err("spc_play failed");
        }
        Ok(())
    }

    pub fn play_filtered(&mut self, out: &mut [i16]) -> Result<(), &'static str> {
        assert!(out.len() % 2 == 0, "stereo pair count");
        let err = unsafe { ffi::spc_play(self.spc, out.len() as i32, out.as_mut_ptr()) };
        if !err.is_null() {
            return Err("spc_play failed");
        }
        unsafe { ffi::spc_filter_run(self.filter, out.as_mut_ptr(), out.len() as i32) };
        Ok(())
    }

    pub fn filter_clear(&mut self) {
        unsafe { ffi::spc_filter_clear(self.filter) }
    }

    /// Load an SPC file image (CPU cross-check micro-tests only).
    pub fn debug_load_spc(&mut self, data: &[u8]) -> Result<(), &'static str> {
        let err = unsafe {
            ffi::spc_load_spc(self.spc, data.as_ptr() as *const std::ffi::c_void, data.len() as i64)
        };
        if err.is_null() {
            Ok(())
        } else {
            Err("spc_load_spc failed")
        }
    }
}

impl Default for Spc {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Spc {
    fn drop(&mut self) {
        unsafe {
            ffi::spc_filter_delete(self.filter);
            ffi::spc_delete(self.spc);
        }
    }
}

impl crate::backend::SpcEngine for Spc {
    fn init_rom(&mut self, rom: &[u8; 0x40]) {
        Spc::init_rom(self, rom)
    }
    fn reset(&mut self) {
        Spc::reset(self)
    }
    unsafe fn set_output(&mut self, out: *mut i16, size: i32) {
        Spc::set_output(self, out, size)
    }
    fn detach_output(&mut self) {
        Spc::detach_output(self)
    }
    fn end_frame(&mut self, end_time: i32) {
        Spc::end_frame(self, end_time)
    }
    fn sample_count(&self) -> i32 {
        Spc::sample_count(self)
    }
    fn read_port(&mut self, time: i32, port: i32) -> i32 {
        Spc::read_port(self, time, port)
    }
    fn write_port(&mut self, time: i32, port: i32, data: i32) {
        Spc::write_port(self, time, port, data)
    }
}

/// The SNES IPL boot ROM, byte-identical to `s_ipl_rom` in the C oracle
/// (`src/audio/spc_boot.c`).
pub const IPL_ROM: [u8; ffi::SPC_ROM_SIZE] = [
    0xCD, 0xEF, 0xBD, 0xE8, 0x00, 0xC6, 0x1D, 0xD0, 0xFC, 0x8F, 0xAA, 0xF4,
    0x8F, 0xBB, 0xF5, 0x78, 0xCC, 0xF4, 0xD0, 0xFB, 0x2F, 0x19, 0xEB, 0xF4,
    0xD0, 0xFC, 0x7E, 0xF4, 0xD0, 0x0B, 0xE4, 0xF5, 0xCB, 0xF4, 0xD7, 0x00,
    0xFC, 0xD0, 0xF3, 0xAB, 0x01, 0x10, 0xEF, 0x7E, 0xF4, 0x10, 0xEB, 0xBA,
    0xF6, 0xDA, 0x00, 0xBA, 0xF4, 0xC4, 0xF4, 0xDD, 0x5D, 0xD0, 0xDB, 0x1F,
    0x00, 0x00, 0xC0, 0xFF,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emulator_boots_and_generates() {
        let mut spc = Spc::new();
        spc.init_rom(&IPL_ROM);
        spc.reset();
        // The IPL ROM idles waiting for the $CC handshake; port 0 must read
        // back the $AA ready byte after some emulated time, like the C
        // ipl_wait_ready loop observes.
        let mut out = [0i16; 256];
        spc.play_filtered(&mut out).expect("generate");
        let p0 = spc.read_port(0, 0);
        assert_eq!(p0, 0xAA, "IPL ready byte on port 0");
    }
}
