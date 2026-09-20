#!/usr/bin/env python3
"""Weapon-class and launch contracts read only from immutable source bytes."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathWeaponStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_formatter_sets_weapon_class_and_ordinary_target_scans_exclude_it(self):
        self.assert_source(0x03AB1D, "a902990900b931000902993100")
        self.assert_source(0x0DE072, "b931000902993100")
        self.assert_source(0x7F20A0, "b5312902f0045ceb207f")
        self.assert_source(0x07F0CD, "b5312902f0045c33f107")

    def test_class_masks_preserve_or_change_every_bit_and_advance_immediately(self):
        self.assert_source(0x7FC3AF,
            "20bcc48db116b5312db11695314cd3ca"
            "20bcc48db116b5310db11695314cd3ca")


if __name__ == "__main__":
    unittest.main()
