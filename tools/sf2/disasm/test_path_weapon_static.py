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
        for selector, handler in ((2, 0x0DDC7D), (4, 0x0DDDC0), (6, 0x0DDD4E),
                (8, 0x0DDD20), (10, 0x0DDCFD), (12, 0x0DDCC3), (14, 0x0DDCC3),
                (16, 0x0DDCC3), (18, 0x0DDC61),
                (20, 0x0DDBD8), (22, 0x0DDBC1), (24, 0x0DDBAA),
                (26, 0x0DDBFA), (28, 0x0DDC1C), (30, 0x0DDC33), (32, 0x0DDC4A)):
            self.assert_source(0x03A8C1 + selector * 3, handler.to_bytes(3, 'little').hex())

    def test_rapid_mesh_profiles_gate_before_allocation_copy_spin_and_reaim_using_retained_point(self):
        self.assert_source(0x0DDCFD,
            'e220c21022e6a906b0045ccee00d20a4e0b0045ccee00dc220a950e4990400'
            'e2208021e220c21022e6a906b0045ccee00d20a4e0b0045ccee00dc220a9fce3'
            '990400e2205ab42bb9dd6a7a99d71c4c74dde220c21022e6a906b0045ccee00d'
            '20a4e0b0045ccee00dc220a9c4e3990400e220a90099d71cc220a973e9992b00'
            'e220b516991600a903098099cb1c5ada5ab42bbbc220acd614bd9c6b990c00bd'
            '9e6b990e00bda06b991000e220fa2288217fe220eb49ff1ad514f0009514fa7a20cfe06b')
        self.assert_source(0x07AA0D, '5ab42bbb7ac220b90c009d9c6bb90e009d9e6bb910009da06be220')
        self.assert_source(0x7F2188, 'da5ac220b90c0038f50c8502b9100038f510850822581d7fc2307afa6b')
        self.assert_source(0x06E879, 'a9e099dd6a8005a92099dd6a')

    def test_alternate_rapid_equipment_gate_precedes_count_gate_and_selects_sprite_or_mesh(self):
        self.assert_source(0x0DDDC0,
            'b42bb9066c2903d0045ccee00d22e6a906b0045ccee00d20a4e0b0045ccee00d'
            '5ab42bb9066c7a29033af01748c220a950e4990400e220683af004a9048022a904'
            '801ec220a958bf990400e220b920000920992000a90099c81ca90099da1ca902'
            '992e00c220a91aeb992b00e220adf21d99120020cfe06b')

    def test_charged_mesh_profile_uses_player_linked_creation_published_pitch_and_caller_roll(self):
        self.assert_source(0x0DDCC3,
            'e220c21020a4e0b0045ccee00dc220a964ca990400e220c220a984f0992b00'
            'e220b516991600adf21d99120020cfe0a978992d00a90a992e006b')
        self.assert_source(0x07D6D3, '9cf21d5ab42bb9a06a7a29f0c920d0045cefd607b5128df21d')

    def test_original_weapon_shape_is_optional_reflection_record_not_a_render_mesh_copy(self):
        self.assert_source(0x0DE0CF,
            'da5a08c220b904008502e220dabb7aa90a2260237fc220a50299626ae220dabb7a287afa60')
        self.assert_source(0x07F26D,
            'a90a223b237fc00000d0045c8bf207c220b9626a8502e220c220a5028004'
            'c220b5047a990400e220')

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
