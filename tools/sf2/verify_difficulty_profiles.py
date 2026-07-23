#!/usr/bin/env python3
"""Verify semantic SF2 difficulty profiles directly from the retail ROM.

This is oracle tooling: source addresses and encodings stay here while the
shipping Rust game receives only the typed concepts printed by this verifier.
"""

from __future__ import annotations

from dataclasses import dataclass

from rom import load_rom


DIFFICULTY_NAMES = ("normal", "hard", "expert")
PROFILE_TABLES = {
    "planetary_defense_units": 0x04E079,
    "opening_attackers": 0x04E07C,
    "total_opening_threat_units": 0x04E07F,
    "battle_carriers": 0x04E052,
    "occupied_planets": 0x04CE8A,
    "strategic_pressure": 0x04EE29,
}
EVENT_START_TABLE = 0x04E04C
EVENT_TABLE = 0x04EF74
OPENING_WAVE_TABLE = 0x04E6F2
WORLD_ASSIGNMENT_TABLE = 0x04EE52

# Retail labels captured by selecting each of the six ordinary planet objects
# with ``SF2_ORACLE_FORCE_OCCUPIED_SELECTION``.  These identities are oracle
# semantics; source selection ordinals never cross into the shipping port.
WORLD_NAMES_BY_SELECTION = (
    "venom",
    "titania",
    "macbeth",
    "eladard",
    "meteor",
    "fortuna",
)


@dataclass(frozen=True)
class DifficultyProfile:
    occupied_planets: int
    planetary_defense_units: int
    opening_attackers: int
    total_opening_threat_units: int
    battle_carriers: int
    opening_wave_records: tuple[int, ...]
    strategic_pressure: int


EXPECTED_PROFILES = {
    "normal": DifficultyProfile(2, 2, 2, 4, 1, (22,), 0),
    "hard": DifficultyProfile(3, 3, 4, 7, 1, (29, 36), 4),
    "expert": DifficultyProfile(3, 6, 4, 10, 2, (43, 50), 12),
}

EXPECTED_EVENTS = {
    "normal": (
        (0, 0xC29C),
        (20, 0xC1CE),
        (25, 0xC22B),
        (35, 0xC1CE),
        (40, 0xC204),
        (65_534, 0xC276),
    ),
    "hard": (
        (0, 0xC29C),
        (38, 0xC1B3),
        (42, 0xC1BC),
        (45, 0xC1DA),
        (50, 0xC1CE),
        (60, 0xC204),
        (85, 0xC244),
        (95, 0xC1CE),
        (110, 0xC22B),
        (120, 0xC204),
        (130, 0xC1CE),
        (190, 0xC1DA),
        (65_534, 0xC276),
    ),
    "expert": (
        (0, 0xC29C),
        (35, 0xC1BC),
        (49, 0xC204),
        (52, 0xC1B3),
        (60, 0xC25D),
        (90, 0xC244),
        (95, 0xC1CE),
        (110, 0xC22B),
        (115, 0xC1CE),
        (120, 0xC204),
        (145, 0xC1CE),
        (165, 0xC1DA),
        (235, 0xC1DA),
        (285, 0xC1DA),
        (65_534, 0xC276),
    ),
}

EXPECTED_ASSIGNMENT_ROWS = {
    "normal": (
        (1, 0),
        (3, 0),
        (4, 0),
        (3, 1),
        (4, 1),
        (4, 3),
    ),
    "hard": (
        (0, 1, 2),
        (0, 1, 3),
        (0, 1, 4),
        (5, 1, 0),
        (0, 2, 3),
        (0, 2, 4),
        (0, 2, 5),
        (0, 3, 4),
        (5, 3, 0),
        (0, 4, 5),
        (1, 2, 3),
        (4, 2, 1),
        (1, 2, 5),
        (1, 3, 4),
        (5, 3, 1),
        (1, 4, 5),
        (4, 3, 2),
        (5, 3, 2),
        (2, 4, 5),
        (3, 4, 5),
    ),
    "expert": (
        (0, 1, 2, 3, 4, 5),
        (0, 1, 3, 5, 4, 2),
        (0, 1, 4, 5, 2, 3),
        (5, 1, 0, 2, 3, 4),
        (0, 2, 3, 4, 5, 1),
        (0, 2, 4, 1, 3, 5),
        (0, 2, 5, 3, 1, 4),
        (0, 3, 4, 2, 5, 1),
        (5, 3, 0, 4, 1, 2),
        (0, 4, 5, 1, 2, 3),
        (1, 2, 3, 4, 5, 0),
        (4, 2, 1, 5, 3, 0),
        (1, 2, 5, 3, 0, 4),
        (1, 3, 4, 2, 0, 5),
        (5, 3, 1, 0, 2, 4),
        (1, 4, 5, 0, 3, 2),
        (4, 3, 2, 1, 5, 0),
        (5, 3, 2, 1, 4, 0),
        (2, 4, 5, 0, 1, 3),
        (3, 4, 5, 2, 0, 1),
    ),
}


def file_offset(source_address: int) -> int:
    bank = source_address >> 16
    address = source_address & 0xFFFF
    if bank > 0x3F or address < 0x8000:
        raise ValueError(f"unsupported source address {source_address:06X}")
    return bank * 0x8000 + (address & 0x7FFF)


def byte(rom: bytes, source_address: int) -> int:
    return rom[file_offset(source_address)]


def word_at_offset(rom: bytes, offset: int) -> int:
    return int.from_bytes(rom[offset : offset + 2], "little")


def word(rom: bytes, source_address: int) -> int:
    return word_at_offset(rom, file_offset(source_address))


def opening_wave_records(rom: bytes, difficulty_index: int) -> tuple[int, ...]:
    table = file_offset(OPENING_WAVE_TABLE)
    list_offset = word_at_offset(rom, table + difficulty_index * 2)
    records = []
    cursor = table + list_offset
    while record_offset := word_at_offset(rom, cursor):
        records.append(record_offset)
        cursor += 2
    for record_offset in records:
        attacker_count = rom[table + record_offset]
        if attacker_count != 2:
            raise AssertionError(
                f"opening wave at offset {record_offset} has {attacker_count} attackers"
            )
    return tuple(records)


def profile(rom: bytes, difficulty_index: int) -> DifficultyProfile:
    values = {
        name: byte(rom, address + difficulty_index)
        for name, address in PROFILE_TABLES.items()
    }
    return DifficultyProfile(
        occupied_planets=values["occupied_planets"],
        planetary_defense_units=values["planetary_defense_units"],
        opening_attackers=values["opening_attackers"],
        total_opening_threat_units=values["total_opening_threat_units"],
        battle_carriers=values["battle_carriers"],
        opening_wave_records=opening_wave_records(rom, difficulty_index),
        strategic_pressure=values["strategic_pressure"],
    )


def strategic_events(rom: bytes, difficulty_index: int) -> tuple[tuple[int, int], ...]:
    start = word(rom, EVENT_START_TABLE + difficulty_index * 2)
    cursor = file_offset(EVENT_TABLE) + start
    events = []
    while True:
        event_time = word_at_offset(rom, cursor)
        handler = word_at_offset(rom, cursor + 2)
        cursor += 4
        if event_time == 65_535:
            break
        events.append((event_time, handler))
    return tuple(events)


def assignment_rows(
    rom: bytes, difficulty_index: int, row_width: int
) -> tuple[tuple[int, ...], ...]:
    table = file_offset(WORLD_ASSIGNMENT_TABLE)
    row_count = word_at_offset(rom, table + difficulty_index * 4)
    row_offset = word_at_offset(rom, table + difficulty_index * 4 + 2)
    cursor = table + row_offset
    return tuple(
        tuple(rom[cursor + row * row_width : cursor + (row + 1) * row_width])
        for row in range(row_count)
    )


def verify_assignment_structure(
    name: str, actual_profile: DifficultyProfile, rows: tuple[tuple[int, ...], ...]
) -> None:
    occupied_count = actual_profile.occupied_planets
    occupied = tuple(row[:occupied_count] for row in rows)
    if any(set(row) - set(range(len(WORLD_NAMES_BY_SELECTION))) for row in rows):
        raise SystemExit(f"{name} world assignment contains an unknown selection")
    if len(set(occupied)) != len(occupied):
        raise SystemExit(f"{name} world assignment repeats an occupied row")

    combinations = {tuple(sorted(row)) for row in occupied}
    if name == "normal":
        allowed = {0, 1, 3, 4}
        expected_combinations = {
            (left, right)
            for left in allowed
            for right in allowed
            if left < right
        }
        if combinations != expected_combinations:
            raise SystemExit("normal assignments do not cover the retail four-world pool")
    else:
        expected_combinations = {
            (first, second, third)
            for first in range(len(WORLD_NAMES_BY_SELECTION))
            for second in range(first + 1, len(WORLD_NAMES_BY_SELECTION))
            for third in range(second + 1, len(WORLD_NAMES_BY_SELECTION))
        }
        if combinations != expected_combinations:
            raise SystemExit(f"{name} assignments do not cover all three-world choices")

    if name == "expert" and any(
        tuple(sorted(row)) != tuple(range(len(WORLD_NAMES_BY_SELECTION))) for row in rows
    ):
        raise SystemExit("each expert assignment row must remain a six-world permutation")


def main() -> None:
    rom = load_rom()
    for difficulty_index, name in enumerate(DIFFICULTY_NAMES):
        actual_profile = profile(rom, difficulty_index)
        if actual_profile != EXPECTED_PROFILES[name]:
            raise SystemExit(
                f"{name} difficulty profile changed: "
                f"expected {EXPECTED_PROFILES[name]}, got {actual_profile}"
            )
        actual_events = strategic_events(rom, difficulty_index)
        if actual_events != EXPECTED_EVENTS[name]:
            raise SystemExit(
                f"{name} strategic events changed: "
                f"expected {EXPECTED_EVENTS[name]}, got {actual_events}"
            )
        actual_assignments = assignment_rows(
            rom,
            difficulty_index,
            actual_profile.planetary_defense_units,
        )
        if actual_assignments != EXPECTED_ASSIGNMENT_ROWS[name]:
            raise SystemExit(
                f"{name} world assignment table changed: "
                f"expected {EXPECTED_ASSIGNMENT_ROWS[name]}, got {actual_assignments}"
            )
        verify_assignment_structure(name, actual_profile, actual_assignments)
        occupied_names = {
            tuple(WORLD_NAMES_BY_SELECTION[selection] for selection in row[: actual_profile.occupied_planets])
            for row in actual_assignments
        }
        print(
            f"verified {name}: occupied_planets={actual_profile.occupied_planets} "
            f"planetary_defense_units={actual_profile.planetary_defense_units} "
            f"opening_attackers={actual_profile.opening_attackers} "
            f"battle_carriers={actual_profile.battle_carriers} "
            f"opening_waves={len(actual_profile.opening_wave_records)} "
            f"strategic_events={len(actual_events)} "
            f"world_assignments={len(occupied_names)}"
        )

    hard_occupied = tuple(row[:3] for row in EXPECTED_ASSIGNMENT_ROWS["hard"])
    expert_occupied = tuple(row[:3] for row in EXPECTED_ASSIGNMENT_ROWS["expert"])
    if expert_occupied != hard_occupied:
        raise SystemExit("Hard and Expert occupied-world choices no longer align")
    print(
        "verified retail world identities: "
        + ", ".join(
            f"{selection}={name}"
            for selection, name in enumerate(WORLD_NAMES_BY_SELECTION)
        )
    )


if __name__ == "__main__":
    main()
