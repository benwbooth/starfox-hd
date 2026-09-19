#!/usr/bin/env python3
"""Assembly-byte checks for player contact leaves, without recordings."""

from pathlib import Path
import re
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerHitControlStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_recovery_clock_controls_marking_not_countdown(self):
        self.assert_source(0x0691B4, "B9 E3 6B 29 3F F0 31 A5 C4 29 03 D0 04 5C CB 91 06 B5 20 09 02 95 20")
        self.assert_source(0x0691CB, "B9 E3 6B 48 29 C0 99 E3 6B 68 3A 19 E3 6B 99 E3 6B F0 0E B5 22 09 08 95 22 B5 21 09 01 95 21 80 0C")
        self.assert_source(0x0691EC, "B5 22 29 F7 95 22 B5 21 29 FE 95 21")

    def test_secondary_protection_pause_and_hold_only_suppress_response(self):
        self.assert_source(0x0691F8, "C2 20 AD 84 1B 89 02 00 E2 20 F0 03 4C 2D 92")
        self.assert_source(0x069207, "B9 E2 6B 29 3F F0 0F 3A 85 3A B9 E2 6B 29 C0 05 3A 99 E2 6B 80 10")
        self.assert_source(0x06921D, "B9 7D 6B 89 80 D0 04 5C 33 92 06 A9 3F 99 E2 6B B5 22 09 08 95 22")

    def test_light_and_heavy_impact_profiles_and_shared_clock_bank(self):
        self.assert_source(0x06AA73, "B9 E3 6B C9 00 F0 04 5C 02 AB 06 A9 04 29 3F 99 E3 6B A9 02 99 11 6C B9 12 6C 09 70 99 12 6C")
        self.assert_source(0x06AAA2, "C2 20 A9 60 00 99 3B 6B E2 20 80 32")
        self.assert_source(0x06AAB2, "A9 0A 29 3F 99 E3 6B A9 08 99 11 6C B9 12 6C 09 70 99 12 6C")
        self.assert_source(0x06AAC6, "C2 20 B9 3B 6B C9 00 00 E2 20 F0 04 5C E0 AA 06 C2 20 A9 80 00 99 3B 6B E2 20")
        self.assert_source(0x06AAE0, "B9 09 6C 09 40 29 7F 99 09 6C A9 1E 99 DA 6A A5 C4 29 01 D0 04 5C 02 AB 06 B9 DA 6A 49 FF 1A 99 DA 6A")

    def test_reserve_tail_clears_damage_without_spill_and_sound_tests_signed_difference(self):
        self.assert_source(0x069824, "B4 2B B9 00 6C F0 13 38 ED 2F CF 99 00 6C 10 05 A9 00 99 00 6C A9 00 8D 2F CF")
        self.assert_source(0x0697F1, "AD 2F CF C9 04 10 04 5C 11 98 06")

    def test_deflection_queues_sound_before_advancing_random_state(self):
        self.assert_source(0x069783, "5A B4 2B A9 01 99 11 6C B9 12 6C 09 08 99 12 6C 7A")
        self.assert_source(0x06AB09, "B9 E7 6B D0 1C C2 20 A9 18 00 EC C3 12 F0 03 09 00 80 22 09 6E 7F E2 20 22 D0 7B 7F 29 07 99 E7 6B")

    def test_turn_uses_high_byte_negation_and_signed_yaw_difference(self):
        self.assert_source(0x06986C, "5A 22 88 21 7F E2 20 EB 49 FF 1A DD E2 1C F0 00 9D E2 1C B5 14 DD E2 1C 30 04 A9 40 80 02 A9 C0")
        self.assert_source(0x06988F, "B4 2B 99 D4 6A C9 80 6A 10 02 69 00 99 AD 6A")
        self.assert_source(0x7F218A, "C2 20 B9 0C 00 38 F5 0C 85 02 B9 10 00 38 F5 10 85 08 22 58 1D 7F")

    def test_reused_native_arctangent_curve_matches_source_table(self):
        root = Path(__file__).resolve().parents[3]
        source = (root / "rust/sf-core/src/aim_angle.rs").read_text()
        body = re.search(r"static ARCTANGENT_CURVE: \[u16; 256\] = \[(.*?)\];", source, re.S).group(1)
        curve = [int(value.replace("_", "")) for value in re.findall(r"[0-9][0-9_]*", body)]
        start = source_offset(0x0DFC74)
        actual = [int.from_bytes(self.rom[start + i * 2:start + i * 2 + 2], "little") for i in range(256)]
        self.assertEqual(curve, actual)


if __name__ == "__main__":
    unittest.main()
