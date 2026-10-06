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

    def test_initializer_registers_same_handler_in_new_continuing_and_separation_order(self):
        for address, kind in [(0x068597, 6), (0x0685AC, 5), (0x0685C1, 7)]:
            self.assert_source(address, f"A9 {kind:02X} 22 60 23 7F C2 20 A9 07 97 99 62 6A E2 20 A9 06 99 64 6A")

    def test_separation_publishes_cursor_before_lookup_and_preserves_damage_parameter(self):
        # Complete wrapper up to callback dispatch. In particular, there are
        # no stores to CF2F/CF30 and no hit-response health subtraction.
        self.assert_source(0x7F3F59, "08 5A 8C 2D CF B9 04 00 A8 E2 20 5A A9 07 22 3B 23 7F C0 00 00 D0 04 5C A3 3F 7F C2 20 B9 62 6A 8D CF 12 E2 20 B9 64 6A 8D D1 12 7A 5A A5 5E 48 A9 7F 48 F4 9B 3F A5 5E 29 EF 85 5E 8F 3A 30 00 DC CF 12")

    def test_deflection_sound_uses_shared_side_queue_before_shared_rng(self):
        self.assert_source(0x06AB05, "5A 08 B4 2B B9 E7 6B D0 1C C2 20 A9 18 00 EC C3 12 F0 03 09 00 80 22 09 6E 7F E2 20 22 D0 7B 7F 29 07 99 E7 6B 28 7A 6B")


if __name__ == "__main__":
    unittest.main()
