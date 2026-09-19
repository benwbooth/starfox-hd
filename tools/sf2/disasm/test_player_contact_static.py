#!/usr/bin/env python3
"""Static correspondence for the player callback; no gameplay recordings."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerContactStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_initial_gates_precede_contact_lookup_and_shared_gates_follow_it(self):
        self.assert_source(0x069710, "B9 E3 6B F0 04 5C 9C 97 06 B9 77 6B 89 01 D0 04 5C 9C 97 06")
        self.assert_source(0x06972A, "5A B4 2B B9 72 6A 7A 29 10 F0 04 5C 9C 97 06")
        self.assert_source(0x069739, "AC 2D CF C2 20 B9 04 00 A8 E2 20 AD F4 D7 D0 04 5C 9C 97 06 AD 72 1D F0 04 5C 9C 97 06")

    def test_timed_and_projectile_deflection_take_distinct_feedback_paths(self):
        self.assert_source(0x069756, "5A B4 2B B9 02 6C 7A 29 1F F0 04 5C 83 97 06")
        self.assert_source(0x069765, "5A B4 2B B9 02 6C 7A 29 40 D0 04 5C AA 97 06 B9 31 00 29 08 C9 08 F0 04 5C AA 97 06 80 11")
        self.assert_source(0x069794, "22 05 AB 06 22 AE F1 07 A9 00 8D 2F CF B5 20 29 FD 95 20 4C 24 98")

    def test_part_feedback_mapping_then_unconditional_hit_flag_consumption(self):
        self.assert_source(0x0697AA, "B9 25 00 29 80 D0 04 5C E3 97 06")
        self.assert_source(0x0697B8, "B5 38 89 02 F0 08 B9 E4 6B 09 80 99 E4 6B")
        self.assert_source(0x0697C6, "B5 38 89 04 F0 08 B9 E4 6B 09 40 99 E4 6B")
        self.assert_source(0x0697D4, "B5 38 89 01 F0 08 B9 E4 6B 09 20 99 E4 6B")
        self.assert_source(0x0697E3, "B5 38 29 F8 95 38 22 42 98 06 22 AE AA 06")

    def test_impact_sound_is_selected_and_dispatched_before_reserve_tail(self):
        self.assert_source(0x0697F1, "AD 2F CF C9 04 10 04 5C 11 98 06 C2 20 A9 12 00 EC C3 12 F0 03 09 00 80 22 09 6E 7F E2 20 80 13")
        self.assert_source(0x069811, "C2 20 A9 13 00 EC C3 12 F0 03 09 00 80 22 09 6E 7F E2 20")
        self.assert_source(0x069824, "B4 2B B9 00 6C F0 13 38 ED 2F CF 99 00 6C 10 05 A9 00 99 00 6C A9 00 8D 2F CF")

    def test_three_independent_turn_suppression_gates(self):
        self.assert_source(0x069847, "AD E2 1D C9 09 F0 51")
        self.assert_source(0x06984E, "5A B4 2B B9 A0 6A 7A 29 F0 C9 20 D0 04 5C 9F 98 06")
        self.assert_source(0x06985F, "B9 31 00 29 08 C9 08 D0 04 5C 9F 98 06")


if __name__ == "__main__":
    unittest.main()
