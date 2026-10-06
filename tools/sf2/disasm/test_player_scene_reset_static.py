"""Shared reset source ownership and deliberately preserved neighboring state."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerSceneResetStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_controller_selection_precedes_clear_and_wrapped_shield_clamp(self):
        self.assert_source(0x069593,
            "5A B5 23 29 40 F0 04 5C A8 95 06 C2 20 AD 96 12 AC 92 12 80 08 "
            "C2 20 AD 98 12 AC 94 12 8D 36 19 8C 38 19 E2 20 7A C2 20 A9 00 00 "
            "8D 38 19 8D 36 19 E2 20 A9 00 8D E2 1D 9C CE 1D 9C DE 1D 9C DF 1D "
            "AD D5 1D CD D1 1D 10 03 8D D1 1D 9C F2 1D")

    def test_control_reset_changes_handoff_flags_but_not_handoff_coordinates(self):
        self.assert_source(0x0695EE,
            "9C E1 1D 9C 08 1E 9C 4D 1B 9C 2F 1E 9C D5 D7 9C 99 1D 9C 72 1D "
            "A9 FE 8D 73 1D 8D 75 1D 8D 76 1D 9C 74 1D 9C 7A 1D A0 00 00 8C 7B 1D")

    def test_reticle_horizontal_repeats_and_focus_and_motion_reset_are_distinct(self):
        self.assert_source(0x069626,
            "9C 1B 1E 9C 1B 1E 9C 0D 1E 9C 0E 1E 9C E5 1C 9C E6 1C "
            "A9 64 8D 30 1E 8D 30 1E C2 20 "
            "9C FF 1D 9C 01 1E 9C 03 1E 9C 05 1E 9C 92 1D 9C 94 1D 9C 96 1D "
            "9C 18 1E 9C 9B 1D 9C EC D7 9C EE D7 9C F0 D7 9C 1C 1E 9C 1E 1E 9C 20 1E "
            "9C 4E 1E 9C 52 1E 9C 3C 1E 9C 40 1E 9C 48 1E 9C 0B 1E 9C 0F 1E")

    def test_shared_reset_follows_storage_copy_and_precedes_target_initialization(self):
        self.assert_source(0x068298, "AD D1 1D 99 00 6C AD D1 1D 99 38 6C")
        self.assert_source(0x068313,
            "20 2C 83 22 8C 95 06 22 CE B0 07 C2 20 A9 5C 84 95 19 E2 20 A9 06 95 1B 6B")
        self.assert_source(0x06835D, "7A 20 60 82 60")

    def test_radio_placement_reads_the_vertical_reticle_publication(self):
        self.assert_source(0x0ACF1A, "AF 75 D7 7E F0 06 A9 8B 8F 44 D7 7E")
        self.assert_source(0x0ACF26,
            "AF 31 1E 00 C9 92 30 0A A9 23 8F 44 D7 7E A9 01 80 02 A9 00 "
            "8F 59 D7 7E A9 00 8F 45 D7 7E 28 6B")


if __name__ == "__main__":
    unittest.main()
