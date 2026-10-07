#!/usr/bin/env python3
"""Flight translation gates, proxy owners, signed thrust and support handoff."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerMotionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        start = source_offset(address)
        expected = bytes.fromhex(expected)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_protection_adjusts_speed_axes_and_thrust_before_scripted_view_gate(self):
        self.assert_source(0x06EE1D, 'ad131ec901d00ab5212920f0045c32ee06a9149518b9846b295f99846ba90099626bc220a92c01186d0f1e853cd50ee2201005a96499626bc220ad841b890200e220f0034cecee')

    def test_thrust_sign_is_high_byte_of_adjacent_word_and_base_velocity_is_added_separately(self):
        self.assert_source(0x06EE64, 'b9626b850220efeec220b532990b6bb9616b1005a900008002b534990d6bb536990f6be220b518850220efee')
        self.assert_source(0x06EE90, 'c220b53218790b6b990b6bb53418790d6b990d6bb53618790f6b990f6b')

    def test_proxy_keeps_generated_velocity_and_reads_fine_pitch_high_before_axis_filter(self):
        self.assert_source(0x06EEEF, 'acd614a50299180022d22b7f5ab42bb9ba6a85027aa502991200dabb7ab5188585b5148d1215b5128d1115221f2d7f')
        self.assert_source(0x06EF21, 'c220b932009532b934009534b936009536e220b42bb9846b2980')

    def test_free_flight_clears_all_carried_and_pending_motion_after_integration(self):
        self.assert_source(0x06EEAD, 'ade21d29ff00c90900d0045c73ef06b90b6b18750c950cb90d6b18750e950eb90f6b1875109510e220c220a900009dc11c9dc31c9dc51c9539953b953de220287a60')

    def test_constrained_branch_retains_vertical_delta_and_copies_only_support_identity_and_group(self):
        self.assert_source(0x06EF84, 'b90d6b18750e950eb90b6b9dc11c18750c8dba1db90f6b9dc51c1875108db61db9116b9532b9136b9536a900009534a900008dab1dc220ad6f1d9de81ce220ad711d9dea1c5aa9012282b20d7a')

    def test_response_is_saved_before_base_velocity_restore_and_obstruction_only_sets_its_bit(self):
        self.assert_source(0x06EFD1, 'c220bde81c8d6f1de220bdea1c8d711dc220b53299116bb53699136badc01d9532adbe1d9534adbc1d9536e220ad55198904f008b9e66b091099e66b287a60')
        self.assert_source(0x069681, '9c0f1ec2209c6f1de2209c711d287afa6b')


if __name__ == '__main__':
    unittest.main()
