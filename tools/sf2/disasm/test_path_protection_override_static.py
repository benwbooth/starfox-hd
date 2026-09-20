"""The shared protection-override branch bypasses IFNOT handling."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM
from extract_path import PathExtractor


class ProtectionOverrideStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_override_tests_only_low_bit_and_uses_direct_jump_or_advance(self):
        self.assertEqual(PathExtractor(self.rom).handler_entry(0x30).handler_address, 0x7F9034)
        self.assert_source(0x7F9034, 'ad0d1e2901f0045cf3ca7f4cbeca')
        self.assert_source(0x7FCAF3, 'c2202020c7952be2204c757e')
        self.assert_source(0x7FCABE, 'e220c220b52b18690300952be2204c757e')
        self.assertEqual(self.rom[0x478F2:0x478F5], bytes.fromhex('30538d'))
        self.assertEqual(self.rom[0x47904:0x47907], bytes.fromhex('30538d'))


if __name__ == '__main__':
    unittest.main()
