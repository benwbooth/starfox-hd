#!/usr/bin/env python3
"""Typed actor arithmetic contracts verified against source bytes only."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathFieldsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_byte_angle_adds_write_only_the_angle_and_advance(self):
        self.assert_source(0x7F8627, "20 BC C4 20 47 CB 20 E0 C4 18 79 00 00 99 00 00 4C BE CA")
        self.assert_source(0x7F863A, "20 BC C4 18 75 12 95 12 4C D3 CA")
        self.assert_source(0x7F8645, "20 BC C4 18 75 14 95 14 4C D3 CA")
        self.assert_source(0x7F8650, "20 BC C4 18 75 16 95 16 4C D3 CA")

    def test_world_position_add_sign_extends_literal_byte_before_word_add(self):
        self.assert_source(0x7F865B, "20 BC C4 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 18 75 0C 95 0C 4C D3 CA")
        self.assert_source(0x7F8687, "18 75 0E 95 0E 4C D3 CA")
        self.assert_source(0x7F86A1, "18 75 10 95 10 4C D3 CA")

    def test_variable_byte_to_word_copy_and_add_both_sign_extend(self):
        self.assert_source(0x7F8929, "BD 00 00 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 18 79 00 00 99 00 00")
        self.assert_source(0x7F897A, "E2 20 BD 00 00 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 99 00 00")

    def test_word_to_byte_copy_retains_low_byte_and_other_byte_is_untouched(self):
        self.assert_source(0x7F8968, "C2 20 BD 00 00 E2 20 99 00 00 FA 4C BE CA")

    def test_variable_helper_uses_first_operand_as_destination_and_second_as_source(self):
        self.assert_source(0x7F8995, "08 E2 20 20 E0 C4 20 47 CB 5A 20 BC C4 20 47 CB FA 28 60")
        self.assert_source(0x7FCB47, "08 C2 20 8E B1 16 29 FF 00 89 80 00 F0 04 18 69 41 1C 18 6D B1 16 A8 28 60")

    def test_variable_copies_and_adds_keep_widths_and_immediate_continuation(self):
        self.assert_source(0x7F88FF, "DA 20 95 89 BD 00 00 18 79 00 00 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8911, "DA 20 95 89 C2 20 BD 00 00 18 79 00 00 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8925, "DA 20 95 89 BD 00 00 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 18 79 00 00 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8946, "DA 20 95 89 BD 00 00 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8954, "DA 20 95 89 C2 20 BD 00 00 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8964, "DA 20 95 89 C2 20 BD 00 00 E2 20 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8976, "DA 20 95 89 E2 20 BD 00 00 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 99 00 00 FA 4C BE CA")

    def test_literal_writes_have_no_motion_or_collision_side_effect(self):
        self.assert_source(0x7F89A8, "20 E0 C4 20 47 CB E2 20 20 BC C4 99 00 00 4C BE CA")
        self.assert_source(0x7F89B9, "20 04 C5 20 47 CB C2 20 20 20 C7 99 00 00 E2 20 4C A9 CA")
        self.assert_source(0x7FC4BC, "84 79 A0 01 00")
        self.assert_source(0x7FC4E0, "84 79 A0 02 00")
        self.assert_source(0x7FC504, "84 79 A0 03 00")

    def test_zero_assignment_width_and_word_add_operand_order(self):
        self.assert_source(0x7F89CC, "20 BC C4 20 47 CB E2 20 A9 00 99 00 00 4C D3 CA")
        self.assert_source(0x7F89DC, "20 BC C4 20 47 CB C2 20 A9 00 00 99 00 00 E2 20 4C D3 CA")
        self.assert_source(0x7F86CD, "20 BC C4 20 47 CB C2 20 20 4C C7 18 79 00 00 99 00 00 E2 20 4C A9 CA")

    def test_increment_and_decrement_wrap_at_destination_width(self):
        self.assert_source(0x7F86EA, "B9 00 00 1A 99 00 00 4C D3 CA")
        self.assert_source(0x7F86FA, "C2 20 B9 00 00 1A 99 00 00 E2 20 4C D3 CA")
        self.assert_source(0x7F870E, "B9 00 00 3A 99 00 00 4C D3 CA")

    def test_negation_uses_complement_plus_one_without_saturation(self):
        self.assert_source(0x7F9A1B, "B9 00 00 49 FF 1A 99 00 00 4C D3 CA")
        self.assert_source(0x7F9A2D, "C2 20 B9 00 00 49 FF FF 1A 99 00 00 4C D3 CA")


if __name__ == "__main__":
    unittest.main()
