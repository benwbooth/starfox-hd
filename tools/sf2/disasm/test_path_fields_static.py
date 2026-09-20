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

    def test_script_parameter_is_path_owned_not_a_health_or_height_alias(self):
        # Authored COPY byte stores health, then decrements the saved byte.
        self.assertEqual(self.rom[0x45E1D:0x45E22], bytes.fromhex("4E 27 2D 6F 27"))
        # Another path saves height's low byte before clearing the height.
        self.assertEqual(self.rom[0x47F2A:0x47F2F], bytes.fromhex("4F 27 0E 6C 0E"))
        # That saved byte is a one-based bit selector, not an elapsed timer.
        self.assertEqual(self.rom[0x47F45:0x47F49], bytes.fromhex("67 27 52 7F"))
        self.assertEqual(self.rom[0x47F4C:0x47F4F], bytes.fromhex("D9 27 A1"))
        self.assertEqual(self.rom[0x47F64:0x47F67], bytes.fromhex("D8 27 A1"))

    def test_script_working_word_holds_numeric_constants_or_a_tested_bit_set(self):
        self.assertEqual(self.rom[0x4D1CF:0x4D1D3], bytes.fromhex("0C C8 00 A3"))
        self.assertEqual(self.rom[0x4D20B:0x4D20F], bytes.fromhex("0C 64 00 A3"))
        self.assertEqual(self.rom[0x4601E:0x46022], bytes.fromhex("0C 10 FF A3"))
        # The imported bit set is tested and explicitly cleared on one arm.
        self.assertEqual(self.rom[0x487D3:0x487E0], bytes.fromhex("7B A3 43 6D 27 DA 27 A3 E0 87 6C A3 42"))

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
