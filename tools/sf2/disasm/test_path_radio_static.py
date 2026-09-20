#!/usr/bin/env python3
"""Complete static source contracts for authored radio request entry."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathRadioStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_message_handlers_share_the_service_but_sample_distinct_operand_kinds(self):
        self.assert_source(0x7F8E69, "20 BC C4 DA 8D B1 16 A5 5E 29 E7 85 5E 8F 3A 30 00 AD B1 16 22 04 CF 0A FA 4C D3 CA")
        self.assert_source(0x7FB7AA, "20 BC C4 20 47 CB DA A5 5E 29 E7 85 5E 8F 3A 30 00 B9 00 00 22 04 CF 0A FA 4C D3 CA")

    def test_full_request_service_decrements_message_and_replaces_pending_placement(self):
        self.assert_source(0x0ACF04, "08 E2 20 C2 10 3A 8F 31 CF 7E A9 01 8F 32 CF 7E A9 97 8F 44 D7 7E AF 75 D7 7E F0 06 A9 8B 8F 44 D7 7E AF 31 1E 00 C9 92 30 0A A9 23 8F 44 D7 7E A9 01 80 02 A9 00 8F 59 D7 7E A9 00 8F 45 D7 7E 28 6B")

    def test_tracked_y_is_a_smoothed_projected_marker_not_actor_world_height(self):
        self.assert_source(0x07A439, "A0 3F 03 8C B0 1D AC D6 14 22 D2 2B 7F DA 5A B4 2B BB 7A C2 20 BD 9C 6B 99 0C 00 BD 9E 6B 99 0E 00 BD A0 6B 99 10 00 E2 20 FA DA BB AC B0 1D 20 E8 AF FA")
        self.assert_source(0x07A4D3, "A5 7B 18 69 18 85 08 A5 08 CD 31 1E F0 21 38 ED 31 1E C9 00 30 08 C9 02 10 0A A9 02 80 06 C9 FE 30 02 A9 FE C9 80 6A 10 02 69 00 18 6D 31 1E 8D 31 1E")


if __name__ == "__main__":
    unittest.main()
