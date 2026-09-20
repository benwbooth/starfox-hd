#!/usr/bin/env python3
"""Death marking, retained health records and the movement-tail boundary."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM
from extract_path import PathExtractor


class PathDeathStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_complete_death_handler_and_direct_callback_tail(self):
        self.assertEqual(PathExtractor(self.rom).handler_entry(0x10).handler_address, 0x7F8B94)
        self.assert_source(0x7F8B94,
            "b528f00dc22029ff00a8e220a90099fb17b5232910d0045cce8b7f"
            "b429c00000f018c220b9290048e220b921000901992100a900992d007a80e3"
            "b52109019521a900952d4c709e")
        self.assert_source(0x7F9E70,
            "20a89ab5222901f0045c8b9e7fb5232910d0045c8b9e7f2219237f")

    def test_health_initialization_and_inherited_health_consumers(self):
        # Six initialized records: old Fox plus five friends. The selector
        # is one-based; byte zero deliberately means no shared health write.
        self.assert_source(0x0382BB,
            "08e220a9288dfb178dfe178dfc178dfd178dff178d0018")
        self.assert_source(0x7F8E88,
            "b528f012c22029ff00a8e220b9fb17c90bb0034cf3ca4cbeca")
        self.assert_source(0x7F8EA1,
            "b528f015c22029ff00a8e220b9fb1738e90ab002a90099fb174ce8ca")

    def test_shared_tail_integrates_relative_then_carry_and_exit_latches(self):
        self.assert_source(0x7F9E8B,
            "b5232904f0045c9f9e7fb5252904d0045cc99e7f"
            "c220bdcf1c187d32009dcf1ce220c220bdd11c187d34009dd11ce220"
            "c220bdd31c187d36009dd31ce220")
        self.assert_source(0x7F9EFD,
            "b52229fd9522b521297f9521b52629fd9526b52629fb95266b")


if __name__ == "__main__":
    unittest.main()
