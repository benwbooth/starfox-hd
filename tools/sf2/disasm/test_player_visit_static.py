"""Ordered player prefix and its canonical published-motion consumers."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerVisitStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_raw_pilot_code_clamps_only_table_lookup_and_limits_use_distinct_tables(self):
        self.assert_source(0x0690D4,
            "08 C2 30 5A B4 2B B9 FF 6B 29 FF 00 C9 06 00 90 03 A9 00 00 7A AA 28 6B")
        self.assert_source(0x06A46E, "20 20 28 28 18 18 19 19 23 23 0A 0A")
        # Warning scan tests the raw pilot pair, not the clamped index.
        self.assert_source(0x06A676,
            "B9 FF 6B 29 FE C9 02 D0 0B B9 77 6B 89 40 D0 04 5C 34 A8 06")

    def test_limits_and_signed_clamp_precede_warning_then_selective_flag_and_motion_clear(self):
        self.assert_source(0x069C27,
            "B4 2B E2 20 DA 22 D4 90 06 BF 74 A4 06 8D D6 1D FA DA 22 D4 90 06 BF 6E A4 06 "
            "8D D5 1D FA AD D5 1D D9 00 6C 10 03 99 00 6C 22 47 A6 06 E2 20 C2 10 B5 25 29 DF "
            "95 25 C2 20 9C 1C 1E 9C 1E 1E 9C 20 1E E2 20")

    def test_low_shield_cue_only_on_arrival_at_ten_with_two_signed_compares_and_caller_side(self):
        self.assert_source(0x069C6B,
            "B9 E8 6B C9 0A F0 3B 1A 99 E8 6B C9 0A D0 33 B9 00 6C C9 0D 10 2C C9 05 30 15 "
            "C2 20 A9 16 00 EC C3 12 F0 03 09 00 80 22 09 6E 7F E2 20 80 13 C2 20 A9 17 00 "
            "EC C3 12 F0 03 09 00 80 22 09 6E 7F E2 20")

    def test_shot_reset_and_parallel_action_precede_all_equipment_publications(self):
        self.assert_source(0x069CAD,
            "C2 20 AD 84 1B 89 02 00 E2 20 D0 03 4C C1 9C A9 00 99 03 6C B4 2B B4 2B E2 20 "
            "22 CF BC 0D B4 2B E2 20 B9 00 6C 8D D1 1D B9 05 6C 8D D3 1D B9 04 6C 8D D2 1D "
            "B9 06 6C 8D D4 1D")

    def test_motion_publication_precedes_saturating_extension_word_then_separate_mode_dispatch(self):
        self.assert_source(0x069CE7,
            "22 15 EA 07 C2 20 BD E4 1C C9 FF FF E2 20 D0 04 5C 09 9D 06 C2 20 BD E4 1C 18 "
            "69 01 00 9D E4 1C E2 20")
        self.assert_source(0x069D09, "C2 20 A9 1A 9D 85 54 E2 20 A9 06 85 56 5C 82 32 7F")
        self.assert_source(0x7F3282, "8C CA 16 BD C7 1C C2 20 29 FF 00 0A 0A A8 E2 20")

    def test_following_uses_selected_suppression_but_shared_delta_not_selected_live_motion(self):
        self.assert_source(0x7F9F16,
            "B5 21 29 08 D0 04 5C 4E 9F 7F AC 1F CF DA BB 7A 5A B4 2B B9 77 6B 7A DA BB 7A "
            "89 04 F0 04 5C 42 9F 7F C2 20 AD 1C 1E 18 75 0C 95 0C C2 20 AD 20 1E 18 75 10 "
            "95 10 E2 20 60")


if __name__ == "__main__":
    unittest.main()
