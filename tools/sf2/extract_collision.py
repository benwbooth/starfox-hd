#!/usr/bin/env python3
"""Extract SF2's typed downward-contact collision profiles.

ShapeHdr +22 either points back to the header (the ordinary bounds collider)
or to a bank-$0A compound profile. A compound profile begins with a logical
box count. Each logical box owns either one 18-byte record or an animation
variant run selected by the candidate object's animation frame.

The optional polygon pointer names a convex X/Z footprint encoded as a vertex
count followed by signed byte pairs. Its scale byte is stored in the record.
The plane coefficients and offset are retained exactly; the oracle-only path
host uses retail's exact fixed-point math kernels to consume them.
"""

from __future__ import annotations

from dataclasses import dataclass
import os
import subprocess

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom, s8, sw16, u16


SHAPE_HEADER_START = 0xBC9C
SHAPE_HEADER_SIZE = 28
SHAPE_HEADER_COUNT = 577
COLLISION_POINTER_OFFSET = 22
COLLISION_BANK_FILE_BASE = 0x50000
COLLISION_RECORD_SIZE = 18


@dataclass(frozen=True)
class CollisionRecord:
    center_x: int
    center_z: int
    width: int
    depth: int
    plane_normal: tuple[int, int, int]
    plane_offset: int
    polygon_pointer: int
    box_flags: int
    polygon_scale: int


@dataclass(frozen=True)
class CollisionGroup:
    variants: tuple[CollisionRecord, ...]


@dataclass(frozen=True)
class CollisionProfile:
    address: int
    groups: tuple[CollisionGroup, ...]


def bank_file_offset(address: int) -> int:
    if not 0x8000 <= address <= 0xFFFF:
        raise ValueError(f"collision pointer ${address:04X} is outside bank $0A")
    return COLLISION_BANK_FILE_BASE + (address & 0x7FFF)


def parse_record(data: bytes, offset: int) -> CollisionRecord:
    return CollisionRecord(
        center_x=sw16(data, offset + 1),
        center_z=sw16(data, offset + 3),
        width=u16(data, offset + 5),
        depth=u16(data, offset + 7),
        plane_normal=(
            s8(data[offset + 9]),
            s8(data[offset + 10]),
            s8(data[offset + 11]),
        ),
        plane_offset=sw16(data, offset + 12),
        polygon_pointer=u16(data, offset + 14),
        box_flags=data[offset + 16],
        polygon_scale=data[offset + 17],
    )


def parse_profile(data: bytes, address: int) -> CollisionProfile:
    cursor = bank_file_offset(address)
    group_count = data[cursor]
    cursor += 1
    if group_count == 0:
        raise ValueError(f"collision profile ${address:04X} has no groups")

    groups: list[CollisionGroup] = []
    for _ in range(group_count):
        variant_count = data[cursor]
        stored_count = variant_count if variant_count != 0 else 1
        variants = tuple(
            parse_record(data, cursor + index * COLLISION_RECORD_SIZE)
            for index in range(stored_count)
        )
        groups.append(CollisionGroup(variants=variants))
        cursor += stored_count * COLLISION_RECORD_SIZE
    return CollisionProfile(address=address, groups=tuple(groups))


def parse_polygon(data: bytes, address: int) -> tuple[tuple[int, int], ...]:
    cursor = bank_file_offset(address)
    vertex_count = data[cursor]
    if vertex_count < 3:
        raise ValueError(f"collision polygon ${address:04X} has {vertex_count} vertices")
    return tuple(
        (s8(data[cursor + 1 + index * 2]), s8(data[cursor + 2 + index * 2]))
        for index in range(vertex_count)
    )


def render(data: bytes) -> str:
    shape_profiles: list[tuple[int, int]] = []
    profile_addresses: set[int] = set()
    for index in range(SHAPE_HEADER_COUNT):
        shape_id = SHAPE_HEADER_START + index * SHAPE_HEADER_SIZE
        header_offset = shape_id - 0x8000
        profile_address = u16(data, header_offset + COLLISION_POINTER_OFFSET)
        if profile_address != shape_id:
            shape_profiles.append((shape_id, profile_address))
            profile_addresses.add(profile_address)

    profiles = {
        address: parse_profile(data, address) for address in sorted(profile_addresses)
    }
    polygon_addresses = sorted(
        {
            record.polygon_pointer
            for profile in profiles.values()
            for group in profile.groups
            for record in group.variants
            if record.polygon_pointer != 0
        }
    )
    polygons = {address: parse_polygon(data, address) for address in polygon_addresses}
    group_count = sum(len(profile.groups) for profile in profiles.values())
    record_count = sum(
        len(group.variants)
        for profile in profiles.values()
        for group in profile.groups
    )

    lines = [
        AUTOGEN_HEADER.format(tool="extract_collision.py").rstrip(),
        "",
        "//! Typed SF2 downward-contact collision profiles.",
        "//!",
        "//! Shape headers not listed here use their ordinary axis-aligned bounds.",
        "//! Compound profiles preserve every animated box, plane, flag, and exact",
        "//! convex polygon footprint from retail bank `$0A`.",
        "",
        f"pub const COMPOUND_COLLIDER_SHAPE_COUNT: usize = {len(shape_profiles)};",
        f"pub const COLLISION_PROFILE_COUNT: usize = {len(profiles)};",
        f"pub const COLLISION_GROUP_COUNT: usize = {group_count};",
        f"pub const COLLISION_RECORD_COUNT: usize = {record_count};",
        f"pub const COLLISION_POLYGON_COUNT: usize = {len(polygons)};",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct CollisionRecord {",
        "    pub center_x: i16,",
        "    pub center_z: i16,",
        "    pub width: u16,",
        "    pub depth: u16,",
        "    pub plane_normal: [i8; 3],",
        "    pub plane_offset: i16,",
        "    pub polygon: Option<&'static CollisionPolygon>,",
        "    pub box_flags: u8,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct CollisionGroup {",
        "    pub variants: &'static [CollisionRecord],",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct CollisionProfile {",
        "    pub groups: &'static [CollisionGroup],",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct CollisionPolygon {",
        "    pub source_address: u16,",
        "    pub scale: u8,",
        "    pub vertices: &'static [[i8; 2]],",
        "}",
        "",
    ]

    # A polygon can be shared by records with different scale bytes, so emit
    # its raw vertices once and a record-local descriptor for each use.
    for address, vertices in polygons.items():
        lines.append(
            f"static POLYGON_{address:04X}_VERTICES: [[i8; 2]; {len(vertices)}] = ["
        )
        for x, z in vertices:
            lines.append(f"    [{x}, {z}],")
        lines.extend(["];", ""])

    descriptor_names: dict[tuple[int, int], str] = {}
    for profile in profiles.values():
        for group in profile.groups:
            for record in group.variants:
                if record.polygon_pointer == 0:
                    continue
                key = (record.polygon_pointer, record.polygon_scale)
                if key in descriptor_names:
                    continue
                name = f"POLYGON_{record.polygon_pointer:04X}_S{record.polygon_scale}"
                descriptor_names[key] = name
                lines.extend(
                    [
                        f"static {name}: CollisionPolygon = CollisionPolygon {{",
                        f"    source_address: 0x{record.polygon_pointer:04X},",
                        f"    scale: {record.polygon_scale},",
                        f"    vertices: &POLYGON_{record.polygon_pointer:04X}_VERTICES,",
                        "};",
                        "",
                    ]
                )

    for address, profile in profiles.items():
        for group_index, group in enumerate(profile.groups):
            lines.append(
                f"static PROFILE_{address:04X}_GROUP_{group_index}_VARIANTS: "
                f"[CollisionRecord; {len(group.variants)}] = ["
            )
            for record in group.variants:
                polygon = "None"
                if record.polygon_pointer != 0:
                    polygon = (
                        "Some(&"
                        + descriptor_names[(record.polygon_pointer, record.polygon_scale)]
                        + ")"
                    )
                nx, ny, nz = record.plane_normal
                lines.extend(
                    [
                        "    CollisionRecord {",
                        f"        center_x: {record.center_x},",
                        f"        center_z: {record.center_z},",
                        f"        width: {record.width},",
                        f"        depth: {record.depth},",
                        f"        plane_normal: [{nx}, {ny}, {nz}],",
                        f"        plane_offset: {record.plane_offset},",
                        f"        polygon: {polygon},",
                        f"        box_flags: {record.box_flags},",
                        "    },",
                    ]
                )
            lines.extend(["];", ""])

        lines.append(
            f"static PROFILE_{address:04X}_GROUPS: "
            f"[CollisionGroup; {len(profile.groups)}] = ["
        )
        for group_index, _ in enumerate(profile.groups):
            lines.extend(
                [
                    "    CollisionGroup {",
                    f"        variants: &PROFILE_{address:04X}_GROUP_{group_index}_VARIANTS,",
                    "    },",
                ]
            )
        lines.extend(
            [
                "];",
                "",
                f"static PROFILE_{address:04X}: CollisionProfile = CollisionProfile {{",
                f"    groups: &PROFILE_{address:04X}_GROUPS,",
                "};",
                "",
            ]
        )

    lines.extend(
        [
            "/// Return the compound collision profile for a ShapeHdr token.",
            "/// `None` means retail uses the header's ordinary bounds collider.",
            "pub fn collision_profile(shape_id: u16) -> Option<&'static CollisionProfile> {",
            "    match shape_id {",
        ]
    )
    for shape_id, profile_address in shape_profiles:
        lines.append(f"        0x{shape_id:04X} => Some(&PROFILE_{profile_address:04X}),")
    lines.extend(["        _ => None,", "    }", "}", ""])
    return "\n".join(lines)


def extract(data: bytes) -> None:
    output = os.path.join(RUST_SRC, "collision_data.rs")
    with open(output, "w", encoding="utf-8") as handle:
        handle.write(render(data))
    subprocess.run(["rustfmt", "--edition", "2021", output], check=True)
    print(f"  collision: {output}")


if __name__ == "__main__":
    extract(load_rom())
