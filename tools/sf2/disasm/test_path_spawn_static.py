#!/usr/bin/env python3
"""Child spawn operand contracts, checked against static source bytes only."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathSpawnStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_compact_zero_rotation_and_extended_literal_rotation_position_number(self):
        self.assert_source(0x7F9042, "AD 11 19 8D BB 16 C2 20 20 20 C7 8D B7 16 AD BB 16 C9 33 00 F0 27 9C BD 16 9C BF 16 9C C1 16 C2 20 20 28 C8 8D B1 16 20 80 C8 8D B3 16 20 D8 C8 8D B5 16 E2 20 20 6C C6 8D B9 16 80 30 E2 20 20 4C C5 8D BD 16 20 70 C5 8D BF 16 20 94 C5 8D C1 16 C2 20 20 AC C8 8D B1 16 20 04 C9 8D B3 16 20 5C C9 8D B5 16 E2 20 20 D8 C6 8D B9 16")

    def test_byte_readers_are_literal_source_offsets_not_encoded_actor_variables(self):
        for address, offset in ((0xC54C, 5), (0xC570, 6), (0xC594, 7),
                                (0xC5B8, 8), (0xC5DC, 9), (0xC66C, 13),
                                (0xC6D8, 16)):
            with self.subTest(address=address):
                self.assert_source(0x7F0000 | address,
                    f"84 79 A0 {offset:02X} 00 A5 5E 29 EF 85 5E 8F 3A 30 00 B7 F9 8D 11 19 A5 5E 09 10 85 5E 8F 3A 30 00 AD 11 19 A4 79 60")

    def test_word_readers_load_exact_offsets_at_word_width(self):
        for address, offset in ((0xC720, 1), (0xC778, 3), (0xC828, 7),
                                (0xC880, 9), (0xC8D8, 11), (0xC8AC, 10),
                                (0xC904, 12), (0xC95C, 14)):
            with self.subTest(address=address):
                self.assert_source(0x7F0000 | address,
                    f"84 79 A0 {offset:02X} 00 E2 20 A5 5E 29 EF 85 5E 8F 3A 30 00 C2 20 B7 F9 8D 11 19 E2 20 A5 5E 09 10 85 5E 8F 3A 30 00 C2 20 AD 11 19 A4 79 60")

    def test_health_attack_offsets_and_inherited_target_group_precede_child_path(self):
        self.assert_source(0x7F9141, "AD BB 16 C9 33 F0 0E 20 4C C5 99 2D 00 20 70 C5 99 2E 00 80 0C 20 B8 C5 99 2D 00 20 DC C5 99 2E 00 B5 24 29 80 D0 04 5C 74 91 7F B9 24 00 09 80 99 24 00 BD F0 1C 99 F0 1C C2 20 20 78 C7 99 2B 00")

    def test_record_advances_are_17_14_and_7_bytes(self):
        for address, count in ((0x7FCA0C, 17), (0x7FCA2E, 14), (0x7FCA72, 7)):
            self.assert_source(address,
                f"E2 20 C2 20 B5 2B 18 69 {count:02X} 00 95 2B E2 20 4C 75 7E")


if __name__ == "__main__":
    unittest.main()
