"""Live objective count widths and source campaign representation, no execution."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class ObjectiveCountsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_planet_load_and_writeback_keep_the_count_byte(self):
        self.assert_source(0x04B218, 'a900ebaeb51bbd94d7d011adb51b0a186db51b186df2d7aabfe6b0008df4d78da1d760')
        self.assert_source(0x04B24B, 'e220aeb51bada1d79d94d728fa60')

    def test_space_load_sums_nibbles_but_retains_the_packed_campaign_byte(self):
        self.assert_source(0x04B283, 'c220aea71b8a0a853ae220bdc2d78da1d748290f8502684a4a4a4a1865028df4d7')
        self.assert_source(0x04B2B1, 'da08e220aea71bada1d79dc2d7c220')
        # The two word-wide writeback branches observe the companion byte.
        self.assert_source(0x04B2D7, 'adf4d73a9938008006adf4d7993800')

    def test_path_count_mutations_are_wrapping_bytes_not_saturating_nibbles(self):
        self.assert_source(0x7FB805, 'c2202020c7a8e220b900001a9900004cbeca')
        self.assert_source(0x7FB827, 'c2202020c7a8e220b900003a9900004cbeca')
        self.assert_source(0x7FC346, 'c2202020c7a8e2202004c59900004ca9ca')
        self.assertEqual(self.rom[0x45FE5:0x45FEB], bytes.fromhex('e7f4d7e7a1d7'))


if __name__ == '__main__':
    unittest.main()
