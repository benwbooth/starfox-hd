#!/usr/bin/env python3
"""Ambient phase ownership, gate ordering and exact original waveforms."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerAmbientStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_scripted_view_skip_precedes_player_read_and_special_mode_skips_only_bank(self):
        self.assert_source(0x06F2F7,
            'da5a08e220c210c220ad841b890200e220f0034c62f3a900ebb42b'
            'b9a06a29f0c930d0045c36f306')
        self.assert_source(0x06F31F,
            'b9d66a1ac91e9002a90099d66adaaabf66f306fa99d56a')

    def test_second_phase_sign_extends_sample_and_adds_to_retained_word(self):
        self.assert_source(0x06F336,
            'b9db6a1ac9209002a90099db6adaaac220bf84f306898000f005'
            '0900ff800329ff001879e26a99e26ae220fa287afa60')

    def test_exact_waveform_data_and_call_order_between_roll_and_throttle(self):
        self.assert_source(0x06F366,
            '00010202030304040404030302020100fffefefdfdfcfcfcfcfdfdfefeff')
        self.assert_source(0x06F384,
            '01000100000100000000ff0000ff00ffff00ff0000ff00000000010000010001')
        self.assert_source(0x06E258, '08e220c21020ffe720f7f22010f0226fdd07')


if __name__ == '__main__':
    unittest.main()
