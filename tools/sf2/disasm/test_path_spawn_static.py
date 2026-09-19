#!/usr/bin/env python3
"""Child spawn operand contracts, checked against static source bytes only."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathSpawnStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_compact_zero_rotation_and_extended_literal_rotation_position_number(self):
        self.assert_source(0x7F9042, "AD 11 19 8D BB 16 C2 20 20 20 C7 8D B7 16 AD BB 16 C9 33 00 F0 27 9C BD 16 9C BF 16 9C C1 16 C2 20 20 28 C8 8D B1 16 20 80 C8 8D B3 16 20 D8 C8 8D B5 16 E2 20 20 6C C6 8D B9 16 80 30 E2 20 20 4C C5 8D BD 16 20 70 C5 8D BF 16 20 94 C5 8D C1 16 C2 20 20 AC C8 8D B1 16 20 04 C9 8D B3 16 20 5C C9 8D B5 16 E2 20 20 D8 C6 8D B9 16")

    def test_byte_readers_are_literal_source_offsets_not_encoded_actor_variables(self):
        for address, offset in ((0xC54C, 5), (0xC570, 6), (0xC594, 7),
                                (0xC5B8, 8), (0xC5DC, 9), (0xC66C, 13),
                                (0xC6D8, 16)):
            with self.subTest(address=address):
                self.assert_source(0x7F0000 | address,
                    f"84 79 A0 {offset:02X} 00 A5 5E 29 EF 85 5E 8F 3A 30 00 B7 F9 8D 11 19 A5 5E 09 10 85 5E 8F 3A 30 00 AD 11 19 A4 79 60")

    def test_word_readers_load_exact_offsets_at_word_width(self):
        for address, offset in ((0xC720, 1), (0xC778, 3), (0xC828, 7),
                                (0xC880, 9), (0xC8D8, 11), (0xC8AC, 10),
                                (0xC904, 12), (0xC95C, 14)):
            with self.subTest(address=address):
                self.assert_source(0x7F0000 | address,
                    f"84 79 A0 {offset:02X} 00 E2 20 A5 5E 29 EF 85 5E 8F 3A 30 00 C2 20 B7 F9 8D 11 19 E2 20 A5 5E 09 10 85 5E 8F 3A 30 00 C2 20 AD 11 19 A4 79 60")

    def test_health_attack_offsets_and_inherited_target_group_precede_child_path(self):
        self.assert_source(0x7F9141, "AD BB 16 C9 33 F0 0E 20 4C C5 99 2D 00 20 70 C5 99 2E 00 80 0C 20 B8 C5 99 2D 00 20 DC C5 99 2E 00 B5 24 29 80 D0 04 5C 74 91 7F B9 24 00 09 80 99 24 00 BD F0 1C 99 F0 1C C2 20 20 78 C7 99 2B 00")

    def test_record_advances_are_17_14_and_7_bytes(self):
        for address, count in ((0x7FCA0C, 17), (0x7FCA2E, 14), (0x7FCA72, 7)):
            self.assert_source(address,
                f"E2 20 C2 20 B5 2B 18 69 {count:02X} 00 95 2B E2 20 4C 75 7E")

    def test_fresh_initializer_clears_records_not_pool_links_and_applies_world_defaults(self):
        self.assert_source(0x7F29BC, "DA BB 7A 8B A9 7E 48 AB A9 00 5A DA A0 3B 00 95 04 E8 88 D0 FA FA DA A0 3F 00 9D C1 1C E8 88 D0 F9 FA 7A B5 08 09 10 95 08 B5 09 09 08 95 09 B5 31 09 04 95 31 B5 22 09 04 95 22 C2 20 AD 84 1B 89 02 00 E2 20 D0 03 4C 0C 2A B5 26 09 08 95 26 AD 0E 19 9D F0 1C AB DA BB 7A 6B")

    def test_draw_admission_is_cleared_at_preparation_then_set_separately_from_invisibility(self):
        self.assert_source(0x7F120C, "B9 08 00 29 E1 09 08 99 08 00 B9 23 00 29 02 F0 04 5C 60 14 7F")
        self.assert_source(0x0385F4, "E2 20 B9 23 00 29 02 D0 24 C2 20 BF 12 00 70 10 0B B9 08 00 09 14 00 99 08 00 80 09 B9 08 00 09 10 00 99 08 00 C2 20 8A 18 69 26 00 AA")

    def test_general_search_and_bulk_cleanup_test_the_same_eligibility_flag(self):
        self.assert_source(0x7F1E8B, "B4 00 F0 F3 BB E4 3A F0 F7 B5 22 29 04 00 F0 F0 8E 36 14 A6 3A 6B")
        self.assert_source(0x03A97A, "B4 00 E2 20 B5 22 29 04 F0 06 5A 22 56 33 7F 7A BB D0 ED FA 6B")

    def test_allocation_group_is_compared_by_group_retirement(self):
        self.assert_source(0x0DD8EC, "B4 00 BD F0 1C C5 5F D0 0E A9 00 9D E6 1C 9D E7 1C B5 25 09 08 95 25 BB D0 E6")

    def test_allocation_scopes_head_to_caller_before_choosing_attachment_parent(self):
        self.assert_source(0x7F90AF, "C2 20 AD A8 12 48 E2 20 8E A8 12 DA B5 23 29 04 D0 04 5C C8 90 7F B4 06 BB C2 20 AD B7 16 85 5F E2 20 22 17 2A 7F B0 04 5C 2B 91 7F AD B9 16 22 3D 2A 7F DA BB 7A")

    def test_success_sets_relative_pose_and_publishes_last_child_and_caller_without_world_position(self):
        self.assert_source(0x7F90E5, "C2 20 A9 1E 7E 95 19 E2 20 A9 7F 95 1B B5 31 09 10 95 31 C2 20 AD B1 16 9D CF 1C E2 20 C2 20 AD B3 16 9D D1 1C E2 20 C2 20 AD B5 16 9D D3 1C E2 20 AD BD 16 9D D5 1C AD BF 16 9D D6 1C AD C1 16 9D D7 1C DA BB 7A 8C 71 D7 FA C2 20 8A 99 D8 1C E2 20 C2 20 68 8D A8 12 E2 20")

    def test_failed_allocator_returns_null_destination_without_initializing_an_actor(self):
        self.assert_source(0x7F2A17, "86 3A AE A8 12 22 25 29 7F B0 07 A0 00 00 A6 3A 18 6B 9B A6 3A E2 20 22 BC 29 7F C2 20 A5 5F 99 04 00 E2 20 38 6B")

    def test_first_path_strategy_sets_flags_and_clears_repeat_not_wait_or_stack(self):
        self.assert_source(0x7F7E1E, "C2 20 A9 53 7E 95 19 E2 20 A9 7F 95 1B B5 31 09 10 95 31 B5 20 09 08 95 20 B5 24 09 04 95 24 B5 26 09 01 95 26 B5 26 09 10 95 26 A9 00 95 28 9C 6D B2 9C 6E B2")

    def test_shadow_flag_is_published_to_draw_record_and_consumed_as_ground_shadow(self):
        self.assert_source(0x7F1400, "B9 20 00 9D 07 00")
        # GSU shape preparation loads record byte 7 and tests bit 8 before
        # the shadow-height transform; source_offset also handles this LoROM.
        self.assert_source(0x01D2CF, "A8 07 28 59 3D 48 A1 08 71 09 67 01 A8 12 28 59 11 48 3D F0 EE 03")

    def test_path_flag_excludes_shape_footprint_candidates_and_draw_flag_selects_maximum(self):
        self.assert_source(0x7F1BFE, "E4 3A F0 6E B5 31 29 04 00 D0 67 B5 24 29 04 00 D0 60 B4 04")
        self.assert_source(0x7F1325, "B9 26 00 29 10 F0 04 5C 59 13 7F")
        self.assert_source(0x7F1359, "C2 20 A9 E0 2E 85 3A")


if __name__ == "__main__":
    unittest.main()
