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
class PointBlock:
    source_address: int
    words: bool
    mirrored: bool
    first_vertex: int
    count: int


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
class FaceCommand:
    """Decoded authored command, with file-offset edges until emission.

    Addresses are provenance only. The native catalog resolves every edge to
    a node index rather than executing ROM bytecode.
    """
    address: int
    operation: str
    arguments: tuple
    targets: tuple[int | None, ...]


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
    face_program: tuple[FaceCommand, ...]
    point_frames: tuple[tuple[PointBlock, ...], ...]


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


def parse_point_stream(
        data: bytes,
        address: int,
        animation_frame: int,
) -> tuple[tuple[Vertex, ...], tuple[int, ...], tuple[PointBlock, ...]]:
    if address == 0:
        return (), (), ()

    cursor = gsu_to_file(data, address)
    vertices: list[Vertex] = []
    frame_counts: list[int] = []
    visited: set[int] = set()
    blocks: list[PointBlock] = []
    for _ in range(4096):
        if not 0 <= cursor < len(data):
            raise ShapeParseError(f"unterminated point program from ${address:06X}")
        if cursor in visited:
            raise ShapeParseError(f"point-program loop from ${address:06X}")
        visited.add(cursor)
        opcode = data[cursor]

        if opcode in (MVAL_ROTPOINTS8, MVAL_ROTPOINTS16,
                      MVAL_ROTPOINTSX8, MVAL_ROTPOINTSX16):
            if cursor + 2 > len(data):
                raise ShapeParseError("truncated point block header")
            count = data[cursor + 1]
            words = opcode in (MVAL_ROTPOINTS16, MVAL_ROTPOINTSX16)
            mirrored = opcode in (MVAL_ROTPOINTSX8, MVAL_ROTPOINTSX16)
            if count == 0:
                raise ShapeParseError("zero-count point block is a wrapping machine loop")
            if cursor + 2 + count * (6 if words else 3) > len(data):
                raise ShapeParseError("truncated point block coordinates")
            bank = address >> 16
            blocks.append(PointBlock(
                (bank << 16) | (cursor - bank * 0x8000 + 0x8000),
                words, mirrored, len(vertices), count))
            cursor += 2
            for _ in range(count):
                if words:
                    x, y, z = struct.unpack_from("<hhh", data, cursor)
                    cursor += 6
                else:
                    x, y, z = (s8(value) for value in data[cursor : cursor + 3])
                    cursor += 3
                vertices.append(Vertex(x, y, z))
                if mirrored:
                    vertices.append(Vertex((32768 - x) % 65536 - 32768, y, z))
            continue

        if opcode == MVAL_VNORMALS:
            count = data[cursor + 1]
            cursor += 2 + count * 3
            continue
        if opcode == MVAL_ENDPOINTS:
            return tuple(vertices), tuple(frame_counts), tuple(blocks)
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


def parse_vertex_stream(data: bytes, address: int, animation_frame: int):
    vertices, counts, _ = parse_point_stream(data, address, animation_frame)
    return vertices, counts


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


def parse_face_program(data: bytes, address: int):
    if address == 0:
        return (), (), ()

    bank = (address >> 16) & 0xFF
    pending: list[
        tuple[int, tuple[tuple[int, int, int], ...] | None]
    ] = [(gsu_to_file(data, address), None)]
    visited: set[int] = set()
    faces: list[Face] = []
    clipping_planes: list[ClipPlane] = []
    commands: list[FaceCommand] = []
    contexts: dict[int, tuple | None] = {}

    def require(cursor: int, length: int, description: str):
        if cursor < 0 or cursor + length > len(data):
            raise ShapeParseError(f"truncated {description} at ROM file ${cursor:06X}")

    def record(cursor: int, operation: str, arguments=(), targets=()):
        commands.append(FaceCommand(cursor, operation, tuple(arguments), tuple(targets)))

    while pending:
        cursor, visibility_tests = pending.pop()
        while True:
            if cursor in visited:
                if contexts[cursor] != visibility_tests and data[cursor] != MVAL_VIZIS:
                    raise ShapeParseError(f"conflicting visibility contexts at ROM file ${cursor:06X}")
                break
            if not 0 <= cursor < len(data):
                raise ShapeParseError(f"unterminated face program from ${address:06X}")
            if len(visited) >= 65536:
                raise ShapeParseError(f"face program from ${address:06X} exceeded step limit")
            visited.add(cursor)
            contexts[cursor] = visibility_tests
            opcode = data[cursor]

            if opcode == MVAL_VIZIS:
                require(cursor, 2, "visibility table")
                count = data[cursor + 1]
                require(cursor, 2 + count * 3, "visibility table")
                visibility_tests = tuple(
                    tuple(data[cursor + 2 + index * 3 : cursor + 5 + index * 3])
                    for index in range(count)
                )
                record(cursor, "Visibility", visibility_tests, (cursor + 2 + count * 3,))
                cursor += 2 + count * 3
                continue
            if opcode == MVAL_BSPINIT:
                record(cursor, "BeginBsp", targets=(cursor + 1,))
                cursor += 1
                continue
            if opcode == MVAL_BSP:
                require(cursor, 5, "BSP node")
                selector = data[cursor + 1]
                if visibility_tests is None or selector >= len(visibility_tests):
                    raise ShapeParseError(f"BSP visibility {selector} at ROM file ${cursor:06X} has no matching test")
                relative = s16(data, cursor + 2)
                first = cursor + 3 + relative
                # The source zero-extends this byte; zero means no right
                # child, not a command at the operand's own address.
                offset = data[cursor + 4]
                second = cursor + 4 + offset if offset else None
                fallthrough = cursor + 5
                record(cursor, "Bsp", (selector,), (first, fallthrough, second))
                # Preserve the geometry union independently of runtime BSP
                # ordering and conditional coplanar-list submission.
                pending.append((first, visibility_tests))
                if second is not None:
                    pending.append((second, visibility_tests))
                pending.append((fallthrough, visibility_tests))
                break
            if opcode == MVAL_BSPE:
                require(cursor, 3, "BSP leaf")
                target = cursor + 2 + s16(data, cursor + 1)
                record(cursor, "BspLeaf", targets=(target,))
                cursor = target
                continue
            if opcode in (MVAL_BSPEND, MVAL_QUIT, MVAL_ENDSHAPE):
                record(cursor, {MVAL_BSPEND: "ReturnBsp", MVAL_QUIT: "Quit", MVAL_ENDSHAPE: "EndShape"}[opcode])
                break

            if opcode == MVAL_FACES:
                command_address = cursor
                first_face = len(faces)
                cursor += 1
                while True:
                    require(cursor, 1, "face list")
                    if data[cursor] in (0xFE, 0xFF):
                        break
                    vertex_count = data[cursor]
                    if not 2 <= vertex_count <= 12:
                        raise ShapeParseError(
                            f"invalid face arity {vertex_count} at ROM file ${cursor:06X}"
                        )
                    require(cursor, 6 + vertex_count, "face record")
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
                record(command_address, "Faces", (first_face, len(faces) - first_face),
                       (cursor if terminator == 0xFE else None,))
                if terminator == 0xFF:
                    break
                continue

            if opcode == MVAL_GROUPS:
                require(cursor, 2, "group table")
                count = data[cursor + 1]
                require(cursor, 2 + count * 3, "group table")
                groups = []
                targets = []
                for index in range(count):
                    # Source $01:9CEE reads all depth-point bytes first;
                    # the following table contains count 16-bit pointers.
                    pointer = u16(data, cursor + 2 + count + index * 2)
                    target = _face_program_target(data, bank, pointer)
                    groups.append(data[cursor + 2 + index])
                    targets.append(target)
                    pending.append((target, visibility_tests))
                record(cursor, "Groups", groups, targets)
                break
            if opcode == MVAL_SPRITE:
                require(cursor, 4, "sprite")
                record(cursor, "Sprite", data[cursor + 1:cursor + 4], (cursor + 4,))
                cursor += 4
                continue
            if opcode == MVAL_SPRITEVIS:
                require(cursor, 5, "visible sprite")
                record(cursor, "VisibleSprite", data[cursor + 1:cursor + 5], (cursor + 5,))
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
                record(cursor, "ClipPlane", (len(clipping_planes) - 1,),
                       (cursor + CLIP_PLANE_RECORD_SIZE,))
                # This is a continuing command. Stopping here discarded the
                # paired opposing plane and could conceal unknown tail code.
                cursor += CLIP_PLANE_RECORD_SIZE
                continue

            raise ShapeParseError(
                f"unknown face opcode ${opcode:02X} at ROM file ${cursor:06X} "
                f"(root ${address:06X})"
            )

    return tuple(faces), tuple(clipping_planes), tuple(commands)


def parse_faces(data: bytes, address: int) -> tuple[tuple[Face, ...], tuple[ClipPlane, ...]]:
    faces, planes, _ = parse_face_program(data, address)
    return faces, planes


def extract_shapes(data: bytes) -> list[Shape]:
    if data[0x8068:0x806C] != bytes.fromhex("ffadf001"):
        raise ShapeParseError("clipping-plane dispatch signature mismatch")
    for opcode, target in ((4, 0x9971), (8, 0x9742), (0x34, 0x97C9), (0x38, 0x99E8)):
        expected = bytes((0xFF, target & 0xFF, target >> 8, 1))
        if data[0x8000 + opcode:0x8004 + opcode] != expected:
            raise ShapeParseError(f"point dispatch signature mismatch for ${opcode:02X}")
    shapes: list[Shape] = []
    for header in parse_headers(data):
        vertex_frames = parse_vertex_frames(data, header.points_address)
        vertices = vertex_frames[0]
        animation_frames = vertex_frames if len(vertex_frames) > 1 else ()
        faces, clipping_planes, face_program = parse_face_program(data, header.faces_address)
        for frame_index, frame in enumerate(vertex_frames):
            for face_index, face in enumerate(faces):
                for vertex_index in face.vertex_indices:
                    if vertex_index >= len(frame):
                        raise ShapeParseError(
                            f"ShapeHdr ${header.address:04X} frame {frame_index} face "
                            f"{face_index} references vertex {vertex_index}, but only "
                            f"{len(frame)} exist"
                        )
        point_frames = tuple(parse_point_stream(data, header.points_address, frame)[2]
                             for frame in range(len(vertex_frames)))
        shapes.append(Shape(header, vertices, animation_frames, faces, clipping_planes,
                            face_program, point_frames))
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


def emit_face_programs(shapes: list[Shape]) -> None:
    types = "FaceCommand, FaceNode, FaceProgram, NodeId"
    if any(node.operation == "Groups" for shape in shapes for node in shape.face_program):
        types += ", FaceGroup"
    lines = [AUTOGEN_HEADER.format(tool="extract_shapes.py"),
             "//! Authored face-program topology; edges are native node indices.",
             f"use crate::shape_program::{{{types}}};", ""]
    for shape in shapes:
        nodes = shape.face_program
        ids = {node.address: index for index, node in enumerate(nodes)}

        def target(address):
            if address not in ids:
                raise ShapeParseError(f"unresolved face-program edge ${address:06X}")
            if ids[address] > 0xFFFF:
                raise ShapeParseError("face-program node index exceeds native representation")
            return f"NodeId({ids[address]})"

        def optional(address):
            return "None" if address is None else f"Some({target(address)})"

        lines.append(f"static PROGRAM_{shape.header.address:04X}: [FaceNode; {len(nodes)}] = [")
        for node in nodes:
            args, edges = node.arguments, node.targets
            op = node.operation
            if op == "Visibility":
                triangles = ", ".join(f"[{a}, {b}, {c}]" for a, b, c in args)
                value = f"Visibility {{ triangles: &[{triangles}], next: {target(edges[0])} }}"
            elif op == "BeginBsp":
                value = f"BeginBsp {{ root: {target(edges[0])} }}"
            elif op == "Bsp":
                value = (f"Bsp {{ visibility: {args[0]}, coplanar: {target(edges[0])}, "
                         f"left: {target(edges[1])}, right: {optional(edges[2])} }}")
            elif op == "BspLeaf":
                value = f"BspLeaf {{ faces: {target(edges[0])} }}"
            elif op in ("ReturnBsp", "Quit", "EndShape"):
                value = op
            elif op == "Faces":
                value = f"Faces {{ first: {args[0]}, count: {args[1]}, next: {optional(edges[0])} }}"
            elif op == "Groups":
                groups = ", ".join(f"FaceGroup {{ depth_point: {point}, root: {target(edge)} }}"
                                   for point, edge in zip(args, edges))
                value = f"Groups {{ entries: &[{groups}] }}"
            elif op in ("Sprite", "VisibleSprite"):
                value = f"{op} {{ parameters: [{', '.join(map(str, args))}], next: {target(edges[0])} }}"
            elif op == "ClipPlane":
                value = f"ClipPlane {{ plane: {args[0]}, next: {target(edges[0])} }}"
            else:
                raise ShapeParseError(f"unhandled native face command {op}")
            bank = shape.header.faces_address >> 16
            source_address = (bank << 16) | (node.address - bank * 0x8000 + 0x8000)
            lines.append(f"    FaceNode {{ source_address: 0x{source_address:06X}, command: FaceCommand::{value} }},")
        lines.append("];\n")
    lines.append(f"pub static FACE_PROGRAMS: [FaceProgram; {len(shapes)}] = [")
    for shape in shapes:
        root = "Some(NodeId(0))" if shape.face_program else "None"
        lines.append(f"    FaceProgram {{ root: {root}, nodes: &PROGRAM_{shape.header.address:04X} }},")
    lines.append("];\n")
    output = os.path.join(RUST_SRC, "shape_program_data.rs")
    with open(output, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))
    subprocess.run(["rustfmt", "--edition", "2021", output], check=True)


def emit_point_programs(shapes: list[Shape]) -> None:
    lines = [AUTOGEN_HEADER.format(tool="extract_shapes.py"),
             "//! Authored point-block formats and mirror relationships by animation frame.",
             "use crate::point_program::{PointBlock, PointFormat, PointProgram};", "",
             "#[rustfmt::skip]",
             f"pub static POINT_PROGRAMS: [PointProgram; {len(shapes)}] = ["]
    for shape in shapes:
        lines.append("    PointProgram { frames: &[")
        for frame in shape.point_frames:
            blocks = []
            for block in frame:
                format_name = "Words" if block.words else "Bytes"
                blocks.append(
                    f"PointBlock {{ source_address: 0x{block.source_address:06X}, "
                    f"format: PointFormat::{format_name}, mirrored: {str(block.mirrored).lower()}, "
                    f"first_vertex: {block.first_vertex}, count: {block.count} }}")
            lines.append("        &[" + ", ".join(blocks) + "],")
        lines.append("    ] },")
    lines.append("];\n")
    output = os.path.join(RUST_SRC, "point_program_data.rs")
    with open(output, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))
    subprocess.run(["rustfmt", "--edition", "2021", output], check=True)


def extract(data: bytes) -> list[Shape]:
    shapes = extract_shapes(data)
    emit_rust(shapes)
    emit_face_programs(shapes)
    emit_point_programs(shapes)
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
