#!/usr/bin/env python3
"""SF2's inherited TRAIL opcode and its radar consumers, checked statically."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class RadarStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_trail_assigns_radar_channel_not_texture_or_particle_state(self):
        self.assert_source(0x7FC3A6, "20 bc c4 9d ee 1c 4c d3 ca")

    def test_marker_appearance_height_cues_and_special_visible_count(self):
        self.assert_source(0x7F51D5,
            "9c c8 1b 9c ca 1b ad 2e e8 c9 01 00 10 03 ee ca 1b")
        self.assert_source(0x7F51EE,
            "bd ee 1c 29 ff 00 d0 09 bd 00 00 aa d0 f2 4c 66 52 85 5f bd 0c 00 38 ed "
            "84 d9 85 02 ad 88 d9 38 fd 10 00 85 97 64 50 a5 5f c9 1e 00 f0 05 c9 86 "
            "00 d0 02 e6 50 89 80 00 f0 2f 29 7f 00 eb 09 2e 00 85 5f ad ca 1b d0 04 "
            "c6 5f 80 1d bd 0e 00 38 ed 86 d9 85 08 c9 00 fe 30 07 c9 00 02 10 06 80 "
            "08 e6 5f 80 04 c6 5f 80 00 20 a3 52 b0 07 a5 50 f0 03 ee c8 1b 82 90 ff")

    def test_projection_scaling_clipping_and_screen_coordinate_publication(self):
        self.assert_source(0x7F52A3,
            "da a5 02 85 04 a5 97 85 e4 ad 2e e8 f0 27 89 00 80 d0 16 aa a5 04 c9 00 "
            "80 6a 85 04 a5 e4 c9 00 80 6a 85 e4 ca d0 ed 80 0c 49 ff ff 1a aa 06 04 "
            "06 e4 ca d0 f9 a5 02 c9 00 80 6a 18 6d 32 e8 30 43 cd 30 e8 10 3e a5 97 "
            "c9 00 80 6a 18 6d 32 e8 30 32 cd 30 e8 10 2d a5 04 eb e2 20 18 6d 8c d9 "
            "48 c2 20 a5 e4 29 00 ff 18 6d 8c d9 e2 20 68 c2 20 99 3f e8 c8 c8 a5 5f "
            "18 69 40 20 99 3f e8 c8 c8 fa 18 60 fa 38 60")

    def test_radar_zoom_search_recognizes_the_same_object_channel(self):
        self.assert_source(0x0496AE,
            "bd ee 1c 29 ff 00 f0 05 c9 86 00 f0 09 bd 00 00 aa d0 ed 82 2b 00")


if __name__ == "__main__":
    unittest.main()
