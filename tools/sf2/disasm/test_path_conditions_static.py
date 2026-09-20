#!/usr/bin/env python3
"""Source conditional-branch contracts, without game execution."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathConditionsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_ifnot_sets_shared_latch_and_equality_consumes_it(self):
        self.assert_source(0x7FA320, "A9 01 8D 72 B2 4C E8 CA")
        self.assert_source(0x7F8F4D, "AD 72 B2 F0 0D 9C 72 B2 28 D0 04 5C 94 CA 7F 4C 0B CB")

    def test_word_equality_reads_full_literal_and_consumes_ifnot_before_six_byte_continuation(self):
        self.assert_source(0x7F8F69, "20 BC C4 20 47 CB C2 20 20 4C C7 D9 00 00 E2 20 08 AD 72 B2 F0 0D 9C 72 B2 28 D0 04 5C 83 CA 7F 4C 17 CB 28 F0 04 5C 83 CA 7F 4C 17 CB")

    def test_variable_comparison_order_is_second_minus_first_and_widths_are_explicit(self):
        self.assert_source(0x7FB8E4, "20 BC C4 20 47 CB B9 00 00 8D B7 16 20 E0 C4 20 47 CB B9 00 00 CD B7 16 10 03 82 0A 12 4C 94 CA")
        self.assert_source(0x7FB904, "20 BC C4 20 47 CB C2 20 B9 00 00 8D B7 16 E2 20 20 E0 C4 20 47 CB C2 20 B9 00 00 CD B7 16 10 03 82 E4 11 4C 94 CA")
        self.assert_source(0x7FB92A, "20 BC C4 20 47 CB B9 00 00 8D B7 16 20 E0 C4 20 47 CB AD 72 B2 F0 11 9C 72 B2 B9 00 00 CD B7 16 D0 03 82 45 11 4C 0B CB B9 00 00 CD B7 16 D0 03 82 AE 11 4C 94 CA")
        self.assert_source(0x7FB960, "20 BC C4 20 47 CB C2 20 B9 00 00 8D B7 16 E2 20 20 E0 C4 20 47 CB AD 72 B2 F0 13 9C 72 B2 C2 20 B9 00 00 CD B7 16 D0 03 82 09 11 4C 0B CB C2 20 B9 00 00 CD B7 16 D0 03 82 70 11 4C 94 CA")

    def test_spatial_limits_are_word_literals_but_yaw_endpoints_are_bytes(self):
        self.assert_source(0x7F8C75, "C2 20 20 20 C7 8D B1 16 E2 20")
        self.assert_source(0x7FA7CE, "C2 20 20 20 C7 8D B1 16 E2 20")
        self.assert_source(0x7FAB7E, "20 BC C4 8D B3 16 20 E0 C4 38 ED B3 16 8D B5 16")

    def test_auxiliary_continuation_reads_selected_slot_twice_and_does_not_consume_ifnot(self):
        self.assert_source(0x7FB9BC, "AC 1F CF DA BB 7A 5A B4 2B B9 63 6B 7A DA BB 7A 89 40 F0 04 5C DC B9 7F 29 80 F0 04 5C EE B9 7F DA BB 5A B4 2B B9 77 6B 7A FA 89 20 F0 04 5C F1 B9 7F 4C BE CA 4C F3 CA")

    def test_nonzero_conditions_do_not_read_or_clear_ifnot(self):
        self.assert_source(0x7F8F1A, "20 BC C4 20 47 CB B9 00 00 D0 04 5C A9 CA 7F 4C FF CA")
        self.assert_source(0x7F8F2C, "20 BC C4 20 47 CB C2 20 B9 00 00 D0 04 5C A9 CA 7F 4C FF CA")

    def test_zero_and_variable_less_and_bit_conditions_preserve_inversion(self):
        self.assert_source(0x7F8EF4, "20 BC C4 20 47 CB B9 00 00 D0 04 5C FF CA 7F 4C A9 CA")
        self.assert_source(0x7F8F06, "20 BC C4 20 47 CB C2 20 B9 00 00 D0 04 5C FF CA 7F 4C A9 CA")
        self.assert_source(0x7FB8F6, "B9 00 00 CD B7 16 10 03 82 0A 12 4C 94 CA")
        self.assert_source(0x7FB91C, "B9 00 00 CD B7 16 10 03 82 E4 11 4C 94 CA")
        self.assert_source(0x7FB652, "20 EF B5 39 00 00 F0 03 82 AE 14 4C 94 CA")
        # The mask helper widens before returning: IFBIT reads a whole word.
        self.assert_source(0x7FB606, "C2 20 29 FF 00 8E B1 16 AA BF CF B5 7F AE B1 16 60")

    def test_hit_branches_consume_distinct_latches_and_hit_mask_is_literal(self):
        self.assert_source(0x7F9A79, "20 04 C5 8D B1 16 B5 38 2D B1 16 D0 04 5C A9 CA 7F AD B1 16 49 FF 35 38 95 38 4C F3 CA")
        self.assert_source(0x7F9507, "B5 23 29 08 D0 04 5C BE CA 7F B5 23 29 F7 95 23 4C F3 CA")

    def test_end_exits_without_movement_and_hold_installs_movement_without_advancing(self):
        self.assert_source(0x7F8B8B, "B5 25 09 08 95 25 4C FD 9E")
        self.assert_source(0x7F8ECF, "B5 09 09 08 95 09 C2 20 A9 DE 9D 95 19 E2 20 A9 7F 95 1B 4C DE 9D")
        self.assert_source(0x7F9D7C, "C2 20 B9 61 6A 95 2B E2 20 4C 53 7E")

    def test_both_vertical_predicates_use_height_and_wrapped_subtraction_sign(self):
        from path_semantics import PATH_SEMANTIC_BY_OPCODE
        self.assertEqual(PATH_SEMANTIC_BY_OPCODE[0x024].rust_name, "IfSelectedAtOrBelowObject")
        self.assertEqual(PATH_SEMANTIC_BY_OPCODE[0x101].rust_name, "IfSelectedAboveObject")
        self.assert_source(0x7F8E0C, "AC 1F CF C2 20 B9 0E 00 D5 0E 10 16 E2 20 4C BE CA")
        self.assert_source(0x7F8E1D, "AC 1F CF C2 20 B9 0E 00 D5 0E 30 05 E2 20 4C BE CA 4C F3 CA")
        # CABE advances three bytes; CAF3 jumps to its first word operand.
        self.assert_source(0x7FCAC2, "B5 2B 18 69 03 00 95 2B")
        self.assert_source(0x7FCAF3, "C2 20 20 20 C7 95 2B")

    def test_between_uses_subtraction_sign_not_unsigned_or_widened_signed_order(self):
        self.assert_source(0x7F8FBF, "20 E0 C4 D9 00 00 30 04 5C 83 CA 7F 20 04 C5 D9 00 00 10 04 5C 83 CA 7F 4C 17 CB")
        self.assert_source(0x7F9005, "C2 20 20 4C C7 D9 00 00 30 04 5C 61 CA 7F 20 A4 C7 D9 00 00 10 04 5C 61 CA 7F 4C 2F CB")

    def test_distance_uses_zero_height_and_geometry_length_then_unsigned_compare(self):
        self.assert_source(0x7F8C30, "B9 0C 00 38 F5 0C 8F 26 00 70 A9 00 00 8F 28 00 70 B9 10 00 38 F5 10 8F 2A 00 70")
        self.assert_source(0x7F8C50, "A9 01 A2 72 FB 22 7B 78 7F")
        self.assert_source(0x7F8C16, "AD B5 16 CD B1 16 B0 04 5C 0B CB 7F 4C 94 CA")
        self.assert_source(0x7F8C7F, "B4 06 D0 04 5C 94 CA 7F 82 6E FF")

    def test_ground_branch_adds_height_and_offset_and_tests_nonnegative_sum(self):
        self.assert_source(0x7F8CA3, "C2 20 20 20 C7 18 75 0E E2 20 08 AD 72 B2 F0 0D 9C 72 B2")
        self.assert_source(0x7F8CC0, "28 10 04 5C 94 CA 7F 4C 0B CB")

    def test_range_bounds_depth_then_nonnegative_wrapped_xy_manhattan(self):
        self.assert_source(0x7FA7E7, "B9 10 00 38 F5 10 10 04 49 FF FF 1A CD B1 16 E2 20 30 04 5C 94 CA 7F")
        self.assert_source(0x7FA800, "22 03 25 7F C2 20 AD FA 14 C9 00 00 30 05 CD B1 16 30 06")
        self.assert_source(0x7F2518, "B9 0E 00 38 F5 0E 85 08 30 06 18 6D FA 14 80 06 38 AD FA 14 E5 08 8D FA 14")

    def test_relative_yaw_uses_current_minus_selected_and_selected_heading(self):
        self.assert_source(0x7FAB9D, "B5 0C 38 F9 0C 00 85 02 B5 10 38 F9 10 00 85 08")
        self.assert_source(0x7FABAF, "22 58 1D 7F EB 18 79 14 00 38 ED B3 16 CD B5 16 90 04 5C 94 CA 7F 4C 0B CB")

    def test_selected_planes_rotate_right_or_forward_normal_and_use_projection_sign(self):
        self.assert_source(0x7F8DCF, "9B AE 1F CF A9 7F 85 02 64 08 64 97")
        self.assert_source(0x7F8DE5, "22 5B B7 0D BB 89 80 D0 40 4C BE CA")
        self.assert_source(0x7F8DFB, "DA 9B AE 1F CF 22 51 B7 0D FA 89 80 D0 25 4C BE CA")
        self.assert_source(0x0DB751, "E2 20 64 02 64 08 A9 7F 85 97")
        self.assert_source(0x0DB83D, "A4 7F C2 20 B9 0C 00 38 F5 0C 8F 26 00 70 B9 0E 00 38 F5 0E 8F 28 00 70 B9 10 00 38 F5 10 8F 2A 00 70")
        self.assert_source(0x0DB872, "AF AA 00 70 EB E2 20 85 02")

    def test_facing_arc_uses_selected_minus_current_and_current_heading(self):
        self.assert_source(0x7FAB4D, "B9 0C 00 38 F5 0C 85 02 B9 10 00 38 F5 10 85 08")
        self.assert_source(0x7FAB5F, "22 58 1D 7F EB 85 02 20 BC C4 85 97 06 97 18 75 14 18 65 02 C5 97")


if __name__ == "__main__":
    unittest.main()
