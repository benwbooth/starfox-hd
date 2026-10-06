"""Common player formatter, its caller boundaries and distinct flag owners."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerFormatStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_complete_formatter_uses_manual_shape_frame_zero_and_selective_flags(self):
        self.assert_source(0x0682B7,
            "8E 0D 15 8E C1 12 8E 09 15 A9 00 09 80 9D CB 1C A9 00 95 2E "
            "B5 09 29 F7 95 09 B5 22 09 20 95 22 B5 20 09 08 95 20 "
            "B5 24 09 04 95 24 B5 26 09 10 95 26 60")

    def test_storage_tail_enters_formatter_before_enclosing_reset_and_target_reset(self):
        self.assert_source(0x0682A4,
            "C2 20 AD 16 D8 99 33 6C AD 17 D8 99 34 6C E2 20 4C B7 82")
        self.assert_source(0x068313,
            "20 2C 83 22 8C 95 06 22 CE B0 07 C2 20 A9 5C 84 95 19 E2 20 A9 06 95 1B 6B")
        self.assert_source(0x06835D, "7A 20 60 82 60")

    def test_pathhold_and_visibility_are_separate_controls(self):
        self.assert_source(0x7F8ECF, "B5 09 09 08 95 09")
        self.assert_source(0x7F979A,
            "B5 23 29 FD 95 23 B5 21 29 FE 95 21 4C E8 CA")


if __name__ == "__main__":
    unittest.main()
