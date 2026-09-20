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

    def test_common_creation_scopes_allocator_head_and_restores_it_on_both_results(self):
        self.assert_source(0x0DE017,
            "c220ada81248e2208ea812c220a99cbc855fe22022172a7fb0045c9ae00d"
            "c220688da812e220")
        self.assert_source(0x0DE09A, "c220688da812e2201860")

    def test_common_creation_initializes_all_fields_without_running_the_path(self):
        self.assert_source(0x0DE03D,
            "c220a91e7e991900e220a97f991b00a901992d00a901992e00221dab03"
            "b922000904992200b924000904992400b925000902992500"
            "b931000902993100a900098099cb1cb926000910992600"
            "c220a91e7e991900e220a97f991b003860")

    def test_formatter_tail_seeds_path_fields_and_sets_reciprocal_links(self):
        self.assert_source(0x03AC11,
            "b91200991300b91400991500b518991700961c941c6b")

    def test_player_linked_wrapper_uses_hit_side_and_attachment_without_child_chain(self):
        self.assert_source(0x0DE0A4,
            "2017e09023a9ff99f01cb9310009809931009606b5232940d0045ccae00d"
            "b92400098099240038601860")


if __name__ == "__main__":
    unittest.main()
