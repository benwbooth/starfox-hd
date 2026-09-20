#!/usr/bin/env python3
"""Lower reviewed complete source path graphs into typed native catalogs.

No encoded operands or source-address lookup enter gameplay. The allow-list
deliberately contains complete graphs only; unsupported statements abort the
entire generation, rather than inserting placeholders or truncating a graph.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent / "disasm"))
from extract_path import DEFAULT_ROM, PathAddress, PathCommand, PathExtractor
from path_semantics import PATH_SEMANTICS
from extract_shapes import SHAPE_HEADER_START, SHAPE_HEADER_SIZE, SHAPE_HEADER_COUNT
from dump_runtime_routine import source_offset

REPO = Path(__file__).resolve().parents[2]
OUTPUT = REPO / "rust/sf2-game/src/native/authored_paths.rs"
# Independently installed by source actor strategies, not a scanned candidate.
ROOTS = (
    ("ALTERNATE_EXHAUST", PathAddress(0xF536)),
    ("COLOR_CYCLE_SPRITE", PathAddress(0xF593)),
    ("RANDOMIZED_COLOR_PARTICLE", PathAddress(0xF294)),
    ("LOCAL_JITTER_SPRITE", PathAddress(0xF521)),
    ("AUXILIARY_GATED_SPRITE", PathAddress(0xF36F)),
    ("CHILD_DETACHING_SPRITE", PathAddress(0xF540)),
    ("REPEATED_CHILD_SPRITE", PathAddress(0xF561)),
    ("SOUND_COLOR_SPRITE", PathAddress(0xF582)),
    ("CALLBACK_GATED_SPRITE", PathAddress(0xF32C)),
    ("PRIMARY_MOTION_GROUND_LIMITED", PathAddress(0xF029)),
    ("PRIMARY_TARGET_FOLLOWER", PathAddress(0xF38A)),
    ("SHARED_COUNTDOWN_SERVICE", PathAddress(0x04FF)),
    ("COUNTER_MOTION_EFFECT", PathAddress(0xBE65)),
    ("PLAYER_CHARGE_ORB", PathAddress(0xF04F)),
    ("NODE_GATED_TARGET_SERVICE", PathAddress(0x545F)),
    ("ENCOUNTER_RADIO_SERVICE", PathAddress(0x7BC5)),
    ("FIRST_CONTROL_GUIDANCE", PathAddress(0x04B5)),
    ("SURFACE_OR_GROUND_LIMITED", PathAddress(0xEEED)),
    ("PRIMARY_MOTION_SURFACE_LIMITED", PathAddress(0xEE10)),
    ("OCCUPANCY_SURFACE_LIMITED", PathAddress(0xEC98)),
    ("DOUBLED_MOTION_HOMING_PROJECTILE", PathAddress(0xEE2D)),
    ("PRIMARY_MOTION_HOMING_PROJECTILE", PathAddress(0xEE3B)),
    ("DIFFICULTY_HOMING_PROJECTILE", PathAddress(0xEE4C)),
    ("VARIANT_GUIDED_PROJECTILE", PathAddress(0xEF2D)),
    ("OFFSET_GUIDED_PROJECTILE", PathAddress(0xECF7)),
    ("LINKED_PROTECTION_EFFECT", PathAddress(0xF2B9)),
    ("TRIGGERED_LINKED_PROJECTILE", PathAddress(0xF48B)),
    ("ATTACHED_RECOVERY_EFFECT", PathAddress(0xF3AD)),
    ("SCENE_MATERIAL_SCENERY", PathAddress(0x7FAA)),
    ("DISTANCE_GATED_SCENERY", PathAddress(0x7F27)),
    ("HEALTH_ROTATED_DISTANCE_SCENERY", PathAddress(0x7F24)),
    ("TARGETING_UPGRADE_GLOW", PathAddress(0x81E1)),
    ("HIT_TOGGLE_SPRITE", PathAddress(0x8488)),
    ("COUNTED_HIT_TOGGLE_SPRITE", PathAddress(0x8486)),
    ("GROWING_SPRITE_HOLD", PathAddress(0x0059)),
    ("SHAPE_FILTERED_SCENERY", PathAddress(0x7F78)),
    ("DRIFTING_PULSE_SPRITE", PathAddress(0x82E3)),
    ("PART_JITTER_FADE_SPRITE", PathAddress(0x8285)),
    ("FADE_SPRITE", PathAddress(0x8458)),
    ("HEALTH_FADE_SOUND_SPRITE", PathAddress(0x8394)),
    ("ALTERNATE_HEALTH_FADE_SOUND_SPRITE", PathAddress(0x838F)),
    ("SHRINKING_RISE_SPRITE", PathAddress(0x83C2)),
    ("PART_SOUND_BLINK_SPRITE", PathAddress(0x832E)),
    ("OFFSET_SHRINK_SPRITE", PathAddress(0x90FD)),
    ("FIXED_SIZE_FADE_SPRITE", PathAddress(0x9277)),
    ("PHASE_GROWTH_FADE_SPRITE", PathAddress(0xC520)),
    ("SOLID_SPRITE_HOLD", PathAddress(0x4DF1)),
    ("BIASED_DEPTH_HOLD", PathAddress(0xCFD4)),
    ("TABLE_COLOR_REVEAL", PathAddress(0x5667)),
    ("RELATIVE_YAW_EFFECT", PathAddress(0x9904)),
    ("RELATIVE_DRIFT_ROLL_EFFECT", PathAddress(0x9A3B)),
    ("FOOTPRINT_YAW_EFFECT", PathAddress(0xF1BD)),
    ("TIMED_FALLING_YAW_EFFECT", PathAddress(0xF1AE)),
    ("RESET_ANIMATION_YAW_EFFECT", PathAddress(0xF1C9)),
    ("SIX_STEP_SHAPE_EFFECT", PathAddress(0xC1FF)),
    ("DEPTH_BIASED_WAIT_EFFECT", PathAddress(0xFA07)),
    ("HIT_CYCLED_SHAPE", PathAddress(0x20CD)),
    ("PHASE_INCREMENTED_MOTION_FADE_SPRITE", PathAddress(0x83F4)),
    ("RANDOM_SIZE_MOTION_FADE_SPRITE", PathAddress(0x83F9)),
    ("SMALL_RANDOM_MOTION_FADE_SPRITE", PathAddress(0x8402)),
    ("RANDOM_TUMBLING_MESH_EFFECT", PathAddress(0x98E5)),
    ("ROLLING_CONTACT_SHAPE", PathAddress(0x9A04)),
    ("RESET_SHAPE_HOLD", PathAddress(0x77C0)),
    ("DISTANT_SHAPE_HOLD", PathAddress(0x7FA1)),
    ("SHIELD_RECOVERY_PICKUP", PathAddress(0x44B2)),
    ("WEAPON_UPGRADE_PICKUP", PathAddress(0x44B4)),
    ("CONSUMABLE_PICKUP_TYPE_ZERO", PathAddress(0x44B6)),
    ("CONSUMABLE_PICKUP_TYPE_THREE", PathAddress(0x44BA)),
    ("CONSUMABLE_PICKUP_TYPE_ONE", PathAddress(0x44BC)),
    ("RANDOM_TEXTURE_CONTACT_SPRITE", PathAddress(0xF9DD)),
    ("DISTANCE_AIMED_PROJECTILE", PathAddress(0x4DF9)),
    ("RANDOMIZED_YAW_GUIDED_PROJECTILE", PathAddress(0x6BCD)),
    ("DELAYED_CONTACT_PROJECTILE", PathAddress(0x9E9A)),
    ("INVISIBLE_CHILD_RETIREMENT", PathAddress(0x0A0D)),
    ("ALTERNATE_INVISIBLE_CHILD_RETIREMENT", PathAddress(0x432A)),
    ("NONCOLLIDING_PROJECTILE_ATTACHMENT", PathAddress(0x6BCB)),
    ("TEN_TICK_EFFECT", PathAddress(0x7B8F)),
    ("NONCOLLIDING_ROLL_ATTACHMENT", PathAddress(0x99E6)),
    ("NONCOLLIDING_MESH_ATTACHMENT", PathAddress(0x9CDA)),
    ("CONTACT_SUPPRESSED_ATTACHMENT", PathAddress(0xACFE)),
    ("CLIPPED_SHADOWLESS_ATTACHMENT", PathAddress(0x8BF7)),
    ("DEFERRED_FAST_CONTACT_MESH", PathAddress(0xF9F2)),
    ("HIT_RELEASED_RELATIVE_RISE", PathAddress(0x5A8D)),
    ("ALTERNATING_DRIFT_SPRITE", PathAddress(0x830D)),
    ("HIT_RELEASED_ROTATING_ATTACHMENT", PathAddress(0x5EC4)),
    ("HEIGHT_SELECTED_ARC_EFFECT", PathAddress(0xB07C)),
    ("PERIODIC_PART_MOTION_EMITTER", PathAddress(0x2271)),
    ("TIMED_GROUND_JITTER_EMITTER", PathAddress(0x55D1)),
    ("REPEATING_PULSE_EMITTER", PathAddress(0x8294)),
    ("LOW_HEALTH_PULSE_PAIR", PathAddress(0x82B6)),
    ("PLAYER_POSITION_PULSE_PAIR", PathAddress(0x82C3)),
    ("PULSE_PAIR", PathAddress(0x82D9)),
    ("LINKED_SHAPE_REVEAL_ATTACHMENT", PathAddress(0x9A4B)),
    ("SIGNAL_GATED_RELATIVE_LIFT", PathAddress(0x9DA1)),
    ("SIGNAL_GATED_LOOPING_MESH", PathAddress(0xAE1E)),
    ("ACTION_GATED_SOUND_HOLD", PathAddress(0xCFC8)),
    ("ACTION_GATED_ANIMATED_RETIREMENT", PathAddress(0xD399)),
    ("TIMED_SPIN_RISE_EFFECT", PathAddress(0xD3B2)),
    ("HIT_DETACHED_BOUNCING_PART", PathAddress(0xA481)),
    ("HIT_DRIVEN_ROTATING_PART_CONTROLLER", PathAddress(0xA4ED)),
    ("TARGETING_UPGRADE_PICKUP", PathAddress(0x787D)),
)
SEMANTICS = {entry.opcode: entry for entry in PATH_SEMANTICS}
# Independently scheduled child roots with a reviewed, reachable parent spawn.
# This proves installation only; it does not claim the parent graph is lowered.
CHILD_INSTALLERS = {
    PathAddress(0x7FAA): (PathAddress(0x787D), PathAddress(0x7887)),
    PathAddress(0x81E1): (PathAddress(0x787D), PathAddress(0x7895)),
    PathAddress(0x8488): (PathAddress(0x00BC), PathAddress(0x01CA)),
    PathAddress(0x8486): (PathAddress(0x0691), PathAddress(0x097D)),
    PathAddress(0x0059): (PathAddress(0x00BC), PathAddress(0x0210)),
    PathAddress(0x7F78): (PathAddress(0x58B9), PathAddress(0x5916)),
    PathAddress(0x82E3): (PathAddress(0x2651), PathAddress(0x8721)),
    PathAddress(0x8285): (PathAddress(0x4839), PathAddress(0x8714)),
    PathAddress(0x8458): (PathAddress(0x0691), PathAddress(0x08B3)),
    PathAddress(0x8394): (PathAddress(0x32EF), PathAddress(0x334E)),
    PathAddress(0x838F): (PathAddress(0x32EF), PathAddress(0x33EC)),
    PathAddress(0x83C2): (PathAddress(0x1369), PathAddress(0x73F3)),
    PathAddress(0x832E): (PathAddress(0x5B96), PathAddress(0x861D)),
    PathAddress(0x90FD): (PathAddress(0x8D82), PathAddress(0x8F63)),
    PathAddress(0x9277): (PathAddress(0x8D82), PathAddress(0x926F)),
    PathAddress(0xC520): (PathAddress(0xD27B), PathAddress(0xC344)),
    PathAddress(0x4DF1): (PathAddress(0x4D7E), PathAddress(0x4DA7)),
    PathAddress(0xCFD4): (PathAddress(0x4D7E), PathAddress(0x8121)),
    PathAddress(0x5667): (PathAddress(0x546C), PathAddress(0x562E)),
    PathAddress(0x9904): (PathAddress(0x9492), PathAddress(0x94D4)),
    PathAddress(0x9A3B): (PathAddress(0x9492), PathAddress(0x955D)),
    PathAddress(0xF1BD): (PathAddress(0xF136), PathAddress(0xF161)),
    PathAddress(0xF1AE): (PathAddress(0xF136), PathAddress(0xF16F)),
    PathAddress(0xF1C9): (PathAddress(0xF136), PathAddress(0xF176)),
    PathAddress(0xC1FF): (PathAddress(0xF5B4), PathAddress(0xC1D3)),
    PathAddress(0xFA07): (PathAddress(0xF5B4), PathAddress(0xF76C)),
    PathAddress(0x20CD): (PathAddress(0x1AFF), PathAddress(0x1B34)),
    PathAddress(0x83F4): (PathAddress(0x2651), PathAddress(0x25FD)),
    PathAddress(0x83F9): (PathAddress(0x00BC), PathAddress(0x0443)),
    PathAddress(0x8402): (PathAddress(0x2F11), PathAddress(0x3025)),
    PathAddress(0x98E5): (PathAddress(0x9492), PathAddress(0x9880)),
    PathAddress(0x9A04): (PathAddress(0x9492), PathAddress(0x99FC)),
    PathAddress(0x77C0): (PathAddress(0x7442), PathAddress(0x76AD)),
    PathAddress(0x7FA1): (PathAddress(0x7442), PathAddress(0x7791)),
    PathAddress(0x44B2): (PathAddress(0x0F7E), PathAddress(0x8CD3)),
    PathAddress(0x44B4): (PathAddress(0x0F7E), PathAddress(0x8CFB)),
    PathAddress(0x44B6): (PathAddress(0x0F7E), PathAddress(0x8CDD)),
    PathAddress(0x44BA): (PathAddress(0x0F7E), PathAddress(0x8CE7)),
    PathAddress(0x44BC): (PathAddress(0x0F7E), PathAddress(0x8CF1)),
    PathAddress(0xF9DD): (PathAddress(0xF5B4), PathAddress(0xF7A1)),
    PathAddress(0x4DF9): (PathAddress(0x4D7E), PathAddress(0x4DCF)),
    PathAddress(0x6BCD): (PathAddress(0x6A15), PathAddress(0x6BB9)),
    PathAddress(0x9E9A): (PathAddress(0x9492), PathAddress(0x9E46)),
    PathAddress(0x0A0D): (PathAddress(0x0691), PathAddress(0x0958)),
    PathAddress(0x432A): (PathAddress(0x419B), PathAddress(0x424C)),
    PathAddress(0x6BCB): (PathAddress(0x6A15), PathAddress(0x6B7D)),
    PathAddress(0x7B8F): (PathAddress(0x0AE7), PathAddress(0x8570)),
    PathAddress(0x99E6): (PathAddress(0x9492), PathAddress(0x990E)),
    PathAddress(0x9CDA): (PathAddress(0x9492), PathAddress(0x9B90)),
    PathAddress(0xACFE): (PathAddress(0xAA8A), PathAddress(0xAB0B)),
    PathAddress(0x8BF7): (PathAddress(0x7442), PathAddress(0x77FA)),
    PathAddress(0xF9F2): (PathAddress(0xF5B4), PathAddress(0xF765)),
    PathAddress(0x5A8D): (PathAddress(0x58B9), PathAddress(0x592E)),
    PathAddress(0x830D): (PathAddress(0x546C), PathAddress(0x5604)),
    PathAddress(0x5EC4): (PathAddress(0x5E1D), PathAddress(0x5E3C)),
    PathAddress(0xB07C): (PathAddress(0xB05E), PathAddress(0xB066)),
    PathAddress(0x2271): (PathAddress(0x2102), PathAddress(0x21DA)),
    PathAddress(0x55D1): (PathAddress(0x546C), PathAddress(0x556D)),
    PathAddress(0x8294): (PathAddress(0x66EA), PathAddress(0x6962)),
    PathAddress(0x82B6): (PathAddress(0x00BC), PathAddress(0x03F3)),
    PathAddress(0x82C3): (PathAddress(0x2BE9), PathAddress(0x2CCF)),
    PathAddress(0x82D9): (PathAddress(0x2651), PathAddress(0x82A7)),
    PathAddress(0x9A4B): (PathAddress(0x9492), PathAddress(0x96DB)),
    PathAddress(0x9DA1): (PathAddress(0x9492), PathAddress(0x9CF1)),
    PathAddress(0xAE1E): (PathAddress(0xAA8A), PathAddress(0xAAD8)),
    PathAddress(0xCFC8): (PathAddress(0x4D7E), PathAddress(0x4D99)),
    PathAddress(0xD399): (PathAddress(0xD27B), PathAddress(0xD374)),
    PathAddress(0xD3B2): (PathAddress(0xD27B), PathAddress(0xD2A4)),
    PathAddress(0xA481): (PathAddress(0xA2E6), PathAddress(0xA4F6)),
    PathAddress(0xA4ED): (PathAddress(0xA2E6), PathAddress(0xA2FC)),
}


class UnsupportedPath(ValueError):
    pass


@dataclass(frozen=True)
class SelectedOffsetAim:
    """One reviewed argument-preparation sequence, not native scratch state."""

    commands: tuple[PathCommand, ...]
    offset: tuple[int, int, int]

    @property
    def address(self):
        return self.commands[0].address

    @property
    def next(self):
        return self.commands[-1].successors[0]


def variable_bit_masks(rom: bytes) -> tuple[int, ...]:
    # $7F:B5FB decrements and doubles an eight-bit selector, then reads a
    # word from $7F:B5CF. The first sixteen entries are powers of two;
    # the remaining entries overlap instructions but are read as DATA.
    # Decode all reachable words offline, without retaining code execution
    # or source-address lookup in gameplay.
    start = source_offset(0x7FB5CF)
    data = rom[start:start + 256]
    if len(data) != 256:
        raise UnsupportedPath("truncated variable-bit mask data")
    return tuple(int.from_bytes(data[index:index + 2], "little") for index in range(0, 256, 2))


def banked_byte_values(rom: bytes, address: int) -> tuple[int, ...]:
    """Decode every possible byte index, never freeze a mutable memory view.

    The helper adds a zero-extended byte to the low word only. Retaining a
    table that crosses out of the proven ROM window would require a live
    domain-state mapping, not a constant snapshot of those bytes.
    """
    bank, base = address >> 16, address & 0xFFFF
    if not 0 <= bank < 0x40 or base < 0x8000 or base + 255 > 0xFFFF:
        raise UnsupportedPath(f"unreviewed constant-byte lookup window {address:06X}")
    start = source_offset(address)
    data = rom[start:start + 256]
    if len(data) != 256:
        raise UnsupportedPath(f"truncated constant-byte lookup {address:06X}")
    return tuple(data)


def banked_word_values(rom: bytes, address: int) -> tuple[int, ...]:
    # A byte selector is widened BEFORE doubling, giving 256 complete
    # little-endian words rather than aliasing the upper 128 indices.
    if (address & 0xFFFF) + 511 > 0xFFFF:
        raise UnsupportedPath(f"unreviewed constant-word lookup window {address:06X}")
    data = bytes(banked_byte_values(rom, address) + banked_byte_values(rom, address + 256))
    return tuple(int.from_bytes(data[index:index + 2], "little") for index in range(0, 512, 2))


def trigger_kind(condition: int) -> str:
    # Source dispatch table $7F:9B0F. Decode once; no numeric condition
    # selector or handler lookup is retained in the native catalog.
    kinds = [
        "Always",
        *(f"Periodic(TriggerPeriod::{period})" for period in (
            "Two", "Four", "Eight", "Sixteen", "ThirtyTwo", "SixtyFour", "OneTwentyEight")),
        "NewContact", "PlayerContact", "ConsumeHitEvent", "Detached",
        "ZeroHealth", "PlayerCrossing", "PlayerPartTarget",
        "ControlledAuxFlagHigh", "ControlledAuxFlagLow", "TimerPenultimate",
    ]
    if not 0 <= condition < len(kinds):
        raise UnsupportedPath(f"unreviewed trigger condition {condition}")
    return f"TriggerKind::{kinds[condition]}"


@dataclass(frozen=True)
class ChildSpawnParameters:
    """Offline literal operands of the two child-attachment spawn forms.

    Source shape tokens and path addresses remain extraction-only. This
    record does not authorize publishing a native spawn statement until its
    allocation, initialization, and world-service contracts are implemented.
    """

    shape: int
    path: PathAddress
    rotation: tuple[int, int, int]
    hit_points: int
    attack_power: int
    position: tuple[int, int, int]
    number: int


def child_spawn_parameters(command: PathCommand) -> ChildSpawnParameters:
    spec = SEMANTICS.get(command.opcode)
    if (spec is None or spec.handler_address != command.handler_address
            or spec.rust_name not in ("SpawnChild", "SpawnChildAlias")):
        raise UnsupportedPath(f"not a reviewed child spawn at {command.address.label()}")
    raw = bytes.fromhex(command.raw_hex)
    extended = spec.rust_name == "SpawnChildAlias"
    expected = 17 if extended else 14
    if command.prefix_size or len(raw) != expected:
        raise UnsupportedPath(f"unexpected {spec.rust_name} record size at {command.address.label()}")
    rotation = tuple(raw[5:8]) if extended else (0, 0, 0)
    health_at = 8 if extended else 5
    position_at = health_at + 2
    return ChildSpawnParameters(
        shape=int.from_bytes(raw[1:3], "little"),
        path=PathAddress(int.from_bytes(raw[3:5], "little")),
        rotation=rotation,
        hit_points=raw[health_at],
        attack_power=raw[health_at + 1],
        position=tuple(int.from_bytes(raw[index:index + 2], "little", signed=True)
                       for index in range(position_at, position_at + 6, 2)),
        number=raw[-1],
    )


@dataclass(frozen=True)
class IndependentSpawnParameters:
    """Offline literals for the seven-byte, unattached actor spawn form."""

    shape: int
    path: PathAddress
    hit_points: int
    attack_power: int


def independent_spawn_parameters(command: PathCommand) -> IndependentSpawnParameters:
    spec = SEMANTICS.get(command.opcode)
    if (spec is None or spec.handler_address != command.handler_address
            or spec.rust_name != "QuickSpawn"):
        raise UnsupportedPath(f"not a reviewed independent spawn at {command.address.label()}")
    raw = bytes.fromhex(command.raw_hex)
    if command.prefix_size or len(raw) != 7 or raw[0] != command.opcode:
        raise UnsupportedPath(f"unexpected independent spawn record at {command.address.label()}")
    return IndependentSpawnParameters(
        shape=int.from_bytes(raw[1:3], "little"),
        path=PathAddress(int.from_bytes(raw[3:5], "little")),
        hit_points=raw[5],
        attack_power=raw[6],
    )


def shape_index(shape: int) -> int:
    delta = shape - SHAPE_HEADER_START
    if delta < 0 or delta % SHAPE_HEADER_SIZE or delta // SHAPE_HEADER_SIZE >= SHAPE_HEADER_COUNT:
        raise UnsupportedPath(f"shape is not a catalog header: {shape:04X}")
    return delta // SHAPE_HEADER_SIZE


def spawn_shape(shape: int, path: PathAddress | None = None) -> tuple[int, str]:
    index = shape_index(shape)
    # Reviewed transient sprite family, also named by the source pool-pressure
    # sweep. Other shapes need native metadata review; never guess enemy vs
    # scenery vs projectile from a numerically valid shape header alone.
    # Shape 19 also serves unrelated damaging/attached objects. Only this
    # complete, collision-disabled sprite paths are effects;
    # the mesh alone is insufficient evidence for other uses of that shape.
    if index == 19 and path in (PathAddress(0xF5A1), PathAddress(0xF306),
                               PathAddress(0x8486), PathAddress(0x8488)):
        return index, "ObjectKind::Effect"
    # Moving/contact-triggered child of the primary weapon service D11D.
    if index == 7 and path == PathAddress(0xF4CA):
        return index, "ObjectKind::Projectile"
    # Collision-disabled recovery effects: settle, center, then self-frame/tumble
    # before clearing health. Classification applies only to these paths.
    if (index, path) in ((112, PathAddress(0xF3D4)), (113, PathAddress(0xF3DE))):
        return index, "ObjectKind::Effect"
    # The upgrade pickup's collision-disabled presentation children. Restrict
    # classification to their reviewed paths, not all uses of either shape.
    if (index, path) == (305, PathAddress(0x7FAA)):
        return index, "ObjectKind::Scenery"
    if (index, path) == (516, PathAddress(0x81E1)):
        return index, "ObjectKind::Effect"
    if (index, path) == (16, PathAddress(0x0059)):
        return index, "ObjectKind::Effect"
    if (index, path) == (239, PathAddress(0x7F78)):
        return index, "ObjectKind::Scenery"
    if index == 8 and path in (PathAddress(0x82E3), PathAddress(0x8285), PathAddress(0x8458)):
        return index, "ObjectKind::Effect"
    if ((index == 22 and path in (PathAddress(0x8394), PathAddress(0x838F)))
            or (index, path) in ((40, PathAddress(0x83C2)), (36, PathAddress(0x832E)))):
        return index, "ObjectKind::Effect"
    if (index, path) in ((37, PathAddress(0x90FD)), (22, PathAddress(0x9277))):
        return index, "ObjectKind::Effect"
    if (index, path) in ((18, PathAddress(0x4DF1)), (0, PathAddress(0xCFD4)), (0, PathAddress(0x5667))):
        return index, "ObjectKind::Effect"
    if (index, path) == (14, PathAddress(0x83F9)):
        return index, "ObjectKind::Effect"
    # These complete child graphs disable collision before yielding. Do not
    # infer a kind for other uses of their mesh shapes or parent graphs.
    if (index, path) in ((313, PathAddress(0x9904)), (48, PathAddress(0x9A3B)),
                        (88, PathAddress(0xF1BD)), (315, PathAddress(0xF1AE)),
                        (124, PathAddress(0xF1C9)), (399, PathAddress(0xC1FF)),
                        (48, PathAddress(0xFA07))):
        return index, "ObjectKind::Effect"
    if (index, path) == (309, PathAddress(0x98E5)):
        return index, "ObjectKind::Effect"
    # Hittable damaging encounter part: remains attached until its hit event,
    # then detaches, bounces and requests death. This is not a visual-only
    # sprite; scope the classification to this shape AND complete path.
    if (index, path) == (323, PathAddress(0xA481)):
        return index, "ObjectKind::Enemy"
    if index not in (9, 10, 11, 12, 13):
        raise UnsupportedPath(f"unreviewed native spawn kind for shape {shape:04X}")
    return index, "ObjectKind::Effect"


def word_field(variable: int) -> str:
    # CB47 maps these authored operands to the existing actor words. Numeric
    # encodings stop here: generated gameplay statements name actual fields.
    fields = {
        0x0C: "WordField::Position(Axis::X)",
        0x0E: "WordField::Position(Axis::Y)",
        0x10: "WordField::Position(Axis::Z)",
        0x32: "WordField::Velocity(Axis::X)",
        0x34: "WordField::Velocity(Axis::Y)",
        0x36: "WordField::Velocity(Axis::Z)",
        0x39: "WordField::SavedPosition(Axis::X)",
        0x3B: "WordField::SavedPosition(Axis::Y)",
        0x3D: "WordField::SavedPosition(Axis::Z)",
        0x87: "WordField::DepthOffset",
        0x8E: "WordField::RelativePosition(Axis::X)",
        0x90: "WordField::RelativePosition(Axis::Y)",
        0x92: "WordField::RelativePosition(Axis::Z)",
        0xA1: "WordField::MotionPhase",
        0xA3: "WordField::ScriptValue",
    }
    if variable not in fields:
        raise UnsupportedPath(f"unported word operand {variable:02X}")
    return fields[variable]


def byte_field(variable: int) -> str:
    fields = {
        0x0A: "ByteField::TargetSpeed",
        0x0B: "ByteField::Acceleration",
        0x12: "ByteField::Rotation(Axis::X)",
        0x13: "ByteField::ChildNumber",
        0x14: "ByteField::Rotation(Axis::Y)",
        0x16: "ByteField::Rotation(Axis::Z)",
        0x17: "ByteField::WaitTimer",
        0x18: "ByteField::Speed",
        0x27: "ByteField::ScriptParameter",
        0x28: "ByteField::RepeatCounter",
        0x2D: "ByteField::Health",
        0x2E: "ByteField::AttackPower",
        0x2F: "ByteField::WeaponSelection",
        0x89: "ByteField::Animation(AnimationChannel::Color)",
        0x8A: "ByteField::Animation(AnimationChannel::Shape)",
        0x94: "ByteField::RelativeRotation(Axis::X)",
        0x95: "ByteField::RelativeRotation(Axis::Y)",
        0x96: "ByteField::RelativeRotation(Axis::Z)",
        0x99: "ByteField::TextureScrollX",
        0x9A: "ByteField::TextureScrollY",
        0xA9: "ByteField::Part",
        0xAE: "ByteField::ClippingPlane",
        0xAF: "ByteField::SpawnGroup",
    }
    if variable in fields:
        return fields[variable]
    # Each pair aliases one actual typed word; it must not create a separate
    # particle counter or independent byte shadow of the motion phase.
    for base in (0x0C, 0x0E, 0x10, 0x32, 0x34, 0x36, 0x39, 0x3B, 0x3D, 0x87, 0x8E, 0x90, 0x92, 0xA1, 0xA3):
        if variable in (base, base + 1):
            part = "Low" if variable == base else "High"
            return f"ByteField::WordPart {{ field: {word_field(base)}, part: BytePart::{part} }}"
    raise UnsupportedPath(f"unported byte operand {variable:02X}")


def graph(extractor: PathExtractor, root: PathAddress) -> list[PathCommand]:
    pending = [root]
    found = {}
    while pending:
        address = pending.pop()
        if address in found:
            continue
        command = extractor.decode_command(address)
        found[address] = command
        pending.extend(command.successors)
        # Spawned actors run independent paths: these are dependencies, not
        # control-flow successors of the parent. Follow them to closure before
        # lowering or assigning the catalog's globally shared cursor identity.
        if command.opcode in (0x033, 0x0F5):
            child_path = child_spawn_parameters(command).path
            if child_path.offset:
                pending.append(child_path)
        elif command.opcode == 0x05D:
            child_path = independent_spawn_parameters(command).path
            if child_path.offset:
                pending.append(child_path)
    return [found[address] for address in sorted(found)]


def lowering_units(extractor: PathExtractor, root: PathAddress):
    """Fold a closed immediate offset-preparation block into its consumer.

    Every original command remains in the source graph. Native cursors exist
    only at semantic boundaries: entry/callback/spawn edges into the middle
    of a folded sequence are rejected, never aliased to its beginning.
    """
    commands = graph(extractor, root)
    by_address = {command.address: command for command in commands}
    predecessors = {}
    entries = {root}
    for command in commands:
        for successor in command.successors:
            predecessors.setdefault(successor, set()).add(command.address)
        if command.opcode in (0x033, 0x0F5):
            entries.add(child_spawn_parameters(command).path)
        elif command.opcode == 0x05D:
            entries.add(independent_spawn_parameters(command).path)
    consumed = set()
    units = []
    for command in commands:
        if command.address in consumed:
            continue
        if not (command.opcode == 0x0FB and command.raw_hex.startswith("fbb116")):
            units.append(command)
            continue
        block = []
        values = []
        current = command
        for address in (0x16B1, 0x16B3, 0x16B5):
            raw = bytes.fromhex(current.raw_hex)
            if (current.opcode != 0x0FB or current.prefix_size != 0
                    or current.handler_address != SEMANTICS[0x0FB].handler_address
                    or len(raw) != 4 or raw[:3] != b"\xfb" + address.to_bytes(2, "little")
                    or current.successors != (PathAddress((current.address.offset + 4) & 0xFFFF),)):
                raise UnsupportedPath(f"incomplete selected-offset preparation at {command.address.label()}")
            block.append(current)
            values.append(int.from_bytes(raw[3:], "little", signed=True))
            next_address = current.successors[0]
            if next_address not in by_address:
                raise UnsupportedPath(f"missing selected-offset consumer at {command.address.label()}")
            current = by_address[next_address]
        if (current.opcode != 0x130 or current.prefix_size != 1 or current.raw_hex != "0030"
                or current.handler_address != SEMANTICS[0x130].handler_address
                or current.successors != (PathAddress((current.address.offset + 2) & 0xFFFF),)):
            raise UnsupportedPath(f"unexpected selected-offset consumer at {command.address.label()}")
        block.append(current)
        for previous, interior in zip(block, block[1:]):
            if interior.address in entries or predecessors.get(interior.address) != {previous.address}:
                raise UnsupportedPath(f"external entry into selected-offset preparation at {interior.address.label()}")
        consumed.update(part.address for part in block[1:])
        units.append(SelectedOffsetAim(tuple(block), tuple(values)))
    # A preparation can wrap the bank boundary, so an interior command can
    # sort before its leading store. Remove it only after all folds are known.
    return [unit for unit in units if unit.address not in consumed]


def lower_graph(extractor: PathExtractor, root: PathAddress, path_index: int, indices=None):
    commands = lowering_units(extractor, root)
    if indices is None:
        indices = {command.address: index for index, command in enumerate(commands)}

    def cursor(address):
        return f"cursor({path_index}, {indices[address]})"

    statements = []
    for command in commands:
        if isinstance(command, SelectedOffsetAim):
            x, y, z = command.offset
            statements.append(f"Statement::FaceSelectedOffset {{ offset: super::path_steering::AimOffset {{ x: {x}, y: {y}, z: {z} }}, next: {cursor(command.next)} }}")
            continue
        spec = SEMANTICS.get(command.opcode)
        if spec is None or spec.handler_address != command.handler_address:
            raise UnsupportedPath(f"unreviewed handler at {command.address.label()}")
        name = spec.rust_name
        raw = bytes.fromhex(command.raw_hex)
        # The extractor counts only an optional zero escape in prefix_size;
        # the handler's opcode byte is still present after that prefix.
        operand_start = command.prefix_size + 1

        def parameters(count):
            if len(raw) != operand_start + count:
                raise UnsupportedPath(f"unexpected {name} record size at {command.address.label()}")
            return raw[operand_start:]

        def next_cursor():
            if len(command.successors) != 1:
                raise UnsupportedPath(f"unexpected {name} edges at {command.address.label()}")
            return cursor(command.successors[0])

        def branch_cursors(target):
            fallthrough = PathAddress((command.address.offset + len(raw)) & 0xFFFF)
            destination = PathAddress(target)
            if set(command.successors) != {fallthrough, destination}:
                raise UnsupportedPath(f"unexpected {name} branch edges at {command.address.label()}")
            return cursor(destination), cursor(fallthrough)

        if name == "ExplodeObject":
            parameters(0)
            if command.successors:
                raise UnsupportedPath(f"unexpected death-path successor at {command.address.label()}")
            statement = "Statement::MarkForDeath"
        elif name in ("SetExternal1d72", "ClearExternal1d72"):
            if name == "SetExternal1d72":
                value, = parameters(1)
            else:
                parameters(0)
                value = 0
            statement = f"Statement::SetActionGate {{ value: {value}, next: {next_cursor()} }}"
        elif name in ("IfExternal1d72Equal", "IfExternal1d72NotEqual", "IfExternal1d72Zero"):
            if name == "IfExternal1d72Zero":
                low, high = parameters(2)
                value = 0
            else:
                value, low, high = parameters(3)
            condition = "NotEqual" if name == "IfExternal1d72NotEqual" else "Equal"
            taken, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::ActionGateBranch {{ condition: ActionGateCondition::{condition}({value}), taken: {taken}, next: {next_} }}"
        elif name in ("OrExternalD77d", "ClearExternalD77dBits", "ClearExternalD77d"):
            if name == "ClearExternalD77d":
                parameters(0)
                operation = "Reset"
            else:
                low, high = parameters(2)
                mask = low | (high << 8)
                operation = f"{'Raise' if name == 'OrExternalD77d' else 'Clear'}({mask})"
            statement = f"Statement::EncounterSignal {{ command: EncounterSignalCommand::{operation}, next: {next_cursor()} }}"
        elif name in ("IfExternalD77dBitsSet", "IfExternalD77dBitsClear"):
            low, high, target_low, target_high = parameters(4)
            taken, next_ = branch_cursors(target_low | (target_high << 8))
            condition = "AnyRaised" if name == "IfExternalD77dBitsSet" else "AllClear"
            statement = f"Statement::EncounterSignalBranch {{ condition: EncounterSignalCondition::{condition}({low | (high << 8)}), taken: {taken}, next: {next_} }}"
        elif name in ("Become", "BecomeLinked"):
            parameters(0)
            selection = "LastSpawn" if name == "Become" else "Linked"
            statement = f"Statement::SelectActor {{ selection: ActorSelection::{selection}, next: {next_cursor()} }}"
        elif name == "BecomeMotherOrGoto":
            low, high = parameters(2)
            missing, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::SelectActor {{ selection: ActorSelection::LinkedOrBranch {{ missing: {missing} }}, next: {next_} }}"
        elif name in ("BecomeChildLiteralOrGoto", "BecomeChildVariableOrGoto"):
            number, low, high = parameters(3)
            missing, next_ = branch_cursors(low | (high << 8))
            number_ = (f"ByteOperand::Literal({number})" if name == "BecomeChildLiteralOrGoto"
                       else f"ByteOperand::Actor({byte_field(number)})")
            statement = f"Statement::SelectChild {{ number: {number_}, missing: {missing}, next: {next_} }}"
        elif name == "Unbecome":
            parameters(0)
            statement = f"Statement::RestoreActor {{ next: {next_cursor()} }}"
        elif name == "ShapeDead":
            low, high = parameters(2)
            taken, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::AttachmentAbsent {{ taken: {taken}, next: {next_} }}"
        elif name == "IfVariableEqualsExternal1dd4":
            # Despite the legacy census name, C4BC reads a literal operand
            # byte, not an encoded actor variable. BF58 compares it directly
            # with the published active-player weapon level and takes CAFF
            # or CAA9 without consulting or consuming the IFNOT state.
            expected, low, high = parameters(3)
            taken, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::ActiveWeaponLevelEquals {{ expected: {expected}, taken: {taken}, next: {next_} }}"
        elif name == "IfExternal1dddBit80":
            low, high = parameters(2)
            taken, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::TargetingUpgradeOwned {{ taken: {taken}, next: {next_} }}"
        elif name == "SetExternal1dddBit80":
            parameters(0)
            statement = f"Statement::AcquireTargetingUpgrade {{ next: {next_cursor()} }}"
        elif name in ("QueueSelectedMarkerDirect", "QueueSelectedMarkerPair"):
            operands = parameters(1 if name == "QueueSelectedMarkerDirect" else 2)
            cue = operands[0]
            packed_parameter = operands[1] if len(operands) == 2 else 0
            target = "Secondary" if packed_parameter & 0x80 else "Primary"
            cue_ = f"AuthoredCue::new({cue}, {packed_parameter & 0x7F}, PlayerTarget::{target})"
            statement = f"Statement::Sound {{ cue: {cue_}, next: {next_cursor()} }}"
        elif name in ("QueueSelectedMarkerClass1", "QueueSelectedMarkerClass2",
                       "QueueFixedMarker1400", "QueueFixedMarker0320"):
            cue, = parameters(1)
            mode = {
                "QueueSelectedMarkerClass1": "DistanceBands(PathSoundClass::DistanceOnly)",
                "QueueSelectedMarkerClass2": "DistanceBands(PathSoundClass::Positioned)",
                "QueueFixedMarker1400": "RangeLimited(MarkerRange::Wide)",
                "QueueFixedMarker0320": "RangeLimited(MarkerRange::Near)",
            }[name]
            statement = f"Statement::MarkerSound {{ id: {cue}, mode: MarkerCueMode::{mode}, next: {next_cursor()} }}"
        elif name in ("ScheduleAlways", "ScheduleTrigger", "ScheduleRelative", "ScheduleTriggered"):
            if name == "ScheduleRelative":
                delta, condition = parameters(2)
                destination = (command.address.offset + command.prefix_size + delta) & 0xFFFF
                duration = None
            else:
                operands = parameters({"ScheduleAlways": 2, "ScheduleTrigger": 3, "ScheduleTriggered": 4}[name])
                destination = int.from_bytes(operands[:2], "little")
                condition = operands[2] if len(operands) > 2 else 0
                duration = operands[3] if len(operands) > 3 else None
            target, next_ = branch_cursors(destination)
            kind = trigger_kind(condition)
            trigger = (f"Trigger {{ path: {target}, kind: {kind}, timer: 0 }}" if duration is None
                       else f"Trigger::timed({target}, {kind}, {duration})")
            statement = f"Statement::Control(ControlCommand::Register {{ trigger: {trigger}, next: {next_} }})"
        elif name == "CancelTrigger":
            low, high = parameters(2)
            path = PathAddress(low | (high << 8))
            if path not in indices:
                raise UnsupportedPath(f"cancel target lacks catalog identity at {command.address.label()}")
            statement = f"Statement::Control(ControlCommand::Cancel {{ path: {cursor(path)}, next: {next_cursor()} }})"
        elif name == "FreeObjectAuxiliaryAndResetD742":
            parameters(0)
            statement = f"Statement::Control(ControlCommand::Clear {{ next: {next_cursor()} }})"
        elif name == "ForceTriggerPath":
            low, high = parameters(2)
            target, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::Control(ControlCommand::ForceAfterCallbacks {{ target: {target}, next: {next_} }})"
        elif name == "SetFlag26Bit08":
            parameters(0)
            statement = f"Statement::RunWhenPaused {{ enabled: true, next: {next_cursor()} }}"
        elif name in ("ConfigurePlayerAuxiliary", "ConfigurePilotAuxModeA", "ConfigurePilotAuxModeB", "RefreshOwnedPlayerAuxiliaryOrigin"):
            if name != "RefreshOwnedPlayerAuxiliaryOrigin":
                value = int.from_bytes(parameters(2), "little", signed=True)
                configuration = {"ConfigurePlayerAuxiliary": "Configure", "ConfigurePilotAuxModeA": "ConfigureDoubledLowByte", "ConfigurePilotAuxModeB": "ConfigureAlternateAxes"}[name]
                operation = f"{configuration}({value})"
            else:
                parameters(0)
                operation = "RefreshOwnedOrigin"
            statement = f"Statement::PlayerControl {{ command: PlayerControlCommand::{operation}, next: {next_cursor()} }}"
        elif name == "Inline65816":
            # The extractor checks the COMPLETE instruction signature and
            # returned continuation before exposing each reviewed action.
            actions = {
                PathAddress(0xF348): "LatchPrimaryViewFilter",
                PathAddress(0xE78A): "InheritPrimaryHorizontalMotion",
                PathAddress(0xF078): "RefreshSelectedChargeAttachment",
            }
            controls = {
                PathAddress(0xF500): "LockToProjectile",
                PathAddress(0xF391): "LockForLinkedMode",
                PathAddress(0xF39E): "FollowPrimaryPosition",
            }
            attached_motion = {
                PathAddress(0xF3F0): "Settle",
                PathAddress(0xF45B): "Tumble",
                PathAddress(0xF46E): "Center",
            }
            if command.address in (PathAddress(0x8D54), PathAddress(0x8D62)):
                parameters(0)
                enabled = str(command.address == PathAddress(0x8D54)).lower()
                statement = f"Statement::Contact {{ command: ContactCommand::ShapeFootprintSearch({enabled}), next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if command.address in attached_motion:
                parameters(0)
                statement = f"Statement::AttachedEffectMotion {{ command: super::path_steering::AttachedEffectMotion::{attached_motion[command.address]}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if command.address == PathAddress(0xF313):
                parameters(0)
                statement = f"Statement::Appearance {{ command: AppearanceCommand::SuppressDeathEffects(true), next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if command.address == PathAddress(0xF2E4):
                parameters(0)
                ordinary, flicker = PathAddress(0xF2F6), PathAddress(0xF2FD)
                if set(command.successors) != {ordinary, flicker}:
                    raise UnsupportedPath("unexpected protection effect continuation")
                statement = f"Statement::UpdateProtectionEffect {{ ordinary_return: {cursor(ordinary)}, flicker: {cursor(flicker)} }}"
                statements.append(statement)
                continue
            if command.address not in actions and command.address not in controls:
                raise UnsupportedPath(f"unported inline action at {command.address.label()}")
            parameters(0)
            if command.address in controls:
                statement = f"Statement::PlayerControl {{ command: PlayerControlCommand::{controls[command.address]}, next: {next_cursor()} }}"
            else:
                statement = f"Statement::{actions[command.address]} {{ next: {next_cursor()} }}"
        elif name in ("SpawnChild", "SpawnChildAlias"):
            spawn = child_spawn_parameters(command)
            shape, kind = spawn_shape(spawn.shape, spawn.path)
            path = f"Some({cursor(spawn.path)})" if spawn.path.offset else "None"
            x, y, z = spawn.position
            pitch, yaw, roll = spawn.rotation
            position = f"Vector3 {{ x: {x}, y: {y}, z: {z} }}"
            rotation = f"Rotation {{ pitch: Angle::from_units({pitch}), yaw: Angle::from_units({yaw}), roll: Angle::from_units({roll}) }}"
            spawn_ = f"ChildSpawn {{ shape: ShapeId::from_catalog_index({shape}), path: {path}, position: {position}, rotation: {rotation}, hit_points: {spawn.hit_points}, attack_power: {spawn.attack_power}, number: {spawn.number} }}"
            statement = f"Statement::SpawnChild {{ kind: {kind}, parameters: {spawn_}, next: {next_cursor()} }}"
        elif name in ("IfSelectedAuxiliaryMapCellOccupied", "IfCurrentAtOrAboveCollisionTarget"):
            low, high = parameters(2)
            taken, next_ = branch_cursors(low | (high << 8))
            branch = "OccupiedCell" if name == "IfSelectedAuxiliaryMapCellOccupied" else "AtOrAboveSurface"
            statement = f"Statement::{branch} {{ taken: {taken}, next: {next_} }}"
        elif name == "QuickSpawn":
            spawn = independent_spawn_parameters(command)
            shape, kind = spawn_shape(spawn.shape, spawn.path)
            path = f"Some({cursor(spawn.path)})" if spawn.path.offset else "None"
            spawn_ = f"IndependentSpawn {{ shape: ShapeId::from_catalog_index({shape}), path: {path}, hit_points: {spawn.hit_points}, attack_power: {spawn.attack_power} }}"
            statement = f"Statement::SpawnIndependent {{ kind: {kind}, parameters: {spawn_}, next: {next_cursor()} }}"
        elif name == "SwapVariableWords":
            first, second = parameters(2)
            statement = f"Statement::Mutate {{ mutation: Mutation::SwapWords {{ first: {word_field(first)}, second: {word_field(second)} }}, next: {next_cursor()} }}"
        elif name in ("SetVariableBit", "ClearVariableBit", "IfVariableBitSet"):
            operands = parameters(4 if name == "IfVariableBitSet" else 2)
            selector, destination = operands[:2]
            mask = f"WordOperand::IndexedBitMask {{ selector: ByteOperand::Actor({byte_field(selector)}), masks: &VARIABLE_BIT_MASKS }}"
            field = word_field(destination)
            if name == "IfVariableBitSet":
                taken, next_ = branch_cursors(int.from_bytes(operands[2:], "little"))
                condition = f"ActorCondition::AnyWordBitsSet(WordOperand::Actor({field}), {mask})"
                statement = f"Statement::Compare {{ condition: {condition}, taken: {taken}, next: {next_} }}"
            else:
                operation = "SetBits" if name == "SetVariableBit" else "ClearBits"
                mutation = f"Mutation::Word {{ field: {field}, operation: WordOperation::{operation}({mask}) }}"
                statement = f"Statement::Mutate {{ mutation: {mutation}, next: {next_cursor()} }}"
        elif name in (
            "SetVariableByteFromByte", "SetVariableByteFromWord",
            "SetVariableWordFromWord", "SetVariableWordFromByte",
            "AddVariableByteFromByte", "AddVariableByteFromByteAlias",
            "AddVariableWordFromWord", "AddVariableWordFromByte",
        ):
            # Shared source helper $7F:8995 resolves the SECOND operand into
            # the source and the FIRST into the destination for every width.
            destination, source = parameters(2)
            wide_destination = "VariableWord" in name
            wide_source = name.endswith("FromWord")
            kind = "Word" if wide_destination else "Byte"
            operation = "Assign" if name.startswith("Set") else "Add"
            field = word_field(destination) if wide_destination else byte_field(destination)
            if wide_destination:
                value = (f"WordOperand::Actor({word_field(source)})" if wide_source else
                         f"WordOperand::SignedByte(ByteOperand::Actor({byte_field(source)}))")
            else:
                value = (f"ByteOperand::LowWord({word_field(source)})" if wide_source else
                         f"ByteOperand::Actor({byte_field(source)})")
            mutation = f"Mutation::{kind} {{ field: {field}, operation: {kind}Operation::{operation}({value}) }}"
            statement = f"Statement::Mutate {{ mutation: {mutation}, next: {next_cursor()} }}"
        elif name in ("AddRotationX", "AddRotationY", "AddRotationZ", "AddWorldX", "AddWorldY", "AddWorldZ", "AddSignedByteToWord"):
            if name == "AddSignedByteToWord":
                variable, value = parameters(2)
                field = word_field(variable)
            else:
                value, = parameters(1)
                axis = name[-1]
                field = f"ByteField::Rotation(Axis::{axis})" if name.startswith("AddRotation") else f"WordField::Position(Axis::{axis})"
            if name.startswith("AddRotation"):
                mutation = f"Mutation::Byte {{ field: {field}, operation: ByteOperation::Add(ByteOperand::Literal({value})) }}"
            else:
                mutation = f"Mutation::Word {{ field: {field}, operation: WordOperation::Add(WordOperand::SignedByte(ByteOperand::Literal({value}))) }}"
            statement = f"Statement::Mutate {{ mutation: {mutation}, next: {next_cursor()} }}"
        elif name in ("SetByte", "SetWord", "AddByte", "AddWord", "SetZeroByte", "SetZeroWord"):
            wide = name.endswith("Word")
            kind = "Word" if wide else "Byte"
            if name.startswith("SetZero"):
                variable, = parameters(1)
                value = 0
            else:
                operands = parameters(3 if wide else 2)
                # SET reads the destination after its literal; ADD reads it
                # before the literal. Both orders are independently sourced.
                if name.startswith("Set"):
                    variable = operands[-1]
                    value = int.from_bytes(operands[:-1], "little")
                else:
                    variable = operands[0]
                    value = int.from_bytes(operands[1:], "little")
            operation = "Assign" if name.startswith("Set") else "Add"
            if name == "SetWord" and variable == 0x04:
                # A literal shape header becomes a semantic catalog ID. Do
                # not expose raw shape pointers as ordinary word arithmetic.
                statement = f"Statement::Appearance {{ command: AppearanceCommand::Shape(ShapeId::from_catalog_index({shape_index(value)})), next: {next_cursor()} }}"
            elif name == "SetWord" and variable == 0x8C:
                # Shared helper $09:81B6 selects these two bank-01 material
                # tables. $7F:1451 copies the field into the render record.
                # Do not expose pointer arithmetic or infer other table roots.
                if value not in (0x8404, 0x8498):
                    raise UnsupportedPath(f"unreviewed material table {value:04X}")
                statement = f"Statement::Appearance {{ command: AppearanceCommand::MaterialSet(super::render::MaterialSetId::from_catalog_token({value})), next: {next_cursor()} }}"
            else:
                field = word_field(variable) if wide else byte_field(variable)
                mutation = f"Mutation::{kind} {{ field: {field}, operation: {kind}Operation::{operation}({kind}Operand::Literal({value})) }}"
                statement = f"Statement::Mutate {{ mutation: {mutation}, next: {next_cursor()} }}"
        elif name in ("IfSelectedAuxiliaryContinuation", "IfSelectedAuxBit40", "IfSelectedAuxiliaryFlag04Clear", "IfSelectedSlotClass1", "IfSelectedSlotClass2", "IfSelectedSlotClass3"):
            low, high = parameters(2)
            taken, next_ = branch_cursors(low | (high << 8))
            condition = {
                "IfSelectedAuxiliaryContinuation": "Continuation",
                "IfSelectedAuxBit40": "ActionBit40",
                "IfSelectedAuxiliaryFlag04Clear": "ActionBit04Clear",
                "IfSelectedSlotClass1": "ModeClass(super::path_conditions::AuxiliaryModeClass::One)",
                "IfSelectedSlotClass2": "ModeClass(super::path_conditions::AuxiliaryModeClass::Two)",
                "IfSelectedSlotClass3": "ModeClass(super::path_conditions::AuxiliaryModeClass::Three)",
            }[name]
            statement = f"Statement::SelectedAuxiliaryBranch {{ condition: SelectedAuxiliaryCondition::{condition}, taken: {taken}, next: {next_} }}"
        elif name == "AdvanceSelectedAuxiliaryOrGotoWhenSettled":
            amount, low, high = parameters(3)
            taken, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::CollectSelectedConsumables {{ amount: {amount}, already_full: {taken}, next: {next_} }}"
        elif name == "SaturatingAddSelectedAuxWord":
            low, high = parameters(2)
            statement = f"Statement::AwardSelectedScore {{ points: {low | (high << 8)}, next: {next_cursor()} }}"
        elif name == "IncrementSelectedAuxiliaryStage":
            parameters(0)
            statement = f"Statement::UpgradeSelectedWeapon {{ next: {next_cursor()} }}"
        elif name in ("SetSelectedSlotLowNibble1", "SetSelectedSlotLowNibble4", "ClearSelectedAuxiliaryFlag01"):
            parameters(0)
            operation = {
                "SetSelectedSlotLowNibble1": "SetModeLowNibbleOne",
                "SetSelectedSlotLowNibble4": "SetModeLowNibbleFour",
                "ClearSelectedAuxiliaryFlag01": "ClearActionBit01",
            }[name]
            statement = f"Statement::SelectedAuxiliary {{ command: SelectedAuxiliaryCommand::{operation}, next: {next_cursor()} }}"
        elif name in ("MessageLiteral", "MessageVariable"):
            value, = parameters(1)
            number = f"ByteOperand::Literal({value})" if name == "MessageLiteral" else f"ByteOperand::Actor({byte_field(value)})"
            statement = f"Statement::Message {{ number: {number}, next: {next_cursor()} }}"
        elif name == "UpdatePlayerTargetFlag08":
            parameters(0)
            statement = f"Statement::ConsiderPrimaryTarget {{ next: {next_cursor()} }}"
        elif name == "ChildDead":
            number, low, high = parameters(3)
            taken, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::ChildMissing {{ number: {number}, taken: {taken}, next: {next_} }}"
        elif name in ("UnlinkSelf", "UnlinkChild", "RemoveChild", "FlagLinkedObject", "FlagMother", "FlagChild", "RefreshLinkedRotationDeltas", "ClearObjectRelativeReference", "SelectSelfAndClearRelativeTransform"):
            if name not in ("UnlinkChild", "FlagChild", "RemoveChild"):
                parameters(0)
                command_ = "RelationshipCommand::" + {
                    "UnlinkSelf": "UnlinkSelf",
                    "FlagLinkedObject": "SignalLinked",
                    "FlagMother": "SignalLinked",
                    "RefreshLinkedRotationDeltas": "RefreshLinkedRotation",
                    "ClearObjectRelativeReference": "ClearRelativeReference",
                    "SelectSelfAndClearRelativeTransform": "UseSelfRelativeFrame",
                }[name]
            else:
                number, = parameters(1)
                operation = {"FlagChild": "SignalChild", "UnlinkChild": "UnlinkChild", "RemoveChild": "RetireChild"}[name]
                command_ = f"RelationshipCommand::{operation} {{ number: {number} }}"
            statement = f"Statement::Relationship {{ command: {command_}, next: {next_cursor()} }}"
        elif name == "Gosub":
            low, high = parameters(2)
            target, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::Control(ControlCommand::Call {{ target: {target}, next: {next_} }})"
        elif name == "NoOp0B8":
            # Source AFF6 calls the single RTS at AFFC and immediately
            # advances. This is a verified source no-op, not a fallback for
            # any unimplemented handler.
            parameters(0)
            statement = f"Statement::Control(ControlCommand::Jump {{ target: {next_cursor()} }})"
        elif name == "SetObject1ce3":
            value, = parameters(1)
            field = byte_field(0xA2)
            statement = f"Statement::Mutate {{ mutation: Mutation::Byte {{ field: {field}, operation: ByteOperation::Assign(ByteOperand::Literal({value})) }}, next: {next_cursor()} }}"
        elif name in ("Goto", "GotoImmediate"):
            low, high = parameters(2)
            target = PathAddress(low | (high << 8))
            if set(command.successors) != {target}:
                raise UnsupportedPath(f"unexpected {name} destination at {command.address.label()}")
            operation = "Goto" if name == "Goto" else "Jump"
            statement = f"Statement::Control(ControlCommand::{operation} {{ target: {cursor(target)} }})"
        elif name == "Return":
            parameters(0)
            if command.successors:
                raise UnsupportedPath(f"RETURN has static outgoing edges at {command.address.label()}")
            statement = "Statement::Control(ControlCommand::Return)"
        elif name == "Break":
            low, high = parameters(2)
            target = PathAddress(low | (high << 8))
            if set(command.successors) != {target}:
                raise UnsupportedPath(f"unexpected BREAK edges at {command.address.label()}")
            statement = f"Statement::Control(ControlCommand::Break {{ target: {cursor(target)} }})"
        elif name == "PopPathStackPair":
            parameters(0)
            statement = f"Statement::Control(ControlCommand::PopStackPair {{ next: {next_cursor()} }})"
        elif name in ("PushByte", "PushWord", "PullByte", "PullWord"):
            variable, = parameters(1)
            wide = name.endswith("Word")
            field = word_field(variable) if wide else byte_field(variable)
            operation = ("Save" if name.startswith("Push") else "Restore") + ("Word" if wide else "Byte")
            statement = f"Statement::StackValue {{ command: super::path_commands::StackValueCommand::{operation}({field}), next: {next_cursor()} }}"
        elif name in ("IfHitFlag", "IfFlag23Bit08"):
            operands = parameters(3 if name == "IfHitFlag" else 2)
            taken, next_ = branch_cursors(int.from_bytes(operands[:2], "little"))
            branch = f"HitFlags {{ mask: {operands[2]}," if name == "IfHitFlag" else "HitEvent {"
            statement = f"Statement::Branch(BranchCommand::{branch} taken: {taken}, next: {next_} }})"
        elif name == "IfExternalC4BitsSet":
            mask, low, high = parameters(3)
            taken, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::ClockBitsSet {{ mask: {mask}, taken: {taken}, next: {next_} }}"
        elif name == "IfNot":
            parameters(0)
            statement = f"Statement::Branch(BranchCommand::InvertNext {{ next: {next_cursor()} }})"
        elif name == "RandomGoto":
            low, high = parameters(2)
            taken, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::RandomBranch {{ taken: {taken}, next: {next_} }}"
        elif name in ("SetRandomByte", "SetRandomWord", "AddCenteredRandomByte", "AddCenteredRandomWord"):
            wide = name.endswith("Word")
            operands = parameters(3 if wide else 2)
            field = word_field(operands[0]) if wide else byte_field(operands[0])
            mask = int.from_bytes(operands[1:], "little")
            operation = ("Assign" if name.startswith("Set") else "AddCentered") + ("Word" if wide else "Byte")
            statement = f"Statement::Random {{ mutation: RandomMutation::{operation} {{ field: {field}, mask: {mask} }}, next: {next_cursor()} }}"
        elif name in ("IncrementByte", "IncrementWord", "DecrementByte", "DecrementWord", "NegateByte", "NegateWord"):
            variable, = parameters(1)
            wide = name.endswith("Word")
            kind = "Word" if wide else "Byte"
            operation = name.removesuffix(kind)
            field = word_field(variable) if wide else byte_field(variable)
            mutation = f"Mutation::{kind} {{ field: {field}, operation: {kind}Operation::{operation} }}"
            statement = f"Statement::Mutate {{ mutation: {mutation}, next: {next_cursor()} }}"
        elif name in ("DivideByteByTwo", "DivideWordByTwo", "ShiftByteRight"):
            variable, = parameters(1)
            wide = name == "DivideWordByTwo"
            kind = "Word" if wide else "Byte"
            field = word_field(variable) if wide else byte_field(variable)
            operation = "LogicalHalf" if name == "ShiftByteRight" else "HalfTowardZero"
            mutation = f"Mutation::{kind} {{ field: {field}, operation: {kind}Operation::{operation} }}"
            statement = f"Statement::Mutate {{ mutation: {mutation}, next: {next_cursor()} }}"
        elif name in ("IfZeroByte", "IfZeroWord", "IfNotZeroByte", "IfNotZeroWord"):
            variable, low, high = parameters(3)
            taken, next_ = branch_cursors(low | (high << 8))
            wide = name.endswith("Word")
            kind = "Word" if wide else "Byte"
            field = word_field(variable) if wide else byte_field(variable)
            condition = ("Nonzero" if name.startswith("IfNot") else "Zero") + kind
            statement = f"Statement::Compare {{ condition: ActorCondition::{condition}({kind}Operand::Actor({field})), taken: {taken}, next: {next_} }}"
        elif name == "IfSameByte":
            variable, expected, low, high = parameters(4)
            taken, next_ = branch_cursors(low | (high << 8))
            condition = f"ActorCondition::EqualByte(ByteOperand::Actor({byte_field(variable)}), ByteOperand::Literal({expected}))"
            statement = f"Statement::Compare {{ condition: {condition}, taken: {taken}, next: {next_} }}"
        elif name == "IfSameWord":
            operands = parameters(5)
            variable = operands[0]
            expected = int.from_bytes(operands[1:3], "little")
            taken, next_ = branch_cursors(int.from_bytes(operands[3:], "little"))
            if variable == 0x04:
                condition = f"ActorCondition::EqualShape(ShapeId::from_catalog_index({shape_index(expected)}))"
            else:
                condition = f"ActorCondition::EqualWord(WordOperand::Actor({word_field(variable)}), WordOperand::Literal({expected}))"
            statement = f"Statement::Compare {{ condition: {condition}, taken: {taken}, next: {next_} }}"
        elif name in ("IfBetweenByte", "IfBetweenWord"):
            wide = name == "IfBetweenWord"
            width = 2 if wide else 1
            operands = parameters(3 + 2 * width)
            field = word_field(operands[0]) if wide else byte_field(operands[0])
            lower = int.from_bytes(operands[1:1 + width], "little")
            upper = int.from_bytes(operands[1 + width:1 + 2 * width], "little")
            taken, next_ = branch_cursors(int.from_bytes(operands[-2:], "little"))
            kind = "Word" if wide else "Byte"
            condition = f"ActorCondition::Between{kind} {{ value: {kind}Operand::Actor({field}), lower: {kind}Operand::Literal({lower}), upper: {kind}Operand::Literal({upper}) }}"
            statement = f"Statement::Compare {{ condition: {condition}, taken: {taken}, next: {next_} }}"
        elif name in ("IfVariableBytesSame", "IfVariableWordsSame", "IfVariableBytesLess", "IfVariableWordsLess"):
            first, second, low, high = parameters(4)
            wide = "Words" in name
            kind = "Word" if wide else "Byte"
            field = word_field if wide else byte_field
            predicate = f"Equal{kind}" if name.endswith("Same") else f"Second{kind}Less"
            taken, next_ = branch_cursors(low | (high << 8))
            condition = f"ActorCondition::{predicate}({kind}Operand::Actor({field(first)}), {kind}Operand::Actor({field(second)}))"
            statement = f"Statement::Compare {{ condition: {condition}, taken: {taken}, next: {next_} }}"
        elif name in (
            "IfSelectedDistanceLess", "IfMotherDistanceLess", "IfHitGround",
            "IfSelectedWithinRange", "IfSelectedWithinYawArc", "IfSelectedRelativeYawBetween",
            "IfSelectedAboveObject", "IfSelectedAtOrBelowObject",
            "IfProjectedSelectedPointNegative", "IfProjectedSelectedForwardPointNegative",
        ):
            word_conditions = {
                "IfSelectedDistanceLess": "SelectedDistanceLess",
                "IfMotherDistanceLess": "LinkedDistanceLess",
                "IfHitGround": "GroundThreshold",
                "IfSelectedWithinRange": "WithinSelectedRange",
            }
            if name in word_conditions:
                operands = parameters(4)
                value = int.from_bytes(operands[:2], "little", signed=name == "IfHitGround")
                condition = f"{word_conditions[name]}({value})"
            elif name == "IfSelectedWithinYawArc":
                operands = parameters(3)
                condition = f"SelectedWithinYawArc({operands[0]})"
            elif name == "IfSelectedRelativeYawBetween":
                operands = parameters(4)
                condition = f"SelectedRelativeYawBetween {{ lower: {operands[0]}, upper: {operands[1]} }}"
            else:
                operands = parameters(2)
                condition = {
                    "IfSelectedAboveObject": "SelectedAbove",
                    "IfSelectedAtOrBelowObject": "SelectedAtOrBelow",
                    "IfProjectedSelectedPointNegative": "NegativeSelectedPlane(PlaneAxis::Right)",
                    "IfProjectedSelectedForwardPointNegative": "NegativeSelectedPlane(PlaneAxis::Forward)",
                }[name]
            taken, next_ = branch_cursors(int.from_bytes(operands[-2:], "little"))
            statement = f"Statement::Spatial {{ condition: SpatialCondition::{condition}, taken: {taken}, next: {next_} }}"
        elif name in ("CopySelectedWorldPosition", "CopySelectedRotation", "RefreshSelectedRelativeTransform"):
            parameters(0)
            operation = {"CopySelectedWorldPosition": "WorldPosition", "CopySelectedRotation": "WorldRotation", "RefreshSelectedRelativeTransform": "RelativeFrame"}[name]
            statement = f"Statement::CopySelectedTransform {{ command: SelectedTransformCommand::{operation}, next: {next_cursor()} }}"
        elif name in ("AchaseByte", "AchaseWord", "WaitAchaseByte", "ChaseVariableByte", "ChaseVariableWord"):
            wide = name.endswith("Word")
            kind = "Word" if wide else "Byte"
            if name.startswith("ChaseVariable"):
                destination, source = parameters(2)
                field = word_field(destination) if wide else byte_field(destination)
                source_field = word_field(source) if wide else byte_field(source)
                target = f"{kind}Operand::Actor({source_field})"
            else:
                operands = parameters(3 if wide else 2)
                field = word_field(operands[-1]) if wide else byte_field(operands[-1])
                target = f"{kind}Operand::Literal({int.from_bytes(operands[:-1], 'little')})"
            if name == "WaitAchaseByte":
                statement = f"Statement::WaitChase {{ field: {field}, target: {target}, next: {next_cursor()} }}"
            else:
                statement = f"Statement::Mutate {{ mutation: Mutation::{kind} {{ field: {field}, operation: {kind}Operation::Chase({target}) }}, next: {next_cursor()} }}"
        elif name == "SetFlag20Bit02":
            parameters(0)
            statement = f"Statement::Contact {{ command: ContactCommand::MarkHit, next: {next_cursor()} }}"
        elif name in ("MaskFlag31", "OrFlag31"):
            mask, = parameters(1)
            retain = name == "MaskFlag31"
            # Bit 02 has no reviewed domain meaning. Permit preserving it,
            # never silently discard an authored mutation to that source bit.
            if bool(mask & 2) != retain:
                raise UnsupportedPath(f"unreviewed contact class bit 02 at {command.address.label()}")
            operation = "RetainClass" if retain else "IncludeClass"
            selection = f"ContactClassMask {{ groups: ExclusionGroups::from_authored_class({mask & 0xF8}), first_strategy_visit: {str(bool(mask & 4)).lower()}, suppress_attack_damage: {str(bool(mask & 1)).lower()} }}"
            statement = f"Statement::Contact {{ command: ContactCommand::{operation}({selection}), next: {next_cursor()} }}"
        elif name in ("SetFlag22Bit08", "ClearFlag22Bit08", "SetFlag24Bit08"):
            parameters(0)
            operation = "SuppressHitMarker" if name == "SetFlag24Bit08" else "SuppressContactsNextEpoch"
            enabled = "false" if name == "ClearFlag22Bit08" else "true"
            statement = f"Statement::Contact {{ command: ContactCommand::{operation}({enabled}), next: {next_cursor()} }}"
        elif name in ("FaceSelectedImmediate", "FaceSelectedSmooth", "FacePlayerYaw", "FacePlayer", "FaceLinkedSmooth", "FaceMother"):
            parameters(0)
            operation = {
                "FaceSelectedImmediate": "SelectedImmediate",
                "FaceSelectedSmooth": "SelectedSmooth",
                "FacePlayerYaw": "SelectedYaw",
                "FacePlayer": "FixedPlayerImmediate",
                "FaceLinkedSmooth": "LinkedSmooth",
                "FaceMother": "LinkedImmediate",
            }[name]
            statement = f"Statement::Facing {{ command: FacingCommand::{operation}, next: {next_cursor()} }}"
        elif name in ("RotateLocalOffsetYaw", "RotateAroundSelectedYaw", "RotateAroundSelectedYawVariable"):
            value, = parameters(1)
            center = "LocalOrigin" if name == "RotateLocalOffsetYaw" else "Selected"
            operand = f"ByteOperand::Actor({byte_field(value)})" if name.endswith("Variable") else f"ByteOperand::Literal({value})"
            statement = f"Statement::YawOrbit {{ center: OrbitCenter::{center}, angle: {operand}, next: {next_cursor()} }}"
        elif name in ("ContractLocalRadius", "ContractSelectedRadius", "ContractLinkedRadius"):
            value, = parameters(1)
            center = {"ContractLocalRadius": "LocalOrigin", "ContractSelectedRadius": "Selected", "ContractLinkedRadius": "Linked"}[name]
            amount = value if value < 128 else value - 256
            statement = f"Statement::Radius {{ command: RadiusCommand {{ center: RadiusCenter::{center}, amount: {amount} }}, next: {next_cursor()} }}"
        elif name in ("ImportWordIndexed", "ExportWordIndexed"):
            variable, index = parameters(2)
            # $7F:9FE7 widens the second literal without scaling it. Only
            # reviewed live domain fields are mapped, never shared RAM.
            if index == 0x36:
                operation = f"CopyTo({word_field(variable)})" if name.startswith("Import") else f"Assign(WordOperand::Actor({word_field(variable)}))"
                statement = f"Statement::Guidance {{ command: GuidanceCommand::{operation}, next: {next_cursor()} }}"
            elif index == 0x32:
                operation = f"CopyTo({word_field(variable)})" if name.startswith("Import") else f"Assign(WordOperand::Actor({word_field(variable)}))"
                statement = f"Statement::PickupHistory {{ command: super::path_program::PickupHistoryCommand::{operation}, next: {next_cursor()} }}"
            elif index == 0x9A and name.startswith("Import"):
                statement = f"Statement::ImportActiveNodeFlags {{ destination: {word_field(variable)}, next: {next_cursor()} }}"
            elif index in (0x90, 0x92, 0x94) and name.startswith("Import"):
                axis = {0x90: "X", 0x92: "Y", 0x94: "Z"}[index]
                statement = f"Statement::ImportPlayerPosition {{ axis: Axis::{axis}, destination: {word_field(variable)}, next: {next_cursor()} }}"
            else:
                raise UnsupportedPath(f"unported shared word {0xD75C + index:04X} at {command.address.label()}")
        elif name == "InitializePlayerAuxWord":
            amount = int.from_bytes(parameters(2), "little", signed=True)
            statement = f"Statement::InitializePrimaryPitchRecoil {{ amount: {amount}, next: {next_cursor()} }}"
        elif name == "ImportWordAbsolute":
            variable, low, high = parameters(3)
            address = low | (high << 8)
            if address == 0x12C3 and variable == 0x1C:
                statement = f"Statement::LinkPrimaryCollisionExclusion {{ next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address == 0x1E0F:
                statement = f"Statement::ImportEnvironmentPlaneHeight {{ destination: {word_field(variable)}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address in (0xD7EC, 0xD7EE, 0xD7F0):
                axis = {0xD7EC: "X", 0xD7EE: "Y", 0xD7F0: "Z"}[address]
                statement = f"Statement::ImportPlayerPosition {{ axis: Axis::{axis}, destination: {word_field(variable)}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            axes = {0x1E1C: "X", 0x1E1E: "Y", 0x1E20: "Z"}
            if address not in axes:
                raise UnsupportedPath(f"unported shared word {address:04X} at {command.address.label()}")
            statement = f"Statement::ImportPlayerMotion {{ axis: Axis::{axes[address]}, destination: {word_field(variable)}, next: {next_cursor()} }}"
        elif name == "AddVariableByteToExternalByte":
            low, high, variable = parameters(3)
            address = low | (high << 8)
            if address != 0x1E1B:
                raise UnsupportedPath(f"unported shared byte addition {address:04X} at {command.address.label()}")
            statement = f"Statement::AccumulateShieldRecovery {{ amount: ByteOperand::Actor({byte_field(variable)}), next: {next_cursor()} }}"
        elif name in ("ImportByteAbsolute", "ImportByteIndexed", "ExportByteAbsolute",
                       "ExportByteIndexed", "StoreExternalByte", "IncrementExternalByte",
                       "DecrementExternalByte"):
            if name in ("ImportByteAbsolute", "ExportByteAbsolute"):
                variable, low, high = parameters(3)
                address = low | (high << 8)
            elif name in ("ImportByteIndexed", "ExportByteIndexed"):
                variable, index = parameters(2)
                address = 0xD75C + index
            elif name == "StoreExternalByte":
                low, high, value = parameters(3)
                address = low | (high << 8)
            else:
                low, high = parameters(2)
                address = low | (high << 8)
            if name == "ImportByteAbsolute" and 0x1E1C <= address <= 0x1E21:
                axis = ("X", "Y", "Z")[(address - 0x1E1C) // 2]
                part = "High" if address & 1 else "Low"
                statement = f"Statement::ImportPlayerMotionByte {{ axis: Axis::{axis}, part: BytePart::{part}, destination: {byte_field(variable)}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address == 0x1DD6 and name == "ImportByteAbsolute":
                statement = f"Statement::ImportChargeThreshold {{ destination: {byte_field(variable)}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address == 0xD78C and name in ("ImportByteIndexed", "ExportByteIndexed"):
                operation = f"CopyTo({byte_field(variable)})" if name.startswith("Import") else f"Assign(ByteOperand::Actor({byte_field(variable)}))"
                statement = f"Statement::SceneryDistance {{ command: super::path_program::SceneryDistanceCommand::{operation}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address in (0x1DE2, 0x1BB5) and name == "ImportByteAbsolute":
                source = "PlayerConfiguration" if address == 0x1DE2 else "EncounterLocation"
                statement = f"Statement::ImportSceneByte {{ source: super::path_program::SceneByte::{source}, destination: {byte_field(variable)}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address == 0x1D72 and name == "ImportByteAbsolute":
                statement = f"Statement::ImportActionGate {{ destination: {byte_field(variable)}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address == 0x1E1B and name == "StoreExternalByte":
                statement = f"Statement::RequestShieldRecovery {{ amount: ByteOperand::Literal({value}), next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address == 0x1DDF and name in ("ImportByteAbsolute", "StoreExternalByte"):
                operation = f"CopyTo({byte_field(variable)})" if name == "ImportByteAbsolute" else f"Assign(ByteOperand::Literal({value}))"
                statement = f"Statement::LinkedEffectActivity {{ command: super::path_protection::ActivityCommand::{operation}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address == 0x1DD0 and name == "ImportByteAbsolute":
                statement = f"Statement::ImportControlStyle {{ destination: {byte_field(variable)}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address == 0x1E59 and name in ("ImportByteAbsolute", "StoreExternalByte"):
                operation = f"CopyTo({byte_field(variable)})" if name == "ImportByteAbsolute" else f"Assign(ByteOperand::Literal({value}))"
                statement = f"Statement::ProjectileTrigger {{ command: super::path_program::ProjectileTriggerCommand::{operation}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address == 0x1B4D and name == "ImportByteAbsolute":
                statement = f"Statement::ImportSurfaceMode {{ destination: {byte_field(variable)}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address in (0xD7F2, 0x1C06) and name.startswith("Import"):
                source = "Difficulty" if address == 0xD7F2 else "EncounterVariant"
                statement = f"Statement::ImportCampaignByte {{ source: CampaignByte::{source}, destination: {byte_field(variable)}, next: {next_cursor()} }}"
                statements.append(statement)
                continue
            if address != 0xD786:
                raise UnsupportedPath(f"unported shared byte {address:04X} at {command.address.label()}")
            if name.startswith("Import"):
                operation = f"CopyTo({byte_field(variable)})"
            elif name.startswith("Export"):
                operation = f"Assign(ByteOperand::Actor({byte_field(variable)}))"
            elif name == "StoreExternalByte":
                operation = f"Assign(ByteOperand::Literal({value}))"
            else:
                operation = "Increment" if name == "IncrementExternalByte" else "Decrement"
            statement = f"Statement::Countdown {{ command: CountdownCommand::{operation}, next: {next_cursor()} }}"
        elif name in ("AddIndexedByteAndAdvanceFrame", "AddIndexedSignedByteAndAdvanceFrame"):
            low, high, bank, selector, destination, period = parameters(6)
            wide = name == "AddIndexedSignedByteAndAdvanceFrame"
            kind = "SignedWord" if wide else "Byte"
            values = banked_byte_values(extractor.rom, low | (high << 8) | (bank << 16))
            field = (word_field if wide else byte_field)(destination)
            mutation = f"Mutation::IndexedAddAndAdvance {{ field: super::path_fields::IndexedAddField::{kind}({field}), selector: {byte_field(selector)}, values: &[{', '.join(map(str, values))}], period: {period} }}"
            statement = f"Statement::Mutate {{ mutation: {mutation}, next: {next_cursor()} }}"
        elif name in ("IndexByteBanked", "IndexWordBanked"):
            low, high, bank, selector, destination = parameters(5)
            wide = name == "IndexWordBanked"
            kind = "Word" if wide else "Byte"
            values = (banked_word_values if wide else banked_byte_values)(
                extractor.rom, low | (high << 8) | (bank << 16))
            field = (word_field if wide else byte_field)(destination)
            operand = f"{kind}Operand::Lookup {{ selector: {byte_field(selector)}, values: &[{', '.join(map(str, values))}] }}"
            statement = f"Statement::Mutate {{ mutation: Mutation::{kind} {{ field: {field}, operation: {kind}Operation::Assign({operand}) }}, next: {next_cursor()} }}"
        elif name == "WriteObject1ccc":
            value, = parameters(1)
            statement = f"Statement::SpatialLoop {{ sound: super::SpatialLoop::from_authored_control({value}), next: {next_cursor()} }}"
        elif name == "SetObjectBytes0a0b":
            target, amount = parameters(2)
            statement = f"Statement::Motion {{ command: MotionCommand::AccelerateTo {{ target: {target}, amount: {amount} }}, next: {next_cursor()} }}"
        elif name == "SetVelocity":
            speed, = parameters(1)
            statement = f"Statement::Motion {{ command: MotionCommand::SetSpeed({speed}), next: {next_cursor()} }}"
        elif name == "SetWeapon":
            weapon, = parameters(1)
            statement = f"Statement::Mutate {{ mutation: Mutation::Byte {{ field: ByteField::WeaponSelection, operation: ByteOperation::Assign(ByteOperand::Literal({weapon})) }}, next: {next_cursor()} }}"
        elif name in (
            "FollowPlayerDisplacementOn", "FollowPlayerDisplacementOff",
            "GenerateVelocityEachStepOn", "GenerateVelocityEachStepOff",
            "HelicopterOn", "HelicopterOff", "SetFlag26Bit80",
        ):
            parameters(0)
            operation, enabled = {
                "FollowPlayerDisplacementOn": ("FollowPlayerDisplacement", True),
                "FollowPlayerDisplacementOff": ("FollowPlayerDisplacement", False),
                "GenerateVelocityEachStepOn": ("GenerateVelocityEachStep", True),
                "GenerateVelocityEachStepOff": ("GenerateVelocityEachStep", False),
                "HelicopterOn": ("BankTurn", True),
                "HelicopterOff": ("BankTurn", False),
                "SetFlag26Bit80": ("QuadrupleVelocity", True),
            }[name]
            statement = f"Statement::Motion {{ command: MotionCommand::{operation}({str(enabled).lower()}), next: {next_cursor()} }}"
        elif name == "Trail":
            value, = parameters(1)
            statement = f"Statement::Appearance {{ command: AppearanceCommand::RadarMarker(super::radar::RadarMarker::from_packed({value})), next: {next_cursor()} }}"
        elif name == "DisableCollision":
            parameters(0)
            statement = f"Statement::DisableCollision {{ next: {next_cursor()} }}"
        elif name == "SetObject1cef":
            parameters(0)
            statement = f"Statement::Appearance {{ command: AppearanceCommand::ClippingPlane(super::render::ClippingPlaneSelection::FIRST), next: {next_cursor()} }}"
        elif name in ("InvisibleOn", "InvisibleOff", "ClearFlag21Bit01", "SetFlag20Bit08", "ClearFlag20Bit08", "SetFlag26Bit10", "ClearFlag26Bit10", "SetFlag09Bit01", "ClearFlag09Bit01", "SetFlag26Bit20", "ClearFlag26Bit20"):
            parameters(0)
            operation, enabled = {
                "InvisibleOn": ("Visibility", False),
                "InvisibleOff": ("Visibility", True),
                "ClearFlag21Bit01": ("Collision", True),
                "SetFlag20Bit08": ("Shadow", True),
                "ClearFlag20Bit08": ("Shadow", False),
                "SetFlag26Bit10": ("MaximumDrawDistance", True),
                "SetFlag26Bit20": ("ProximityWarningSource", True),
                "ClearFlag26Bit20": ("ProximityWarningSource", False),
                "ClearFlag26Bit10": ("MaximumDrawDistance", False),
                "SetFlag09Bit01": ("FarSortBias", True),
                "ClearFlag09Bit01": ("FarSortBias", False),
            }[name]
            statement = f"Statement::Appearance {{ command: AppearanceCommand::{operation}({str(enabled).lower()}), next: {next_cursor()} }}"
        elif name == "WaitOne":
            parameters(0)
            statement = f"Statement::Control(ControlCommand::WaitOne {{ next: {next_cursor()} }})"
        elif name in ("Wait", "WaitVariable"):
            value, = parameters(1)
            duration = f"ByteOperand::Literal({value})" if name == "Wait" else f"ByteOperand::Actor({byte_field(value)})"
            statement = f"Statement::Wait {{ duration: {duration}, next: {next_cursor()} }}"
        elif name == "Sprite":
            color, size = parameters(2)
            statement = f"Statement::Sprite {{ color: {color}, size: {size}, next: {next_cursor()} }}"
        elif name == "DoQueue":
            count, = parameters(1)
            statement = f"Statement::Control(ControlCommand::BeginLoop {{ iterations: {count}, next: {next_cursor()} }})"
        elif name in ("DoVariableByte", "DoVariableWord"):
            variable, = parameters(1)
            iterations = (f"WordOperand::UnsignedByte(ByteOperand::Actor({byte_field(variable)}))"
                          if name == "DoVariableByte" else f"WordOperand::Actor({word_field(variable)})")
            statement = f"Statement::BeginLoop {{ iterations: {iterations}, next: {next_cursor()} }}"
        elif name in ("InitAnimation", "InitColorAnimation", "WriteObject1ccb80"):
            if name == "WriteObject1ccb80":
                parameters(0)
                value = 0
            else:
                value, = parameters(1)
            channel = "Color" if name == "InitColorAnimation" else "Shape"
            statement = f"Statement::Animation {{ command: AnimationCommand::Initialize {{ channel: AnimationChannel::{channel}, value: {value} }}, next: {next_cursor()} }}"
        elif name in ("AddAnimation", "AddColorAnimation"):
            amount, period = parameters(2)
            channel = "Shape" if name == "AddAnimation" else "Color"
            statement = f"Statement::Animation {{ command: AnimationCommand::Advance {{ channel: AnimationChannel::{channel}, amount: {amount}, period: {period} }}, next: {next_cursor()} }}"
        elif name in ("Next", "ImmediateNext"):
            parameters(0)
            immediate = "true" if name == "ImmediateNext" else "false"
            statement = f"Statement::Control(ControlCommand::Next {{ immediate: {immediate}, next: {next_cursor()} }})"
        elif name in ("End", "PathHold", "SetFlag26Bit40AndHold"):
            parameters(0)
            if command.successors:
                raise UnsupportedPath(f"{name} has outgoing edges at {command.address.label()}")
            operation = {
                "End": "End", "PathHold": "Hold",
                "SetFlag26Bit40AndHold": "SuspendAndMove",
            }[name]
            statement = f"Statement::Control(ControlCommand::{operation})"
        else:
            raise UnsupportedPath(f"unsupported {name} at {command.address.label()}")
        statements.append(statement)
    return indices[root], statements


def generate(rom: bytes, roots=ROOTS) -> str:
    extractor = PathExtractor(rom)
    discovered = set(extractor.discover_roots())
    declarations = []
    # One shared address-to-semantic-index layout across all lowered roots.
    # Duplicating a callee in each root graph would give the same source path
    # multiple identities, breaking callback cancellation/cursor comparisons.
    addresses = sorted({command.address for _, root in roots for command in lowering_units(extractor, root)})
    source_addresses = {command.address for _, root in roots for command in graph(extractor, root)}
    indices = {address: index for index, address in enumerate(addresses)}
    unique_statements = {}
    for name, root in roots:
        if root not in discovered:
            installer = CHILD_INSTALLERS.get(root)
            if installer is None or installer[0] not in discovered:
                raise UnsupportedPath(f"{name} has no verified source installer")
            parent, source = installer
            spawn = next((command for command in graph(extractor, parent)
                          if command.address == source), None)
            installed = (independent_spawn_parameters(spawn).path if spawn is not None and spawn.opcode == 0x05D
                         else child_spawn_parameters(spawn).path if spawn is not None else None)
            if installed != root:
                raise UnsupportedPath(f"{name} has no verified child installer")
        entry, statements = lower_graph(extractor, root, 0, indices)
        declarations.append(f"pub const {name}: PathCursor = cursor(0, {entry});")
        for command, statement in zip(lowering_units(extractor, root), statements, strict=True):
            previous = unique_statements.setdefault(command.address, statement)
            if previous != statement:
                raise UnsupportedPath(f"inconsistent shared statement at {command.address.label()}")
    source = """// @generated by tools/sf2/generate_native_paths.py; do not edit.
//! Complete statically lowered source paths. This is an explicit subset,
//! not a fallback catalog for paths that have not been ported.
use super::path_appearance::{AnimationChannel, AnimationCommand};
use super::path_commands::{BranchCommand, ControlCommand, MotionCommand};
use super::path_fields::{Axis, ByteField, ByteOperand, ByteOperation, BytePart, Mutation, WordField, WordOperation};
use super::path_program::{ActorCondition, PathCatalog, SelectedAuxiliaryCondition, Statement};
use super::path_random::RandomMutation;
use super::path_relationships::RelationshipCommand;
use super::{PathCursor, PathId};

const fn cursor(path: u16, command_index: u16) -> PathCursor {
    PathCursor { path: PathId::from_catalog_index(path), command_index }
}
"""
    if any("ActorSelection::" in statement for statement in unique_statements.values()):
        source += "use super::path_actor_context::ActorSelection;\n"
    if any("EncounterSignalCommand::" in statement for statement in unique_statements.values()):
        source += "use super::path_program::EncounterSignalCommand;\n"
    if any("ActionGateCondition::" in statement for statement in unique_statements.values()):
        source += "use super::path_program::ActionGateCondition;\n"
    if any("EncounterSignalCondition::" in statement for statement in unique_statements.values()):
        source += "use super::path_program::EncounterSignalCondition;\n"
    if any("WordOperand::" in statement for statement in unique_statements.values()):
        source += "use super::path_fields::WordOperand;\n"
    if any("AppearanceCommand::" in statement for statement in unique_statements.values()):
        source += "use super::path_appearance::AppearanceCommand;\n"
    if any("SpatialCondition::" in statement for statement in unique_statements.values()):
        source += "use super::path_conditions::SpatialCondition;\n"
    if any("FacingCommand::" in statement for statement in unique_statements.values()):
        source += "use super::path_steering::FacingCommand;\n"
    if any("OrbitCenter::" in statement for statement in unique_statements.values()):
        source += "use super::path_program::OrbitCenter;\n"
    if any("SelectedAuxiliaryCommand::" in statement for statement in unique_statements.values()):
        source += "use super::path_program::SelectedAuxiliaryCommand;\n"
    if any("CampaignByte::" in statement for statement in unique_statements.values()):
        source += "use super::path_program::CampaignByte;\n"
    if any("GuidanceCommand::" in statement for statement in unique_statements.values()):
        source += "use super::path_program::GuidanceCommand;\n"
    if any("RadiusCommand" in statement for statement in unique_statements.values()):
        source += "use super::path_steering::{RadiusCenter, RadiusCommand};\n"
    if any("PlayerControlCommand::" in statement for statement in unique_statements.values()):
        source += "use super::path_player_control::PlayerControlCommand;\n"
    if any("CountdownCommand::" in statement for statement in unique_statements.values()):
        source += "use super::path_countdown::CountdownCommand;\n"
    if any("SelectedTransformCommand::" in statement for statement in unique_statements.values()):
        source += "use super::path_relationships::SelectedTransformCommand;\n"
    if any("ContactCommand::" in statement for statement in unique_statements.values()):
        source += "use super::path_contact::ContactCommand;\n"
    if any("ContactClassMask" in statement for statement in unique_statements.values()):
        source += "use super::path_contact::ContactClassMask;\nuse super::collision_pass::ExclusionGroups;\n"
    if any("PlaneAxis::" in statement for statement in unique_statements.values()):
        source += "use super::path_control::PlaneAxis;\n"
    if any("VARIABLE_BIT_MASKS" in statement for statement in unique_statements.values()):
        source += "const VARIABLE_BIT_MASKS: [u16; 128] = ["
        source += ", ".join(f"0x{mask:04X}" for mask in variable_bit_masks(rom))
        source += "];\n"
    if any("Statement::Sound" in statement for statement in unique_statements.values()):
        source += "use super::path_sound::AuthoredCue;\nuse super::path_control::PlayerTarget;\n"
    for sound_type in ("MarkerCueMode", "PathSoundClass", "MarkerRange"):
        if any(f"{sound_type}::" in statement for statement in unique_statements.values()):
            source += f"use super::path_sound::{sound_type};\n"
    if any("ControlCommand::Register" in statement for statement in unique_statements.values()):
        source += "use super::path_triggers::{Trigger, TriggerKind};\n"
    if any("TriggerPeriod::" in statement for statement in unique_statements.values()):
        source += "use super::path_control::TriggerPeriod;\n"
    if any("Statement::SpawnChild" in statement for statement in unique_statements.values()):
        source += "use super::path_spawn::ChildSpawn;\nuse super::{Angle, ObjectKind, Rotation, ShapeId, Vector3};\n"
    elif any("Statement::SpawnIndependent" in statement for statement in unique_statements.values()):
        source += "use super::{ObjectKind, ShapeId};\n"
    elif any("ShapeId::" in statement for statement in unique_statements.values()):
        source += "use super::ShapeId;\n"
    if any("Statement::SpawnIndependent" in statement for statement in unique_statements.values()):
        source += "use super::path_spawn::IndependentSpawn;\n"
    source += "\n".join(declarations)
    source += f"\npub const LOWERED_ROOT_COUNT: usize = {len(roots)};"
    source += f"\npub const LOWERED_COMMAND_COUNT: usize = {len(unique_statements)};"
    source += f"\npub const LOWERED_SOURCE_COMMAND_COUNT: usize = {len(source_addresses)};"
    source += "\npub fn catalog() -> PathCatalog {\nPathCatalog::new(vec!["
    source += "vec![" + ",\n".join(unique_statements[address] for address in addresses) + "]"
    source += "]).expect(\"generated catalog indices fit native cursors\")\n}\n"
    return subprocess.run(
        ["rustfmt", "--edition", "2021", "--emit", "stdout"],
        input=source, text=True, capture_output=True, check=True,
    ).stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    source = generate(args.rom.read_bytes())
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != source:
            print(f"out of date: {OUTPUT}", file=sys.stderr)
            return 1
    else:
        OUTPUT.write_text(source)
    print(f"Native path catalog: {len(ROOTS)} complete source root(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
