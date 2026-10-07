#!/usr/bin/env python3
"""Complete source side effects for node-exit view operations."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class NodeExitStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_initial_view_wraps_all_coordinates_then_sets_only_angle_bytes(self):
        self.source(0x06FA66, 'c220a9008018750c950ca9008018750e950ea900801875109510e220a91a9512a9409514a90095166b')

    def test_yaw_damping_clears_projection_for_both_signs_and_uses_signed_floor(self):
        self.source(0x07F501, 'c220ad521e10039c521e9c521ea03f03b91400c900806a853cc900806ac900806a18653c991400e2206b')

    def test_base_bias_and_node_variant_have_distinct_live_producers_and_consumers(self):
        self.source(0x079527, 'a508186d521e186d441e8d3c1e')
        self.source(0x04B26E, 'b93e008d091e')
        self.assertEqual(self.rom[0x4B93A:0x4B93E], bytes.fromhex('792e091e'))
        self.source(0x07FE51, '48e664c364c364c3')

    def test_map_restore_copies_both_identity_parts_without_clearing_saved_state(self):
        self.source(0x7FBF3D, 'ad771d8d2e19daae781d8e5716fa4ce8ca')

    def test_rotation_copy_zeroes_each_low_byte_without_negation(self):
        self.source(0x7FC005, 'c2202020c7a8b5112900ff991200b5132900ff991400b5152900ff991600e2204cbeca')

    def test_tracking_aim_uses_shared_target_and_masks_pitch_shift(self):
        self.source(0x7FBE38, 'acff1d8002b4069c9d14dac2202020c7aa2078c72907008db116e22022a5217f')
        self.source(0x7FBE8C, 'acff1d8007ac1fcf8002b4069c9d14dac2202020c7aa2078c72907008db116e22022a5217f')


if __name__ == '__main__':
    unittest.main()
