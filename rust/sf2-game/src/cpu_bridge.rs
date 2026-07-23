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
const COLLISION_MATH_PROGRAM_BANK: u8 = 1;
const COLLISION_ROTATE_PROGRAM: u16 = 0xFD62;
const COLLISION_POLYGON_PROGRAM: u16 = 0xFCD7;
const COLLISION_SURFACE_PROGRAM: u16 = 0xFA31;
const COLLISION_ANGLE_INPUT: u16 = 0x0022;
const COLLISION_POLYGON_POINTER: u16 = 0x0016;
const COLLISION_X_INPUT: u16 = 0x0026;
const COLLISION_SURFACE_VALUE: u16 = 0x0028;
const COLLISION_Z_INPUT: u16 = 0x002A;
const COLLISION_ROTATED_Z: u16 = 0x002E;
const COLLISION_POLYGON_SCALE: u16 = 0x0030;
const COLLISION_ROTATED_X: u16 = 0x0068;
const COLLISION_PLANE_X: u16 = 0x00AA;
const COLLISION_PLANE_Y: u16 = 0x00AC;
const COLLISION_PLANE_Z: u16 = 0x00AE;
// A normal per-object strategy returns in tens of thousands of bus cycles.
// Keep the guard tight enough that a routine which has entered a retail
// scheduler/NMI wait cannot freeze the 20 Hz host loop for several seconds.
const MAX_STRATEGY_CYCLES: u64 = 1_000_000;

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

    fn write_collision_math_word(&mut self, address: u16, value: u16) {
        let index = usize::from(address);
        self.gsu.ram[index] = value as u8;
        self.gsu.ram[index + 1] = (value >> 8) as u8;
    }

    fn read_collision_math_word(&self, address: u16) -> u16 {
        let index = usize::from(address);
        u16::from(self.gsu.ram[index]) | (u16::from(self.gsu.ram[index + 1]) << 8)
    }

    fn run_collision_math_job(&mut self, memory: &mut Memory, program: u16) {
        self.gsu.ram.copy_from_slice(memory.gsu_ram());
        self.gsu.r = self.gsu_regs;
        self.gsu.run(COLLISION_MATH_PROGRAM_BANK, program);
        self.gsu_regs = self.gsu.r;
        let (program_bank, _, status) = self.gsu.execution_state();
        self.gsu_pbr = program_bank;
        self.gsu_sfr = status;
        memory.gsu_ram_mut().copy_from_slice(&self.gsu.ram);
    }

    /// Rotate an X/Z contact probe into a candidate object's local yaw frame
    /// with retail's exact fixed-point Super FX kernel.
    pub(crate) fn rotate_collision_probe(
        &mut self,
        memory: &mut Memory,
        yaw: u8,
        x: i16,
        z: i16,
    ) -> (i16, i16) {
        self.gsu.ram.copy_from_slice(memory.gsu_ram());
        self.write_collision_math_word(COLLISION_ROTATED_X, x as u16);
        self.write_collision_math_word(COLLISION_ROTATED_Z, z as u16);
        self.write_collision_math_word(COLLISION_ANGLE_INPUT, u16::from(yaw).wrapping_neg());
        memory.gsu_ram_mut().copy_from_slice(&self.gsu.ram);
        self.run_collision_math_job(memory, COLLISION_ROTATE_PROGRAM);
        (
            self.read_collision_math_word(COLLISION_ROTATED_X) as i16,
            self.read_collision_math_word(COLLISION_ROTATED_Z) as i16,
        )
    }

    /// Test a local X/Z probe against one exact retail convex footprint.
    pub(crate) fn collision_polygon_contains(
        &mut self,
        memory: &mut Memory,
        source_address: u16,
        scale: u8,
        x: i16,
        z: i16,
    ) -> bool {
        self.gsu.ram.copy_from_slice(memory.gsu_ram());
        self.write_collision_math_word(COLLISION_X_INPUT, x as u16);
        self.write_collision_math_word(COLLISION_Z_INPUT, z as u16);
        self.write_collision_math_word(COLLISION_POLYGON_POINTER, source_address);
        self.write_collision_math_word(COLLISION_POLYGON_SCALE, u16::from(scale));
        memory.gsu_ram_mut().copy_from_slice(&self.gsu.ram);
        self.run_collision_math_job(memory, COLLISION_POLYGON_PROGRAM);
        self.read_collision_math_word(COLLISION_POLYGON_POINTER) == 0
    }

    /// Project a local X/Z probe onto one exact retail collision plane.
    pub(crate) fn project_collision_surface(
        &mut self,
        memory: &mut Memory,
        normal: [i8; 3],
        plane_offset: i16,
        x: i16,
        z: i16,
    ) -> i16 {
        self.gsu.ram.copy_from_slice(memory.gsu_ram());
        self.write_collision_math_word(COLLISION_X_INPUT, x as u16);
        self.write_collision_math_word(COLLISION_Z_INPUT, z as u16);
        self.write_collision_math_word(COLLISION_SURFACE_VALUE, plane_offset as u16);
        self.write_collision_math_word(COLLISION_PLANE_X, u16::from(normal[0] as u8) << 8);
        self.write_collision_math_word(COLLISION_PLANE_Y, u16::from(normal[1] as u8) << 8);
        self.write_collision_math_word(COLLISION_PLANE_Z, u16::from(normal[2] as u8) << 8);
        memory.gsu_ram_mut().copy_from_slice(&self.gsu.ram);
        self.run_collision_math_job(memory, COLLISION_SURFACE_PROGRAM);
        self.read_collision_math_word(COLLISION_SURFACE_VALUE) as i16
    }

    /// Execute one retail far-call routine with 8-bit A, 16-bit X/Y and DBR
    /// `$7E`, the invariant used by SF2's strategy dispatcher.
    pub(crate) fn call_strategy(
        &mut self,
        memory: &mut Memory,
        target: u32,
        object: u16,
    ) -> Result<(), String> {
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
        Ok(())
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
