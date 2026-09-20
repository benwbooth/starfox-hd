#!/usr/bin/env python3
"""Encounter synchronization masks: exact handlers, no source execution."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathEncounterSignalsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_raise_and_clear_use_literal_word_masks_and_preserve_other_bits(self):
        self.assert_source(0x7FB7C6, "c2202020c70c7dd74cbeca")
        self.assert_source(0x7FB7D1, "c2202020c71c7dd74cbeca")

    def test_branch_tests_any_intersection_and_bypasses_ifnot(self):
        self.assert_source(0x7FB7DC, "c2202020c72c7dd7f0038222134c94ca")
        self.assert_source(0x7FB7EC, "c2202020c72c7dd7d0038212134c94ca")
        self.assert_source(0x7FCB0B, "c2202078c7952be2204c757e")
        self.assert_source(0x7FCA94, "e220c220b52b18690500952be2204c757e")

    def test_reset_clears_both_bytes_and_every_operation_uses_immediate_continuation(self):
        self.assert_source(0x7FB7FC, "9c7dd79c7ed74ce8ca")
        self.assert_source(0x7FCABE, "e220c220b52b18690300952be2204c757e")
        self.assert_source(0x7FCAE8, "e220c220f62be2204c757e")


if __name__ == "__main__":
    unittest.main()
