"""Shared static certification for SF2 hostile-projectile generators."""

from __future__ import annotations

import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DISASM_DIRECTORY = Path(__file__).with_name("disasm")
sys.path.insert(0, str(DISASM_DIRECTORY))

from dump_runtime_routine import source_offset  # noqa: E402
from extract_map import DEFAULT_ROM  # noqa: E402
from extract_path import PathAddress, PathExtractor  # noqa: E402
from path_semantics import PATH_SEMANTICS  # noqa: E402


def validate_static_hostile_projectile_path() -> None:
    """Require the reviewed retail bytecode behind the semantic action model."""
    extractor = PathExtractor(Path(DEFAULT_ROM).read_bytes())
    semantic_names = {spec.opcode: spec.rust_name for spec in PATH_SEMANTICS}
    expected = {
        0xEE9B: ("SetVelocity", "063f"),
        0xEE9D: ("IfSelectedDistanceLess", "14e803adee"),
        0xEEA2: ("RotateAroundSelectedPitch", "ae7f"),
        0xEEA4: ("RotateAroundSelectedPitch", "ae7f"),
        0xEEA6: ("RotateAroundSelectedPitch", "ae7f"),
        0xEEA8: ("FaceSelectedImmediate", "000f"),
        0xEEAD: ("FaceSelectedImmediate", "000f"),
        0xEEAF: ("SetVelocity", "063f"),
        0xEEC6: ("Next", "44"),
        0xEED2: ("Wait", "030f"),
        0xEEDA: ("FaceSelectedSmooth", "09"),
    }
    for address, (semantic_name, raw_hex) in expected.items():
        command = extractor.decode_command(PathAddress(address))
        actual = (semantic_names.get(command.opcode), command.raw_hex)
        if actual != (semantic_name, raw_hex):
            raise SystemExit(
                f"hostile projectile path changed at {address:04X}: "
                f"expected {(semantic_name, raw_hex)}, found {actual}"
            )


def validate_static_collision_gate() -> None:
    """Require the retail gate and source-level meaning behind eligibility."""
    rom = Path(DEFAULT_ROM).read_bytes()
    gate_address = 0x7F32CE
    expected_gate = bytes.fromhex("B5 21 29 01 00 D0 60")
    gate_offset = source_offset(gate_address)
    actual_gate = rom[gate_offset : gate_offset + len(expected_gate)]
    if actual_gate != expected_gate:
        raise SystemExit(
            "hostile projectile collision-list gate changed at 7F:32CE: "
            f"expected {expected_gate.hex()}, found {actual_gate.hex()}"
        )

    path_source = (
        REPO_ROOT / "reference" / "ultrastarfox" / "SF" / "PATH" / "PATHS.ASM"
    ).read_text(encoding="latin-1")
    strategy_flags = (
        REPO_ROOT / "reference" / "ultrastarfox" / "SF" / "INC" / "STRATEQU.INC"
    ).read_text(encoding="latin-1")
    required_path_source = (
        ".collisionson SHORTA",
        "s_clr_alsflag\tx,colldisable",
        ".collisionsoff SHORTA",
        "s_set_alsflag\tx,colldisable",
    )
    if any(text not in path_source for text in required_path_source):
        raise SystemExit("reference collision enable/disable path semantics changed")
    if "make_sflag\tcolldisable" not in strategy_flags:
        raise SystemExit("reference collision-disable strategy flag changed")


def read_collision_eligibility(
    path: Path,
    expected_count: int,
    encounter_description: str,
) -> list[bool]:
    """Read one semantic collision decision per chronological projectile."""
    eligibility = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("track="):
            continue
        values = dict(
            token.split("=", 1)
            for token in line.split()
            if "=" in token
        )
        expected_track = len(eligibility)
        if int(values["track"]) != expected_track:
            raise SystemExit(
                f"{encounter_description} projectile collision fixture is not sequential: "
                f"expected track {expected_track}, found {values['track']}"
            )
        enabled = values.get("collision_enabled")
        if enabled not in ("true", "false"):
            raise SystemExit(
                f"{encounter_description} projectile track {expected_track} has invalid "
                f"collision_enabled={enabled}"
            )
        eligibility.append(enabled == "true")
    if len(eligibility) != expected_count:
        raise SystemExit(
            f"expected {expected_count} {encounter_description} collision entries, "
            f"found {len(eligibility)}"
        )
    return eligibility
