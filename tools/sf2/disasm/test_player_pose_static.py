"""Source boundaries and data owners of the full flight pose composer."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerPoseStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_trim_precedes_mode_specific_fine_pitch_chase(self):
        self.assert_source(0x06ECB0,
            "DA 5A 08 E2 20 C2 10 B4 2B B9 D4 6A 85 3A A9 00 22 82 27 7F 99 D4 6A "
            "5A B4 2B B9 A0 6A 7A 29 0F C9 01")
        self.assert_source(0x06ECDA, "B9 B9 6A 85 3A AD 36 1E 22 A3 25 7F 99 B9 6A")
        self.assert_source(0x06ECEF, "B9 B9 6A 85 3A AD 36 1E 22 67 25 7F 99 B9 6A")

    def test_linked_pitch_uses_retained_rotation_and_arithmetic_not_truncated_halves(self):
        self.assert_source(0x06ED00,
            "B9 77 6B 89 01 D0 04 5C 31 ED 06 B9 63 6B 29 80 D0 04 5C 31 ED 06 "
            "B9 32 6B 95 12 B5 12 49 FF 1A 95 12 B5 12 C9 80 6A C9 80 6A 18 75 12 95 12")

    def test_unlocked_yaw_publishes_motion_before_integrating_fine_heading(self):
        self.assert_source(0x06ED61,
            "C2 20 AD 38 1E 99 CD 6A E2 20 C2 20 B9 BB 6A 18 6D 38 1E 99 BB 6A E2 20")
        self.assert_source(0x06ED8E, "B5 14 18 79 DE 6A 95 14")

    def test_roll_gate_decays_visible_angle_without_overwriting_retained_bank(self):
        self.assert_source(0x06ED96,
            "5A B4 2B B9 72 6A 7A 29 10 F0 04 5C B6 ED 06 5A B4 2B B9 A0 6A 7A "
            "29 F0 C9 10 D0 04 5C C3 ED 06 B5 16 C9 80 6A 10 02 69 00 95 16 80 43")

    def test_bank_recovery_and_all_four_additive_roll_terms_are_included(self):
        self.assert_source(0x06EDCE,
            "B9 BD 6A C9 80 6A 10 02 69 00 85 3A C9 80 6A 10 02 69 00 18 65 3A 99 BD 6A")
        self.assert_source(0x06EDE7,
            "B9 BD 6A 18 79 DD 6A 99 BD 6A B9 BD 6A 18 79 D5 6A 18 79 D8 6A "
            "18 79 D7 6A 18 79 DA 6A 95 16 28 7A FA 60")

    def test_caller_orders_pose_after_roll_ambient_and_speed_before_motion(self):
        self.assert_source(0x06E25D,
            "20 FF E7 20 F7 F2 20 10 F0 22 6F DD 07 20 5D F0 20 B0 EC 20 0A EE")


if __name__ == "__main__":
    unittest.main()
