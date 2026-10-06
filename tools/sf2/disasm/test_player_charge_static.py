#!/usr/bin/env python3
"""Charged-fire source contracts. Expectations are original bytes, not replay output."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerChargeStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_contact_and_action_gates_then_processed_held_and_pressed_bits(self):
        self.assert_source(0x07DABB, "B4 2B B9 72 6A 7A 29 10 D0 04 5C D3 DA 07 B9 09 6C 09 40 29 7F 99 09 6C AD 72 1D F0 04 5C 7A DB 07")
        self.assert_source(0x07DADC, "AD 39 19 89 80 F0 11 AD 37 19 89 80 D0 0A B9 09 6C 09 80 99 09 6C 80 08 B9 09 6C 29 EF 99 09 6C")
        self.assert_source(0x07DAFC, "B9 09 6C 89 10 F0 04 5C 84 DB 07 AD 37 19 89 80 D0 0B AD 39 19 89 80 F0 04 5C 84 DB 07 B9 08 6C CD D6 1D D0 59")

    def test_launch_reset_selector_and_effects_are_independent_of_allocation_success(self):
        self.assert_source(0x07DB21, "5A A9 00 8D B7 14 A9 00 8D B6 14 9C B8 14 9C B9 14 A9 00 8D B0 14 A9 00 8D B2 14 A9 00 8D B4 14 A9 10 22 9C A8 03 C0 00 00 D0 04 5C 50 DB 07 7A")
        self.assert_source(0x07DB51, "B9 09 6C 09 10 99 09 6C A9 05 99 60 6B B9 A0 6A 29 F0 C9 20 D0 04 5C 7A DB 07 A9 05 99 58 6B C2 20 A9 1E 00 99 56 6B E2 20 B9 09 6C 09 40 29 7F 99 09 6C")

    def test_decay_stops_sound_marks_direct_child_and_halves_signed_level(self):
        self.assert_source(0x07DB93, "29 DF 99 09 6C C2 20 A9 F4 00 EC C3 12 F0 03 09 00 80 22 09 6E 7F E2 20")
        self.assert_source(0x07DBAB, "5A E2 20 A9 27 22 7B 2A 7F C0 00 00 D0 04 5C CB DB 07 A9 27 22 7B 2A 7F B9 25 00 09 08 99 25 00 7A B4 2B A9 00 99 07 6C B9 08 6C C9 80 6A 10 02 69 00 99 08 6C B9 08 6C D0 0B B9 09 6C 29 BF 99 09 6C 4C 7C DC")

    def test_effect_threshold_precedes_charge_increment_and_sound_latch_is_independent(self):
        self.assert_source(0x07DBF0, "B9 09 6C 89 40 F0 04 5C 7C DC 07 89 80 D0 04 5C 7C DC 07 B9 08 6C C9 08 90 23 22 C5 CD 07 B9 09 6C 89 20 D0 18 09 20 99 09 6C C2 20 A9 31 00 EC C3 12 F0 03 09 00 80 22 09 6E 7F E2 20")

    def test_fine_word_add_wraps_before_unsigned_clamp_and_all_pilot_rows_match(self):
        self.assert_source(0x07DC2D, "B9 08 6C CD D6 1D F0 47 DA 22 EC 90 06 C2 20 BF 7F DC 07 85 02 E2 20 FA C2 20 AD D6 1D 29 FF 00 EB 85 3A A5 02 18 79 07 6C C5 3A 90 1D")
        self.assert_source(0x07DC5A, "E2 20 C2 20 A9 35 00 EC C3 12 F0 03 09 00 80 22 09 6E 7F E2 20 C2 20 AD D5 1D 29 00 FF 99 07 6C E2 20 28 7A 6B")
        self.assert_source(0x07DC7F, "80 01 " * 6)

    def test_effect_installer_whole_source_preserves_allocation_and_mode_read_order(self):
        self.assert_source(0x07CDC5, "5A 08 E2 20 C2 10 A9 27 22 7B 2A 7F C0 00 00 F0 04 5C 2E CE 07 C2 20 A9 9C BC 85 5F E2 20 22 17 2A 7F B0 04 5C 2E CE 07 A9 27 22 3D 2A 7F C2 20 A9 1E 7E 99 19 00 E2 20 A9 7F 99 1B 00 5A B4 2B B9 A0 6A 7A 29 F0 C9 10 D0 04 5C 17 CE 07 A9 14 80 02 A9 46 99 E4 1C A9 00 99 E5 1C C2 20 A9 4F F0 99 2B 00 E2 20 20 F5 BE 28 7A 6B")
        self.assert_source(0x07BEF5, "08 E2 20 C2 10 C2 20 A9 1E 7E 99 19 00 E2 20 A9 7F 99 1B 00 A9 FF 99 F0 1C A9 01 99 2D 00 A9 01 99 2E 00 B9 22 00 09 04 99 22 00 22 AA 2B 7F 22 D2 2B 7F B9 26 00 09 08 99 26 00 B9 31 00 29 EF 99 31 00 B9 21 00 09 01 99 21 00 28 60")

    def test_input_producer_computes_pressed_edges_before_processed_player_publication(self):
        self.assert_source(0x7F0E50, "C2 20 AD 18 42 48 4D 92 12 2D 18 42 8D 96 12 AD 1A 42 48 4D 94 12 2D 1A 42 8D 98 12 68 8D 94 12 68 8D 92 12 28 60")
        self.assert_source(0x06948D, "C2 20 AD 96 12 AC 92 12 80 08 C2 20 AD 98 12 AC 94 12 8D 36 19 8C 38 19 E2 20")


if __name__ == "__main__":
    unittest.main()
