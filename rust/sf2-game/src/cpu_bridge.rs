//! Exact compatibility executor for SF2 65816 strategies that have not yet
//! been replaced by clean-room Rust routines.
//!
//! This is deliberately a narrow subroutine boundary, not a second game-state
//! model: the CPU and GSU operate directly on [`Memory`]'s WRAM/GSU RAM, then
//! return control to the recovered Rust map/path runtime.  As individual
//! strategies are proven against retail they can leave this bridge without a
//! behavior discontinuity.

#[path = "../../sf-oracle/src/gsu.rs"]
mod gsu;

use self::gsu::Gsu;
use crate::memory::Memory;
use w65c816::{AddressType, Signals, System, CPU};

const STUB_PC: u16 = 0x0200;
const PARAM_A: u16 = 0x00F0;
const PARAM_X: u16 = 0x00F2;
const PARAM_Y: u16 = 0x00F4;
const CARRY_STATUS_BIT: u8 = 0x01;
// A normal per-object strategy returns in tens of thousands of bus cycles.
// Keep the guard tight enough that a routine which has entered a retail
// scheduler/NMI wait cannot freeze the 20 Hz host loop for several seconds.
const MAX_STRATEGY_CYCLES: u64 = 1_000_000;

/// Verification-only return state from an exact retail subroutine.
pub(crate) struct RoutineResult {
    pub(crate) value: u8,
    pub(crate) condition: bool,
}

pub(crate) struct CpuBridge {
    gsu: Gsu,
    gsu_regs: [u16; 16],
    gsu_pbr: u8,
    gsu_sfr: u16,
    mpy_a: u8,
    rdmpy: u16,
    dividend: u16,
    quotient: u16,
}

impl CpuBridge {
    pub(crate) fn new(rom: Vec<u8>) -> Self {
        Self {
            gsu: Gsu::new(rom),
            gsu_regs: [0; 16],
            gsu_pbr: 0,
            gsu_sfr: 0,
            mpy_a: 0,
            rdmpy: 0,
            dividend: 0,
            quotient: 0,
        }
    }

    /// Execute one retail far-call routine with 8-bit A, 16-bit X/Y and DBR
    /// `$7E`, the invariant used by SF2's strategy dispatcher.
    pub(crate) fn call_strategy(
        &mut self,
        memory: &mut Memory,
        target: u32,
        object: u16,
    ) -> Result<(), String> {
        self.call_routine(memory, target, object).map(|_| ())
    }

    /// Execute an oracle routine and retain the low return value plus the
    /// carry condition used by reviewed predicate helpers. Shipping builds do
    /// not compile this compatibility executor.
    pub(crate) fn call_routine(
        &mut self,
        memory: &mut Memory,
        target: u32,
        object: u16,
    ) -> Result<RoutineResult, String> {
        // Reset/bootstrap traffic is an oracle-harness implementation detail,
        // not retail game state. Preserve the parameter, stack and stub pages
        // while leaving the real direct-page scratch bytes visible.
        let saved_params: Vec<u8> = (PARAM_A..=PARAM_Y + 1)
            .map(|address| memory.read_byte(address))
            .collect();
        let saved_boot: Vec<u8> = (0x0100u16..0x0300)
            .map(|address| memory.read_byte(address))
            .collect();

        memory.write_word(PARAM_A, 0);
        memory.write_word(PARAM_X, object);
        memory.write_word(PARAM_Y, 0);
        let stub = [
            0x18, // CLC
            0xFB, // XCE: native mode
            0xC2,
            0x30, // REP #$30: 16-bit A/X/Y
            0xE2,
            0x20, // SEP #$20: 8-bit A, 16-bit X/Y
            0xA9,
            0x7E, // LDA #$7E
            0x48, // PHA
            0xAB, // PLB
            0xA5,
            PARAM_A as u8, // LDA $F0
            0xA6,
            PARAM_X as u8, // LDX $F2
            0xA4,
            PARAM_Y as u8, // LDY $F4
            0x22,
            target as u8,
            (target >> 8) as u8,
            (target >> 16) as u8, // JSL target
            0xDB,                 // STP
        ];
        for (index, byte) in stub.into_iter().enumerate() {
            memory.write_byte(STUB_PC.wrapping_add(index as u16), byte);
        }

        let mut bus = CpuBus {
            memory,
            gsu: &mut self.gsu,
            gsu_regs: &mut self.gsu_regs,
            gsu_pbr: &mut self.gsu_pbr,
            gsu_sfr: &mut self.gsu_sfr,
            mpy_a: &mut self.mpy_a,
            rdmpy: &mut self.rdmpy,
            dividend: &mut self.dividend,
            quotient: &mut self.quotient,
            res_line: true,
        };
        let mut cpu = CPU::new();
        let mut started = false;
        let mut cycles = 0u64;
        loop {
            cpu.cycle(&mut bus);
            cycles += 1;
            if !cpu.stopped() {
                started = true;
            } else if started {
                break;
            }
            if cycles > MAX_STRATEGY_CYCLES {
                break;
            }
        }

        for (index, byte) in saved_params.into_iter().enumerate() {
            bus.memory
                .write_byte(PARAM_A.wrapping_add(index as u16), byte);
        }
        for (index, byte) in saved_boot.into_iter().enumerate() {
            bus.memory
                .write_byte(0x0100u16.wrapping_add(index as u16), byte);
        }
        if !cpu.stopped() || cycles > MAX_STRATEGY_CYCLES {
            return Err(format!(
                "65816 strategy ${target:06X} for object ${object:04X} exceeded the {MAX_STRATEGY_CYCLES}-cycle guard at ${:02X}:{:04X}",
                cpu.pbr(),
                cpu.pc(),
            ));
        }
        Ok(RoutineResult {
            value: cpu.a(),
            condition: cpu.p() & CARRY_STATUS_BIT != 0,
        })
    }
}

struct CpuBus<'a> {
    memory: &'a mut Memory,
    gsu: &'a mut Gsu,
    gsu_regs: &'a mut [u16; 16],
    gsu_pbr: &'a mut u8,
    gsu_sfr: &'a mut u16,
    mpy_a: &'a mut u8,
    rdmpy: &'a mut u16,
    dividend: &'a mut u16,
    quotient: &'a mut u16,
    res_line: bool,
}

impl CpuBus<'_> {
    fn is_register_bank(bank: u8) -> bool {
        bank <= 0x3F || (0x80..=0xBF).contains(&bank)
    }

    fn is_gsu_register(address: u32) -> bool {
        let bank = (address >> 16) as u8;
        let offset = address as u16;
        Self::is_register_bank(bank) && (0x3000..=0x303F).contains(&offset)
    }

    fn is_math_register(address: u32) -> bool {
        let bank = (address >> 16) as u8;
        let offset = address as u16;
        Self::is_register_bank(bank) && (0x4202..=0x4217).contains(&offset)
    }

    fn read_gsu_register(&self, offset: u16) -> u8 {
        match offset {
            0x3000..=0x301F => {
                let register = usize::from((offset - 0x3000) >> 1);
                let value = self.gsu_regs[register];
                if offset & 1 == 0 {
                    value as u8
                } else {
                    (value >> 8) as u8
                }
            }
            0x3030 => *self.gsu_sfr as u8,
            0x3031 => (*self.gsu_sfr >> 8) as u8,
            0x3034 => *self.gsu_pbr,
            0x303B => 0x52,
            _ => 0,
        }
    }

    fn write_gsu_register(&mut self, offset: u16, value: u8) {
        match offset {
            0x3000..=0x301F => {
                let register = usize::from((offset - 0x3000) >> 1);
                if offset & 1 == 0 {
                    self.gsu_regs[register] = (self.gsu_regs[register] & 0xFF00) | u16::from(value);
                } else {
                    self.gsu_regs[register] =
                        (self.gsu_regs[register] & 0x00FF) | (u16::from(value) << 8);
                    if register == 15 {
                        self.kick_gsu();
                    }
                }
            }
            0x3030 => *self.gsu_sfr = (*self.gsu_sfr & 0xFF00) | u16::from(value),
            0x3031 => *self.gsu_sfr = (*self.gsu_sfr & 0x00FF) | (u16::from(value) << 8),
            0x3034 => *self.gsu_pbr = value,
            _ => {}
        }
    }

    fn kick_gsu(&mut self) {
        self.gsu.ram.copy_from_slice(self.memory.gsu_ram());
        self.gsu.r = *self.gsu_regs;
        self.gsu.run(*self.gsu_pbr, self.gsu_regs[15]);
        *self.gsu_regs = self.gsu.r;
        self.memory.gsu_ram_mut().copy_from_slice(&self.gsu.ram);
        // The core ran synchronously to STOP, so expose G clear to the CPU's
        // standard `runmario_l` polling loop.
        *self.gsu_sfr &= !0x0020;
    }

    fn read8(&mut self, address: u32) -> u8 {
        if Self::is_math_register(address) {
            return match address as u16 {
                0x4214 => *self.quotient as u8,
                0x4215 => (*self.quotient >> 8) as u8,
                0x4216 => *self.rdmpy as u8,
                0x4217 => (*self.rdmpy >> 8) as u8,
                _ => 0,
            };
        }
        if Self::is_gsu_register(address) {
            return self.read_gsu_register(address as u16);
        }

        let bank = (address >> 16) as u8;
        let offset = address as u16;
        if bank & 0xFE == 0x70 {
            return self.memory.gsu_ram()[usize::from(offset)];
        }
        if Self::is_register_bank(bank) && (0x6000..=0x7FFF).contains(&offset) {
            return self.memory.gsu_ram()[usize::from(offset & 0x1FFF)];
        }
        if Self::is_register_bank(bank) && offset < 0x2000 {
            return self.memory.read_byte(offset);
        }
        if (0x40..=0x5F).contains(&bank) || (0xC0..=0xDF).contains(&bank) {
            let index = (usize::from(bank & 0x1F) << 16) | usize::from(offset);
            return self.memory.rom().get(index).copied().unwrap_or(0);
        }
        self.memory.read_long_byte(address)
    }

    fn write8(&mut self, address: u32, value: u8) {
        if Self::is_math_register(address) {
            match address as u16 {
                0x4202 => *self.mpy_a = value,
                0x4203 => *self.rdmpy = u16::from(*self.mpy_a) * u16::from(value),
                0x4204 => *self.dividend = (*self.dividend & 0xFF00) | u16::from(value),
                0x4205 => *self.dividend = (*self.dividend & 0x00FF) | (u16::from(value) << 8),
                0x4206 => {
                    if value == 0 {
                        *self.quotient = 0xFFFF;
                        *self.rdmpy = *self.dividend;
                    } else {
                        *self.quotient = *self.dividend / u16::from(value);
                        *self.rdmpy = *self.dividend % u16::from(value);
                    }
                }
                _ => {}
            }
            return;
        }
        if Self::is_gsu_register(address) {
            self.write_gsu_register(address as u16, value);
            return;
        }

        let bank = (address >> 16) as u8;
        let offset = address as u16;
        if bank & 0xFE == 0x70 {
            self.memory.gsu_ram_mut()[usize::from(offset)] = value;
            return;
        }
        if Self::is_register_bank(bank) && (0x6000..=0x7FFF).contains(&offset) {
            self.memory.gsu_ram_mut()[usize::from(offset & 0x1FFF)] = value;
            return;
        }
        if Self::is_register_bank(bank) && offset < 0x2000 {
            self.memory.write_byte(offset, value);
            return;
        }
        self.memory.write_long_byte(address, value);
    }
}

impl System for CpuBus<'_> {
    fn read(&mut self, address: u32, _kind: AddressType, _signals: &Signals) -> u8 {
        match address {
            0x00_FFFC => STUB_PC as u8,
            0x00_FFFD => (STUB_PC >> 8) as u8,
            _ => self.read8(address),
        }
    }

    fn write(&mut self, address: u32, value: u8, _kind: AddressType, _signals: &Signals) {
        self.write8(address, value);
    }

    fn res(&mut self) -> bool {
        let active = self.res_line;
        self.res_line = false;
        active
    }
}
