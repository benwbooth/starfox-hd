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

use w65c816::{AddressType, Signals, System, CPU};

/// LoROM + WRAM bus over the retail ROM.
pub struct SnesBus {
    rom: Vec<u8>,
    /// 128 KB: banks $7E and $7F.
    wram: Vec<u8>,
    /// 32 KB GSU cart RAM, bank $70:$0000-$7FFF (m_* vars, drawlist).
    gsuram: Vec<u8>,
    /// Reset line: pulsed true once to boot the CPU out of its power-on STP.
    res_line: bool,
    /// PPU/CPU math registers (used by e.g. n3dvecs' `mulslog`).
    mpy_a: u8,      // $4202 WRMPYA
    rdmpy: u16,     // $4216/7 RDMPY (product, or divide remainder)
    dividend: u16,  // $4204/5 WRDIV
    quotient: u16,  // $4214/5 RDDIV
}

impl SnesBus {
    pub fn new(rom: Vec<u8>) -> Self {
        SnesBus {
            rom,
            wram: vec![0u8; 0x2_0000],
            gsuram: vec![0u8; 0x8000],
            res_line: true,
            mpy_a: 0,
            rdmpy: 0,
            dividend: 0,
            quotient: 0,
        }
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
        // GSU cart RAM: bank $70 (and $71 mirror), offsets < $8000.
        if ((addr >> 16) & 0xFF) & 0xFE == 0x70 && (addr & 0xFFFF) < 0x8000 {
            return self.gsuram[(addr & 0x7FFF) as usize];
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
        // GSU cart RAM: bank $70 (and $71 mirror), offsets < $8000.
        if ((addr >> 16) & 0xFF) & 0xFE == 0x70 && (addr & 0xFFFF) < 0x8000 {
            self.gsuram[(addr & 0x7FFF) as usize] = v;
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

impl System for SnesBus {
    fn read(&mut self, addr: u32, _at: AddressType, _s: &Signals) -> u8 {
        // Override the reset vector so power-on reset jumps into our stub.
        match addr {
            0x00_FFFC => return STUB_PC as u8,
            0x00_FFFD => return (STUB_PC >> 8) as u8,
            _ => {}
        }
        self.read8(addr)
    }
    fn write(&mut self, addr: u32, data: u8, _at: AddressType, _s: &Signals) {
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
        Entry { a: 0, x: 0, y: 0, d: 0, dbr: 0, p: 0x00 }
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
    // callee expects), load entry regs, JSL the target, STP.
    let mx = entry.p & 0x30;
    let stub: [u8; 17] = [
        0x18, // CLC
        0xFB, // XCE            -> native mode
        0xC2, 0x30, // REP #$30 -> 16-bit A/X/Y
        0xE2, mx, // SEP #mx    -> 8-bit A and/or X per entry.p
        0xA5, PA as u8, // LDA $F0
        0xA6, PX as u8, // LDX $F2
        0xA4, PY as u8, // LDY $F4
        0x22, target as u8, (target >> 8) as u8, (target >> 16) as u8, // JSL target
        0xDB, // STP
    ];
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
    Exit { a: cpu.a(), c: cpu.c(), x: cpu.x(), y: cpu.y() }
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
    // RTS trap: STP at low-WRAM $0300 (reached in the target's bank via mirror).
    bus.write8(0x00_0300, 0xDB);

    let mx = entry.p & 0x30;
    let stub: [u8; 19] = [
        0x18, // CLC
        0xFB, // XCE
        0xC2, 0x30, // REP #$30
        0xE2, mx, // SEP #mx
        0xA5, PA as u8, // LDA $F0
        0xA6, PX as u8, // LDX $F2
        0xA4, PY as u8, // LDY $F4
        0xF4, 0xFF, 0x02, // PEA $02FF (RTS -> $0300)
        0x5C, target as u8, (target >> 8) as u8, (target >> 16) as u8, // JML target
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
    Exit { a: cpu.a(), c: cpu.c(), x: cpu.x(), y: cpu.y() }
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
        eprintln!("ORACLE self-test: 1111+2222 -> wram[$14]={sum}, A={}", exit.a);
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
        let expect = [0x64, 0x03, 0x64, 0x09, 0x64, 0x8B, 0x86, 0x70, 0x84, 0x72, 0x8B];
        assert_eq!(got, expect, "symbol -> ROM -> disassembly chain broke");
    }
}
