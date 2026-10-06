#!/usr/bin/env python3
"""Exact speed tables, dependency gates, cadence and field-width boundaries."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerSpeedStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        start = source_offset(address)
        expected = bytes.fromhex(expected)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_all_seven_six_pilot_tables(self):
        self.assert_source(0x06DBC0, '282a201e30321e2018172626535543416462282a201e3230e0e0e4e6d7d5080806060b0b696b5c5a7d7b')

    def test_action_and_contact_gates_jump_to_speed_without_reading_pilot_or_mode(self):
        self.assert_source(0x06F05D, '5a08e220c210b42bad721df00da9268dc11da9008dba1d4c7bf1b9726a2910d0045c8ff006a9008dc11da9008dba1d4c7bf1')

    def test_boost_precedes_brake_and_heading_lock_preserves_inherited_targets(self):
        self.assert_source(0x06F0C6, 'b9776b8940d0045cf0f006adb81d8db91dade21dc909f009adbd1d8dba1d4c62f1adb71d8dba1d4c62f1b9776b8920d0045c07f106adbc1d8dba1d8db91d4c62f15ab42bb9776b7a8904f0045c62f106')

    def test_directions_use_input_high_byte_not_shoulders_and_even_visit_keeps_target(self):
        self.assert_source(0x06F116, 'ad39198902d011ad39198901d00aa9008dba1d8db91d8034a5c42901d0045c88f106b518cdbb1df01a300d38e901cdbb1d1010adbb1d800b186901cdbb1d3003adbb1d9518a9008dba1d8026')

    def test_nonprimary_submode_copies_alternate_before_base_speed_chase(self):
        self.assert_source(0x06F162, 'b9a06a290fc901d0045c7bf106adc01d8dc11dadb91d8dba1db518853aadc11d22b5277f9518')

    def test_thrust_sign_extends_before_subtraction_and_stores_only_low_byte(self):
        self.assert_source(0x06F188, 'c220b9626b898000f0050900ff800329ff008597adba1d898000f0050900ff800329ff0085e4e220c220a5e4c597f03838e597c90000300ac90800100da908008008c9f8ff3003a9f8ffc900806a1003690000c900806a1003690000c900806a10036900001865978597e220a59799626b287a60')


if __name__ == '__main__':
    unittest.main()
