#!/usr/bin/env python3
"""SF2 audio bank extractor (SF2_RECON.md phase 1, task 1).

Parses the SF2 SPC upload chain from 0xCBE1E onward.  The block format is
byte-identical to SF1 (see src/audio/spc_boot.c ipl_upload_file and
rust/sf-audio/src/boot.rs Booter::upload_file): each blob is a chain of
[len:2 LE][dest:2 LE][data...] blocks terminated by [00 00][exec:2 LE].  After
a blob's blocks are uploaded, execution transfers to the driver entry ($0400).

Emits:
  data/sf2/snd/SF2SND##.BIN   -- one raw blob per file (chain incl. terminator),
                                 so sf-audio's Booter can upload it unchanged.
  rust/sf2-data/src/audio.rs  -- blob manifest (id, file, rom offset, exec,
                                 per-block dest/size).
"""

from __future__ import annotations

import os

from rom import (AUTOGEN_HEADER, DATA_DIR, RUST_SRC, load_rom, u16)

CHAIN_START = 0xCBE1E
DRIVER_ENTRY = 0x0400


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


def extract(d: bytes):
    blobs = parse_chain(d)
    snd_dir = os.path.join(DATA_DIR, "snd")
    os.makedirs(snd_dir, exist_ok=True)

    manifest = []
    for i, b in enumerate(blobs):
        fname = f"SF2SND{i:02d}.BIN"
        raw = d[b["start"]:b["end"]]
        with open(os.path.join(snd_dir, fname), "wb") as f:
            f.write(raw)
        # driver blob = blob 0, which uploads the driver code to $0400.
        is_driver = any(dest == DRIVER_ENTRY for (_sz, dest) in b["blocks"])
        manifest.append({
            "id": i,
            "file": fname,
            "off": b["start"],
            "end": b["end"],
            "exec": b["exec"],
            "is_driver": is_driver and i == 0,
            "blocks": b["blocks"],
        })

    emit_rust(manifest, blobs[-1]["end"])
    return manifest


def emit_rust(manifest, chain_end):
    L = []
    L.append(AUTOGEN_HEADER.format(tool="extract_audio.py"))
    L.append("//! SF2 SPC audio blob manifest (SF2_RECON.md 0xCBE1E chain).")
    L.append("//!")
    L.append("//! The upload-block format is byte-identical to SF1, so sf-audio's")
    L.append("//! `Booter` (rust/sf-audio/src/boot.rs) can upload each")
    L.append("//! `data/sf2/snd/SF2SND##.BIN` unchanged: repeated")
    L.append("//! `[len:2 LE][dest:2 LE][data..]` blocks, `[00 00][exec:2 LE]`")
    L.append("//! terminator, then jump to the driver entry ($0400).")
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
    L.append("    /// True for the driver blob (uploads code to $0400).")
    L.append("    pub is_driver: bool,")
    L.append("    pub blocks: &'static [AudioBlock],")
    L.append("}")
    L.append("")
    L.append(f"pub const AUDIO_CHAIN_START: u32 = 0x{CHAIN_START:05X};")
    L.append(f"pub const AUDIO_CHAIN_END: u32 = 0x{chain_end:05X};")
    L.append(f"pub const SPC_DRIVER_ENTRY: u16 = 0x{DRIVER_ENTRY:04X};")
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
            f"exec: 0x{m['exec']:04X}, is_driver: {str(m['is_driver']).lower()}, "
            f"blocks: &BLOCKS_{m['id']:02} }},")
    L.append("];")
    L.append("")
    with open(os.path.join(RUST_SRC, "audio.rs"), "w") as f:
        f.write("\n".join(L))
    print(f"  audio.rs: {len(manifest)} blobs, chain 0x{CHAIN_START:05X}-0x{chain_end:05X}")


if __name__ == "__main__":
    extract(load_rom())
