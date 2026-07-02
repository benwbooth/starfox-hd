//! Raw FFI over the bundled snes_spc C API (`src/snes_spc/spc.h`).
//! Same emulator the C oracle links, so output is bit-identical.

use std::os::raw::{c_char, c_int, c_long, c_short, c_uchar, c_void};

pub const SPC_ROM_SIZE: usize = 0x40;
pub const SPC_RAM_SIZE: usize = 0x10000;
pub const SPC_SAMPLE_RATE: u32 = 32000;
pub const SPC_CLOCKS_PER_SAMPLE: i32 = 32;
pub const SPC_FILTER_GAIN_UNIT: c_int = 0x100;
pub const SPC_FILTER_BASS_NORM: c_int = 8;

#[repr(C)]
pub struct SnesSpc {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct SpcFilter {
    _opaque: [u8; 0],
}

pub type SpcErr = *const c_char; // NULL on success
pub type SpcTime = c_int;
pub type SpcSample = c_short;

extern "C" {
    pub fn spc_new() -> *mut SnesSpc;
    pub fn spc_delete(spc: *mut SnesSpc);
    pub fn spc_init_rom(spc: *mut SnesSpc, rom: *const c_uchar);
    pub fn spc_set_output(spc: *mut SnesSpc, out: *mut SpcSample, out_size: c_int);
    pub fn spc_reset(spc: *mut SnesSpc);
    pub fn spc_get_ram(spc: *mut SnesSpc) -> *mut c_uchar;
    pub fn spc_read_port(spc: *mut SnesSpc, time: SpcTime, port: c_int) -> c_int;
    pub fn spc_write_port(spc: *mut SnesSpc, time: SpcTime, port: c_int, data: c_int);
    pub fn spc_end_frame(spc: *mut SnesSpc, end_time: SpcTime);
    pub fn spc_load_spc(spc: *mut SnesSpc, spc_in: *const c_void, size: c_long) -> SpcErr;
    pub fn spc_play(spc: *mut SnesSpc, count: c_int, out: *mut c_short) -> SpcErr;
    pub fn spc_skip(spc: *mut SnesSpc, count: c_int) -> SpcErr;

    pub fn spc_filter_new() -> *mut SpcFilter;
    pub fn spc_filter_delete(filter: *mut SpcFilter);
    pub fn spc_filter_run(filter: *mut SpcFilter, io: *mut SpcSample, count: c_int);
    pub fn spc_filter_clear(filter: *mut SpcFilter);
    pub fn spc_filter_set_gain(filter: *mut SpcFilter, gain: c_int);
    pub fn spc_filter_set_bass(filter: *mut SpcFilter, bass: c_int);
}
