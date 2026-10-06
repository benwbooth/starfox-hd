"""Reticle preparation ownership and complete host projection boundaries."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerReticleProducerStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_preparation_is_scheduled_by_each_player_mode_not_display_retention(self):
        self.assert_source(0x068799, "22 38 B0 07 22 CC D6 07 22 B2 DA 07")
        self.assert_source(0x068AC3, "22 38 B0 07 22 5B AF 07")
        self.assert_source(0x068E16, "22 38 B0 07 20 EF 96 22 B2 DA 07")
        self.assert_source(0x06F6A3, "22 38 B0 07 22 E2 8F 06")

    def test_preparation_clears_before_lazy_gates_and_only_linked_mode_copies_roll(self):
        self.assert_source(0x07B03F,
            "B4 2B A9 00 99 A4 6B C2 20 AD 96 1B 89 00 01 E2 20 D0 4F "
            "C2 20 AD 84 1B 89 02 00 E2 20 F0 03 4C A1 B0 AD F4 D7 D0 04 "
            "5C A1 B0 07 AD 0D 1E 29 01 D0 30 AD 2F 1E 89 80 D0 04 "
            "5C A1 B0 07 B9 7D 6B 89 80 F0 04 5C A1 B0 07")
        self.assert_source(0x07B087,
            "B9 63 6B 89 40 F0 04 5C 9A B0 07 29 80 F0 04 5C A8 B0 07 "
            "A9 40 99 A3 6B 80 16 A9 00 99 A3 6B 80 0F A9 C0 99 A3 6B "
            "A9 00 99 A4 6B B5 16 99 A2 6B AD A6 1A 89 02 F0 04 "
            "5C CA B0 07 B9 A4 6B 09 04 99 A4 6B 28 7A FA 6B")

    def test_inactive_position_resets_both_axes_and_active_copies_actual_player_rotation(self):
        self.assert_source(0x07A418,
            "AD 2F 1E 89 80 D0 04 5C 2E A4 07 B9 A3 6B 89 40 F0 04 "
            "5C 39 A4 07 A9 64 8D 30 1E 8D 31 1E 4C 05 A5 "
            "A0 3F 03 8C B0 1D AC D6 14 22 D2 2B 7F")
        self.assert_source(0x7F2BD2, "B5 12 99 12 00 B5 14 99 14 00 B5 16 99 16 00 6B")
        self.assert_source(0x07A44E,
            "BD 9C 6B 99 0C 00 BD 9E 6B 99 0E 00 BD A0 6B 99 10 00 "
            "E2 20 FA DA BB AC B0 1D 20 E8 AF FA A9 08 8D AE 1D")

    def test_projection_uses_saved_origin_and_publishes_angles_without_rebuilding_matrix(self):
        self.assert_source(0x07AFE8,
            "08 C2 30 B9 39 00 85 C7 B9 3B 00 85 C9 B9 3D 00 80 10")
        self.assert_source(0x07B00A,
            "85 CB B9 12 00 8D 95 15 B9 14 00 8D 97 15 B9 16 00 8D 99 15 "
            "E2 20 A5 5E 48 22 FE 77 7F 22 7E 8E 03 68 85 5E 8F 3A 30 00 "
            "22 4B 78 7F 28 60")
        self.assert_source(0x038E95,
            "A9 E2 85 97 B5 12 22 4E 3A 7F A5 04 85 02 A5 0A 85 08 "
            "A5 E4 85 97 B5 14 22 A9 38 7F")
        self.assert_source(0x038F00, "A9 01 A2 3A 91 22 7B 78 7F")
        self.assert_source(0x038F25, "A9 01 A2 0A D5 22 7B 78 7F")


if __name__ == "__main__":
    unittest.main()
