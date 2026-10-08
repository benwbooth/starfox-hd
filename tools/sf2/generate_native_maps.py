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
    InlineCall,
    InlineSelectGsuProgram,
    InlineSetPilotLinkedFlag,
    InlineWordBits,
    MapAddress,
    MapExtractor,
)
from generate_native_paths import ROOTS as PATH_ROOTS, shape_index  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
OUTPUT = REPO / "rust/sf2-game/src/native/authored_maps.rs"

# Complete roots: (Rust name, script address). The scene-player prologue is
# the initial map tuple at bank 05's start; the attract scenes are installed
# by the title stage ($03:BCC9 and $03:B8DC).
ROOTS = (
    ("SCENE_PLAYER_PROLOGUE", MapAddress(5, 0x0003)),
    ("ATTRACT_SCENE_SIX", MapAddress(5, 0x7BE7)),
    ("ATTRACT_SCENE_SEVEN", MapAddress(5, 0x0035)),
    # Scene launchers installed by the stage table ($03:BA5D..BD90): each
    # loads its scene, publishes the scene selection (1D73) and parks.
    ("SCENE_EIGHT_LAUNCHER", MapAddress(5, 0x00A7)),
    # Scene four: encounter variant 4 (1C06), then scene twenty-five.
    ("SCENE_FOUR_LAUNCHER", MapAddress(5, 0x7C2E)),
    ("SCENE_FIVE_LAUNCHER", MapAddress(5, 0x7C83)),
    ("SCENE_THREE_LAUNCHER", MapAddress(5, 0x7CBD)),
    ("SCENE_ONE_LAUNCHER", MapAddress(5, 0x7CF7)),
    ("SCENE_ONE_ALTERNATE_LAUNCHER", MapAddress(5, 0x7D31)),
    ("SCENE_TWENTY_EIGHT_LAUNCHER", MapAddress(5, 0x7D6B)),
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
    0x1E13: lambda v: f"MapEffect::PlayerCarryMode({v})",
    0x1DE2: lambda v: f"MapEffect::PlayerConfiguration({v})",
    0x1DE3: lambda v: f"MapEffect::PlayerConfigurationVariant({v})",
    0x1DEA: lambda v: f"MapEffect::PlacementHeading({v})",
    0x1D75: lambda v: f"MapEffect::ExitSceneSelection(ExitScene::Primary, {v})",
    0x1D76: lambda v: f"MapEffect::ExitSceneSelection(ExitScene::Alternate, {v})",
    0x1E68: lambda v: f"MapEffect::Presentation(PresentationByte::BackdropProgram, {v})",
    0x70285E: lambda v: f"MapEffect::GsuParameter(GsuParameter::PatternMode, {v})",
    0x1C06: lambda v: f"MapEffect::EncounterVariant({v})",
    0x1E17: lambda v: f"MapEffect::StageExitMode({v})",
}
WORD_STORES = {
    0x1E44: lambda v: f"MapEffect::CameraProjectionBase({signed(v)})",
    0x1E32: lambda v: f"MapEffect::CameraHeightLimit(HeightLimit::Top, {signed(v)})",
    0x1E34: lambda v: f"MapEffect::CameraHeightLimit(HeightLimit::Bottom, {signed(v)})",
    0x1E0F: lambda v: f"MapEffect::EnvironmentPlane({signed(v)})",
    0x18B9: lambda v: f"MapEffect::RenderPlane({signed(v)})",
    0x7ED739: lambda v: f"MapEffect::StreamingRadiusLimit({v})",
    0x1DE4: lambda v: f"MapEffect::PlacementCoordinate(Axis::X, {signed(v)})",
    0x1DE6: lambda v: f"MapEffect::PlacementCoordinate(Axis::Y, {signed(v)})",
    0x1DE8: lambda v: f"MapEffect::PlacementCoordinate(Axis::Z, {signed(v)})",
    0x1D80: lambda v: f"MapEffect::DeferredSceneLoad({v})",
    0x1E5A: lambda v: f"MapEffect::AltitudeGaugeScale({v})",
    0x702862: lambda v: f"MapEffect::GsuParameter(GsuParameter::BackdropColor, {v})",
    0x70285C: lambda v: f"MapEffect::GsuParameter(GsuParameter::PatternOffset, {v})",
}
# Scene-player initializers for the primary ($06:82F9) and secondary
# ($06:82ED) hit sides.
SPAWN_BEHAVIORS = {
    0x0682F9: ("ObjectKind::Player", "Behavior::PlayerSceneInit(HitSide::Primary)"),
    0x0682ED: ("ObjectKind::Player", "Behavior::PlayerSceneInit(HitSide::Secondary)"),
}
# Path-actor strategies: $7F:7E1E is the shared one-time path prefix;
# $7F:7E00 first publishes health 10 and attack 10.
PATH_SPAWNS = {
    0x7F7E1E: "PathEntry::Plain",
    0x7F7E00: "PathEntry::DefaultCombat",
}
LOWERED_PATHS = {root.offset: name for name, root in PATH_ROOTS}
# Opcode 36 writes one byte of the current actor ($03:9A87).
CURRENT_BYTES = {
    0x2D: "MapEffect::ActorHitPoints",
    0x2E: "MapEffect::ActorAttackPower",
}
CALLS = {
    # $03:DD6F: display mode 02, fade progress (F4) clear, scene style 02.
    0x03DD6E: "MapEffect::ResetSceneDisplay",
}
def _signed_byte(value: int) -> int:
    return value - 0x100 if value & 0x80 else value


# Inline calls whose operands are direct-page bytes/words set by the map
# records falling into them: target -> (required {variable: wide}, variables
# the call stores but nothing reads, effect builder).
ARGUMENT_CALLS = {
    # $06:9A2F: the primary player's vertical profile (6BF5..6BFA).
    0x069A2F: ({0x08: True, 0x0A: True, 0x02: False, 0x04: False}, set(), lambda v: (
        "MapEffect::PlayerVerticalProfile(VerticalProfile { "
        f"upper_height_offset: {signed(v[0x08])}, lower_height_offset: {signed(v[0x0A])}, "
        f"up_pitch: {v[0x02]}, down_pitch: {v[0x04]} }})")),
    # $06:9A5F: camera pitch limits (6BFD/6BFE); 6BFB, 6B49 and 6B59 have
    # no reader (the load at $07:8EA8 is overwritten before use).
    0x069A5F: ({0x02: False, 0x04: False}, {0x08, 0x0A, 0xA7}, lambda v: (
        "MapEffect::CameraPitchProfile(CameraPitchProfile { "
        f"up: {_signed_byte(v[0x02])}, down: {_signed_byte(v[0x04])} }})")),
}
PLAIN_CALLS = {
    0x069A92: "MapEffect::PlacePrimaryPlayer",
    0x069ACD: "MapEffect::LinkControlledPilots",
    0x069B04: "MapEffect::OccupancyExempt(true)",
    0x069B20: "MapEffect::OccupancyExempt(false)",
}
INLINE_BRANCHES = {
    # 1AA6 bit 02: the shared single-player display policy.
    (0x1AA6, 0x02): "MapCondition::SinglePlayer",
    # 1B88 bit 2000: the shared scene-event flag.
    (0x1B88, 0x2000): "MapCondition::SceneEvent",
}


class UnsupportedMap(Exception):
    pass


def region_entry(extractor: MapExtractor, address: MapAddress) -> MapAddress:
    """The script published to 1657/192E when the player enters the region."""
    return MapAddress(extractor.byte(address, 8), extractor.word(address, 6))


def signed(word: int) -> int:
    return word - 0x10000 if word & 0x8000 else word


def graph(extractor: MapExtractor, root: MapAddress, edges: dict | None = None):
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
        if opcode == 0x5C and (extractor.word(address, 2) | extractor.byte(address, 4) << 16) == 0x001D77:
            # The saved continuation is a later control transfer ($7F:BF3D).
            previous = MapAddress(address.bank, address.offset - 6)
            target = MapAddress(extractor.byte(address, 1), extractor.word(previous, 1))
            if extractor.byte(previous) == 0x5E and extractor.byte(target) in RECORD_SIZES:
                successors = successors + [target]
        if opcode == 0x5C and (extractor.word(address, 2) | extractor.byte(address, 4) << 16) == 0x001D7A:
            # The stage-exit script runs when a bank-0D stage action installs it.
            previous = MapAddress(address.bank, address.offset - 6)
            if extractor.byte(previous) == 0x5E and (extractor.word(previous, 3) | extractor.byte(previous, 5) << 16) == 0x001D7B:
                successors = successors + [MapAddress(extractor.byte(address, 1), extractor.word(previous, 1))]
        if opcode == 0x94 and not extractor.byte(address, 1) & 0x80:
            # A scannable region's script runs when the player enters it.
            successors = successors + [region_entry(extractor, address)]
        if opcode == 0x2E and extractor._is_external_phase_gate(address) and len(successors) == 2:
            # A host-released gate continues at the next record. Within one
            # script that is the next gate's hold; after the last gate it is
            # the following script, which is not part of this root.
            continuation = successors[1]
            if not (extractor.byte(continuation) == 0x12
                    and extractor.word(continuation, 1) == PHASE_HOLD):
                successors = successors[:1]
        if edges is not None:
            edges[address] = successors
        pending.extend(reversed(successors))
    return order, inline_exits


def lower(rom: bytes, errors: list | None = None) -> str:
    extractor = MapExtractor(rom)
    addresses: list[MapAddress] = []
    inline_exits = {}
    edges: dict[MapAddress, list[MapAddress]] = {}
    for _, root in ROOTS:
        order, exits = graph(extractor, root, edges)
        inline_exits.update(exits)
        for address in order:
            if address not in addresses:
                addresses.append(address)
    addresses.sort()
    index = {address: i for i, address in enumerate(addresses)}
    predecessors: dict[MapAddress, set[MapAddress]] = {}
    for source, targets in edges.items():
        for target in targets:
            predecessors.setdefault(target, set()).add(source)

    def dp_store(address: MapAddress):
        """(variable, value, wide) for a direct-page store record, else None."""
        opcode = extractor.byte(address)
        if opcode == 0x5C and extractor.byte(address, 4) == 0 and extractor.word(address, 2) < 0x100:
            return extractor.word(address, 2), extractor.byte(address, 1), False
        if opcode == 0x5E and extractor.byte(address, 5) == 0 and extractor.word(address, 3) < 0x100:
            return extractor.word(address, 3), extractor.word(address, 1), True
        return None

    def argument_call(address: MapAddress):
        """The inline call target consuming the direct-page run at address."""
        while dp_store(address) is not None:
            address = MapAddress(address.bank, address.offset + RECORD_SIZES[extractor.byte(address)])
        if extractor.byte(address) != 0x78 or address not in inline_exits:
            return None, address
        action = extractor._decode_inline_action(address, inline_exits[address])
        if not isinstance(action, InlineCall) or action.target not in ARGUMENT_CALLS:
            return None, address
        return action, address

    def call_arguments(call: MapAddress) -> dict[int, tuple[int, bool]]:
        """Direct-page values set by the run that falls into `call` only."""
        arguments = {}
        current = call
        while True:
            sources = predecessors.get(current, set())
            if len(sources) != 1:
                break
            (previous,) = sources
            store = dp_store(previous)
            if store is None or MapAddress(previous.bank, previous.offset + RECORD_SIZES[extractor.byte(previous)]) != current:
                break
            variable, value, wide = store
            arguments.setdefault(variable, (value, wide))
            current = previous
        return arguments

    def cursor(address: MapAddress) -> str:
        if address not in index:
            raise UnsupportedMap(f"successor outside the reviewed graph: {address.label()}")
        return f"MapCursor::from_index({index[address]})"

    statements = []
    phase_exits = []
    def lower_one(address):
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
            elif dp_store(address) is not None:
                # Direct-page arguments of the inline call that ends the run.
                variable = dp_store(address)[0]
                action, call = argument_call(address)
                if action is None:
                    raise UnsupportedMap(f"direct-page store {variable:02X} without a reviewed consumer at {address.label()}")
                required, unread, _ = ARGUMENT_CALLS[action.target]
                if variable not in required and variable not in unread:
                    raise UnsupportedMap(f"direct-page store {variable:02X} is not an argument of {action.target:06X}")
                statements.append(f"MapInstruction::Jump({cursor(nxt)})")
            elif opcode == 0x5C:
                value, target = b(1), w(2) | b(4) << 16
                if target == 0x001D7A:
                    previous = MapAddress(address.bank, address.offset - 6)
                    if extractor.byte(previous) != 0x5E or (extractor.word(previous, 3) | extractor.byte(previous, 5) << 16) != 0x001D7B:
                        raise UnsupportedMap(f"unpaired stage-exit bank at {address.label()}")
                    script = MapAddress(value, extractor.word(previous, 1))
                    statements.append(f"MapInstruction::Apply {{ effect: MapEffect::StageExitScript({cursor(script)}), next: {cursor(nxt)} }}")
                elif target == 0x001D77:
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
                if target == 0x001D7B:
                    following = nxt
                    if extractor.byte(following) != 0x5C or (extractor.word(following, 2) | extractor.byte(following, 4) << 16) != 0x001D7A:
                        raise UnsupportedMap(f"unpaired stage-exit offset at {address.label()}")
                    statements.append(f"MapInstruction::Jump({cursor(nxt)})")
                elif target == 0x001D78:
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
            elif opcode == 0x86 and extractor._spawn(address, opcode).strategy in PATH_SPAWNS:
                spawn = extractor._spawn(address, opcode)
                spec = (f"MapSpawn::PathActor {{ shape: ShapeId::from_catalog_index({shape_index(spawn.shape)}), "
                        f"position: Vector3 {{ x: {spawn.x}, y: {spawn.y}, z: {spawn.z} }}, entry: {PATH_SPAWNS[spawn.strategy]} }}")
                statements.append(f"MapInstruction::Spawn {{ specification: {spec}, marker: {spawn.delay}, next: {cursor(nxt)} }}")
            elif opcode == 0x8C:
                path = w(1)
                if path not in LOWERED_PATHS:
                    raise UnsupportedMap(f"map path {path:04X} is not in the native path catalog")
                statements.append(f"MapInstruction::ApplyToCurrent {{ effect: MapEffect::InstallPath(authored_paths::{LOWERED_PATHS[path]}), next: {cursor(nxt)} }}")
            elif opcode == 0x36:
                field, value = w(1), b(3)
                if field not in CURRENT_BYTES:
                    raise UnsupportedMap(f"unreviewed current-actor byte {field:02X}")
                statements.append(f"MapInstruction::ApplyToCurrent {{ effect: {CURRENT_BYTES[field]}({value}), next: {cursor(nxt)} }}")
            elif opcode == 0x86:
                spawn = extractor._spawn(address, opcode)
                if spawn.strategy not in SPAWN_BEHAVIORS:
                    raise UnsupportedMap(f"unreviewed spawn strategy {spawn.strategy:06X} at {address.label()}")
                kind, behavior = SPAWN_BEHAVIORS[spawn.strategy]
                spec = (f"MapSpawn::Actor(MapActorSpawn {{ kind: {kind}, shape: ShapeId::from_catalog_index({shape_index(spawn.shape)}), "
                        f"behavior: {behavior}, position: Vector3 {{ x: {spawn.x}, y: {spawn.y}, z: {spawn.z} }} }})")
                statements.append(f"MapInstruction::Spawn {{ specification: {spec}, marker: {spawn.delay}, next: {cursor(nxt)} }}")
            elif opcode == 0x90:
                # $03:953F: record flags 02 (path); pitch and roll clear.
                path = w(10)
                if path not in LOWERED_PATHS:
                    raise UnsupportedMap(f"map path {path:04X} is not in the native path catalog")
                record = (f"PathRecord {{ position: Vector3 {{ x: {signed(w(1))}, y: {signed(w(3))}, z: {signed(w(5))} }}, "
                          f"yaw: Angle::from_units({b(7)}), shape: ShapeId::from_catalog_index({shape_index(w(8))}), "
                          f"path: authored_paths::{LOWERED_PATHS[path]} }}")
                statements.append(f"MapInstruction::Apply {{ effect: MapEffect::DeclarePathRecord({record}), next: {cursor(nxt)} }}")
            elif opcode == 0x94:
                # $03:90EF: bounds are high bytes; bit 7 of the index byte
                # leaves the region unscannable (byte 0F clear).
                scannable = not b(1) & 0x80
                if scannable:
                    entry = f"Some({cursor(region_entry(extractor, address))})"
                elif (b(6), b(7), b(8)) == (0, 0, 0):
                    entry = "None"
                else:
                    raise UnsupportedMap(f"unscannable region with a script at {address.label()}")
                region = (f"MapRegion {{ origin_x: {b(2) << 8}, origin_z: {b(3) << 8}, width: {b(4) << 8}, "
                          f"depth: {b(5) << 8}, entry: {entry} }}")
                statements.append(f"MapInstruction::Apply {{ effect: MapEffect::RegisterRegion {{ index: {b(1) & 0x7F}, region: {region} }}, next: {cursor(nxt)} }}")
            elif opcode == 0xA4:
                # $03:A03A: scenario flag word (E087) bit 0400.
                taken = MapAddress(address.bank, w(1))
                statements.append(f"MapInstruction::Branch {{ condition: MapCondition::ExternalEvent, taken: {cursor(taken)}, otherwise: {cursor(nxt)} }}")
            elif opcode == 0xA2:
                # $03:A030: encounter layout bit 01.
                taken = MapAddress(address.bank, w(1))
                statements.append(f"MapInstruction::Branch {{ condition: MapCondition::EncounterLayoutOdd, taken: {cursor(taken)}, otherwise: {cursor(nxt)} }}")
            elif opcode == 0x9E:
                # $03:90BF: compare the encounter layout (1BA5).
                taken = MapAddress(address.bank, w(2))
                statements.append(f"MapInstruction::Branch {{ condition: MapCondition::EncounterLayout({b(1)}), taken: {cursor(taken)}, otherwise: {cursor(nxt)} }}")
            elif opcode == 0x78:
                action = extractor._decode_inline_action(address, inline_exits[address])
                if isinstance(action, InlineCall) and action.accumulator is None and action.target in ARGUMENT_CALLS:
                    required, _, build = ARGUMENT_CALLS[action.target]
                    arguments = call_arguments(address)
                    values = {}
                    for variable, wide in required.items():
                        if variable not in arguments or arguments[variable][1] != wide:
                            raise UnsupportedMap(f"call {action.target:06X} lacks argument {variable:02X} at {address.label()}")
                        values[variable] = arguments[variable][0]
                    continuation = MapAddress(address.bank, action.continuation)
                    statements.append(f"MapInstruction::Apply {{ effect: {build(values)}, next: {cursor(continuation)} }}")
                    return
                if isinstance(action, InlineCall) and action.accumulator is None and action.target in PLAIN_CALLS:
                    continuation = MapAddress(address.bank, action.continuation)
                    statements.append(f"MapInstruction::Apply {{ effect: {PLAIN_CALLS[action.target]}, next: {cursor(continuation)} }}")
                    return
                if isinstance(action, InlineCall) and action.target == 0x0DDA7A and action.accumulator is not None:
                    # $0D:DA7A: bit 80 draws, otherwise erases, the marker of
                    # region slot (accumulator & 7F).
                    continuation = MapAddress(address.bank, action.continuation)
                    effect = f"MapEffect::RegionMarker {{ region: {action.accumulator & 0x7F}, drawn: {str(bool(action.accumulator & 0x80)).lower()} }}"
                    statements.append(f"MapInstruction::Apply {{ effect: {effect}, next: {cursor(continuation)} }}")
                    return
                if isinstance(action, (InlineSetPilotLinkedFlag, InlineSelectGsuProgram)):
                    effect = ("MapEffect::LinkPilotTransitions" if isinstance(action, InlineSetPilotLinkedFlag)
                              else "MapEffect::SelectBackdropTable")
                    statements.append(f"MapInstruction::Apply {{ effect: {effect}, next: {cursor(MapAddress(address.bank, action.continuation))} }}")
                    return
                if isinstance(action, InlineWordBits):
                    if (action.address & 0xFFFF, action.mask) != (0x1B84, 0x0100):
                        raise UnsupportedMap(f"unreviewed inline word bits {action} at {address.label()}")
                    effect = f"MapEffect::ModeFlags {{ bits: {action.mask}, set: {str(action.set_bits).lower()} }}"
                    statements.append(f"MapInstruction::Apply {{ effect: {effect}, next: {cursor(MapAddress(address.bank, action.continuation))} }}")
                    return
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


    for address in addresses:
        if errors is not None:
            try:
                lower_one(address)
            except UnsupportedMap as error:
                errors.append(str(error))
            continue
        lower_one(address)
    if errors is not None:
        return ""

    body = "\n".join(statements)
    lines = [
        "// Auto-generated by tools/sf2/generate_native_maps.py; do not edit by hand.",
        "//! Reviewed authored map scripts as a typed native catalog.",
        "",
        *(["use super::authored_paths;"] if "authored_paths::" in body else []),
        "use super::map_effects::{"
        + ", ".join(name for name in ("DisplayModeRequest", "ExitScene", "GsuParameter", "HeightLimit", "MapEffect", "MapSpawn", "PathEntry", "PathRecord", "PresentationByte")
                    if name in ("MapEffect", "MapSpawn") or f"{name}::" in body or f"{name} {{" in body)
        + "};",
        *(["use super::map_streaming::MapRegion;"] if "MapRegion {" in body else []),
        *(["use super::player_vertical::VerticalProfile;"] if "VerticalProfile {" in body else []),
        *(["use super::path_fields::Axis;"] if "Axis::" in body else []),
        *(["use super::player_camera_angles::CameraPitchProfile;"] if "CameraPitchProfile {" in body else []),
        "use super::scene_map::{",
        "    CatalogError, MapActorSpawn, MapCatalog, MapCondition, MapCursor, MapInstruction, PhaseExit,",
        "};",
        "use super::hit_response::HitSide;",
        "use super::{"
        + ", ".join(name for name in ("Angle", "Behavior", "ObjectKind", "ShapeId", "Vector3") if f"{name}::" in body or f"{name} {{" in body)
        + "};",
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
