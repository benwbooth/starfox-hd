#!/usr/bin/env python3
"""Static source checks for typed program-resource capacity and ownership."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class ProgramResourcesStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_initial_capacity_and_single_free_block(self):
        self.assert_source(0x7F1741, "A2 02 00 9E 61 6A 9E 63 6A A9 FE 47 9D 65 6A 8E 61 6A")

    def test_rounding_minimum_first_fit_and_empty_list_anomaly(self):
        self.assert_source(0x7F18C1, "1A 29 FE FF F0 72 1A 1A C9 06 00 B0 03 A9 06 00")
        self.assert_source(0x7F18D1, "AE 61 6A F0 63 DD 65 6A 90 0D F0 0B BC 61 6A BB D0 F3 A9 00 00 80 51")

    def test_split_only_above_six_units_and_allocate_from_high_end(self):
        self.assert_source(0x7F18EB, "BD 65 6A 38 ED 61 B2 C9 06 00 90 16 F0 14 9D 65 6A 8A 18 7D 65 6A AA AD 61 B2 9D 61 6A E8 E8 8A 80 2C")

    def test_release_checks_adjacency_then_inserts_at_head(self):
        self.assert_source(0x7F17AF, "98 18 79 65 6A CD 61 B2 F0 24 CC 63 B2 F0 2F B9 61 6A A8 D0 EB")
        self.assert_source(0x7F17C4, "BD 61 6A 9D 65 6A AC 61 6A 8E 61 6A 8A 99 63 6A 98 9D 61 6A 9E 63 6A 80 50")
        self.assert_source(0x7F17DD, "B9 65 6A 18 7D 61 6A 99 65 6A BB 20 5A 18 80 40")
        self.assert_source(0x7F1868, "CC 63 B2 F0 11 98 18 79 65 6A CD 61 B2 F0 1C B9 61 6A A8 D0 EB")

    def test_actor_allocation_adds_link_and_prepends_chain(self):
        self.assert_source(0x7F194E, "18 69 02 00 22 A7 18 7F C9 00 00 F0 0F 5A A8 BD DC 1C 99 61 6A 98 9D DC 1C 7A 1A 1A 6B")

    def test_actor_retirement_walks_chain_before_clearing_auxiliary_handles(self):
        self.assert_source(0x7F19B3, "BC DC 1C F0 10 DA BE 61 6A 98 22 77 17 7F 9B D0 F5 FA 9E DC 1C 9E EC 1C 9E DE 1C 9E E0 1C")

    def test_replacement_allocates_new_before_copy_and_old_release(self):
        self.assert_source(0x7F1B00, "C0 00 00 D0 03 82 46 FE")
        self.assert_source(0x7F1B1C, "AD 2B CF 22 4E 19 7F F0 3E 48 5A DA BB A8")
        self.assert_source(0x7F1B4A, "FA 68 22 6B 19 7F 68 8D 2B CF")


if __name__ == "__main__":
    unittest.main()
