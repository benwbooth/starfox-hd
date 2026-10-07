#!/usr/bin/env python3
"""Source-bound exit-shield birth, strategy, and shared engine publication."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class SpecialExitStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_installer_reads_primary_low_five_bits_then_attaches_to_current_and_publishes_activity(self):
        self.source(0x06F953, 'da5a08acc312dab62b9bfab9026c291ff051c220a9ccc5855fe22022172a7fb0045c91f906a90c223d2a7fdabb7ac220a9baf99519e220a906951bdabb7ac00000f02022f1be07c220a9baf9991900e220a906991b00a9018ddf1db921000901992100287afa6b')

    def test_strategy_spins_then_parent_visibility_shape_then_frame_and_gate(self):
        self.source(0x06F9BA, 'b52609109526b52109019521bdd51c1869089dd51cbdd71c1869069dd71cb406b923002902f0045cfdf906c220b90400c99cbce220f00ca90909809dcb1cad721dd006b525090895256b')

    def test_complete_controller_publishes_shared_engine_byte_and_reuses_actual_shield_installer(self):
        self.assertEqual(self.rom[0x4D2D5:0x4D2D9], bytes.fromhex('fbe51c08'))
        self.assertEqual(self.rom[0x4E967:0x4E973], bytes.fromhex('892253f906c220a972e96b42'))

    def test_clock_gate_zero_jumps_to_word_operand_two_nonzero_advances_four(self):
        self.source(0x7FBD06, '20bcc425c4d0034cffca4ca9ca')
        self.source(0x7FCAFF, 'c220204cc7952be2204c757e')
        self.source(0x7FCAA9, 'e220c220b52b18690400952be2204c757e')


if __name__ == '__main__':
    unittest.main()
