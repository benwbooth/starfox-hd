#!/usr/bin/env python3
"""Recover the timed player/controller stream paired with each scene root.

This is source analysis, not a shipping interpreter. A complete path graph
alone does not cover these parallel native service calls or their effects.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from enum import IntEnum
from pathlib import Path

from extract_intro_paths import authored_scene_roots
from extract_map import DEFAULT_ROM


class TimingCondition(IntEnum):
    AT = 0
    FROM = 1
    BEFORE = 2
    INTERVAL = 3
    ALWAYS = 4


@dataclass(frozen=True)
class TimedService:
    address: int
    condition: TimingCondition
    start: int
    stop: int | None
    service: int

    def applies(self, tick: int) -> bool:
        if not 0 <= tick <= 65535:
            raise ValueError("controller tick outside source unsigned-word range")
        if self.condition == TimingCondition.AT:
            return tick == self.start
        if self.condition == TimingCondition.FROM:
            return tick >= self.start
        if self.condition == TimingCondition.BEFORE:
            return tick < self.start
        if self.condition == TimingCondition.INTERVAL:
            return self.start <= tick < self.stop
        return True


@dataclass(frozen=True)
class SceneController:
    path_root: int
    script: int
    commands: tuple[TimedService, ...]


def authored_scene_controller(rom: bytes, index: int) -> SceneController:
    roots = authored_scene_roots(rom)
    if not 0 <= index < len(roots):
        raise ValueError("scene index outside authored table")
    # The neighboring three bytes, loaded by overlapping word copies, supply
    # the controller stream pointer. They are not part of the actor path.
    signatures = (
        (0x03294A, "bfc9d40d99136cbfcad40d99146c"),
        (0x06BD2E, "e60229ff00c90400f017c90300f04bc90100f026c90200f031c90000f00c"),
        (0x06BD5E, "ada51dc702d04f4ca0bd"),
        (0x06BD6E, "ada51dc702903f4ca0bd"),
        (0x06BD7E, "ada51dc702b02f4ca0bd"),
        (0x06BD8E, "ada51dc702901fe602e602c702b017"),
        (0x06BDA0, "e602e602a702853aa9b2bd3a48e2206c3a00"),
    )
    for offset, signature in signatures:
        expected = bytes.fromhex(signature)
        if rom[offset:offset + len(expected)] != expected:
            raise ValueError(f"controller source signature mismatch at file {offset:#x}")
    record = 0x06D4C7 + index * 8
    pointer = int.from_bytes(rom[record + 2:record + 5], "little")
    bank = pointer >> 16
    start = pointer & 65535
    if bank & 127 >= 126 or start < 32768:
        raise ValueError(f"controller pointer is not mapped ROM: {pointer:#x}")
    file_base = (bank & 127) * 32768
    cursor = start
    commands = []
    for _ in range(1024):
        offset = file_base + cursor - 32768
        if cursor > 65534 or offset + 2 > len(rom):
            raise ValueError("truncated controller stream")
        if int.from_bytes(rom[offset:offset + 2], "little") in (16383, 32767, 65535):
            return SceneController(roots[index], pointer, tuple(commands))
        try:
            condition = TimingCondition(rom[offset])
        except ValueError as error:
            raise ValueError(f"unknown timing condition at {bank:02X}:{cursor:04X}") from error
        length = 7 if condition == TimingCondition.INTERVAL else 5
        if cursor + length > 65536 or offset + length > len(rom):
            raise ValueError("truncated controller record")
        start_tick = int.from_bytes(rom[offset + 1:offset + 3], "little")
        stop_tick = (int.from_bytes(rom[offset + 3:offset + 5], "little")
                     if condition == TimingCondition.INTERVAL else None)
        service = int.from_bytes(rom[offset + length - 2:offset + length], "little")
        if service < 32768:
            raise ValueError("controller service outside the native code bank")
        commands.append(TimedService((bank << 16) | cursor, condition, start_tick,
                                     stop_tick, 0x0D0000 | service))
        cursor += length
    raise ValueError("controller stream exceeds bounded record count")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--scene", type=int, default=6)
    args = parser.parse_args()
    scene = authored_scene_controller(args.rom.read_bytes(), args.scene)
    print(f"scene={args.scene} path=44:{scene.path_root:04X} controller={scene.script:06X} "
          f"services={len(scene.commands)}")
    for command in scene.commands:
        condition = f"{command.condition.name} {command.start}"
        if command.stop is not None:
            condition += f"..{command.stop} (end exclusive)"
        print(f"{command.address:06X} {condition} service={command.service:06X}")


if __name__ == "__main__":
    main()
