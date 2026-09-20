"""Rectangular patrol source contracts, static bytes without CPU execution."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class RectangularPatrolStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_selector_variants_and_damage_preserve_signed_half_step_words(self):
        self.assert_source(0x089118, '0b012f1726110b022f1726116b2f')
        self.assert_source(0x0891A3, '0b46a1ee2da1ae114cc911ee2da9c81107a9fb5080398f8054398050843d8f84543d846da242')
        self.assert_source(0x7FCB47, '08c2208eb11629ff00898000f0041869411c186db116a82860')
        self.assert_source(0x7FA5D4, '20bcc42047cbc220b9000010011ac900806a9900004cd3ca')

    def test_player_publication_uses_displacement_except_in_alternate_mode(self):
        self.assert_source(0x07EA15, '08e220c210c220b50c8decd7b50e8deed7b5108df0d7e2205ab42bb9a06a7a29f0c910f0045c51ea07c220b5328d1c1eb5348d1e1eb5368d201e8014c220bdc11c8d1c1ebdc31c8d1e1ebdc51c8d201e286b')

    def test_death_axis_zero_waits_and_depth_axis_adds_an_extra_yield(self):
        self.assert_source(0x0891C9, '4ba3115c6794ec112a9401e11161058f3d54103d4416ee1161058f39540c394417ee1103053d015e0141b4860007c0005701009d6400e587d7418b8c4168878a2a2f021112e588d710')

    def test_attachment_cooldown_launches_and_eighteen_step_signed_arc(self):
        self.assert_source(0x089283, '5cc4fd2300fd040a194c9012424ba8124b8c126ba1611200031bb300a10e1472207120441067a9ae126fa94ea1140aee14a1ba1207a1fc07a1024e14a1f2a405c6124269a9e412fa74f5b0bea1f501010000ecff3c00095de0cee51264040b1ea942')
        self.assert_source(0x00B31B, 'ced8e0e8eef4f8fcfe00000204080c1218202832')


if __name__ == '__main__':
    unittest.main()
