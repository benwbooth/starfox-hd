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

    def test_path_weapon_dispatch_table_entries(self):
        for selector, handler in ((2, 0x0DDC7D), (18, 0x0DDC61),
                (20, 0x0DDBD8), (22, 0x0DDBC1), (24, 0x0DDBAA),
                (26, 0x0DDBFA), (28, 0x0DDC1C), (30, 0x0DDC33), (32, 0x0DDC4A)):
            self.assert_source(0x03A8C1 + selector * 3, handler.to_bytes(3, 'little').hex())

    def test_all_eight_simple_profiles_have_complete_source_bodies(self):
        profiles = (
            (0x0DDBAA, 0xEE2D, "2038de"),
            (0x0DDBC1, 0xEE3B, "2038de"),
            (0x0DDBD8, 0xEE4C, "a928853aa946853c20d9df2038de"),
            (0x0DDBFA, 0xEC98, "a928853aa93c853c20d9df2038de"),
            (0x0DDC1C, 0xEC98, "2038de"),
            (0x0DDC33, 0xECF7, "2038de"),
            (0x0DDC4A, 0xEE10, "2038de"),
            (0x0DDC61, 0xEF2D, "b9250029fd992500"),
        )
        for address, path, tail in profiles:
            self.assert_source(address, "2017e0b0045ccee00dc220a9"
                + path.to_bytes(2, 'little').hex() + "992b00e220" + tail + "6b")

    def test_heavy_variant_compares_both_player_identities_and_sets_both_damage_fields(self):
        self.assert_source(0x0DDC7D,
            "2017e0b0045ccee00decc312f01fecc512f01a2038de"
            "c220a9edee992b00e220a978992d00a902992e004cbedc"
            "c220a929f0992b00e220a978992d00a902992e006b")

    def test_speed_selection_reads_primary_mode_then_immediately_generates_velocity(self):
        self.assert_source(0x0DDFD9,
            "5aacc312dabb7a5ab42bb9a06a7adabb7a29f0c910f0045cf8df0d"
            "a53a8002a53c7a991800dabb7ab5188585b5148d1215b5128d1115"
            "221f2d7fdabb7a60")

    def test_hostile_classification_primary_yaw_conditional_random_wrapping_counts_and_groups(self):
        self.assert_source(0x0DDE38,
            "daaec312b514fa18698038f91400186940c980901a22d07b7f2903853a"
            "a53af00bb921000901992100ee691dee6b1db931000950993100"
            "b93100091099310060")

    def test_path_fire_resets_every_launch_input_and_substitutes_reserved_actor_on_failure(self):
        self.assert_source(0x7F88C4,
            "a55e29e7855e8f3a3000a9008db014a9008db214a9008db414a9008db714"
            "a9008db6149cb8149cb914b52f229ca803c00000d003acd6148c71d760")
        self.assert_source(0x7F885E, "20c488b9310009109931004ce8ca")


if __name__ == "__main__":
    unittest.main()
