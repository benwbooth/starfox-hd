#!/usr/bin/env python3
"""Constrained movement order, carry widths, escape and final measurement."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class SurfaceMotionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        start = source_offset(address)
        expected = bytes.fromhex(expected)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_dead_zone_precedes_saved_carry_gravity_and_word_precision_shift(self):
        self.assert_source(0x0DB2B7, 'b53218690300c90600b003a9030038e9030018750c950c')
        self.assert_source(0x0DB2FC, 'b50c187539950cb50e18753b950eb51018753d9510adad1d29ff00d008b534186dab1d9534b5320a0a0a9532')

    def test_same_support_and_group_add_support_vertical_velocity_before_contact_test(self):
        self.assert_source(0x0DB381, 'bde81ccd4b19d02bbdea1c29ff00cd4d19d020bce81cf01bb9340018750e850a1869030038e508100382e501a50a950e4c24b4')

    def test_large_penetration_escapes_then_restores_horizontal_position_before_requery(self):
        self.assert_source(0x0DB3CC, 'c91e00b003825000ad5519890200f0038260000904008d5519ad5519890100f015ad4f19950cad53199510ad55190902008d5519801120b9b8b50c186502950cb5101865979510bce81cf000ad55190901008d5519822dff')

    def test_ground_carry_uses_low_tilt_bytes_and_adds_undivided_vertical_delta(self):
        self.assert_source(0x0DB44D, 'af20007029ff00eb49ffff1a8d8e15af24007029ff00eb49ffff1a8d92159c9015')
        self.assert_source(0x0DB4F6, 'c900806a1003690000187dc31c953b')
        self.assert_source(0x0DB5BC, 'bdc31c953bbdc51c')

    def test_clear_motion_delta_and_measure_ones_complement_displacement(self):
        self.assert_source(0x0DB60A, '9ec11c9ec51c9ec31c')
        self.assert_source(0x0DB670, 'ad4f1949ffff18750c8f260070ad511949ffff18750e8f280070ad531949ffff1875108f2a0070dae220a901a272fb227b787f')

    def test_polygon_escape_negates_then_rotates_to_world_but_rectangle_tail_does_not(self):
        self.assert_source(0x0DB938, 'af26007049ffff1a8f680070af2a007049ffff1a8f2e007068f0138f220070e220a901a262fd227b787fc220c210')
        self.assert_source(0x0DB983, 'a9ffff855f64026497ad971a100449ffff1ac55fb009855fad971a85026497ad9b1a')
        self.assert_source(0x0DB9CE, 'ad9d1a100449ffff1ac55fb009855fad9d1a859764027a2860')

    def test_rotation_reverses_low_word_borrow_before_high_word_subtraction(self):
        # Unlike the addition path, the low subtraction is oppositely
        # ordered to the following high subtraction. Do not replace with
        # ordinary widened vector rotation.
        self.assert_source(0x01FE9B, '2316b9183d9f24172116b53d9f27643d6813962211')


if __name__ == '__main__':
    unittest.main()
