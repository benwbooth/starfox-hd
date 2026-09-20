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

    def test_complete_parent_children_and_callback_graph(self):
        self.assert_source(0x09F3AD,
            "00 04 37 f5 dc c8 d4 f3 01 01 05 00 9c ff 38 ff 29 "
            "f5 f8 c8 de f3 01 01 fb ff 9c ff 38 ff 2a 61 2d 6b 16 6b 12 44 0f "
            "0b 03 0a 0c 28 00 a3 17 e5 f3 0b 08 0a 0c d8 ff a3 "
            "00 65 7a f4 00 23 f7 ef 5c 61 05 89 22 ae fa 06 c2 20 a9 fb f3 6b 44 "
            "00 65 6e f4 00 19 03 05 fb 1b 1e 28 03 0a fb 1b 1e 28 03 0a fb 1b 1e 28 "
            "61 0a 0c 03 00 87 00 2d 01 23 f4 0c 01 00 87 44 0c 03 00 87 62 0a 44 "
            "79 a2 4d 1b 8a 2a a2 02 53 f4 7c a3 0f 1e ef a3 0e 53 f4 "
            "4f a1 8e b5 6c 8e 6c 90 6c 92 61 0a 85 16 a1 75 fc 44 17 6b f4 "
            "58 a1 1f 07 a1 f0 61 0a 89 22 48 fb 06 c2 20 a9 66 f4 6b "
            "00 53 6b f4 44 6b 2d 19 89 22 fb fa 06 c2 20 a9 79 f4 6b 42 "
            "93 a1 79 a1 72 1d 67 a1 87 f4 4c 8a f4 95 a1 42 0f")

    def test_parent_installer_sets_the_self_relative_link(self):
        self.assert_source(0x07D18B,
            "c2 20 a9 ad f3 99 2b 00 e2 20 20 f5 be c2 20 98 99 d8 1c e2 20 28 7a 38 6b")

    def test_shared_recovery_consumer_clears_then_wraps_clamps_and_requests_feedback(self):
        self.assert_source(0x069F36,
            "ad 1b 1e 9c 1b 1e 8d ae 1d f0 13 18 79 00 6c cd d5 1d "
            "90 03 ad d5 1d 99 00 6c 22 c6 d0 07")
        self.assert_source(0x069824,
            "b4 2b b9 00 6c f0 13 38 ed 2f cf 99 00 6c 10 05 a9 00 99 00 6c a9 00 8d 2f cf")

    def test_environment_plane_reaches_render_height_and_player_vertical_limit(self):
        self.assert_source(0x07C3EC, "c2 20 a9 24 fa 8d 11 1e ad 0f 1e 8f b9 18 00")
        self.assert_source(0x06D7E2,
            "c2 20 ad 0f 1e 18 69 c8 00 d5 0e e2 20 10 11 "
            "c2 20 95 0e 99 ef 6b a9 00 00 95 34 9d c3 1c e2 20")

    def test_clock_bits_branch_is_direct_and_does_not_consume_ifnot(self):
        self.assert_source(0x7FBD06, "20 bc c4 25 c4 d0 03 4c ff ca 4c a9 ca")

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
