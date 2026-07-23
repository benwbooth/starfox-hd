#!/usr/bin/env python3
"""Table-driven 65816 disassembler for the SF2 host banks.

Clean-room: decodes the retail ROM's own machine bytes. Tracks the M
(accumulator width) and X (index width) processor flags via SEP/REP so
immediate operands get the correct 1- vs 2-byte width -- essential for a
correct linear/flow disassembly of 65816 code.

All offsets here are LoROM *file* offsets (file offset == linear ROM offset
for this headerless 1 MB image); CPU addresses are reconstructed as
$bb:hhhh where bb = file_off >> 15 | 0x00 mapping and hhhh = 0x8000 + (off & 0x7FFF).
"""
from __future__ import annotations
from dataclasses import dataclass

# Addressing modes
IMP="imp"        # implied
ACC="acc"        # A
IMM_M="imm_m"    # immediate, width follows M flag
IMM_X="imm_x"    # immediate, width follows X flag
IMM8="imm8"      # always 1-byte immediate (SEP/REP/COP/BRK/WDM)
DP="dp"          # direct page  d
DPX="dpx"        # d,x
DPY="dpy"        # d,y
IDP="idp"        # (d)
IDX="idx"        # (d,x)
IDY="idy"        # (d),y
IDL="idl"        # [d]
IDLY="idly"      # [d],y
ABS="abs"        # a
ABX="abx"        # a,x
ABY="aby"        # a,y
ABL="abl"        # al (long)
ABLX="ablx"      # al,x
IND="ind"        # (a)  -- JMP (a)
IAX="iax"        # (a,x) -- JMP (a,x)
IAL="ial"        # [a]  -- JML [a]
REL="rel"        # branch rel8
RELL="rell"      # branch rel16 (BRL/PER)
SR="sr"          # d,s  stack relative
SRY="sry"        # (d,s),y
BM="bm"          # block move src,dst (MVN/MVP)

# operand byte length per mode (immediate modes resolved at decode time)
_FIXLEN = {
    IMP:0, ACC:0, IMM8:1, DP:1, DPX:1, DPY:1, IDP:1, IDX:1, IDY:1, IDL:1,
    IDLY:1, ABS:2, ABX:2, ABY:2, ABL:3, ABLX:3, IND:2, IAX:2, IAL:2,
    REL:1, RELL:2, SR:1, SRY:1, BM:2,
}

# opcode table: byte -> (mnemonic, mode)
OPS = {
0x00:("BRK",IMM8),0x01:("ORA",IDX),0x02:("COP",IMM8),0x03:("ORA",SR),
0x04:("TSB",DP),0x05:("ORA",DP),0x06:("ASL",DP),0x07:("ORA",IDL),
0x08:("PHP",IMP),0x09:("ORA",IMM_M),0x0A:("ASL",ACC),0x0B:("PHD",IMP),
0x0C:("TSB",ABS),0x0D:("ORA",ABS),0x0E:("ASL",ABS),0x0F:("ORA",ABL),
0x10:("BPL",REL),0x11:("ORA",IDY),0x12:("ORA",IDP),0x13:("ORA",SRY),
0x14:("TRB",DP),0x15:("ORA",DPX),0x16:("ASL",DPX),0x17:("ORA",IDLY),
0x18:("CLC",IMP),0x19:("ORA",ABY),0x1A:("INC",ACC),0x1B:("TCS",IMP),
0x1C:("TRB",ABS),0x1D:("ORA",ABX),0x1E:("ASL",ABX),0x1F:("ORA",ABLX),
0x20:("JSR",ABS),0x21:("AND",IDX),0x22:("JSL",ABL),0x23:("AND",SR),
0x24:("BIT",DP),0x25:("AND",DP),0x26:("ROL",DP),0x27:("AND",IDL),
0x28:("PLP",IMP),0x29:("AND",IMM_M),0x2A:("ROL",ACC),0x2B:("PLD",IMP),
0x2C:("BIT",ABS),0x2D:("AND",ABS),0x2E:("ROL",ABS),0x2F:("AND",ABL),
0x30:("BMI",REL),0x31:("AND",IDY),0x32:("AND",IDP),0x33:("AND",SRY),
0x34:("BIT",DPX),0x35:("AND",DPX),0x36:("ROL",DPX),0x37:("AND",IDLY),
0x38:("SEC",IMP),0x39:("AND",ABY),0x3A:("DEC",ACC),0x3B:("TSC",IMP),
0x3C:("BIT",ABX),0x3D:("AND",ABX),0x3E:("ROL",ABX),0x3F:("AND",ABLX),
0x40:("RTI",IMP),0x41:("EOR",IDX),0x42:("WDM",IMM8),0x43:("EOR",SR),
0x44:("MVP",BM),0x45:("EOR",DP),0x46:("LSR",DP),0x47:("EOR",IDL),
0x48:("PHA",IMP),0x49:("EOR",IMM_M),0x4A:("LSR",ACC),0x4B:("PHK",IMP),
0x4C:("JMP",ABS),0x4D:("EOR",ABS),0x4E:("LSR",ABS),0x4F:("EOR",ABL),
0x50:("BVC",REL),0x51:("EOR",IDY),0x52:("EOR",IDP),0x53:("EOR",SRY),
0x54:("MVN",BM),0x55:("EOR",DPX),0x56:("LSR",DPX),0x57:("EOR",IDLY),
0x58:("CLI",IMP),0x59:("EOR",ABY),0x5A:("PHY",IMP),0x5B:("TCD",IMP),
0x5C:("JML",ABL),0x5D:("EOR",ABX),0x5E:("LSR",ABX),0x5F:("EOR",ABLX),
0x60:("RTS",IMP),0x61:("ADC",IDX),0x62:("PER",RELL),0x63:("ADC",SR),
0x64:("STZ",DP),0x65:("ADC",DP),0x66:("ROR",DP),0x67:("ADC",IDL),
0x68:("PLA",IMP),0x69:("ADC",IMM_M),0x6A:("ROR",ACC),0x6B:("RTL",IMP),
0x6C:("JMP",IND),0x6D:("ADC",ABS),0x6E:("ROR",ABS),0x6F:("ADC",ABL),
0x70:("BVS",REL),0x71:("ADC",IDY),0x72:("ADC",IDP),0x73:("ADC",SRY),
0x74:("STZ",DPX),0x75:("ADC",DPX),0x76:("ROR",DPX),0x77:("ADC",IDLY),
0x78:("SEI",IMP),0x79:("ADC",ABY),0x7A:("PLY",IMP),0x7B:("TDC",IMP),
0x7C:("JMP",IAX),0x7D:("ADC",ABX),0x7E:("ROR",ABX),0x7F:("ADC",ABLX),
0x80:("BRA",REL),0x81:("STA",IDX),0x82:("BRL",RELL),0x83:("STA",SR),
0x84:("STY",DP),0x85:("STA",DP),0x86:("STX",DP),0x87:("STA",IDL),
0x88:("DEY",IMP),0x89:("BIT",IMM_M),0x8A:("TXA",IMP),0x8B:("PHB",IMP),
0x8C:("STY",ABS),0x8D:("STA",ABS),0x8E:("STX",ABS),0x8F:("STA",ABL),
0x90:("BCC",REL),0x91:("STA",IDY),0x92:("STA",IDP),0x93:("STA",SRY),
0x94:("STY",DPX),0x95:("STA",DPX),0x96:("STX",DPY),0x97:("STA",IDLY),
0x98:("TYA",IMP),0x99:("STA",ABY),0x9A:("TXS",IMP),0x9B:("TXY",IMP),
0x9C:("STZ",ABS),0x9D:("STA",ABX),0x9E:("STZ",ABX),0x9F:("STA",ABLX),
0xA0:("LDY",IMM_X),0xA1:("LDA",IDX),0xA2:("LDX",IMM_X),0xA3:("LDA",SR),
0xA4:("LDY",DP),0xA5:("LDA",DP),0xA6:("LDX",DP),0xA7:("LDA",IDL),
0xA8:("TAY",IMP),0xA9:("LDA",IMM_M),0xAA:("TAX",IMP),0xAB:("PLB",IMP),
0xAC:("LDY",ABS),0xAD:("LDA",ABS),0xAE:("LDX",ABS),0xAF:("LDA",ABL),
0xB0:("BCS",REL),0xB1:("LDA",IDY),0xB2:("LDA",IDP),0xB3:("LDA",SRY),
0xB4:("LDY",DPX),0xB5:("LDA",DPX),0xB6:("LDX",DPY),0xB7:("LDA",IDLY),
0xB8:("CLV",IMP),0xB9:("LDA",ABY),0xBA:("TSX",IMP),0xBB:("TYX",IMP),
0xBC:("LDY",ABX),0xBD:("LDA",ABX),0xBE:("LDX",ABY),0xBF:("LDA",ABLX),
0xC0:("CPY",IMM_X),0xC1:("CMP",IDX),0xC2:("REP",IMM8),0xC3:("CMP",SR),
0xC4:("CPY",DP),0xC5:("CMP",DP),0xC6:("DEC",DP),0xC7:("CMP",IDL),
0xC8:("INY",IMP),0xC9:("CMP",IMM_M),0xCA:("DEX",IMP),0xCB:("WAI",IMP),
0xCC:("CPY",ABS),0xCD:("CMP",ABS),0xCE:("DEC",ABS),0xCF:("CMP",ABL),
0xD0:("BNE",REL),0xD1:("CMP",IDY),0xD2:("CMP",IDP),0xD3:("CMP",SRY),
0xD4:("PEI",DP),0xD5:("CMP",DPX),0xD6:("DEC",DPX),0xD7:("CMP",IDLY),
0xD8:("CLD",IMP),0xD9:("CMP",ABY),0xDA:("PHX",IMP),0xDB:("STP",IMP),
0xDC:("JML",IAL),0xDD:("CMP",ABX),0xDE:("DEC",ABX),0xDF:("CMP",ABLX),
0xE0:("CPX",IMM_X),0xE1:("SBC",IDX),0xE2:("SEP",IMM8),0xE3:("SBC",SR),
0xE4:("CPX",DP),0xE5:("SBC",DP),0xE6:("INC",DP),0xE7:("SBC",IDL),
0xE8:("INX",IMP),0xE9:("SBC",IMM_M),0xEA:("NOP",IMP),0xEB:("XBA",IMP),
0xEC:("CPX",ABS),0xED:("SBC",ABS),0xEE:("INC",ABS),0xEF:("SBC",ABL),
0xF0:("BEQ",REL),0xF1:("SBC",IDY),0xF2:("SBC",IDP),0xF3:("SBC",SRY),
0xF4:("PEA",ABS),0xF5:("SBC",DPX),0xF6:("INC",DPX),0xF7:("SBC",IDLY),
0xF8:("SED",IMP),0xF9:("SBC",ABY),0xFA:("PLX",IMP),0xFB:("XCE",IMP),
0xFC:("JSR",IAX),0xFD:("SBC",ABX),0xFE:("INC",ABX),0xFF:("SBC",ABLX),
}

BRANCHES = {"BPL","BMI","BVC","BVS","BCC","BCS","BNE","BEQ","BRA","BRL"}
JUMPS = {"JMP","JML"}
CALLS = {"JSR","JSL"}
RETURNS = {"RTS","RTL","RTI"}
STOPS = RETURNS | {"BRA","JMP","JML","STP"}  # linear-flow terminators


def file_to_cpu(off: int) -> int:
    """LoROM file offset -> CPU long address bb:hhhh (banks $00-$3F, low ROM)."""
    bank = off >> 15
    addr = 0x8000 + (off & 0x7FFF)
    return (bank << 16) | addr


def cpu_to_file(cpu: int) -> int | None:
    """CPU long address -> file offset (LoROM low banks $00-$3F only)."""
    bank = (cpu >> 16) & 0xFF
    addr = cpu & 0xFFFF
    if addr < 0x8000:
        return None
    if bank <= 0x3F:
        return (bank << 15) | (addr & 0x7FFF)
    if 0x80 <= bank <= 0xBF:
        return ((bank - 0x80) << 15) | (addr & 0x7FFF)
    return None


@dataclass
class Insn:
    off: int          # file offset
    cpu: int          # cpu long address
    raw: bytes
    mnem: str
    mode: str
    operand: int      # numeric operand (as-read, little endian)
    length: int
    m: int            # M flag state at decode (0=16bit,1=8bit)
    x: int
    target: int | None = None   # resolved absolute cpu addr for branch/jump/call


def _read(rom, off, n):
    return int.from_bytes(rom[off:off+n], "little")


def decode_one(rom: bytes, off: int, m: int, x: int) -> Insn:
    """Decode a single instruction. m/x are current flag states (1=8bit)."""
    op = rom[off]
    mnem, mode = OPS[op]
    cur = off + 1
    if mode == IMM_M:
        n = 1 if m else 2
    elif mode == IMM_X:
        n = 1 if x else 2
    else:
        n = _FIXLEN[mode]
    operand = _read(rom, cur, n) if n else 0
    length = 1 + n
    raw = rom[off:off+length]
    cpu = file_to_cpu(off)
    ins = Insn(off, cpu, raw, mnem, mode, operand, length, m, x)

    # resolve control-flow targets
    if mode == REL:
        rel = operand if operand < 0x80 else operand - 0x100
        ins.target = ((cpu + length) & 0xFFFF) + rel & 0xFFFF
        ins.target |= (cpu & 0xFF0000)
    elif mode == RELL:
        rel = operand if operand < 0x8000 else operand - 0x10000
        ins.target = (((cpu + length) & 0xFFFF) + rel & 0xFFFF) | (cpu & 0xFF0000)
    elif mnem in ("JMP","JSR") and mode == ABS:
        ins.target = (cpu & 0xFF0000) | operand
    elif mnem in ("JML","JSL") and mode == ABL:
        ins.target = operand
    return ins


def update_flags(ins: Insn, m: int, x: int):
    """Apply SEP/REP effects to (m,x). Returns new (m,x)."""
    if ins.mnem == "SEP":
        if ins.operand & 0x20: m = 1
        if ins.operand & 0x10: x = 1
    elif ins.mnem == "REP":
        if ins.operand & 0x20: m = 0
        if ins.operand & 0x10: x = 0
    return m, x


def fmt_operand(ins: Insn, labels: dict | None = None) -> str:
    md, v = ins.mode, ins.operand
    def lbl(addr):
        if labels and addr in labels:
            return labels[addr]
        return None
    if md in (IMP,): return ""
    if md == ACC: return "A"
    if md in (IMM_M, IMM_X):
        return f"#${v:0{ins.length*0+ (4 if ins.length==3 else 2)}X}" if False else (f"#${v:04X}" if ins.length==3 else f"#${v:02X}")
    if md == IMM8: return f"#${v:02X}"
    if md == DP:  return f"${v:02X}"
    if md == DPX: return f"${v:02X},X"
    if md == DPY: return f"${v:02X},Y"
    if md == IDP: return f"(${v:02X})"
    if md == IDX: return f"(${v:02X},X)"
    if md == IDY: return f"(${v:02X}),Y"
    if md == IDL: return f"[${v:02X}]"
    if md == IDLY: return f"[${v:02X}],Y"
    if md == ABS:
        t = lbl(ins.target) if ins.target is not None else None
        return t or f"${v:04X}"
    if md == ABX: return f"${v:04X},X"
    if md == ABY: return f"${v:04X},Y"
    if md == ABL:
        t = lbl(ins.target) if ins.target is not None else None
        return t or f"${v:06X}"
    if md == ABLX: return f"${v:06X},X"
    if md == IND: return f"(${v:04X})"
    if md == IAX: return f"(${v:04X},X)"
    if md == IAL: return f"[${v:04X}]"
    if md in (REL, RELL):
        t = lbl(ins.target) if ins.target is not None else None
        return t or f"${ins.target & 0xFFFF:04X}"
    if md == SR:  return f"${v:02X},S"
    if md == SRY: return f"(${v:02X},S),Y"
    if md == BM:  return f"${v & 0xFF:02X},${(v>>8)&0xFF:02X}"
    return f"${v:X}"


def fmt_insn(ins: Insn, labels: dict | None = None) -> str:
    rawhex = " ".join(f"{b:02X}" for b in ins.raw)
    oper = fmt_operand(ins, labels)
    flags = f"{'m' if ins.m else 'M'}{'x' if ins.x else 'X'}"
    lab = ""
    if labels and ins.cpu in labels:
        lab = labels[ins.cpu] + ":"
    text = f"{ins.mnem} {oper}".rstrip()
    return f"{ins.off:06X}  {ins.cpu>>16:02X}:{ins.cpu&0xFFFF:04X}  {rawhex:<12} [{flags}] {lab:<16}{text}"
