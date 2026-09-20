#!/usr/bin/env python3
"""Static-only provenance for decoded authored cue enqueue operations."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathSoundStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_direct_cue_zeroes_parameter_and_pair_reads_two_literals(self):
        self.assert_source(0x7FA412, "20 BC C4 8D 31 1C 9C 32 1C AC 1F CF 20 39 A4 4C D3 CA")
        self.assert_source(0x7FA424, "20 BC C4 8D 31 1C 20 E0 C4 8D 32 1C AC 1F CF 20 39 A4 4C BE CA")

    def test_routing_ors_secondary_except_for_primary_actor_and_fixed_fallback(self):
        self.assert_source(0x7FA439, "C2 20 AD 31 1C DA AE 16 1D CC C3 12 F0 08 C0 3F 03 F0 03 09 00 80 9D F6 1C E2 20 AD 16 1D 1A 1A 29 1F 8D 16 1D FA 60")

    def test_literal_readers_do_not_sample_actor_fields(self):
        for address, offset in ((0x7FC4BC, 1), (0x7FC4E0, 2)):
            self.assert_source(address, f"84 79 A0 {offset:02X} 00 A5 5E 29 EF 85 5E 8F 3A 30 00 B7 F9 8D 11 19 A5 5E 09 10 85 5E 8F 3A 30 00 AD 11 19 A4 79 60")

    def test_advances_preserve_wait_and_continue_immediately(self):
        for address, count in ((0x7FCAD3, 2), (0x7FCABE, 3)):
            self.assert_source(address, f"E2 20 C2 20 B5 2B 18 69 {count:02X} 00 95 2B E2 20 4C 75 7E")


if __name__ == "__main__":
    unittest.main()
