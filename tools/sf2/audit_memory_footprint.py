#!/usr/bin/env python3
"""Static memory-footprint audit of ported SF2 65816 routines.

Every native Rust doc comment of the form `$BB:AAAA..EEEE` names a source
routine. This tool linearly decodes each such range (tracking SEP/REP) and
collects the RAM it reads and writes: absolute globals, player-record fields
(`$6Axx..$6Cxx,Y`) and long WRAM/GSU-RAM addresses. Each address is then
checked against the Rust port and the oracle tests:

  local    the address is named in the Rust file that ports the routine
  known    named elsewhere in sf2-game or the SF2 oracle tests
  unknown  named nowhere: the port neither models nor documents it

An unknown address is a suspect, not a proven defect: it may be a scratch
temporary that is dead on exit. Confirm a suspect with a per-function
original-code test before changing the port. Object fields (`$xx,X`) and
direct-page temporaries are not audited here.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "disasm"))

import cpu65816 as cpu  # noqa: E402
from dump_runtime_routine import source_offset  # noqa: E402
from extract_map import DEFAULT_ROM  # noqa: E402

REPO = HERE.parents[1]
NATIVE = REPO / "rust/sf2-game/src"
ORACLE = REPO / "rust/sf-oracle/tests"
# 65816 code banks. Bank 44 names path bytecode; low banks hold GSU code.
CODE_BANKS = {0x06, 0x07, 0x0D, 0x7F}
RANGE = re.compile(r"\$([0-9A-F]{2}):([0-9A-F]{4})\.\.([0-9A-F]{4})")
READS = {"LDA", "LDX", "LDY", "CMP", "CPX", "CPY", "ADC", "SBC", "AND", "ORA",
         "EOR", "BIT", "TRB", "TSB", "INC", "DEC", "ASL", "LSR", "ROL", "ROR"}
WRITES = {"STA", "STX", "STY", "STZ", "TRB", "TSB", "INC", "DEC", "ASL", "LSR",
          "ROL", "ROR"}
PLAYER_RECORD = range(0x6A61, 0x6C40)
MAX_INSTRUCTIONS = 2000


@dataclass
class Routine:
    start: int
    end: int
    files: set[Path] = field(default_factory=set)
    reads: dict[int, set[str]] = field(default_factory=lambda: defaultdict(set))
    writes: dict[int, set[str]] = field(default_factory=lambda: defaultdict(set))
    calls: set[int] = field(default_factory=set)
    note: str = ""
    # First access in linear order: "read", "read-after-call" or "write".
    first: dict[int, str] = field(default_factory=dict)
    called: bool = False
    word_high_bytes: set[int] = field(default_factory=set)

    def label(self) -> str:
        return f"${self.start >> 16:02X}:{self.start & 0xFFFF:04X}..{self.end & 0xFFFF:04X}"


def annotated_routines() -> dict[tuple[int, int], Routine]:
    routines: dict[tuple[int, int], Routine] = {}
    for path in sorted(NATIVE.rglob("*.rs")):
        if path.name.endswith("_tests.rs"):
            continue
        for match in RANGE.finditer(path.read_text()):
            bank = int(match[1], 16)
            if bank not in CODE_BANKS:
                continue
            start = bank << 16 | int(match[2], 16)
            end = bank << 16 | int(match[3], 16)
            if end < start:
                continue
            routines.setdefault((start, end), Routine(start, end)).files.add(path)
    return routines


def width(insn: cpu.Insn) -> int:
    if insn.mnem in ("LDX", "LDY", "STX", "STY", "CPX", "CPY"):
        return 1 if insn.x else 2
    return 1 if insn.m else 2


def classify(insn: cpu.Insn) -> tuple[int, str] | None:
    """Return (address, kind) for an audited RAM operand."""
    mode, operand = insn.mode, insn.operand
    if mode == cpu.ABS and 0x0100 <= operand < 0x2000:
        return operand, "global"
    if mode == cpu.ABS and 0x6000 <= operand < 0x8000:
        return operand, "global"
    if mode in (cpu.ABX, cpu.ABY) and operand in PLAYER_RECORD:
        return operand, "player"
    if mode in (cpu.ABL, cpu.ABLX):
        bank = operand >> 16
        if mode == cpu.ABLX and (operand & 0xFFFF) < 0x0100:
            return None  # long,X object-field access, not a global
        if bank in (0x7E, 0x7F) or (bank == 0x00 and operand < 0x2000):
            return operand & 0xFFFF if bank == 0 else operand, "long" if mode == cpu.ABL else "long-indexed"
        if bank == 0x70:
            return operand, "gsu-ram"
    return None


def decode(rom: bytes, routine: Routine) -> None:
    address, m, x = routine.start, 1, 0
    for _ in range(MAX_INSTRUCTIONS):
        if address > routine.end:
            return
        try:
            insn = cpu.decode_one(rom, source_offset(address), m, x)
        except (KeyError, ValueError, IndexError) as error:
            routine.note = f"decode stopped at {address:06X}: {error}"
            return
        insn.cpu = address
        if insn.mnem in cpu.CALLS and insn.mode in (cpu.ABS, cpu.ABL):
            target = insn.operand if insn.mode == cpu.ABL else (address & 0xFF0000 | insn.operand)
            routine.calls.add(target)
            routine.called = True
        operand = classify(insn)
        if operand is not None:
            target, kind = operand
            tag = f"{kind}/{width(insn) * 8}"
            # A 16-bit access also covers the following byte.
            for byte in range(target, target + width(insn)):
                if byte not in routine.first:
                    if insn.mnem in READS:
                        routine.first[byte] = "read-after-call" if routine.called else "read"
                    else:
                        routine.first[byte] = "write"
                if byte != target:
                    routine.word_high_bytes.add(byte)
            if insn.mnem in READS:
                routine.reads[target].add(tag)
            if insn.mnem in WRITES:
                routine.writes[target].add(tag)
        m, x = cpu.update_flags(insn, m, x)
        address += insn.length
    routine.note = "instruction limit reached"


def file_label(offset: int) -> str:
    if 0x010000 <= offset < 0x017E00:
        return f"7F:{offset - 0x010000:04X}"
    if 0x050000 <= offset < 0x054E00:
        return f"7F:{offset - 0x050000 + 0x7E00:04X}"
    address = cpu.file_to_cpu(offset)
    return f"{address >> 16:02X}:{address & 0xFFFF:04X}"


def xref_index(rom: bytes) -> dict[int, list[tuple[str, str]]]:
    """Byte-pattern scan for absolute/long operands. Data can alias code, so
    this is a lead for review, not a control-flow-accurate reference list."""
    by_mode = defaultdict(list)
    for opcode, (mnemonic, mode) in cpu.OPS.items():
        if mnemonic in READS or mnemonic in WRITES:
            by_mode[mode].append((opcode, mnemonic))
    index: dict[int, list[tuple[str, str]]] = defaultdict(list)
    for offset in range(len(rom) - 3):
        entry = cpu.OPS.get(rom[offset])
        if entry is None:
            continue
        mnemonic, mode = entry
        if mnemonic not in READS and mnemonic not in WRITES:
            continue
        if mode in (cpu.ABS, cpu.ABX, cpu.ABY):
            target = rom[offset + 1] | rom[offset + 2] << 8
        elif mode in (cpu.ABL, cpu.ABLX):
            target = rom[offset + 1] | rom[offset + 2] << 8 | rom[offset + 3] << 16
            if target >> 16 == 0x7E or target >> 16 == 0x00:
                target &= 0xFFFF
        else:
            continue
        index[target].append((file_label(offset), mnemonic))
    return index


def mentions(text: str) -> set[int]:
    found = set()
    for token in re.findall(r"(?<![0-9A-Za-z_])(?:\$|0x)?([0-9A-Fa-f]{4,6})(?![0-9A-Za-z_])", text):
        found.add(int(token, 16))
    return found


def callee_known(target: int, annotated: dict[tuple[int, int], Routine], text: str) -> bool:
    if any(start <= target <= end for start, end in annotated):
        return True
    label = f"{target >> 16:02X}:{target & 0xFFFF:04X}"
    return label in text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--filter", default="", help="only Rust files whose name contains this")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--all", action="store_true", help="also list known addresses")
    args = parser.parse_args()

    rom = args.rom.read_bytes()
    routines = annotated_routines()
    texts = {path: path.read_text() for path in NATIVE.rglob("*.rs")}
    texts.update({path: path.read_text() for path in ORACLE.rglob("*.rs") if "sf2" in path.name})
    everywhere = set().union(*(mentions(text) for text in texts.values()))
    all_text = "\n".join(texts.values())

    xrefs = xref_index(rom)
    report = []
    for routine in sorted(routines.values(), key=lambda r: r.start):
        if args.filter and not any(args.filter in path.name for path in routine.files):
            continue
        decode(rom, routine)
        local = set().union(*(mentions(texts[path]) for path in routine.files))

        def status(address: int, tags: set[str]) -> str:
            kind = next(iter(tags)).split("/")[0]
            wide = any(tag.endswith("/16") for tag in tags)
            candidates = {address, address & 0xFFFF} | ({address + 1} if wide else set())
            if candidates & local:
                return "local"
            if candidates & everywhere:
                return "known"
            # High byte of a documented word, or a later word of a documented
            # vector (three words): plausible, but not proven, coverage.
            low_byte_used = address - 1 in routine.reads or address - 1 in routine.writes
            if address - 1 in everywhere and (address in routine.word_high_bytes or low_byte_used):
                return "adjacent"
            if kind == "player" and any(address - delta in everywhere for delta in range(1, 6)):
                return "adjacent"
            return "unknown"

        def priority(address: int) -> str:
            first = routine.first.get(address)
            if first == "read":
                return "HIGH live-in"
            if first == "read-after-call":
                return "MED live-in after call"
            if address in routine.reads or address in routine.word_high_bytes:
                return "LOW scratch"
            return "MED write-only"

        entry = {
            "routine": routine.label(),
            "priority": {f"{a:04X}": priority(a) for a in set(routine.reads) | set(routine.writes)},
            "files": sorted(str(path.relative_to(REPO)) for path in routine.files),
            "note": routine.note,
            "writes": {f"{a:04X}": [status(a, t), sorted(t)] for a, t in sorted(routine.writes.items())},
            "reads": {f"{a:04X}": [status(a, t), sorted(t)] for a, t in sorted(routine.reads.items())},
            "unported_calls": [f"{t >> 16:02X}:{t & 0xFFFF:04X}" for t in sorted(routine.calls)
                               if not callee_known(t, routines, all_text)],
        }
        report.append(entry)

    if args.json:
        print(json.dumps(report, indent=1))
        return 0
    totals = defaultdict(int)
    for entry in report:
        lines = []
        for access in ("writes", "reads"):
            for address, (state, tags) in entry[access].items():
                totals[f"{access}-{state}"] += 1
                if state == "unknown" or args.all:
                    sites = xrefs.get(int(address, 16), [])
                    readers = sorted({site for site, mnemonic in sites if mnemonic in READS})
                    shown = " ".join(readers[:8]) + (" ..." if len(readers) > 8 else "")
                    lines.append(f"  {access[:-1]:5} {address:>6} {state:7} {entry['priority'][address]:22} {','.join(tags):16} "
                                 f"rom-readers={len(readers)}: {shown}")
        if entry["unported_calls"]:
            lines.append(f"  calls unported: {' '.join(entry['unported_calls'])}")
        if entry["note"]:
            lines.append(f"  note: {entry['note']}")
        if lines:
            print(f"{entry['routine']}  {' '.join(Path(f).name for f in entry['files'])}")
            print("\n".join(lines))
    print(f"\n{len(report)} routines; " + ", ".join(f"{k}={v}" for k, v in sorted(totals.items())))
    print("Suspects only: confirm each with an original-code test before changing the port.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
