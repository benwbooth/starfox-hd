#!/usr/bin/env python3
"""Typed actor arithmetic contracts verified against source bytes only."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathFieldsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_byte_angle_adds_write_only_the_angle_and_advance(self):
        self.assert_source(0x7F8627, "20 BC C4 20 47 CB 20 E0 C4 18 79 00 00 99 00 00 4C BE CA")
        self.assert_source(0x7F863A, "20 BC C4 18 75 12 95 12 4C D3 CA")
        self.assert_source(0x7F8645, "20 BC C4 18 75 14 95 14 4C D3 CA")
        self.assert_source(0x7F8650, "20 BC C4 18 75 16 95 16 4C D3 CA")

    def test_phase_high_literal_alias_and_source_defined_noop_have_no_other_effects(self):
        self.assert_source(0x7FB130, "20 BC C4 9D E3 1C 4C D3 CA")
        self.assert_source(0x7FAFF6, "20 FC AF 4C E8 CA 60")

    def test_banked_byte_lookup_zero_extends_index_and_samples_before_destination_write(self):
        self.assert_source(0x7FA60E, "20 FF A6 AD B7 16 18 65 54 85 54 E2 20 A5 5E 29 E7 85 5E 8F 3A 30 00 A7 54 99 00 00 4C 83 CA")
        self.assert_source(0x7FA6FF, "20 04 C5 85 56 C2 20 20 20 C7 85 54 E2 20 20 28 C5 20 47 CB B9 00 00 8C B3 16 8D B7 16 9C B8 16 20 4C C5 20 47 CB C2 20 60")
        self.assert_source(0x7FCA83, "E2 20 C2 20 B5 2B 18 69 06 00 95 2B E2 20 4C 75 7E")
        for address, data in ((0x4F2FD, "9055fb07a28a"), (0x45671, "902afc06a189"),
                              (0x42B3B, "9041fc06a9a9")):
            self.assertEqual(self.rom[address:address + 6], bytes.fromhex(data))

    def test_banked_word_lookup_doubles_after_helper_has_entered_word_width(self):
        self.assert_source(0x7FA62D, "20 FF A6 AD B7 16 0A 18 65 54 85 54 E2 20 A5 5E 29 E7 85 5E 8F 3A 30 00 C2 20 A7 54 99 00 00 4C 83 CA")
        self.assert_source(0x7FA713, "B9 00 00 8C B3 16 8D B7 16 9C B8 16 20 4C C5 20 47 CB C2 20 60")
        self.assertEqual(self.rom[0x45585:0x4558B], bytes.fromhex("9155fc06a192"))

    def test_indexed_add_resamples_selector_after_destination_write_then_bounds_after_increment(self):
        self.assert_source(0x7FA690,
            "20 FF A6 AD B7 16 18 65 54 85 54 E2 20 A5 5E 29 E7 85 5E 8F 3A 30 00 "
            "A7 54 18 79 00 00 99 00 00 E2 20 AC B3 16 B9 00 00 1A 99 00 00 "
            "20 70 C5 3A D9 00 00 B0 05 A9 00 99 00 00 4C 72 CA")
        self.assert_source(0x7FA6CD,
            "20 FF A6 AD B7 16 18 65 54 85 54 E2 20 A5 5E 29 E7 85 5E 8F 3A 30 00 "
            "A7 54 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 18 79 00 00 99 00 00 4C B0 A6")
        self.assert_source(0x7FC570,
            "84 79 A0 06 00 A5 5E 29 EF 85 5E 8F 3A 30 00 B7 F9 8D 11 19 "
            "A5 5E 09 10 85 5E 8F 3A 30 00 AD 11 19 A4 79 60")
        self.assert_source(0x7FCA72, "E2 20 C2 20 B5 2B 18 69 07 00 95 2B E2 20 4C 75 7E")
        self.assertEqual(self.rom[0x4129A:0x412A2], bytes.fromhex("00 03 1B B3 00 A1 0E 14"))

    def test_script_parameter_is_path_owned_not_a_health_or_height_alias(self):
        # Authored COPY byte stores health, then decrements the saved byte.
        self.assertEqual(self.rom[0x45E1D:0x45E22], bytes.fromhex("4E 27 2D 6F 27"))
        # Another path saves height's low byte before clearing the height.
        self.assertEqual(self.rom[0x47F2A:0x47F2F], bytes.fromhex("4F 27 0E 6C 0E"))
        # That saved byte is a one-based bit selector, not an elapsed timer.
        self.assertEqual(self.rom[0x47F45:0x47F49], bytes.fromhex("67 27 52 7F"))
        self.assertEqual(self.rom[0x47F4C:0x47F4F], bytes.fromhex("D9 27 A1"))
        self.assertEqual(self.rom[0x47F64:0x47F67], bytes.fromhex("D8 27 A1"))

    def test_animation_operands_modify_packed_controls_not_resolved_frames(self):
        # Extended variable decoding adds $1C41; $89/$8A therefore alias
        # precisely the color/shape controls used by the animation handlers.
        self.assert_source(0x7FCB47, "08 C2 20 8E B1 16 29 FF 00 89 80 00 F0 04 18 69 41 1C 18 6D B1 16 A8 28 60")
        self.assert_source(0x7F8CFD, "20 BC C4 8D B1 16 AD B1 16 09 80 9D CB 1C 4C D3 CA")
        self.assert_source(0x7F8D3A, "20 BC C4 8D B1 16 AD B1 16 09 80 9D CA 1C 4C D3 CA")
        self.assert_source(0x7F1406, "B9 CB 1C 30 02 A5 C4 29 7F 9D 19 00 B9 CA 1C 30 02 A5 C4 29 7F 9D 1A 00")
        for address, expected in (
            (0x433DA, "6d8a"), (0x445ED, "5389a3"),
            (0x4582F, "4e8a2d"), (0x4966E, "0b8689"),
            (0x49F36, "6b89"), (0x4AE44, "528aa2"), (0x4AF00, "52a18a"),
        ):
            data = bytes.fromhex(expected)
            self.assertEqual(self.rom[address:address + len(data)], data)

    def test_script_working_word_holds_numeric_constants_or_a_tested_bit_set(self):
        self.assertEqual(self.rom[0x4D1CF:0x4D1D3], bytes.fromhex("0C C8 00 A3"))
        self.assertEqual(self.rom[0x4D20B:0x4D20F], bytes.fromhex("0C 64 00 A3"))
        self.assertEqual(self.rom[0x4601E:0x46022], bytes.fromhex("0C 10 FF A3"))
        # The imported bit set is tested and explicitly cleared on one arm.
        self.assertEqual(self.rom[0x487D3:0x487E0], bytes.fromhex("7B A3 43 6D 27 DA 27 A3 E0 87 6C A3 42"))

    def test_indexed_node_import_widens_literal_index_and_copies_the_full_word(self):
        self.assert_source(0x7F9F75, "20e79fc220adb7169900004cbeca")
        self.assert_source(0x7F9FE7, "20e0c4c22029ff00a8b95cd78db716e22020bcc42047cb60")
        self.assert_source(0x7FCABE, "e220c220b52b18690300952be2204c757e")
        self.assertEqual(self.rom[0x4545F:0x4546C], bytes.fromhex("48 7b a3 9a da 2d a3 53 8d c7 16 60 54"))
        self.assertEqual(self.rom[0x48D53], 0x0F)

    def test_active_node_flags_low_byte_load_and_full_word_writeback_are_distinct(self):
        # Active node at DB07 supplies node kind, scenario, and flags. The
        # flag load is byte-wide; its high byte is not implicitly zeroed.
        self.assert_source(0x04B1FC, "ae07dbbd0400c22029ff00e2208db51bbd08008da51bbd0a008df6d7")
        self.assert_source(0x04B23B, "da08c220ada51b9d0800adf6d79d0a00e220aeb51bada1d79d94d728fa60")
        # Another live writer changes bit 5 except in scenarios 3 and 9.
        self.assert_source(0x06A241, "ade21dc909f00cc903f008adf6d709208df6d7")

    def test_world_position_add_sign_extends_literal_byte_before_word_add(self):
        self.assert_source(0x7F865B, "20 BC C4 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 18 75 0C 95 0C 4C D3 CA")
        self.assert_source(0x7F8687, "18 75 0E 95 0E 4C D3 CA")
        self.assert_source(0x7F86A1, "18 75 10 95 10 4C D3 CA")

    def test_variable_byte_to_word_copy_and_add_both_sign_extend(self):
        self.assert_source(0x7F8929, "BD 00 00 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 18 79 00 00 99 00 00")
        self.assert_source(0x7F897A, "E2 20 BD 00 00 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 99 00 00")

    def test_word_to_byte_copy_retains_low_byte_and_other_byte_is_untouched(self):
        self.assert_source(0x7F8968, "C2 20 BD 00 00 E2 20 99 00 00 FA 4C BE CA")

    def test_variable_helper_uses_first_operand_as_destination_and_second_as_source(self):
        self.assert_source(0x7F8995, "08 E2 20 20 E0 C4 20 47 CB 5A 20 BC C4 20 47 CB FA 28 60")
        self.assert_source(0x7FCB47, "08 C2 20 8E B1 16 29 FF 00 89 80 00 F0 04 18 69 41 1C 18 6D B1 16 A8 28 60")

    def test_word_swap_reads_both_words_then_writes_second_before_first(self):
        self.assert_source(0x7FC452, "20bcc42047cb5ac220b900008db316e22020e0c42047cbc220b9000048adb316990000687a9900004cbeca")

    def test_variable_copies_and_adds_keep_widths_and_immediate_continuation(self):
        self.assert_source(0x7F88FF, "DA 20 95 89 BD 00 00 18 79 00 00 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8911, "DA 20 95 89 C2 20 BD 00 00 18 79 00 00 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8925, "DA 20 95 89 BD 00 00 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 18 79 00 00 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8946, "DA 20 95 89 BD 00 00 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8954, "DA 20 95 89 C2 20 BD 00 00 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8964, "DA 20 95 89 C2 20 BD 00 00 E2 20 99 00 00 FA 4C BE CA")
        self.assert_source(0x7F8976, "DA 20 95 89 E2 20 BD 00 00 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 99 00 00 FA 4C BE CA")

    def test_literal_writes_have_no_motion_or_collision_side_effect(self):
        self.assert_source(0x7F89A8, "20 E0 C4 20 47 CB E2 20 20 BC C4 99 00 00 4C BE CA")
        self.assert_source(0x7F89B9, "20 04 C5 20 47 CB C2 20 20 20 C7 99 00 00 E2 20 4C A9 CA")
        self.assert_source(0x7FC4BC, "84 79 A0 01 00")
        self.assert_source(0x7FC4E0, "84 79 A0 02 00")
        self.assert_source(0x7FC504, "84 79 A0 03 00")

    def test_zero_assignment_width_and_word_add_operand_order(self):
        self.assert_source(0x7F89CC, "20 BC C4 20 47 CB E2 20 A9 00 99 00 00 4C D3 CA")
        self.assert_source(0x7F89DC, "20 BC C4 20 47 CB C2 20 A9 00 00 99 00 00 E2 20 4C D3 CA")
        self.assert_source(0x7F86CD, "20 BC C4 20 47 CB C2 20 20 4C C7 18 79 00 00 99 00 00 E2 20 4C A9 CA")

    def test_increment_and_decrement_wrap_at_destination_width(self):
        self.assert_source(0x7F86EA, "B9 00 00 1A 99 00 00 4C D3 CA")
        self.assert_source(0x7F86FA, "C2 20 B9 00 00 1A 99 00 00 E2 20 4C D3 CA")
        self.assert_source(0x7F870E, "B9 00 00 3A 99 00 00 4C D3 CA")

    def test_negation_uses_complement_plus_one_without_saturation(self):
        self.assert_source(0x7F9A1B, "B9 00 00 49 FF 1A 99 00 00 4C D3 CA")
        self.assert_source(0x7F9A2D, "C2 20 B9 00 00 49 FF FF 1A 99 00 00 4C D3 CA")

    def test_variable_bit_helper_reads_selector_byte_and_wraps_before_word_lookup(self):
        self.assert_source(0x7FB5EF, "20 E0 C4 20 47 CB 5A 20 FB B5 7A 60")
        self.assert_source(0x7FB5FB, "20 BC C4 20 47 CB B9 00 00 3A 0A C2 20 29 FF 00 8E B1 16 AA BF CF B5 7F AE B1 16 60")

    def test_signed_halves_increment_negatives_before_sign_preserving_shift(self):
        self.assert_source(0x7FA5BF, "20 BC C4 20 47 CB B9 00 00 10 01 1A C9 80 6A 99 00 00 4C D3 CA")
        self.assert_source(0x7FA5D4, "20 BC C4 20 47 CB C2 20 B9 00 00 10 01 1A C9 00 80 6A 99 00 00 4C D3 CA")
        self.assert_source(0x7FA5EC, "20 BC C4 20 47 CB B9 00 00 4A 99 00 00 4C D3 CA")

    def test_arithmetic_chase_literals_wrap_difference_and_apply_three_signed_halves(self):
        self.assert_source(0x7F9FFF,
            "20bcc48db31620e0c42047cbb900008db116adb316cdb116f02f38edb116c9003008"
            "c908100aa9088006c9f83002a9f8c9806a10026900c9806a10026900c9806a10026900"
            "186db1168db116adb1169900004cbeca")
        self.assert_source(0x7FA054,
            "c2202020c78db316e2202004c52047cbc220b900008db116e220c220adb316cdb116f03a"
            "38edb116c90000300ac90800100da908008008c9f8ff3003a9f8ffc900806a1003690000"
            "c900806a1003690000c900806a1003690000186db1168db116e220c220adb116990000e2204ca9ca")

    def test_variable_chase_samples_both_fields_before_writing_destination(self):
        self.assert_source(0x7FA1B5,
            "da209589b900008db116bd00008db316adb316cdb116f02f38edb116c9003008c908100a"
            "a9088006c9f83002a9f8c9806a10026900c9806a10026900c9806a10026900186db116"
            "8db116adb116990000fa4cbeca")
        self.assert_source(0x7FA209,
            "da209589c220b900008db116bd00008db316e220c220adb316cdb116f03a38edb116c90000"
            "300ac90800100da908008008c9f8ff3003a9f8ffc900806a1003690000c900806a1003690000"
            "c900806a1003690000186db1168db116e220c220adb116990000fa4cbeca")

    def test_waiting_chase_checks_original_equality_and_only_unequal_case_runs_movement(self):
        self.assert_source(0x7FA0C4,
            "20bcc48db31620e0c42047cbb900008db116adb316cdb116d0045c16a17f38edb116c9003008"
            "c908100aa9088006c9f83002a9f8c9806a10026900c9806a10026900c9806a10026900186db116"
            "8db11680045c23a17fadb1169900004cde9dadb1169900004cbeca")

    def test_variable_bit_handlers_mutate_words_and_test_without_ifnot(self):
        self.assert_source(0x7FB637, "20 EF B5 19 00 00 99 00 00 4C BE CA")
        self.assert_source(0x7FB643, "20 EF B5 49 FF FF 39 00 00 99 00 00 4C BE CA")
        self.assert_source(0x7FB652, "20 EF B5 39 00 00 F0 03 82 AE 14 4C 94 CA")

    def test_entire_reachable_bit_lookup_includes_adjacent_bytes_as_data(self):
        self.assert_source(0x7FB5CF,
            "0100020004000800100020004000800000010002000400080010002000400080"
            "20e0c42047cb5a20fbb57a6020bcc42047cbb900003a0ac22029ff008eb116aa"
            "bfcfb57faeb1166020fbb50c33cf4cd3ca20fbb51c33cf4cd3ca20fbb52c33cf"
            "f00382cb144ca9ca20efb51900009900004cbeca20efb549ffff390000990000"
            "4cbeca20efb5390000f00382ae144c94caac1fcfdabb7a5ab42bb9776b29fb99"
            "776ba90099616a7adabb7a4ce8caac1fcfdabb7a5ab42bb9776b090499776b7a"
            "dabb7a4ce8caac1fcf20bcc48508c220b50c8502b50e8508b5108597204cc785"
            "04a9ff7f850ae220dabb7a5ab42bb9776b090499776ba50899ae6ac220a50299")


if __name__ == "__main__":
    unittest.main()
