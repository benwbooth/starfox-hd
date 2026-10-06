#!/usr/bin/env python3
"""Source proof for weapon pitch publication and the full ordered target scan."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerWeaponAimStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_zero_publication_precedes_mode_read_and_non_walker_copies_pitch(self):
        self.assert_source(0x07D6CC, "DA 5A 08 E2 20 C2 10 9C F2 1D 5A B4 2B B9 A0 6A 7A 29 F0 C9 20 D0 04 5C EF D6 07 B5 12 8D F2 1D 4C 8B D7")

    def test_walker_window_and_no_candidate_skip_proxy_and_steering_reset(self):
        self.assert_source(0x07D6EF, "DA A0 00 00 84 3C A9 32 85 E4 A9 0A 85 97 C2 20 A9 00 00 85 3E A9 58 1B 85 3A 22 EB 1F 7F C0 00 00 E2 20 D0 04 5C 48 D7 07")
        self.assert_source(0x07D748, "FA A4 3C F0 29 AC D6 14 9C 9D 14 22 A5 21 7F")

    def test_prediction_is_overwritten_by_the_complete_position_copy_helper(self):
        self.assert_source(0x07D718, "84 02 AE D6 14 C2 20 B9 32 00 0A 0A 18 79 0C 00 95 0C B9 34 00 0A 0A 18 79 0E 00 95 0E B9 36 00 0A 0A 18 79 10 00 95 10 E2 20 22 BE 2B 7F 84 3C")
        self.assert_source(0x7F2BBE, "C2 20 B9 0C 00 95 0C B9 0E 00 95 0E B9 10 00 95 10 E2 20 6B")

    def test_two_toward_zero_halves_high_byte_and_signed_clamp(self):
        self.assert_source(0x07D757, "C2 20 C9 00 80 6A 10 03 69 00 00 85 3A C9 00 80 6A 10 03 69 00 00 18 65 3A E2 20 EB 8D F2 1D AD F2 1D 18 69 19 30 08 C9 32 30 09 A9 19 80 02 A9 E7 8D F2 1D")
        self.assert_source(0x7F21A5, "DA 5A 22 9F 24 7F C2 20 AD DE 12 85 08 B9 0E 00 38 F5 0E 85 02 22 58 1D 7F C2 30 7A FA 6B")

    def test_search_starts_at_active_head_and_uses_strict_wrapping_signed_range(self):
        self.assert_source(0x7F1FEB, "64 3C 86 04 AE A8 12 E4 04 D0 04 5C ED 20 7F B5 22 29 04 00 D0 04 5C ED 20 7F A4 04 22 9F 24 7F C2 20 AD DE 12 C5 3A 30 04 5C ED 20 7F C5 3E 10 04 5C ED 20 7F")

    def test_search_angles_have_opposite_vertical_difference_and_signed_half(self):
        self.assert_source(0x7F2020, "C2 20 B5 0C 38 F9 0C 00 85 02 B5 10 38 F9 10 00 85 08 E2 20 22 58 1D 7F E2 20 EB 85 02 A5 97 18 79 14 00 18 65 02 C9 80 6A C5 97 90 04 5C EB 20 7F")
        self.assert_source(0x7F2051, "DA 5A 22 9F 24 7F C2 20 AD DE 12 85 08 B9 0E 00 38 F5 0E 85 02 22 58 1D 7F C2 30 7A FA E2 20 EB 85 02 A5 E4 18 79 12 00 18 65 02 C9 80 6A C5 E4 90 04 5C EB 20 7F")

    def test_class_filters_are_not_generic_collision_exclusions(self):
        self.assert_source(0x7F2087, "B9 31 00 29 20 C9 20 F0 04 5C A0 20 7F B5 31 29 20 C9 20 D0 04 5C EB 20 7F B5 31 29 02 F0 04 5C EB 20 7F B5 31 29 10 C9 10 F0 04 5C EB 20 7F B5 31 29 80 C9 80 D0 04 5C EB 20 7F B5 31 29 08 C9 08 D0 04 5C EB 20 7F")

    def test_final_flags_acceptance_and_list_iteration_have_no_health_or_death_gate(self):
        self.assert_source(0x7F20CE, "B5 21 29 01 F0 04 5C EB 20 7F B5 22 29 08 F0 04 5C EB 20 7F C2 20 AD DE 12 85 3A 86 3C C2 20 B4 00 BB F0 04 5C F2 1F 7F 8C 36 14 A4 3C A6 04 6B")


if __name__ == "__main__":
    unittest.main()
