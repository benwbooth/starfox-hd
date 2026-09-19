#!/usr/bin/env python3
"""Literal masks, random-call ordering and operand aliases from source bytes."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathRandomStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_byte_assignment_draws_once_even_if_literal_mask_is_zero(self):
        self.assert_source(0x7F9A3C, "20 E0 C4 8D B5 16 20 BC C4 20 47 CB 22 D0 7B 7F 2D B5 16 99 00 00 4C BE CA")

    def test_word_assignment_draws_high_then_low_before_applying_mask(self):
        self.assert_source(0x7F9A55, "C2 20 20 4C C7 8D B5 16 E2 20 20 BC C4 20 47 CB 22 D0 7B 7F EB 22 D0 7B 7F C2 20 2D B5 16 99 00 00 4C A9 CA")

    def test_centered_byte_subtracts_logical_half_mask_and_adds_destination(self):
        self.assert_source(0x7FC3EB, "20 E0 C4 8D B5 16 20 BC C4 20 47 CB 22 D0 7B 7F 2D B5 16 85 3A AD B5 16 4A 49 FF 1A 18 65 3A 18 79 00 00 99 00 00 4C BE CA")

    def test_centered_word_keeps_two_draw_order_and_word_arithmetic(self):
        self.assert_source(0x7FC414, "C2 20 20 4C C7 8D B5 16 E2 20 20 BC C4 20 47 CB 22 D0 7B 7F EB 22 D0 7B 7F C2 20 2D B5 16 85 3A AD B5 16 4A 49 FF FF 1A 18 65 3A 18 79 00 00 99 00 00 4C A9 CA")

    def test_extended_operand_decoder_and_motion_phase_are_not_carry_yaw(self):
        self.assert_source(0x7FCB47, "08 C2 20 8E B1 16 29 FF 00 89 80 00 F0 04 18 69 41 1C 18 6D B1 16 A8 28 60")
        # Source A1 therefore maps to motion phase 1CE2. Carrier yaw is a
        # DIFFERENT byte at 1CC3 and must not be used for this path counter.
        self.assert_source(0x7FBB13, "E2 20 BD 14 00 9D C3 1C 60")
        self.assert_source(0x06986C, "5A 22 88 21 7F E2 20 EB 49 FF 1A DD E2 1C F0 00 9D E2 1C B5 14 DD E2 1C")


if __name__ == "__main__":
    unittest.main()
