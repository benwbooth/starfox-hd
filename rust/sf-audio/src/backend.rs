//! Backend-selection glue. `Booter`/`load_track` in `boot.rs` are generic over
//! this trait so the boot code drives the native pure-Rust `sf-spc` engine.

pub trait SpcEngine {
    fn init_rom(&mut self, rom: &[u8; 0x40]);
    fn reset(&mut self);
    /// # Safety: `out` must stay valid until re-armed or detached.
    unsafe fn set_output(&mut self, out: *mut i16, size: i32);
    fn detach_output(&mut self);
    fn end_frame(&mut self, end_time: i32);
    fn sample_count(&self) -> i32;
    fn read_port(&mut self, time: i32, port: i32) -> i32;
    fn write_port(&mut self, time: i32, port: i32, data: i32);
}
