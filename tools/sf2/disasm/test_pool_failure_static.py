#!/usr/bin/env python3
"""Source-bound non-returning object-pool failure, not a carry-clear return."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PoolFailureStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_empty_strategy_free_list_enters_fatal_handler_before_unreachable_return(self):
        self.assert_source(0x7F2925, "C2 20 9B AE AA 12 D0 06 E2 20 BB 4C 69 29")
        self.assert_source(0x7F2969, "A5 5E 29 E7 85 5E 8F 3A 30 00 22 32 80 00 18 6B")

    def test_fatal_handler_sets_error_color_then_disables_interrupts_and_loops_forever(self):
        self.assert_source(0x008032,
            "C2 30 A9 E0 03 8F 2C 19 00 80 00 E2 20 A9 00 48 AB "
            "8E D8 16 9C 00 42 9C 0C 42 78")
        self.assert_source(0x008095,
            "9C 21 21 AD 2C 19 8D 22 21 AD 2D 19 8D 22 21 A2 0F 00 A0 00 00 "
            "88 D0 FD CA D0 F7 9C 21 21 9C 22 21 9C 22 21 A2 0A 00 A0 00 00 "
            "88 D0 FD CA D0 F7 80 CE")

    def test_recovery_commits_request_and_shield_before_feedback_can_fault(self):
        self.assert_source(0x069F36,
            "AD 1B 1E 9C 1B 1E 8D AE 1D F0 13 18 79 00 6C CD D5 1D "
            "90 03 AD D5 1D 99 00 6C 22 C6 D0 07")


if __name__ == "__main__":
    unittest.main()
