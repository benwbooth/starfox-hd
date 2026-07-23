#!/usr/bin/env python3
"""Compact and verify retail Wall Spider activation and defeat evidence."""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path


DEFAULT_OUTPUT = Path(__file__).with_name("fixtures") / "wall_spider.trace"
MISSION_SELECTION = 4
CAMPAIGN_WORLD = "meteor"
ENCOUNTER_MAP = "05:4893"
PARENT_SHAPE = "EB50"
CORE_SHAPE = "EB6C"
CORE_OBJECT = "06F0"
WAITING_PATH = "5F92"
ACTIVE_PATH = "5FA5"
DAMAGED_PATH = "5FCC"
DEFEATED_PATH = "5FF2"
COSMETIC_PATH = "6002"
PARENT_FINAL_PATH = "5F36"
MAXIMUM_DURABILITY = 125
CLAMPED_DURABILITY = 3
DAMAGED_DURABILITY = 1
WALKER_SHAPE = "C94C"
FIRST_INTERMEDIATE_SHAPE = "C2F4"
SECOND_INTERMEDIATE_SHAPE = "C310"
FLIGHT_SHAPE = "C268"


@dataclass(frozen=True)
class Evidence:
    activation_sha256: str
    attack_sha256: str
    transformation_sha256: str
    trigger_partial_retail_frame: int
    trigger_armed_retail_frame: int
    active_retail_frame: int
    projectile_retail_frame: int
    damage_retail_frame: int
    objective_decrement_retail_frame: int
    parent_final_retail_frame: int
    transformation_input_retail_frame: int
    first_intermediate_retail_frame: int
    second_intermediate_retail_frame: int
    flight_retail_frame: int


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_records(path: Path) -> list[dict[str, str]]:
    return [fields(line) for line in path.read_text(encoding="utf-8").splitlines()]


def objects(record: dict[str, str]) -> list[list[str]]:
    parsed = []
    for encoded in (
        record.get("objects", "").removeprefix("[").removesuffix("]").split(";")
    ):
        if not encoded:
            continue
        values = encoded.split(",")
        if len(values) < 16:
            raise SystemExit(f"malformed oracle object record: {encoded}")
        parsed.append(values)
    return parsed


def object_by_address(record: dict[str, str], address: str) -> list[str] | None:
    return next((obj for obj in objects(record) if obj[0] == address), None)


def object_by_shape(
    record: dict[str, str], shape: str, *, exclude_address: str | None = None
) -> list[str] | None:
    return next(
        (
            obj
            for obj in objects(record)
            if obj[1] == shape and obj[0] != exclude_address
        ),
        None,
    )


def objective_count(record: dict[str, str]) -> int:
    mirrors = tuple(map(int, record["objectives"].split(",")))
    if len(mirrors) != 2 or mirrors[0] != mirrors[1]:
        raise SystemExit(f"retail objective mirrors disagree: {mirrors}")
    return mirrors[0]


def mission_samples(records: list[dict[str, str]]) -> list[dict[str, str]]:
    samples = [
        record
        for record in records
        if record.get("event") in {"state", "sortie", "checkpoint"}
        and record.get("selection") == str(MISSION_SELECTION)
        and record.get("map") == ENCOUNTER_MAP
        and "objects" in record
        and "objectives" in record
    ]
    if not samples:
        raise SystemExit("oracle trace has no Wall Spider mission samples")
    return samples


def relative_frame(record: dict[str, str], baseline: int) -> int:
    return int(record["elapsed"]) - baseline


def extract_activation(path: Path) -> tuple[str, int, int, int]:
    records = read_records(path)
    unexpected_events = {
        record.get("event")
        for record in records
        if record.get("event") not in {"state", "sortie", "checkpoint"}
    }
    if unexpected_events:
        raise SystemExit(
            f"activation trace contains oracle forcing events: {sorted(unexpected_events)}"
        )
    samples = mission_samples(records)
    if {sample.get("input") for sample in samples} - {
        "idle",
        "script-right",
        "script-up",
    }:
        raise SystemExit("activation trace uses unexpected input")
    if any(objective_count(sample) != 2 for sample in samples):
        raise SystemExit("Wall Spider activation changes campaign objectives")

    baseline = int(samples[0]["elapsed"])
    parent = object_by_shape(samples[0], PARENT_SHAPE)
    core = object_by_address(samples[0], CORE_OBJECT)
    if (
        parent is None
        or core is None
        or core[1] != CORE_SHAPE
        or parent[10] != "5F16"
        or core[10] != WAITING_PATH
        or int(core[15]) != MAXIMUM_DURABILITY
        or samples[0].get("coretrigger") != "0"
    ):
        raise SystemExit("activation trace lacks the dormant retail Wall Spider state")

    partial = next(
        (sample for sample in samples if sample.get("coretrigger") == "254"), None
    )
    armed = next(
        (sample for sample in samples if sample.get("coretrigger") == "255"), None
    )
    active = next(
        (
            sample
            for sample in samples
            if (core := object_by_address(sample, CORE_OBJECT)) is not None
            and core[1] == CORE_SHAPE
            and core[10] == ACTIVE_PATH
        ),
        None,
    )
    if partial is None or armed is None or active is None:
        raise SystemExit("activation trace never arms the retail Wall Spider core")
    partial_frame = relative_frame(partial, baseline)
    armed_frame = relative_frame(armed, baseline)
    active_frame = relative_frame(active, baseline)
    if not 0 < partial_frame < armed_frame < active_frame:
        raise SystemExit("Wall Spider activation edges are out of order")
    return digest(path), partial_frame, armed_frame, active_frame


def extract_attack(path: Path) -> tuple[str, int, int, int, int]:
    records = read_records(path)
    samples = mission_samples(records)
    baseline = int(samples[0]["elapsed"])

    binding = next(
        (
            record
            for record in records
            if record.get("event") == "forced-target-object-bound"
            and record.get("object") == CORE_OBJECT
            and record.get("shape") == CORE_SHAPE
        ),
        None,
    )
    if binding is None:
        raise SystemExit("attack trace is not bound to the original Wall Spider core")

    clamped = next(
        (
            sample
            for sample in samples
            if (core := object_by_address(sample, CORE_OBJECT)) is not None
            and core[1] == CORE_SHAPE
            and core[10] == ACTIVE_PATH
            and int(core[15]) == CLAMPED_DURABILITY
            and objective_count(sample) == 2
        ),
        None,
    )
    if clamped is None:
        raise SystemExit("attack trace lacks the accelerated active-core state")

    projectile = next(
        (
            record
            for record in records
            if record.get("event") == "forced-projectile-locked"
            and record.get("target") == CORE_OBJECT
            and record.get("target_shape") == CORE_SHAPE
        ),
        None,
    )
    damaged = next(
        (
            sample
            for sample in samples
            if (core := object_by_address(sample, CORE_OBJECT)) is not None
            and core[1] == CORE_SHAPE
            and core[10] == DAMAGED_PATH
            and int(core[15]) == DAMAGED_DURABILITY
            and objective_count(sample) == 2
        ),
        None,
    )
    if projectile is None or damaged is None:
        raise SystemExit("attack trace lacks a retail projectile hit reaction")

    decrements = [
        record
        for record in records
        if record.get("event") == "map-control-write"
        and record.get("address") in {"D7F4", "D7A1"}
        and record.get("value") == "1"
        and record.get("context", "").startswith(f"{CORE_OBJECT},")
    ]
    if {record["address"] for record in decrements} != {"D7F4", "D7A1"}:
        raise SystemExit("attack trace does not update both campaign-objective mirrors")
    decrement_elapsed = {record["elapsed"] for record in decrements}
    if len(decrement_elapsed) != 1:
        raise SystemExit("campaign-objective mirrors are not decremented together")

    defeated = next(
        (
            sample
            for sample in samples
            if objective_count(sample) == 1
            and (core := object_by_address(sample, CORE_OBJECT)) is not None
            and core[1] == CORE_SHAPE
            and core[10] == DEFEATED_PATH
            and int(core[15]) == DAMAGED_DURABILITY
            and (
                cosmetic := object_by_shape(
                    sample, CORE_SHAPE, exclude_address=CORE_OBJECT
                )
            )
            is not None
            and cosmetic[10] == COSMETIC_PATH
            and int(cosmetic[15]) == 0
        ),
        None,
    )
    parent_final = next(
        (
            sample
            for sample in samples
            if (parent := object_by_shape(sample, PARENT_SHAPE)) is not None
            and parent[10] == PARENT_FINAL_PATH
            and objective_count(sample) == 1
        ),
        None,
    )
    if defeated is None or parent_final is None:
        raise SystemExit("attack trace lacks the retail Wall Spider aftermath")

    projectile_frame = relative_frame(projectile, baseline)
    damage_frame = relative_frame(damaged, baseline)
    decrement_frame = int(decrement_elapsed.pop()) - baseline
    parent_final_frame = relative_frame(parent_final, baseline)
    if not 0 < projectile_frame < damage_frame < decrement_frame < parent_final_frame:
        raise SystemExit("Wall Spider attack and aftermath edges are out of order")
    return (
        digest(path),
        projectile_frame,
        damage_frame,
        decrement_frame,
        parent_final_frame,
    )


def extract_transformation(path: Path) -> tuple[str, int, int, int, int]:
    samples = mission_samples(read_records(path))
    baseline = int(samples[0]["elapsed"])
    if any(objective_count(sample) != 1 for sample in samples):
        raise SystemExit("transformation trace changes campaign objectives")

    def first_player_sample(*, shape: str | None = None, input_name: str | None = None):
        return next(
            (
                sample
                for sample in samples
                if (input_name is None or sample.get("input") == input_name)
                and (player := object_by_address(sample, sample["player"])) is not None
                and (shape is None or player[1] == shape)
            ),
            None,
        )

    initial = first_player_sample(shape=WALKER_SHAPE)
    transformation_input = first_player_sample(input_name="script-select")
    first_intermediate = first_player_sample(shape=FIRST_INTERMEDIATE_SHAPE)
    second_intermediate = first_player_sample(shape=SECOND_INTERMEDIATE_SHAPE)
    flight = first_player_sample(shape=FLIGHT_SHAPE)
    if None in (
        initial,
        transformation_input,
        first_intermediate,
        second_intermediate,
        flight,
    ):
        raise SystemExit("transformation trace lacks the retail craft-form sequence")

    input_frame = relative_frame(transformation_input, baseline)
    first_frame = relative_frame(first_intermediate, baseline)
    second_frame = relative_frame(second_intermediate, baseline)
    flight_frame = relative_frame(flight, baseline)
    if not 0 < input_frame < first_frame < second_frame < flight_frame:
        raise SystemExit("Wall Spider post-fight transformation is out of order")
    return digest(path), input_frame, first_frame, second_frame, flight_frame


def extract(activation: Path, attack: Path, transformation: Path) -> Evidence:
    activation_values = extract_activation(activation)
    attack_values = extract_attack(attack)
    transformation_values = extract_transformation(transformation)
    return Evidence(
        activation_sha256=activation_values[0],
        attack_sha256=attack_values[0],
        transformation_sha256=transformation_values[0],
        trigger_partial_retail_frame=activation_values[1],
        trigger_armed_retail_frame=activation_values[2],
        active_retail_frame=activation_values[3],
        projectile_retail_frame=attack_values[1],
        damage_retail_frame=attack_values[2],
        objective_decrement_retail_frame=attack_values[3],
        parent_final_retail_frame=attack_values[4],
        transformation_input_retail_frame=transformation_values[1],
        first_intermediate_retail_frame=transformation_values[2],
        second_intermediate_retail_frame=transformation_values[3],
        flight_retail_frame=transformation_values[4],
    )


def render(evidence: Evidence) -> str:
    return "\n".join(
        [
            "# Compact Mesen oracle evidence for the retail Wall Spider encounter.",
            f"# Natural activation SHA-256: {evidence.activation_sha256}",
            f"# Exact-core attack SHA-256: {evidence.attack_sha256}",
            f"# Post-fight transformation SHA-256: {evidence.transformation_sha256}",
            (
                f"encounter world={CAMPAIGN_WORLD} mission_selection={MISSION_SELECTION} "
                f"encounter_map={ENCOUNTER_MAP.split(':')[1]} parent_shape={PARENT_SHAPE} "
                f"core_shape={CORE_SHAPE} core_maximum_durability={MAXIMUM_DURABILITY}"
            ),
            (
                "activation input=right_then_forward "
                f"trigger_partial_retail_frame={evidence.trigger_partial_retail_frame} "
                f"trigger_armed_retail_frame={evidence.trigger_armed_retail_frame} "
                f"active_retail_frame={evidence.active_retail_frame} "
                f"waiting_path={WAITING_PATH} active_path={ACTIVE_PATH}"
            ),
            (
                f"attack target_object={CORE_OBJECT} "
                "oracle_acceleration=health_clamp_and_projectile_reposition "
                f"starting_durability={CLAMPED_DURABILITY} "
                f"projectile_retail_frame={evidence.projectile_retail_frame} "
                f"damage_retail_frame={evidence.damage_retail_frame} "
                f"damaged_durability={DAMAGED_DURABILITY} damaged_path={DAMAGED_PATH}"
            ),
            (
                "campaign objective_mirrors_before=2 objective_mirrors_after=1 "
                f"decrement_retail_frame={evidence.objective_decrement_retail_frame}"
            ),
            (
                f"aftermath core_path={DEFEATED_PATH} core_durability={DAMAGED_DURABILITY} "
                f"cosmetic_shape={CORE_SHAPE} cosmetic_path={COSMETIC_PATH} "
                f"parent_final_path={PARENT_FINAL_PATH} "
                f"parent_final_retail_frame={evidence.parent_final_retail_frame}"
            ),
            (
                f"transformation input=select walker_shape={WALKER_SHAPE} "
                f"input_retail_frame={evidence.transformation_input_retail_frame} "
                f"first_intermediate_shape={FIRST_INTERMEDIATE_SHAPE} "
                f"first_intermediate_retail_frame={evidence.first_intermediate_retail_frame} "
                f"second_intermediate_shape={SECOND_INTERMEDIATE_SHAPE} "
                f"second_intermediate_retail_frame={evidence.second_intermediate_retail_frame} "
                f"flight_shape={FLIGHT_SHAPE} flight_retail_frame={evidence.flight_retail_frame}"
            ),
            "",
        ]
    )


def validate_compact(path: Path) -> None:
    content = path.read_text(encoding="utf-8")
    lines = [line for line in content.splitlines() if line and not line.startswith("#")]
    if len(lines) != 6:
        raise SystemExit("Wall Spider fixture has an unexpected record count")
    encounter, activation, attack, campaign, aftermath, transformation = [
        fields(line) for line in lines
    ]
    if encounter != {
        "world": CAMPAIGN_WORLD,
        "mission_selection": str(MISSION_SELECTION),
        "encounter_map": ENCOUNTER_MAP.split(":")[1],
        "parent_shape": PARENT_SHAPE,
        "core_shape": CORE_SHAPE,
        "core_maximum_durability": str(MAXIMUM_DURABILITY),
    }:
        raise SystemExit("Wall Spider encounter fixture is inconsistent")
    if activation != {
        "input": "right_then_forward",
        "trigger_partial_retail_frame": "167",
        "trigger_armed_retail_frame": "475",
        "active_retail_frame": "483",
        "waiting_path": WAITING_PATH,
        "active_path": ACTIVE_PATH,
    }:
        raise SystemExit("Wall Spider activation fixture is inconsistent")
    if attack != {
        "target_object": CORE_OBJECT,
        "oracle_acceleration": "health_clamp_and_projectile_reposition",
        "starting_durability": str(CLAMPED_DURABILITY),
        "projectile_retail_frame": "104",
        "damage_retail_frame": "111",
        "damaged_durability": str(DAMAGED_DURABILITY),
        "damaged_path": DAMAGED_PATH,
    }:
        raise SystemExit("Wall Spider attack fixture is inconsistent")
    if campaign != {
        "objective_mirrors_before": "2",
        "objective_mirrors_after": "1",
        "decrement_retail_frame": "116",
    }:
        raise SystemExit("Wall Spider campaign-objective fixture is inconsistent")
    if aftermath != {
        "core_path": DEFEATED_PATH,
        "core_durability": str(DAMAGED_DURABILITY),
        "cosmetic_shape": CORE_SHAPE,
        "cosmetic_path": COSMETIC_PATH,
        "parent_final_path": PARENT_FINAL_PATH,
        "parent_final_retail_frame": "131",
    }:
        raise SystemExit("Wall Spider aftermath fixture is inconsistent")
    if transformation != {
        "input": "select",
        "walker_shape": WALKER_SHAPE,
        "input_retail_frame": "3",
        "first_intermediate_shape": FIRST_INTERMEDIATE_SHAPE,
        "first_intermediate_retail_frame": "15",
        "second_intermediate_shape": SECOND_INTERMEDIATE_SHAPE,
        "second_intermediate_retail_frame": "47",
        "flight_shape": FLIGHT_SHAPE,
        "flight_retail_frame": "71",
    }:
        raise SystemExit("Wall Spider transformation fixture is inconsistent")
    hashes = [line.rsplit(" ", 1)[-1] for line in content.splitlines()[1:4]]
    if any(
        len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
        for value in hashes
    ):
        raise SystemExit("Wall Spider fixture has an invalid source digest")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--activation-source", type=Path)
    parser.add_argument("--attack-source", type=Path)
    parser.add_argument("--transformation-source", type=Path)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    sources = (
        args.activation_source,
        args.attack_source,
        args.transformation_source,
    )
    if any(sources):
        if not all(sources):
            raise SystemExit("all three oracle source traces are required")
        generated = render(extract(*sources))
        if args.check:
            if (
                not args.output.is_file()
                or args.output.read_text(encoding="utf-8") != generated
            ):
                raise SystemExit(f"compact fixture is out of date: {args.output}")
            action = "verified"
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(generated, encoding="utf-8")
            action = "generated"
    else:
        validate_compact(args.output)
        action = "verified"
    print(
        f"{action} {args.output}: retail Wall Spider activation, defeat, and transformation"
    )


if __name__ == "__main__":
    main()
