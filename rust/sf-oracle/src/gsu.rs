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
//! memory ops; pixel-plot (PLOT/RPIX/COLOR) is out of scope (rendering only).
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
    b_flag: bool, // WITH set (in-place / MOVE modifier)
    romb_pending: bool,
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
            romb_pending: false,
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

    /// LoROM program fetch at R15 in bank `pbr`.
    fn fetch(&mut self) -> u8 {
        let pc = self.r[15];
        let addr = ((self.pbr as usize & 0x7F) << 15) | ((pc as usize) & 0x7FFF);
        let b = self.rom.get(addr % self.rom.len().max(1)).copied().unwrap_or(0);
        self.r[15] = pc.wrapping_add(1);
        b
    }

    /// Start execution at `pbr:pc`, run to STOP (or the cycle guard).
    pub fn run(&mut self, pbr: u8, pc: u16) {
        self.pbr = pbr;
        self.r[15] = pc;
        self.set_flag(sfr::G, true);
        let mut guard = 0u64;
        while self.flag(sfr::G) && guard < 5_000_000 {
            self.step();
            guard += 1;
        }
    }

    fn src(&self) -> u16 {
        self.r[self.sreg]
    }
    fn write_dst(&mut self, v: u16) {
        // R15 (PC) writes are jumps; otherwise normal.
        self.r[self.dreg] = v;
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

    fn step(&mut self) {
        let op = self.fetch();
        match op {
            0x00 => {
                // STOP
                self.set_flag(sfr::G, false);
                return;
            }
            0x01 | 0x02 => { /* NOP / CACHE */ }
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
                    0x05 => true,                                             // BRA
                    0x06 => self.flag(sfr::S) != self.flag(sfr::OV),          // BLT
                    0x07 => self.flag(sfr::S) == self.flag(sfr::OV),          // BGE
                    0x08 => !self.flag(sfr::Z),                               // BNE
                    0x09 => self.flag(sfr::Z),                                // BEQ
                    0x0A => !self.flag(sfr::S),                               // BPL
                    0x0B => self.flag(sfr::S),                                // BMI
                    0x0C => !self.flag(sfr::CY),                              // BCC
                    0x0D => self.flag(sfr::CY),                               // BCS
                    0x0E => !self.flag(sfr::OV),                              // BVC
                    0x0F => self.flag(sfr::OV),                               // BVS
                    _ => unreachable!(),
                };
                if take {
                    self.r[15] = (self.r[15] as i32 + rel) as u16;
                }
                // branches don't reset alt via the shared path; fall through
                self.reset_prefix();
                return;
            }
            0x10..=0x1F => {
                if self.b_flag {
                    // MOVE Rn <- Sreg (TO with the WITH/B flag set).
                    let v = self.r[self.sreg];
                    self.r[(op & 0xF) as usize] = v;
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
                // STW/STB (Rn): store Sreg to GSU RAM at Rn; ALT2 => byte.
                let a = self.r[(op & 0xF) as usize] as usize & 0xFFFF;
                let s = self.src();
                self.ram[a] = s as u8;
                if !self.alt2 {
                    self.ram[(a + 1) & 0xFFFF] = (s >> 8) as u8;
                }
            }
            0x3C => {
                // LOOP: DEC r12; if !=0 branch to r13
                let v = self.r[12].wrapping_sub(1);
                self.r[12] = v;
                self.set_zs(v);
                if v != 0 {
                    self.r[15] = self.r[13];
                }
            }
            0x3D => {
                self.alt1 = true;
                return;
            }
            0x3E => {
                self.alt2 = true;
                return;
            }
            0x3F => {
                self.alt1 = true;
                self.alt2 = true;
                return;
            }
            0x40..=0x4B => {
                // LDW/LDB (Rn) — RAM load; ALT2 => byte
                let a = self.r[(op & 0xF) as usize] as usize;
                let v = if self.alt2 {
                    self.ram[a & 0xFFFF] as u16
                } else {
                    self.ram[a & 0xFFFF] as u16 | ((self.ram[(a + 1) & 0xFFFF] as u16) << 8)
                };
                self.write_dst(v);
                self.set_zs(v);
            }
            0x4D => {
                // SWAP bytes
                let s = self.src();
                let v = s.rotate_left(8);
                self.write_dst(v);
                self.set_zs(v);
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
                let cin = if self.alt1 { self.flag(sfr::CY) as u16 } else { 0 };
                let v = self.add_flags(self.src(), operand, cin);
                self.write_dst(v);
            }
            0x60..=0x6F => {
                // SUB/SBC/SUB#/CMP
                let n = (op & 0xF) as usize;
                let is_cmp = self.alt1 && self.alt2; // ALT3 => CMP
                let operand = if self.alt2 && !self.alt1 { n as u16 } else { self.r[n] };
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
                    (self.src() as u32 * operand as u32) as u16 // UMULT
                } else {
                    ((self.src() as i16 as i32) * (operand as i16 as i32)) as u16 // MULT
                };
                self.write_dst(v);
                self.set_zs(v);
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
                // JMP/LJMP Rn
                let n = (op & 0xF) as usize;
                if self.alt1 {
                    self.pbr = self.r[n] as u8; // LJMP sets pbr from... simplified
                }
                self.r[15] = self.r[n];
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
            }
            0xA0..=0xAF => {
                // IBT Rn,#pp (ALT0) / LMS Rn,(imm) (ALT1) / SMS (imm),Rn (ALT2).
                // LMS/SMS use a short word-index (imm*2) into GSU RAM.
                let n = (op & 0xF) as usize;
                let imm = self.fetch();
                if self.alt1 {
                    let a = (imm as usize) << 1;
                    self.r[n] = self.ram[a] as u16 | ((self.ram[(a + 1) & 0xFFFF] as u16) << 8);
                } else if self.alt2 {
                    let a = (imm as usize) << 1;
                    self.ram[a] = self.r[n] as u8;
                    self.ram[(a + 1) & 0xFFFF] = (self.r[n] >> 8) as u8;
                } else {
                    self.r[n] = imm as i8 as i16 as u16; // IBT sign-extends
                }
                self.reset_prefix();
                return;
            }
            0xB0..=0xBF => {
                if self.b_flag {
                    // MOVES Rn <- Sreg (FROM with the WITH/B flag), sets flags.
                    let v = self.r[self.sreg];
                    self.r[(op & 0xF) as usize] = v;
                    self.set_zs(v);
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
                self.r[n] = v;
                self.set_zs(v);
            }
            0xDF => {
                // GETC/RAMB/ROMB
                if self.alt2 {
                    self.rombr = self.src() as u8; // ROMB
                } else if self.alt1 {
                    // RAMB
                } else {
                    self.romb_pending = true; // GETC (color, no-op for math)
                }
            }
            0xE0..=0xEE => {
                // DEC Rn
                let n = (op & 0xF) as usize;
                let v = self.r[n].wrapping_sub(1);
                self.r[n] = v;
                self.set_zs(v);
            }
            0xEF => {
                // GETB variants: read ROM byte at rombr:R14
                let a = ((self.rombr as usize & 0x7F) << 15) | ((self.r[14] as usize) & 0x7FFF);
                let b = self.rom.get(a % self.rom.len().max(1)).copied().unwrap_or(0);
                let cur = self.r[self.dreg];
                let v = match (self.alt1, self.alt2) {
                    (false, false) => (cur & 0xFF00) | b as u16, // GETB
                    (true, false) => (cur & 0x00FF) | ((b as u16) << 8), // GETBH
                    (false, true) => (cur & 0xFF00) | b as u16, // GETBL
                    (true, true) => b as i8 as i16 as u16,      // GETBS
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
                    self.r[n] = self.ram[a] as u16 | ((self.ram[(a + 1) & 0xFFFF] as u16) << 8);
                } else if self.alt2 {
                    let a = imm as usize & 0xFFFF;
                    self.ram[a] = self.r[n] as u8;
                    self.ram[(a + 1) & 0xFFFF] = (self.r[n] >> 8) as u8;
                } else {
                    self.r[n] = imm; // IWT
                }
                self.reset_prefix();
                return;
            }
            _ => { /* unimplemented op: treat as NOP for now */ }
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
        eprintln!("GSU mem/mult/move: r2={:#06x} r4={:#06x} r0={}", gsu.r[2], gsu.r[4], gsu.r[0]);
        assert_eq!(gsu.r[2], 0xABCD, "MOVE r2<-r1");
        assert_eq!(gsu.r[4], 0x1234, "SM/LM RAM round-trip");
        assert_eq!(gsu.r[0], 25, "FMULT high16(16384*100)");
    }
}
