//! SPC-700 CPU core + timers + $F0-$FF SMP register page + memory + sample
//! buffering. Fresh Rust implementation matching the observable behavior of
//! snes_spc 0.9.0's `SNES_SPC.cpp` / `SPC_CPU.h` bit-for-bit: full instruction
//! set with exact per-opcode cycle counts, the three timers with stage
//! dividers, DSP catch-up timing, IPL ROM mapping gated by the control-reg
//! ROM-enable bit, and the extra-sample carry buffer that shapes play()/
//! end_frame() output boundaries.

use crate::dsp::{Dsp, EXTRA_SIZE};

pub const ROM_SIZE: usize = 0x40;
pub const RAM_SIZE: usize = 0x10000;
pub const CLOCKS_PER_SAMPLE: i32 = 32;
const ROM_ADDR: usize = 0xFFC0;
const TIMER_COUNT: usize = 3;
const PORT_COUNT: usize = 4;
const NO_READ_BEFORE_WRITE: i32 = 0x2000;

// SMP register indices ($F0-$FF, low nibble)
const R_TEST: usize = 0x0;
const R_CONTROL: usize = 0x1;
const R_DSPADDR: usize = 0x2;
const R_DSPDATA: usize = 0x3;
const R_CPUIO0: usize = 0x4;
const R_T0TARGET: usize = 0xA;
const R_T0OUT: usize = 0xD;

// PSW flag bits (stored positions)
const N80: i32 = 0x80;
const V40: i32 = 0x40;
const P20: i32 = 0x20;
const B10: i32 = 0x10;
const H08: i32 = 0x08;
const I04: i32 = 0x04;
const Z02: i32 = 0x02;
const NZ_NEG_MASK: i32 = 0x880;

const CYCLE_TABLE_PACKED: [u8; 128] = [
    0x28, 0x47, 0x34, 0x36, 0x26, 0x54, 0x54, 0x68, 0x48, 0x47, 0x45, 0x56, 0x55, 0x65, 0x22, 0x46,
    0x28, 0x47, 0x34, 0x36, 0x26, 0x54, 0x54, 0x74, 0x48, 0x47, 0x45, 0x56, 0x55, 0x65, 0x22, 0x38,
    0x28, 0x47, 0x34, 0x36, 0x26, 0x44, 0x54, 0x66, 0x48, 0x47, 0x45, 0x56, 0x55, 0x45, 0x22, 0x43,
    0x28, 0x47, 0x34, 0x36, 0x26, 0x44, 0x54, 0x75, 0x48, 0x47, 0x45, 0x56, 0x55, 0x55, 0x22, 0x36,
    0x28, 0x47, 0x34, 0x36, 0x26, 0x54, 0x52, 0x45, 0x48, 0x47, 0x45, 0x56, 0x55, 0x55, 0x22, 0xC5,
    0x38, 0x47, 0x34, 0x36, 0x26, 0x44, 0x52, 0x44, 0x48, 0x47, 0x45, 0x56, 0x55, 0x55, 0x22, 0x34,
    0x38, 0x47, 0x45, 0x47, 0x25, 0x64, 0x52, 0x49, 0x48, 0x47, 0x56, 0x67, 0x45, 0x55, 0x22, 0x83,
    0x28, 0x47, 0x34, 0x36, 0x24, 0x53, 0x43, 0x40, 0x48, 0x47, 0x45, 0x56, 0x34, 0x54, 0x22, 0x60,
];

#[derive(Clone, Copy, Default)]
struct Timer {
    next_time: i32,
    prescaler: i32,
    period: i32,
    divider: i32,
    enabled: i32,
    counter: i32,
}

#[inline]
fn if_0_256(n: i32) -> i32 {
    ((n - 1) as u8) as i32 + 1
}

pub struct SnesSpc {
    ram: Box<[u8]>,
    ram_ptr: *mut u8,
    dsp: Dsp,

    // $F0-$FF register page: [0] output side (REGS), [1] input side (REGS_IN)
    smp_regs: [[u8; 0x10]; 2],

    // CPU registers (uncached copy; live in locals during run)
    pc: u16,
    a: u8,
    x: u8,
    y: u8,
    psw: u8,
    sp: u8,

    timers: [Timer; TIMER_COUNT],
    dsp_time: i32,
    spc_time: i32,
    tempo: i32,
    extra_clocks: i32,

    rom_enabled: i32,
    rom: [u8; ROM_SIZE],
    hi_ram: [u8; ROM_SIZE],

    cycle_table: [u8; 256],
    cpu_error: bool,

    // sample output buffering (mirrors SNES_SPC extra_buf scheme)
    buf_ptr: *mut i16,
    buf_len: i32,
    extra_buf: [i16; EXTRA_SIZE],
    extra_pos: i32,
}

const TEMPO_UNIT: i32 = 0x100;

impl SnesSpc {
    pub fn new() -> Box<Self> {
        // RAM sized 0x10000 + 0x100 trailing pad; all access masked to 0xFFFF.
        let mut ram: Box<[u8]> = vec![0u8; RAM_SIZE + 0x100].into_boxed_slice();
        let ram_ptr = ram.as_mut_ptr();
        let dsp = Dsp::new(ram_ptr);
        let mut s = Box::new(SnesSpc {
            ram,
            ram_ptr,
            dsp,
            smp_regs: [[0; 0x10]; 2],
            pc: 0,
            a: 0,
            x: 0,
            y: 0,
            psw: 0,
            sp: 0,
            timers: [Timer::default(); TIMER_COUNT],
            dsp_time: 0,
            spc_time: 0,
            tempo: TEMPO_UNIT,
            extra_clocks: 0,
            rom_enabled: 0,
            rom: [0; ROM_SIZE],
            hi_ram: [0; ROM_SIZE],
            cycle_table: [0; 256],
            cpu_error: false,
            buf_ptr: std::ptr::null_mut(),
            buf_len: 0,
            extra_buf: [0; EXTRA_SIZE],
            extra_pos: 0,
        });
        s.init();
        s
    }

    fn init(&mut self) {
        self.tempo = TEMPO_UNIT;
        self.rom[0x3E] = 0xFF;
        self.rom[0x3F] = 0xC0;
        for i in 0..128 {
            let n = CYCLE_TABLE_PACKED[i];
            self.cycle_table[i * 2] = n >> 4;
            self.cycle_table[i * 2 + 1] = n & 0x0F;
        }
        self.reset();
    }

    pub fn init_rom(&mut self, rom: &[u8; ROM_SIZE]) {
        self.rom.copy_from_slice(rom);
    }

    pub fn ram_mut(&mut self) -> &mut [u8] {
        &mut self.ram[0..RAM_SIZE]
    }

    // ---- raw RAM access (shared with DSP via ram_ptr) ----
    #[inline]
    fn rc(&self, addr: usize) -> u8 {
        unsafe { *self.ram_ptr.add(addr & 0xFFFF) }
    }
    #[inline]
    fn wc(&self, addr: usize, v: u8) {
        unsafe { *self.ram_ptr.add(addr & 0xFFFF) = v }
    }
    #[inline]
    fn rp16(&self, addr: i32) -> i32 {
        self.rc(addr as usize & 0xFFFF) as i32 | ((self.rc((addr as usize).wrapping_add(1) & 0xFFFF) as i32) << 8)
    }

    // ---- tempo / timers ----
    fn set_tempo(&mut self, t: i32) {
        self.tempo = t;
        let timer2_shift = 4;
        let other_shift = 3;
        let mut t = t;
        if t == 0 {
            t = 1;
        }
        let timer2_rate = 1 << timer2_shift;
        let mut rate = (timer2_rate * TEMPO_UNIT + (t >> 1)) / t;
        if rate < timer2_rate / 4 {
            rate = timer2_rate / 4;
        }
        self.timers[2].prescaler = rate;
        self.timers[1].prescaler = rate << other_shift;
        self.timers[0].prescaler = rate << other_shift;
    }

    fn run_timer_(&mut self, i: usize, time: i32) {
        let t = &mut self.timers[i];
        let elapsed = (time - t.next_time) / t.prescaler + 1;
        t.next_time += elapsed * t.prescaler;
        if t.enabled != 0 {
            let remain = if_0_256(t.period - t.divider);
            let mut divider = t.divider + elapsed;
            let over = elapsed - remain;
            if over >= 0 {
                let n = over / t.period;
                t.counter = (t.counter + 1 + n) & 0x0F;
                divider = over - n * t.period;
            }
            t.divider = (divider as u8) as i32;
        }
    }
    #[inline]
    fn run_timer(&mut self, i: usize, time: i32) {
        if time >= self.timers[i].next_time {
            self.run_timer_(i, time);
        }
    }

    // ---- ROM mapping ----
    fn enable_rom(&mut self, enable: i32) {
        if self.rom_enabled != enable {
            self.rom_enabled = enable;
            if enable != 0 {
                for i in 0..ROM_SIZE {
                    self.hi_ram[i] = self.rc(ROM_ADDR + i);
                }
                for i in 0..ROM_SIZE {
                    self.wc(ROM_ADDR + i, self.rom[i]);
                }
            } else {
                for i in 0..ROM_SIZE {
                    self.wc(ROM_ADDR + i, self.hi_ram[i]);
                }
            }
        }
    }

    // ---- DSP catch-up + register access ----
    #[inline]
    fn run_dsp(&mut self, time: i32) {
        let count = time - self.dsp_time;
        if count > 0 {
            self.dsp_time = time;
            self.dsp.run(count);
        }
    }
    fn dsp_read(&mut self, time: i32) -> i32 {
        self.run_dsp(time);
        self.dsp.read((self.smp_regs[0][R_DSPADDR] & 0x7F) as usize)
    }
    fn dsp_write(&mut self, data: i32, time: i32) {
        self.run_dsp(time);
        let addr = self.smp_regs[0][R_DSPADDR];
        if addr <= 0x7F {
            self.dsp.write(addr as usize, data);
        }
    }

    // ---- SMP register writes ----
    fn cpu_write_smp_reg_(&mut self, data: i32, time: i32, addr: usize) {
        match addr {
            R_T0TARGET | 0xB | 0xC => {
                let i = addr - R_T0TARGET;
                let period = if_0_256(data & 0xFF);
                if self.timers[i].period != period {
                    self.run_timer(i, time);
                    self.timers[i].period = period;
                }
            }
            R_T0OUT | 0xE | 0xF => {
                if data < NO_READ_BEFORE_WRITE / 2 {
                    let i = addr - R_T0OUT;
                    self.run_timer(i, time - 1);
                    self.timers[i].counter = 0;
                }
            }
            0x8 | 0x9 => {
                self.smp_regs[1][addr] = data as u8;
            }
            R_TEST => {}
            R_CONTROL => {
                if data & 0x10 != 0 {
                    self.smp_regs[1][R_CPUIO0] = 0;
                    self.smp_regs[1][R_CPUIO0 + 1] = 0;
                }
                if data & 0x20 != 0 {
                    self.smp_regs[1][R_CPUIO0 + 2] = 0;
                    self.smp_regs[1][R_CPUIO0 + 3] = 0;
                }
                for i in 0..TIMER_COUNT {
                    let enabled = data >> i & 1;
                    if self.timers[i].enabled != enabled {
                        self.run_timer(i, time);
                        self.timers[i].enabled = enabled;
                        if enabled != 0 {
                            self.timers[i].divider = 0;
                            self.timers[i].counter = 0;
                        }
                    }
                }
                self.enable_rom(data & 0x80);
            }
            _ => {}
        }
    }
    fn cpu_write_smp_reg(&mut self, data: i32, time: i32, addr: usize) {
        if addr == R_DSPDATA {
            self.dsp_write(data, time);
        } else {
            self.cpu_write_smp_reg_(data, time, addr);
        }
    }
    fn cpu_write_high(&mut self, data: i32, i: usize) {
        self.hi_ram[i] = data as u8;
        if self.rom_enabled != 0 {
            self.wc(i + ROM_ADDR, self.rom[i]);
        }
    }
    fn cpu_write(&mut self, data: i32, addr: u16, time: i32) {
        self.wc(addr as usize, data as u8);
        if addr >= 0xF0 {
            let reg = (addr - 0xF0) as usize;
            if reg < 0x10 {
                self.smp_regs[0][reg] = data as u8;
                if reg != 2 && (reg < 4 || reg > 7) {
                    self.cpu_write_smp_reg(data, time, reg);
                }
            } else if addr as usize >= ROM_ADDR {
                self.cpu_write_high(data, addr as usize - ROM_ADDR);
            }
        }
    }

    // ---- SMP register reads ----
    fn cpu_read_smp_reg(&mut self, reg: usize, time: i32) -> i32 {
        let mut result = self.smp_regs[1][reg] as i32;
        let r = reg as i32 - R_DSPADDR as i32;
        if (r as u32) <= 1 {
            result = self.smp_regs[0][R_DSPADDR] as i32;
            if (r as u32) == 1 {
                result = self.dsp_read(time);
            }
        }
        result
    }
    fn cpu_read(&mut self, addr: u16, time: i32) -> i32 {
        let mut result = self.rc(addr as usize) as i32;
        let mut reg = addr as i32 - 0xF0;
        if reg >= 0 {
            reg -= 0x10;
            if (reg as u32) >= 0xFF00 {
                reg += 0x10 - R_T0OUT as i32;
                if (reg as u32) < TIMER_COUNT as u32 {
                    let i = reg as usize;
                    if time >= self.timers[i].next_time {
                        self.run_timer_(i, time);
                    }
                    result = self.timers[i].counter;
                    self.timers[i].counter = 0;
                } else if reg < 0 {
                    result = self.cpu_read_smp_reg((reg + R_T0OUT as i32) as usize, time);
                } else {
                    let a2 = (reg + (R_T0OUT as i32 + 0xF0 - 0x10000)) as u16;
                    result = self.cpu_read(a2, time);
                }
            }
        }
        result
    }

    // ---- reset ----
    fn load_regs(&mut self, base: usize) {
        for i in 0..0x10 {
            let v = self.rc(base + i);
            self.smp_regs[0][i] = v;
            self.smp_regs[1][i] = v;
        }
        self.smp_regs[1][R_TEST] = 0;
        self.smp_regs[1][R_CONTROL] = 0;
        self.smp_regs[1][R_T0TARGET] = 0;
        self.smp_regs[1][R_T0TARGET + 1] = 0;
        self.smp_regs[1][R_T0TARGET + 2] = 0;
    }
    fn ram_loaded(&mut self) {
        self.rom_enabled = 0;
        self.load_regs(0xF0);
    }
    fn timers_loaded(&mut self) {
        for i in 0..TIMER_COUNT {
            self.timers[i].period = if_0_256(self.smp_regs[0][R_T0TARGET + i] as i32);
            self.timers[i].enabled = self.smp_regs[0][R_CONTROL] as i32 >> i & 1;
            self.timers[i].counter = self.smp_regs[1][R_T0OUT + i] as i32 & 0x0F;
        }
        self.set_tempo(self.tempo);
    }
    fn regs_loaded(&mut self) {
        let ctl = self.smp_regs[0][R_CONTROL] as i32 & 0x80;
        self.enable_rom(ctl);
        self.timers_loaded();
    }
    fn reset_time_regs(&mut self) {
        self.cpu_error = false;
        self.spc_time = 0;
        self.dsp_time = 0;
        for i in 0..TIMER_COUNT {
            self.timers[i].next_time = 1;
            self.timers[i].divider = 0;
        }
        self.regs_loaded();
        self.extra_clocks = 0;
        self.reset_buf();
    }
    fn reset_common(&mut self, timer_counter_init: u8) {
        for i in 0..TIMER_COUNT {
            self.smp_regs[1][R_T0OUT + i] = timer_counter_init;
        }
        self.pc = ROM_ADDR as u16;
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.psw = 0;
        self.sp = 0;
        self.smp_regs[0][R_TEST] = 0x0A;
        self.smp_regs[0][R_CONTROL] = 0xB0;
        for i in 0..PORT_COUNT {
            self.smp_regs[1][R_CPUIO0 + i] = 0;
        }
        self.reset_time_regs();
    }
    pub fn reset(&mut self) {
        for i in 0..RAM_SIZE {
            self.wc(i, 0xFF);
        }
        self.ram_loaded();
        self.reset_common(0x0F);
        self.dsp.reset();
    }
    pub fn soft_reset(&mut self) {
        self.reset_common(0);
        self.dsp.soft_reset();
    }

    // ---- sample output buffering ----
    fn reset_buf(&mut self) {
        for i in 0..EXTRA_SIZE / 2 {
            self.extra_buf[i] = 0;
        }
        self.extra_pos = (EXTRA_SIZE / 2) as i32;
        self.buf_ptr = std::ptr::null_mut();
        self.dsp.set_output_none();
    }
    /// # Safety: `out` must remain valid until re-armed / detached.
    pub unsafe fn set_output(&mut self, out: *mut i16, size: i32) {
        debug_assert!(size & 1 == 0);
        self.extra_clocks &= CLOCKS_PER_SAMPLE - 1;
        if !out.is_null() {
            self.buf_ptr = out;
            self.buf_len = size;
            let copied = self.extra_pos.min(size);
            for k in 0..copied {
                *out.add(k as usize) = self.extra_buf[k as usize];
            }
            if copied >= size {
                // external buffer already full; remaining extras spill to DSP extra
                let remaining = self.extra_pos - size;
                for k in 0..remaining {
                    self.dsp.extra_mut()[k as usize] = self.extra_buf[(size + k) as usize];
                }
                self.dsp.set_output_extra(remaining);
            } else {
                self.dsp
                    .set_output_external(out.add(copied as usize), size - copied);
            }
        } else {
            self.reset_buf();
        }
    }
    pub fn detach_output(&mut self) {
        unsafe { self.set_output(std::ptr::null_mut(), 0) }
    }

    fn save_extra(&mut self) {
        let sc = ((self.extra_clocks >> 5) * 2).max(0);
        let idx = self.dsp.dsp_sample_count();
        let mut w = 0usize;
        if self.dsp_external() {
            let cap = self.dsp_ext_cap();
            let copied = self.buf_len - cap;
            if idx <= cap {
                let end = copied + idx;
                let mut i = sc;
                while i < end {
                    self.extra_buf[w] = unsafe { *self.buf_ptr.add(i as usize) };
                    w += 1;
                    i += 1;
                }
            } else {
                // DSP overflowed the external buffer into its 16-sample extra
                // ring. The C++ out pointer wraps within `extra`, so out_pos is
                // `(idx - cap) mod extra_size`, not the raw overshoot count.
                // (When this fires during boot the samples are discarded at
                // detach_output; play()'s small overshoot stays < extra_size.)
                let mut i = sc.max(0);
                while i < self.buf_len && w < EXTRA_SIZE {
                    self.extra_buf[w] = unsafe { *self.buf_ptr.add(i as usize) };
                    w += 1;
                    i += 1;
                }
                let over = idx - cap;
                let extra_end = over.rem_euclid(EXTRA_SIZE as i32);
                for k in 0..extra_end {
                    if w < EXTRA_SIZE {
                        self.extra_buf[w] = self.dsp.extra()[k as usize];
                        w += 1;
                    }
                }
            }
        } else {
            // Rare: DSP wrote only into its extra buffer (external full at arm).
            let mut i = sc;
            while i < self.buf_len {
                self.extra_buf[w] = unsafe { *self.buf_ptr.add(i as usize) };
                w += 1;
                i += 1;
            }
        }
        self.extra_pos = w as i32;
    }

    fn dsp_external(&self) -> bool {
        self.dsp.output_external()
    }
    fn dsp_ext_cap(&self) -> i32 {
        self.dsp.output_ext_cap()
    }

    pub fn sample_count(&self) -> i32 {
        (self.extra_clocks >> 5) * 2
    }

    // ---- frame stepping ----
    pub fn end_frame(&mut self, end_time: i32) {
        if end_time > self.spc_time {
            self.run_until_(end_time);
        }
        self.spc_time -= end_time;
        self.extra_clocks += end_time;
        for i in 0..TIMER_COUNT {
            self.run_timer(i, 0);
        }
        if self.dsp_time < 0 {
            let count = -self.dsp_time;
            self.dsp_time = 0;
            self.dsp.run(count);
        }
        if !self.buf_ptr.is_null() {
            self.save_extra();
        }
    }

    pub fn read_port(&mut self, time: i32, port: usize) -> i32 {
        self.run_until_(time);
        self.smp_regs[0][R_CPUIO0 + port] as i32
    }
    pub fn write_port(&mut self, time: i32, port: usize, data: i32) {
        self.run_until_(time);
        self.smp_regs[1][R_CPUIO0 + port] = data as u8;
    }

    /// Load an SPC file image (sets PC/regs/RAM/DSP), matching
    /// `SNES_SPC::load_spc`. Used by the CPU cross-check micro-tests to run
    /// poked RAM programs from an identical starting state on both engines.
    pub fn load_spc(&mut self, data: &[u8]) -> Result<(), &'static str> {
        const SIG: &[u8] = b"SNES-SPC700 Sound File Data v0.30\x1A\x1A";
        if data.len() < 35 || data[..27] != SIG[..27] {
            return Err("Not an SPC file");
        }
        if data.len() < 0x10180 {
            return Err("Corrupt SPC file");
        }
        let pcl = data[37] as u16;
        let pch = data[38] as u16;
        self.pc = pch * 0x100 + pcl;
        self.a = data[39];
        self.x = data[40];
        self.y = data[41];
        self.psw = data[42];
        self.sp = data[43];
        for i in 0..RAM_SIZE {
            self.wc(i, data[0x100 + i]);
        }
        self.ram_loaded();
        let mut dsp_regs = [0u8; 128];
        dsp_regs.copy_from_slice(&data[0x10100..0x10180]);
        self.dsp.load(&dsp_regs);
        self.reset_time_regs();
        Ok(())
    }

    pub fn play(&mut self, count: i32, out: *mut i16) -> bool {
        debug_assert!(count & 1 == 0);
        if count != 0 {
            unsafe { self.set_output(out, count) };
            self.end_frame(count * (CLOCKS_PER_SAMPLE / 2));
        }
        let err = self.cpu_error;
        self.cpu_error = false;
        !err
    }

    // ---- addressing modes (side-effect-free except pc) ----
    #[inline]
    fn addr_mode(&self, opcode: u8, base: u8, pc: &mut u16, data: i32, dp: i32, x: i32, y: i32) -> i32 {
        let off = opcode as i32 - base as i32;
        match off {
            -2 => {
                *pc = pc.wrapping_sub(1);
                x + dp
            }
            0x0F => self.rp16(data + dp) + y,
            -1 => self.rp16(((data + x) & 0xFF) + dp),
            0x0E => {
                let d = data + y;
                *pc = pc.wrapping_add(1);
                d + 0x100 * self.rc(*pc as usize) as i32
            }
            0x0D => {
                let d = data + x;
                *pc = pc.wrapping_add(1);
                d + 0x100 * self.rc(*pc as usize) as i32
            }
            -3 => {
                *pc = pc.wrapping_add(1);
                data + 0x100 * self.rc(*pc as usize) as i32
            }
            0x0C => ((data + x) & 0xFF) + dp,
            -4 => data + dp,
            _ => unreachable!("addr_mode off {off}"),
        }
    }

    // ---- the interpreter ----
    fn run_until_(&mut self, end_time: i32) {
        let mut rel_time = self.spc_time - end_time;
        debug_assert!(rel_time <= 0);
        self.spc_time = end_time;
        self.dsp_time += rel_time;
        for i in 0..TIMER_COUNT {
            self.timers[i].next_time += rel_time;
        }

        let mut a = self.a;
        let mut x = self.x;
        let mut y = self.y;
        let mut pc = self.pc;
        let mut sp = self.sp;
        let mut psw: i32;
        let mut c: i32;
        let mut nz: i32;
        let mut dp: i32;
        {
            let inp = self.psw as i32;
            psw = inp;
            c = inp << 8;
            dp = inp << 3 & 0x100;
            nz = (inp << 4 & 0x800) | (!inp & Z02);
        }

        let mut stopped = false;

        'run: loop {
            let opcode = self.rc(pc as usize);
            rel_time += self.cycle_table[opcode as usize] as i32;
            if rel_time > 0 {
                rel_time -= self.cycle_table[self.rc(pc as usize) as usize] as i32;
                break 'run;
            }
            pc = pc.wrapping_add(1);
            let mut data = self.rc(pc as usize) as i32;

            // --- helper macros (capture self, rel_time, pc, etc.) ---
            macro_rules! read {
                ($t:expr, $addr:expr) => {
                    self.cpu_read((($addr) & 0xFFFF) as u16, rel_time + $t)
                };
            }
            macro_rules! write {
                ($t:expr, $addr:expr, $d:expr) => {
                    self.cpu_write($d, (($addr) & 0xFFFF) as u16, rel_time + $t)
                };
            }
            macro_rules! push {
                ($d:expr) => {{
                    self.wc(0x100 + sp as usize, ($d) as u8);
                    sp = sp.wrapping_sub(1);
                }};
            }
            macro_rules! push16 {
                ($d:expr) => {{
                    let v = ($d) as i32;
                    push!((v >> 8) & 0xFF);
                    push!(v & 0xFF);
                }};
            }
            macro_rules! pop {
                () => {{
                    sp = sp.wrapping_add(1);
                    self.rc(0x100 + sp as usize)
                }};
            }
            macro_rules! get_psw {
                () => {{
                    let mut out = psw & !(N80 | P20 | Z02 | 0x01);
                    out |= c >> 8 & 0x01;
                    out |= dp >> 3 & P20;
                    out |= ((nz >> 4) | nz) & N80;
                    if (nz as u8) == 0 {
                        out |= Z02;
                    }
                    out
                }};
            }
            macro_rules! set_psw {
                ($inp:expr) => {{
                    let inp = $inp;
                    psw = inp;
                    c = inp << 8;
                    dp = inp << 3 & 0x100;
                    nz = (inp << 4 & 0x800) | (!inp & Z02);
                }};
            }
            macro_rules! mem_bit {
                ($rel:expr) => {{
                    let addr = self.rp16(pc as i32);
                    let t = self.cpu_read((addr & 0x1FFF) as u16, rel_time + $rel) >> (addr >> 13);
                    t << 8 & 0x100
                }};
            }
            macro_rules! branch {
                ($cond:expr) => {{
                    pc = pc.wrapping_add(1);
                    let disp = (data as i8 as i16) as u16;
                    pc = pc.wrapping_add(disp);
                    if $cond {
                        continue 'run;
                    }
                    pc = pc.wrapping_sub(disp);
                    rel_time -= 2;
                    continue 'run;
                }};
            }
            macro_rules! cbranch {
                ($cond:expr) => {{
                    pc = pc.wrapping_add(1);
                    if $cond {
                        pc = pc.wrapping_add(self.rc(pc as usize) as i8 as i16 as u16);
                        pc = pc.wrapping_add(1);
                    } else {
                        rel_time -= 2;
                        pc = pc.wrapping_add(1);
                    }
                    continue 'run;
                }};
            }

            match opcode {
                // ---- branches (common) ----
                0xF0 => branch!((nz as u8) == 0),           // BEQ
                0xD0 => branch!((nz as u8) != 0),           // BNE
                0x30 => branch!((nz & NZ_NEG_MASK) != 0),   // BMI
                0x10 => branch!((nz & NZ_NEG_MASK) == 0),   // BPL
                0xB0 => branch!((c & 0x100) != 0),          // BCS
                0x90 => branch!((c & 0x100) == 0),          // BCC
                0x70 => branch!((psw & V40) != 0),          // BVS
                0x50 => branch!((psw & V40) == 0),          // BVC
                0x2F => {
                    // BRA
                    pc = pc.wrapping_add(data as i8 as i16 as u16);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                0x3F => {
                    // CALL
                    let old_addr = (pc as i32 + 2) & 0xFFFF;
                    let target = self.rp16(pc as i32);
                    pc = target as u16;
                    push16!(old_addr);
                    continue 'run;
                }
                0x6F => {
                    // RET
                    let l = pop!() as i32;
                    let h = pop!() as i32;
                    pc = (l | (h << 8)) as u16;
                    continue 'run;
                }

                0xE4 => {
                    // MOV a, dp (timer)
                    pc = pc.wrapping_add(1);
                    let v = read!(0, dp + data);
                    nz = v;
                    a = v as u8;
                    continue 'run;
                }
                0xFA => {
                    // MOV dp, dp
                    let temp = read!(-2, dp + data);
                    let src = temp + NO_READ_BEFORE_WRITE;
                    let dst = self.rc(pc.wrapping_add(1) as usize) as i32;
                    pc = pc.wrapping_add(2);
                    write!(0, dp + dst, src);
                    continue 'run;
                }
                0x8F => {
                    // MOV dp, #imm
                    let temp = self.rc(pc.wrapping_add(1) as usize) as i32;
                    pc = pc.wrapping_add(2);
                    write!(0, dp + temp, data);
                    continue 'run;
                }
                0xC4 => {
                    // MOV dp, a
                    pc = pc.wrapping_add(1);
                    write!(0, dp + data, a as i32);
                    continue 'run;
                }

                // ---- MOV A, addr (base 0xE8, NO_DP addr modes) ----
                0xE5 | 0xE6 | 0xE7 | 0xF4 | 0xF5 | 0xF6 | 0xF7 => {
                    let addr = self.addr_mode(opcode, 0xE8, &mut pc, data, dp, x as i32, y as i32);
                    let v = read!(0, addr);
                    nz = v;
                    a = v as u8;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xBF => {
                    // MOV A,(X)+
                    let temp = x as i32 + dp;
                    x = x.wrapping_add(1);
                    let v = read!(-1, temp);
                    nz = v;
                    a = v as u8;
                    continue 'run;
                }
                0xE8 => {
                    // MOV A, imm
                    a = data as u8;
                    nz = data;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xF9 => {
                    // MOV X, dp+Y
                    data = (data + y as i32) & 0xFF;
                    let v = read!(0, dp + data);
                    nz = v;
                    x = v as u8;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xF8 => {
                    // MOV X, dp
                    let v = read!(0, dp + data);
                    nz = v;
                    x = v as u8;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xE9 => {
                    // MOV X, abs
                    let addr = self.rp16(pc as i32);
                    pc = pc.wrapping_add(1);
                    let v = read!(0, addr);
                    nz = v;
                    x = v as u8;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xCD => {
                    // MOV X, imm
                    x = data as u8;
                    nz = data;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xFB => {
                    // MOV Y, dp+X
                    data = (data + x as i32) & 0xFF;
                    pc = pc.wrapping_add(1);
                    let v = read!(0, dp + data);
                    nz = v;
                    y = v as u8;
                    continue 'run;
                }
                0xEB => {
                    // MOV Y, dp
                    pc = pc.wrapping_add(1);
                    let v = read!(0, dp + data);
                    nz = v;
                    y = v as u8;
                    continue 'run;
                }
                0xEC => {
                    // MOV Y, abs
                    let temp = self.rp16(pc as i32);
                    pc = pc.wrapping_add(2);
                    let v = read!(0, temp);
                    nz = v;
                    y = v as u8;
                    continue 'run;
                }
                0x8D => {
                    // MOV Y, imm
                    y = data as u8;
                    nz = data;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                // ---- MOV addr, A (base 0xC8, NO_DP) ----
                0xC5 | 0xC6 | 0xC7 | 0xD4 | 0xD5 | 0xD6 | 0xD7 => {
                    let addr = self.addr_mode(opcode, 0xC8, &mut pc, data, dp, x as i32, y as i32);
                    write!(0, addr, a as i32);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xCC => {
                    // MOV abs, Y
                    let addr = self.rp16(pc as i32);
                    write!(0, addr, y as i32);
                    pc = pc.wrapping_add(2);
                    continue 'run;
                }
                0xC9 => {
                    // MOV abs, X
                    let addr = self.rp16(pc as i32);
                    write!(0, addr, x as i32);
                    pc = pc.wrapping_add(2);
                    continue 'run;
                }
                0xD9 => {
                    // MOV dp+Y, X
                    data = (data + y as i32) & 0xFF;
                    write!(0, data + dp, x as i32);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xD8 => {
                    // MOV dp, X
                    write!(0, data + dp, x as i32);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xDB => {
                    // MOV dp+X, Y
                    data = (data + x as i32) & 0xFF;
                    write!(0, data + dp, y as i32);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xCB => {
                    // MOV dp, Y
                    write!(0, data + dp, y as i32);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                // ---- reg<->reg MOVs ----
                0x7D => { a = x; nz = x as i32; continue 'run; } // MOV A,X
                0xDD => { a = y; nz = y as i32; continue 'run; } // MOV A,Y
                0x5D => { x = a; nz = a as i32; continue 'run; } // MOV X,A
                0xFD => { y = a; nz = a as i32; continue 'run; } // MOV Y,A
                0x9D => { x = sp; nz = sp as i32; continue 'run; } // MOV X,SP
                0xBD => { sp = x; continue 'run; } // MOV SP,X
                0xAF => {
                    // MOV (X)+, A
                    write!(0, dp + x as i32, a as i32 + NO_READ_BEFORE_WRITE);
                    x = x.wrapping_add(1);
                    continue 'run;
                }

                // ---- logical (AND/OR/EOR) addr modes ----
                0x24 | 0x25 | 0x26 | 0x27 | 0x34 | 0x35 | 0x36 | 0x37 => {
                    let addr = self.addr_mode(opcode, 0x28, &mut pc, data, dp, x as i32, y as i32);
                    let v = read!(0, addr);
                    let r = a as i32 & v;
                    a = r as u8;
                    nz = r;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0x28 => { let r = a as i32 & data; a = r as u8; nz = r; pc = pc.wrapping_add(1); continue 'run; }
                0x04 | 0x05 | 0x06 | 0x07 | 0x14 | 0x15 | 0x16 | 0x17 => {
                    let addr = self.addr_mode(opcode, 0x08, &mut pc, data, dp, x as i32, y as i32);
                    let v = read!(0, addr);
                    let r = a as i32 | v;
                    a = r as u8;
                    nz = r;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0x08 => { let r = a as i32 | data; a = r as u8; nz = r; pc = pc.wrapping_add(1); continue 'run; }
                0x44 | 0x45 | 0x46 | 0x47 | 0x54 | 0x55 | 0x56 | 0x57 => {
                    let addr = self.addr_mode(opcode, 0x48, &mut pc, data, dp, x as i32, y as i32);
                    let v = read!(0, addr);
                    let r = a as i32 ^ v;
                    a = r as u8;
                    nz = r;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0x48 => { let r = a as i32 ^ data; a = r as u8; nz = r; pc = pc.wrapping_add(1); continue 'run; }

                // logical X,Y / dp,dp / dp,imm forms
                0x39 | 0x29 | 0x38 // AND
                | 0x19 | 0x09 | 0x18 // OR
                | 0x59 | 0x49 | 0x58 // EOR
                => {
                    let is_and = matches!(opcode, 0x39 | 0x29 | 0x38);
                    let is_or = matches!(opcode, 0x19 | 0x09 | 0x18);
                    let src;
                    let addr;
                    match opcode {
                        0x39 | 0x19 | 0x59 => {
                            // X,Y  (op+0x11)
                            src = read!(-2, dp + y as i32);
                            addr = x as i32 + dp;
                        }
                        0x29 | 0x09 | 0x49 => {
                            // dp,dp  (op+0x01)
                            src = read!(-3, dp + data);
                            let addr2 = pc.wrapping_add(1);
                            pc = pc.wrapping_add(2);
                            addr = self.rc(addr2 as usize) as i32 + dp;
                        }
                        _ => {
                            // dp,imm  (op+0x10): 0x38,0x18,0x58
                            src = data;
                            let addr2 = pc.wrapping_add(1);
                            pc = pc.wrapping_add(2);
                            addr = self.rc(addr2 as usize) as i32 + dp;
                        }
                    }
                    let dst = read!(-1, addr);
                    let r = if is_and {
                        src & dst
                    } else if is_or {
                        src | dst
                    } else {
                        src ^ dst
                    };
                    nz = r;
                    write!(0, addr, r);
                    continue 'run;
                }

                // ---- arithmetic CMP ----
                0x64 | 0x65 | 0x66 | 0x67 | 0x74 | 0x75 | 0x76 | 0x77 => {
                    let addr = self.addr_mode(opcode, 0x68, &mut pc, data, dp, x as i32, y as i32);
                    let v = read!(0, addr);
                    nz = a as i32 - v;
                    c = !nz;
                    nz &= 0xFF;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0x68 => {
                    nz = a as i32 - data;
                    c = !nz;
                    nz &= 0xFF;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0x79 => {
                    // CMP (X),(Y)
                    let d = read!(-2, dp + y as i32);
                    nz = read!(-1, dp + x as i32) - d;
                    c = !nz;
                    nz &= 0xFF;
                    continue 'run;
                }
                0x69 => {
                    // CMP dp,dp
                    let d = read!(-3, dp + data);
                    let dst = self.rc(pc.wrapping_add(1) as usize) as i32;
                    pc = pc.wrapping_add(1);
                    nz = read!(-1, dp + dst) - d;
                    c = !nz;
                    nz &= 0xFF;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0x78 => {
                    // CMP dp,imm
                    let dst = self.rc(pc.wrapping_add(1) as usize) as i32;
                    pc = pc.wrapping_add(1);
                    nz = read!(-1, dp + dst) - data;
                    c = !nz;
                    nz &= 0xFF;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0x3E => {
                    // CMP X,dp
                    let v = read!(0, dp + data);
                    nz = x as i32 - v;
                    c = !nz;
                    nz &= 0xFF;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0x1E => {
                    // CMP X,abs
                    let addr = self.rp16(pc as i32);
                    pc = pc.wrapping_add(1);
                    let v = read!(0, addr);
                    nz = x as i32 - v;
                    c = !nz;
                    nz &= 0xFF;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xC8 => {
                    // CMP X,imm
                    nz = x as i32 - data;
                    c = !nz;
                    nz &= 0xFF;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0x7E => {
                    // CMP Y,dp
                    let v = read!(0, dp + data);
                    nz = y as i32 - v;
                    c = !nz;
                    nz &= 0xFF;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0x5E => {
                    // CMP Y,abs
                    let addr = self.rp16(pc as i32);
                    pc = pc.wrapping_add(1);
                    let v = read!(0, addr);
                    nz = y as i32 - v;
                    c = !nz;
                    nz &= 0xFF;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xAD => {
                    // CMP Y,imm
                    nz = y as i32 - data;
                    c = !nz;
                    nz &= 0xFF;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                // ---- INC/DEC reg ----
                0xBC => { nz = a as i32 + 1; a = nz as u8; continue 'run; }
                0x3D => { nz = x as i32 + 1; x = nz as u8; continue 'run; }
                0xFC => { nz = y as i32 + 1; y = nz as u8; continue 'run; }
                0x9C => { nz = a as i32 - 1; a = nz as u8; continue 'run; }
                0x1D => { nz = x as i32 - 1; x = nz as u8; continue 'run; }
                0xDC => { nz = y as i32 - 1; y = nz as u8; continue 'run; }

                // INC/DEC memory
                0x9B | 0xBB | 0x8B | 0xAB | 0x8C | 0xAC => {
                    let addr = match opcode {
                        0x9B | 0xBB => { let d = (data + x as i32) & 0xFF; d + dp }
                        0x8B | 0xAB => data + dp,
                        _ => {
                            let a2 = self.rp16(pc as i32);
                            pc = pc.wrapping_add(1);
                            a2
                        }
                    };
                    nz = (opcode as i32 >> 4 & 2) - 1;
                    nz += read!(-1, addr);
                    write!(0, addr, nz);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                // ---- shifts / rotates on A ----
                0x5C | 0x7C => {
                    // LSR A / ROR A
                    if opcode == 0x5C {
                        c = 0;
                    }
                    nz = (c >> 1 & 0x80) | (a as i32 >> 1);
                    c = (a as i32) << 8;
                    a = nz as u8;
                    continue 'run;
                }
                0x1C | 0x3C => {
                    // ASL A / ROL A
                    if opcode == 0x1C {
                        c = 0;
                    }
                    let temp = c >> 8 & 1;
                    c = (a as i32) << 1;
                    nz = c | temp;
                    a = nz as u8;
                    continue 'run;
                }

                // ---- shifts / rotates on memory (ASL/ROL) ----
                0x0B | 0x1B | 0x3B | 0x2B | 0x0C | 0x2C => {
                    let addr = match opcode {
                        0x0B => { c = 0; data + dp }
                        0x1B => { c = 0; ((data + x as i32) & 0xFF) + dp }
                        0x3B => { ((data + x as i32) & 0xFF) + dp }
                        0x2B => data + dp,
                        _ => {
                            if opcode == 0x0C {
                                c = 0;
                            }
                            let a2 = self.rp16(pc as i32);
                            pc = pc.wrapping_add(1);
                            a2
                        }
                    };
                    nz = c >> 8 & 1;
                    c = read!(-1, addr) << 1;
                    nz |= c;
                    write!(0, addr, nz);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                // LSR/ROR memory
                0x4B | 0x5B | 0x7B | 0x6B | 0x4C | 0x6C => {
                    let addr = match opcode {
                        0x4B => { c = 0; data + dp }
                        0x5B => { c = 0; ((data + x as i32) & 0xFF) + dp }
                        0x7B => { ((data + x as i32) & 0xFF) + dp }
                        0x6B => data + dp,
                        _ => {
                            if opcode == 0x4C {
                                c = 0;
                            }
                            let a2 = self.rp16(pc as i32);
                            pc = pc.wrapping_add(1);
                            a2
                        }
                    };
                    let temp = read!(-1, addr);
                    nz = (c >> 1 & 0x80) | (temp >> 1);
                    c = temp << 8;
                    write!(0, addr, nz);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                0x9F => {
                    // XCN
                    a = (a >> 4) | (a << 4);
                    nz = a as i32;
                    continue 'run;
                }

                // ---- 16-bit transfers ----
                0xBA => {
                    // MOVW YA, dp
                    let lo = read!(-2, dp + data);
                    a = lo as u8;
                    nz = (lo & 0x7F) | (lo >> 1);
                    let hi = read!(0, dp + ((data + 1) & 0xFF));
                    y = hi as u8;
                    nz |= hi;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }
                0xDA => {
                    // MOVW dp, YA
                    write!(-1, dp + data, a as i32);
                    write!(0, dp + ((data + 1) & 0xFF), y as i32 + NO_READ_BEFORE_WRITE);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                // ---- 16-bit INCW/DECW ----
                0x3A | 0x1A => {
                    let a0 = data + dp;
                    let mut temp = read!(-3, a0);
                    temp += (opcode as i32 >> 4 & 2) - 1;
                    nz = ((temp >> 1) | temp) & 0x7F;
                    write!(-2, a0, temp);
                    let a1 = ((data + 1) & 0xFF) + dp;
                    temp = ((temp >> 8) + read!(-1, a1)) & 0xFF;
                    nz |= temp;
                    write!(0, a1, temp);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                0x7A | 0x9A => {
                    // ADDW / SUBW YA, dp
                    let mut lo = read!(-2, dp + data);
                    let mut hi = read!(0, dp + ((data + 1) & 0xFF));
                    if opcode == 0x9A {
                        lo = (lo ^ 0xFF) + 1;
                        hi ^= 0xFF;
                    }
                    lo += a as i32;
                    let mut result = y as i32 + hi + (lo >> 8);
                    let flags = hi ^ y as i32 ^ result;
                    psw = (psw & !(V40 | H08)) | (flags >> 1 & H08) | ((flags + 0x80) >> 2 & V40);
                    c = result;
                    a = lo as u8;
                    result &= 0xFF;
                    y = result as u8;
                    nz = (((lo >> 1) | lo) & 0x7F) | result;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                0x5A => {
                    // CMPW YA, dp
                    let mut temp = a as i32 - read!(-1, dp + data);
                    nz = ((temp >> 1) | temp) & 0x7F;
                    temp = y as i32 + (temp >> 8);
                    temp -= read!(0, dp + ((data + 1) & 0xFF));
                    nz |= temp;
                    c = !temp;
                    nz &= 0xFF;
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                0xCF => {
                    // MUL YA
                    let temp = (y as u32) * (a as u32);
                    a = temp as u8;
                    nz = (((temp >> 1) | temp) & 0x7F) as i32;
                    y = (temp >> 8) as u8;
                    nz |= y as i32;
                    continue 'run;
                }
                0x9E => {
                    // DIV YA, X
                    let ya = (y as u32) * 0x100 + a as u32;
                    psw &= !(H08 | V40);
                    if y >= x {
                        psw |= V40;
                    }
                    if (y & 15) >= (x & 15) {
                        psw |= H08;
                    }
                    let (na, ny);
                    if (y as u32) < (x as u32) * 2 {
                        na = ya / x as u32;
                        ny = ya - na * x as u32;
                    } else {
                        na = 255 - (ya - (x as u32) * 0x200) / (256 - x as u32);
                        ny = x as u32 + (ya - (x as u32) * 0x200) % (256 - x as u32);
                    }
                    nz = (na as u8) as i32;
                    a = na as u8;
                    y = ny as u8;
                    continue 'run;
                }

                0xDF => {
                    // DAA
                    if a > 0x99 || (c & 0x100) != 0 {
                        a = a.wrapping_add(0x60);
                        c = 0x100;
                    }
                    if (a & 0x0F) > 9 || (psw & H08) != 0 {
                        a = a.wrapping_add(0x06);
                    }
                    nz = a as i32;
                    continue 'run;
                }
                0xBE => {
                    // DAS
                    if a > 0x99 || (c & 0x100) == 0 {
                        a = a.wrapping_sub(0x60);
                        c = 0;
                    }
                    if (a & 0x0F) > 9 || (psw & H08) == 0 {
                        a = a.wrapping_sub(0x06);
                    }
                    nz = a as i32;
                    continue 'run;
                }

                // ---- bit-conditional branches BBS/BBC ----
                0x03 | 0x23 | 0x43 | 0x63 | 0x83 | 0xA3 | 0xC3 | 0xE3 => {
                    let bitv = read!(-4, dp + data) >> (opcode as i32 >> 5) & 1;
                    cbranch!(bitv != 0);
                }
                0x13 | 0x33 | 0x53 | 0x73 | 0x93 | 0xB3 | 0xD3 | 0xF3 => {
                    let bitv = read!(-4, dp + data) >> (opcode as i32 >> 5) & 1;
                    cbranch!(bitv == 0);
                }

                0xDE => {
                    // CBNE dp+X, rel
                    data = (data + x as i32) & 0xFF;
                    let temp = read!(-4, dp + data);
                    cbranch!(temp != a as i32);
                }
                0x2E => {
                    // CBNE dp, rel
                    let temp = read!(-4, dp + data);
                    cbranch!(temp != a as i32);
                }
                0x6E => {
                    // DBNZ dp, rel
                    let temp = (read!(-4, dp + data) - 1) & 0xFF;
                    write!(-3, dp + (data & 0xFF), temp + NO_READ_BEFORE_WRITE);
                    cbranch!(temp != 0);
                }
                0xFE => {
                    // DBNZ Y, rel
                    y = y.wrapping_sub(1);
                    branch!(y != 0);
                }

                0x1F => {
                    // JMP [abs+X]
                    let addr = (self.rp16(pc as i32) + x as i32) & 0xFFFF;
                    pc = self.rp16(addr) as u16;
                    continue 'run;
                }
                0x5F => {
                    // JMP abs
                    pc = self.rp16(pc as i32) as u16;
                    continue 'run;
                }

                0x0F => {
                    // BRK
                    let ret_addr = pc as i32;
                    pc = self.rp16(0xFFDE) as u16;
                    push16!(ret_addr);
                    let temp = get_psw!();
                    psw = (psw | B10) & !I04;
                    push!(temp);
                    continue 'run;
                }
                0x4F => {
                    // PCALL
                    let ret_addr = (pc as i32 + 1) & 0xFFFF;
                    pc = (0xFF00 | data) as u16;
                    push16!(ret_addr);
                    continue 'run;
                }
                0x01 | 0x11 | 0x21 | 0x31 | 0x41 | 0x51 | 0x61 | 0x71 | 0x81 | 0x91 | 0xA1
                | 0xB1 | 0xC1 | 0xD1 | 0xE1 | 0xF1 => {
                    // TCALL n
                    let ret_addr = pc as i32;
                    pc = self.rp16(0xFFDE - (opcode as i32 >> 3)) as u16;
                    push16!(ret_addr);
                    continue 'run;
                }

                // ---- stack ops ----
                0x7F => {
                    // RET1
                    let temp = pop!() as i32;
                    let l = pop!() as i32;
                    let h = pop!() as i32;
                    pc = (l | (h << 8)) as u16;
                    set_psw!(temp);
                    continue 'run;
                }
                0x8E => {
                    // POP PSW
                    let temp = pop!() as i32;
                    set_psw!(temp);
                    continue 'run;
                }
                0x0D => {
                    let temp = get_psw!();
                    push!(temp);
                    continue 'run;
                }
                0x2D => { push!(a as i32); continue 'run; }
                0x4D => { push!(x as i32); continue 'run; }
                0x6D => { push!(y as i32); continue 'run; }
                0xAE => { a = pop!(); continue 'run; }
                0xCE => { x = pop!(); continue 'run; }
                0xEE => { y = pop!(); continue 'run; }

                // ---- SET1/CLR1 dp.bit ----
                0x02 | 0x22 | 0x42 | 0x62 | 0x82 | 0xA2 | 0xC2 | 0xE2 | 0x12 | 0x32 | 0x52
                | 0x72 | 0x92 | 0xB2 | 0xD2 | 0xF2 => {
                    let mut bit = 1 << (opcode as i32 >> 5);
                    let mask = !bit;
                    if opcode & 0x10 != 0 {
                        bit = 0;
                    }
                    let addr = data + dp;
                    let v = (read!(-1, addr) & mask) | bit;
                    write!(0, addr, v);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                0x0E | 0x4E => {
                    // TSET1 / TCLR1 abs
                    let addr = self.rp16(pc as i32);
                    pc = pc.wrapping_add(2);
                    let mut temp = read!(-2, addr);
                    nz = (a as i32 - temp) & 0xFF;
                    temp &= !(a as i32);
                    if opcode == 0x0E {
                        temp |= a as i32;
                    }
                    write!(0, addr, temp);
                    continue 'run;
                }

                0x4A => { c &= mem_bit!(0); pc = pc.wrapping_add(2); continue 'run; } // AND1 C,mem.bit
                0x6A => { c &= !mem_bit!(0); pc = pc.wrapping_add(2); continue 'run; } // AND1 C,/mem.bit
                0x0A => { c |= mem_bit!(-1); pc = pc.wrapping_add(2); continue 'run; } // OR1
                0x2A => { c |= !mem_bit!(-1); pc = pc.wrapping_add(2); continue 'run; } // OR1 /
                0x8A => { c ^= mem_bit!(-1); pc = pc.wrapping_add(2); continue 'run; } // EOR1
                0xEA => {
                    // NOT1 mem.bit
                    let d = self.rp16(pc as i32);
                    pc = pc.wrapping_add(2);
                    let mut temp = read!(-1, d & 0x1FFF);
                    temp ^= 1 << (d >> 13);
                    write!(0, d & 0x1FFF, temp);
                    continue 'run;
                }
                0xCA => {
                    // MOV1 mem.bit, C
                    let d = self.rp16(pc as i32);
                    pc = pc.wrapping_add(2);
                    let mut temp = read!(-2, d & 0x1FFF);
                    let bit = d >> 13;
                    temp = (temp & !(1 << bit)) | ((c >> 8 & 1) << bit);
                    write!(0, d & 0x1FFF, temp + NO_READ_BEFORE_WRITE);
                    continue 'run;
                }
                0xAA => { c = mem_bit!(0); pc = pc.wrapping_add(2); continue 'run; } // MOV1 C,mem.bit

                // ---- PSW flag ops ----
                0x60 => { c = 0; continue 'run; }          // CLRC
                0x80 => { c = !0; continue 'run; }         // SETC
                0xED => { c ^= 0x100; continue 'run; }     // NOTC
                0xE0 => { psw &= !(V40 | H08); continue 'run; } // CLRV
                0x20 => { dp = 0; continue 'run; }         // CLRP
                0x40 => { dp = 0x100; continue 'run; }     // SETP
                0xA0 => { psw |= I04; continue 'run; }     // EI
                0xC0 => { psw &= !I04; continue 'run; }    // DI

                0x00 => { continue 'run; } // NOP

                0xFF | 0xEF => {
                    // STOP / SLEEP
                    if opcode == 0xFF && pc == 0x0000 {
                        continue 'run;
                    }
                    pc = pc.wrapping_sub(1);
                    rel_time = 0;
                    self.cpu_error = true;
                    stopped = true;
                    break 'run;
                }

                // ---- ADC/SBC (handled explicitly to avoid closure gymnastics) ----
                0x88 | 0xA8 | 0x84 | 0x85 | 0x86 | 0x87 | 0x94 | 0x95 | 0x96 | 0x97 | 0xA4
                | 0xA5 | 0xA6 | 0xA7 | 0xB4 | 0xB5 | 0xB6 | 0xB7 | 0x89 | 0xA9 | 0x98 | 0xB8 => {
                    // Compute (operand value, dest addr [-1 = A], nz seed)
                    let mut dval;
                    let addr_var: i32;
                    match opcode {
                        0x88 | 0xA8 => {
                            // imm
                            dval = data;
                            addr_var = -1;
                            nz = a as i32;
                        }
                        0x89 | 0xA9 => {
                            // dp,dp
                            dval = read!(-3, dp + data);
                            let dst = self.rc(pc.wrapping_add(1) as usize) as i32;
                            pc = pc.wrapping_add(1);
                            addr_var = dp + dst;
                            nz = read!(-1, addr_var);
                        }
                        0x98 | 0xB8 => {
                            // dp,imm
                            dval = data;
                            let dst = self.rc(pc.wrapping_add(1) as usize) as i32;
                            pc = pc.wrapping_add(1);
                            addr_var = dp + dst;
                            nz = read!(-1, addr_var);
                        }
                        _ => {
                            // addr modes -> value at effective addr, dest = A.
                            // Normalize SBC (0xA_/0xB_) to the shared ADC base decoding.
                            let addr = self.addr_mode(opcode & 0xDF, 0x88, &mut pc, data, dp, x as i32, y as i32);
                            dval = read!(0, addr);
                            addr_var = -1;
                            nz = a as i32;
                        }
                    }
                    if opcode >= 0xA0 {
                        dval ^= 0xFF;
                    }
                    let mut flags = dval ^ nz;
                    nz += dval + (c >> 8 & 1);
                    flags ^= nz;
                    psw = (psw & !(V40 | H08)) | (flags >> 1 & H08) | ((flags + 0x80) >> 2 & V40);
                    c = nz;
                    if addr_var < 0 {
                        a = nz as u8;
                    } else {
                        write!(0, addr_var, nz);
                    }
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                // ADC/SBC (X),(Y) and dp,dp/dp,imm SBC variants already covered above via 0x99/0xB9? handle here:
                0xB9 | 0x99 => {
                    // (X),(Y)
                    pc = pc.wrapping_sub(1);
                    let mut dval = read!(-2, dp + y as i32);
                    let addr_var = x as i32 + dp;
                    nz = read!(-1, addr_var);
                    if opcode >= 0xA0 {
                        dval ^= 0xFF;
                    }
                    let mut flags = dval ^ nz;
                    nz += dval + (c >> 8 & 1);
                    flags ^= nz;
                    psw = (psw & !(V40 | H08)) | (flags >> 1 & H08) | ((flags + 0x80) >> 2 & V40);
                    c = nz;
                    write!(0, addr_var, nz);
                    pc = pc.wrapping_add(1);
                    continue 'run;
                }

                #[allow(unreachable_patterns)]
                _ => unreachable!("unhandled opcode {:#04x}", opcode),
            }
        }

        // uncache registers
        let _ = stopped;
        self.pc = pc;
        self.sp = sp;
        self.a = a;
        self.x = x;
        self.y = y;
        {
            let mut out = psw & !(N80 | P20 | Z02 | 0x01);
            out |= c >> 8 & 0x01;
            out |= dp >> 3 & P20;
            out |= ((nz >> 4) | nz) & N80;
            if (nz as u8) == 0 {
                out |= Z02;
            }
            self.psw = out as u8;
        }

        // epilogue
        self.spc_time += rel_time;
        self.dsp_time -= rel_time;
        for i in 0..TIMER_COUNT {
            self.timers[i].next_time -= rel_time;
        }
    }
}
