#!/usr/bin/env python3
"""Source ordering and field selection for native path movement."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathMovementStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_velocity_write_defers_only_in_per_step_generation_mode(self):
        self.assert_source(0x7F854A, "20 BC C4 95 18 B5 21 29 10 F0 04 5C D3 CA 7F 20 5F 85 4C D3 CA")

    def test_acceleration_writes_target_and_amount_without_regenerating(self):
        self.assert_source(0x7F8C96, "20 BC C4 95 0A 20 E0 C4 95 0B 4C BE CA")

    def test_acceleration_regenerates_before_bank_turn_only_without_per_step_gate(self):
        self.assert_source(0x7F9DE8, "B5 0B D0 04 5C 1F 9E 7F")
        self.assert_source(0x7F9E12, "B5 21 29 10 F0 04 5C 1F 9E 7F 20 5F 85")
        self.assert_source(0x7F9E1F, "B5 21 29 40 D0 04 5C 3E 9E 7F")

    def test_displacement_precedes_per_step_generation_and_world_integration(self):
        self.assert_source(0x7F9E3E, "20 16 9F B5 21 29 10 D0 04 5C 4E 9E 7F 20 5F 85 B5 23 29 04 F0 04 5C 66 9E 7F B5 25 29 04 F0 04 5C 66 9E 7F 22 24 2C 7F")

    def test_relative_direction_uses_local_angles_except_self_reference(self):
        self.assert_source(0x7F855F, "B5 25 29 04 F0 04 5C 90 85 7F B5 23 29 04 F0 04 5C 90 85 7F")
        self.assert_source(0x7F8573, "B5 18 85 85 B5 14 8D 12 15 B5 12 8D 11 15 22 1F 2D 7F")
        self.assert_source(0x7F8590, "C2 20 8A DD D8 1C E2 20 F0 D9 B5 18 85 85 BD D6 1C 8D 12 15 BD D5 1C 8D 11 15 22 1F 2D 7F")

    def test_flags_and_relative_velocity_are_resampled_after_callback_and_child_refresh(self):
        self.assert_source(0x7F9E70, "20 A8 9A")
        self.assert_source(0x7F9E87, "22 19 23 7F B5 23 29 04 F0 04 5C 9F 9E 7F B5 25 29 04 D0 04 5C C9 9E 7F")
        self.assert_source(0x7F9E9F, "C2 20 BD CF 1C 18 7D 32 00 9D CF 1C")

    def test_displacement_follow_and_velocity_regeneration_have_different_flags(self):
        from path_semantics import PATH_SEMANTIC_BY_OPCODE
        self.assertEqual(PATH_SEMANTIC_BY_OPCODE[0x001].rust_name, "FollowPlayerDisplacementOn")
        self.assertEqual(PATH_SEMANTIC_BY_OPCODE[0x002].rust_name, "FollowPlayerDisplacementOff")
        self.assertEqual(PATH_SEMANTIC_BY_OPCODE[0x004].rust_name, "GenerateVelocityEachStepOn")
        self.assertEqual(PATH_SEMANTIC_BY_OPCODE[0x005].rust_name, "GenerateVelocityEachStepOff")
        self.assert_source(0x7F84E9, "B5 21 09 08 95 21 4C E8 CA")
        self.assert_source(0x7F8514, "B5 21 09 10 95 21 4C E8 CA")
        self.assert_source(0x7F9F16, "B5 21 29 08 D0 04 5C 4E 9F 7F")


if __name__ == "__main__":
    unittest.main()
