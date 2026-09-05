#!/usr/bin/env python3
"""Extract Star Fox 2's complete retail ShapeHdr/mesh table.

SF2 retains the original Argonaut shape bytecode exactly.  The previous
extractor searched for a single ``pointsb`` block followed immediately by
``endpoints``; real shapes chain point commands (including mirrored point
blocks) and put visibility/BSP bytecode between the header's face pointer and
the reachable face streams.  That incomplete scan therefore mislabeled real
geometry as statistical noise.

The retail facts used here are mechanically checkable:

* 577 packed, 28-byte ShapeHdr records fill ``$00:BC9C..$00:FB9C``.  Reset
  code begins at ``$00:FBB8``, the very next byte.
* ShapeHdr stores a 24-bit point pointer, an in-bank 16-bit face-program
  pointer, exact bounds/material/LOD metadata, and uses the SF1 layout.
* Point commands are the SHMACS.INC opcodes ``pointsb/pointsw``, their mirrored
  variants, ``frames``, ``jump``, ``vnormals``, and ``endpoints``.
* Face programs use ``vizis``, BSP branches, group tables, and ordinary
  ``faces`` records.  Extraction follows every BSP branch so every polygon is
  retained, independent of the current camera-facing decision.

Every emitted face index is checked against the decoded vertex array.  The
retail table currently produces 11,860 vertices and 10,524 faces with no
out-of-range indices.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
import os
import struct
import subprocess

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom


SHAPE_HEADER_START = 0xBC9C
SHAPE_HEADER_SIZE = 28
SHAPE_HEADER_COUNT = 577
SHAPE_HEADER_END = SHAPE_HEADER_START + (SHAPE_HEADER_COUNT - 1) * SHAPE_HEADER_SIZE
RESET_ENTRY = 0xFBB8

# Argonaut shape interpreter commands (reference/ultrastarfox/SF/INC/SHMACS.INC).
MVAL_ENDSHAPE = 0x00
MVAL_ROTPOINTS8 = 0x04
MVAL_ROTPOINTS16 = 0x08
MVAL_ENDPOINTS = 0x0C
MVAL_GROUPS = 0x10
MVAL_FACES = 0x14
MVAL_FRAMES = 0x1C
MVAL_JUMP = 0x20
MVAL_BSP = 0x28
MVAL_VIZIS = 0x30
MVAL_ROTPOINTSX16 = 0x34
MVAL_ROTPOINTSX8 = 0x38
MVAL_BSPINIT = 0x3C
MVAL_BSPEND = 0x40
MVAL_BSPE = 0x44
MVAL_QUIT = 0x48
MVAL_VNORMALS = 0x4C
MVAL_SPRITE = 0x50
MVAL_SPRITEVIS = 0x54
# SF2 shape command installs a clipping plane from two local-space points.
# Dispatch $01:8068 jumps to $01:F0AD; it consumes slot + six signed words.
MVAL_CLIP_PLANE = 0x68
CLIP_PLANE_RECORD_SIZE = 14
CLIP_PLANE_SLOT_COUNT = 8


class ShapeParseError(RuntimeError):
    pass


@dataclass(frozen=True)
class Vertex:
    x: int
    y: int
    z: int


@dataclass(frozen=True)
class Face:
    # Resolved retail `vizis` triangle. None is selector $FF, used for
    # deliberately two-sided faces and wireframe segments.
    visibility_vertices: tuple[int, int, int] | None
    color_index: int
    normal: tuple[int, int, int]
    vertex_indices: tuple[int, ...]


@dataclass(frozen=True)
class ClipPlane:
    slot: int
    origin: Vertex
    direction_point: Vertex


@dataclass(frozen=True)
class ShapeHeader:
    index: int
    address: int
    points_address: int
    faces_address: int
    sort_z: int
    shift: int
    radius: int
    xmax: int
    ymax: int
    zmax: int
    size: int
    color_table: int
    shadow: int
    simple1: int
    simple2: int
    simple3: int


@dataclass(frozen=True)
class Shape:
    header: ShapeHeader
    vertices: tuple[Vertex, ...]
    animation_frames: tuple[tuple[Vertex, ...], ...]
    faces: tuple[Face, ...]
    clipping_planes: tuple[ClipPlane, ...]


def s8(value: int) -> int:
    return value - 0x100 if value & 0x80 else value


def s16(data: bytes, offset: int) -> int:
    return struct.unpack_from("<h", data, offset)[0]


def u16(data: bytes, offset: int) -> int:
    return struct.unpack_from("<H", data, offset)[0]


def gsu_to_file(data: bytes, address: int) -> int:
    """Translate a GSU ROM address to this headerless ROM's file offset."""
    bank = (address >> 16) & 0xFF
    cpu_address = address & 0xFFFF
    if cpu_address < 0x8000:
        raise ShapeParseError(f"GSU data pointer ${address:06X} is below $8000")
    offset = bank * 0x8000 + (cpu_address & 0x7FFF)
    if offset >= len(data):
        raise ShapeParseError(f"GSU data pointer ${address:06X} is outside the ROM")
    return offset


def parse_headers(data: bytes) -> list[ShapeHeader]:
    if SHAPE_HEADER_END + SHAPE_HEADER_SIZE != RESET_ENTRY:
        raise AssertionError("ShapeHdr table must end immediately before reset")

    headers: list[ShapeHeader] = []
    for index in range(SHAPE_HEADER_COUNT):
        address = SHAPE_HEADER_START + index * SHAPE_HEADER_SIZE
        offset = address - 0x8000
        raw = data[offset : offset + SHAPE_HEADER_SIZE]
        if len(raw) != SHAPE_HEADER_SIZE:
            raise ShapeParseError(f"truncated ShapeHdr at ${address:04X}")

        points_address = raw[0] | (raw[1] << 8) | (raw[2] << 16)
        faces_pointer = u16(raw, 3)
        faces_address = ((points_address >> 16) << 16) | faces_pointer if faces_pointer else 0
        header = ShapeHeader(
            index=index,
            address=address,
            points_address=points_address,
            faces_address=faces_address,
            sort_z=s16(raw, 5),
            shift=raw[7],
            radius=u16(raw, 8),
            xmax=u16(raw, 10),
            ymax=u16(raw, 12),
            zmax=u16(raw, 14),
            size=u16(raw, 16),
            color_table=u16(raw, 18),
            shadow=u16(raw, 20),
            simple1=u16(raw, 22),
            simple2=u16(raw, 24),
            simple3=u16(raw, 26),
        )
        if header.shift > 15:
            raise ShapeParseError(f"ShapeHdr ${address:04X} has invalid shift {header.shift}")
        if points_address:
            gsu_to_file(data, points_address)
        if faces_address:
            gsu_to_file(data, faces_address)
        headers.append(header)
    return headers


def parse_vertex_stream(
        data: bytes,
        address: int,
        animation_frame: int,
) -> tuple[tuple[Vertex, ...], tuple[int, ...]]:
    if address == 0:
        return (), ()

    cursor = gsu_to_file(data, address)
    vertices: list[Vertex] = []
    frame_counts: list[int] = []
    visited: set[int] = set()
    for _ in range(4096):
        if cursor in visited:
            raise ShapeParseError(f"point-program loop from ${address:06X}")
        visited.add(cursor)
        opcode = data[cursor]

        if opcode in (MVAL_ROTPOINTS8, MVAL_ROTPOINTS16,
                      MVAL_ROTPOINTSX8, MVAL_ROTPOINTSX16):
            count = data[cursor + 1]
            cursor += 2
            words = opcode in (MVAL_ROTPOINTS16, MVAL_ROTPOINTSX16)
            mirrored = opcode in (MVAL_ROTPOINTSX8, MVAL_ROTPOINTSX16)
            for _ in range(count):
                if words:
                    x, y, z = struct.unpack_from("<hhh", data, cursor)
                    cursor += 6
                else:
                    x, y, z = (s8(value) for value in data[cursor : cursor + 3])
                    cursor += 3
                vertices.append(Vertex(x, y, z))
                if mirrored:
                    vertices.append(Vertex(-x, y, z))
            continue

        if opcode == MVAL_VNORMALS:
            count = data[cursor + 1]
            cursor += 2 + count * 3
            continue
        if opcode == MVAL_ENDPOINTS:
            return tuple(vertices), tuple(frame_counts)
        if opcode == MVAL_JUMP:
            cursor = cursor + 2 + s16(data, cursor + 1)
            continue
        if opcode == MVAL_FRAMES:
            frame_count = data[cursor + 1]
            if frame_count == 0:
                raise ShapeParseError(f"zero-frame point animation at ${address:06X}")
            frame_counts.append(frame_count)
            selected = animation_frame % frame_count
            table_entry = cursor + 2 + selected * 2
            # jumptab's relative word is target-*-1, so the runtime target is
            # word_offset + 1 + signed displacement.
            cursor = table_entry + 1 + s16(data, table_entry)
            continue

        raise ShapeParseError(
            f"unknown point opcode ${opcode:02X} at ROM file ${cursor:06X} "
            f"(root ${address:06X})"
        )
    raise ShapeParseError(f"point program from ${address:06X} exceeded step limit")


def parse_vertex_frames(
        data: bytes,
        address: int,
) -> tuple[tuple[Vertex, ...], ...]:
    """Decode the complete period of a retail point animation.

    An object has one animation counter. Every Frames opcode independently
    wraps that counter by its own table size, so the complete typed geometry
    period is the least common multiple of all reached table sizes.
    """
    first, counts = parse_vertex_stream(data, address, 0)
    if not counts:
        return (first,)

    period = math.lcm(*counts)
    while True:
        frames: list[tuple[Vertex, ...]] = []
        discovered = set(counts)
        for frame_index in range(period):
            vertices, reached_counts = parse_vertex_stream(data, address, frame_index)
            frames.append(vertices)
            discovered.update(reached_counts)
        complete_period = math.lcm(*discovered)
        if complete_period == period:
            break
        period = complete_period

    lengths = {len(vertices) for vertices in frames}
    if len(lengths) != 1:
        raise ShapeParseError(
            f"point animation at ${address:06X} changes vertex count: {sorted(lengths)}")
    return tuple(frames)


def _face_program_target(data: bytes, bank: int, pointer: int) -> int:
    return gsu_to_file(data, (bank << 16) | (pointer & 0xFFFF))


def parse_faces(data: bytes, address: int) -> tuple[tuple[Face, ...], tuple[ClipPlane, ...]]:
    if address == 0:
        return (), ()

    bank = (address >> 16) & 0xFF
    pending: list[
        tuple[int, tuple[tuple[int, int, int], ...] | None]
    ] = [(gsu_to_file(data, address), None)]
    visited: set[int] = set()
    faces: list[Face] = []
    clipping_planes: list[ClipPlane] = []

    while pending:
        cursor, visibility_tests = pending.pop()
        while cursor not in visited:
            if not 0 <= cursor < len(data):
                raise ShapeParseError(f"unterminated face program from ${address:06X}")
            if len(visited) >= 65536:
                raise ShapeParseError(f"face program from ${address:06X} exceeded step limit")
            visited.add(cursor)
            opcode = data[cursor]

            if opcode == MVAL_VIZIS:
                count = data[cursor + 1]
                visibility_tests = tuple(
                    tuple(data[cursor + 2 + index * 3 : cursor + 5 + index * 3])
                    for index in range(count)
                )
                cursor += 2 + count * 3
                continue
            if opcode == MVAL_BSPINIT:
                cursor += 1
                continue
            if opcode == MVAL_BSP:
                if visibility_tests is not None:
                    relative = s16(data, cursor + 2)
                    first = cursor + 3 + relative
                    second = cursor + 4 + s8(data[cursor + 4])
                    fallthrough = cursor + 5
                else:
                    relative = s16(data, cursor + 1)
                    first = cursor + 2 + relative
                    second = cursor + 3 + s8(data[cursor + 3])
                    fallthrough = cursor + 4
                # A BSP node orders three streams rather than choosing one:
                # the node's own coplanar face list and the two spatial
                # children.  The latter are encoded as the fall-through path
                # and the byte-relative path (normally small BSPE thunks).
                pending.append((first, visibility_tests))
                pending.append((second, visibility_tests))
                pending.append((fallthrough, visibility_tests))
                break
            if opcode == MVAL_BSPE:
                cursor = cursor + 2 + s16(data, cursor + 1)
                continue
            if opcode in (MVAL_BSPEND, MVAL_QUIT, MVAL_ENDSHAPE):
                break

            if opcode == MVAL_FACES:
                cursor += 1
                while data[cursor] not in (0xFE, 0xFF):
                    vertex_count = data[cursor]
                    if not 2 <= vertex_count <= 12:
                        raise ShapeParseError(
                            f"invalid face arity {vertex_count} at ROM file ${cursor:06X}"
                        )
                    visibility_selector = data[cursor + 1]
                    color_index = data[cursor + 2]
                    normal = tuple(s8(value) for value in data[cursor + 3 : cursor + 6])
                    indices = tuple(data[cursor + 6 : cursor + 6 + vertex_count])
                    if visibility_selector == 0xFF:
                        visibility_vertices = None
                    elif visibility_tests is None:
                        raise ShapeParseError(
                            f"face visibility {visibility_selector} at ROM file "
                            f"${cursor:06X} has no visibility table"
                        )
                    elif visibility_selector >= len(visibility_tests):
                        raise ShapeParseError(
                            f"face visibility {visibility_selector} at ROM file "
                            f"${cursor:06X} exceeds {len(visibility_tests)} tests"
                        )
                    else:
                        visibility_vertices = visibility_tests[visibility_selector]
                    faces.append(Face(
                        visibility_vertices,
                        color_index,
                        normal,
                        indices,
                    ))
                    cursor += 6 + vertex_count
                terminator = data[cursor]
                cursor += 1
                if terminator == 0xFF:
                    break
                continue

            if opcode == MVAL_GROUPS:
                count = data[cursor + 1]
                for index in range(count):
                    record = cursor + 2 + index * 3
                    pointer = u16(data, record + 1)
                    pending.append((_face_program_target(data, bank, pointer), visibility_tests))
                break
            if opcode == MVAL_SPRITE:
                cursor += 4
                continue
            if opcode == MVAL_SPRITEVIS:
                cursor += 5
                continue
            if opcode == MVAL_CLIP_PLANE:
                if cursor + CLIP_PLANE_RECORD_SIZE > len(data):
                    raise ShapeParseError(f"truncated clipping plane at ROM file ${cursor:06X}")
                slot = data[cursor + 1]
                if not 1 <= slot <= CLIP_PLANE_SLOT_COUNT:
                    raise ShapeParseError(f"invalid clipping-plane slot {slot} at ROM file ${cursor:06X}")
                clipping_planes.append(ClipPlane(
                    slot,
                    Vertex(*(s16(data, cursor + offset) for offset in (2, 4, 6))),
                    Vertex(*(s16(data, cursor + offset) for offset in (8, 10, 12))),
                ))
                # This is a continuing command. Stopping here discarded the
                # paired opposing plane and could conceal unknown tail code.
                cursor += CLIP_PLANE_RECORD_SIZE
                continue

            raise ShapeParseError(
                f"unknown face opcode ${opcode:02X} at ROM file ${cursor:06X} "
                f"(root ${address:06X})"
            )

    return tuple(faces), tuple(clipping_planes)


def extract_shapes(data: bytes) -> list[Shape]:
    if data[0x8068:0x806C] != bytes.fromhex("ffadf001"):
        raise ShapeParseError("clipping-plane dispatch signature mismatch")
    shapes: list[Shape] = []
    for header in parse_headers(data):
        vertex_frames = parse_vertex_frames(data, header.points_address)
        vertices = vertex_frames[0]
        animation_frames = vertex_frames if len(vertex_frames) > 1 else ()
        faces, clipping_planes = parse_faces(data, header.faces_address)
        for frame_index, frame in enumerate(vertex_frames):
            for face_index, face in enumerate(faces):
                for vertex_index in face.vertex_indices:
                    if vertex_index >= len(frame):
                        raise ShapeParseError(
                            f"ShapeHdr ${header.address:04X} frame {frame_index} face "
                            f"{face_index} references vertex {vertex_index}, but only "
                            f"{len(frame)} exist"
                        )
        shapes.append(Shape(header, vertices, animation_frames, faces, clipping_planes))
    return shapes


def emit_rust(shapes: list[Shape]) -> None:
    lines: list[str] = [AUTOGEN_HEADER.format(tool="extract_shapes.py")]
    lines.extend([
        "//! Complete Star Fox 2 retail ShapeHdr and polygon mesh table.",
        "//!",
        "//! Coordinates and normals retain their exact signed integer stream values;",
        "//! render-space scaling is `coordinate * (1 << shift)`. BSP extraction follows",
        "//! both camera-dependent branches, producing the complete face union while",
        "//! resolving every face selector to its exact camera-facing test triangle.",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct ShapeVertex {",
        "    pub x: i16,",
        "    pub y: i16,",
        "    pub z: i16,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct ShapeFace {",
        "    pub vertex_indices: [u16; 12],",
        "    pub num_verts: u8,",
        "    pub color_index: u8,",
        "    /// Resolved retail `vizis` triangle; `None` is a two-sided face.",
        "    pub visibility_vertices: Option<[u16; 3]>,",
        "    pub normal: [i8; 3],",
        "}",
        "",
        "/// Authored clipping-plane command. Both points are in local shape",
        "/// coordinates; direction_point is an endpoint, not a unit normal.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct ShapeClipPlane {",
        "    pub slot: u8,",
        "    pub origin: ShapeVertex,",
        "    pub direction_point: ShapeVertex,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy)]",
        "pub struct ShapeDataEntry {",
        "    pub header_index: u16,",
        "    /// Bank-$00 ShapeHdr address used as the runtime shape token.",
        "    pub shape_id: u16,",
        "    pub points_address: u32,",
        "    pub faces_address: u32,",
        "    pub sort_z: i16,",
        "    pub shift: u8,",
        "    pub radius: u16,",
        "    pub bounds: [u16; 3],",
        "    pub size: u16,",
        "    pub color_table: u16,",
        "    pub lods: [u16; 4],",
        "    pub clipping_planes: &'static [ShapeClipPlane],",
        "    pub vertices: &'static [ShapeVertex],",
        "    pub animation_frames: &'static [&'static [ShapeVertex]],",
        "    pub faces: &'static [ShapeFace],",
        "    pub name: &'static str,",
        "}",
        "",
        "const fn v(x: i16, y: i16, z: i16) -> ShapeVertex {",
        "    ShapeVertex { x, y, z }",
        "}",
        "",
        "const fn f(indices: [u16; 12], n: u8, color: u8,",
        "           visibility_vertices: Option<[u16; 3]>, normal: [i8; 3]) -> ShapeFace {",
        "    ShapeFace { vertex_indices: indices, num_verts: n, color_index: color,",
        "                visibility_vertices, normal }",
        "}",
        "",
    ])

    for shape in shapes:
        address = shape.header.address
        lines.append(f"// ShapeHdr ${address:04X}")
        frames = shape.animation_frames or (shape.vertices,)
        for frame_index, vertices in enumerate(frames):
            suffix = "" if frame_index == 0 else f"_FRAME_{frame_index}"
            lines.append(
                f"static SHAPE_{address:04X}{suffix}_VERTS: "
                f"[ShapeVertex; {len(vertices)}] = ["
            )
            for vertex in vertices:
                lines.append(f"    v({vertex.x}, {vertex.y}, {vertex.z}),")
            lines.append("];")
        if shape.animation_frames:
            lines.append(
                f"static SHAPE_{address:04X}_ANIMATION_FRAMES: "
                f"[&[ShapeVertex]; {len(shape.animation_frames)}] = ["
            )
            for frame_index in range(len(shape.animation_frames)):
                suffix = "" if frame_index == 0 else f"_FRAME_{frame_index}"
                lines.append(f"    &SHAPE_{address:04X}{suffix}_VERTS,")
            lines.append("];")
        lines.append(
            f"static SHAPE_{address:04X}_FACES: [ShapeFace; {len(shape.faces)}] = ["
        )
        for face in shape.faces:
            padded = list(face.vertex_indices) + [0] * (12 - len(face.vertex_indices))
            indices = ", ".join(str(value) for value in padded)
            normal = ", ".join(str(value) for value in face.normal)
            visibility = (
                f"Some([{', '.join(str(value) for value in face.visibility_vertices)}])"
                if face.visibility_vertices is not None else "None"
            )
            lines.append(
                f"    f([{indices}], {len(face.vertex_indices)}, {face.color_index}, "
                f"{visibility}, [{normal}]),"
            )
        lines.append("];")
        if shape.clipping_planes:
            lines.append(f"static SHAPE_{address:04X}_CLIP_PLANES: [ShapeClipPlane; {len(shape.clipping_planes)}] = [")
            for plane in shape.clipping_planes:
                origin, point = plane.origin, plane.direction_point
                lines.append(f"    ShapeClipPlane {{ slot: {plane.slot}, "
                             f"origin: v({origin.x}, {origin.y}, {origin.z}), "
                             f"direction_point: v({point.x}, {point.y}, {point.z}) }},")
            lines.append("];")
        lines.append("")

    lines.extend([
        f"pub const SHAPE_HEADER_START: u16 = 0x{SHAPE_HEADER_START:04X};",
        f"pub const SHAPE_HEADER_SIZE: u16 = 0x{SHAPE_HEADER_SIZE:02X};",
        f"pub const SHAPE_DATA_COUNT: usize = {len(shapes)};",
        "pub static SHAPE_DATA: [ShapeDataEntry; SHAPE_DATA_COUNT] = [",
    ])
    for shape in shapes:
        h = shape.header
        clipping_planes = f"&SHAPE_{h.address:04X}_CLIP_PLANES" if shape.clipping_planes else "&[]"
        animation_frames = (
            f"&SHAPE_{h.address:04X}_ANIMATION_FRAMES"
            if shape.animation_frames else "&[]"
        )
        lines.append(
            "    ShapeDataEntry { "
            f"header_index: {h.index}, shape_id: 0x{h.address:04X}, "
            f"points_address: 0x{h.points_address:06X}, faces_address: 0x{h.faces_address:06X}, "
            f"sort_z: {h.sort_z}, shift: {h.shift}, radius: {h.radius}, "
            f"bounds: [{h.xmax}, {h.ymax}, {h.zmax}], size: {h.size}, "
            f"color_table: 0x{h.color_table:04X}, "
            f"lods: [0x{h.shadow:04X}, 0x{h.simple1:04X}, 0x{h.simple2:04X}, 0x{h.simple3:04X}], "
            f"clipping_planes: {clipping_planes}, vertices: &SHAPE_{h.address:04X}_VERTS, "
            f"animation_frames: {animation_frames}, "
            f"faces: &SHAPE_{h.address:04X}_FACES, name: \"sf2_shape_{h.address:04x}\" "
            "},"
        )
    lines.extend([
        "];",
        "",
        "/// Resolve the exact 16-bit runtime ShapeHdr token.",
        "pub fn shape_by_id(shape_id: u16) -> Option<&'static ShapeDataEntry> {",
        "    let delta = shape_id.checked_sub(SHAPE_HEADER_START)?;",
        "    if delta % SHAPE_HEADER_SIZE != 0 {",
        "        return None;",
        "    }",
        "    SHAPE_DATA.get((delta / SHAPE_HEADER_SIZE) as usize)",
        "}",
        "",
    ])

    output = os.path.join(RUST_SRC, "shape_data.rs")
    with open(output, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))
    subprocess.run(["rustfmt", "--edition", "2021", output], check=True)


def extract(data: bytes) -> list[Shape]:
    shapes = extract_shapes(data)
    emit_rust(shapes)
    vertex_count = sum(len(shape.vertices) for shape in shapes)
    animation_frame_count = sum(len(shape.animation_frames) for shape in shapes)
    face_count = sum(len(shape.faces) for shape in shapes)
    clipping_plane_count = sum(len(shape.clipping_planes) for shape in shapes)
    print(
        f"  shape_data.rs: {len(shapes)} verified ShapeHdrs, "
        f"{vertex_count} vertices, {face_count} faces, "
        f"{animation_frame_count} animation frames, "
        f"{clipping_plane_count} clipping-plane definitions"
    )
    return shapes


if __name__ == "__main__":
    extract(load_rom())
