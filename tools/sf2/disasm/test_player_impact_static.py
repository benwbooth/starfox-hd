#!/usr/bin/env python3
"""Source-owned contact fields, post-flight recoil and surface damage order."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerImpactStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        start = source_offset(address)
        expected = bytes.fromhex(expected)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_recoil_replaces_horizontal_velocity_integrates_all_axes_then_decays(self):
        self.assert_source(0x06E273, '5ab42bb9ad6ad0045cd0e206a9409de21cbde21c187d14009de21cb9ad6a9de31cbde31c8585bde21c228b2d7fc220a5029532a5979536e22022242c7fb9ad6ac900f016300b38e914c900100da9008009186914c9003002a90099ad6a')

    def test_horizontal_helper_advances_the_negated_heading_before_single_products(self):
        self.assert_source(0x7F2D8B, '867d847f6408640949ff1aaae210e8c8bf923d7f8581bfd23d7f8582a585')

    def test_contact_turn_updates_pose_yaw_and_motion_recoil_not_a_private_snapshot(self):
        self.assert_source(0x06988C, 'acd614b42b99d46ac9806a1002690099ad6a7a286b')

    def test_impact_updates_shared_charge_control_and_heading_return_bank(self):
        self.assert_source(0x06AAE0, 'b9096c0940297f99096ca91e99da6aa5c42901d0045c02ab06b9da6a49ff1a99da6a287a6b')

    def test_position_history_is_captured_before_mode_movement(self):
        self.assert_source(0x069E25, 'c230b50c99c76ab50e99c96ab51099cb6ae220')

    def test_first_damage_probe_preserves_group_but_publishes_height(self):
        self.assert_source(0x07E195, 'b42bbdea1c48223aaf0d689dea1cc220a5088dae1d')

    def test_forecast_uses_old_position_and_retained_displacement_then_clears_only_obstruction(self):
        self.assert_source(0x07E1D2, 'dac220aed614b90b6b1879c76a950cb90d6b1879c96a950eb90f6b1879cb6a9510e220223aaf0dc220bde81c8502bdeb1c29ff008504e2209bfab42bc220b50ec508e22010045ceee207a504c906d0045ceee207b9e66b29ef99e66bc220a502e220d0045ceee207c220a5029de81ce220a5049deb1c')

    def test_protection_tests_low_five_bits_and_exits_after_sound_only(self):
        self.assert_source(0x07E248, '5ab42bb9026c7a291fd0045c67e2072205ab06b9e26bf0045ceee2074ceee2')

    def test_turn_pitch_impact_and_reserve_precede_cue_and_empty_reserve_kills_hull(self):
        self.assert_source(0x07E27D, '22429806b42bade21d29ffc909f00ec220b9b96a186900e899b96ae22022aeaa06bce81cb92e00853a5ab42bb9006cf00c38e53a1002a90099006c8004a900952d7a')

    def test_surface_sound_threshold_is_unsigned_and_routes_by_primary_identity(self):
        self.assert_source(0x07E2BF, 'b92e00c9049015c220a91200ecc312f00309008022096e7fe2208013c220a91300ecc312f00309008022096e7fe220287afa6b')


if __name__ == '__main__':
    unittest.main()
