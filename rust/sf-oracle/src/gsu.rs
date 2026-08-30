//! Super-FX / GSU core (arithmetic subset) for the ROM oracle.
//!
//! Star Fox runs its 3D/angle math on the GSU (the "MARIO" chip): the 65816
//! dispatches via `runmario_l` to a GSU program (e.g. `mcallarctan16`). The
//! 65816 oracle can't run those, so this executes GSU code directly to validate
//! GSU-side functions (arctan16, 3D transforms) against the Rust port.
//!
//! The GSU ISA is prefix-modal (see tools/sf2/disasm/gsu.py, the disassembler
//! this shares opcodes with): FROM/TO/WITH set the source/dest register for the
//! *next* instruction; ALT1/2/3 modify the next opcode. Registers are 16 x
//! 16-bit; R15 is PC, R14 auto-increments ROM reads (GETB), R0 is the default
//! src/dst. This implements the ALU / immediate / branch / shift / mul / basic
//! memory ops and the pixel-plot instructions used to build the cartridge-RAM
//! framebuffer copied into SNES VRAM.
//!
//! STATUS: core + arithmetic verified by the self-test below. Memory ops
//! (LDW/STW/GETB) and end-to-end validation vs a real ROM GSU routine are the
//! next step (see memory rom-oracle-plan).

/// SFR (status/flag register) bits.
mod sfr {
    pub const Z: u16 = 1 << 1; // zero
    pub const CY: u16 = 1 << 2; // carry
    pub const S: u16 = 1 << 3; // sign
    pub const OV: u16 = 1 << 4; // overflow
    pub const G: u16 = 1 << 5; // go (running)
}

/// One eight-pixel row in the Super FX plot pipeline.  The hardware keeps a
/// primary and secondary row across STOP/restart boundaries; pixels do not
/// reach cartridge RAM until a later cache displacement (or RPIX) flushes
/// them.  Several Star Fox 2 render jobs deliberately rely on that persistence.
#[derive(Clone, Copy, Default)]
struct PixelCache {
    x: u8,
    y: u8,
    pixels: [u8; 8],
    valid_bits: u8,
}

/// GSU execution core. `rom` is the cartridge ROM (program + GETB data); `ram`
/// is the 64 KB GSU work RAM (shared with the 65816 for arguments/results).
pub struct Gsu {
    pub r: [u16; 16],
    pub ram: Vec<u8>,
    rom: Vec<u8>,
    sfr: u16,
    pbr: u8,   // program bank (R15 fetches from rom via this)
    rombr: u8, // ROM data bank (GETB)
    sreg: usize,
    dreg: usize,
    alt1: bool,
    alt2: bool,
    b_flag: bool,  // WITH set (in-place / MOVE modifier)
    last_ram: u16, // last RAM word address (for SBK)
    /// Graphics registers written through the CPU-visible GSU register file.
    scbr: u8,
    scmr: u8,
    clock_select: bool,
    high_speed_mode: bool,
    cache_base: u16,
    cache_valid: [bool; 32],
    /// Signed distance between elapsed SNES master clocks and GSU work. Mesen
    /// executes an instruction atomically once its start boundary is reached,
    /// so an expensive instruction may temporarily leave a negative balance.
    master_clock_credit: i64,
    timing_active: bool,
    current_timing_cost: u64,
    instruction_executed: bool,
    prefetched_instruction: Option<u8>,
    /// Per-job clocks: program fetch, RAM wait, ROM wait, multiply, pixel bus.
    timing_breakdown: [u64; 5],
    ram_delay: u8,
    rom_delay: u8,
    waiting_for_ram_access: bool,
    waiting_for_rom_access: bool,
    colr: u8,
    por: u8,
    primary_pixel_cache: PixelCache,
    secondary_pixel_cache: PixelCache,
    /// Pending R15 write (pbr, pc). The next opcode has already been prefetched
    /// from the sequential stream and executes as a delay slot. Before that
    /// instruction reads any operands, however, R15 already points at the new
    /// target, so a multi-byte delay instruction consumes bytes there.
    pending_branch: Option<(u8, u16)>,
    /// LJMP also rebases and invalidates the 512-byte program cache once its
    /// already-prefetched delay slot has retired.
    pending_cache_reset: bool,
    pub trace_range: Option<(u16, u16)>,
    /// Instructions executed by the most recent [`run`](Self::run).
    pub last_run_steps: u64,
    pub last_run_hit_limit: bool,
    last_run_samples: Vec<(u64, u8, u16, u16, u16, u16, u16, u16, u16)>,
    /// Total PLOT instructions executed, for retail-host coverage diagnostics.
    pub plot_count: u64,
    pub plot_write_count: u64,
    /// Optional one-address execution watch used by oracle input/capture tools.
    execution_watch: Option<u32>,
    execution_watch_ram: Option<(u16, u32, u32)>,
    execution_watch_hit: bool,
    execution_watch_visits: u64,
    execution_watch_last_ram: u32,
    execution_watch_values: Vec<u32>,
    execution_capture_watch: Option<(u32, usize, usize)>,
    execution_captures: Vec<ExecutionCapture>,
    pixel_write_watch: Option<(u8, u8)>,
    pixel_write_captures: Vec<ExecutionCapture>,
    current_instruction: u32,
    target_ram_writes: Vec<(u32, u16, u8, u8)>,
    pc_trace: Vec<u32>,
    register_trace: Vec<String>,
    trace_next_run: bool,
    trace_this_run: bool,
    point_states: Vec<(u32, [u16; 16], u16, usize, usize, bool, bool, bool)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionCapture {
    pub instruction: u32,
    pub memory: Vec<u8>,
    pub values: [u16; 16],
    pub color: u8,
}

impl Gsu {
    pub fn new(rom: Vec<u8>) -> Self {
        Gsu {
            r: [0; 16],
            ram: vec![0u8; 0x1_0000],
            rom,
            sfr: 0,
            pbr: 0,
            rombr: 0,
            sreg: 0,
            dreg: 0,
            alt1: false,
            alt2: false,
            b_flag: false,
            last_ram: 0,
            scbr: 0,
            scmr: 0,
            clock_select: false,
            high_speed_mode: false,
            cache_base: 0,
            cache_valid: [false; 32],
            master_clock_credit: 0,
            timing_active: false,
            current_timing_cost: 0,
            instruction_executed: false,
            prefetched_instruction: None,
            timing_breakdown: [0; 5],
            ram_delay: 0,
            rom_delay: 0,
            waiting_for_ram_access: false,
            waiting_for_rom_access: false,
            colr: 0,
            por: 0,
            primary_pixel_cache: PixelCache::default(),
            secondary_pixel_cache: PixelCache::default(),
            pending_branch: None,
            pending_cache_reset: false,
            trace_range: None,
            last_run_steps: 0,
            last_run_hit_limit: false,
            last_run_samples: Vec::new(),
            plot_count: 0,
            plot_write_count: 0,
            execution_watch: None,
            execution_watch_ram: None,
            execution_watch_hit: false,
            execution_watch_visits: 0,
            execution_watch_last_ram: 0,
            execution_watch_values: Vec::new(),
            execution_capture_watch: None,
            execution_captures: Vec::new(),
            pixel_write_watch: None,
            pixel_write_captures: Vec::new(),
            current_instruction: 0,
            target_ram_writes: Vec::new(),
            pc_trace: Vec::new(),
            register_trace: Vec::new(),
            trace_next_run: false,
            trace_this_run: false,
            point_states: Vec::new(),
        }
    }

    fn flag(&self, b: u16) -> bool {
        self.sfr & b != 0
    }
    fn set_flag(&mut self, b: u16, on: bool) {
        if on {
            self.sfr |= b;
        } else {
            self.sfr &= !b;
        }
    }
    fn set_zs(&mut self, v: u16) {
        self.set_flag(sfr::Z, v == 0);
        self.set_flag(sfr::S, v & 0x8000 != 0);
    }

    #[inline]
    fn rom_byte(&self, bank: u8, address: u16) -> u8 {
        let offset = if bank >= 0x40 {
            (usize::from(bank & 0x1F) << 16) | usize::from(address)
        } else {
            (usize::from(bank & 0x3F) << 15) | (usize::from(address) & 0x7FFF)
        };
        self.rom
            .get(offset % self.rom.len().max(1))
            .copied()
            .unwrap_or(0)
    }

    #[inline]
    fn memory_latency(&self) -> u8 {
        if self.clock_select {
            5
        } else {
            6
        }
    }

    /// Advance the GSU's internal clock and let pending RAM/ROM-buffer
    /// operations overlap that elapsed time, as they do in silicon.
    fn time_step(&mut self, clocks: u64) {
        if !self.timing_active || clocks == 0 {
            return;
        }
        self.current_timing_cost = self.current_timing_cost.saturating_add(clocks);
        let elapsed = clocks.min(u64::from(u8::MAX)) as u8;
        let ram_was_pending = self.ram_delay != 0;
        let rom_was_pending = self.rom_delay != 0;
        self.ram_delay = self.ram_delay.saturating_sub(elapsed);
        self.rom_delay = self.rom_delay.saturating_sub(elapsed);
        if ram_was_pending && self.ram_delay == 0 {
            self.time_require_ram_access();
        }
        if rom_was_pending && self.rom_delay == 0 {
            self.time_require_rom_access();
        }
    }

    fn time_charge(&mut self, category: usize, clocks: u64) {
        if self.timing_active {
            self.timing_breakdown[category] =
                self.timing_breakdown[category].saturating_add(clocks);
        }
        self.time_step(clocks);
    }

    fn time_wait_ram(&mut self) {
        self.time_charge(1, u64::from(self.ram_delay));
    }

    fn time_wait_rom(&mut self) {
        self.time_charge(2, u64::from(self.rom_delay));
    }

    fn time_require_ram_access(&mut self) {
        if self.timing_active && self.scmr & 0x08 == 0 {
            self.waiting_for_ram_access = true;
        }
    }

    fn time_require_rom_access(&mut self) {
        if self.timing_active && self.scmr & 0x10 == 0 {
            self.waiting_for_rom_access = true;
        }
    }

    /// Charge a program-byte read, including a 16-byte cache-line fill. Cache
    /// fills arbitrate for the same ROM/RAM operation pipeline as GETB and the
    /// data-memory instructions.
    fn time_program_fetch(&mut self, pc: u16) {
        if !self.timing_active {
            return;
        }
        let cache_address = pc.wrapping_sub(self.cache_base);
        if cache_address < 512 {
            let line = usize::from(cache_address >> 4);
            if !self.cache_valid[line] {
                if self.pbr >= 0x60 {
                    self.time_wait_ram();
                    self.time_require_ram_access();
                } else {
                    self.time_wait_rom();
                    self.time_require_rom_access();
                }
                self.time_charge(0, u64::from(self.memory_latency()) * 16);
            }
            self.time_charge(0, if self.clock_select { 1 } else { 2 });
        } else {
            if self.pbr >= 0x60 {
                self.time_wait_ram();
                self.time_require_ram_access();
            } else {
                self.time_wait_rom();
                self.time_require_rom_access();
            }
            self.time_charge(0, u64::from(self.memory_latency()));
        }
    }

    /// Program fetch at R15. Banks $00-$5f execute from the Super FX ROM
    /// windows; banks $60-$7f execute generated code from cartridge RAM.
    fn fetch(&mut self) -> u8 {
        let pc = self.r[15];
        self.time_program_fetch(pc);
        let cache_address = pc.wrapping_sub(self.cache_base);
        if cache_address < 512 {
            self.cache_valid[usize::from(cache_address >> 4)] = true;
        }
        let b = if self.pbr >= 0x60 {
            self.ram[usize::from(pc)]
        } else {
            self.rom_byte(self.pbr, pc)
        };
        self.r[15] = pc.wrapping_add(1);
        b
    }

    /// Start execution at `pbr:pc`, run to STOP (or the cycle guard).
    pub fn run(&mut self, pbr: u8, pc: u16) {
        // SF2's full-screen transforms contain several nested 16-bit loops;
        // valid jobs exceed five million instructions. Keep a high emergency
        // guard for corrupt execution without truncating real cartridge work.
        self.run_with_limit(pbr, pc, 50_000_000);
    }

    /// Start execution with an explicit instruction guard for trace probes.
    pub fn run_with_limit(&mut self, pbr: u8, pc: u16, max_steps: u64) {
        self.start(pbr, pc);
        self.run_slice(max_steps);
        self.last_run_hit_limit = self.is_running() && self.last_run_steps == max_steps;
    }

    /// Arm a job without executing it.  The retail host uses this together
    /// with [`Self::run_slice`] to advance the 65816 and Super FX concurrently.
    pub fn start(&mut self, pbr: u8, pc: u16) {
        let pbr = pbr & 0x7F;
        if self.pbr != pbr {
            self.pbr = pbr;
            self.cache_valid.fill(false);
        }
        self.r[15] = pc;
        self.master_clock_credit = 0;
        self.pending_branch = None;
        self.pending_cache_reset = false;
        self.prefetched_instruction = None;
        self.set_flag(sfr::G, true);
        self.last_run_steps = 0;
        self.last_run_hit_limit = false;
        self.last_run_samples.clear();
        self.target_ram_writes.clear();
        self.timing_breakdown = [0; 5];
        self.pc_trace.clear();
        self.register_trace.clear();
        self.trace_this_run = std::mem::take(&mut self.trace_next_run);
        self.point_states.clear();
        self.last_run_samples.push((
            0, self.pbr, self.r[15], self.r[0], self.r[1], self.r[12], self.r[13], self.r[14],
            self.sfr,
        ));
    }

    /// Capture the next job's instruction/register trace for an oracle diff.
    pub fn trace_next_run(&mut self) {
        self.trace_next_run = true;
    }

    fn execute_one(&mut self, timed: bool) -> u64 {
        // The delay opcode itself was prefetched before the prior R15 write.
        // Apply the write after fetching that opcode but before it fetches any
        // operands, matching the GSU's one-byte program-read buffer.
        let redirect = self.pending_branch.take();
        let reset_cache = std::mem::take(&mut self.pending_cache_reset);
        self.timing_active = timed;
        self.current_timing_cost = 0;
        self.instruction_executed = false;
        self.step(redirect, reset_cache);
        self.timing_active = false;
        if self.instruction_executed {
            self.last_run_steps += 1;
            if self.last_run_steps % 100_000 == 0 {
                self.last_run_samples.push((
                    self.last_run_steps,
                    self.pbr,
                    self.r[15],
                    self.r[0],
                    self.r[1],
                    self.r[12],
                    self.r[13],
                    self.r[14],
                    self.sfr,
                ));
            }
        }
        self.current_timing_cost
    }

    /// Execute at most `max_steps` instructions of the armed job.  Returns
    /// true once STOP has been reached. Direct oracle calls intentionally
    /// ignore wall-clock timing.
    pub fn run_slice(&mut self, max_steps: u64) -> bool {
        let target = self.last_run_steps.saturating_add(max_steps);
        while self.flag(sfr::G) && self.last_run_steps < target {
            self.execute_one(false);
        }
        !self.flag(sfr::G)
    }

    /// Advance by real SNES master clocks. Unlike `run_slice`, this waits for
    /// program-cache, RAM/ROM-buffer, multiply, and pixel-bus costs. Like the
    /// hardware, an instruction begins once its boundary is reached and may
    /// atomically overshoot the current master clock by its remaining cost.
    pub fn run_master_slice(&mut self, master_clocks: u64) -> bool {
        if self.waiting_for_ram_access || self.waiting_for_rom_access {
            self.master_clock_credit = self
                .master_clock_credit
                .saturating_add(master_clocks.min(i64::MAX as u64) as i64)
                .min(0);
            return !self.flag(sfr::G);
        }
        self.master_clock_credit = self
            .master_clock_credit
            .saturating_add(master_clocks.min(i64::MAX as u64) as i64);
        while self.flag(sfr::G)
            && !self.waiting_for_ram_access
            && !self.waiting_for_rom_access
            && self.master_clock_credit > 0
        {
            let cost = self.execute_one(true).max(1);
            self.master_clock_credit = self
                .master_clock_credit
                .saturating_sub(cost.min(i64::MAX as u64) as i64);
        }
        !self.flag(sfr::G)
    }

    pub fn is_running(&self) -> bool {
        self.flag(sfr::G)
    }

    /// SCMR bits 3/4 grant the GSU its RAM and ROM buses respectively.
    pub fn memory_access_enabled(&self) -> bool {
        self.scmr & 0x18 == 0x18
    }

    pub fn bus_waiting(&self) -> bool {
        self.waiting_for_ram_access || self.waiting_for_rom_access
    }

    /// SCMR bit 3 grants the GSU exclusive access to cartridge RAM while it is
    /// running.  The SNES CPU sees zero on reads and drops writes until the bit
    /// is cleared (or the GSU stops).
    pub fn ram_access_enabled(&self) -> bool {
        self.scmr & 0x08 != 0
    }

    pub fn stop_at_instruction_limit(&mut self) {
        self.last_run_hit_limit = true;
        self.set_flag(sfr::G, false);
    }

    /// `(program_bank, pc, status)` after the most recent instruction.
    pub fn execution_state(&self) -> (u8, u16, u16) {
        (self.pbr, self.r[15], self.sfr)
    }

    pub fn last_run_samples(&self) -> Vec<(u64, u8, u16, u16, u16, u16, u16, u16, u16)> {
        self.last_run_samples.clone()
    }

    pub fn watch_execution(&mut self, pbr: u8, pc: u16) {
        self.execution_watch = Some((u32::from(pbr) << 16) | u32::from(pc));
        self.execution_watch_ram = None;
        self.execution_watch_hit = false;
        self.execution_watch_visits = 0;
        self.execution_watch_values.clear();
    }

    pub fn watch_execution_with_ram_mask(
        &mut self,
        pbr: u8,
        pc: u16,
        ram_address: u16,
        value: u32,
        mask: u32,
    ) {
        self.execution_watch = Some((u32::from(pbr) << 16) | u32::from(pc));
        self.execution_watch_ram = Some((ram_address, value, mask));
        self.execution_watch_hit = false;
        self.execution_watch_visits = 0;
        self.execution_watch_values.clear();
    }

    pub fn execution_watch_hit(&self) -> bool {
        self.execution_watch_hit
    }

    pub fn execution_watch_state(&self) -> (u64, u32, bool) {
        (
            self.execution_watch_visits,
            self.execution_watch_last_ram,
            self.execution_watch_hit,
        )
    }

    pub fn execution_watch_values(&self) -> Vec<u32> {
        self.execution_watch_values.clone()
    }

    /// Capture one bounded RAM window whenever the selected instruction is
    /// entered. This is oracle-only instrumentation for locating the first
    /// renderer-stage divergence.
    pub fn watch_execution_capture(
        &mut self,
        program_bank: u8,
        instruction: u16,
        ram_start: usize,
        ram_len: usize,
    ) {
        assert!(ram_start.saturating_add(ram_len) <= self.ram.len());
        self.execution_capture_watch = Some((
            (u32::from(program_bank) << 16) | u32::from(instruction),
            ram_start,
            ram_len,
        ));
        self.execution_captures.clear();
    }

    pub fn take_execution_captures(&mut self) -> Vec<ExecutionCapture> {
        std::mem::take(&mut self.execution_captures)
    }

    /// Capture every non-transparent write to one source-bitmap pixel. This
    /// is oracle-only instrumentation used to identify the primitive that
    /// produced a differing retail pixel.
    pub fn watch_pixel_writes(&mut self, x: u8, y: u8) {
        self.pixel_write_watch = Some((x, y));
        self.pixel_write_captures.clear();
    }

    pub fn take_pixel_write_captures(&mut self) -> Vec<ExecutionCapture> {
        std::mem::take(&mut self.pixel_write_captures)
    }

    pub fn target_ram_writes(&self) -> Vec<(u32, u16, u8, u8)> {
        self.target_ram_writes.clone()
    }

    pub fn timing_breakdown(&self) -> [u64; 5] {
        self.timing_breakdown
    }

    pub fn pc_trace(&self) -> Vec<u32> {
        self.pc_trace.clone()
    }

    pub fn register_trace(&self) -> Vec<String> {
        self.register_trace.clone()
    }

    pub fn point_states(&self) -> Vec<(u32, [u16; 16], u16, usize, usize, bool, bool, bool)> {
        self.point_states.clone()
    }

    fn commit_ram(&mut self, address: usize, value: u8) {
        let address = address & 0xFFFF;
        let old_value = self.ram[address];
        if matches!(
            address,
            0x003A | 0x003B | 0x03C6 | 0x03C7 | 0x24C2..=0x24C5 | 0x24DA | 0x4128 | 0x41C8
        ) && old_value != value
        {
            self.target_ram_writes.push((
                self.current_instruction,
                address as u16,
                old_value,
                value,
            ));
        }
        self.ram[address] = value;
    }

    /// Normal GSU RAM stores use a delayed single-byte write pipeline. A
    /// second load/store must wait for it, while intervening opcode fetches can
    /// retire the delay in parallel.
    fn write_ram(&mut self, address: usize, value: u8) {
        if self.timing_active {
            self.time_wait_ram();
            self.ram_delay = self.memory_latency();
        }
        // State changes remain instruction-atomic in this semantic core. The
        // timing pipeline above preserves when later GSU operations may use it.
        self.commit_ram(address, value);
    }

    fn read_ram_byte(&mut self, address: usize) -> u8 {
        self.time_wait_ram();
        self.time_require_ram_access();
        self.ram[address & 0xFFFF]
    }

    fn read_rom_buffer(&mut self) -> u8 {
        self.time_wait_rom();
        self.rom_byte(self.rombr, self.r[14])
    }

    pub(crate) fn set_screen_base(&mut self, value: u8) {
        self.scbr = value;
    }

    pub(crate) fn set_screen_mode(&mut self, value: u8) {
        self.scmr = value;
        if value & 0x08 != 0 {
            self.waiting_for_ram_access = false;
        }
        if value & 0x10 != 0 {
            self.waiting_for_rom_access = false;
        }
    }

    pub(crate) fn set_clock_select(&mut self, value: u8) {
        self.clock_select = value & 1 != 0;
    }

    pub(crate) fn set_config(&mut self, value: u8) {
        self.high_speed_mode = value & 0x20 != 0;
    }

    pub(crate) fn clock_select(&self) -> bool {
        self.clock_select
    }

    pub(crate) fn set_program_bank(&mut self, value: u8) {
        self.pbr = value & 0x7F;
        self.cache_valid.fill(false);
    }

    pub(crate) fn screen_state(&self) -> (u8, u8, u8, u8, u64, u64, u64, u8, u16) {
        (
            self.scbr,
            self.scmr,
            self.colr,
            self.por,
            self.plot_count,
            self.plot_write_count,
            self.last_run_steps,
            self.pbr,
            self.r[15],
        )
    }

    fn color(&mut self, source: u8) {
        self.colr = if self.por & 0x04 != 0 {
            (self.colr & 0xF0) | (source >> 4)
        } else if self.por & 0x08 != 0 {
            (self.colr & 0xF0) | (source & 0x0F)
        } else {
            source
        };
    }

    fn screen_mode(&self) -> (usize, usize) {
        let md = usize::from(self.scmr & 3);
        let bpp = 2 << (md - (md >> 1));
        let ht = usize::from((self.scmr >> 5) & 1) * 2 + usize::from((self.scmr >> 2) & 1);
        (bpp, ht)
    }

    fn pixel_address(&self, x: u8, y: u8) -> (usize, usize) {
        let (_, ht) = self.screen_mode();
        let layout = if self.por & 0x10 != 0 { 3 } else { ht };
        let x = usize::from(x);
        let y = usize::from(y);
        let character = match layout {
            0 => ((x & 0xF8) << 1) + ((y & 0xF8) >> 3),
            1 => ((x & 0xF8) << 1) + ((x & 0xF8) >> 1) + ((y & 0xF8) >> 3),
            2 => ((x & 0xF8) << 1) + (x & 0xF8) + ((y & 0xF8) >> 3),
            _ => ((y & 0x80) << 2) + ((x & 0x80) << 1) + ((y & 0x78) << 1) + ((x & 0x78) >> 3),
        };
        let (bpp, _) = self.screen_mode();
        let address = character * (bpp << 3) + usize::from(self.scbr) * 0x400 + (y & 7) * 2;
        (address & 0xFFFF, bpp)
    }

    fn is_transparent_pixel(&self) -> bool {
        let color = if self.por & 0x08 != 0 {
            self.colr & 0x0F
        } else {
            self.colr
        };
        match self.screen_mode().0 {
            2 => color & 0x03 == 0,
            4 => color & 0x0F == 0,
            _ => color == 0,
        }
    }

    fn write_pixel_cache(&mut self, cache: PixelCache) {
        if cache.valid_bits == 0 {
            return;
        }

        let (base, bpp) = self.pixel_address(cache.x, cache.y);
        for plane in 0..bpp {
            let mut value = 0u8;
            for x in 0..8 {
                value |= ((cache.pixels[x] >> plane) & 1) << x;
            }

            let offset = (plane >> 1) * 16 + (plane & 1);
            let address = (base + offset) & 0xFFFF;
            if cache.valid_bits != 0xFF {
                self.time_charge(4, u64::from(self.memory_latency()));
                value &= cache.valid_bits;
                value |= self.ram[address] & !cache.valid_bits;
            }
            self.time_charge(4, u64::from(self.memory_latency()));
            self.time_require_ram_access();
            self.commit_ram(address, value);
        }
    }

    fn flush_primary_pixel_cache(&mut self, x: u8, y: u8) {
        self.write_pixel_cache(self.secondary_pixel_cache);
        self.secondary_pixel_cache = self.primary_pixel_cache;
        self.primary_pixel_cache.valid_bits = 0;
        self.primary_pixel_cache.x = x & 0xF8;
        self.primary_pixel_cache.y = y;
    }

    fn plot(&mut self, x: u8, y: u8) {
        // POR bit 0 selects transparent plotting. The hardware skips zero in
        // the active color depth while the bit is clear.
        if self.por & 1 == 0 && self.is_transparent_pixel() {
            return;
        }
        let mut color = self.colr;
        if self.por & 0x02 != 0 && self.scmr & 3 != 3 {
            if (x ^ y) & 1 != 0 {
                color >>= 4;
            }
            color &= 0x0F;
        }
        if self.pixel_write_watch == Some((x, y)) {
            self.pixel_write_captures.push(ExecutionCapture {
                instruction: self.current_instruction,
                memory: self.ram.clone(),
                values: self.r,
                color,
            });
        }
        if self.primary_pixel_cache.x != x & 0xF8 || self.primary_pixel_cache.y != y {
            self.flush_primary_pixel_cache(x, y);
        }

        let x_offset = 7 - usize::from(x & 7);
        self.primary_pixel_cache.pixels[x_offset] = color;
        self.primary_pixel_cache.valid_bits |= 1 << x_offset;
        if self.primary_pixel_cache.valid_bits == 0xFF {
            self.flush_primary_pixel_cache(x, y);
        }
        self.plot_write_count = self.plot_write_count.wrapping_add(1);
    }

    fn read_pixel(&mut self, x: u8, y: u8) -> u8 {
        self.write_pixel_cache(self.secondary_pixel_cache);
        self.secondary_pixel_cache.valid_bits = 0;
        self.write_pixel_cache(self.primary_pixel_cache);
        self.primary_pixel_cache.valid_bits = 0;

        let (base, bpp) = self.pixel_address(x, y);
        let bit = 7 - usize::from(x & 7);
        let mut color = 0u8;
        for plane in 0..bpp {
            let offset = (plane >> 1) * 16 + (plane & 1);
            color |= ((self.ram[(base + offset) & 0xFFFF] >> bit) & 1) << plane;
            self.time_charge(4, u64::from(self.memory_latency()));
        }
        color
    }

    fn src(&self) -> u16 {
        self.r[self.sreg]
    }
    fn write_reg(&mut self, reg: usize, value: u16) {
        // Every write to R15 is pipelined on the GSU.  The instruction already
        // fetched after the writer executes as a delay slot, then execution
        // continues at the written address.  This is not limited to JMP and
        // branches: Star Fox 2 deliberately uses `IWT r15,target` followed by
        // a meaningful WITH/FROM delay-slot instruction for GSU calls.
        if reg == 15 {
            self.pending_branch = Some((self.pbr, value));
        } else {
            self.r[reg] = value;
            if reg == 14 && self.timing_active {
                self.rom_delay = self.memory_latency();
            }
        }
    }
    fn write_dst(&mut self, v: u16) {
        self.write_reg(self.dreg, v);
    }

    /// Reset the per-instruction prefix/ALT state (after a non-prefix op).
    fn reset_prefix(&mut self) {
        self.sreg = 0;
        self.dreg = 0;
        self.alt1 = false;
        self.alt2 = false;
        self.b_flag = false;
    }

    fn add_flags(&mut self, a: u16, b: u16, carry_in: u16) -> u16 {
        let sum = a as u32 + b as u32 + carry_in as u32;
        let res = sum as u16;
        self.set_flag(sfr::CY, sum > 0xFFFF);
        let ov = (!(a ^ b) & (a ^ res)) & 0x8000 != 0;
        self.set_flag(sfr::OV, ov);
        self.set_zs(res);
        res
    }
    fn sub_flags(&mut self, a: u16, b: u16, borrow_in: u16) -> u16 {
        let diff = (a as i32) - (b as i32) - (borrow_in as i32);
        let res = diff as u16;
        self.set_flag(sfr::CY, diff >= 0); // GSU carry = no borrow
        let ov = ((a ^ b) & (a ^ res)) & 0x8000 != 0;
        self.set_flag(sfr::OV, ov);
        self.set_zs(res);
        res
    }

    fn step(&mut self, redirect: Option<(u8, u16)>, reset_cache: bool) {
        let address = (u32::from(self.pbr) << 16) | u32::from(self.r[15]);
        self.current_instruction = address;
        if self.trace_this_run && self.pc_trace.len() < 1_000_000 {
            self.pc_trace.push(address);
            let mut fields = Vec::with_capacity(26);
            fields.push(format!("{address:06X}"));
            fields.extend(self.r.iter().map(|value| format!("{value:04X}")));
            fields.push(format!("{:04X}", self.sfr));
            fields.push(format!("{:02X}", self.sreg));
            fields.push(format!("{:02X}", self.dreg));
            fields.push(u8::from(self.alt1).to_string());
            fields.push(u8::from(self.alt2).to_string());
            fields.push(u8::from(self.b_flag).to_string());
            fields.push(format!("{:02X}", self.rombr));
            fields.push(format!("{:04X}", self.last_ram));
            self.register_trace.push(fields.join(" "));
        }
        if self.trace_this_run
            && matches!(
                address,
                0x01_D805
                    | 0x01_D807
                    | 0x01_D80B
                    | 0x01_D80E
                    | 0x01_D810
                    | 0x01_D812
                    | 0x01_D813
                    | 0x01_D818
            )
        {
            self.point_states.push((
                address,
                self.r,
                self.sfr,
                self.sreg,
                self.dreg,
                self.alt1,
                self.alt2,
                self.b_flag,
            ));
        }
        if self.execution_watch == Some(address) {
            self.execution_watch_visits = self.execution_watch_visits.wrapping_add(1);
            let matched = self
                .execution_watch_ram
                .is_none_or(|(ram_address, expected, mask)| {
                    let base = usize::from(ram_address);
                    let actual = u32::from_le_bytes([
                        self.ram[base],
                        self.ram[(base + 1) & 0xFFFF],
                        self.ram[(base + 2) & 0xFFFF],
                        self.ram[(base + 3) & 0xFFFF],
                    ]);
                    self.execution_watch_last_ram = actual;
                    if !self.execution_watch_values.contains(&actual) {
                        self.execution_watch_values.push(actual);
                    }
                    actual & mask == expected & mask
                });
            self.execution_watch_hit |= matched;
        }
        if let Some((watched_address, ram_start, ram_len)) = self.execution_capture_watch {
            if watched_address == address {
                self.execution_captures.push(ExecutionCapture {
                    instruction: address,
                    memory: self.ram[ram_start..ram_start + ram_len].to_vec(),
                    values: self.r,
                    color: self.colr,
                });
            }
        }
        if let Some((lo, hi)) = self.trace_range {
            let pc = self.r[15];
            if pc >= lo && pc < hi {
                eprintln!(
                    "  pc={:04X} op={:02X} r0={:04X} r1={:04X} r4={:04X} r6={:04X} r9={:04X} r12={:X} r13={:04X} CY={} Z={} sreg={} dreg={} b={}",
                    pc,
                    if self.pbr >= 0x60 {
                        self.ram[usize::from(pc)]
                    } else {
                        self.rom_byte(self.pbr, pc)
                    },
                    self.r[0], self.r[1], self.r[4], self.r[6], self.r[9], self.r[12], self.r[13],
                    self.flag(sfr::CY) as u8, self.flag(sfr::Z) as u8,
                    self.sreg, self.dreg, self.b_flag as u8,
                );
            }
        }
        let op = if let Some(prefetched) = self.prefetched_instruction.take() {
            // The denied fetch already filled the one-byte program buffer and
            // advanced the semantic PC. Retire that byte without fetching or
            // charging it a second time once SCMR restores access.
            self.r[15] = self.r[15].wrapping_add(1);
            prefetched
        } else {
            let fetched = self.fetch();
            if self.timing_active && (self.waiting_for_ram_access || self.waiting_for_rom_access) {
                // A denied program-bus fetch fills the one-byte read buffer but
                // cannot retire the fetched opcode. Keep the semantic PC at
                // that opcode until SCMR restores access. This is observable
                // in SF1's bitmap clear: an IRQ revokes ROM access just before
                // the final cache-line fill, so STOP remains pending until the
                // next IRQ.
                self.r[15] = address as u16;
                self.prefetched_instruction = Some(fetched);
                self.pending_branch = redirect;
                self.pending_cache_reset = reset_cache;
                return;
            }
            fetched
        };
        self.instruction_executed = true;
        if let Some((pbr, pc)) = redirect {
            if reset_cache || self.pbr != pbr {
                self.pbr = pbr;
                self.cache_base = pc & 0xFFF0;
                self.cache_valid.fill(false);
            }
            self.r[15] = pc;
        }
        match op {
            0x00 => {
                // STOP
                self.set_flag(sfr::G, false);
                self.reset_prefix();
                return;
            }
            0x01 => { /* NOP */ }
            0x02 => {
                let base = self.r[15] & 0xFFF0;
                if self.cache_base != base {
                    self.cache_base = base;
                    self.cache_valid.fill(false);
                }
            }
            0x03 => {
                // LSR
                let s = self.src();
                self.set_flag(sfr::CY, s & 1 != 0);
                let v = s >> 1;
                self.write_dst(v);
                self.set_zs(v);
            }
            0x04 => {
                // ROL
                let s = self.src();
                let cin = self.flag(sfr::CY) as u16;
                self.set_flag(sfr::CY, s & 0x8000 != 0);
                let v = (s << 1) | cin;
                self.write_dst(v);
                self.set_zs(v);
            }
            0x05..=0x0F => {
                // Branches: fetch signed offset, apply if condition met.
                let rel = self.fetch() as i8 as i32;
                let take = match op {
                    0x05 => true, // BRA
                    // SuperFX branch encoding (verified against the retail-built
                    // ROM's `blt`/`bge` at $01:81ED, `marctan16`): $06 = BGE
                    // (S==OV), $07 = BLT (S!=OV). Previously these were swapped,
                    // which made `marctan16` skip the operand swap on off-axis
                    // inputs and divide by zero (see gsu_arctan.rs / AUDIT).
                    0x06 => self.flag(sfr::S) == self.flag(sfr::OV), // BGE
                    0x07 => self.flag(sfr::S) != self.flag(sfr::OV), // BLT
                    0x08 => !self.flag(sfr::Z),                      // BNE
                    0x09 => self.flag(sfr::Z),                       // BEQ
                    0x0A => !self.flag(sfr::S),                      // BPL
                    0x0B => self.flag(sfr::S),                       // BMI
                    0x0C => !self.flag(sfr::CY),                     // BCC
                    0x0D => self.flag(sfr::CY),                      // BCS
                    0x0E => !self.flag(sfr::OV),                     // BVC
                    0x0F => self.flag(sfr::OV),                      // BVS
                    _ => unreachable!(),
                };
                if take {
                    let target = (self.r[15] as i32 + rel) as u16;
                    self.pending_branch = Some((self.pbr, target));
                }
                // A branch does not consume WITH/FROM/TO.  Its sequentially
                // fetched delay-slot instruction consumes that prefix before
                // a taken target is applied.  SF2's renderer uses this as
                // `WITH r14; BRA target; ADD r2` to advance its ROM cursor.
                return;
            }
            0x10..=0x1F => {
                if self.b_flag {
                    // MOVE Rn <- Sreg (TO with the WITH/B flag set).
                    let v = self.r[self.sreg];
                    self.write_reg((op & 0xF) as usize, v);
                } else {
                    self.dreg = (op & 0xF) as usize;
                    return; // TO prefix: keep alt, don't reset
                }
            }
            0x20..=0x2F => {
                self.sreg = (op & 0xF) as usize;
                self.dreg = (op & 0xF) as usize;
                self.b_flag = true;
                return;
            }
            0x30..=0x3B => {
                // STW/STB (Rn): store Sreg to GSU RAM at Rn; ALT1 => byte.
                // GSU words pair address A with A^1, not unconditionally A+1.
                let a = self.r[(op & 0xF) as usize] as usize & 0xFFFF;
                let s = self.src();
                self.last_ram = a as u16;
                self.write_ram(a, s as u8);
                if !self.alt1 {
                    self.write_ram(a ^ 1, (s >> 8) as u8);
                }
            }
            0x3C => {
                // LOOP: DEC r12; if !=0 branch to r13
                let v = self.r[12].wrapping_sub(1);
                self.r[12] = v;
                self.set_zs(v);
                if v != 0 {
                    self.pending_branch = Some((self.pbr, self.r[13]));
                }
            }
            0x3D => {
                self.b_flag = false;
                self.alt1 = true;
                return;
            }
            0x3E => {
                self.b_flag = false;
                self.alt2 = true;
                return;
            }
            0x3F => {
                self.b_flag = false;
                self.alt1 = true;
                self.alt2 = true;
                return;
            }
            0x40..=0x4B => {
                // LDW/LDB (Rn) — RAM load; ALT1 => byte. LOAD does not alter
                // arithmetic flags on the hardware.
                let a = self.r[(op & 0xF) as usize] as usize;
                self.last_ram = a as u16;
                let v = if self.alt1 {
                    self.read_ram_byte(a) as u16
                } else {
                    self.read_ram_byte(a) as u16
                        | ((self.read_ram_byte((a & 0xFFFF) ^ 1) as u16) << 8)
                };
                self.write_dst(v);
            }
            0x4C => {
                if self.alt1 {
                    let value = u16::from(self.read_pixel(self.r[1] as u8, self.r[2] as u8));
                    self.write_dst(value);
                    self.set_zs(value);
                } else {
                    self.plot(self.r[1] as u8, self.r[2] as u8);
                    self.plot_count = self.plot_count.wrapping_add(1);
                    self.r[1] = self.r[1].wrapping_add(1);
                }
            }
            0x4D => {
                // SWAP bytes
                let s = self.src();
                let v = s.rotate_left(8);
                self.write_dst(v);
                self.set_zs(v);
            }
            0x4E => {
                if self.alt1 {
                    self.por = self.src() as u8;
                } else {
                    self.color(self.src() as u8);
                }
            }
            0x4F => {
                // NOT
                let v = !self.src();
                self.write_dst(v);
                self.set_zs(v);
            }
            0x50..=0x5F => {
                // ADD/ADC/ADD#/ADC#
                let n = (op & 0xF) as usize;
                let operand = if self.alt2 { n as u16 } else { self.r[n] };
                let cin = if self.alt1 {
                    self.flag(sfr::CY) as u16
                } else {
                    0
                };
                let v = self.add_flags(self.src(), operand, cin);
                self.write_dst(v);
            }
            0x60..=0x6F => {
                // SUB/SBC/SUB#/CMP
                let n = (op & 0xF) as usize;
                let is_cmp = self.alt1 && self.alt2; // ALT3 => CMP
                let operand = if self.alt2 && !self.alt1 {
                    n as u16
                } else {
                    self.r[n]
                };
                let bin = if self.alt1 && !self.alt2 {
                    (!self.flag(sfr::CY)) as u16
                } else {
                    0
                };
                let v = self.sub_flags(self.src(), operand, bin);
                if !is_cmp {
                    self.write_dst(v);
                }
            }
            0x70 => {
                // MERGE
                let v = (self.r[7] & 0xFF00) | (self.r[8] >> 8);
                self.write_dst(v);
                self.set_flag(sfr::CY, v & 0xE0E0 != 0);
                self.set_flag(sfr::OV, v & 0xC0C0 != 0);
                self.set_flag(sfr::S, v & 0x8080 != 0);
                self.set_flag(sfr::Z, v & 0xF0F0 != 0);
            }
            0x71..=0x7F => {
                // AND/BIC/AND#/BIC#
                let n = (op & 0xF) as usize;
                let operand = if self.alt2 { n as u16 } else { self.r[n] };
                let operand = if self.alt1 { !operand } else { operand }; // BIC
                let v = self.src() & operand;
                self.write_dst(v);
                self.set_zs(v);
            }
            0x80..=0x8F => {
                // MULT/UMULT/MULT#/UMULT#  (low 16 of product)
                let n = (op & 0xF) as usize;
                let operand = if self.alt2 { n as u16 } else { self.r[n] };
                let v = if self.alt1 {
                    (self.src() as u8 as u16).wrapping_mul(operand as u8 as u16)
                // UMULT
                } else {
                    ((self.src() as u8 as i8 as i16) * (operand as u8 as i8 as i16)) as u16
                    // MULT
                };
                self.write_dst(v);
                self.set_zs(v);
                self.time_charge(3, if self.high_speed_mode { 1 } else { 2 });
            }
            0x90 => {
                // SBK: store Sreg to the last-accessed RAM word address.
                let a = self.last_ram as usize & 0xFFFF;
                let s = self.src();
                self.write_ram(a, s as u8);
                self.write_ram(a ^ 1, (s >> 8) as u8);
            }
            0x91..=0x94 => {
                // LINK #n: R11 = R15 + n (return address for a GSU subroutine).
                self.r[11] = self.r[15].wrapping_add((op & 0xF) as u16);
            }
            0x95 => {
                // SEX: sign-extend low byte
                let v = self.src() as u8 as i8 as i16 as u16;
                self.write_dst(v);
                self.set_zs(v);
            }
            0x96 => {
                // ASR / DIV2 (ALT1)
                let s = self.src();
                self.set_flag(sfr::CY, s & 1 != 0);
                let v = ((s as i16) >> 1) as u16;
                let v = if self.alt1 && s == 0xFFFF { 0 } else { v }; // DIV2 rounds -1 -> 0
                self.write_dst(v);
                self.set_zs(v);
            }
            0x97 => {
                // ROR
                let s = self.src();
                let cin = (self.flag(sfr::CY) as u16) << 15;
                self.set_flag(sfr::CY, s & 1 != 0);
                let v = (s >> 1) | cin;
                self.write_dst(v);
                self.set_zs(v);
            }
            0x98..=0x9D => {
                // JMP/LJMP Rn (delayed). LJMP (ALT1) also sets the program bank
                // from the opcode register, while the address comes from Sreg.
                let n = (op & 0xF) as usize;
                let (pbr, pc) = if self.alt1 {
                    (self.r[n] as u8 & 0x7F, self.src())
                } else {
                    (self.pbr, self.r[n])
                };
                self.pending_branch = Some((pbr, pc));
                self.pending_cache_reset = self.alt1;
            }
            0x9E => {
                // LOB: low byte, zero high
                let v = self.src() & 0xFF;
                self.write_dst(v);
                self.set_flag(sfr::Z, v == 0);
                self.set_flag(sfr::S, v & 0x80 != 0);
            }
            0x9F => {
                // FMULT / LMULT: signed Sreg * R6; result = high 16 of the
                // 32-bit product. LMULT (ALT1) also writes the low 16 to R4.
                let p = (self.src() as i16 as i32) * (self.r[6] as i16 as i32);
                let hi = (p >> 16) as u16;
                if self.alt1 {
                    self.r[4] = p as u16;
                }
                self.write_dst(hi);
                self.set_zs(hi);
                self.set_flag(sfr::CY, (p >> 15) & 1 != 0);
                let multiply_clocks = if self.high_speed_mode { 3 } else { 7 };
                let clock_scale = if self.clock_select { 1 } else { 2 };
                self.time_charge(3, multiply_clocks * clock_scale);
            }
            0xA0..=0xAF => {
                // IBT Rn,#pp (ALT0) / LMS Rn,(imm) (ALT1) / SMS (imm),Rn (ALT2).
                // LMS/SMS use a short word-index (imm*2) into GSU RAM.
                let n = (op & 0xF) as usize;
                let imm = self.fetch();
                if self.alt1 {
                    let a = (imm as usize) << 1;
                    self.last_ram = a as u16;
                    let value =
                        self.read_ram_byte(a) as u16 | ((self.read_ram_byte(a | 1) as u16) << 8);
                    self.write_reg(n, value);
                } else if self.alt2 {
                    let a = (imm as usize) << 1;
                    self.last_ram = a as u16;
                    self.write_ram(a, self.r[n] as u8);
                    self.write_ram(a | 1, (self.r[n] >> 8) as u8);
                } else {
                    self.write_reg(n, imm as i8 as i16 as u16); // IBT sign-extends
                }
                self.reset_prefix();
                return;
            }
            0xB0..=0xBF => {
                if self.b_flag {
                    // MOVES Dreg <- Rn (FROM with the WITH/B flag): the operand
                    // register is the SOURCE, Dreg (set by WITH) the dest; sets
                    // flags. Used for the abs idiom `WITH r6; FROM r1` -> r6=r1.
                    let v = self.r[(op & 0xF) as usize];
                    self.write_reg(self.dreg, v);
                    self.set_zs(v);
                    self.set_flag(sfr::OV, v & 0x80 != 0);
                } else {
                    self.sreg = (op & 0xF) as usize;
                    return; // FROM prefix
                }
            }
            0xC0 => {
                // HIB: high byte -> low
                let v = self.src() >> 8;
                self.write_dst(v);
                self.set_flag(sfr::Z, v == 0);
                self.set_flag(sfr::S, v & 0x80 != 0);
            }
            0xC1..=0xCF => {
                // OR/XOR/OR#/XOR#
                let n = (op & 0xF) as usize;
                let operand = if self.alt2 { n as u16 } else { self.r[n] };
                let v = if self.alt1 {
                    self.src() ^ operand
                } else {
                    self.src() | operand
                };
                self.write_dst(v);
                self.set_zs(v);
            }
            0xD0..=0xDE => {
                // INC Rn
                let n = (op & 0xF) as usize;
                let v = self.r[n].wrapping_add(1);
                self.write_reg(n, v);
                self.set_zs(v);
            }
            0xDF => {
                // GETC (alt0) / RAMB (ALT2) / ROMB (ALT3 = alt1&alt2).
                if self.alt1 && self.alt2 {
                    self.time_wait_rom();
                    self.rombr = self.src() as u8; // ROMB: set ROM data bank
                } else if self.alt2 {
                    self.time_wait_ram();
                    // RAMB: set RAM bank (single 64K RAM here — no-op)
                } else {
                    let value = self.read_rom_buffer();
                    self.color(value);
                }
            }
            0xE0..=0xEE => {
                // DEC Rn
                let n = (op & 0xF) as usize;
                let v = self.r[n].wrapping_sub(1);
                self.write_reg(n, v);
                self.set_zs(v);
            }
            0xEF => {
                // GETB variants: read ROM byte at rombr:R14
                let b = self.read_rom_buffer();
                let cur = self.r[self.dreg];
                let v = match (self.alt1, self.alt2) {
                    (false, false) => b as u16,                          // GETB
                    (true, false) => (cur & 0x00FF) | ((b as u16) << 8), // GETBH
                    (false, true) => (cur & 0xFF00) | b as u16,          // GETBL
                    (true, true) => b as i8 as i16 as u16,               // GETBS
                };
                self.write_dst(v);
            }
            0xF0..=0xFF => {
                // IWT Rn,#word (ALT0) / LM Rn,(addr) (ALT1) / SM (addr),Rn (ALT2).
                let n = (op & 0xF) as usize;
                let lo = self.fetch() as u16;
                let hi = self.fetch() as u16;
                let imm = lo | (hi << 8);
                if self.alt1 {
                    let a = imm as usize & 0xFFFF;
                    self.last_ram = a as u16;
                    let value =
                        self.read_ram_byte(a) as u16 | ((self.read_ram_byte(a ^ 1) as u16) << 8);
                    self.write_reg(n, value);
                } else if self.alt2 {
                    let a = imm as usize & 0xFFFF;
                    self.last_ram = a as u16;
                    self.write_ram(a, self.r[n] as u8);
                    self.write_ram(a ^ 1, (self.r[n] >> 8) as u8);
                } else {
                    self.write_reg(n, imm); // IWT (R15 writes are delayed)
                }
                self.reset_prefix();
                return;
            }
        }
        self.reset_prefix();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-assembled GSU program exercising the core: immediates, register
    /// prefixes, ALU, and STOP. Computes (5 + 3) * 2 = 16 into R3.
    ///   IBT r1,#5 ; IBT r2,#3 ; FROM r1 ; TO r3 ; ADD r2 ; (r3=8)
    ///   FROM r3 ; TO r3 ; ADD r3 ; (r3=16) ; STOP
    #[test]
    fn gsu_core_arithmetic() {
        let mut prog = vec![0u8; 0x8000];
        let code = [
            0xA1, 0x05, // IBT r1,#5
            0xA2, 0x03, // IBT r2,#3
            0xB1, //       FROM r1
            0x13, //       TO r3
            0x52, //       ADD r2      -> r3 = 5+3 = 8
            0xB3, //       FROM r3
            0x13, //       TO r3
            0x53, //       ADD r3      -> r3 = 8+8 = 16
            0x00, //       STOP
        ];
        prog[..code.len()].copy_from_slice(&code);
        let mut gsu = Gsu::new(prog);
        gsu.run(0, 0);
        eprintln!("GSU self-test: r3 = {}", gsu.r[3]);
        assert_eq!(gsu.r[3], 16, "(5+3)*2 should be 16");
        assert_eq!(gsu.r[1], 5);
        assert_eq!(gsu.r[2], 3);
    }

    /// Exercises the MOVE idiom, RAM store/load round-trip, and FMULT.
    #[test]
    fn gsu_memory_mult_move() {
        let mut prog = vec![0u8; 0x8000];
        let code = [
            0xF1, 0xCD, 0xAB, // IWT r1,#$ABCD
            0x21, //             WITH r1
            0x12, //             TO r2   -> MOVE r2 = r1
            0xF3, 0x34, 0x12, // IWT r3,#$1234
            0x3E, 0xF3, 0x10, 0x00, // ALT2 ; SM ($0010),r3
            0x3D, 0xF4, 0x10, 0x00, // ALT1 ; LM r4,($0010)
            0xF6, 0x64, 0x00, // IWT r6,#100
            0xF5, 0x00, 0x40, // IWT r5,#$4000 (16384)
            0xB5, //             FROM r5
            0x9F, //             FMULT -> r0 = high16(16384*100) = 25
            0x00, //             STOP
        ];
        prog[..code.len()].copy_from_slice(&code);
        let mut gsu = Gsu::new(prog);
        gsu.run(0, 0);
        eprintln!(
            "GSU mem/mult/move: r2={:#06x} r4={:#06x} r0={}",
            gsu.r[2], gsu.r[4], gsu.r[0]
        );
        assert_eq!(gsu.r[2], 0xABCD, "MOVE r2<-r1");
        assert_eq!(gsu.r[4], 0x1234, "SM/LM RAM round-trip");
        assert_eq!(gsu.r[0], 25, "FMULT high16(16384*100)");
    }

    /// R15 writes use the already-fetched next instruction as a delay slot.
    /// SF2's decompressor relies on this exact sequence for every helper call:
    /// `IWT r15,target ; WITH rN`, with the callee consuming the WITH prefix.
    #[test]
    fn iwt_r15_executes_meaningful_delay_slot() {
        let mut prog = vec![0u8; 0x8000];
        let setup = [
            0xF1, 0xCD, 0xAB, // IWT r1,#$ABCD
            0xFF, 0x10, 0x00, // IWT r15,#$0010
            0x24, //             WITH r4 (delay slot)
        ];
        prog[..setup.len()].copy_from_slice(&setup);
        prog[0x10..0x12].copy_from_slice(&[
            0xB1, // FROM r1 -> MOVES r4,r1 because delay slot set WITH r4
            0x00, // STOP
        ]);

        let mut gsu = Gsu::new(prog);
        gsu.run(0, 0);
        assert_eq!(gsu.r[4], 0xABCD);
    }

    /// Branch instructions preserve a pending register prefix for the delay
    /// slot.  This sequence is taken directly from SF2's renderer at $01:9F2E.
    #[test]
    fn branch_preserves_prefix_for_meaningful_delay_slot() {
        let mut prog = vec![0u8; 0x8000];
        let code = [
            0xF2, 0x08, 0x00, // IWT r2,#8
            0xFE, 0x64, 0x00, // IWT r14,#100
            0x2E, //             WITH r14
            0x05, 0x02, //       BRA $000B
            0x52, //             ADD r2 (delay slot): r14 += r2
            0x00, //             skipped STOP
            0x00, // target:     STOP
        ];
        prog[..code.len()].copy_from_slice(&code);

        let mut gsu = Gsu::new(prog);
        gsu.run(0, 0);
        assert_eq!(gsu.r[14], 108);
        assert_eq!(gsu.r[0], 0, "delay slot must not use the default register");
    }

    /// A taken branch redirects R15 before its already-prefetched delay opcode
    /// reads operands. This is the exact pattern at SF2 `$01:DAC2`: the
    /// `$DAC4 IBT r12` delay instruction consumes target byte `$DAE4` (also
    /// opcode CACHE) as its immediate, so CACHE itself is never executed.
    #[test]
    fn branch_delay_operand_comes_from_target_stream() {
        let mut prog = vec![0u8; 0x8000];
        let code = [
            0x05, 0x03, // BRA $0005
            0xAC, 0x99, // IBT r12 (delay); sequential $99 is not consumed
            0x00, // skipped STOP
            0x02, // target byte becomes the IBT immediate, not CACHE
            0x00, // STOP
        ];
        prog[..code.len()].copy_from_slice(&code);

        let mut gsu = Gsu::new(prog);
        gsu.run(0, 0);
        assert_eq!(gsu.r[12], 2);
        assert_eq!(gsu.cache_base, 0, "target CACHE byte must not execute");
        assert_eq!(gsu.last_run_steps, 3, "BRA, delay IBT, STOP");
    }

    fn fnv1a(bytes: &[u8]) -> u32 {
        bytes.iter().fold(0x811C_9DC5, |hash, byte| {
            (hash ^ *byte as u32).wrapping_mul(0x0100_0193)
        })
    }

    /// Full retail SF2 decompression, externally verified byte-for-byte against
    /// Mesen 2.1.1's independent Super FX core. The ROM is user-owned and
    /// gitignored, so a checkout without it skips this oracle test.
    #[test]
    fn sf2_retail_decompressor_matches_mesen_oracle() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let Ok(rom) = std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc")) else {
            eprintln!("SKIP: Star Fox 2 retail ROM not present");
            return;
        };

        let mut gsu = Gsu::new(rom);
        gsu.ram[0x002C..0x002E].copy_from_slice(&0x3B50u16.to_le_bytes());
        gsu.ram[0x0068..0x006A].copy_from_slice(&0x9F9Cu16.to_le_bytes());
        gsu.ram[0x006A..0x006C].copy_from_slice(&0x0019u16.to_le_bytes());
        gsu.ram[0x00A2..0x00A4].copy_from_slice(&0u16.to_le_bytes());
        gsu.run_with_limit(0x01, 0xD9FF, 1_000_000);

        assert_eq!(gsu.last_run_steps, 504_523, "decompressor must reach STOP");
        assert_eq!(gsu.r[1], 0x3B50, "output cursor must reach its base");
        assert_eq!(gsu.r[14], 0x9238, "compressed source cursor");
        assert_eq!(
            fnv1a(&gsu.ram[0x3B50..0x5B70]),
            0xC78A_FF13,
            "8,224 decompressed bytes must match the independent Mesen oracle"
        );
    }
}
