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


if __name__ == '__main__':
    unittest.main()
