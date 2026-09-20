#!/usr/bin/env python3
"""Primary target-control source contracts, without executing source code."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathPlayerControlStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_path_handlers_read_word_parameter_and_call_distinct_primary_services(self):
        self.assert_source(0x7FC13B, "C2 20 20 20 C7 85 3A E2 20 A5 5E 29 E7 85 5E 8F 3A 30 00 22 46 B7 07 4C BE CA")
        self.assert_source(0x7FC189, "E2 20 A5 5E 29 E7 85 5E 8F 3A 30 00 22 33 B8 07 4C E8 CA")

    def test_selected_mode_low_nibble_updates_preserve_class_and_advance_immediately(self):
        self.assert_source(0x7FB081, "AC 1F CF DA BB 7A 5A B4 2B B9 A1 6A 29 F0 09 01 99 A1 6A 7A DA BB 7A 4C E8 CA")
        self.assert_source(0x7FB04D, "AC 1F CF DA BB 7A 5A B4 2B B9 A1 6A 29 F0 09 04 99 A1 6A 7A DA BB 7A 4C E8 CA")

    def test_selected_action_clear_is_not_the_other_auxiliary_flag_or(self):
        self.assert_source(0x7FB77A, "AC 1F CF DA BB 7A 5A B4 2B B9 77 6B 29 FE 99 77 6B 7A DA BB 7A 4C E8 CA")
        self.assert_source(0x7FBAA5, "AC 1F CF DA B6 2B 9B FA 20 BC C4 19 E4 6B 99 E4 6B 4C D3 CA")

    def test_configuration_wrapper_does_not_skip_final_owner_refresh_when_locked(self):
        self.assert_source(0x07B746, "DA 08 E2 20 C2 10 A9 03 85 3C A9 1F 85 3E 22 9F B7 07 A9 04 85 3A A9 08 85 3C A9 08 85 3E 22 9B B8 07 A9 1F 85 3A 85 3C 85 3E 22 61 B8 07 22 33 B8 07 28 FA 6B")

    def test_projectile_target_temporarily_unlocks_replaces_owner_and_locks_again(self):
        self.assert_source(0x07B67D, "da 08 e2 20 c2 10 a0 00 00 84 3a ac c3 12 da bb 7a 5a b4 2b b9 8c 6a 29 7f 99 8c 6a 7a da bb 7a 5a da b6 2b 9b fa a9 0a 99 ea 6b 7a a9 03 85 3c a9 1f 85 3e 22 9f b7 07 a9 03 85 3a a9 03 85 3c a9 02 85 3e 22 9b b8 07 a9 19 85 3a 85 3c a9 1f 85 3e 22 61 b8 07 22 33 b8 07 ac c3 12 da bb 7a 5a b4 2b b9 8c 6a 09 80 99 8c 6a 7a da bb 7a 28 fa 6b")
        # Consumer establishes a delay, not a projectile lifetime counter.
        self.assert_source(0x07EA73, "ae c3 12 b4 2b b9 ea 6b f0 15 3a 99 ea 6b f0 07 b9 e9 6b 09 18 80 05 b9 e9 6b 29 f7 99 e9 6b")

    def test_pitch_recoil_is_initialized_only_at_zero_then_reversed_and_damped(self):
        self.assert_source(0x7FB1DF, "ac c3 12 20 bc c4 85 02 20 e0 c4 85 03 da bb 7a 5a b4 2b c2 20 b9 3b 6b c9 00 00 e2 20 f0 04 5c 0b b2 7f c2 20 a5 02 99 3b 6b e2 20 7a da bb 7a 4c be ca")
        self.assert_source(0x079AAB, "5a 08 e2 20 c2 10 b4 2b c2 20 b9 3b 6b 49 ff ff 1a 99 3b 6b e2 20 c2 20 b9 3b 6b c9 00 00 f0 1c 30 0e 38 e9 10 00 c9 00 00 10 11 a9 00 00 80 0c 18 69 10 00 c9 00 00 30 03 a9 00 00 99 3b 6b e2 20 28 7a 6b")
        self.assert_source(0x0796A3, "c2 20 b9 3b 6b 0a 18 75 12 95 12 e2 20")

    def test_alternate_configuration_handlers_use_distinct_helpers_and_low_byte_only_shift(self):
        self.assert_source(0x7FC155, "C2 20 20 20 C7 85 3A E2 20 A5 5E 29 E7 85 5E 8F 3A 30 00 22 26 B7 07 4C BE CA C2 20 20 20 C7 85 3A E2 20 A5 5E 29 E7 85 5E 8F 3A 30 00 22 EF B6 07 4C BE CA")
        self.assert_source(0x07B6EF, "DA 08 E2 20 C2 10 06 3A A9 03 85 3C A9 1F 85 3E 22 9F B7 07 A9 01 85 3A A9 02 85 3C A9 02 85 3E 22 9B B8 07 A9 1F 85 3A 85 3C 85 3E 22 61 B8 07 22 33 B8 07 28 FA 6B DA 08 E2 20 C2 10 A9 03 85 3C A9 1F 85 3E 22 9F B7 07 A9 06 85 3A A9 06 85 3C A9 03 85 3E 80 1E")

    def test_target_configuration_clears_offset_before_lock_and_clamps_negative_secondary_range(self):
        self.assert_source(0x07B79F, "5A 08 E2 20 C2 10 AC C3 12 DA B6 2B 9B FA B9 8C 6A 29 BF 99 8C 6A B9 8C 6A 29 80 F0 04 5C FA B7 07 C2 20 A9 02 00 99 1C 6C B5 0C 99 92 6A B5 0E 99 94 6A B5 10 99 96 6A A9 FF 00 99 24 6C A5 3A 99 90 6A 10 03 A9 01 00 99 26 6C 8A 99 98 6A E2 20 A5 3C 99 29 6C A5 3E 99 28 6C 28 7A 6B")

    def test_refresh_checks_owner_while_rate_and_limit_setters_independently_check_lock(self):
        self.assert_source(0x07B833, "5A 08 E2 20 C2 10 AC C3 12 20 42 B8 28 7A 6B DA B6 2B 9B FA C2 20 8A D9 98 6A D0 0F B5 0C 99 92 6A B5 0E 99 94 6A B5 10 99 96 6A E2 20 60 5A 08 E2 20 C2 10 AC C3 12 20 7B B8 AD A6 1A 89 02 F0 04 5C 78 B8 07 28 7A 6B DA B6 2B 9B FA B9 8C 6A 29 80 F0 04 5C 9A B8 07 A5 3A 99 2A 6C A5 3C 99 2B 6C A5 3E 99 2C 6C 60 5A 08 E2 20 C2 10 AC C3 12 20 AA B8 28 7A 6B DA B6 2B 9B FA B9 8C 6A 29 80 F0 04 5C C9 B8 07 A5 3A 99 8D 6A A5 3C 99 8E 6A A5 3E 99 8F 6A 60")

    def test_linked_lock_sets_rates_limits_and_conditionally_both_ranges(self):
        self.assert_source(0x07F5EC, "DA AE C3 12 B4 2B B9 8C 6A 09 80 99 8C 6A A9 10 99 2A 6C A9 10 99 2B 6C A9 1F 99 2C 6C A9 03 99 8D 6A A9 03 99 8E 6A A9 04 99 8F 6A B9 63 6B 29 80 D0 04 5C 32 F6 07 C2 20 A9 08 00 99 90 6A 99 26 6C E2 20 FA 6B FA 6B")

    def test_position_copy_or_two_stage_byte_offset_uses_primary_not_selected(self):
        self.assert_source(0x07F634, "5A AC C3 12 DA BB 7A 5A B4 2B B9 63 6B 7A DA BB 7A 29 80 D0 04 5C BB F6 07 A9 00 85 02 85 04 89 80 F0 06 A9 FF 85 05 80 02 64 05 A9 00 85 08 85 0A 89 80 F0 06 A9 FF 85 0B 80 02 64 0B A9 50 85 97 85 E4 89 80 F0 06 A9 FF 85 E5 80 02 64 E5 B9 12 00 22 4E 3A 7F A5 04 85 02 A5 0A 85 08 A5 E4 85 97 B9 14 00 22 A9 38 7F C2 20 A5 04 18 79 0C 00 95 0C A5 E4 18 79 10 00 95 10 A5 0A 18 79 0E 00 95 0E E2 20 7A 6B C2 20 B9 0C 00 95 0C B9 0E 00 95 0E B9 10 00 95 10 E2 20 7A 6B")


if __name__ == "__main__":
    unittest.main()
