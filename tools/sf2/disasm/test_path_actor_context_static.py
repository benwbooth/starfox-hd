#!/usr/bin/env python3
"""Temporary path ownership contracts from complete static handler bytes."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathActorContextStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_linked_selection_saves_caller_and_target_path_then_advances_target(self):
        self.assert_source(0x7FA86D,
            "8E 6D D7 C2 20 B5 2B 48 E2 20 B4 06 C2 20 B9 2B 00 8D 6F D7 "
            "E2 20 BB C2 20 68 95 2B E2 20 4C E8 CA")

    def test_missing_mother_branches_before_either_context_save(self):
        self.assert_source(0x7FA88E,
            "C2 20 B5 06 E2 20 D0 04 5C F3 CA 7F 8E 6D D7 C2 20 B5 2B 48 E2 20 "
            "B4 06 C2 20 B9 2B 00 8D 6F D7 E2 20 BB C2 20 68 95 2B E2 20 4C BE CA")

    def test_variable_child_reads_original_actor_byte_and_saves_caller_even_on_miss(self):
        self.assert_source(0x7FA8BB,
            "20 BC C4 20 47 CB B9 00 00 8D B1 16 8E 6D D7 C2 20 B5 2B 48 E2 20 "
            "B5 23 29 10 F0 04 5C DE A8 7F B4 06 BB AD B1 16 22 7B 2A 7F C0 00 00 "
            "D0 0D AE 6D D7 C2 20 68 95 2B E2 20 82 08 22 C2 20 B9 2B 00 8D 6F D7 "
            "E2 20 BB C2 20 68 95 2B E2 20 4C A9 CA")

    def test_literal_child_uses_owner_flag_or_mother_without_null_parent_guard(self):
        self.assert_source(0x7FA90C,
            "20 BC C4 8D B1 16 8E 6D D7 C2 20 B5 2B 48 E2 20 B5 23 29 10 F0 04 "
            "5C 29 A9 7F B4 06 BB AD B1 16 22 7B 2A 7F C0 00 00 D0 0D AE 6D D7 "
            "C2 20 68 95 2B E2 20 82 BD 21 C2 20 B9 2B 00 8D 6F D7 E2 20 BB C2 20 "
            "68 95 2B E2 20 4C A9 CA")

    def test_restore_writes_borrowed_path_then_caller_continuation_without_clearing_slots(self):
        self.assert_source(0x7FA957,
            "C2 20 B5 2B 48 E2 20 C2 20 AD 6F D7 95 2B E2 20 AE 6D D7 C2 20 "
            "68 95 2B E2 20 4C E8 CA")

    def test_latest_spawn_selection_uses_persistent_spawn_identity_and_single_save_pair(self):
        self.assert_source(0x7FA974,
            "8E 6D D7 C2 20 B5 2B 48 E2 20 AC 71 D7 C2 20 B9 2B 00 8D 6F D7 "
            "E2 20 BB C2 20 68 95 2B E2 20 4C E8 CA")

    def test_immediate_continuations_do_not_clear_wait_or_refresh_player_selection(self):
        self.assert_source(0x7FCAA9, "E2 20 C2 20 B5 2B 18 69 04 00 95 2B E2 20 4C 75 7E")
        self.assert_source(0x7FCABE, "E2 20 C2 20 B5 2B 18 69 03 00 95 2B E2 20 4C 75 7E")
        self.assert_source(0x7FCAE8, "E2 20 C2 20 F6 2B E2 20 4C 75 7E")
        self.assert_source(0x7FCAF3, "C2 20 20 20 C7 95 2B E2 20 4C 75 7E")
        self.assert_source(0x7FCAFF, "C2 20 20 4C C7 95 2B E2 20 4C 75 7E")

    def test_child_lookup_uses_complete_number_byte_and_first_matching_sibling(self):
        self.assert_source(0x7F2A7B, "8D 2A 19 DA B4 29 F0 0B B9 13 00 CD 2A 19 F0 03 BB 80 F1 FA 6B")


if __name__ == "__main__":
    unittest.main()
