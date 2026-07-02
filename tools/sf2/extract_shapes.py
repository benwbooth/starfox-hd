#!/usr/bin/env python3
"""SF2 3D shape extractor (SF2_RECON.md phase 1, task 2) -- BEST EFFORT.

Applies the SF1 point/face byte-grammar (INC/SHMACS.INC, mirrored by
tools/shape_compiler.py) to SF2's shape banks 0x12-0x17:

  point block : 04 <count> <count x (x,y,z) signed bytes> 0C   (pointsb/pb/endpoints)
                08 <count> <count x (x,y,z) signed words> 0C    (pointsw)
  face record : <N> <vis> <col> <nx> <ny> <-nz> <idx x N>       N in 2..12
  face stream : face records terminated by FE (continue) / FF (quit)

FINDING (see the run report): this grammar does NOT resolve SF2's shapes.
Byte-scanning banks 0x12-0x17 yields only a handful of strict point-block
matches (mostly coincidental) and ZERO self-consistent face streams -- the
04/0C/14/FE byte densities do not line up at any consistent coordinate stride.
SF2 is a GSU-2 title; its 3D data is evidently reordered/re-encoded (and/or
compressed) relative to SF1, exactly the case the recon flags as needing a
live disassembly to pin the shapehdr/pointer layout.

So shape geometry extraction is DEFERRED to the disassembly-gated phase.  This
tool still emits the byte-coordinate point-block CANDIDATES it does find, as an
explicitly-unverified catalog (faces empty), so downstream code has the
sf-render-shaped struct layout in place and the count is inspectable.

Emits rust/sf2-data/src/shape_data.rs (mirrors rust/sf-render/src/shape_data.rs).
"""

from __future__ import annotations

import os

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom, s8, sw16

BANKS = {
    0x12: (0x90000, 0x98000),
    0x13: (0x98000, 0xA0000),
    0x14: (0xA0000, 0xA8000),
    0x15: (0xA8000, 0xB0000),
    0x16: (0xB0000, 0xB8000),
    0x17: (0xB8000, 0xC0000),
}


def parse_point_block(d, o, end):
    """Strict SF1 point block. Returns (op, verts, next_off) or None."""
    op = d[o]
    if op == 4:
        stride = 3
    elif op == 8:
        stride = 6
    else:
        return None
    cnt = d[o + 1]
    if not (2 <= cnt <= 48):
        return None
    p = o + 2
    verts = []
    for _ in range(cnt):
        if p + stride > end:
            return None
        if stride == 3:
            verts.append((s8(d[p]), s8(d[p + 1]), s8(d[p + 2])))
        else:
            verts.append((sw16(d, p), sw16(d, p + 2), sw16(d, p + 4)))
        p += stride
    if p >= end or d[p] != 0x0C:
        return None
    return (op, verts, p + 1)


def parse_face_stream(d, o, end, nverts, minf=1):
    """Strict SF1 face stream. Returns (faces, next_off) or None."""
    p = o
    if p < end and d[p] == 0x14:  # optional 'faces' marker
        p += 1
    faces = []
    while p < end:
        n = d[p]
        if n in (0xFE, 0xFF):
            return (faces, p + 1) if len(faces) >= minf else None
        if not (2 <= n <= 12):
            return None
        rl = 6 + n
        if p + rl > end:
            return None
        idx = [d[p + 6 + k] for k in range(n)]
        if any(v >= nverts for v in idx):
            return None
        faces.append((d[p + 2], idx))
        p += rl
        if len(faces) > 250:
            return None
    return None


def scan(d):
    """Return (byte_candidates, word_candidates, face_stream_count)."""
    byte_c, word_c = [], []
    face_streams = 0
    for bank, (s, e) in BANKS.items():
        o = s
        while o < e - 4:
            pb = parse_point_block(d, o, e)
            if pb:
                op, verts, nxt = pb
                (byte_c if op == 4 else word_c).append((bank, o, verts))
                o = nxt
                continue
            o += 1
        # independent face-stream census (relaxed index bound)
        o = s
        while o < e - 8:
            fs = parse_face_stream(d, o, e, 256, minf=3)
            if fs:
                face_streams += 1
                o = fs[1]
            else:
                o += 1
    return byte_c, word_c, face_streams


def emit_rust(byte_candidates, word_c, face_streams):
    # Only byte-coordinate candidates are emitted: op-8 word matches with
    # +-32000 coords are statistical noise, not meshes.
    cands = byte_candidates
    L = []
    L.append(AUTOGEN_HEADER.format(tool="extract_shapes.py"))
    L.append("//! SF2 3D shape data -- UNVERIFIED CANDIDATES ONLY.")
    L.append("//!")
    L.append("//! Struct layout mirrors `sf_render::shape_data`. HOWEVER: applying")
    L.append("//! SF1's point/face byte-grammar to SF2 banks 0x12-0x17 does NOT")
    L.append("//! recover SF2's shapes -- 0 self-consistent face streams parse and")
    L.append("//! the point-block matches below are largely coincidental. SF2's")
    L.append("//! GSU-2 3D data is reordered/re-encoded (likely compressed)")
    L.append("//! relative to SF1. Real shape extraction is DEFERRED to the")
    L.append("//! disassembly-gated phase (see docs/SF2_RECON.md sections 3 & 5).")
    L.append(f"//! Diagnostics: byte point-block candidates={len(cands)}, "
             f"word candidates={len(word_c)}, face streams={face_streams}.")
    L.append("")
    L.append("/// Mesh vertex, matching `sf_render::shape_data::ShapeVertex`.")
    L.append("#[derive(Debug, Clone, Copy, PartialEq)]")
    L.append("pub struct ShapeVertex {")
    L.append("    pub x: f32,")
    L.append("    pub y: f32,")
    L.append("    pub z: f32,")
    L.append("}")
    L.append("")
    L.append("/// Polygon face, matching `sf_render::shape_data::ShapeFace`.")
    L.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
    L.append("pub struct ShapeFace {")
    L.append("    pub vertex_indices: [u16; 12],")
    L.append("    pub num_verts: u8,")
    L.append("    pub color_index: u8,")
    L.append("}")
    L.append("")
    L.append("/// One candidate shape, matching `sf_render::shape_data::ShapeDataEntry`.")
    L.append("#[derive(Debug, Clone, Copy)]")
    L.append("pub struct ShapeDataEntry {")
    L.append("    pub shape_id: u16,")
    L.append("    /// Source ROM file offset of the point block (for later verification).")
    L.append("    pub rom_off: u32,")
    L.append("    pub vertices: &'static [ShapeVertex],")
    L.append("    pub faces: &'static [ShapeFace],")
    L.append("    pub name: &'static str,")
    L.append("}")
    L.append("")
    L.append("const fn v(x: f32, y: f32, z: f32) -> ShapeVertex { ShapeVertex { x, y, z } }")
    L.append("")
    for i, (bank, off, verts) in enumerate(cands):
        L.append(f"// Candidate {i}: bank ${bank:02X} @ 0x{off:06X}, {len(verts)} verts")
        L.append(f"static SHAPE_{i}_VERTS: [ShapeVertex; {len(verts)}] = [")
        for (x, y, z) in verts:
            # Y-negation to match sf-render (Star Fox Y-down -> OpenGL Y-up).
            ny = -y if y != 0 else 0
            L.append(f"    v({float(x):.1f}, {float(ny):.1f}, {float(z):.1f}),")
        L.append("];")
    L.append("")
    L.append("static NO_FACES: [ShapeFace; 0] = [];")
    L.append("")
    L.append(f"pub const SHAPE_DATA_COUNT: usize = {len(cands)};")
    L.append("pub static SHAPE_DATA: [ShapeDataEntry; SHAPE_DATA_COUNT] = [")
    for i, (bank, off, verts) in enumerate(cands):
        L.append(
            f"    ShapeDataEntry {{ shape_id: {i}, rom_off: 0x{off:06X}, "
            f"vertices: &SHAPE_{i}_VERTS, faces: &NO_FACES, "
            f'name: "sf2_cand_{i}" }},')
    L.append("];")
    L.append("")
    with open(os.path.join(RUST_SRC, "shape_data.rs"), "w") as f:
        f.write("\n".join(L))
    print(f"  shape_data.rs: {len(cands)} byte candidates (UNVERIFIED), "
          f"{len(word_c)} word candidates ignored, {face_streams} face streams")


def extract(d):
    byte_c, word_c, face_streams = scan(d)
    emit_rust(byte_c, word_c, face_streams)
    return byte_c, word_c, face_streams


if __name__ == "__main__":
    extract(load_rom())
