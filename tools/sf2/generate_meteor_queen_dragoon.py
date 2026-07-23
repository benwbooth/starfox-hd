#!/usr/bin/env python3
"""Compact and verify Meteor's retail Queen Dragoon oracle evidence."""

from __future__ import annotations

import argparse
import hashlib
from collections import Counter
from dataclasses import dataclass
from pathlib import Path


DEFAULT_OUTPUT = Path(__file__).with_name("fixtures") / "meteor_queen_dragoon.trace"
MISSION_NAME = "meteor"
MISSION_SELECTION = 4
ACTIVE_MAP = "05:4012"
BOSS_NAME = "queen_dragoon"
BODY_SHAPE = "FA84"
LINKED_COMPONENT_SHAPES = ("FAA0", "FABC")
BODY_EXPLOSION_SHAPE = "BDD0"
COMPONENT_BURST_SHAPE = "BD98"
COMPONENT_DEBRIS_SHAPE = "BDEC"
MAXIMUM_DURABILITY = 80
EXPECTED_LINKED_COMPONENTS = 4
EXPECTED_LOGIC_OBJECTS = 5
EXPECTED_MOVE_EVENTS_PER_OBJECT = 4
EXPECTED_MOVE_EVENTS = EXPECTED_LOGIC_OBJECTS * EXPECTED_MOVE_EVENTS_PER_OBJECT


@dataclass(frozen=True)
class Evidence:
    sortie_sha256: str
    actor_logic_sha256: str
    forced_return_sha256: str
    objectives_before: int
    objectives_after: int
    defeat_retail_frame: int
    explosion_retail_frame: int
    forced_return_delay_retail_frames: int
    move_events: int


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def objects(value: str) -> list[list[str]]:
    records = []
    for record in value.removeprefix("[").removesuffix("]").split(";"):
        if not record:
            continue
        parts = record.split(",")
        if len(parts) < 16:
            raise SystemExit(f"malformed oracle object record: {record}")
        records.append(parts)
    return records


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def objective_count(values: dict[str, str]) -> int:
    mirrors = tuple(map(int, values["objectives"].split(",")))
    if len(mirrors) != 2 or mirrors[0] != mirrors[1]:
        raise SystemExit(f"retail objective mirrors disagree: {mirrors}")
    return mirrors[0]


def extract_forced_return(forced_return_trace: Path) -> int:
    records = [
        fields(line)
        for line in forced_return_trace.read_text(encoding="utf-8").splitlines()
    ]
    completed_index, completed = next(
        (
            (index, record)
            for index, record in enumerate(records)
            if record.get("event") == "objective-remaining-forced"
            and record.get("remaining") == "0"
        ),
        (None, None),
    )
    if completed_index is None or completed is None:
        raise SystemExit("forced-return trace lacks its objective-count injection")
    completed_elapsed = int(completed["elapsed"])
    prior_objectives = next(
        (
            record.get("objectives")
            for record in reversed(records[:completed_index])
            if "objectives" in record
        ),
        None,
    )
    if prior_objectives != "2,2":
        raise SystemExit(
            "forced-return trace does not begin with two Meteor objectives"
        )
    returned = next(
        (
            record
            for record in records
            if int(record.get("elapsed", "0")) > completed_elapsed
            and record.get("event") == "state"
            and record.get("mode") == "7"
        ),
        None,
    )
    if returned is None:
        raise SystemExit("forced-return trace never reaches the strategic map")
    if returned.get("map") != ACTIVE_MAP:
        raise SystemExit(
            "Meteor unexpectedly changes active maps before campaign return"
        )
    route_records = [
        record
        for record in records[completed_index + 1 :]
        if int(record.get("elapsed", "0")) <= int(returned["elapsed"])
    ]
    if any(record.get("map", ACTIVE_MAP) != ACTIVE_MAP for record in route_records):
        raise SystemExit("Meteor changes active maps during its return flight")
    if any(record.get("input") not in {None, "script-up"} for record in route_records):
        raise SystemExit("Meteor forced-return route uses unexpected oracle input")
    return int(returned["elapsed"]) - completed_elapsed


def extract(
    sortie_trace: Path, actor_logic_trace: Path, forced_return_trace: Path
) -> Evidence:
    samples = []
    for line in sortie_trace.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if (
            values.get("selection") == str(MISSION_SELECTION)
            and values.get("map") == ACTIVE_MAP
            and "objects" in values
            and "objectives" in values
        ):
            samples.append((int(values["elapsed"]), values, objects(values["objects"])))
    if not samples:
        raise SystemExit("sortie trace has no active Meteor samples")

    entry = next(
        (
            sample
            for sample in samples
            if any(
                record[1] == BODY_SHAPE and int(record[15]) == MAXIMUM_DURABILITY
                for record in sample[2]
            )
        ),
        None,
    )
    if entry is None:
        raise SystemExit("sortie trace has no full-durability Queen Dragoon body")
    entry_elapsed, entry_values, entry_objects = entry
    entry_shapes = Counter(record[1] for record in entry_objects)
    if entry_shapes[BODY_SHAPE] != 1:
        raise SystemExit("Queen Dragoon needs exactly one retail body")
    for shape in LINKED_COMPONENT_SHAPES:
        if entry_shapes[shape] != 2:
            raise SystemExit(f"Queen Dragoon needs two linked {shape} components")

    defeat = next(
        (
            sample
            for sample in samples
            if sample[0] >= entry_elapsed
            and any(
                record[1] == BODY_SHAPE and int(record[15]) == 0 for record in sample[2]
            )
        ),
        None,
    )
    if defeat is None:
        raise SystemExit("sortie trace has no retail Queen Dragoon defeat sample")

    explosion = next(
        (
            sample
            for sample in samples
            if sample[0] >= defeat[0]
            and Counter(record[1] for record in sample[2])[BODY_EXPLOSION_SHAPE] == 1
            and Counter(record[1] for record in sample[2])[COMPONENT_BURST_SHAPE]
            == EXPECTED_LINKED_COMPONENTS
            and Counter(record[1] for record in sample[2])[COMPONENT_DEBRIS_SHAPE]
            == EXPECTED_LINKED_COMPONENTS
        ),
        None,
    )
    if explosion is None:
        raise SystemExit("sortie trace has no complete Queen Dragoon explosion sample")

    actor_lines = actor_logic_trace.read_text(encoding="utf-8").splitlines()
    move_records = [fields(line) for line in actor_lines if "event=move" in line]
    traced_objects = {record["object"] for record in move_records}
    if len(traced_objects) != EXPECTED_LOGIC_OBJECTS:
        raise SystemExit("actor trace must cover the body and four linked components")
    move_events_by_object = Counter(record["object"] for record in move_records)
    if set(move_events_by_object.values()) != {EXPECTED_MOVE_EVENTS_PER_OBJECT}:
        raise SystemExit("actor trace must cover four operations for every component")
    if Counter(record["shape"] for record in move_records).keys() != {
        BODY_SHAPE,
        *LINKED_COMPONENT_SHAPES,
    }:
        raise SystemExit("actor trace contains an unexpected Queen Dragoon component")

    return Evidence(
        sortie_sha256=digest(sortie_trace),
        actor_logic_sha256=digest(actor_logic_trace),
        forced_return_sha256=digest(forced_return_trace),
        objectives_before=objective_count(entry_values),
        objectives_after=objective_count(explosion[1]),
        defeat_retail_frame=defeat[0] - entry_elapsed,
        explosion_retail_frame=explosion[0] - entry_elapsed,
        forced_return_delay_retail_frames=extract_forced_return(forced_return_trace),
        move_events=len(move_records),
    )


def render(evidence: Evidence) -> str:
    return "\n".join(
        [
            "# Compact Mesen oracle evidence for Meteor's Queen Dragoon encounter.",
            f"# Raw sortie SHA-256: {evidence.sortie_sha256}",
            f"# Raw actor-logic SHA-256: {evidence.actor_logic_sha256}",
            (f"# Raw forced-objective return SHA-256: {evidence.forced_return_sha256}"),
            (
                "# This trace deliberately injects zero remaining objectives; "
                "it proves only the resulting return presentation, not natural "
                "Meteor completion."
            ),
            (
                f"mission name={MISSION_NAME} mission_selection={MISSION_SELECTION} "
                f"active_map={ACTIVE_MAP.split(':')[1]} "
                f"objectives_before={evidence.objectives_before} "
                f"objectives_after={evidence.objectives_after}"
            ),
            (
                f"boss name={BOSS_NAME} maximum_durability={MAXIMUM_DURABILITY} "
                f"defeat_retail_frame={evidence.defeat_retail_frame} "
                f"explosion_retail_frame={evidence.explosion_retail_frame}"
            ),
            f"component role=body source_shape={BODY_SHAPE} count=1",
            f"component role=linked_pair_a source_shape={LINKED_COMPONENT_SHAPES[0]} count=2",
            f"component role=linked_pair_b source_shape={LINKED_COMPONENT_SHAPES[1]} count=2",
            f"explosion role=body source_shape={BODY_EXPLOSION_SHAPE} count=1",
            (
                f"explosion role=component_burst source_shape={COMPONENT_BURST_SHAPE} "
                f"count={EXPECTED_LINKED_COMPONENTS}"
            ),
            (
                f"explosion role=component_debris source_shape={COMPONENT_DEBRIS_SHAPE} "
                f"count={EXPECTED_LINKED_COMPONENTS}"
            ),
            (
                f"logic traced_objects={EXPECTED_LOGIC_OBJECTS} "
                f"move_events={evidence.move_events}"
            ),
            (
                "forced_return injected_remaining_objectives=0 "
                "return_input=forward "
                "natural_completion_proven=false "
                f"observed_return_retail_frames={evidence.forced_return_delay_retail_frames} "
                "active_map_unchanged=true"
            ),
            "",
        ]
    )


def validate_compact(path: Path) -> None:
    content = path.read_text(encoding="utf-8")
    lines = [line for line in content.splitlines() if line and not line.startswith("#")]
    if len(lines) != 10:
        raise SystemExit("Queen Dragoon fixture has an unexpected record count")
    mission, boss, *components, logic, forced_return = [fields(line) for line in lines]
    if mission != {
        "name": MISSION_NAME,
        "mission_selection": str(MISSION_SELECTION),
        "active_map": ACTIVE_MAP.split(":")[1],
        "objectives_before": "2",
        "objectives_after": "2",
    }:
        raise SystemExit("Queen Dragoon mission fixture is inconsistent")
    if boss != {
        "name": BOSS_NAME,
        "maximum_durability": str(MAXIMUM_DURABILITY),
        "defeat_retail_frame": "23",
        "explosion_retail_frame": "43",
    }:
        raise SystemExit("Queen Dragoon boss fixture is inconsistent")
    expected_components = {
        ("body", BODY_SHAPE, "1"),
        ("linked_pair_a", LINKED_COMPONENT_SHAPES[0], "2"),
        ("linked_pair_b", LINKED_COMPONENT_SHAPES[1], "2"),
        ("body", BODY_EXPLOSION_SHAPE, "1"),
        ("component_burst", COMPONENT_BURST_SHAPE, str(EXPECTED_LINKED_COMPONENTS)),
        ("component_debris", COMPONENT_DEBRIS_SHAPE, str(EXPECTED_LINKED_COMPONENTS)),
    }
    actual_components = {
        (record["role"], record["source_shape"], record["count"])
        for record in components
    }
    if actual_components != expected_components:
        raise SystemExit("Queen Dragoon component fixture is inconsistent")
    if logic != {
        "traced_objects": str(EXPECTED_LOGIC_OBJECTS),
        "move_events": str(EXPECTED_MOVE_EVENTS),
    }:
        raise SystemExit("Queen Dragoon actor-logic fixture is inconsistent")
    if forced_return != {
        "injected_remaining_objectives": "0",
        "return_input": "forward",
        "natural_completion_proven": "false",
        "observed_return_retail_frames": "833",
        "active_map_unchanged": "true",
    }:
        raise SystemExit("Queen Dragoon forced-return fixture is inconsistent")
    hashes = [
        line.rsplit(" ", 1)[-1]
        for line in content.splitlines()
        if line.startswith("# Raw ")
    ]
    if len(hashes) != 3:
        raise SystemExit("Queen Dragoon fixture must bind exactly three raw traces")
    if any(
        len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
        for value in hashes
    ):
        raise SystemExit("Queen Dragoon fixture has an invalid source digest")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--actor-source", type=Path)
    parser.add_argument(
        "--forced-return-source",
        "--completion-source",
        dest="forced_return_source",
        type=Path,
        help=(
            "oracle trace with an explicit objective-count injection; "
            "--completion-source remains as a compatibility alias"
        ),
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.source:
        if not args.actor_source:
            raise SystemExit("--actor-source is required with --source")
        if not args.forced_return_source:
            raise SystemExit("--forced-return-source is required with --source")
        generated = render(
            extract(args.source, args.actor_source, args.forced_return_source)
        )
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
        f"{action} {args.output}: retail Queen Dragoon body plus four linked components"
    )


if __name__ == "__main__":
    main()
