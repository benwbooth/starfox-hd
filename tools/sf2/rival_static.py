"""Static retail-path certification shared by SF2 rival generators."""

from __future__ import annotations

import sys
from pathlib import Path


DISASM_DIRECTORY = Path(__file__).with_name("disasm")
sys.path.insert(0, str(DISASM_DIRECTORY))

from extract_map import DEFAULT_ROM  # noqa: E402
from extract_path import PathAddress, PathExtractor  # noqa: E402
from path_semantics import PATH_SEMANTICS  # noqa: E402


def validate_static_leon_pressure_path() -> None:
    """Require every reviewed retail command used by Leon's pressure flight."""
    result = PathExtractor(Path(DEFAULT_ROM).read_bytes()).extract()
    commands = {command.address.offset: command for command in result.commands}
    semantic_names = {spec.opcode: spec.rust_name for spec in PATH_SEMANTICS}
    expected = {
        # The extended dispatcher consumes the zero escape before entering
        # FaceSelectedImmediate, so runtime logs report pointer 00DE while
        # the complete static command begins at 00DD.
        0x00DD: ("FaceSelectedImmediate", "000f"),
        0x00EB: ("ScheduleRelative", "fda100"),
        0x0169: ("SetObjectBytes0a0b", "186401"),
        0x0174: ("IncrementWord", "6e8e"),
        0x0194: ("FacePlayerYaw", "0a"),
        0x027E: ("WaitAchaseByte", "830016"),
        0x028F: ("IfNot", "8a"),
        0x029A: ("FaceSelectedSmooth", "09"),
        0x029E: ("FacePlayerYaw", "0a"),
        0x872C: ("PushByte", "93a1"),
        0x872E: ("ImportByteIndexed", "7aa179"),
        0x8731: ("IfNotZeroByte", "69a13a87"),
        0x8735: ("PullByte", "95a1"),
        0x8737: ("Goto", "162c87"),
        0x873A: ("PullByte", "95a1"),
        0x873C: ("Return", "42"),
    }
    for address, (expected_name, expected_raw) in expected.items():
        command = commands.get(address)
        if command is None:
            raise SystemExit(
                f"Leon pressure retail path command {address:04X} is no longer reachable"
            )
        actual = (semantic_names.get(command.opcode), command.raw_hex)
        if actual != (expected_name, expected_raw):
            raise SystemExit(
                f"Leon pressure retail path changed at {address:04X}: "
                f"expected {(expected_name, expected_raw)}, found {actual}"
            )

    expected_loop = {
        0x872C: (0x872E,),
        0x872E: (0x8731,),
        0x8731: (0x8735, 0x873A),
        0x8735: (0x8737,),
        0x8737: (0x872C,),
        0x873A: (0x873C,),
        0x873C: (),
    }
    for address, expected_successors in expected_loop.items():
        actual_successors = tuple(
            successor.offset for successor in commands[address].successors
        )
        if actual_successors != expected_successors:
            raise SystemExit(
                f"Leon pressure setup flow changed at {address:04X}: "
                f"expected {expected_successors}, found {actual_successors}"
            )

    schedule = commands[0x00EB]
    if schedule.successors != (PathAddress(0x00EE), PathAddress(0x018C)):
        raise SystemExit("Leon pressure relative schedule targets changed")
