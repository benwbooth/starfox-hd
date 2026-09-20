#!/usr/bin/env python3
"""Complete proximity warning source bodies; no original-program execution."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class ProximityWarningStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_candidate_flag_controls_only_membership_and_advances_escaped_opcode(self):
        self.assert_source(0x7FC372, "b5 26 09 20 95 26 4c e8 ca b5 26 29 df 95 26 4c e8 ca")

    def test_scan_entry_guards_use_full_movement_class_and_transition_pair(self):
        self.assert_source(0x06A647,
            "5a 08 e2 20 c2 10 c2 20 ad 84 1b 89 02 00 e2 20 f0 03 4c 34 a8 b4 2b b9 a0 6a 29 f0 c9 10 f0 04 "
            "5c 34 a8 06 b9 77 6b 89 20 f0 04 5c 34 a8 06 b9 ff 6b 29 fe c9 02 d0 0b b9 77 6b 89 40 d0 04 5c 34 a8 06")

    def test_complete_live_scan_strict_bounds_signed_offset_score_and_rearming(self):
        self.assert_source(0x06A68A,
            "da c2 20 a0 ff 7f 8c b6 1d 8c b4 1d a0 00 00 8c b2 1d 8c b0 1d ac a8 12 8e b8 1d bb e2 20 b5 26 "
            "29 20 d0 04 5c 56 a7 06 c2 20 ac b8 1d b9 0e 00 38 f5 0e 10 04 49 ff ff 1a c9 2c 01 90 04 5c 4e "
            "a7 06 22 9f 24 7f c2 20 ad de 12 c9 f4 01 90 04 5c 4e a7 06 b5 0c 85 3a b5 10 85 3e 22 6d a5 06 "
            "a5 e4 30 60 c9 96 00 10 5b e2 20 b5 22 29 10 f0 04 5c 56 a7 06 c2 20 a5 e4 18 65 04 cd b4 1d b0 "
            "43 cd b6 1d b0 2c 48 ad b2 1d 8d b0 1d ad b6 1d 8d b4 1d ad c0 1d 8d bc 1d ad be 1d 8d ba 1d 68 "
            "8e b2 1d 8d b6 1d a5 04 8d c0 1d a5 e4 8d be 1d 80 1a 8e b0 1d 8d b4 1d a5 04 8d bc 1d a5 e4 8d "
            "ba 1d 80 08 e2 20 b5 22 29 ef 95 22 c2 20 b4 00 f0 04 5c a5 a6 06")

    def test_two_candidate_side_coupling_shape_override_latching_and_audio_routing(self):
        self.assert_source(0x06A760,
            "a2 00 00 fa 64 02 ac b2 1d d0 04 5c e0 a7 06 ad c0 1d 10 04 49 ff ff 1a c9 32 00 90 14 c9 82 00 "
            "b0 23 ad c0 1d 30 05 a9 01 00 80 08 a9 04 00 80 03 a9 02 00 85 02 20 37 a8 e2 20 b9 22 00 09 10 "
            "99 22 00 c2 20 ac b0 1d f0 36 ad bc 1d 10 04 49 ff ff 1a c9 32 00 90 14 c9 82 00 b0 23 ad c0 1d "
            "30 05 a9 01 00 80 08 a9 04 00 80 03 a9 02 00 05 02 85 02 20 37 a8 e2 20 b9 22 00 09 10 99 22 00 "
            "e2 20 a5 02 f0 4e 89 02 d0 34 89 04 d0 18 c2 20 a9 ab 00 ec c3 12 f0 03 09 00 80 09 00 20 22 09 "
            "6e 7f e2 20 80 2e c2 20 a9 ab 00 ec c3 12 f0 03 09 00 80 09 00 10 22 09 6e 7f e2 20 80 16 c2 20 "
            "a9 ab 00 ec c3 12 f0 03 09 00 80 09 00 00 22 09 6e 7f e2 20 28 7a 6b da 5a bb b4 04 bb bf 10 00 "
            "00 89 00 ff f0 05 a9 02 00 85 02 7a fa 60")

    def test_projection_uses_fixed_view_marker_and_separately_truncated_products(self):
        self.assert_source(0x06A56D,
            "da 5a 08 e2 20 c2 10 a2 3f 03 c2 20 a5 3a 38 f5 0c 85 02 85 04 a5 3e 38 f5 10 85 97 85 e4 e2 20 "
            "b5 15 22 8a 37 7f 28 7a fa 6b")
        self.assert_source(0x7F378A,
            "86 3a 84 3c 08 8b e2 10 aa a9 00 48 ab bd 26 8e 8d 2d 15 bd 66 8e 8d 2e 15 a5 02 85 6a a5 03 85 "
            "6b ad 2d 15 85 6c 22 e1 89 03 a5 6d 85 e4 a5 6e 85 e5 a5 97 85 6a a5 98 85 6b ad 2e 15 85 6c 22 "
            "e1 89 03 a5 6d 18 65 e4 85 e4 a5 6e 65 e5 85 e5 a5 97 85 6a a5 98 85 6b ad 2d 15 85 6c 22 e1 89 "
            "03 a5 6d 85 04 a5 6e 85 05 a5 02 85 6a a5 03 85 6b ad 2e 15 85 6c 22 e1 89 03 a5 6d 38 e5 04 85 "
            "04 a5 6e e5 05 85 05 ab 28 a4 3c a6 3a 6b")
        self.assert_source(0x0389E1,
            "20 e5 89 6b 64 6f 64 70 64 6e a5 6c 30 23 0a 8d 02 42 a5 6b 30 0c a5 6a 8d 03 42 ea ea a5 6b 4c "
            "4b 8a a9 00 38 e5 6a 8d 03 42 a9 00 e5 6b 4c 27 8a 49 ff 1a 0a 8d 02 42 a5 6b 30 20 a5 6a 8d 03 "
            "42 ea ea ea a5 6b ac 17 42 8d 03 42 84 6d c2 20 a5 6f 38 ed 16 42 38 e5 6d 4c 5d 8a a9 00 38 e5 "
            "6a 8d 03 42 ea ea a9 00 e5 6b ac 17 42 8d 03 42 84 6d c2 21 a5 6f 6d 16 42 18 65 6d 85 6d e2 20 60")


if __name__ == "__main__":
    unittest.main()
