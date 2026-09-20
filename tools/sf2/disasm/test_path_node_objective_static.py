"""Node objective contracts from source bytes, without CPU execution."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM, MapAddress, MapExtractor


class NodeObjectiveStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_map_installers_and_full_word_export_do_not_scale_the_literal_index(self):
        commands = MapExtractor(self.rom).extract().commands
        self.assertEqual([(c.address.bank, c.address.offset) for c in commands if c.raw_hex == '8c6c54'],
                         [(5, 18979), (5, 19290)])
        self.assert_source(0x7F9FAF, '20bf9f29ff00a8adb716995cd74cbeca')
        self.assert_source(0x7F9FBF, '20bcc42047cbc220b900008db716204cc7a860')
        # Fifth one-based selector raises 0x10, preserving all other bits.
        self.assertEqual(self.rom[0x454FC:0x45508], bytes.fromhex('7ba39a0b05a1d8a1a380a39a'))

    def test_node_loading_is_byte_wide_but_campaign_writeback_is_word_wide(self):
        self.assert_source(0x04B1FC, 'ae07dbbd0400c22029ff00e2208db51bbd08008da51bbd0a008df6d7')
        self.assert_source(0x04B23B, 'da08c220ada51b9d0800adf6d79d0a00e220aeb51bada1d79d94d728fa60')

    def test_four_reveal_panels_use_signed_offsets_and_separate_shapes(self):
        self.assert_source(0x06FC45, '4cffc4ff3c00b400ccda04db44d994da')
        # HIGH is the construction selector; LOW remains the color cycle.
        self.assertEqual(self.rom[0x4563D:0x45652], bytes.fromhex('7aa2089145fc06a28e914dfc06a20407a2024e13a2'))
        self.assertEqual(self.rom[0x45671:0x45682], bytes.fromhex('902afc06a1896da18a2aa10781566ba142'))

    def test_installed_layouts_retain_the_null_target_cleanup_diagnostic(self):
        # The actual map installations choose initial health/count 2 and 1.
        # Their constructor never creates slot 4, yet cleanup requests it.
        extractor = MapExtractor(self.rom)
        for address, value in [(18971, '362d0002'), (19282, '362d0001')]:
            start = extractor.file_offset(MapAddress(5, address))
            self.assertEqual(self.rom[start:start + 4], bytes.fromhex(value))
        self.assertEqual(self.rom[0x4550D:0x45515], bytes.fromhex('6604660366026601'))
        self.assert_source(0x7F2A7B, '8d2a19dab429f00bb91300cd2a19f003bb80f1fa6b')
        self.assert_source(0x7F8B7B, '227b2a7fb925000908992500fa4cd3ca')


if __name__ == '__main__':
    unittest.main()
