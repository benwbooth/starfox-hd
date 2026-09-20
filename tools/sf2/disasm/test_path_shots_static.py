"""Static shot-accounting source contracts; no original instruction execution."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathShotsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_linked_paths_wrap_increment_but_saturate_decrement(self):
        self.assert_source(0x7FBDC9,
            'b406dab62b9bfab9036cc900d0045ce2bd7fb9036c3a99036c4ce8ca'
            'b406dab62b9bfadabb7afe036cdabb7a4ce8ca')
        self.assert_source(0x06A9F8,
            'da5a08e220c210b406bbb42bb9036cc900d0045c16aa06b9036c3a99036c287afa6b')

    def test_launch_gate_uses_negative_flag_and_initialization_clears_full_byte(self):
        self.assert_source(0x06A9E6, '085ab42bb9036c7ac908100328386b28186b')
        self.assert_source(0x06DB4D, 'a90099de6a99ad6a99036c')
        self.assert_source(0x069CAD, 'c220ad841b890200e220d0034cc19ca90099036c')

    def test_shape_lookup_zero_extends_selector_before_doubling_and_uses_decoded_authored_groups(self):
        self.assert_source(0x7FA62D, '20ffa6adb7160a1865548554e220a55e29e7855e8f3a3000c220a7549900004c83ca')
        self.assert_source(0x7FA6FF, '2004c58556c2202020c78554e2202028c52047cbb900008cb3168db7169cb816204cc52047cbc22060')
        self.assert_source(0x06FE93, 'e0e3b4cb04e920e918e4d0cb3ce920e96ce4eccb88e4a4e4e0e3c4e39cbc9cbc18e4fce39cbc9cbc6ce450e49cbc9cbc')
        self.assert_source(0x06FEC3, '6ce4eccb88e4a4e474bf90bfacbfc8bf6ce450e49cbc9cbc74bf58bf9cbc9cbc')

    def test_scene_reset_and_controller_publish_the_shared_flight_override(self):
        self.assert_source(0x03B948, 'a9008fd8d77e')
        self.assertEqual(self.rom[0x494A9:0x494AD], bytes.fromhex('fbd8d701'))
        self.assertEqual(self.rom[0x4EAFD:0x4EB1A], bytes.fromhex('7aa27c2aa2010eeb79a24d1b2aa20014eb063e0b170a42063e0b170a42'))
        self.assertEqual(self.rom[0x4EC7B:0x4EC98], bytes.fromhex('7aa27c2aa2018cec79a24d1b2aa20092ec0b190a063c420b060a063c42'))


if __name__ == '__main__':
    unittest.main()
