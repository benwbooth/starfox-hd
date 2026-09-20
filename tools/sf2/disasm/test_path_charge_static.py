#!/usr/bin/env python3
"""Static source evidence for the complete charge-orb path and shared inputs."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathChargeStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_threshold_uses_pilot_table_and_swaps_with_active_pilot(self):
        self.assert_source(0x069C27, "B4 2B E2 20 DA 22 D4 90 06 BF 74 A4 06 8D D6 1D FA")
        self.assert_source(0x06A474, "19 19 23 23 0A 0A")
        self.assert_source(0x06A353, "AD D6 1D 48 AD DC 1D 8D D6 1D 68 8D DC 1D")
        self.assert_source(0x7F9F4F, "20 D2 9F AD B7 16 99 00 00 4C A9 CA")
        self.assert_source(0x7F9FD2, "C2 20 20 4C C7 A8 B9 00 00 8D B7 16 E2 20 20 BC C4 20 47 CB 60")

    def test_callback_reads_selected_auxiliary_and_retains_complete_level_byte(self):
        # Full callee: selected linked mode changes size, reference and
        # relative Y/Z are refreshed unconditionally, level is not masked.
        self.assert_source(0x07F54E,
            "5A AC 1F CF DA BB 7A 5A B4 2B B9 63 6B 7A DA BB 7A 29 80 D0 04 5C 6C F5 07 "
            "A9 FF 9D DA 1C C2 20 98 9D D8 1C BD E4 1C 9D D3 1C A9 00 00 9D D1 1C "
            "E2 20 DA B6 2B 9B FA B9 08 6C 9D E2 1C 7A 6B")

    def test_complete_root_waits_grows_compares_and_keeps_callback_after_hold(self):
        expected = bytes.fromhex(
            "5C 03 02 0C 40 BE 04 4D 00 00 F8 78 F0 61 08 6D 99 44 79 A2 D6 1D "
            "F0 A1 A2 6D F0 16 61 F0 4D 00 00 0B 08 99 0C 78 BE 04 19 "
            "89 22 4E F5 07 C2 20 A9 83 F0 6B 42")
        self.assertEqual(self.rom[0x4F04F:0x4F04F + len(expected)], expected)


if __name__ == "__main__":
    unittest.main()
