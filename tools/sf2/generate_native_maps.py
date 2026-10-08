#!/usr/bin/env python3
"""Lower reviewed complete authored map scripts into a typed native catalog.

Map records become `MapInstruction<MapEffect, MapSpawn>` values. Stores name
their domain owner, spawn records their native behavior, and the inline
65816 blocks their reviewed action; no encoded address or source operand
enters gameplay. Unsupported records abort the whole generation instead of
being skipped or replaced by placeholders.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent / "disasm"))
from extract_map import (  # noqa: E402
    DEFAULT_ROM,
    RECORD_SIZES,
    InlineBranchWordBits,
    MapAddress,
    MapExtractor,
)
from generate_native_paths import shape_index  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
OUTPUT = REPO / "rust/sf2-game/src/native/authored_maps.rs"

# Complete roots: (Rust name, script address). The scene-player prologue is
# the initial map tuple at bank 05's start; the attract scenes are installed
# by the title stage ($03:BCC9 and $03:B8DC).
ROOTS = (
    ("SCENE_PLAYER_PROLOGUE", MapAddress(5, 0x0003)),
    ("ATTRACT_SCENE_SIX", MapAddress(5, 0x7BE7)),
    ("ATTRACT_SCENE_SEVEN", MapAddress(5, 0x0035)),
)

PHASE_HOLD = 0x1388

# Reviewed scene-visible store targets: (opcode, address) -> effect builder.
BYTE_STORES = {
    0x1D72: lambda v: f"MapEffect::ActionGate({v})",
    0x1D73: lambda v: f"MapEffect::SceneSelection({v})",
    0x1D74: lambda v: f"MapEffect::HandoffFlags({v})",
    0x18BB: lambda v: f"MapEffect::Presentation(PresentationByte::SceneStyle, {v})",
    0x1B49: lambda v: f"MapEffect::Presentation(PresentationByte::TitleLayout, {v})",
    0x1D57: lambda v: f"MapEffect::Presentation(PresentationByte::PlayerCountLatch, {v})",
}
WORD_STORES = {
    0x1E44: lambda v: f"MapEffect::CameraProjectionBase({v - 0x10000 if v & 0x8000 else v})",
}
# Scene-player initializers for the primary ($06:82F9) and secondary
# ($06:82ED) hit sides.
SPAWN_BEHAVIORS = {
    0x0682F9: ("ObjectKind::Player", "Behavior::PlayerSceneInit(HitSide::Primary)"),
    0x0682ED: ("ObjectKind::Player", "Behavior::PlayerSceneInit(HitSide::Secondary)"),
}
CALLS = {
    # $03:DD6F: display mode 02, fade progress (F4) clear, scene style 02.
    0x03DD6E: "MapEffect::ResetSceneDisplay",
}
INLINE_BRANCHES = {
    # 1AA6 bit 02: the shared single-player display policy.
    (0x1AA6, 0x02): "MapCondition::SinglePlayer",
}


class UnsupportedMap(Exception):
    pass


def graph(extractor: MapExtractor, root: MapAddress):
    pending = [root]
    seen: dict[MapAddress, int] = {}
    order = []
    inline_exits = {}
    while pending:
        address = pending.pop()
        if address in seen:
            continue
        opcode = extractor.byte(address)
        if opcode not in RECORD_SIZES:
            raise UnsupportedMap(f"invalid opcode {opcode:02X} at {address.label()}")
        seen[address] = opcode
        order.append(address)
        successors = extractor._successors(address, opcode, inline_exits, [])
        if opcode == 0x2E and extractor._is_external_phase_gate(address) and len(successors) == 2:
            # A host-released gate continues at the next record. Within one
            # script that is the next gate's hold; after the last gate it is
            # the following script, which is not part of this root.
            continuation = successors[1]
            if not (extractor.byte(continuation) == 0x12
                    and extractor.word(continuation, 1) == PHASE_HOLD):
                successors = successors[:1]
        pending.extend(reversed(successors))
    return order, inline_exits


def lower(rom: bytes) -> str:
    extractor = MapExtractor(rom)
    addresses: list[MapAddress] = []
    inline_exits = {}
    for _, root in ROOTS:
        order, exits = graph(extractor, root)
        inline_exits.update(exits)
        for address in order:
            if address not in addresses:
                addresses.append(address)
    addresses.sort()
    index = {address: i for i, address in enumerate(addresses)}

    def cursor(address: MapAddress) -> str:
        if address not in index:
            raise UnsupportedMap(f"successor outside the reviewed graph: {address.label()}")
        return f"MapCursor::from_index({index[address]})"

    statements = []
    phase_exits = []
    for address in addresses:
        opcode = extractor.byte(address)
        size = RECORD_SIZES[opcode]
        nxt = MapAddress(address.bank, address.offset + size)
        b = lambda i: extractor.byte(address, i)  # noqa: E731
        w = lambda i: extractor.word(address, i)  # noqa: E731
        if opcode == 0x02:
            statements.append("MapInstruction::Stop")
        elif opcode == 0x12:
            statements.append(f"MapInstruction::Yield {{ marker: {w(1)}, next: {cursor(nxt)} }}")
        elif opcode == 0x2E:
            target = MapAddress(b(3), w(1))
            statements.append(f"MapInstruction::Jump({cursor(target)})")
            hold = MapAddress(address.bank, address.offset - 3)
            if (extractor._is_external_phase_gate(address) and target == hold
                    and extractor.word(hold, 1) == PHASE_HOLD and nxt in index):
                phase_exits.append(f"PhaseExit {{ parked: {cursor(address)}, continuation: {cursor(nxt)} }}")
        elif opcode == 0x50:
            statements.append(f"MapInstruction::Apply {{ effect: MapEffect::DisplayMode(DisplayModeRequest::Blank), next: {cursor(nxt)} }}")
        elif opcode == 0x4E:
            statements.append(f"MapInstruction::Apply {{ effect: MapEffect::DisplayMode(DisplayModeRequest::Scene), next: {cursor(nxt)} }}")
        elif opcode == 0x4C:
            statements.append(f"MapInstruction::Await {{ condition: MapCondition::DisplayReady, retry_marker: Some(1), next: {cursor(nxt)} }}")
        elif opcode == 0x64:
            statements.append(f"MapInstruction::Await {{ condition: MapCondition::LoadTableIdle, retry_marker: None, next: {cursor(nxt)} }}")
        elif opcode == 0x10:
            statements.append(f"MapInstruction::Apply {{ effect: MapEffect::SceneLoad({w(1)}), next: {cursor(nxt)} }}")
        elif opcode == 0x66:
            statements.append(f"MapInstruction::Apply {{ effect: MapEffect::LoaderHold, next: {cursor(nxt)} }}")
        elif opcode == 0x9A:
            statements.append(f"MapInstruction::Apply {{ effect: MapEffect::AmbientControl({b(1)}), next: {cursor(nxt)} }}")
        elif opcode == 0x5C:
            value, target = b(1), w(2) | b(4) << 16
            if target == 0x001D77:
                # The continuation bank completes the preceding offset store.
                previous = MapAddress(address.bank, address.offset - 6)
                if extractor.byte(previous) != 0x5E or (extractor.word(previous, 3) | extractor.byte(previous, 5) << 16) != 0x001D78:
                    raise UnsupportedMap(f"unpaired continuation bank at {address.label()}")
                continuation = MapAddress(value, extractor.word(previous, 1))
                statements.append(f"MapInstruction::Apply {{ effect: MapEffect::SaveContinuation({cursor(continuation)}), next: {cursor(nxt)} }}")
            elif target in BYTE_STORES:
                statements.append(f"MapInstruction::Apply {{ effect: {BYTE_STORES[target](value)}, next: {cursor(nxt)} }}")
            else:
                raise UnsupportedMap(f"unreviewed byte store {target:06X} at {address.label()}")
        elif opcode == 0x5E:
            value, target = w(1), w(3) | b(5) << 16
            if target == 0x001D78:
                # Paired with the following bank store; that record lowers both.
                following = nxt
                if extractor.byte(following) != 0x5C or (extractor.word(following, 2) | extractor.byte(following, 4) << 16) != 0x001D77:
                    raise UnsupportedMap(f"unpaired continuation offset at {address.label()}")
                statements.append(f"MapInstruction::Jump({cursor(nxt)})")
            elif target in WORD_STORES:
                statements.append(f"MapInstruction::Apply {{ effect: {WORD_STORES[target](value)}, next: {cursor(nxt)} }}")
            else:
                raise UnsupportedMap(f"unreviewed word store {target:06X} at {address.label()}")
        elif opcode == 0x7A:
            target = w(1) | b(3) << 16
            if target not in CALLS:
                raise UnsupportedMap(f"unreviewed map call {target:06X} at {address.label()}")
            statements.append(f"MapInstruction::Apply {{ effect: {CALLS[target]}, next: {cursor(MapAddress(address.bank, address.offset + 4))} }}")
        elif opcode == 0x86:
            spawn = extractor._spawn(address, opcode)
            if spawn.strategy not in SPAWN_BEHAVIORS:
                raise UnsupportedMap(f"unreviewed spawn strategy {spawn.strategy:06X} at {address.label()}")
            kind, behavior = SPAWN_BEHAVIORS[spawn.strategy]
            spec = (f"MapSpawn::Actor(MapActorSpawn {{ kind: {kind}, shape: ShapeId::from_catalog_index({shape_index(spawn.shape)}), "
                    f"behavior: {behavior}, position: Vector3 {{ x: {spawn.x}, y: {spawn.y}, z: {spawn.z} }} }})")
            statements.append(f"MapInstruction::Spawn {{ specification: {spec}, marker: {spawn.delay}, next: {cursor(nxt)} }}")
        elif opcode == 0x78:
            action = extractor._decode_inline_action(address, inline_exits[address])
            if not isinstance(action, InlineBranchWordBits):
                raise UnsupportedMap(f"unreviewed inline action {action} at {address.label()}")
            key = (action.address & 0xFFFF, action.mask)
            if key not in INLINE_BRANCHES:
                raise UnsupportedMap(f"unreviewed inline branch {key} at {address.label()}")
            taken = MapAddress(address.bank, action.if_set)
            otherwise = MapAddress(address.bank, action.if_clear)
            statements.append(f"MapInstruction::Branch {{ condition: {INLINE_BRANCHES[key]}, taken: {cursor(taken)}, otherwise: {cursor(otherwise)} }}")
        else:
            raise UnsupportedMap(f"unreviewed map opcode {opcode:02X} at {address.label()}")

    lines = [
        "// Auto-generated by tools/sf2/generate_native_maps.py; do not edit by hand.",
        "//! Reviewed authored map scripts as a typed native catalog.",
        "",
        "use super::map_effects::{DisplayModeRequest, MapEffect, MapSpawn, PresentationByte};",
        "use super::scene_map::{",
        "    CatalogError, MapActorSpawn, MapCatalog, MapCondition, MapCursor, MapInstruction, PhaseExit,",
        "};",
        "use super::hit_response::HitSide;",
        "use super::{Behavior, ObjectKind, ShapeId, Vector3};",
        "",
        f"pub const MAP_COMMAND_COUNT: usize = {len(statements)};",
    ]
    for name, root in ROOTS:
        lines.append(f"/// `${root.label()}`.")
        lines.append(f"pub const {name}: MapCursor = {cursor(root)};")
    lines.append("")
    lines.append(f"pub static MAP_INSTRUCTIONS: [MapInstruction<MapEffect, MapSpawn>; {len(statements)}] = [")
    for address, statement in zip(addresses, statements):
        lines.append(f"    // ${address.label()}")
        lines.append(f"    {statement},")
    lines.append("];")
    lines.append("")
    lines.append(f"pub static MAP_PHASE_EXITS: [PhaseExit; {len(phase_exits)}] = [")
    for exit_ in phase_exits:
        lines.append(f"    {exit_},")
    lines.append("];")
    lines.append("")
    lines.append("pub fn catalog() -> Result<MapCatalog<'static, MapEffect, MapSpawn>, CatalogError> {")
    lines.append("    MapCatalog::new(&MAP_INSTRUCTIONS, &MAP_PHASE_EXITS)")
    lines.append("}")
    source = "\n".join(lines) + "\n"
    return subprocess.run(["rustfmt", "--edition", "2021", "--emit", "stdout"],
                          input=source, text=True, capture_output=True, check=True).stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    source = lower(args.rom.read_bytes())
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != source:
            print(f"out of date: {OUTPUT}", file=sys.stderr)
            return 1
    else:
        OUTPUT.write_text(source)
    print(f"Native map catalog: {len(ROOTS)} complete root(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
