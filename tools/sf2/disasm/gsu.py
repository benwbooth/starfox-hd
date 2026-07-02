#!/usr/bin/env python3
"""GSU / Super FX (GSU-2) disassembler for the SF2 3D/shape banks.

The Super FX instruction set is prefix-modal: opcodes $3D/$3E/$3F (ALT1/ALT2/
ALT3) change the meaning of the *following* opcode, and $10-$1F (TO), $20-$2F
(WITH), $B0-$BF (FROM) are register prefixes. This decoder tracks the ALT state
across a linear stream and selects the correct mnemonic per (opcode, alt).

Instruction lengths: most ops are 1 byte; branches ($05-$0F) take a 1-byte
signed offset; IBT/LMS/SMS ($A0-$AF) take 1 immediate byte; IWT/LM/SM
($F0-$FF) take a 2-byte immediate word. Verified against SF1 GSU source in
reference/ultrastarfox/SF/MARIO/*.MC.
"""
from __future__ import annotations
from dataclasses import dataclass

R = lambda n: f"r{n}"

# Per-opcode decode. Value can be:
#   ("MNEM", extra_bytes)                      -- alt-independent
#   {0:(...),1:(...),2:(...),3:(...)}          -- alt-dependent variants
# extra_bytes: 0,1,2 ; special 'branch' means 1 signed byte offset.

def _reg_ops():
    t = {}
    # 0x00-0x0F control / branches
    t[0x00]=("STOP",0); t[0x01]=("NOP",0); t[0x02]=("CACHE",0)
    t[0x03]=("LSR",0); t[0x04]=("ROL",0)
    for op,mn in [(0x05,"BRA"),(0x06,"BLT"),(0x07,"BGE"),(0x08,"BNE"),(0x09,"BEQ"),
                  (0x0A,"BPL"),(0x0B,"BMI"),(0x0C,"BCC"),(0x0D,"BCS"),(0x0E,"BVC"),(0x0F,"BVS")]:
        t[op]=(mn,"branch")
    # 0x10-0x1F TO / MOVE(with B)   -- render as TO rN
    for n in range(16): t[0x10+n]=(f"TO {R(n)}",0)
    # 0x20-0x2F WITH rN
    for n in range(16): t[0x20+n]=(f"WITH {R(n)}",0)
    # 0x30-0x3B STORE (rN)  ; 0x3C LOOP ; 0x3D/E/F ALT prefixes
    for n in range(12): t[0x30+n]={0:(f"STW ({R(n)})",0),2:(f"STB ({R(n)})",0)}
    t[0x3C]=("LOOP",0); t[0x3D]=("ALT1",0); t[0x3E]=("ALT2",0); t[0x3F]=("ALT3",0)
    # 0x40-0x4B LOAD (rN) ; 0x4C PLOT/RPIX ; 0x4D SWAP ; 0x4E COLOR/CMODE ; 0x4F NOT
    for n in range(12): t[0x40+n]={0:(f"LDW ({R(n)})",0),2:(f"LDB ({R(n)})",0)}
    t[0x4C]={0:("PLOT",0),1:("RPIX",0)}
    t[0x4D]=("SWAP",0)
    t[0x4E]={0:("COLOR",0),1:("CMODE",0)}
    t[0x4F]=("NOT",0)
    # 0x50-0x5F ADD/ADC/ADD#n ; 0x60-0x6F SUB/SBC/SUB#n/CMP
    for n in range(16):
        t[0x50+n]={0:(f"ADD {R(n)}",0),1:(f"ADC {R(n)}",0),2:(f"ADD #{n}",0),3:(f"ADC #{n}",0)}
    for n in range(16):
        t[0x60+n]={0:(f"SUB {R(n)}",0),1:(f"SBC {R(n)}",0),2:(f"SUB #{n}",0),3:(f"CMP {R(n)}",0)}
    # 0x70 MERGE ; 0x71-0x7F AND/BIC/AND#n
    t[0x70]=("MERGE",0)
    for n in range(1,16):
        t[0x70+n]={0:(f"AND {R(n)}",0),1:(f"BIC {R(n)}",0),2:(f"AND #{n}",0),3:(f"BIC #{n}",0)}
    # 0x80-0x8F MULT/UMULT/MULT#n/UMULT#n
    for n in range(16):
        t[0x80+n]={0:(f"MULT {R(n)}",0),1:(f"UMULT {R(n)}",0),2:(f"MULT #{n}",0),3:(f"UMULT #{n}",0)}
    # 0x90 SBK ; 0x91-0x94 LINK #n ; 0x95 SEX ; 0x96 ASR/DIV2 ; 0x97 ROR
    t[0x90]=("SBK",0)
    for n in range(1,5): t[0x90+n]=(f"LINK #{n}",0)
    t[0x95]=("SEX",0); t[0x96]={0:("ASR",0),1:("DIV2",0)}; t[0x97]=("ROR",0)
    # 0x98-0x9D JMP rN / LJMP rN ; 0x9E LOB ; 0x9F FMULT/LMULT
    for n in range(8,14): t[0x98+(n-8)]={0:(f"JMP {R(n)}",0),1:(f"LJMP {R(n)}",0)}
    t[0x9E]=("LOB",0); t[0x9F]={0:("FMULT",0),1:("LMULT",0)}
    # 0xA0-0xAF IBT rN,#pp / LMS rN,(#) / SMS rN,(#)  -- 1 imm byte
    for n in range(16):
        t[0xA0+n]={0:(f"IBT {R(n)},#",1),1:(f"LMS {R(n)},",1),2:(f"SMS {R(n)},",1)}
    # 0xB0-0xBF FROM rN / MOVES(with B)
    for n in range(16): t[0xB0+n]=(f"FROM {R(n)}",0)
    # 0xC0 HIB ; 0xC1-0xCF OR/XOR/OR#n/XOR#n
    t[0xC0]=("HIB",0)
    for n in range(1,16):
        t[0xC0+n]={0:(f"OR {R(n)}",0),1:(f"XOR {R(n)}",0),2:(f"OR #{n}",0),3:(f"XOR #{n}",0)}
    # 0xD0-0xDE INC rN ; 0xDF GETC/RAMB/ROMB
    for n in range(15): t[0xD0+n]=(f"INC {R(n)}",0)
    t[0xDF]={0:("GETC",0),2:("RAMB",0),3:("ROMB",0)}
    # 0xE0-0xEE DEC rN ; 0xEF GETB/GETBH/GETBL/GETBS
    for n in range(15): t[0xE0+n]=(f"DEC {R(n)}",0)
    t[0xEF]={0:("GETB",0),1:("GETBH",0),2:("GETBL",0),3:("GETBS",0)}
    # 0xF0-0xFF IWT rN,#pppp / LM rN,(#) / SM rN,(#)  -- 2 imm bytes
    for n in range(16):
        t[0xF0+n]={0:(f"IWT {R(n)},#",2),1:(f"LM {R(n)},",2),2:(f"SM {R(n)},",2)}
    return t

TABLE = _reg_ops()
PREFIX_OPS = set(range(0x10,0x20)) | set(range(0x20,0x30)) | set(range(0xB0,0xC0))
ALT_OPS = {0x3D:1, 0x3E:2, 0x3F:3}


@dataclass
class GsuInsn:
    off: int
    raw: bytes
    text: str
    length: int
    is_branch: bool = False
    target: int | None = None


def decode_one(rom: bytes, off: int, alt: int):
    op = rom[off]
    entry = TABLE.get(op)
    if entry is None:
        return GsuInsn(off, rom[off:off+1], f"DB ${op:02X}", 1), 0
    if isinstance(entry, dict):
        variant = entry.get(alt) or entry.get(0)
    else:
        variant = entry
    mnem, extra = variant
    length = 1
    text = mnem
    tgt = None
    is_branch = False
    if extra == "branch":
        rel = rom[off+1]
        rel = rel - 256 if rel >= 128 else rel
        tgt = off + 2 + rel
        text = f"{mnem} ${tgt:06X}"
        length = 2
        is_branch = True
    elif extra == 1:
        imm = rom[off+1]
        text = f"{mnem}${imm:02X}" if mnem.endswith((",","#")) else f"{mnem} #${imm:02X}"
        length = 2
    elif extra == 2:
        imm = rom[off+1] | (rom[off+2] << 8)
        text = f"{mnem}${imm:04X}" if mnem.endswith((",","#")) else f"{mnem} #${imm:04X}"
        length = 3
    raw = rom[off:off+length]
    ins = GsuInsn(off, raw, text, length, is_branch, tgt)
    # new alt state: ALT ops set it; prefix ops keep it; everything else clears
    if op in ALT_OPS:
        new_alt = ALT_OPS[op]
    elif op in PREFIX_OPS or op == 0x3C:  # LOOP keeps alt? no; prefixes keep
        new_alt = alt if op in PREFIX_OPS else 0
    else:
        new_alt = 0
    return ins, new_alt


def disasm(rom: bytes, off: int, count: int):
    out = []
    alt = 0
    for _ in range(count):
        if off >= len(rom): break
        ins, alt = decode_one(rom, off, alt)
        rawhex = " ".join(f"{b:02X}" for b in ins.raw)
        out.append(f"{ins.off:06X}  {rawhex:<9} {ins.text}")
        off += ins.length
    return "\n".join(out)


if __name__ == "__main__":
    import os
    REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
    with open(os.path.join(REPO, "Star Fox 2 (USA, Europe).sfc"), "rb") as f:
        rom = f.read()
    print(disasm(rom, 0x90000, 40))
