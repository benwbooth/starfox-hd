#!/usr/bin/env python3
"""Extract and decode the retail SF2 audio upload files.

The broad terminator-delimited catalog begins at 0xCBE1E, but that address is
not the reset uploader's driver pointer.  Reset passes CPU $1A:8000 (file
0xD0000), an embedded upload-file boundary inside the first broad catalog
region.  Both forms use the SF1-compatible
[len:2 LE][dest:2 LE][data...]...[00 00][exec:2 LE] protocol.

Emits:
  data/sf2/snd/SF2DRIVER.BIN  -- exact reset-path driver upload file.
  data/sf2/snd/SF2SND##.BIN   -- broad catalog regions used to resolve the
                                 host's later upload pointers.
  rust/sf2-data/src/audio.rs  -- blob manifest (id, file, rom offset, exec,
                                 per-block dest/size).
"""

from __future__ import annotations

import os

from rom import (AUTOGEN_HEADER, DATA_DIR, RUST_SRC, load_rom, u16)

CHAIN_START = 0xCBE1E
DRIVER_ENTRY = 0x0400
# The reset path passes CPU $1A:8000 to the uploader. This is an embedded
# self-describing subchain inside the first broad terminator-delimited region;
# beginning at CHAIN_START would upload two unrelated preceding blocks.
DRIVER_START = 0xD0000
DRIVER_FILE = "SF2DRIVER.BIN"
MODE_TABLE = 0x1B873
MODE_COUNT = 16
PROGRAM_TABLE = 0x1E495
CONDITIONAL_OFFSET_TABLE = 0x1E724
CONDITIONAL_PROGRAM_TABLE = 0x1E738
CONDITIONAL_PROGRAM_COUNT = 10
PILOT_BLOB_TABLE = 0x1E376
PILOT_BLOB_COUNT = 7
BASE_BOOT_BLOB_IDS = (3, 4)


def parse_chain(d: bytes):
    """Walk the [len][dest]...[00 00][exec] blob chain from CHAIN_START.

    Returns a list of blobs: dicts with start, end, exec, blocks[(size,dest)].
    Stops when the next word is no longer a plausible block header (the region
    after the last blob is 0xFF ROM padding).
    """
    blobs = []
    o = CHAIN_START
    cur_start = o
    blocks = []
    while o + 4 <= len(d):
        count = u16(d, o)
        dest = u16(d, o + 2)
        if count == 0:
            # terminator: 'dest' is the per-file exec addr (always $0400 here).
            blobs.append({
                "start": cur_start,
                "end": o + 4,
                "exec": dest,
                "blocks": blocks,
            })
            o += 4
            cur_start = o
            blocks = []
            # End of chain: padding (0xFF) makes count == 0xFFFF and overflow.
            if o + 4 > len(d):
                break
            nxt = u16(d, o)
            if nxt == 0xFFFF or o + 4 + nxt > len(d):
                break
            continue
        if o + 4 + count > len(d):
            break
        blocks.append((count, dest))
        o += 4 + count
    return blobs


def parse_blob(d: bytes, start: int):
    """Parse one self-describing upload blob beginning at an exact pointer."""
    blocks = []
    o = start
    while o + 4 <= len(d):
        count = u16(d, o)
        dest = u16(d, o + 2)
        if count == 0:
            return {"start": start, "end": o + 4, "exec": dest, "blocks": blocks}
        if o + 4 + count > len(d):
            raise ValueError(f"audio blob at 0x{start:06X} overflows the ROM")
        blocks.append((count, dest))
        o += 4 + count
    raise ValueError(f"audio blob at 0x{start:06X} has no terminator")


def file_to_cpu(offset: int) -> int:
    return ((offset // 0x8000) << 16) | 0x8000 | (offset % 0x8000)


def pointer_at(d: bytes, offset: int) -> int:
    return u16(d, offset) | (d[offset + 2] << 16)


def blob_id_by_pointer(manifest, pointer: int) -> int:
    ids = {file_to_cpu(blob["off"]): blob["id"] for blob in manifest}
    if pointer not in ids:
        raise ValueError(f"audio pointer ${pointer:06X} is not a blob start")
    return ids[pointer]


def parse_program(d: bytes, manifest, record_offset: int):
    start = PROGRAM_TABLE + record_offset
    count = d[start]
    control = d[start + 1]
    blobs = [
        blob_id_by_pointer(manifest, pointer_at(d, start + 2 + index * 3))
        for index in range(count)
    ]
    return {
        "record_offset": record_offset,
        "preload_command": control & 0x0F,
        "start_cue": ((control >> 4) + 0xF9) & 0xFF,
        "blobs": blobs,
    }


def extract_oracle_catalog(d: bytes, manifest):
    programs = []
    record_offset = 0
    while PROGRAM_TABLE + record_offset < CONDITIONAL_OFFSET_TABLE:
        program = parse_program(d, manifest, record_offset)
        programs.append(program)
        record_offset += 2 + len(program["blobs"]) * 3
    if PROGRAM_TABLE + record_offset != CONDITIONAL_OFFSET_TABLE:
        raise ValueError("audio program table does not end at its offset table")

    program_index_by_offset = {
        program["record_offset"]: index for index, program in enumerate(programs)
    }
    source_mode_program_indices = []
    for mode in range(MODE_COUNT):
        entry = MODE_TABLE + mode * 4
        offset = u16(d, entry + 2)
        source_mode_program_indices.append(program_index_by_offset[offset])

    conditional = []
    for index in range(CONDITIONAL_PROGRAM_COUNT):
        record = CONDITIONAL_PROGRAM_TABLE + u16(d, CONDITIONAL_OFFSET_TABLE + index * 2)
        count = d[record]
        conditional.append([
            blob_id_by_pointer(manifest, pointer_at(d, record + 1 + item * 3))
            for item in range(count)
        ])

    pilots = [
        blob_id_by_pointer(manifest, pointer_at(d, PILOT_BLOB_TABLE + index * 3))
        for index in range(PILOT_BLOB_COUNT)
    ]
    emit_oracle_catalog(programs, source_mode_program_indices, conditional, pilots)


def extract(d: bytes):
    blobs = parse_chain(d)
    driver = parse_blob(d, DRIVER_START)
    if driver["exec"] != DRIVER_ENTRY:
        raise ValueError("embedded SF2 driver has an unexpected entry point")
    snd_dir = os.path.join(DATA_DIR, "snd")
    os.makedirs(snd_dir, exist_ok=True)
    with open(os.path.join(snd_dir, DRIVER_FILE), "wb") as f:
        f.write(d[driver["start"]:driver["end"]])

    manifest = []
    for i, b in enumerate(blobs):
        fname = f"SF2SND{i:02d}.BIN"
        raw = d[b["start"]:b["end"]]
        with open(os.path.join(snd_dir, fname), "wb") as f:
            f.write(raw)
        contains_driver_code = any(
            dest == DRIVER_ENTRY for (_sz, dest) in b["blocks"]
        )
        manifest.append({
            "id": i,
            "file": fname,
            "off": b["start"],
            "end": b["end"],
            "exec": b["exec"],
            "contains_driver_code": contains_driver_code,
            "blocks": b["blocks"],
        })

    emit_rust(manifest, blobs[-1]["end"], driver)
    extract_oracle_catalog(d, manifest)
    return manifest


def emit_rust(manifest, chain_end, driver):
    L = []
    L.append(AUTOGEN_HEADER.format(tool="extract_audio.py"))
    L.append("//! SF2 SPC audio upload-file manifest.")
    L.append("//!")
    L.append("//! `AUDIO_BLOBS` describes the broad terminator-delimited catalog")
    L.append("//! beginning at `AUDIO_CHAIN_START`. The reset path instead passes")
    L.append("//! `DRIVER_UPLOAD_START`, an embedded boundary within catalog blob 0.")
    L.append("//! All exact host-selected files use the SF1-compatible repeated")
    L.append("//! `[len:2 LE][dest:2 LE][data..]` blocks, `[00 00][exec:2 LE]`")
    L.append("//! terminator, then transfer to the encoded entry point.")
    L.append("")
    L.append("/// One `[len][dest]` upload block inside a blob.")
    L.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
    L.append("pub struct AudioBlock {")
    L.append("    /// SPC ARAM destination address.")
    L.append("    pub dest: u16,")
    L.append("    /// Payload byte count.")
    L.append("    pub size: u16,")
    L.append("}")
    L.append("")
    L.append("/// One self-describing upload blob (one SF2SND##.BIN file).")
    L.append("#[derive(Debug, Clone, Copy)]")
    L.append("pub struct AudioBlob {")
    L.append("    pub id: u16,")
    L.append("    /// Basename under `data/sf2/snd/`.")
    L.append("    pub file: &'static str,")
    L.append("    /// File offset of the blob's first block in the retail ROM.")
    L.append("    pub rom_off: u32,")
    L.append("    /// File offset just past the blob's terminator.")
    L.append("    pub rom_end: u32,")
    L.append("    /// Per-file exec word (the driver entry, $0400).")
    L.append("    pub exec: u16,")
    L.append("    /// Whether this broad region contains an upload to the driver entry.")
    L.append("    pub contains_driver_code: bool,")
    L.append("    pub blocks: &'static [AudioBlock],")
    L.append("}")
    L.append("")
    L.append(f"pub const AUDIO_CHAIN_START: u32 = 0x{CHAIN_START:05X};")
    L.append(f"pub const AUDIO_CHAIN_END: u32 = 0x{chain_end:05X};")
    L.append(f"pub const SPC_DRIVER_ENTRY: u16 = 0x{DRIVER_ENTRY:04X};")
    L.append(f"pub const DRIVER_UPLOAD_FILE: &str = \"{DRIVER_FILE}\";")
    L.append(f"pub const DRIVER_UPLOAD_START: u32 = 0x{driver['start']:06X};")
    L.append(f"pub const DRIVER_UPLOAD_END: u32 = 0x{driver['end']:06X};")
    L.append(f"pub const AUDIO_BLOB_COUNT: usize = {len(manifest)};")
    L.append("")
    # Per-blob block slices.
    for m in manifest:
        blks = ", ".join(
            f"AudioBlock {{ dest: 0x{dest:04X}, size: {sz} }}"
            for (sz, dest) in m["blocks"])
        L.append(f"static BLOCKS_{m['id']:02}: [AudioBlock; {len(m['blocks'])}] = [{blks}];")
    L.append("")
    L.append("pub static AUDIO_BLOBS: [AudioBlob; AUDIO_BLOB_COUNT] = [")
    for m in manifest:
        L.append(
            f"    AudioBlob {{ id: {m['id']}, file: \"{m['file']}\", "
            f"rom_off: 0x{m['off']:06X}, rom_end: 0x{m['end']:06X}, "
            f"exec: 0x{m['exec']:04X}, "
            f"contains_driver_code: {str(m['contains_driver_code']).lower()}, "
            f"blocks: &BLOCKS_{m['id']:02} }},")
    L.append("];")
    L.append("")
    with open(os.path.join(RUST_SRC, "audio.rs"), "w") as f:
        f.write("\n".join(L))
    print(f"  audio.rs: {len(manifest)} blobs, chain 0x{CHAIN_START:05X}-0x{chain_end:05X}")


def emit_oracle_catalog(programs, source_mode_program_indices, conditional, pilots):
    lines = []
    lines.append(AUTOGEN_HEADER.format(tool="extract_audio.py"))
    lines.append("//! Verification-only decoding of the retail SF2 host audio tables.")
    lines.append("")
    lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
    lines.append("pub struct UploadProgram {")
    lines.append("    pub source_record_offset: u16,")
    lines.append("    pub preload_command: Option<u8>,")
    lines.append("    pub start_cue: u8,")
    lines.append("    pub blob_ids: &'static [u16],")
    lines.append("}")
    lines.append("")
    lines.append(f"pub const SOURCE_MODE_COUNT: usize = {MODE_COUNT};")
    lines.append(f"pub const UPLOAD_PROGRAM_COUNT: usize = {len(programs)};")
    lines.append(
        "pub const BASE_BOOT_BLOB_IDS: [u16; 2] = ["
        + ", ".join(str(value) for value in BASE_BOOT_BLOB_IDS)
        + "];"
    )
    lines.append("")
    for index, program in enumerate(programs):
        values = ", ".join(str(value) for value in program["blobs"])
        lines.append(
            f"static PROGRAM_{index:02}_BLOBS: [u16; {len(program['blobs'])}] = [{values}];"
        )
    lines.append("")
    lines.append("pub static UPLOAD_PROGRAMS: [UploadProgram; UPLOAD_PROGRAM_COUNT] = [")
    for index, program in enumerate(programs):
        preload = (
            "None"
            if program["preload_command"] == 0
            else f"Some(0x{program['preload_command']:02X})"
        )
        lines.append(
            "    UploadProgram { "
            f"source_record_offset: 0x{program['record_offset']:03X}, "
            f"preload_command: {preload}, "
            f"start_cue: 0x{program['start_cue']:02X}, "
            f"blob_ids: &PROGRAM_{index:02}_BLOBS "
            "},"
        )
    lines.append("];")
    lines.append("")
    lines.append(
        f"pub const SOURCE_MODE_PROGRAM_INDEX: [usize; {len(source_mode_program_indices)}] = ["
        + ", ".join(str(value) for value in source_mode_program_indices)
        + "];"
    )
    lines.append("")
    for index, values in enumerate(conditional):
        joined = ", ".join(str(value) for value in values)
        lines.append(
            f"static CONDITIONAL_{index:02}_BLOBS: [u16; {len(values)}] = [{joined}];"
        )
    lines.append("")
    lines.append(
        f"pub static CONDITIONAL_BLOB_IDS: [&[u16]; {len(conditional)}] = ["
    )
    for index in range(len(conditional)):
        lines.append(f"    &CONDITIONAL_{index:02}_BLOBS,")
    lines.append("];")
    lines.append("")
    lines.append(
        f"pub const PILOT_BLOB_IDS: [u16; {len(pilots)}] = ["
        + ", ".join(str(value) for value in pilots)
        + "];"
    )
    lines.append("")
    with open(os.path.join(RUST_SRC, "oracle_audio.rs"), "w") as f:
        f.write("\n".join(lines))
    print(f"  oracle_audio.rs: {len(programs)} mode programs")


if __name__ == "__main__":
    extract(load_rom())
