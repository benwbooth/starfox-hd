"""Original horizontal-control boundaries, pilot tables and call order."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerSteeringStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_full_horizontal_entry_calls_lean_before_direction_edges(self):
        self.assert_source(0x06E4B1,
            "5A 08 E2 20 C2 10 B4 2B 22 F5 E8 06 C2 20 A9 00 03 2C 36 19")
        self.assert_source(0x06E4FA, "DA BB 7A FE 17 6B DA BB 7A A9 FA 99 C0 6A")

    def test_neutral_lean_zeroes_response_but_does_not_store_lean(self):
        self.assert_source(0x06E974,
            "C2 20 A9 00 00 85 08 E2 20 C2 20 A9 00 00 85 0A E2 20 "
            "C2 20 A9 00 00 99 D2 6A E2 20 4C E7 E9")

    def test_response_is_initialized_only_on_normal_unlocked_branch(self):
        self.assert_source(0x06E919,
            "C2 20 A9 80 02 85 0A E2 20 DA 22 D4 90 06 BF EA E9 06 85 09 FA 80 12")
        self.assert_source(0x06E930,
            "DA 22 D4 90 06 BF F0 E9 06 85 09 FA 80 04 A9 08 85 09")
        self.assert_source(0x06E9A0, "B9 D2 6A 85 3A A5 0A 22 67 25 7F 99 D2 6A")

    def test_all_pilot_parameters_and_fallback_are_source_bound(self):
        self.assert_source(0x0690DA, "B9 FF 6B 29 FF 00 C9 06 00 90 03 A9 00 00")
        self.assert_source(0x06E7D5, "00 02 90 01 20 02 20 02 80 01 80 01")
        self.assert_source(0x06E7E1, "00 03 90 02 20 03 20 03 00 03 80 02")
        self.assert_source(0x06E7ED, "C0 03 " * 6)
        self.assert_source(0x06E7F9, "28 " * 6)
        self.assert_source(0x06E9EA, "18 10 08 08 18 18" + " 18" * 6)

    def test_contact_ignore_runs_both_shoulder_bank_chases(self):
        self.assert_source(0x06E6A0,
            "C2 20 B9 D8 6A 85 3A A9 00 00 22 34 25 7F 99 D8 6A E2 20 "
            "C2 20 B9 D8 6A 85 3A A9 00 00 22 A3 25 7F 99 D8 6A")

    def test_heading_and_lateral_consumers_preserve_widths(self):
        self.assert_source(0x06E795, "B9 DA 6A 85 3A A5 97 22 82 27 7F 99 DA 6A")
        self.assert_source(0x06E7A3, "C2 20 B9 BB 6A 85 3A A5 08 22 34 25 7F 99 BB 6A")
        self.assert_source(0x06E73F, "B9 D1 6A 0A 0A 85 02")
        self.assert_source(0x06E769, "22 DA E0 07 C2 20 A5 09 99 15 6B E2 20 28 7A 60")

    def test_turn_adjustment_is_a_camera_bank_target_not_player_speed(self):
        self.assert_source(0x07867C,
            "B9 C0 6A 8D AF 1D A9 00 8D AE 1D C2 20 B9 35 6B CD AE 1D")
        self.assert_source(0x0796CA, "C2 20 B9 35 6B 95 16 8D 0B 1E")


if __name__ == "__main__":
    unittest.main()
