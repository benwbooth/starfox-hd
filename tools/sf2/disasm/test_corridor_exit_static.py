#!/usr/bin/env python3
"""Source ownership and complete side effects of the two corridor exits."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class CorridorExitStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_reset_resolves_path_selected_player_and_clears_two_distinct_fields(self):
        self.assert_source(0x7FB660, 'ac1fcfdabb7a5ab42bb9776b29fb99776ba90099616a7adabb7a4ce8ca')

    def test_entry_protects_primary_and_tests_entry_bit_before_setting_start_bit(self):
        self.assert_source(0x07F95F, '5aacc312dab62b9bfaa93f99e26b7aad741d8904d00509058d741d6b')

    def test_mode_path_commands_write_pending_request_not_the_current_mode(self):
        self.assert_source(0x7FB081, 'ac1fcfdabb7a5ab42bb9a16a29f0090199a16a7adabb7a4ce8ca')
        self.assert_source(0x7FB04D, 'ac1fcfdabb7a5ab42bb9a16a29f0090499a16a7adabb7a4ce8ca')

    def test_level_corridor_chases_the_same_word_twice_after_its_one_time_entry_helper(self):
        # This apparently unusual pair really does address A3 twice. It is
        # not permission to substitute a width/height chase on distinct axes.
        self.assertEqual(self.rom[0x4D237:0x4D252], bytes.fromhex('0062006e4139e8820a00a3826400a38a14c80034d24153d2163ed2'))

    def test_common_origin_clears_camera_roll_before_importing_the_handoff(self):
        self.assertEqual(self.rom[0x4D263:0x4D27B], bytes.fromhex('fc0b1e00007c0c881d7c108c1d6c0e79148e1d6b126b1642'))


if __name__ == '__main__':
    unittest.main()
