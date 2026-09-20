#!/usr/bin/env python3
"""Static source contracts for decoded native path control statements."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathCommandsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_wait_compares_before_increment_and_clears_only_on_success(self):
        self.assert_source(0x7F84FB, "20 BC C4 D5 17 D0 04 5C CF CA 7F F6 17 4C DE 9D")
        self.assert_source(0x7FCACF, "A9 00 95 17")

    def test_contact_class_masks_modify_one_class_byte_with_immediate_continuation(self):
        self.assert_source(0x7FC3AF, "20 BC C4 8D B1 16 B5 31 2D B1 16 95 31 4C D3 CA")
        self.assert_source(0x7FC3BF, "20 BC C4 8D B1 16 B5 31 0D B1 16 95 31 4C D3 CA")

    def test_contact_suppression_and_marker_are_distinct_flags(self):
        self.assert_source(0x7F9022, "B5 22 09 08 95 22 4C E8 CA")
        self.assert_source(0x7F902B, "B5 22 29 F7 95 22 4C E8 CA")
        self.assert_source(0x7F8538, "B5 24 09 08 95 24 4C E8 CA")

    def test_wait_one_advances_without_reset_and_goto_sets_target_before_movement(self):
        self.assert_source(0x7F850B, "C2 20 F6 2B E2 20 4C DE 9D")
        self.assert_source(0x7F8C8A, "C2 20 20 20 C7 95 2B E2 20 4C DE 9D")

    def test_jump_enters_dispatch_without_refreshing_player_selection(self):
        self.assert_source(0x7FCAF3, "C2 20 20 20 C7 95 2B E2 20 4C 75 7E")

    def test_immediate_loop_repeat_refreshes_selection_but_normal_repeat_moves(self):
        self.assert_source(0x7F971E, "B9 61 6A 3A F0 B7 99 61 6A 88 88 88 88 B9 61 6A 95 2B E2 20 4C 53 7E")
        self.assert_source(0x7F96C4, "B9 61 6A 3A F0 11 99 61 6A 88 88 88 88 B9 61 6A 95 2B E2 20 4C DE 9D")

    def test_break_and_pair_discard_pop_twice_without_inspecting_values(self):
        pop = "C2 20 BD DE 1C 22 C1 1A 7F 9D DE 1C AD 69 B2 8D B1 16 E2 20"
        self.assert_source(0x7F9735, f"{pop} {pop} 4C F3 CA")
        self.assert_source(0x7F9760, f"{pop} {pop} 4C E8 CA")

    def test_contact_parameter_is_the_same_byte_used_as_path_speed_target(self):
        self.assert_source(0x03A41E, "B9 0A 00 8D 30 CF")
        self.assert_source(0x7F9DF0, "B5 18 D5 0A 30 0B 38 F5 0B D5 0A 10 13 B5 0A")

    def test_byte_loop_counter_is_distinct_from_wait_and_word_loop_stack(self):
        self.assert_source(0x7F860D, "20 BC C4 D5 15 F0 0E F6 15 C2 20 20 4C C7 95 2B E2 20 4C DE 9D 74 15 4C A9 CA")

    def test_hit_mask_is_last_operand_and_only_success_consumes_matching_bits(self):
        self.assert_source(0x7F9A79, "20 04 C5 8D B1 16 B5 38 2D B1 16 D0 04 5C A9 CA 7F AD B1 16 49 FF 35 38 95 38 4C F3 CA")
        self.assert_source(0x7F9507, "B5 23 29 08 D0 04 5C BE CA 7F B5 23 29 F7 95 23 4C F3 CA")


if __name__ == "__main__":
    unittest.main()
