//! Pure-Rust SPC backend: wraps the `sf-spc` engine behind the exact same
//! `Spc` surface the C-oracle FFI wrapper exposes, so `Booter`/`SpcPlayer`
//! run on it unchanged. This is the default backend.

use sf_spc::{Filter, SnesSpc, BASS_NORM, GAIN_UNIT};

/// SPC-700 emulator + SNES output filter (mirrors the FFI `Spc`).
pub struct Spc {
    spc: Box<SnesSpc>,
    filter: Filter,
}

// The engine holds a raw pointer into its own (boxed, pointer-stable) RAM and
// has no thread affinity; ownership is unique.
unsafe impl Send for Spc {}

impl Spc {
    pub fn new() -> Self {
        let mut filter = Filter::new();
        filter.set_gain(GAIN_UNIT);
        filter.set_bass(BASS_NORM);
        Spc {
            spc: SnesSpc::new(),
            filter,
        }
    }

    pub fn init_rom(&mut self, rom: &[u8; sf_spc::ROM_SIZE]) {
        self.spc.init_rom(rom);
    }

    pub fn reset(&mut self) {
        self.spc.reset();
    }

    pub fn ram_mut(&mut self) -> &mut [u8] {
        self.spc.ram_mut()
    }

    pub fn read_port(&mut self, time: i32, port: i32) -> i32 {
        self.spc.read_port(time, port as usize)
    }

    pub fn write_port(&mut self, time: i32, port: i32, data: i32) {
        self.spc.write_port(time, port as usize, data);
    }

    pub fn end_frame(&mut self, end_time: i32) {
        self.spc.end_frame(end_time);
    }

    pub fn sample_count(&self) -> i32 {
        self.spc.sample_count()
    }

    /// # Safety
    /// `out` must stay valid until the buffer is re-armed, detached, or
    /// replaced by a `play_filtered` call.
    pub unsafe fn set_output(&mut self, out: *mut i16, out_size: i32) {
        self.spc.set_output(out, out_size)
    }

    pub fn detach_output(&mut self) {
        self.spc.detach_output();
    }

    pub fn skip(&mut self, count: i32) -> Result<(), &'static str> {
        if self.spc.play(count, std::ptr::null_mut()) {
            Ok(())
        } else {
            Err("spc play failed")
        }
    }

    /// Generate interleaved stereo samples WITHOUT the output filter (raw
    /// DSP/CPU output) — used by pre-filter parity cross-checks.
    pub fn play_raw(&mut self, out: &mut [i16]) -> Result<(), &'static str> {
        assert!(out.len() % 2 == 0, "stereo pair count");
        if !self.spc.play(out.len() as i32, out.as_mut_ptr()) {
            return Err("spc play failed");
        }
        Ok(())
    }

    pub fn play_filtered(&mut self, out: &mut [i16]) -> Result<(), &'static str> {
        assert!(out.len() % 2 == 0, "stereo pair count");
        if !self.spc.play(out.len() as i32, out.as_mut_ptr()) {
            return Err("spc play failed");
        }
        self.filter.run(out);
        Ok(())
    }

    pub fn filter_clear(&mut self) {
        self.filter.clear();
    }

    /// Load an SPC file image (CPU cross-check micro-tests only).
    pub fn debug_load_spc(&mut self, data: &[u8]) -> Result<(), &'static str> {
        self.spc.load_spc(data)
    }
}

impl Default for Spc {
    fn default() -> Self {
        Self::new()
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

/// The SNES IPL boot ROM (byte-identical to the C oracle).
pub use sf_spc::IPL_ROM;
