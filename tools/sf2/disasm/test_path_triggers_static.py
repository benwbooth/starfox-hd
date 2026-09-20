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

    def test_always_registration_zeroes_timer_and_condition_and_reads_literal_path(self):
        self.assert_source(0x7F97A9, "9C BD 16 A9 00 8D BF 16 20 F3 97 82 07 33")
        self.assert_source(0x7F97F3, "C2 20 20 20 C7 8D C1 16")

    def test_conditional_relative_and_timed_builders_keep_predicate_separate_from_expiry(self):
        self.assert_source(0x7F97E4, "9C BD 16 20 04 C5 8D BF 16 20 F3 97 82 B6 32")
        self.assert_source(0x7F97CA, "9C BD 16 20 E0 C4 8D BF 16 20 BC C4 C2 20 29 FF 00 18 75 2B 20 F8 97 82 DA 32")
        self.assert_source(0x7F97B7, "20 04 C5 8D BF 16 20 28 C5 1A 8D BD 16 20 F3 97 82 CA 32")
        self.assert_source(0x7F9B00, "B9 63 6A C2 20 29 FF 00 0A AA E2 20 7C 0F 9B")

    def test_cancel_reads_identity_not_a_call_target_and_clear_has_its_own_advance(self):
        self.assert_source(0x7F98F6, "C2 20 20 20 C7 8D BD 16 9C BF 16 E2 20 20 09 99 4C BE CA")
        self.assert_source(0x7F98DC, "BC E0 1C C2 20 98 22 6B 19 7F E2 20 9E E0 1C 9E E1 1C A9 01 8D 42 D7 4C E8 CA")

    def test_trigger_condition_and_timer_readers_use_third_and_fourth_literal_bytes(self):
        for address, offset in ((0x7FC504, 3), (0x7FC528, 4)):
            self.assert_source(address, f"84 79 A0 {offset:02X} 00 A5 5E 29 EF 85 5E 8F 3A 30 00 B7 F9 8D 11 19 A5 5E 09 10 85 5E 8F 3A 30 00 AD 11 19 A4 79 60")

    def test_callback_primary_view_filter_latches_only_low_phase_byte(self):
        self.assert_source(0x09F349, "AC C3 12 B9 25 00 29 20 D0 04 5C 5C F3 09 A9 01 9D E2 1C C2 20 A9 62 F3 6B")

    def test_view_filter_is_not_a_death_flag_and_is_copied_to_related_actor(self):
        self.assert_source(0x7F1262, "B9 25 00 29 20 F0 0A B9 23 00 29 40 F0 03 82 ED 01")
        self.assert_source(0x7F130F, "B9 25 00 29 20 F0 0A B9 23 00 29 40 D0 03 82 40 01")
        self.assert_source(0x06BAC3, "B9 25 00 29 20 F0 04 5C D6 BA 06 B5 25 29 DF 95 25 80 06 B5 25 09 20 95 25")

    def test_run_when_paused_flag_advances_without_moving_or_touching_other_flags(self):
        self.assert_source(0x7FB30E, "B5 26 09 08 95 26 4C E8 CA")

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

    def test_depth_increments_for_calls_but_only_decrements_inside_callbacks(self):
        self.assert_source(0x7F955E, "EE 1D 6A")
        self.assert_source(0x7F958D, "EE 1D 6A")
        self.assert_source(0x7F95A6, "AC 6D B2 F0 09 CE 1D 6A D0 04 5C 88 9D 7F")

    def test_batch_entry_keeps_mode_until_actual_callback_entry(self):
        self.assert_source(0x7F9AA8, "E2 20 AD 6D B2 0D 6E B2 F0 04 22 27 80 00 B4 2B 8C 6D B2 BC E0 1C D0 04 5C D7 9D 7F B9 61 6A D0 04 5C D7 9D 7F")
        self.assert_source(0x7F9D74, "9C 71 B2 A9 01 8D 1D 6A")

    def test_force_and_deferred_call_have_distinct_immediate_effects(self):
        self.assert_source(0x7F99A9, "AC 6D B2 D0 04 5C BE CA 7F A9 01 8D 71 B2 A9 00 95 17 95 15 C2 20 A9 53 7E 95 19 E2 20 A9 7F 95 1B C2 20 20 20 C7 8D 6D B2 4C BE CA")
        self.assert_source(0x7FBC5A, "AC 6D B2 D0 04 5C BE CA 7F A9 02 8D 71 B2 C2 20 A9 53 7E 95 19 E2 20 A9 7F 95 1B C2 20 20 20 C7 8D 6F B2 4C BE CA")

    def test_deferred_call_pushes_adjusted_continuation_without_incrementing_depth(self):
        self.assert_source(0x7F9DA5, "C2 20 CE 6D B2 CE 6D B2 CE 6D B2 E2 20 C2 20 AD 6D B2 8D 69 B2 BD DE 1C 22 0F 1A 7F 9D DE 1C E2 20 E2 20 AC 6F B2 94 2B 80 08")
        self.assert_source(0x7F9DCF, "20 FC AF AC 6D B2 94 2B 9C 6D B2 9C 6E B2 60")
        self.assert_source(0x7FAFFC, "60")

    def test_dispatch_table_contains_all_eighteen_authored_predicates(self):
        self.assert_source(0x7F9B0F, "68 9D 46 9D 4A 9D 4E 9D 52 9D 56 9D 5A 9D 5E 9D 38 9D 1E 9D 0A 9D F8 9C EB 9C 02 9C 7F 9B 41 9B 60 9B 33 9B")

    def test_hit_event_is_consumed_but_contact_latches_are_not(self):
        self.assert_source(0x7F9D0A, "FA DA B5 23 29 08 D0 04 5C 88 9D 7F B5 23 29 F7 95 23 80 4A")
        self.assert_source(0x7F9D1E, "FA DA 5A B5 26 29 02 F0 04 5C BA 9C 7F B5 26 29 04 F0 04 5C DB 9C 7F 7A 80 50")
        self.assert_source(0x7F9D38, "FA DA B5 22 29 02 D0 04 5C 88 9D 7F 80 22")

    def test_part_target_keeps_source_secondary_mismatch_and_primary_selection(self):
        self.assert_source(0x7F9B94, "29 F0 C9 20 D0 04 5C A0 9B 7F 80 20")
        self.assert_source(0x7F9BA0, "C2 20 8A D9 E8 1C E2 20 D0 16 BD EA 1C D9 EA 1C F0 03 82 0B 00 B9 24 00 29 02 F0 04 5C BA 9C 7F")
        self.assert_source(0x7F9BDE, "C2 20 8A D9 E8 1C E2 20 D0 16 BD EA 1C D9 EA 1C F0 03 82 E8 00 B9 24 00 29 02 F0 04 5C BA 9C 7F")

    def test_selection_updates_global_target_and_actor_side(self):
        self.assert_source(0x7F9CBA, "AC C3 12 8C 1F CF B5 24 29 7F 95 24 7A 82 9E 00")
        self.assert_source(0x7F9CDB, "AC C5 12 8C 1F CF B5 24 09 80 95 24 7A 82 7D 00")

    def test_every_path_entry_refreshes_selected_player_from_actor_side(self):
        self.assert_source(0x7F7E5D, "B5 24 29 80 F0 04 5C 6F 7E 7F AC C3 12 8C 1F CF 80 06 AC C5 12 8C 1F CF")
        self.assert_source(0x7F9D7C, "C2 20 B9 61 6A 95 2B E2 20 4C 53 7E")

    def test_controlled_flags_and_health_attachment_predicates(self):
        self.assert_source(0x7F9B4B, "B4 2B B9 65 6B 7A DA BB 7A 89 10")
        self.assert_source(0x7F9B6A, "B4 2B B9 65 6B 7A DA BB 7A 89 08")
        self.assert_source(0x7F9CEB, "FA DA B5 2D D0 04 5C 68 9D 7F 82 90 00")
        self.assert_source(0x7F9CF8, "FA DA C2 20 B5 06 E2 20 F0 04 5C 88 9D 7F 5C 68 9D 7F")


if __name__ == "__main__":
    unittest.main()
