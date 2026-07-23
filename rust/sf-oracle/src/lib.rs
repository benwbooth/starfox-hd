//! ROM oracle: execute real Star Fox (SNES) 65816 subroutines from the retail
//! ROM and diff them against the Rust port. Uses the `w65c816` core (validated
//! against the SingleStepTests/65816 suite) with a LoROM bus over the retail
//! ROM + 128 KB WRAM.
//!
//! The retail ROM is `Star Fox (USA) (Rev 2).sfc` at the repo root (gitignored).
//! It is headerless LoROM. Game logic (movement, init, collision, strats) is
//! pure 65816 and directly executable here; 3D/render math lives on the GSU and
//! is out of scope for this harness.

pub mod gsu;
mod ppu;
mod retail;
pub use ppu::PpuFrame;
pub use retail::*;

use w65c816::{AddressType, Signals, System, CPU};

/// One completed CPU-triggered Super FX job, retained by the retail-machine
/// diagnostics.  The RAM probe contains `$003A`, `$24C2`, `$24C4`, and `$0014`
/// at job entry; these locations include the full-screen renderer dimensions
/// whose corruption turns the `$01:CD99` finite loops into wraparound loops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GsuRunEvent {
    pub sequence: u64,
    pub pbr: u8,
    pub pc: u16,
    pub ram_probe: [u16; 4],
    pub exit_ram_probe: [u16; 4],
    pub entry_regs: [u16; 16],
    pub entry_tick: u64,
    pub exit_tick: u64,
    pub steps: u64,
    pub hit_limit: bool,
    pub exit_pbr: u8,
    pub exit_pc: u16,
    pub target_ram_writes: Vec<(u32, u16, u8, u8)>,
    /// GSU clocks spent on program fetch, RAM waits, ROM waits, multiply, and
    /// pixel-cache bus traffic, respectively.
    pub timing_breakdown: [u64; 5],
}

#[derive(Debug, Clone)]
struct ActiveGsuRun {
    sequence: u64,
    pbr: u8,
    pc: u16,
    ram_probe: [u16; 4],
    entry_regs: [u16; 16],
    entry_tick: u64,
}

/// LoROM + WRAM bus over the retail ROM.
pub struct SnesBus {
    rom: Vec<u8>,
    /// 128 KB: banks $7E and $7F.
    wram: Vec<u8>,
    /// 64 KB GSU cart RAM, mirrored in banks $70/$71. SF1 uses the low half;
    /// SF2's GSU-2 cartridge uses the full bank and installs generated code
    /// from `$70:8000-$FFFF` into WRAM.
    gsuram: Vec<u8>,
    /// Reset line: pulsed true once to boot the CPU out of its power-on STP.
    res_line: bool,
    /// PPU/CPU math registers (used by e.g. n3dvecs' `mulslog`).
    mpy_a: u8, // $4202 WRMPYA
    rdmpy: u16,    // $4216/7 RDMPY (product, or divide remainder)
    dividend: u16, // $4204/5 WRDIV
    quotient: u16, // $4214/5 RDDIV
    /// Optional GSU (Super-FX) co-processor. When present, the memory-mapped
    /// GSU registers ($3000-$303F, banks $00-$3F/$80-$BF) are live: writing the
    /// high byte of R15 ($301F) *kicks* the chip — the CPU's `runmario_l`
    /// (`sta m_pbr; stx mr15; .wait lda m_sfr; and #$20; bne .wait`) drives 3D /
    /// spawn math exactly as on hardware. See [`SnesBus::enable_gsu`].
    gsu: Option<Box<gsu::Gsu>>,
    /// Shadow of the memory-mapped GSU register file (R0..R15). Kept in sync
    /// with `gsu.r`; separated so register reads/writes need no `&mut gsu`.
    gsu_regs: [u16; 16],
    gsu_pbr: u8,  // $3034 program bank
    gsu_sfr: u16, // $3030/1 status/flag register presented to the CPU
    /// Number of times the CPU kicked the GSU (diagnostic — proves the per-tick
    /// path actually invoked the chip).
    pub gsu_kicks: u64,
    gsu_last_entry: (u8, u16),
    gsu_last_entry_ram_probe: [u16; 4],
    gsu_step_limit_hits: u64,
    gsu_recent_runs: Vec<GsuRunEvent>,
    gsu_active_run: Option<ActiveGsuRun>,
    gsu_ticks: u64,
    gsu_first_cd99_entry_ram: Option<Vec<u8>>,
    gsu_first_cd99_exit_ram: Option<Vec<u8>>,
    gsu_first_cd99_pc_trace: Option<Vec<u32>>,
    gsu_first_cd99_register_trace: Option<Vec<String>>,
    gsu_first_cd99_point_states: Option<Vec<(u32, [u16; 16], u16, usize, usize, bool, bool, bool)>>,
    gsu_first_ce37_entry_ram: Option<Vec<u8>>,
    gsu_first_ce37_exit_ram: Option<Vec<u8>>,
    gsu_first_ce37_pc_trace: Option<Vec<u32>>,
    gsu_first_ce37_register_trace: Option<Vec<String>>,
    gsu_first_d9ff_register_trace: Option<Vec<String>>,
}

impl SnesBus {
    pub fn new(rom: Vec<u8>) -> Self {
        SnesBus {
            rom,
            wram: vec![0u8; 0x2_0000],
            gsuram: vec![0u8; 0x1_0000],
            res_line: true,
            mpy_a: 0,
            rdmpy: 0,
            dividend: 0,
            quotient: 0,
            gsu: None,
            gsu_regs: [0; 16],
            gsu_pbr: 0,
            gsu_sfr: 0,
            gsu_kicks: 0,
            gsu_last_entry: (0, 0),
            gsu_last_entry_ram_probe: [0; 4],
            gsu_step_limit_hits: 0,
            gsu_recent_runs: Vec::new(),
            gsu_active_run: None,
            gsu_ticks: 0,
            gsu_first_cd99_entry_ram: None,
            gsu_first_cd99_exit_ram: None,
            gsu_first_cd99_pc_trace: None,
            gsu_first_cd99_register_trace: None,
            gsu_first_cd99_point_states: None,
            gsu_first_ce37_entry_ram: None,
            gsu_first_ce37_exit_ram: None,
            gsu_first_ce37_pc_trace: None,
            gsu_first_ce37_register_trace: None,
            gsu_first_d9ff_register_trace: None,
        }
    }

    /// Attach a GSU that shares this bus's cartridge ROM. After this, CPU stores
    /// to the memory-mapped GSU registers run real GSU programs (RAM shared via
    /// bank $70). Idempotent-safe: replaces any prior GSU.
    pub fn enable_gsu(&mut self) {
        let mut gsu = gsu::Gsu::new(self.rom.clone());
        gsu.ram.copy_from_slice(&self.gsuram);
        self.gsu = Some(Box::new(gsu));
    }

    pub fn gsu_plot_count(&self) -> u64 {
        self.gsu.as_ref().map_or(0, |gsu| gsu.plot_count)
    }

    pub fn gsu_screen_state(&self) -> Option<(u8, u8, u8, u8, u64, u64, u64, u8, u16)> {
        self.gsu.as_ref().map(|gsu| gsu.screen_state())
    }

    pub fn watch_gsu_execution(&mut self, pbr: u8, pc: u16) {
        if let Some(gsu) = self.gsu.as_mut() {
            gsu.watch_execution(pbr, pc);
        }
    }

    pub fn watch_gsu_execution_with_ram_mask(
        &mut self,
        pbr: u8,
        pc: u16,
        ram_address: u16,
        value: u32,
        mask: u32,
    ) {
        if let Some(gsu) = self.gsu.as_mut() {
            gsu.watch_execution_with_ram_mask(pbr, pc, ram_address, value, mask);
        }
    }

    pub fn gsu_execution_watch_hit(&self) -> bool {
        self.gsu
            .as_ref()
            .is_some_and(|gsu| gsu.execution_watch_hit())
    }

    pub fn gsu_execution_watch_state(&self) -> Option<(u64, u32, bool)> {
        self.gsu.as_ref().map(|gsu| gsu.execution_watch_state())
    }

    pub fn gsu_execution_watch_values(&self) -> Vec<u32> {
        self.gsu
            .as_ref()
            .map_or_else(Vec::new, |gsu| gsu.execution_watch_values())
    }

    pub fn gsu_run_debug_state(&self) -> Option<((u8, u16), (u8, u16, u16), u64, bool, u64)> {
        self.gsu.as_ref().map(|gsu| {
            (
                self.gsu_last_entry,
                gsu.execution_state(),
                gsu.last_run_steps,
                gsu.last_run_hit_limit,
                self.gsu_step_limit_hits,
            )
        })
    }

    pub fn gsu_last_run_samples(&self) -> Vec<(u64, u8, u16, u16, u16, u16, u16, u16, u16)> {
        self.gsu
            .as_ref()
            .map_or_else(Vec::new, |gsu| gsu.last_run_samples())
    }

    pub fn gsu_last_entry_ram_probe(&self) -> [u16; 4] {
        self.gsu_last_entry_ram_probe
    }

    pub fn gsu_recent_runs(&self) -> Vec<GsuRunEvent> {
        self.gsu_recent_runs.clone()
    }

    pub fn gsu_first_cd99_entry_ram(&self) -> Option<Vec<u8>> {
        self.gsu_first_cd99_entry_ram.clone()
    }

    pub fn gsu_first_cd99_exit_ram(&self) -> Option<Vec<u8>> {
        self.gsu_first_cd99_exit_ram.clone()
    }

    pub fn gsu_first_cd99_pc_trace(&self) -> Option<Vec<u32>> {
        self.gsu_first_cd99_pc_trace.clone()
    }

    pub fn gsu_first_cd99_register_trace(&self) -> Option<Vec<String>> {
        self.gsu_first_cd99_register_trace.clone()
    }

    pub fn gsu_first_cd99_point_states(
        &self,
    ) -> Option<Vec<(u32, [u16; 16], u16, usize, usize, bool, bool, bool)>> {
        self.gsu_first_cd99_point_states.clone()
    }

    pub fn gsu_first_ce37_entry_ram(&self) -> Option<Vec<u8>> {
        self.gsu_first_ce37_entry_ram.clone()
    }

    pub fn gsu_first_ce37_exit_ram(&self) -> Option<Vec<u8>> {
        self.gsu_first_ce37_exit_ram.clone()
    }

    pub fn gsu_first_ce37_pc_trace(&self) -> Option<Vec<u32>> {
        self.gsu_first_ce37_pc_trace.clone()
    }

    pub fn gsu_first_ce37_register_trace(&self) -> Option<Vec<String>> {
        self.gsu_first_ce37_register_trace.clone()
    }

    pub fn gsu_first_d9ff_register_trace(&self) -> Option<Vec<String>> {
        self.gsu_first_d9ff_register_trace.clone()
    }

    /// True for the memory-mapped GSU register block ($3000-$303F) in the
    /// CPU-visible register banks.
    fn is_gsu_reg(addr: u32) -> bool {
        let bank = (addr >> 16) & 0xFF;
        let off = addr & 0xFFFF;
        (bank <= 0x3F || (0x80..=0xBF).contains(&bank)) && (0x3000..=0x303F).contains(&off)
    }

    /// Read a memory-mapped GSU register (only meaningful while `gsu.is_some()`).
    fn gsu_reg_read(&self, off: u16) -> u8 {
        match off {
            0x3000..=0x301F => {
                let n = ((off - 0x3000) >> 1) as usize;
                let w = self.gsu_regs[n];
                if off & 1 == 0 {
                    w as u8
                } else {
                    (w >> 8) as u8
                }
            }
            0x3030 => self.gsu_sfr as u8,
            0x3031 => (self.gsu_sfr >> 8) as u8,
            0x3034 => self.gsu_pbr,
            0x3037 => 0,
            0x3039 => u8::from(self.gsu.as_ref().is_some_and(|gsu| gsu.clock_select())),
            0x303B => 0x52, // VCR — report a Super-FX version code
            _ => 0,
        }
    }

    /// Write a memory-mapped GSU register. Writing R15-high ($301F) launches the
    /// GSU from `pbr:R15`; later CPU cycles advance it concurrently.
    fn gsu_reg_write(&mut self, off: u16, v: u8) {
        match off {
            0x3000..=0x301F => {
                let n = ((off - 0x3000) >> 1) as usize;
                if off & 1 == 0 {
                    self.gsu_regs[n] = (self.gsu_regs[n] & 0xFF00) | v as u16;
                } else {
                    self.gsu_regs[n] = (self.gsu_regs[n] & 0x00FF) | ((v as u16) << 8);
                    if n == 15 {
                        self.gsu_kick();
                    }
                }
            }
            0x3030 => self.gsu_sfr = (self.gsu_sfr & 0xFF00) | v as u16,
            0x3031 => self.gsu_sfr = (self.gsu_sfr & 0x00FF) | ((v as u16) << 8),
            0x3034 => {
                self.gsu_pbr = v & 0x7F;
                if let Some(gsu) = self.gsu.as_mut() {
                    gsu.set_program_bank(v);
                }
            }
            0x3037 => {
                if let Some(gsu) = self.gsu.as_mut() {
                    gsu.set_config(v);
                }
            }
            0x3038 => {
                if let Some(gsu) = self.gsu.as_mut() {
                    gsu.set_screen_base(v);
                }
            }
            0x3039 => {
                if let Some(gsu) = self.gsu.as_mut() {
                    gsu.set_clock_select(v);
                }
            }
            0x303A => {
                if let Some(gsu) = self.gsu.as_mut() {
                    gsu.set_screen_mode(v);
                }
            }
            _ => {}
        }
    }

    /// Arm the attached GSU from `pbr:R15`. Execution advances beside the CPU
    /// from [`Self::tick_gsu`], and both processors use the same RAM vector.
    fn gsu_kick(&mut self) {
        let Some(g) = self.gsu.as_mut() else {
            return;
        };
        g.r = self.gsu_regs;
        let pbr = self.gsu_pbr;
        let pc = self.gsu_regs[15];
        let entry_regs = self.gsu_regs;
        let capture_cd99 = pbr == 0x01 && pc == 0xCD99 && self.gsu_first_cd99_entry_ram.is_none();
        let capture_ce37 = pbr == 0x01 && pc == 0xCE37 && self.gsu_first_ce37_entry_ram.is_none();
        let capture_d9ff =
            pbr == 0x01 && pc == 0xD9FF && self.gsu_first_d9ff_register_trace.is_none();
        if capture_cd99 {
            self.gsu_first_cd99_entry_ram = Some(g.ram.clone());
        }
        if capture_ce37 {
            self.gsu_first_ce37_entry_ram = Some(g.ram.clone());
        }
        if capture_cd99 || capture_ce37 || capture_d9ff {
            g.trace_next_run();
        }
        self.gsu_last_entry = (pbr, pc);
        self.gsu_last_entry_ram_probe = [0x003A, 0x24C2, 0x24C4, 0x0014]
            .map(|address| u16::from_le_bytes([g.ram[address], g.ram[address + 1]]));
        g.start(pbr, pc);
        self.gsu_sfr |= 0x0020;
        self.gsu_kicks += 1;
        self.gsu_active_run = Some(ActiveGsuRun {
            sequence: self.gsu_kicks,
            pbr,
            pc,
            ram_probe: self.gsu_last_entry_ram_probe,
            entry_regs,
            entry_tick: self.gsu_ticks,
        });
    }

    /// Advance an in-flight Super FX job by a bounded master-clock slice.
    pub fn tick_gsu(&mut self, master_clocks: u64) {
        const JOB_INSTRUCTION_LIMIT: u64 = 10_000_000;
        self.gsu_ticks = self.gsu_ticks.wrapping_add(1);
        if self.gsu_active_run.is_none() {
            return;
        }

        // The GSU only stalls when an instruction actually touches a denied
        // RAM/ROM bus. Cached ALU code continues if one SCMR grant is absent;
        // the core tracks and releases those access-specific waits itself.
        let can_advance = self.gsu.as_ref().is_some_and(|gsu| gsu.is_running());
        if can_advance {
            let gsu = self.gsu.as_mut().expect("active GSU job has a core");
            gsu.run_master_slice(master_clocks);
            if gsu.is_running() && gsu.last_run_steps >= JOB_INSTRUCTION_LIMIT {
                gsu.stop_at_instruction_limit();
            }
        }

        if self.gsu.as_ref().is_some_and(|gsu| !gsu.is_running()) {
            self.finish_gsu_run();
        }
    }

    pub fn gsu_clock_select(&self) -> bool {
        self.gsu.as_ref().is_some_and(|gsu| gsu.clock_select())
    }

    fn finish_gsu_run(&mut self) {
        let Some(active) = self.gsu_active_run.take() else {
            return;
        };
        let g = self.gsu.as_mut().expect("active GSU job has a core");
        if active.pbr == 0x01 && active.pc == 0xCD99 && self.gsu_first_cd99_exit_ram.is_none() {
            self.gsu_first_cd99_exit_ram = Some(g.ram.clone());
            self.gsu_first_cd99_pc_trace = Some(g.pc_trace());
            self.gsu_first_cd99_register_trace = Some(g.register_trace());
            self.gsu_first_cd99_point_states = Some(g.point_states());
        }
        if active.pbr == 0x01 && active.pc == 0xCE37 && self.gsu_first_ce37_exit_ram.is_none() {
            self.gsu_first_ce37_exit_ram = Some(g.ram.clone());
            self.gsu_first_ce37_pc_trace = Some(g.pc_trace());
            self.gsu_first_ce37_register_trace = Some(g.register_trace());
        }
        if active.pbr == 0x01 && active.pc == 0xD9FF && self.gsu_first_d9ff_register_trace.is_none()
        {
            self.gsu_first_d9ff_register_trace = Some(g.register_trace());
        }
        if g.last_run_hit_limit {
            self.gsu_step_limit_hits = self.gsu_step_limit_hits.wrapping_add(1);
        }
        let (exit_pbr, exit_pc, _) = g.execution_state();
        let exit_ram_probe = [0x003A, 0x24C2, 0x24C4, 0x0014]
            .map(|address| u16::from_le_bytes([g.ram[address], g.ram[address + 1]]));
        self.gsu_recent_runs.push(GsuRunEvent {
            sequence: active.sequence,
            pbr: active.pbr,
            pc: active.pc,
            ram_probe: active.ram_probe,
            exit_ram_probe,
            entry_regs: active.entry_regs,
            entry_tick: active.entry_tick,
            exit_tick: self.gsu_ticks,
            steps: g.last_run_steps,
            hit_limit: g.last_run_hit_limit,
            exit_pbr,
            exit_pc,
            target_ram_writes: g.target_ram_writes(),
            timing_breakdown: g.timing_breakdown(),
        });
        if self.gsu_recent_runs.len() > 256 {
            self.gsu_recent_runs.remove(0);
        }
        self.gsu_regs = g.r;
        self.gsu_sfr &= !0x0020;
    }

    /// True for the CPU math registers ($4202-06 write, $4214-17 read), which
    /// live in banks $00-$3F / $80-$BF.
    fn is_math_reg(addr: u32) -> bool {
        let bank = (addr >> 16) & 0xFF;
        let off = addr & 0xFFFF;
        (bank <= 0x3F || (0x80..=0xBF).contains(&bank)) && (0x4202..=0x4217).contains(&off)
    }

    /// Map a 24-bit CPU address to a ROM offset (LoROM) or WRAM index.
    /// Returns `Ok(rom_offset)` for ROM, `Err(wram_index)` for WRAM, or `None`
    /// for unmapped (hardware register) space.
    fn classify(&self, addr: u32) -> Option<Result<usize, usize>> {
        let bank = (addr >> 16) & 0xFF;
        let off = (addr & 0xFFFF) as usize;
        // WRAM banks $7E/$7F (full 64 KB each).
        if bank == 0x7E {
            return Some(Err(off));
        }
        if bank == 0x7F {
            return Some(Err(0x1_0000 + off));
        }
        // Low-RAM mirror $00-$3F / $80-$BF : $0000-$1FFF -> WRAM $7E:0000.
        if off < 0x2000 && (bank <= 0x3F || (0x80..=0xBF).contains(&bank)) {
            return Some(Err(off));
        }
        // Super FX carts expose ROM as a linear, full-bank window at
        // $40-$5F/$C0-$DF.  This is not ordinary LoROM mirroring: for example
        // SF2's CPU read at $44:B66D addresses ROM offset $04:B66D, not
        // `((bank & $7f) * $8000 + $366d)`.  The title/attract object VM uses
        // these long pointers extensively.
        if self.gsu.is_some() && ((0x40..=0x5F).contains(&bank) || (0xC0..=0xDF).contains(&bank)) {
            let o = (((bank as usize) & 0x1F) << 16) | off;
            return Some(Ok(o % self.rom.len().max(1)));
        }
        // LoROM: $xx:8000-FFFF -> ((bank&0x7F)<<15) | (off-0x8000).
        if off >= 0x8000 {
            let o = (((bank as usize) & 0x7F) << 15) | (off - 0x8000);
            return Some(Ok(o % self.rom.len().max(1)));
        }
        None
    }

    pub fn read8(&self, addr: u32) -> u8 {
        if Self::is_math_reg(addr) {
            return match addr & 0xFFFF {
                0x4214 => self.quotient as u8,
                0x4215 => (self.quotient >> 8) as u8,
                0x4216 => self.rdmpy as u8,
                0x4217 => (self.rdmpy >> 8) as u8,
                _ => 0,
            };
        }
        // Memory-mapped GSU registers ($3000-$303F) when a GSU is attached.
        if self.gsu.is_some() && Self::is_gsu_reg(addr) {
            return self.gsu_reg_read((addr & 0xFFFF) as u16);
        }
        // GSU cart RAM: complete 64 KiB bank $70 (and bank $71 mirror).
        if ((addr >> 16) & 0xFF) & 0xFE == 0x70 {
            if self
                .gsu
                .as_ref()
                .is_some_and(|gsu| gsu.is_running() && gsu.ram_access_enabled())
            {
                return 0;
            }
            let index = (addr & 0xFFFF) as usize;
            return self
                .gsu
                .as_ref()
                .map_or(self.gsuram[index], |gsu| gsu.ram[index]);
        }
        // CPU-side Super FX RAM aperture.  Each register bank mirrors the low
        // 8 KiB at $6000-$7FFF; banks $70/$71 above expose the full 64 KiB.
        let bank = (addr >> 16) & 0xFF;
        let off = addr & 0xFFFF;
        if self.gsu.is_some()
            && (0x6000..=0x7FFF).contains(&off)
            && (bank <= 0x3E || (0x80..=0xBE).contains(&bank))
        {
            if self
                .gsu
                .as_ref()
                .is_some_and(|gsu| gsu.is_running() && gsu.ram_access_enabled())
            {
                return 0;
            }
            let index = (off & 0x1FFF) as usize;
            return self
                .gsu
                .as_ref()
                .map_or(self.gsuram[index], |gsu| gsu.ram[index]);
        }
        match self.classify(addr) {
            Some(Ok(o)) => self.rom.get(o).copied().unwrap_or(0),
            Some(Err(i)) => self.wram.get(i).copied().unwrap_or(0),
            None => 0,
        }
    }
    pub fn write8(&mut self, addr: u32, v: u8) {
        if Self::is_math_reg(addr) {
            match addr & 0xFFFF {
                0x4202 => self.mpy_a = v,
                0x4203 => self.rdmpy = self.mpy_a as u16 * v as u16,
                0x4204 => self.dividend = (self.dividend & 0xFF00) | v as u16,
                0x4205 => self.dividend = (self.dividend & 0x00FF) | ((v as u16) << 8),
                0x4206 => {
                    if v == 0 {
                        self.quotient = 0xFFFF;
                        self.rdmpy = self.dividend;
                    } else {
                        self.quotient = self.dividend / v as u16;
                        self.rdmpy = self.dividend % v as u16;
                    }
                }
                _ => {}
            }
            return;
        }
        // Memory-mapped GSU registers ($3000-$303F) when a GSU is attached.
        if self.gsu.is_some() && Self::is_gsu_reg(addr) {
            self.gsu_reg_write((addr & 0xFFFF) as u16, v);
            return;
        }
        // GSU cart RAM: complete 64 KiB bank $70 (and bank $71 mirror).
        if ((addr >> 16) & 0xFF) & 0xFE == 0x70 {
            if self
                .gsu
                .as_ref()
                .is_some_and(|gsu| gsu.is_running() && gsu.ram_access_enabled())
            {
                return;
            }
            let index = (addr & 0xFFFF) as usize;
            if let Some(gsu) = self.gsu.as_mut() {
                gsu.ram[index] = v;
            } else {
                self.gsuram[index] = v;
            }
            return;
        }
        let bank = (addr >> 16) & 0xFF;
        let off = addr & 0xFFFF;
        if self.gsu.is_some()
            && (0x6000..=0x7FFF).contains(&off)
            && (bank <= 0x3E || (0x80..=0xBE).contains(&bank))
        {
            if self
                .gsu
                .as_ref()
                .is_some_and(|gsu| gsu.is_running() && gsu.ram_access_enabled())
            {
                return;
            }
            let index = (off & 0x1FFF) as usize;
            if let Some(gsu) = self.gsu.as_mut() {
                gsu.ram[index] = v;
            } else {
                self.gsuram[index] = v;
            }
            return;
        }
        if let Some(Err(i)) = self.classify(addr) {
            if let Some(b) = self.wram.get_mut(i) {
                *b = v;
            }
        }
        // ROM / unmapped writes ignored.
    }
    pub fn read16(&self, addr: u32) -> u16 {
        self.read8(addr) as u16 | ((self.read8(addr.wrapping_add(1)) as u16) << 8)
    }
    pub fn write16(&mut self, addr: u32, v: u16) {
        self.write8(addr, v as u8);
        self.write8(addr.wrapping_add(1), (v >> 8) as u8);
    }
    /// Read WRAM at a low-RAM offset (bank $7E).
    pub fn wram_read16(&self, wram_addr: u32) -> u16 {
        self.read16(0x7E_0000 | wram_addr)
    }
    pub fn wram_write16(&mut self, wram_addr: u32, v: u16) {
        self.write16(0x7E_0000 | wram_addr, v);
    }
}

/// Address of the bootstrap stub in low WRAM ($00:0200).
const STUB_PC: u16 = 0x0200;
/// The lightweight leaf-call harness uses the normal slow-ROM cycle length.
/// The full retail machine has address-specific timing in `RetailBootBus`.
const LEAF_CALL_MASTER_CLOCKS_PER_CYCLE: u64 = 8;

impl System for SnesBus {
    fn read(&mut self, addr: u32, _at: AddressType, _s: &Signals) -> u8 {
        // Leaf calls still run cartridge coprocessors concurrently with the
        // host CPU. Without this, a RAM trampoline can kick the GSU and then
        // wait forever on status because only the host core is advancing.
        self.tick_gsu(LEAF_CALL_MASTER_CLOCKS_PER_CYCLE);
        // Override the reset vector so power-on reset jumps into our stub.
        match addr {
            0x00_FFFC => return STUB_PC as u8,
            0x00_FFFD => return (STUB_PC >> 8) as u8,
            _ => {}
        }
        self.read8(addr)
    }
    fn write(&mut self, addr: u32, data: u8, _at: AddressType, _s: &Signals) {
        self.tick_gsu(LEAF_CALL_MASTER_CLOCKS_PER_CYCLE);
        self.write8(addr, data);
    }
    fn res(&mut self) -> bool {
        // One-shot reset pulse (mirrors the core's test harness).
        let r = self.res_line;
        if r {
            self.res_line = false;
        }
        r
    }
}

/// CPU entry state for a call.
pub struct Entry {
    pub a: u16,
    pub x: u16,
    pub y: u16,
    pub d: u16,
    pub dbr: u8,
    /// Processor status (native). Default 0x00 = 16-bit A/X/Y (M=0,X=0).
    pub p: u8,
}
impl Default for Entry {
    fn default() -> Self {
        Entry {
            a: 0,
            x: 0,
            y: 0,
            d: 0,
            dbr: 0,
            p: 0x00,
        }
    }
}

/// Result registers after a call.
pub struct Exit {
    pub a: u8,
    /// Full 16-bit accumulator (C = B:A).
    pub c: u16,
    pub x: u16,
    pub y: u16,
}

/// Run a far (`JSL`/`RTL`) subroutine at `target` (24-bit) to its return.
/// Seed WRAM inputs on `bus` first; read WRAM outputs from `bus` afterward.
/// `entry.a/x/y` are loaded into the registers on entry (16-bit).
///
/// A power-on reset (which the core requires) jumps to a bootstrap stub in low
/// WRAM that: enters native mode, sets 16-bit A/X/Y, loads A/X/Y from a param
/// block, `JSL target`, then `STP`. After `STP`, `a()/x()/y()` hold the
/// routine's return registers.
pub fn call(bus: &mut SnesBus, target: u32, entry: &Entry) -> Exit {
    // Param block for entry registers (direct page, low WRAM).
    const PA: u32 = 0x00F0;
    const PX: u32 = 0x00F2;
    const PY: u32 = 0x00F4;
    bus.wram_write16(PA, entry.a);
    bus.wram_write16(PX, entry.x);
    bus.wram_write16(PY, entry.y);

    // Bootstrap stub at $00:0200. Enter native mode, set the M/X width bits
    // from entry.p (SEP #mx after REP #$30 makes 8-bit whichever of A/X the
    // callee expects), optionally set DBR (strat code uses abs,x xalblks in
    // bank $7E), load entry regs, JSL the target, STP.
    let mx = entry.p & 0x30;
    let dbr = entry.dbr;
    let mut stub: Vec<u8> = vec![
        0x18, // CLC
        0xFB, // XCE            -> native mode
        0xC2, 0x30, // REP #$30 -> 16-bit A/X/Y
        0xE2, mx, // SEP #mx    -> 8-bit A and/or X per entry.p
    ];
    if dbr != 0 {
        stub.extend_from_slice(&[
            0xA9, dbr,  // LDA #dbr
            0x48, // PHA
            0xAB, // PLB
        ]);
    }
    stub.extend_from_slice(&[
        0xA5,
        PA as u8, // LDA $F0
        0xA6,
        PX as u8, // LDX $F2
        0xA4,
        PY as u8, // LDY $F4
        0x22,
        target as u8,
        (target >> 8) as u8,
        (target >> 16) as u8, // JSL target
        0xDB,                 // STP
    ]);
    for (i, b) in stub.iter().enumerate() {
        bus.write8(0x00_0000 + STUB_PC as u32 + i as u32, *b);
    }

    bus.res_line = true; // pulse reset for this run
    let mut cpu = CPU::new();
    let mut started = false;
    let mut guard = 0u64;
    loop {
        cpu.cycle(bus);
        guard += 1;
        // stp is set at power-on; it clears once reset completes and the stub
        // starts running, then sets again at the stub's STP.
        if !cpu.stopped() {
            started = true;
        } else if started {
            break;
        }
        if guard > 50_000_000 {
            break;
        }
    }
    Exit {
        a: cpu.a(),
        c: cpu.c(),
        x: cpu.x(),
        y: cpu.y(),
    }
}

/// Run a near (`JSR`/`RTS`) subroutine at `target` (24-bit). Same contract as
/// [`call`] but for functions that return with `RTS` (2-byte, same bank). The
/// bootstrap pre-pushes a 2-byte return and `JML`s to the target so its `RTS`
/// lands on a STP trap in the target's bank.
pub fn call_near(bus: &mut SnesBus, target: u32, entry: &Entry) -> Exit {
    const PA: u32 = 0x00F0;
    const PX: u32 = 0x00F2;
    const PY: u32 = 0x00F4;
    bus.wram_write16(PA, entry.a);
    bus.wram_write16(PX, entry.x);
    bus.wram_write16(PY, entry.y);
    // RTS trap. ROM banks see `$0300` through the low-WRAM mirror. A routine
    // executing from SF2's generated bank-$7F code must instead return to an
    // unused byte in that same full WRAM bank.
    let target_bank = target >> 16;
    let trap_pc = if matches!(target_bank, 0x7E | 0x7F) {
        0xD000u16
    } else {
        0x0300
    };
    bus.write8((target_bank << 16) | u32::from(trap_pc), 0xDB);

    let mx = entry.p & 0x30;
    let return_address = trap_pc.wrapping_sub(1);
    let stub: [u8; 19] = [
        0x18, // CLC
        0xFB, // XCE
        0xC2,
        0x30, // REP #$30
        0xE2,
        mx, // SEP #mx
        0xA5,
        PA as u8, // LDA $F0
        0xA6,
        PX as u8, // LDX $F2
        0xA4,
        PY as u8, // LDY $F4
        0xF4,
        return_address as u8,
        (return_address >> 8) as u8, // PEA trap-1 (RTS -> trap)
        0x5C,
        target as u8,
        (target >> 8) as u8,
        (target >> 16) as u8, // JML target
    ];
    for (i, b) in stub.iter().enumerate() {
        bus.write8(0x00_0000 + STUB_PC as u32 + i as u32, *b);
    }

    bus.res_line = true;
    let mut cpu = CPU::new();
    let mut started = false;
    let mut guard = 0u64;
    loop {
        cpu.cycle(bus);
        guard += 1;
        if !cpu.stopped() {
            started = true;
        } else if started {
            break;
        }
        if guard > 50_000_000 {
            break;
        }
    }
    Exit {
        a: cpu.a(),
        c: cpu.c(),
        x: cpu.x(),
        y: cpu.y(),
    }
}

/// Load the retail ROM from the repo root.
pub fn load_retail_rom() -> Option<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)?
        .to_path_buf();
    std::fs::read(root.join("Star Fox (USA) (Rev 2).sfc")).ok()
}

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// Load the ROM built from the reference disassembly (`sf-oracle/data/sf.sfc`).
/// This is the ROM the symbol map (`SYMBOLS.TXT`) refers to; its gameplay logic
/// is the same 65816 code the Rust port was written from. Regenerate with the
/// dosbox build (see memory `rom-oracle-plan`). Not committed (ROM data).
pub fn load_built_rom() -> Option<Vec<u8>> {
    std::fs::read(data_dir().join("sf.sfc")).ok()
}

/// Parse `data/symbols.txt` (`LABEL\t$00xxxxxx` per line) into name -> SNES
/// LoROM address. Emitted by the disassembly build (`MAPDEC SF.MAP`).
pub fn load_symbols() -> std::collections::HashMap<String, u32> {
    let mut map = std::collections::HashMap::new();
    let Ok(txt) = std::fs::read_to_string(data_dir().join("symbols.txt")) else {
        return map;
    };
    for line in txt.lines() {
        let mut it = line.split('\t');
        if let (Some(name), Some(addr)) = (it.next(), it.next()) {
            let hex = addr.trim().trim_start_matches('$');
            if let Ok(v) = u32::from_str_radix(hex, 16) {
                map.insert(name.trim().to_string(), v);
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Self-test: inject a tiny routine into ROM space and run it, proving the
    /// bus, CPU setup, stack seeding, and return-trap all work end to end.
    ///   REP #$20 ; LDA $10 ; CLC ; ADC $12 ; STA $14 ; RTL
    #[test]
    fn harness_runs_injected_routine() {
        let mut rom = vec![0u8; 0x2000];
        let code = [
            0xC2, 0x20, // REP #$20  (16-bit A)
            0xA5, 0x10, // LDA $10
            0x18, //       CLC
            0x65, 0x12, // ADC $12
            0x85, 0x14, // STA $14
            0x6B, //       RTL
        ];
        rom[..code.len()].copy_from_slice(&code);
        let mut bus = SnesBus::new(rom);
        bus.wram_write16(0x10, 1111);
        bus.wram_write16(0x12, 2222);

        let exit = call(&mut bus, 0x00_8000, &Entry::default());
        let sum = bus.wram_read16(0x14);
        eprintln!(
            "ORACLE self-test: 1111+2222 -> wram[$14]={sum}, A={}",
            exit.a
        );
        assert_eq!(sum, 3333, "injected routine should compute 1111+2222");
    }

    /// The retail ROM is present, headerless, and LoROM ("STAR FOX" @ $00:FFC0).
    #[test]
    fn retail_rom_loads_lorom() {
        let Some(rom) = load_retail_rom() else {
            eprintln!("skip: retail ROM not found at repo root");
            return;
        };
        assert_eq!(rom.len(), 0x10_0000, "expected 1 MB headerless ROM");
        let bus = SnesBus::new(rom);
        let title: String = (0..8).map(|i| bus.read8(0x00_FFC0 + i) as char).collect();
        eprintln!("ORACLE ROM title @ $FFC0: {title:?}");
        assert_eq!(title, "STAR FOX", "LoROM header title mismatch");
    }

    #[test]
    fn super_fx_cpu_uses_linear_full_bank_rom_window() {
        let mut rom = vec![0u8; 0x10_0000];
        rom[0x04_B66D] = 0x1E;
        rom[0x02_366D] = 0xD0;
        let mut bus = SnesBus::new(rom);

        // Generic LoROM mapping is retained until the GSU cartridge mapping is
        // attached, then $40-$5F/$C0-$DF become linear 64 KiB ROM banks.
        assert_eq!(bus.read8(0x44_B66D), 0xD0);
        bus.enable_gsu();
        assert_eq!(bus.read8(0x44_B66D), 0x1E);
        assert_eq!(bus.read8(0xC4_B66D), 0x1E);
    }

    #[test]
    fn super_fx_cpu_ram_windows_cover_the_full_64k_bank() {
        let mut bus = SnesBus::new(vec![0; 0x10_0000]);
        bus.enable_gsu();

        bus.write8(0x70_F0E4, 0xA5);
        assert_eq!(bus.read8(0x70_F0E4), 0xA5);
        assert_eq!(bus.read8(0x71_F0E4), 0xA5, "$71 mirrors $70");

        bus.write8(0x00_6123, 0x5A);
        assert_eq!(bus.read8(0x80_6123), 0x5A, "low 8 KiB CPU aperture mirrors");
        assert_eq!(bus.read8(0x70_0123), 0x5A);
    }

    /// End-to-end resolution: a symbol resolves to ROM bytes that match the
    /// disassembly. `N3DVECS_L` opens `stz x1+1; stz y1+1; stz z1+1; stx tmpx;
    /// sty tmpy; phb`. This pins the SYMBOLS.TXT -> built-ROM -> ASM chain that
    /// the differential tests rely on.
    #[test]
    fn symbol_resolves_to_matching_rom_code() {
        let syms = load_symbols();
        let Some(&addr) = syms.get("N3DVECS_L") else {
            eprintln!("skip: symbols.txt not present");
            return;
        };
        let Some(rom) = load_built_rom() else {
            eprintln!("skip: built ROM data/sf.sfc not present");
            return;
        };
        let bus = SnesBus::new(rom);
        let got: Vec<u8> = (0..11).map(|i| bus.read8(addr + i)).collect();
        eprintln!("ORACLE N3DVECS_L @ ${addr:06X}: {got:02X?}");
        // stz $03; stz $09; stz $8B; stx $70; sty $72; phb
        let expect = [
            0x64, 0x03, 0x64, 0x09, 0x64, 0x8B, 0x86, 0x70, 0x84, 0x72, 0x8B,
        ];
        assert_eq!(got, expect, "symbol -> ROM -> disassembly chain broke");
    }
}
