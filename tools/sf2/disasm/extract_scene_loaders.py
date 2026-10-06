#!/usr/bin/env python3
"""Recover scene-loader control flow and embedded artwork descriptors.

Analysis only, not a runtime instruction interpreter. The authored map's load
requests select null-terminated lists of *bank-first* function pointers. Artwork
loader helpers consume seven inline data bytes by adjusting their caller's
return address. Ordinary linear disassembly wrongly executes those bytes.

This inventory retains both conditional branches, repeated asset uses, call
boundaries and polling loops. It makes no claim about interrupt/DMA completion,
loader reachability outside the discovered maps, or native implementation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import asdict, dataclass
from pathlib import Path

import cpu65816 as cpu
from extract_map import DEFAULT_ROM, MapExtractor


TABLE = 0x03D774
TABLE_END = 0x03D843  # End of the last null-terminated entry (offset $CC).
CODE_START, CODE_END = 0x03C7B3, 0x03D570
MAX_NODES = 8192
ROOT = Path(__file__).resolve().parents[3]
PACKET_FIXTURE = ROOT / "rust/sf-oracle/tests/fixtures/sf2_scene_artwork_packets.rs"

# These are reviewed source summaries, not assumed calling conventions. The
# four setup helpers end through D5E8/D5F9 (M=1/X=0). D674/D6B1 consume seven
# bytes and return M=1 while retaining X. D6E7 explicitly returns M=1/X=0.
# Full-body gates include their immediate shared helpers and data readers.
SOURCE_GATES = (
    (0x019D4E, 0x019D96, "307350b3ee56242d48f33530bf98a86b00341998b76f2abac021285a5bdf1913"),
    (0x01D570, 0x01D60A, "caef15cc046ac67ff8918e095993c99ff1b394c216a07aaa6730d0098ae60223"),
    (0x01D60A, 0x01D674, "7ef35dc2122d903e10cc5f1e183817b4354da54201477906e1e844fe16c8527a"),
    (0x01D674, 0x01D716, "bc443d20f88d17a76651ad5adccc70e27307d3d0748c94151257298c42adb8c3"),
)
SETUP_HELPERS = frozenset((0x03D570, 0x03D58E, 0x03D5AC, 0x03D5CA))
PACKET_HELPERS = {0x03D674: "characters", 0x03D6B1: "tile_map"}
PALETTE_HELPER = 0x03D6E7


@dataclass(frozen=True)
class ArtworkPacket:
    call: int
    kind: str
    compressed_end: int
    destination: int | None
    transfer_bytes: int
    decoded_bytes: int


@dataclass(frozen=True)
class LoaderNode:
    address: int
    accumulator_bytes: int
    index_bytes: int
    raw_hex: str
    mnemonic: str
    operand: int
    successors: tuple[int, ...]
    call_target: int | None = None
    packet: ArtworkPacket | None = None


@dataclass(frozen=True)
class SceneLoad:
    table_offset: int
    requested_by: tuple[int, ...]
    routines: tuple[int, ...]


@dataclass(frozen=True)
class SceneLoaders:
    requests: tuple[SceneLoad, ...]
    nodes: tuple[LoaderNode, ...]

    def reachable(self, root: int) -> tuple[LoaderNode, ...]:
        nodes = {node.address: node for node in self.nodes}
        pending, visited = [root], set()
        while pending:
            address = pending.pop()
            if address in visited:
                continue
            visited.add(address)
            if address not in nodes:
                raise ValueError(f"missing loader node {address:06X}")
            pending.extend(nodes[address].successors)
        return tuple(nodes[address] for address in sorted(visited))


def rom_bytes(rom: bytes, address: int, count: int) -> bytes:
    offset = cpu.cpu_to_file(address)
    if offset is None or address & 65535 < 32768:
        raise ValueError(f"not mapped ROM: {address:06X}")
    if (address & 65535) + count > 65536 or offset + count > len(rom):
        raise ValueError(f"truncated ROM record: {address:06X} length {count}")
    return rom[offset:offset + count]


def load_sequence(rom: bytes, table_offset: int) -> tuple[int, ...]:
    if table_offset < 0 or table_offset % 3:
        raise ValueError(f"unaligned loader-table offset {table_offset}")
    address, result = TABLE + table_offset, []
    while TABLE <= address and address + 3 <= TABLE_END:
        record = rom_bytes(rom, address, 3)
        if record == bytes(3):
            return tuple(result)
        target = record[0] << 16 | int.from_bytes(record[1:], "little")
        if not CODE_START <= target < CODE_END:
            raise ValueError(f"loader target outside reviewed code: {target:06X}")
        result.append(target)
        address += 3
    raise ValueError("unterminated or out-of-range loader table")


def artwork_packet(rom: bytes, call: int, helper: int) -> ArtworkPacket:
    record = rom_bytes(rom, call + 3, 7)
    end = int.from_bytes(record[:3], "little")
    # The backward stream ends *before* the supplied pointer. Its final word
    # is big-endian decoded length. Transfer size is a separate little-endian
    # field and need not equal that length (character streams include padding).
    length = int.from_bytes(rom_bytes(rom, end - 2, 2), "big")
    destination = int.from_bytes(record[3:5], "little")
    transfer = int.from_bytes(record[5:7], "little")
    if transfer == 0 or transfer > length:
        raise ValueError(f"artwork transfer exceeds decoded stream at {call:06X}")
    return ArtworkPacket(call, PACKET_HELPERS[helper], end, destination or None,
                         transfer, length)


def extract_scene_loaders(rom: bytes) -> SceneLoaders:
    for start, stop, expected in SOURCE_GATES:
        if hashlib.sha256(rom[start:stop]).hexdigest() != expected:
            raise ValueError(f"scene-loader source signature mismatch at file {start:#x}")
    maps = MapExtractor(rom).extract()
    if maps.invalid_opcodes or maps.unresolved_inline_exits:
        raise ValueError("incomplete source map graph")
    requests: dict[int, list[int]] = {}
    for command in maps.commands:
        if command.opcode == 0x10:
            value = int.from_bytes(bytes.fromhex(command.raw_hex)[1:3], "little")
            requests.setdefault(value, []).append(command.address.cpu)
    loads = tuple(SceneLoad(value, tuple(sorted(callers)), load_sequence(rom, value))
                  for value, callers in sorted(requests.items()))
    pending = [(root, 1, 0) for load in loads for root in load.routines]
    nodes: dict[int, LoaderNode] = {}
    occupied: dict[int, int] = {}
    widths: dict[int, tuple[int, int]] = {}
    while pending:
        address, m_flag, x_flag = pending.pop()
        if address in widths:
            if widths[address] != (m_flag, x_flag):
                raise ValueError(f"conflicting instruction widths at {address:06X}")
            continue
        if len(nodes) >= MAX_NODES:
            raise ValueError("scene-loader graph exceeds node budget")
        if not CODE_START <= address < CODE_END:
            raise ValueError(f"control flow outside reviewed loaders: {address:06X}")
        # Decode length first, then require every instruction byte to exist.
        offset = cpu.cpu_to_file(address)
        rom_bytes(rom, address, 1)
        ins = cpu.decode_one(rom, offset, m_flag, x_flag)
        raw = rom_bytes(rom, address, ins.length)
        next_m, next_x = cpu.update_flags(ins, m_flag, x_flag)
        following = address + ins.length
        target, packet = None, None
        if ins.mnem in cpu.CALLS:
            target = ins.target
            if target in PACKET_HELPERS and ins.mnem == "JSR":
                packet = artwork_packet(rom, address, target)
                following += 7
                next_m = 1
            elif target in SETUP_HELPERS and ins.mnem == "JSR":
                next_m, next_x = 1, 0
            elif target == PALETTE_HELPER and ins.mnem == "JSL":
                next_m, next_x = 1, 0
            else:
                raise ValueError(f"unreviewed loader call at {address:06X}: {target!r}")
        if ins.mnem in {"BRK", "COP", "STP", "RTI", "RTS", "PLP", "XCE"}:
            raise ValueError(f"unsupported loader control at {address:06X}: {ins.mnem}")
        if ins.mnem == "RTL":
            successors = ()
        elif ins.mnem in cpu.BRANCHES | cpu.JUMPS:
            if ins.target is None:
                raise ValueError(f"unresolved loader branch at {address:06X}")
            successors = ((ins.target,) if ins.mnem in {"BRA", "BRL", "JMP", "JML"}
                          else (following, ins.target))
        else:
            successors = (following,)
        # Packet payload belongs to its call node, never to an instruction.
        for byte in range(address, following):
            if byte in occupied and occupied[byte] != address:
                raise ValueError(f"overlapping instruction or inline packet at {byte:06X}")
            occupied[byte] = address
        widths[address] = (m_flag, x_flag)
        nodes[address] = LoaderNode(address, 1 if m_flag else 2, 1 if x_flag else 2,
                                    raw.hex(), ins.mnem, ins.operand, successors, target, packet)
        pending.extend((successor, next_m, next_x) for successor in successors)
    return SceneLoaders(loads, tuple(nodes[key] for key in sorted(nodes)))


def render_packet_fixture(result: SceneLoaders) -> str:
    lines = [
        "// Generated by tools/sf2/disasm/extract_scene_loaders.py --write-fixture.",
        "// Source operands only. Expected decompression bytes come from the original GSU.",
        "// (call, helper, stream end, explicit destination, transfer bytes, decoded bytes)",
        "const PACKETS: &[(u32, u16, u32, u16, usize, usize)] = &[",
    ]
    for node in result.nodes:
        if (packet := node.packet) is not None:
            lines.append(f"    (0x{packet.call:06X}, 0x{node.call_target & 65535:04X}, "
                         f"0x{packet.compressed_end:06X}, {packet.destination or 0}, "
                         f"{packet.transfer_bytes}, {packet.decoded_bytes}),")
    lines.extend([
        "];",
        "// Complete reachable loader instruction and inline-data footprint.",
        "const SOURCE_SHA256: &str = \"" + footprint_digest(result) + "\";",
        "const SOURCE_SPANS: &[(u32, usize)] = &[",
    ])
    for start, length in footprint_spans(result):
        lines.append(f"    (0x{start:06X}, {length}),")
    lines.extend(["];", ""])
    return "\n".join(lines)


def footprint_spans(result: SceneLoaders) -> tuple[tuple[int, int], ...]:
    spans = []
    for node in result.nodes:
        length = len(bytes.fromhex(node.raw_hex)) + (7 if node.packet else 0)
        if spans and spans[-1][0] + spans[-1][1] == node.address:
            start, old_length = spans.pop()
            spans.append((start, old_length + length))
        else:
            spans.append((node.address, length))
    return tuple(spans)


def footprint_digest(result: SceneLoaders) -> str:
    # Includes the descriptor's exact original bytes, not runtime output.
    digest = hashlib.sha256()
    for node in result.nodes:
        digest.update(bytes.fromhex(node.raw_hex))
        if (packet := node.packet) is not None:
            digest.update(packet.compressed_end.to_bytes(3, "little"))
            digest.update((packet.destination or 0).to_bytes(2, "little"))
            digest.update(packet.transfer_bytes.to_bytes(2, "little"))
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--json", action="store_true")
    output = parser.add_mutually_exclusive_group()
    output.add_argument("--write-fixture", action="store_true")
    output.add_argument("--check", action="store_true")
    args = parser.parse_args()
    result = extract_scene_loaders(args.rom.read_bytes())
    if args.write_fixture:
        PACKET_FIXTURE.parent.mkdir(parents=True, exist_ok=True)
        PACKET_FIXTURE.write_text(render_packet_fixture(result))
    if args.check and (not PACKET_FIXTURE.exists()
                       or PACKET_FIXTURE.read_text() != render_packet_fixture(result)):
        raise SystemExit("scene artwork source fixture is stale")
    if args.json:
        print(json.dumps(asdict(result), indent=2))
        return
    for load in result.requests:
        print(f"load {load.table_offset:04X}: "
              + ", ".join(f"{root:06X}" for root in load.routines)
              + f" ({len(load.requested_by)} map requests)")
    packets = [node.packet for node in result.nodes if node.packet is not None]
    print(f"{len(result.nodes)} instruction nodes, {len(packets)} artwork packets, "
          f"{len({packet.compressed_end for packet in packets})} distinct artwork streams")
    print("Source inventory only; native scene installation is not established.")


if __name__ == "__main__":
    main()
