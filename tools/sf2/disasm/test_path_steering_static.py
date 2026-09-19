#!/usr/bin/env python3
"""Assembly contracts for native facing commands; no original execution."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathSteeringStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_immediate_selected_and_fixed_slots_have_distinct_target_selection(self):
        self.assert_source(0x7F872C, "AC 1F CF 9C 9D 14 22 A5 21 7F")
        self.assert_source(0x7F8755, "AC 1F CF CC C3 12 D0 05 A0 3F 03 80 03 A0 7E 03 9C 9D 14")

    def test_selected_smoothing_clamps_numerator_then_takes_two_signed_halves(self):
        self.assert_source(0x7F879C, "38 F5 14 C9 00 30 08 C9 04 10 0A A9 04 80 06 C9 FC 30 02 A9 FC C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 14")
        self.assert_source(0x7F87DB, "38 F5 12 C9 00 30 08 C9 04 10 0A A9 04 80 06 C9 FC 30 02 A9 FC C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 12")

    def test_linked_smoothing_uses_three_halves_and_skips_absent_base_link(self):
        self.assert_source(0x7F8A72, "B4 06 D0 04 5C E8 CA 7F 9C 9D 14")
        self.assert_source(0x7F8A8C, "38 F5 12 C9 00 30 08 C9 08 10 0A A9 08 80 06 C9 F8 30 02 A9 F8 C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 12")

    def test_only_selected_variants_refresh_relative_yaw_and_not_pitch(self):
        self.assert_source(0x7F874D, "95 14 20 3F 88 4C E8 CA")
        self.assert_source(0x7F8783, "95 14 20 3F 88 4C E8 CA")
        self.assert_source(0x7F87C2, "95 14 20 3F 88 4C E8 CA")
        self.assert_source(0x7F8837, "95 14 20 3F 88 4C E8 CA")
        self.assert_source(0x7F883F, "B5 25 29 04 D0 04 5C 55 88 7F BC D8 1C B5 14 38 F9 14 00 9D D6 1C 60")

    def test_linked_alignment_counts_equality_before_assignment(self):
        self.assert_source(0x7F8B29, "EB D5 12 D0 04 5C 38 8B 7F 95 12 5C 3B 8B 7F EE 9D 14")
        self.assert_source(0x7F8B45, "D5 14 D0 04 5C 53 8B 7F 95 14 5C 56 8B 7F EE 9D 14")


if __name__ == "__main__":
    unittest.main()
