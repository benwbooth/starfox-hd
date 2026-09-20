#!/usr/bin/env python3
"""Shared countdown provenance from source bytes, without executing gameplay."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathCountdownStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_counter_service_guards_zero_and_yields_at_its_loop_back_edge(self):
        self.assertEqual(self.rom[0x404FF:0x40502], bytes.fromhex("170e05"))
        self.assertEqual(self.rom[0x4050E:0x4051F], bytes.fromhex("f7ef48c47aa92a67a91c05e786d7161205"))
        for address, value in ((0x400C6, 70), (0x40354, 60), (0x404DF, 80)):
            self.assertEqual(self.rom[address:address + 4], bytes((0xFB, 0x86, 0xD7, value)))
        # The broader source initializer clears the timer and its neighbor
        # as a word; the native path interface exposes only this byte.
        self.assert_source(0x04B1C0, "8B 08 E2 20 A9 7E 48 AB C2 30 9C 86 D7")

    def test_absolute_and_indexed_import_alias_one_shared_byte(self):
        self.assert_source(0x7F9F4F, "20 D2 9F AD B7 16 99 00 00 4C A9 CA 20 E7 9F AD B7 16 99 00 00 4C BE CA")
        self.assert_source(0x7F9FD2, "C2 20 20 4C C7 A8 B9 00 00 8D B7 16 E2 20 20 BC C4 20 47 CB 60")
        self.assert_source(0x7F9FE7, "20 E0 C4 C2 20 29 FF 00 A8 B9 5C D7 8D B7 16 E2 20 20 BC C4 20 47 CB 60")

    def test_exports_sample_actor_before_writing_and_indexed_export_masks_next_opcode(self):
        self.assert_source(0x7F9F83, "20 BF 9F E2 20 AD B7 16 99 00 00 4C A9 CA")
        self.assert_source(0x7F9F9D, "20 BF 9F 29 FF 00 A8 E2 20 AD B7 16 99 5C D7 4C BE CA")
        self.assert_source(0x7F9FBF, "20 BC C4 20 47 CB C2 20 B9 00 00 8D B7 16 20 4C C7 A8 60")
        self.assert_source(0x7FC346, "C2 20 20 20 C7 A8 E2 20 20 04 C5 99 00 00 4C A9 CA")

    def test_external_byte_arithmetic_wraps_without_an_implicit_zero_guard(self):
        self.assert_source(0x7FB805, "C2 20 20 20 C7 A8 E2 20 B9 00 00 1A 99 00 00 4C BE CA")
        self.assert_source(0x7FB827, "C2 20 20 20 C7 A8 E2 20 B9 00 00 3A 99 00 00 4C BE CA")


if __name__ == "__main__":
    unittest.main()
