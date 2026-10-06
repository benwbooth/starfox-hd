"""Source contracts for strategy-time loop selection and later publication."""

import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PositionalAudioStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(raw)], raw)

    def test_strategy_tail_uses_global_gate_actor_control_and_fixed_primary_marker(self):
        # The gate is absolute, not indexed: this is NOT an actor-local flag.
        self.assert_source(0x7F365A,
            'da5a08e220c210add31c8901d00bbdcc1cf006a03f0320b836287afa')

    def test_unsigned_nearest_comparison_keeps_ties_and_distance_preserves_carry(self):
        self.assert_source(0x7F36B8,
            'da088dc01b203637c220adb516cdf41cb0628df41ce220adc01b8ded1c8eee1c')
        self.assert_source(0x7F3736,
            '08c220bd0c0038f90c008502100449ffff1a8504bd100038f910008508'
            '100449ffff1a850a1865046a8db5162860')

    def test_distance_and_bearing_bits_are_ored_into_the_complete_authored_control(self):
        self.assert_source(0x7F36D8,
            'c220a20000adf41cc99001300de8c9e8033007e8c9d0073001e8'
            'e220bf2f377f0dc01b8dc01b')
        self.assert_source(0x7F3700,
            '22581d7fe220eb38f91500a20000c910900fc9f0b00bc9709006'
            'c99090038002e8e8bf33377f0dc01b8dec1c28fa60')
        self.assert_source(0x7F372F, '001020304080c0')

    def test_epoch_reset_and_frozen_or_empty_publication_are_separate_boundaries(self):
        self.assert_source(0x03C193, 'a2ffff8ef41c')
        self.assert_source(0x03815A,
            'c220ad841b890100e220d02cadec1c8de61caded1c8de71caeee1c'
            '8ee81caef41c8eea1cc220adf41c1ad00c9ce61c9ce71ca200008ee81c')


if __name__ == '__main__':
    unittest.main()
