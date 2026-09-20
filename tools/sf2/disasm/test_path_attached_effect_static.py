#!/usr/bin/env python3
"""Static evidence for attached-effect motion and the depth word views."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class AttachedEffectStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_settle_chases_forward_twice_then_vertical_twice(self):
        self.assert_source(0x06FAAE,
            "c2 20 bd d3 1c 85 3a a9 c8 00 22 a3 25 7f 9d d3 1c e2 20 "
            "c2 20 bd d3 1c 85 3a a9 c8 00 22 a3 25 7f 9d d3 1c e2 20 "
            "c2 20 bd d1 1c 85 3a a9 00 00 22 a3 25 7f 9d d1 1c e2 20 "
            "c2 20 bd d1 1c 85 3a a9 00 00 22 a3 25 7f 9d d1 1c e2 20 6b")

    def test_center_uses_live_script_word_and_asymmetric_axis_counts(self):
        self.assert_source(0x06FAFB,
            "c2 20 bd cf 1c 85 3a bd e4 1c 22 a3 25 7f 9d cf 1c e2 20 "
            "c2 20 bd d3 1c 85 3a a9 00 00 22 a3 25 7f 9d d3 1c e2 20 "
            "c2 20 bd d3 1c 85 3a a9 00 00 22 a3 25 7f 9d d3 1c e2 20 "
            "c2 20 bd d1 1c 85 3a a9 00 00 22 a3 25 7f 9d d1 1c e2 20 6b")

    def test_tumble_changes_relative_pose_with_byte_and_word_arithmetic(self):
        self.assert_source(0x06FB48,
            "bd d5 1c 18 69 f8 9d d5 1c "
            "c2 20 bd d1 1c 18 69 0a 00 9d d1 1c e2 20 "
            "c2 20 bd d3 1c 18 69 f6 ff 9d d3 1c e2 20 "
            "bd d7 1c 18 7d e2 1c 9d d7 1c "
            "bd d6 1c 18 7d e2 1c 9d d6 1c 6b")

    def test_chase_uses_wrapped_signed_difference_and_three_toward_zero_halves(self):
        self.assert_source(0x7F25A3,
            "c5 3a d0 04 5c 21 29 7f 38 e5 3a c9 00 00 30 0a "
            "c9 08 00 10 0d a9 08 00 80 08 c9 f8 ff 30 03 a9 f8 ff "
            "c9 00 80 6a 10 03 69 00 00 c9 00 80 6a 10 03 69 00 00 "
            "c9 00 80 6a 10 03 69 00 00 18 65 3a 85 3a 82 35 03")
        self.assert_source(0x7F291D, "a5 3a 18 6b a5 3a 38 6b")

    def test_depth_operands_and_byte_only_render_and_sprite_consumers(self):
        self.assert_source(0x7FCB47,
            "08 c2 20 8e b1 16 29 ff 00 89 80 00 f0 04 18 69 41 1c "
            "18 6d b1 16 a8 28 60")
        self.assert_source(0x09F416, "0c 03 00 87")
        self.assert_source(0x7F1424, "b9 c8 1c 9d 1b 00 b9 da 1c 9d 1c 00 b9 db 1c 9d 1d 00 c2 20")
        self.assert_source(0x7F99D5,
            "20 bc c4 8d b1 16 20 e0 c4 8d b3 16 b5 20 09 20 95 20 "
            "ad b1 16 9d c8 1c ad b3 16 9d da 1c 4c be ca")


if __name__ == "__main__":
    unittest.main()
