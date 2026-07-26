#!/usr/bin/env python3
"""Extract the assembled SF1 path VM catalog and generate Rust metadata.

The reference assembler places PATHDATA.ASM, DPATHDAT.ASM, and KPATHDAT.ASM
in one contiguous `paths` blob.  The original VM indexes that blob using
16-bit offsets relative to `paths`; START_PATH and every branch/spawn macro
emit those relative offsets.  The extraction therefore keeps the section at
offset zero, exactly as the original VM sees it.

The ROM is user-owned and remains under data/ (gitignored).  Only symbol-
derived offsets/addresses and a checksum are generated into Rust source.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ROM = ROOT / "rust" / "sf-oracle" / "data" / "sf.sfc"
DEFAULT_SYMBOLS = ROOT / "reference" / "ultrastarfox" / "SYMBOLS.TXT"
IDS_RS = ROOT / "rust" / "sf-path" / "src" / "ids.rs"
ISTRATS_ASM = ROOT / "reference" / "ultrastarfox" / "SF" / "STRAT" / "ISTRATS.ASM"
OUT_BIN = ROOT / "data" / "path_catalog.bin"
OUT_RS = ROOT / "rust" / "sf-path" / "src" / "rom_catalog_data.rs"

CATALOG_START_SYMBOL = "PATHS"
# First routine after KPATHDAT's final shared path subroutine.
CATALOG_END_SYMBOL = "D2COPYPOS_YX"
PATH_MISSING = 0xFFFF


def parse_symbols(path: Path) -> dict[str, int]:
    symbols: dict[str, int] = {}
    for line in path.read_text(encoding="latin1", errors="ignore").splitlines():
        match = re.fullmatch(r"(\S+)\s+\$([0-9A-Fa-f]+)", line.strip())
        if match:
            symbols[match.group(1).upper()] = int(match.group(2), 16)
    return symbols


def lorom_offset(address: int) -> int:
    bank = (address >> 16) & 0x7F
    word = address & 0xFFFF
    if word < 0x8000:
        raise ValueError(f"not a LoROM address: ${address:06X}")
    return bank * 0x8000 + (word - 0x8000)


def fnv1a(data: bytes) -> int:
    value = 0x811C9DC5
    for byte in data:
        value = ((value ^ byte) * 0x01000193) & 0xFFFFFFFF
    return value


def parse_path_ids() -> list[tuple[str, int]]:
    text = IDS_RS.read_text()
    rows = [
        (match.group(1), int(match.group(2)))
        for match in re.finditer(
            r"pub const PATH_ID_([A-Z0-9_]+): u16 = (\d+);", text
        )
    ]
    if not rows:
        raise RuntimeError("no PATH_ID_* constants found")
    return rows


def path_symbol(const_name: str) -> str:
    # START_PATH foo defines PATH_FOO as an offset relative to `paths`.
    # PCOINEXPLODE is a shared path subroutine rather than a START_PATH, so
    # only its absolute assembler label exists.
    if const_name == "PCOINEXPLODE":
        return "PCOINEXPLODE"
    return f"PATH_{const_name}"


def parse_istrat_names() -> list[str]:
    names: list[str] = []
    for raw in ISTRATS_ASM.read_text(encoding="latin1", errors="ignore").splitlines():
        line = raw.split(";", 1)[0]
        match = re.match(r"^\s*def_istrat\s+([A-Za-z0-9_]+)", line, re.IGNORECASE)
        # The source defines the `def_istrat MACRO` itself before invoking
        # it.  That definition is not an ISTRATS table row: `ci` remains zero
        # until the first real invocation (`player`).  Counting it shifts
        # every assembled strategy address by one and makes exact path
        # P_SETSTRAT operands resolve to the following strategy.
        if match and match.group(1).lower() != "macro":
            names.append(match.group(1).upper())
    if len(names) > 256:
        raise RuntimeError(f"istrat table has {len(names)} rows, expected <= 256")
    return names


def generate_metadata(
    symbols: dict[str, int],
    start: int,
    end: int,
    section_hash: int,
) -> str:
    assignments: list[str] = []
    mapped = 0
    for const_name, path_id in parse_path_ids():
        symbol = path_symbol(const_name)
        value = symbols.get(symbol)
        if value is None:
            if const_name != "CUTCREDS":
                raise RuntimeError(f"missing path symbol {symbol} for id {path_id}")
            continue
        offset = value - start if const_name == "PCOINEXPLODE" else value
        if not (0 <= offset < end - start):
            raise RuntimeError(f"path symbol {symbol} outside catalog: ${value:06X}")
        assignments.append(
            f"    offsets[PATH_ID_{const_name} as usize] = 0x{offset:04X};"
        )
        mapped += 1

    # These IPs are the P_START65816 opcode bytes in the assembled catalog.
    # The offsets are stable outputs of the checked reference build and are
    # validated against opcode $71 before the metadata is written.
    inline = {
        "tow_0_set_expstrat": 0x04DF15 - start,
        "robexplode_nopolyexp": 0x04E8E8 - start,
        "dsmoke_init_colanim": 0x04DF80 - start,
        "dsmoke_add_colanim": 0x04DF90 - start,
        "pbooston_makeengine": symbols["PBOOSTON"] - start,
        "pboostcode_updateengine": symbols["PBOOSTCODE"] - start,
        "makepollen": 0x04A446 - start,
        "e_big_bird_touch": 0x04A6B7 - start,
        # DPATHDAT dintro1: RELTOPLAYER(1) + TRIGGER(4) + TRAIL(2), then
        # the special signed half/clamp X chase P_START65816.
        "dintro1_zoom_to_centre": symbols["PATH_DINTRO1"] + 7,
        # Trigger target `.keep4000`: pin the text four thousand source units
        # ahead of the live view before returning to the path dispatcher.
        "dintro1_keep_distance": symbols["PATH_DINTRO1"] + 0x60,
        "checkifend1": symbols["PSTAGE1"] + 21 - start,
        "checkifend2": symbols["PSTAGE2"] + 21 - start,
        "checkifend3": symbols["PSTAGE3"] + 21 - start,
        "checkifend4": symbols["PSTAGE4"] + 21 - start,
        "checkifend5": symbols["PSTAGE5"] + 21 - start,
        "checkifend6": symbols["PSTAGE6"] + 21 - start,
        "checkifend7": symbols["PSTAGE7"] + 21 - start,
    }

    istrat_names = parse_istrat_names()
    istrat_addrs = [symbols.get(f"{name}_ISTRAT", 0) for name in istrat_names]
    istrat_addrs.extend([0] * (256 - len(istrat_addrs)))
    istrat_rows = []
    for i in range(0, 256, 4):
        row = ", ".join(f"0x{value:06X}" for value in istrat_addrs[i : i + 4])
        istrat_rows.append(f"    {row},")

    inline_rows = "\n".join(
        f"        {name}: 0x{value:04X}," for name, value in inline.items()
    )
    continuation_by_name = {
        "tow_0_set_expstrat": 0x3B0D,
        "robexplode_nopolyexp": 0x44D5,
        "dsmoke_init_colanim": 0x3B6E,
        "dsmoke_add_colanim": 0x3B90,
        "pbooston_makeengine": 0x473D,
        "pboostcode_updateengine": 0x474D,
        "makepollen": 0x0031,
        "e_big_bird_touch": 0x02C0,
        "dintro1_zoom_to_centre": symbols["PATH_DINTRO1"] + 0x41,
        "dintro1_keep_distance": symbols["PATH_DINTRO1"] + 0x71,
        "checkifend1": 0x48CB,
        "checkifend2": 0x48FD,
        "checkifend3": 0x492F,
        "checkifend4": 0x4961,
        "checkifend5": 0x4993,
        "checkifend6": 0x49C5,
        "checkifend7": 0x49F7,
    }
    if continuation_by_name.keys() != inline.keys():
        raise RuntimeError("inline callback continuation metadata is incomplete")
    continuation_rows = "\n".join(
        f"        (0x{inline[name]:04X}, 0x{continuation:04X}),"
        for name, continuation in continuation_by_name.items()
    )
    assignment_text = "\n".join(assignments)
    istrat_text = "\n".join(istrat_rows)
    return f'''//! Generated by `tools/extract_sf1_paths.py`; do not hand-edit.
//!
//! This file contains only symbol-derived metadata. The assembled bytecode
//! remains in the user-owned, gitignored `data/path_catalog.bin`.

use crate::builder::PATH_MISSING_OFFSET;
use crate::ids::*;
use crate::literals::InlineIps;

pub const ROM_PATH_CATALOG_SIZE: usize = 0x{end - start:X};
pub const ROM_PATH_CPU_BASE: usize = 0x{start & 0xFFFF:04X};
pub const ROM_PATH_CPU_END: usize = 0x{end & 0xFFFF:04X};
pub const ROM_PATH_SECTION_FNV1A: u32 = 0x{section_hash:08X};
pub const ROM_PATH_MAPPED_IDS: usize = {mapped};
pub const ROM_DINTRO1_LOOP_IP: u16 = 0x{symbols['PATH_DINTRO1'] + 0x3E:04X};
pub const ROM_DINTRO1_EXIT_IP: u16 = 0x{symbols['PATH_DINTRO1'] + 0x41:04X};

pub fn offsets() -> Vec<u16> {{
    let mut offsets = vec![PATH_MISSING_OFFSET; PATH_DATA_COUNT_LITERAL as usize];
{assignment_text}
    offsets
}}

pub const fn inline_ips() -> InlineIps {{
    InlineIps {{
{inline_rows}
    }}
}}

/// Generated native-action continuations. These replace runtime decoding of
/// the original inline instruction blobs.
pub const fn inline_continuations() -> [(u16, u16); {len(inline)}] {{
    [
{continuation_rows}
    ]
}}

/// Actual 24-bit strategy addresses emitted by P_SETSTRAT in the assembled
/// ROM, indexed by the canonical ISTRATS.ASM row.
pub const ROM_ISTRAT_ADDRS: [u32; 256] = [
{istrat_text}
];

pub const ROM_GATE3_ISTRAT_ADDR: u32 = 0x{symbols['GATE3_ISTRAT']:06X};
pub const ROM_TOW0EXPLODE_ISTRAT_ADDR: u32 = 0x{symbols['TOW0EXPLODE_ISTRAT']:06X};
'''


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--symbols", type=Path, default=DEFAULT_SYMBOLS)
    parser.add_argument("--out-bin", type=Path, default=OUT_BIN)
    parser.add_argument("--out-rs", type=Path, default=OUT_RS)
    args = parser.parse_args()

    symbols = parse_symbols(args.symbols)
    start = symbols[CATALOG_START_SYMBOL]
    end = symbols[CATALOG_END_SYMBOL]
    if start >> 16 != end >> 16:
        raise RuntimeError(f"path catalog crosses banks: ${start:06X}..${end:06X}")

    rom = args.rom.read_bytes()
    start_off = lorom_offset(start)
    end_off = lorom_offset(end)
    section = rom[start_off:end_off]
    expected_len = (end & 0xFFFF) - (start & 0xFFFF)
    if len(section) != expected_len:
        raise RuntimeError("truncated path section")
    if section[:3] != bytes((0x5B, 0xA6, 0x15)):
        raise RuntimeError("PATHS does not begin with e_gate bytecode")

    catalog = section

    metadata = generate_metadata(symbols, start, end, fnv1a(section))
    # Validate every registered inline IP against P_START65816 ($71).
    for match in re.finditer(r"(?:[a-z0-9_]+): 0x([0-9A-F]{4}),", metadata):
        ip = int(match.group(1), 16)
        if catalog[ip] != 0x71:
            raise RuntimeError(f"inline callback IP ${ip:04X} is ${catalog[ip]:02X}, not $71")

    args.out_bin.parent.mkdir(parents=True, exist_ok=True)
    args.out_rs.parent.mkdir(parents=True, exist_ok=True)
    args.out_bin.write_bytes(catalog)
    args.out_rs.write_text(metadata)
    print(
        f"wrote {args.out_bin} ({len(catalog)} bytes; section ${start:06X}..${end:06X})"
    )
    print(f"wrote {args.out_rs}")


if __name__ == "__main__":
    main()
