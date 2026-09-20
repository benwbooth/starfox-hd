"""Paired-part patrol source publication and graph-sensitive contracts."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PairedPatrolStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_original_identity_survives_health_initialization_and_retirement_publishes_only_low_byte(self):
        self.assert_source(0x08B114, '4e272d7aa131da27a1538d006902418589cd487aa12e2aa1013231162731')
        self.assert_source(0x08B1B8, '4e272d7aa131da27a1538d0069024e142ef8a032418589cd487aa12e2aa101dc3116d131')
        self.assert_source(0x098768, '7aa131d827a17fa13142')
        self.assert_source(0x098985, '0b642d0b042e04ccf68642')

    def test_detached_part_arc_and_callbacks_preserve_both_manual_animation_seeds(self):
        self.assert_source(0x08B27C, 'f52cd1b93264041e003c000000019cf99bf510d1b9326404e2ff3c000000029c1b049b42')
        self.assert_source(0x08B2B9, '5cc4fd3000fd040a194cc632424beb324bc2328f8e8f8e50328e58a23f07a2e00b02a1610f00031bb300a10e145214a244101c010842')
        self.assert_source(0x00B31B, 'ced8e0e8eef4f8fcfe00000204080c1218202832')

    def test_two_charge_emissions_wait_fifteen_and_the_charge_itself_grows_ten_steps(self):
        self.assert_source(0x08B2AD, '391a48610241178c030f440f')
        self.assert_source(0x098C17, 'f5ccbe368c0a0a0000000000000b42')
        self.assert_source(0x098C36, '4d0008cd5c391afa7b610a07990244000ff2350f')


if __name__ == '__main__':
    unittest.main()
