#!/usr/bin/env python3
"""Complete response gates, probe order, byte writes and wing metadata."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerSurfaceStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        start = source_offset(address)
        expected = bytes.fromhex(expected)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_clears_events_and_clipping_before_gates_then_publishes_plane_and_upper_limit(self):
        self.assert_source(0x07DD6F, 'da5a08e220c210b42ba9008dba1da9009def1cad4d1b2907c900d0045c3edf07b97d6b8980f0045c3edf07c220b97d6a49ffff1a8dc01db9f56b8db81de220a9008db01d')

    def test_feedback_writes_only_pitch_high_and_bank_low_before_carry_gate_and_recoil(self):
        self.assert_source(0x07DEEB, '890ff04fb42b890cf004a9188002a9e81899ba6ab9d86ac9806ac9806a99d86aad131ec901d00ab5212920f0045c3edf07c220b93b6bc90000e220f0045c36df07c220a94000993b6be2202009e0287afa386b2009e0287afa18')

    def test_probe_uses_byte_roll_pitch_yaw_then_word_position_add_without_scaling(self):
        self.assert_source(0x07DF7E, 'b51622f03b7fa5048502a50a8508a5e48597b512224e3a7fa5048502a50a8508a5e48597b51422a9387fc220a504187d0c00990c00a5e4187d1000991000a50a187d0e00990e00e22060')

    def test_scripted_view_sound_gate_precedes_masked_event_dispatch(self):
        self.assert_source(0x07E009, 'c220ad841b890200e220f0034cc8e0c220daadba1d2907000aaabf2fe007853ae220fa6c3a00')
        self.assert_source(0x07E02F, 'c8e06de056e03fe0c8e0b2e09be084e0')

    def test_wing_class_lookup_and_all_three_offset_tables(self):
        self.assert_source(0x07D49A, 'e220c210da22f89006c220bfc3d4078502e220c220bfc9d4078508e220c220bfcfd4078597e220fa60230023000f000f0014001400d8ffd8ffceff')

    def test_wing_number_is_inherited_and_path_installation_uses_existing_formatter(self):
        self.assert_source(0x07D43D, 'c220a98cc0855fe22022172a7fb0045c84d407a500223d2a7f')
        self.assert_source(0x07D484, 'c00000f00fc220a936f5992b00e22020f5be38601860')
        # Locked speed retains the surface event byte or copies upper-limit
        # high byte; it does not manufacture a neutral target.
        self.assert_source(0x06F10E, '8904f0045c62f106')
        self.assert_source(0x06F162, 'b9a06a290fc901d0045c7bf106adc01d8dc11dadb91d8dba1d')


if __name__ == '__main__':
    unittest.main()
