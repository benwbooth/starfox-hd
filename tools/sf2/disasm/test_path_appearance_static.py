#!/usr/bin/env python3
"""Animation and sprite contracts from static source bytes, no CPU execution."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathAppearanceStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_visibility_couples_collision_without_touching_draw_admission_or_contact_latches(self):
        self.assert_source(0x7F978B, "B5 23 09 02 95 23 B5 21 09 01 95 21 4C E8 CA")
        self.assert_source(0x7F979A, "B5 23 29 FD 95 23 B5 21 29 FE 95 21 4C E8 CA")
        self.assert_source(0x7F9A96, "B5 21 29 FE 95 21 4C E8 CA")
        self.assert_source(0x7F120C, "B9 08 00 29 E1 09 08 99 08 00 B9 23 00 29 02 F0 04 5C 60 14 7F")

    def test_shadow_and_maximum_draw_distance_commands_only_toggle_their_named_flag(self):
        self.assert_source(0x7FA38F, "B5 20 09 08 95 20 4C E8 CA")
        self.assert_source(0x7FA398, "B5 20 29 F7 95 20 4C E8 CA")
        self.assert_source(0x7FB4DF, "B5 26 29 EF 95 26 4C E8 CA")
        self.assert_source(0x7FB4E8, "B5 26 09 10 95 26 4C E8 CA")

    def test_initialization_forces_manual_selection(self):
        self.assert_source(0x7F8CFD, "20 BC C4 8D B1 16 AD B1 16 09 80 9D CB 1C 4C D3 CA")
        self.assert_source(0x7F8D3A, "20 BC C4 8D B1 16 AD B1 16 09 80 9D CA 1C 4C D3 CA")

    def test_literal_material_assignment_is_a_word_store_then_render_record_copy(self):
        self.assert_source(0x7F89B9,
            "20 04 C5 20 47 CB C2 20 20 20 C7 99 00 00 E2 20 4C A9 CA")
        self.assert_source(0x7FCB47,
            "08 C2 20 8E B1 16 29 FF 00 89 80 00 F0 04 18 69 41 1C "
            "18 6D B1 16 A8 28 60")
        self.assertEqual(0x8C + 0x1C41, 0x1CCD)
        self.assert_source(0x7F1451, "B9 CD 1C 9D 16 00")
        self.assert_source(0x0981D3, "0C 98 84 8C 17 DE 81 0C 04 84 8C")

    def test_scene_material_helper_retains_stack_branch_order_and_full_scene_reads(self):
        self.assert_source(0x0981B6,
            "93 A1 79 A1 E2 1D 8A 2A A1 09 DE 81 79 A1 B5 1B 2A A1 02 DA 81 "
            "2A A1 05 D3 81 17 DE 81 0C 98 84 8C 17 DE 81 0C 04 84 8C 95 A1 42")
        self.assert_source(0x08FFAA,
            "41 B6 81 8D 2B 04 88 EB C3 7F 2B 04 A4 EB C3 7F 2B 04 C0 EB C3 7F "
            "17 C5 7F 00 67 0B 64 2D 5C F7 EF CC 00 1E")
        self.assert_source(0x08F887, "F5 F8 DD AA 7F 0A 0A 00 00 00 00 00 00 01")
        self.assert_source(0x7F9F4F, "20 D2 9F AD B7 16 99 00 00 4C A9 CA")
        self.assert_source(0x7F9FD2,
            "C2 20 20 4C C7 A8 B9 00 00 8D B7 16 E2 20 20 BC C4 20 47 CB 60")
        self.assert_source(0x04B1FC,
            "AE 07 DB BD 04 00 C2 20 29 FF 00 E2 20 8D B5 1B")
        self.assert_source(0x0685E7,
            "AD E2 1D C9 0A 00 90 03 A9 00 00 0A AA BF 5E 9D 06 AA")

    def test_far_sort_flag_publishes_fixed_bias_separately_from_world_position(self):
        self.assert_source(0x7FBCF4, "B5 09 09 01 95 09 4C E8 CA")
        self.assert_source(0x7FBCFD, "B5 09 29 FE 95 09 4C E8 CA")
        self.assert_source(0x7F122C,
            "B9 09 00 89 01 C2 20 F0 05 A9 98 3A 80 03 A9 00 00 9D 02 00 "
            "B9 0C 00 9D 20 00 B9 0E 00 9D 22 00 B9 10 00 9D 24 00 B9 04 00 9D 08 00")

    def test_distance_scenery_complete_loops_and_footprint_helpers(self):
        self.assert_source(0x08FF24, "4E 14 2D 41 B6 81 4F 27 0E 6C 0E 41 52 86 CD 17 3E 7F")
        self.assert_source(0x08FF3E,
            "14 00 02 55 7F 00 29 67 27 52 7F 7A A1 30 D9 27 A1 7F A1 30 16 3E 7F "
            "8A 14 58 02 3E 7F 00 2A 67 27 6A 7F 7A A1 30 D8 27 A1 7F A1 30 16 55 7F")
        self.assert_source(0x098652, "8D 2E 0B 64 2D 0B 04 2E 41 54 8D 5C CC F7 EF 42")
        self.assert_source(0x098D54, "89 B5 24 29 FB 95 24 C2 20 A9 61 8D 6B 42")
        self.assert_source(0x098D62, "89 B5 24 09 04 95 24 C2 20 A9 6F 8D 6B 42")
        self.assert_source(0x7F1BFE, "E4 3A F0 6E B5 31 29 04 00 D0 67 B5 24 29 04 00 D0 60 B4 04")

    def test_targeting_upgrade_gate_acquisition_and_independent_glow(self):
        self.assert_source(0x7FC47D, "AD DD 1D 09 80 8D DD 1D 4C E8 CA")
        self.assert_source(0x7FC488, "AD DD 1D 89 80 F0 04 5C 96 C4 7F 4C BE CA 4C F3 CA")
        self.assert_source(0x07A50A, "AD DD 1D 89 80 D0 04 5C F9 A5 07")
        self.assert_source(0x06A399, "AD DD 1D 48 29 80 4A 85 3A 68 29 40 0A 05 3A 8D DD 1D")
        self.assert_source(0x08F895, "F5 0C F5 E1 81 0A 0A 00 00 00 00 00 00 02")
        self.assert_source(0x0981E1, "5C 1D 02 78 1D 03 16 E2 81")

    def test_scenery_mask_transfer_is_byte_wide_even_though_import_helper_reads_a_word(self):
        self.assert_source(0x7F9F5B, "20 E7 9F AD B7 16 99 00 00 4C BE CA")
        self.assert_source(0x7F9FE7,
            "20 E0 C4 C2 20 29 FF 00 A8 B9 5C D7 8D B7 16 E2 20 20 BC C4 20 47 CB 60")
        self.assert_source(0x7F9F9D, "20 BF 9F 29 FF 00 A8 E2 20 AD B7 16 99 5C D7 4C BE CA")
        self.assert_source(0x7F9FBF, "20 BC C4 20 47 CB C2 20 B9 00 00 8D B7 16 20 4C C7 A8 60")
        self.assert_source(0x04B1C8, "C2 30 9C 86 D7 9C 88 D7 9C 8A D7 9C 8C D7")

    def test_zero_frame_short_form_is_manual_shape_initialization_not_automatic_animation(self):
        self.assert_source(0x7FC3CF, "A9 00 09 80 9D CB 1C 4C E8 CA")

    def test_both_channels_add_then_correct_once_not_modulo(self):
        operands = "20 BC C4 8D B1 16 20 E0 C4 8D B7 16"
        arithmetic = "18 6D B1 16 30 04 18 6D B7 16 29 7F CD B7 16 90 04 38 ED B7 16 09 80"
        self.assert_source(0x7F8D0E, f"{operands} BD CB 1C {arithmetic} 9D CB 1C 4C BE CA")
        self.assert_source(0x7F8D4B, f"{operands} BD CA 1C {arithmetic} 9D CA 1C 4C BE CA")

    def test_presentation_uses_clock_only_when_control_sign_bit_is_clear(self):
        self.assert_source(0x7F1406, "B9 CB 1C 30 02 A5 C4 29 7F 9D 19 00 B9 CA 1C 30 02 A5 C4 29 7F 9D 1A 00")

    def test_sprite_sets_actor_flag_and_two_existing_extension_channels(self):
        self.assert_source(0x7F99D5, "20 BC C4 8D B1 16 20 E0 C4 8D B3 16 B5 20 09 20 95 20 AD B1 16 9D C8 1C AD B3 16 9D DA 1C 4C BE CA")

    def test_effect_collision_disable_is_the_existing_collision_queue_gate(self):
        self.assert_source(0x7F9A9F, "B5 21 09 01 95 21 4C E8 CA")
        self.assert_source(0x7F32CE, "B5 21 29 01 00 D0 60")


if __name__ == "__main__":
    unittest.main()
