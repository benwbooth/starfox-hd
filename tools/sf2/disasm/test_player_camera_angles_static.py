"""Camera pitch and publication share retained auxiliary and view fields."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerCameraAnglesStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_pitch_uses_shared_linked_mode_and_clears_nonlinked_increment_first(self):
        self.source(0x0786C0, "b9636b8940f0045cd386072980f0045ca78707")
        self.source(0x0786DF, "a90000993f6be220")
        self.source(0x0786FD, "b9fd6bc9806a8daf1d")

    def test_pitch_candidate_sign_selects_unsigned_target_comparison(self):
        self.source(0x078880, "c220b9316b18793f6b85023007cdae1d901e8005cdae1db017")
        self.source(0x07889D, "b9316b853aadae1d22a3257f99316b")

    def test_pose_publishes_full_roll_before_position_and_resets_view_rear_distance(self):
        self.source(0x0796CC, "b9356b95168d0b1ee2208000287afa60")
        self.source(0x0796E2, "a03f03c220b916008d0b1e287a60")
        self.source(0x0796FA, "c220a90000992900e2205ab42bbb7ac220bdc16a990c00bdc36a990e00bdc56a991000287afa60")


if __name__ == "__main__":
    unittest.main()
