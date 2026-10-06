"""Vertical configuration, input-history, terrain and caller source contracts."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerVerticalStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_map_configuration_selects_primary_and_copies_full_offsets_and_pitch_bytes(self):
        self.assert_source(0x069A3B, "AE C3 12 20 44 9A 4C 22 9C")
        self.assert_source(0x069A44,
            "B4 2B C2 20 A5 08 99 F5 6B A5 0A 99 F7 6B E2 20 "
            "A5 02 99 F9 6B A5 04 99 FA 6B 60")
        # These are map records, not CPU instructions: word/byte setters
        # supply the actual parameters to the following inline map call.
        self.assert_source(0x05819D,
            "5E 58 02 08 00 00 5E 0C 00 0A 00 00 5C 0F 02 00 00 "
            "5C F1 04 00 00 78 22 2F 9A 06 A2 BC 01 6B")

    def test_history_is_full_word_xor_or_and_before_the_prior_held_store(self):
        self.assert_source(0x06F2D9,
            "5A 08 C2 30 B4 2B B9 89 6B 4D 38 19 19 87 6B 2D 38 19 "
            "99 87 6B AD 38 19 99 89 6B 28 7A 6B")

    def test_flight_pitch_uses_retained_down_then_the_shared_contact_plane(self):
        self.assert_source(0x06E440, "5A B4 2B B9 88 6B 7A 89 04")
        self.assert_source(0x06E47E,
            "B9 7D 6B 89 80 D0 04 5C AE E4 06 C2 20 A9 CE FF 18 6D 0F 1E")
        self.assert_source(0x06E496,
            "C2 20 B5 0E C5 3C E2 20 10 04 5C AE E4 06 C2 20 A9 00 C0 8D 36 1E")

    def test_held_pitch_checks_activity_then_keeps_fine_pitch_when_neutral(self):
        self.assert_source(0x06E3A4,
            "A9 00 00 8D 36 1E E2 20 B9 77 6B 89 01 D0 04 5C F4 E3 06")
        self.assert_source(0x06E3EA, "C2 20 B9 B9 6A 8D 36 1E E2 20 28 7A 60")

    def test_callers_keep_history_before_steering_and_correct_pitch_limit_pair(self):
        self.assert_source(0x06E2F6, "22 D9 F2 06")
        self.assert_source(0x06E30B, "20 B1 E4 20 9A E3 20 10 EA 20 58 E2")
        self.assert_source(0x06E347, "22 D9 F2 06")
        self.assert_source(0x06E36E, "20 B1 E4 20 F7 E3 20 F6 E9 20 58 E2")

    def test_terrain_response_clamps_unsigned_distance_and_uses_signed_full_multiply(self):
        self.assert_source(0x06EBC7,
            "C2 20 A5 08 29 80 FF E2 20 D0 04 5C DF EB 06 C2 20 A9 7F 00 85 08")
        self.assert_source(0x06EBDF,
            "DA A6 08 C2 20 BF 66 8E 00 89 80 00 F0 05 09 00 FF 80 03 "
            "29 FF 00 85 02 E2 20 FA 22 FC E0 07 60")
        self.assert_source(0x06EB99, "20 C7 EB A5 09 0A 99 D7 6A 60")

    def test_hard_lower_includes_zero_and_halves_arithmetically_before_neutral_recovery(self):
        self.assert_source(0x06EC5C, "AD 36 1E 30 4D")
        self.assert_source(0x06EC6D, "AD 36 1E C9 00 80 6A 8D 36 1E")
        self.assert_source(0x06EA27,
            "C2 20 AD 38 19 89 00 0C E2 20 D0 0E B9 CF 6A 85 3A "
            "A9 00 22 82 27 7F 99 CF 6A")


if __name__ == "__main__":
    unittest.main()
