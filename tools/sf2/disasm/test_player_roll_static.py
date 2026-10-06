"""Shoulder arbitration and the complete shared barrel-roll controller."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerRollStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_right_edge_precedes_left_and_held_both_uses_retained_preference(self):
        self.assert_source(0x069075,
            "5A B4 2B AD 36 19 89 10 F0 07 B9 7E 6B 09 80 80 0C AD 36 19 89 20 F0 08 "
            "B9 7E 6B 29 7F 99 7E 6B B9 7E 6B 29 9F 99 7E 6B AD 38 19 29 30 F0 2E "
            "C9 30 F0 06 89 20 D0 10 80 1C B9 7E 6B 89 80 D0 0E")

    def test_protection_precedes_active_branch_and_action_latch_clear(self):
        self.assert_source(0x06E7FF,
            "5A 08 E2 20 C2 10 B4 2B B9 DD 6A F0 0A B9 02 6C 09 40 99 02 6C 80 08 "
            "B9 02 6C 29 BF 99 02 6C B9 DD 6A C9 00 F0 04 5C C9 E8 06 "
            "B9 77 6B 29 EF 99 77 6B AD 36 19 29 30")

    def test_tap_match_window_and_impulse_direction_use_distinct_inputs(self):
        self.assert_source(0x06E847,
            "AD 38 19 29 30 C9 30 D0 04 5C B6 E8 06 B9 DC 6A 29 30 C5 3A F0 04 "
            "5C 99 E8 06 B9 DC 6A 29 0F C9 07 90 04 5C 99 E8 06 B9 7E 6B 89 40 "
            "F0 04 5C 80 E8 06 A9 E0 99 DD 6A 80 05 A9 20 99 DD 6A")

    def test_active_decay_wraps_and_expires_tap_window_without_clearing_action_latch(self):
        self.assert_source(0x06E8C9,
            "E2 20 B9 DD 6A 30 04 5C DF E8 06 B9 DD 6A 18 69 02 99 DD 6A 80 09 "
            "B9 DD 6A 38 E9 02 99 DD 6A B9 DC 6A 29 F0 09 0F 99 DC 6A 28 7A 60")

    def test_flight_calls_arbitration_before_control_and_roll_before_ambient_pose(self):
        self.assert_source(0x06872C, "22 57 94 06 22 75 90 06")
        self.assert_source(0x06875F, "22 2C E3 06 20 26 9A")
        self.assert_source(0x06E374, "20 F6 E9 20 58 E2")
        self.assert_source(0x06E258,
            "08 E2 20 C2 10 20 FF E7 20 F7 F2 20 10 F0 22 6F DD 07")

    def test_walker_steering_consumes_the_same_prepared_shoulder_choice(self):
        self.assert_source(0x068AAC, "22 75 90 06 22 F1 81 07 22 05 AC 06")
        self.assert_source(0x06AC80, "20 82 B4")
        self.assert_source(0x06B4D2,
            "B9 7E 6B 89 40 F0 04 5C E8 B4 06 89 20 F0 04 5C 27 B5 06 4C 6D B5")


if __name__ == "__main__":
    unittest.main()
