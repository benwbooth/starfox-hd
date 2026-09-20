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

    def test_inline_primary_motion_inherits_velocity_only_for_mode_class_one(self):
        self.assert_source(0x09E78A, "89 22 85 A8 06 C2 20 A9 95 E7 6B 42")
        # Primary selection (not CF1F) -> auxiliary 6AA0 high nibble. Only
        # class 10 adds base velocity; all others add extension displacement.
        # Both arms write X/Z velocity only and preserve arithmetic wrapping.
        self.assert_source(0x06A885,
            "5A 08 AC C3 12 80 04 5A 08 B4 06 DA BB 7A 5A B4 2B B9 A0 6A 7A DA BB 7A "
            "29 F0 C9 10 F0 04 5C BB A8 06 C2 20 B9 32 00 18 75 32 95 32 "
            "B9 36 00 18 75 36 95 36 80 12 C2 20 B9 C1 1C 18 75 32 95 32 "
            "B9 C5 1C 18 75 36 95 36 E2 20 28 7A 6B")

    def test_published_motion_snapshot_selects_velocity_or_retained_displacement(self):
        self.assert_source(0x069CE7, "22 15 EA 07")
        self.assert_source(0x07EA15,
            "08 E2 20 C2 10 C2 20 B5 0C 8D EC D7 B5 0E 8D EE D7 B5 10 8D F0 D7 "
            "E2 20 5A B4 2B B9 A0 6A 7A 29 F0 C9 10 F0 04 5C 51 EA 07 "
            "C2 20 B5 32 8D 1C 1E B5 34 8D 1E 1E B5 36 8D 20 1E 80 14 "
            "C2 20 BD C1 1C 8D 1C 1E BD C3 1C 8D 1E 1E BD C5 1C 8D 20 1E 28 6B")

    def test_word_import_reads_snapshot_and_counter_motion_root_repeats_after_setup(self):
        self.assert_source(0x7F89A8, "20 E0 C4 20 47 CB E2 20 20 BC C4 99 00 00 4C BE CA")
        self.assert_source(0x7F9F67, "20 D2 9F C2 20 AD B7 16 99 00 00 4C A9 CA")
        self.assert_source(0x7F9FD2,
            "C2 20 20 4C C7 A8 B9 00 00 8D B7 16 E2 20 20 BC C4 20 47 CB 60")
        # Path bank 44 is a decoded data window, not a normal CPU ROM bank.
        expected = bytes.fromhex("5c 00 29 7c 32 1c 1e 7c 36 20 1e 57 32 57 36 00 44 0b 40 12 0b 80 16 16 68 be")
        self.assertEqual(self.rom[0x4BE65:0x4BE65 + len(expected)], expected)

    def test_byte_motion_import_uses_same_snapshot_but_truncates_before_authored_arithmetic(self):
        self.assert_source(0x7F9F4F, "20 D2 9F AD B7 16 99 00 00 4C A9 CA")
        # The shared loader reads a word, then returns in byte mode after
        # resolving the actor destination. The import stores only its low
        # byte, even when the address is an odd byte inside a motion word.
        self.assert_source(0x7F9FD2,
            "C2 20 20 4C C7 A8 B9 00 00 8D B7 16 E2 20 20 BC C4 20 47 CB 60")
        self.assert_source(0x098463,
            "93 A1 79 A1 1C 1E 52 A1 A1 56 A1 55 0C A1 79 A1 20 1E "
            "52 A1 A1 56 A1 55 10 A1 95 A1 42")
        self.assert_source(0x08CE8B, "79 A1 20 1E EE A2 A1 9A 4E")

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

    def test_motion_mode_toggles_are_immediate_and_quadrupling_is_a_separate_flag(self):
        self.assert_source(0x7F84D7, "B5 21 29 BF 95 21 4C E8 CA")
        self.assert_source(0x7F84E0, "B5 21 09 40 95 21 4C E8 CA")
        self.assert_source(0x7F84F2, "B5 21 29 F7 95 21 4C E8 CA")
        self.assert_source(0x7F851D, "B5 21 29 EF 95 21 4C E8 CA")
        self.assert_source(0x7F8526, "B5 26 09 80 95 26 4C E8 CA")
        self.assert_source(0x7F85AE, "B5 26 29 80 F0 04 5C FC 85 7F 60")
        self.assert_source(0x7F85FC, "C2 20 16 32 16 32 16 34 16 34 16 36 16 36 E2 20 60")

    def test_child_gate_precedes_relative_integration_and_walks_first_child_only(self):
        self.assert_source(0x7F9E73, "B5 22 29 01 F0 04 5C 8B 9E 7F B5 23 29 10 D0 04 5C 8B 9E 7F 22 19 23 7F")
        self.assert_source(0x7F2322, "B4 29 F0 07 BB 22 29 22 7F 80 F5")

    def test_extension_parent_has_priority_and_only_its_self_reference_skips(self):
        self.assert_source(0x7F222A, "B4 06 C2 20 BD D8 1C F0 0A 86 02 C5 02 D0 03 82 D9 00 A8")
        self.assert_source(0x7F225A, "22 BE 2B 7F BD D5 1C 18 79 12 00 95 12 BD D6 1C 18 79 14 00 95 14 BD D7 1C 18 79 16 00 95 16")
        self.assert_source(0x7F2279, "B9 12 00 8D 8F 15 B9 14 00 8D 91 15 B9 16 00 8D 93 15")

    def test_carry_reselects_player_and_clears_only_low_continuity_byte(self):
        self.assert_source(0x7F9ED3, "AC 1F CF B9 24 00 29 02 D0 04 5C EB 9E 7F C2 20 8A D9 E8 1C E2 20 F0 07 A9 00 9D C1 1C 80 0B BD C1 1C F0 03 20 1C BB 20 F7 BA")

    def test_carry_snapshot_word_and_byte_writes_are_distinct(self):
        self.assert_source(0x7FBB01, "B5 0C 95 39 B5 0E 95 3B B5 10 95 3D A9 01 00 9D C1 1C E2 20 BD 14 00 9D C3 1C 60")

    def test_carry_scales_horizontal_before_rotation_and_uses_unrotated_vertical(self):
        self.assert_source(0x7FBB2D, "B9 ED 6B 38 F5 39 0A 0A 0A 0A 85 02 85 B7 B9 EF 6B 38 F5 3B 85 08 85 B9 B9 F1 6B 38 F5 3D 0A 0A 0A 0A 85 97 85 BB")
        self.assert_source(0x7FBC0F, "A5 08 18 75 0E 99 EF 6B")

    def test_common_exit_clears_contact_latches_without_clearing_pending_damage(self):
        self.assert_source(0x7F9EFD, "B5 22 29 FD 95 22 B5 21 29 7F 95 21 B5 26 29 FD 95 26 B5 26 29 FB 95 26 6B")


if __name__ == "__main__":
    unittest.main()
