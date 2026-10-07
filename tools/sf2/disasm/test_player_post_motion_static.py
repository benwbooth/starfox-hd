"""Exact continuous post-mode caller, state producers and reset boundaries."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerPostMotionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_cooldown_wrap_then_first_direct_child_pose_from_fine_heading(self):
        self.source(0x069DBA, 'e220c210b42bb9e76bf0063a290799e76bb42bb9bc6a8508a916227b2a7fc00000f01122aa2b7fa900991200991600a508991400')

    def test_location_specific_progress_request_precedes_position_history(self):
        self.source(0x069DEE, 'b42bade21dc909d02ec220adb51bc90b00e220d009ad87d7c9fff00f8019ad87d7c9fef006c9ffd00e800cb9e96b8980d00209e099e96bc230b50c99c76ab50e99c96ab51099cb6ae220')

    def test_music_gates_precede_per_player_latch_and_shared_request(self):
        self.source(0x069E38, 'adf4d7f023c220ad8a1b892000e220f017adde1df012b9716a8920d00b092099716aa90122f86d7f')

    def test_contact_edge_walker_bypass_sided_cue_recoil_and_impulse(self):
        self.source(0x069E60, 'b9e66b8910f07c29ef99e66b8908d078090899e66b5ab42bb9a06a7a29f0c920d0045ce89e06c220a9a700ecc312f00309008022096e7fe220a90299116cb9126c096899126cc220b93b6bc90000e220f0045cc09e06c220a98000993b6be220b9776b8920f0045cd79e06c220a9460099566be2208011c220a90a0099566be220800529e799e66b')

    def test_distinct_campaign_path_and_player_reset_owners(self):
        self.source(0x02E52B, 'a920000c8a1ba901008d9dd7')
        self.source(0x04BE51, 'a920001c8a1b')
        self.source(0x088456, 'fbde1d01')
        self.source(0x08848F, 'fbde1d01')
        self.source(0x0695C8, '8de21d9cce1d9cde1d9cdf1d')


if __name__ == '__main__':
    unittest.main()
