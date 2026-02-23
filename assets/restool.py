#!/usr/bin/env python3
"""
Star Fox ROM Data Extractor

Extracts raw data from a Star Fox SNES ROM at known addresses.
Usage: python3 restool.py "Star Fox (USA) (Rev 2).sfc" [output_dir]
"""

import sys
import os
import struct

# Star Fox (USA) Rev 2 ROM layout
# ROM is HiROM mapped: bank $00-$3F at $8000-$FFFF
# Shape data, map data, sound data at known offsets

ROM_SIZE = 1048576  # 1MB (8Mbit)

# Known data regions in the ROM
# These offsets are for the headerless ROM
DATA_REGIONS = {
    # Sound data (SPC700 driver + BRR samples)
    "snd_driver": (0x00F800, 0x010000),    # SPC700 driver code
    "snd_samples": (0x040000, 0x060000),   # BRR sample data

    # Shape/model data
    "shapes": (0x060000, 0x090000),         # 3D shape definitions

    # Map/level data
    "maps": (0x090000, 0x0C0000),           # Level script data

    # Graphics (tiles, sprites)
    "gfx_title": (0x010000, 0x018000),     # Title screen graphics
    "gfx_sprites": (0x018000, 0x020000),   # Sprite tiles
    "gfx_font": (0x020000, 0x022000),      # Font data

    # Palette data
    "palettes": (0x0C0000, 0x0C2000),      # Color palettes

    # Lookup tables (sin/cos, etc.)
    "tables": (0x0C2000, 0x0C4000),        # Math tables
}


def extract_rom(rom_path, output_dir):
    """Extract all known data regions from the ROM."""
    with open(rom_path, "rb") as f:
        rom = f.read()

    # Check for SMC header (512 bytes)
    has_header = (len(rom) % 1024) == 512
    if has_header:
        print(f"SMC header detected, stripping {512} bytes")
        rom = rom[512:]

    print(f"ROM size: {len(rom)} bytes ({len(rom) // 1024}KB)")

    if len(rom) != ROM_SIZE:
        print(f"Warning: expected {ROM_SIZE} bytes, got {len(rom)}")

    os.makedirs(output_dir, exist_ok=True)

    for name, (start, end) in DATA_REGIONS.items():
        if start >= len(rom):
            print(f"  SKIP {name}: offset {start:#x} beyond ROM size")
            continue

        actual_end = min(end, len(rom))
        data = rom[start:actual_end]
        out_path = os.path.join(output_dir, f"{name}.bin")

        with open(out_path, "wb") as f:
            f.write(data)

        print(f"  {name}: {start:#08x}-{actual_end:#08x} ({len(data)} bytes) -> {out_path}")

    # Also dump the full ROM info
    # Check SNES header at $FFC0 (HiROM)
    if len(rom) >= 0xFFE0:
        title = rom[0xFFC0:0xFFD5].decode("ascii", errors="replace").strip()
        rom_makeup = rom[0xFFD5]
        rom_type = rom[0xFFD6]
        rom_size_byte = rom[0xFFD7]
        sram_size_byte = rom[0xFFD8]
        print(f"\nROM Header:")
        print(f"  Title: {title}")
        print(f"  Makeup: {rom_makeup:#04x} ({'HiROM' if rom_makeup & 1 else 'LoROM'})")
        print(f"  Type: {rom_type:#04x}")
        print(f"  ROM size: {(1 << rom_size_byte)}KB")
        print(f"  SRAM size: {(1 << sram_size_byte) if sram_size_byte else 0}KB")

    print(f"\nExtracted {len(DATA_REGIONS)} regions to {output_dir}/")


def dump_region(rom_path, offset, size, output_path):
    """Dump a specific ROM region."""
    with open(rom_path, "rb") as f:
        rom = f.read()

    has_header = (len(rom) % 1024) == 512
    if has_header:
        rom = rom[512:]

    data = rom[offset : offset + size]
    with open(output_path, "wb") as f:
        f.write(data)
    print(f"Dumped {len(data)} bytes from {offset:#x} to {output_path}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python3 restool.py <rom_file> [output_dir]")
        print("       python3 restool.py <rom_file> --dump <offset> <size> <output>")
        sys.exit(1)

    rom_path = sys.argv[1]
    if not os.path.exists(rom_path):
        print(f"Error: ROM file not found: {rom_path}")
        sys.exit(1)

    if len(sys.argv) >= 5 and sys.argv[2] == "--dump":
        offset = int(sys.argv[3], 0)
        size = int(sys.argv[4], 0)
        output = sys.argv[5] if len(sys.argv) > 5 else "dump.bin"
        dump_region(rom_path, offset, size, output)
    else:
        output_dir = sys.argv[2] if len(sys.argv) > 2 else "data"
        extract_rom(rom_path, output_dir)
