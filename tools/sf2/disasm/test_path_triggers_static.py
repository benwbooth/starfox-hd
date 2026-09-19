#!/usr/bin/env python3
"""Static source checks for ordered path trigger storage and iteration."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathTriggersStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_timed_registration_increments_byte_and_initial_cost_is_five(self):
        self.assert_source(0x7F97B7, "20 04 C5 8D BF 16 20 28 C5 1A 8D BD 16")
        self.assert_source(0x7F980A, "C2 20 A9 05 00 22 4E 19 7F")

    def test_growth_cost_is_byte_count_times_four_plus_one(self):
        self.assert_source(0x7F983A, "BC E0 1C B9 61 6A 1A 0A 0A 1A 8D B1 16 9C B2 16")
        self.assert_source(0x7F985A, "C2 20 AD B1 16 22 4E 19 7F")
        self.assert_source(0x7F98A5, "BC E0 1C C2 20 98 22 6B 19 7F")

    def test_deletion_adjusts_position_and_decrements_budget_for_current_or_later(self):
        self.assert_source(0x7F9953, "8C B3 16 98 3A CD 40 D7 B0 11 CE 40 D7 CE 40 D7 CE 40 D7 CE 40 D7 CD 40 D7 90 0A E2 20 CE 42 D7 D0 03 EE 42 D7")

    def test_singleton_delete_frees_without_touching_iteration(self):
        self.assert_source(0x7F9915, "C2 20 AD BD 16 D9 62 6A F0 01 60 C2 20 98 22 6B 19 7F E2 20 9E E0 1C 9E E1 1C 60")

    def test_clear_finishes_after_current_callback(self):
        self.assert_source(0x7F98E8, "9E E0 1C 9E E1 1C A9 01 8D 42 D7 4C E8 CA")

    def test_timers_expire_before_callback_and_zero_timer_retains_observation(self):
        self.assert_source(0x7F9ADE, "B9 64 6A F0 1C 3A 99 64 6A 8D 43 D7 D0 13 C2 20 B9 61 6A 8D BD 16 8C BF 16 E2 20 20 09 99 82 8A 02")
        self.assert_source(0x7F9B33, "FA DA AD 43 D7 3A F0 03 82 4A 02 4C 68 9D")

    def test_common_advance_decrements_budget_again(self):
        self.assert_source(0x7F9D89, "AC 40 D7 C8 C8 C8 C8 E2 20 AD 42 D7 3A F0 04 5C CE 9A 7F")


if __name__ == "__main__":
    unittest.main()
