#!/usr/bin/env python3
"""Source-owned corridor producer, wrapped octant correction and history."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerBoundaryStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        start = source_offset(address)
        expected = bytes.fromhex(expected)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_action_gate_and_family_precede_fixed_height_leveling(self):
        self.assert_source(0x07E2F9, '5ab42bb9776b7a8904d0045c81e307b42b5ab42bb9a06a7a29f0c910f0045c42e307')
        self.assert_source(0x07E31B, 'c220a9d8ff8508225bf206e220c220a50299b96ae220c220b50e853aa9d8ff22a3257f950ee220')

    def test_level_pitch_uses_wrapped_height_difference_and_four_hundred_lookahead(self):
        self.assert_source(0x06F262, 'c220a50838fd0e008502a9900185082250af07')

    def test_geometry_uses_retained_center_width_and_the_shared_steering_heading(self):
        self.assert_source(0x07E342, 'c220b9af6a8dbc1db9b36a8db81db9b56a8db61db9ae6a29ff008db21de220')

    def test_correction_copies_back_all_axes_but_replaces_only_horizontal_history(self):
        self.assert_source(0x07E361, 'acd61422aa2b7f20d6e3901422be2b7fb42bc220b50c99ed6bb51099f16be220')

    def test_vertical_admission_has_exclusive_low_inclusive_high_and_negative_bypass(self):
        self.assert_source(0x07E38D, 'c220a50a3012a50838e50ad50e1035a50818650ad50e302c')

    def test_probe_always_clears_proxy_hit_before_eight_direction_dispatch(self):
        self.assert_source(0x07E3EF, 'c210b52029fd9520c220b50c85028504b510859785e4adb21d')

    def test_sum_corrections_negate_before_signed_half_and_difference_corrections_after(self):
        self.assert_source(0x07E51D, '49ffff1ac900806a4818750c950c688040')
        self.assert_source(0x07E543, 'c900806a4818750c950c6849ffff1a801a')

    def test_diagonal_span_uses_signed_full_product_shifted_by_one_byte(self):
        self.assert_source(0x07E5B9, 'adb61d8502a96a01850422fce00760')
        self.assert_source(0x07E580, 'adb81d186dbc1d1865098dbe1dadb81d186dbc1d38e5098dc01d60')

    def test_installer_combines_quantized_anchor_heading_and_authored_offset(self):
        self.assert_source(0x07F893, '08e220c210b51429e01865a785a78dbf16')
        self.assert_source(0x07F8CE, '890400f0045ce4f807dabb2284e307fab0045c4ff907')

    def test_installer_publishes_shared_flag_heading_center_and_extents_then_clears_parameters(self):
        self.assert_source(0x07F91C, 'b42bb9776b090499776badbf1699ae6ac220a50299af6aa50899b16aa59799b36aadb71699b56aadb91699b76ae2207adabb7a')
        self.assert_source(0x07F94F, 'c220a900008db7168db9168dbb16286b')


if __name__ == '__main__':
    unittest.main()
