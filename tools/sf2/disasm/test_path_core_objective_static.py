"""Source-only planetary core installation, thresholds and beam contracts."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM, MapExtractor


class CoreObjectiveStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_four_maps_install_two_or_four_defender_gates(self):
        commands = MapExtractor(self.rom).extract().commands
        roots = [(i, c) for i, c in enumerate(commands) if c.raw_hex == '8c1d5e']
        self.assertEqual([(c.address.bank, c.address.offset) for _, c in roots],
                         [(5, 0x2B7D), (5, 0x2B9D), (5, 0x4FB9), (5, 0x526D)])
        self.assertEqual([commands[i - 2].raw_hex for i, _ in roots],
                         ['362d0002', '362d0004', '362d0002', '362d0004'])
        self.assertEqual([commands[i - 1].raw_hex for i, _ in roots], ['362e000a'] * 4)

    def test_shield_release_and_contact_material_phases_are_exact_source_commands(self):
        self.assertEqual(self.rom[0x45E4A:0x45E68], bytes.fromhex('eea227525e164a5e3d0203323d017aa19867a1675e41375f032016585e19'))
        self.assertEqual(self.rom[0x45EEB:0x45EF6], bytes.fromhex('2e0c74818c3f9d5f16f05e'))
        self.assertEqual(self.rom[0x45F9D:0x45FD3], bytes.fromhex('0cf4818c2ffd090807950816a55f0b69a1ee2da1d25f2b8cfe82bd5f4aaf85050cfe828c0069040b4ba1ee2da1d25f4bab5f4cd35f42'))
        # These are actual words inside the already-extracted material data,
        # not invented phase IDs or pointer arithmetic in native execution.
        self.assert_source(0x018174, '993f993f993f993f')
        self.assert_source(0x0181F4, '0000010102020303')
        self.assert_source(0x0182FE, '0000010103030202')

    def test_beam_constructor_resets_selector_emits_four_and_renumbers_two_through_five(self):
        self.assertEqual(self.rom[0x45F37:0x45F69], bytes.fromhex('fb64d7006104f5ccbe695f64010000e8fe0000029c7a2e0891c9fb062e8e91cdfb062e92072e024e132e6f2ee564d79b4542'))
        self.assert_source(0x06FBC9, '000000001801e8fe00000000')


if __name__ == '__main__':
    unittest.main()
