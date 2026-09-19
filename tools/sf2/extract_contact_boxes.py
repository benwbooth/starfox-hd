#!/usr/bin/env python3
"""Extract object-contact boxes from the SF2 shape headers, without execution.

This is NOT the downward-surface profile at ShapeHdr+22. generate_collist
($7F:3302) reads the contact-box pointer at ShapeHdr+8, whose records live in
the statically copied bank-$7F data. The collision traversal selects a masked
animation variant BEFORE reading that variant's next-group link. Preserve
those links as typed group IDs rather than assuming a fixed flat box list.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import subprocess

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom, sw16, u16

SHAPE_HEADER_START = 0xBC9C
SHAPE_HEADER_SIZE = 28
SHAPE_HEADER_COUNT = 577
CONTACT_POINTER_OFFSET = 8
RECORD_SIZE = 18
COPIED_BANK_FILE_BASE = 0x10000
COPIED_BANK_LIMIT = 0x7E00


@dataclass(frozen=True)
class ContactBox:
    next_group: int
    center: tuple[int, int, int]
    half_extents: tuple[int, int, int]
    rotation_flags: int
    hit_flags: int


@dataclass(frozen=True)
class ContactGroup:
    variant_mask: int
    variants: tuple[ContactBox, ...]


def parse(data: bytes) -> tuple[dict[int, int], dict[int, ContactGroup]]:
    roots = {}
    for index in range(SHAPE_HEADER_COUNT):
        header = SHAPE_HEADER_START - 0x8000 + index * SHAPE_HEADER_SIZE
        pointer = u16(data, header + CONTACT_POINTER_OFFSET)
        if pointer:
            roots[index] = pointer
    pending = list(roots.values())
    groups = {}
    while pending:
        address = pending.pop()
        if address in groups:
            continue
        if not 0 < address < COPIED_BANK_LIMIT:
            raise ValueError(f"contact group ${address:04X} is outside copied source data")
        offset = COPIED_BANK_FILE_BASE + address
        count = data[offset + 2]
        stored_count = max(1, count)
        if address + stored_count * RECORD_SIZE > COPIED_BANK_LIMIT:
            raise ValueError(f"contact variants at ${address:04X} overrun copied data")
        records = []
        for variant in range(stored_count):
            start = offset + variant * RECORD_SIZE
            record = ContactBox(
                next_group=u16(data, start),
                center=tuple(sw16(data, start + axis) for axis in (3, 5, 7)),
                half_extents=tuple(u16(data, start + axis) for axis in (10, 12, 14)),
                rotation_flags=data[start + 9],
                hit_flags=data[start + 16],
            )
            records.append(record)
            if record.next_group:
                pending.append(record.next_group)
        groups[address] = ContactGroup((count - 1) & 0x7F if count else 0, tuple(records))

    # Every selectable frame must terminate. There are only 128 possible
    # low frame values, so this is a complete static branch check here.
    for root in roots.values():
        for frame in range(128):
            address = root
            seen = set()
            while address:
                if address in seen:
                    raise ValueError(f"cyclic contact-box chain at ${address:04X}, frame {frame}")
                seen.add(address)
                group = groups[address]
                address = group.variants[frame & group.variant_mask].next_group
    return roots, dict(sorted(groups.items()))


def render(data: bytes) -> str:
    roots, groups = parse(data)
    ids = {address: index for index, address in enumerate(groups)}
    lines = [
        AUTOGEN_HEADER.format(tool="extract_contact_boxes.py").rstrip(),
        "",
        "//! Authored object-contact boxes, distinct from downward-surface profiles.",
        "//! Variant-specific continuation links are decoded into typed group IDs.",
        "",
        f"pub const CONTACT_BOX_SHAPE_COUNT: usize = {len(roots)};",
        f"pub const CONTACT_BOX_GROUP_COUNT: usize = {len(groups)};",
        f"pub const CONTACT_BOX_RECORD_COUNT: usize = {sum(len(g.variants) for g in groups.values())};",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct ContactBoxGroupId(usize);",
        "",
        "impl ContactBoxGroupId {",
        "    pub fn group(self) -> &'static ContactBoxGroup { &GROUPS[self.0] }",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct ContactBox {",
        "    pub next: Option<ContactBoxGroupId>,",
        "    pub center: [i16; 3],",
        "    pub half_extents: [u16; 3],",
        "    /// High nibble selects rotation; low nibble is the byte-axis shift.",
        "    pub rotation_flags: u8,",
        "    pub hit_flags: u8,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct ContactBoxGroup {",
        "    pub variant_mask: u8,",
        "    pub variants: &'static [ContactBox],",
        "}",
        "",
    ]
    for address, group in groups.items():
        lines.append(f"// Source contact-box group $7F:{address:04X}.")
        lines.append(f"static BOXES_{ids[address]}: [ContactBox; {len(group.variants)}] = [")
        for record in group.variants:
            next_id = f"Some(ContactBoxGroupId({ids[record.next_group]}))" if record.next_group else "None"
            lines.extend([
                "    ContactBox {",
                f"        next: {next_id},",
                f"        center: {list(record.center)},",
                f"        half_extents: {list(record.half_extents)},",
                f"        rotation_flags: 0x{record.rotation_flags:02X},",
                f"        hit_flags: 0x{record.hit_flags:02X},",
                "    },",
            ])
        lines.extend(["];", ""])
    lines.append(f"static GROUPS: [ContactBoxGroup; {len(groups)}] = [")
    for address, group in groups.items():
        lines.append(f"    ContactBoxGroup {{ variant_mask: 0x{group.variant_mask:02X}, variants: &BOXES_{ids[address]} }},")
    lines.extend(["];", "", "/// None selects the shape header's ordinary bounds.", "pub fn contact_boxes_by_index(index: usize) -> Option<ContactBoxGroupId> {", "    match index {"])
    for index, root in roots.items():
        lines.append(f"        {index} => Some(ContactBoxGroupId({ids[root]})),")
    lines.extend(["        _ => None,", "    }", "}", ""])
    return "\n".join(lines)


def extract(data: bytes) -> None:
    output = Path(RUST_SRC) / "contact_box_data.rs"
    output.write_text(render(data), encoding="utf-8")
    subprocess.run(["rustfmt", "--edition", "2021", str(output)], check=True)
    print(f"  contact boxes: {output}")


if __name__ == "__main__":
    extract(load_rom())
