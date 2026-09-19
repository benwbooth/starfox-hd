#!/usr/bin/env python3
"""Assembly-backed checks for native path control; no gameplay execution."""

from pathlib import Path
import re
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM
from extract_path import PathAddress, PathExtractor
from path_semantics import PATH_SEMANTIC_BY_OPCODE


ROOT = Path(__file__).resolve().parents[3]


@unittest.skipUnless(Path(DEFAULT_ROM).is_file(), "retail SF2 ROM is not present")
class PathControlStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()
        cls.extractor = PathExtractor(cls.rom)

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_forward_projection_host_rotation_order_and_byte_inputs(self):
        self.assert_source(0x0DB751, "E2 20 64 02 64 08 A9 7F 85 97")
        self.assert_source(0x0DB7C0, "B5 16 22 F0 3B 7F")
        self.assert_source(0x0DB7D2, "B5 12 22 4E 3A 7F")
        self.assert_source(0x0DB7E4, "B5 14 22 A9 38 7F")
        # Each rotated byte becomes the high byte of a signed coefficient.
        self.assert_source(0x0DB810, "C2 20 B9 0C 00 EB 29 00 FF 8F 68 00 70")
        self.assert_source(0x0DB821, "B9 0E 00 EB 29 00 FF 8F 2C 00 70")
        self.assert_source(0x0DB830, "B9 10 00 EB 29 00 FF 8F 2E 00 70")
        self.assert_source(0x0DB863, "A9 01 A2 86 FC 22 7B 78 7F")

    def test_geometry_dot_product_doubles_each_word_before_signed_high_product(self):
        # Three ADD-self/FMUL pairs, separate high-word truncations, and
        # wrapping word sums. A combined wide dot product is not equivalent.
        self.assert_source(
            0x01FC86,
            "60 3F DF 3E DF 3D A0 13 3D A6 34 50 11 9F "
            "3D A0 14 3D A6 16 50 9F 11 51 "
            "3D A0 15 3D A6 17 50 9F 51 3E A0 55 00 01",
        )

    def test_square_root_uses_sixteen_word_remainder_iterations(self):
        self.assert_source(
            0x01FA64,
            "A6 00 A8 00 AC 10 FD 6E FA 25 55 24 04 28 04 "
            "25 55 24 04 28 04 26 56 B6 17 56 B7 68 0D 04 "
            "28 3D 67 D6 3C 25 9B 01",
        )

    def test_normalization_division_preconditions_sign_bits_and_bounds_remainder(self):
        self.assert_source(
            0x01FBAB,
            "20 B0 0B 06 01 26 B6 0A 06 01 26 03 03 24 97 "
            "AC 10 24 54 04 2F 1D 66 0C 07 01 24 04 3C 04 "
            "9B 03 56 24 54 3C 04 9B 03",
        )
        self.assert_source(
            0x01FC0D,
            "26 10 A9 00 03 09 04 01 05 FA D9 60 F4 FF 7F "
            "2C B9 09 07 01 2F 1D 24 54 3C 04 26 16 94 FF AB FB 01 24 16",
        )
        self.assert_source(
            0x01FC70,
            "3D A6 34 26 56 B1 9F 3E A0 13 B2 9F 3E A0 14 B3 9F 3E A0 15 00 01",
        )

    def test_shared_byte_trig_tables_match_the_sf2_rotation_callees(self):
        source = (ROOT / "rust/sf-core/src/snes_trig.rs").read_text()
        for name, address in (
            ("SINTAB", 0x7F3D92), ("COSTAB", 0x7F3DD2),
            ("SINTAB", 0x008E26), ("COSTAB", 0x008E66),
        ):
            match = re.search(rf"pub static {name}: \[i8; 256\] = \[(.*?)\];", source, re.S)
            self.assertIsNotNone(match)
            values = [int(value.strip()) & 255 for value in match[1].split(",") if value.strip()]
            self.assertEqual(len(values), 256)
            offset = source_offset(address)
            self.assertEqual(bytes(values), self.rom[offset:offset + 256])

    def test_crossing_trigger_latches_and_tests_both_players_in_priority_order(self):
        self.assert_source(0x7F9C0C, "B5 24 29 40 F0 04 5C 52 9C 7F")
        self.assert_source(0x7F9C16, "B5 24 09 40 95 24 AC C3 12")
        self.assert_source(0x7F9C37, "AC C5 12 F0 16 22 51 B7 0D")
        self.assert_source(0x7F9C52, "AC C3 12 F0 1F 22 51 B7 0D 89 80")
        self.assert_source(0x7F9C76, "AC C5 12 F0 1F 22 51 B7 0D 89 80")
        self.assert_source(0x7F9CA9, "B5 24 29 BF 95 24")
        self.assert_source(0x7F9CCA, "B5 24 29 BF 95 24")
        # Callback selection also changes which player later path ops target.
        self.assert_source(0x7F9CBA, "AC C3 12 8C 1F CF B5 24 29 7F 95 24")
        self.assert_source(0x7F9CDB, "AC C5 12 8C 1F CF B5 24 09 80 95 24")

    def test_loop_wait_and_periodic_clock_branch_instructions(self):
        self.assert_source(0x7F96C4, "B9 61 6A 3A F0 11 99 61 6A")
        self.assert_source(0x7F84FB, "20 BC C4 D5 17 D0 04 5C CF CA 7F F6 17 4C DE 9D")
        self.assert_source(0x7FCACF, "A9 00 95 17")
        self.assert_source(0x7F9D46, "A9 01 80 18 A9 03 80 14 A9 07 80 10")
        self.assert_source(0x7F9D52, "A9 0F 80 0C A9 1F 80 08 A9 3F 80 04 A9 7F 80 00")
        self.assert_source(0x7F9D62, "25 C4 F0 02 80 20")

    def test_hostile_laser_authored_control_flow(self):
        expected = {
            0xEE4C: ("SetByte", "0b0a2d"),
            0xEE4F: ("ImportByteIndexed", "7a2e96"),
            0xEE52: ("IfSameByte", "2a2e0068ee"),
            0xEE57: ("IfSameByte", "2a2e0262ee"),
            0xEE5C: ("SetByte", "0b032e"),
            0xEE62: ("SetByte", "0b042e"),
            0xEE68: ("SetByte", "0b022e"),
            0xEE6E: ("IfSelectedDistanceLess", "14e02e74ee"),
            0xEE73: ("End", "0f"),
            0xEE74: ("QueueSelectedMarkerClass2", "fa72"),
            0xEE81: ("SetVelocity", "061e"),
            0xEE9B: ("SetVelocity", "063f"),
            0xEE9D: ("IfSelectedDistanceLess", "14e803adee"),
            0xEEA2: ("RotateAroundSelectedPitch", "ae7f"),
            0xEEA4: ("RotateAroundSelectedPitch", "ae7f"),
            0xEEA6: ("RotateAroundSelectedPitch", "ae7f"),
            0xEEA8: ("FaceSelectedImmediate", "000f"),
            0xEEAA: ("Goto", "169dee"),
            0xEEAD: ("FaceSelectedImmediate", "000f"),
            0xEEAF: ("SetVelocity", "063f"),
            0xEEB1: ("GotoImmediate", "17bcee"),
            0xEEBC: ("ScheduleTrigger", "4ad5ee01"),
            0xEEC0: ("ScheduleTrigger", "4ac8ee0d"),
            0xEEC4: ("DoQueue", "6128"),
            0xEEC6: ("Next", "44"),
            0xEEC7: ("End", "0f"),
            0xEEC8: ("ForceTriggerPath", "4cccee"),
            0xEECC: ("CancelTrigger", "4bd5ee"),
            0xEECF: ("CancelTrigger", "4bc8ee"),
            0xEED2: ("Wait", "030f"),
            0xEED4: ("End", "0f"),
            0xEED5: ("IfSelectedWithinYawArc", "a420daee"),
            0xEEDA: ("FaceSelectedSmooth", "09"),
            0xEF18: ("DoVariableByte", "620a"),
            0xEF1A: ("Next", "44"),
            0xEF1B: ("End", "0f"),
            0xEF1C: ("IfHitGround", "1a000026ef"),
            0xEF21: ("IfCurrentAtOrAboveCollisionTarget", "005326ef"),
            0xEF26: ("ForceTriggerPath", "4c1bef"),
            0xF018: ("ForceTriggerPath", "4c1cf0"),
            0xF01B: ("Return", "42"),
            0xF01C: ("End", "0f"),
        }
        for address, (semantic, raw) in expected.items():
            command = self.extractor.decode_command(PathAddress(address))
            self.assertEqual(PATH_SEMANTIC_BY_OPCODE[command.opcode].rust_name, semantic)
            self.assertEqual(command.raw_hex, raw)

    def test_movement_acceleration_and_bank_rounding(self):
        self.assert_source(
            0x7F9DF0,
            "B5 18 D5 0A 30 0B 38 F5 0B D5 0A 10 13 B5 0A 80 09 "
            "18 75 0B D5 0A 30 08 B5 0A 95 18 74 0B 80 02 95 18",
        )
        self.assert_source(
            0x7F9E29,
            "B5 16 C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 14 95 14",
        )

    def test_ordinary_integration_precedes_callbacks_but_attached_motion_follows(self):
        self.assert_source(0x7F9E62, "22 24 2C 7F")
        self.assert_source(0x7F9E70, "20 A8 9A")
        self.assert_source(
            0x7F2C24,
            "C2 20 B5 0C 18 75 32 95 0C B5 0E 18 75 34 95 0E "
            "B5 10 18 75 36 95 10 E2 20 6B",
        )
        self.assert_source(0x7F9E9F, "C2 20 BD CF 1C 18 7D 32 00 9D CF 1C")
        self.assert_source(0x7F9EAD, "C2 20 BD D1 1C 18 7D 34 00 9D D1 1C")
        self.assert_source(0x7F9EBB, "C2 20 BD D3 1C 18 7D 36 00 9D D3 1C")
        # The forced-path post-callback hook has no additional side effects.
        self.assert_source(0x7FAFFC, "60")

    def test_selected_displacement_skips_only_horizontal_axis(self):
        self.assert_source(0x7F9F30, "89 04 F0 04 5C 42 9F 7F")
        self.assert_source(0x7F9F38, "C2 20 AD 1C 1E 18 75 0C 95 0C")
        self.assert_source(0x7F9F42, "C2 20 AD 20 1E 18 75 10 95 10 E2 20 60")

    def test_direction_velocity_keeps_doubled_byte_products_and_world_scaling(self):
        self.assert_source(0x7F3078, "AD 12 15 49 FF 1A A8 AD 11 15 AA")
        self.assert_source(0x7F30A0, "A5 85 30 18 0A 8F 02 42 00")
        self.assert_source(0x7F30BC, "49 FF 1A 0A 8F 02 42 00")
        self.assert_source(0x7F30F5, "A5 02 30 18 0A 8F 02 42 00")
        self.assert_source(0x7F85FC, "C2 20 16 32 16 32 16 34 16 34 16 36 16 36 E2 20 60")

    def test_selected_distance_uses_wrapped_horizontal_deltas_and_geometry_length(self):
        self.assert_source(0x7F8C30, "B9 0C 00 38 F5 0C 8F 26 00 70")
        self.assert_source(0x7F8C3A, "A9 00 00 8F 28 00 70")
        self.assert_source(0x7F8C41, "B9 10 00 38 F5 10 8F 2A 00 70")
        self.assert_source(0x7F8C50, "A9 01 A2 72 FB 22 7B 78 7F")

    def test_smooth_face_clamps_small_differences_then_rounds_two_halves(self):
        self.assert_source(
            0x7F87DB,
            "38 F5 12 C9 00 30 08 C9 04 10 0A A9 04 80 06 C9 FC 30 02 A9 FC "
            "C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 12",
        )
        self.assert_source(0x7F880A, "49 FF 1A D5 14")
        self.assert_source(0x7FAB69, "85 97 06 97 18 75 14 18 65 02 C5 97 B0 04")

    def test_hit_callback_and_surface_comparisons(self):
        self.assert_source(0x7F9D38, "FA DA B5 22 29 02 D0 04 5C 88 9D 7F")
        self.assert_source(0x7FBF9C, "C2 20 B5 0E 38 E5 08 30 03 4C F3 CA")
        self.assert_source(0x7F8CA3, "C2 20 20 20 C7 18 75 0E")

    def test_collision_rotation_scales_input_words_and_preserves_product_carries(self):
        self.assert_source(0x01FD6D, "3D A0 34 50 11 50 3D A0 17 50 13 50")
        self.assert_source(0x01FE78, "F0 66 8E 1E 52 EF 15 4D F0 26 8E 1E 52 EF 19 4D")
        self.assert_source(
            0x01FE88,
            "21 16 B9 18 3D 9F 24 17 23 16 B5 3D 9F 27 54 3D 58 12 96 "
            "23 16 B9 18 3D 9F 24 17 21 16 B5 3D 9F 27 64 3D 68 13 96",
        )

    def test_collision_polygon_uses_full_signed_cross_product_and_inclusive_edges(self):
        self.assert_source(
            0x01FD2F,
            "B9 13 67 BA 15 68 B1 17 67 B2 18 68 28 16 23 3D 9F "
            "24 18 25 16 27 3D 9F 24 15 25 68 27 3D 63 0B 0B",
        )
        self.assert_source(0x01FD50, "29 17 2A 18 3C 01 A0 00 05 03 01 A0 FF 3E A0 0B 00 01")

    def test_collision_plane_truncates_before_doubling_and_signed_division(self):
        self.assert_source(
            0x01FA39,
            "3D A0 13 3D A6 55 9F 11 50 3D A0 15 3D A6 57 9F 50 11 51 "
            "3D A0 14 21 60 21 15 A4 00 3D A6 56 94 FF 8A FA 01 24 96 3E A4 14",
        )
        self.assert_source(
            0x01FA8A,
            "02 B5 17 3D C6 60 26 B6 0A 02 16 66 25 B5 0A 05 "
            "14 64 15 3D 65 24 54 B5 04 AC 10 2F 1D 66 0C 08 01 "
            "24 04 3C 04 05 06 01 56 24 54 3C 04 27 B7 0A 03 24 4F D4 9B 01",
        )

    def test_surface_admission_uses_no_extra_footprint_margin(self):
        self.assert_source(0x0DB07E, "BD 05 00 4A 18 7D 01 00 38 E5 02 DD 05 00 B0 10")
        self.assert_source(0x0DB08E, "BD 07 00 4A 18 7D 03 00 38 E5 97 DD 07 00 90 09")
        # The +2 is only the later vertical admission margin.
        self.assert_source(0x0DB157, "A5 08 38 79 0C 00 1A D5 0E 10 03 82 6D 00")
        self.assert_source(0x0DB165, "A5 08 CD 5D 19 30 03 82 63 00")
        # Both failed and accepted surface candidates resume at the NEXT
        # OBJECT link, not the compound-group loop at $B09E.
        self.assert_source(0x0DB1D8, "C2 20 AC 49 19 B6 00 F0 14")
        self.assert_source(0x0DB1FE, "AD 5D 19 C9 00 20 D0 03 A9 00 00 85 08")

    def test_world_occupancy_quantization_and_bit_masks(self):
        self.assert_source(0x00B063, "01 00 02 00 04 00 08 00 10 00 20 00 40 00 80 00")
        self.assert_source(0x0DDAD1, "A5 02 EB 4A 29 7F 00 85 5F")
        self.assert_source(0x0DDAED, "A5 97 29 00 FE EB 0A 0A 0A 18 65 5F 85 5F")
        self.assert_source(0x0DDAFB, "A5 04 18 69 FF 01 EB 4A 29 7F 00 85 04")
        self.assert_source(0x0DDB08, "A5 E4 18 69 FF 01 EB 4A 29 7F 00 85 E4")
        self.assert_source(0x0DDB75, "A5 02 EB 4A 29 7F 00 85 5F")
        self.assert_source(0x0DDB91, "A5 97 29 00 FE EB 0A 0A 0A 18 65 5F AA")
        self.assert_source(0x0DDBA0, "BD 36 CF 25 0A 85 02")

    def test_world_marker_half_row_boundary_and_countdown_widths(self):
        # At every eighth BYTE boundary, subtract a full 16-byte row.
        self.assert_source(
            0x0DDB36,
            "18 26 0A 90 10 26 0A E8 C2 20 8A 89 07 00 D0 05 38 E9 10 00 AA",
        )
        # Width decrements in byte mode; depth decrements in word mode.
        self.assert_source(0x0DDB4B, "E2 20 C6 04 D0 CF")
        self.assert_source(0x0DDB59, "A5 5F 18 69 10 00 C9 00 08 90 04 38 E9 00 08 85 5F C6 E4 D0 A7")
        # Selected auxiliary exemption short-circuits the occupancy test.
        self.assert_source(0x7FB745, "B4 2B B9 EB 6B 7A DA BB 7A 89 80 F0 04 5C BE CA 7F")

    def test_shared_allocator_marks_effects_only_after_consuming_final_slot(self):
        self.assert_source(0x7F2925, "C2 20 9B AE AA 12 D0 06 E2 20 BB 4C 69 29")
        self.assert_source(0x7F295D, "DA AE AA 12 D0 03 20 79 29 FA 38 6B")
        self.assert_source(0x7F2979, "DA 5A AE A8 12 B4 00 F0 37 5A B5 20 29 10")
        self.assert_source(
            0x7F298D,
            "C2 20 B5 04 C9 98 BD F0 16 C9 B4 BD F0 11 C9 D0 BD F0 0C "
            "C9 EC BD F0 07 C9 08 BE F0 02 80 08 E2 20 B5 25 09 08 95 25",
        )
        source = (ROOT / "rust/sf2-data/src/shape_data.rs").read_text()
        for index, shape in enumerate((0xBD98, 0xBDB4, 0xBDD0, 0xBDEC, 0xBE08), start=9):
            self.assertRegex(source, rf"header_index: {index},\s+shape_id: 0x{shape:04X},")
        # The new allocation is cleared after the sweep, including its
        # retirement flag, before the requested shape is installed.
        self.assert_source(0x7F2A1C, "22 25 29 7F")
        self.assert_source(0x7F2A2E, "22 BC 29 7F C2 20 A5 5F 99 04 00")
        self.assert_source(0x7F29C4, "A9 00 5A DA A0 3B 00 95 04 E8 88 D0 FA")

    def test_weapon_allocation_temporarily_changes_pressure_head_and_clears_bank(self):
        self.assert_source(0x0DE019, "AD A8 12 48 E2 20 8E A8 12")
        self.assert_source(0x0DE02B, "22 17 2A 7F B0 04 5C 9A E0 0D")
        self.assert_source(0x0DE035, "C2 20 68 8D A8 12")
        self.assert_source(0x03AB2A, "22 D2 2B 7F A9 00 99 16 00")
        self.assert_source(0x03AC1D, "B5 18 99 17 00 96 1C 94 1C 6B")

    def test_strategy_epoch_increments_word_clock_once_and_saves_live_cursor(self):
        self.assert_source(0x7F3504, "E6 C4 D0 02 E6 C5")
        self.assert_source(0x7F350E, "AE A8 12 AF 30 30 00 29 20 F0 25")
        self.assert_source(0x7F3519, "B5 26 29 40 F0 04 5C 2F 35 7F")
        self.assert_source(0x7F3523, "9C D6 12 22 96 35 7F AD D6 12 D0 07")
        self.assert_source(0x7F3536, "B4 00 22 56 33 7F 80 F3 8E 42 19")
        # Remainder loads the saved next actor and never increments the clock.
        self.assert_source(0x7F3560, "AE 42 19 F0 25 B5 26 29 40 F0 04 5C 7B 35 7F")
        self.assert_source(0x7F356F, "9C D6 12 22 96 35 7F AD D6 12 D0 07")
        self.assert_source(0x7F3582, "B4 00 22 56 33 7F 80 F3 8E 42 19")

    def test_strategy_selection_death_hit_pause_and_post_service_precedence(self):
        self.assert_source(0x7F35A8, "EC D6 14 D0 03 82 A0 00")
        self.assert_source(0x7F35B5, "B5 2D D0 35 B5 31 29 04 F0 04 5C EE 35 7F")
        self.assert_source(0x7F35C3, "B5 26 29 08 F0 04 5C DC 35 7F")
        self.assert_source(0x7F35CD, "C2 20 AD 84 1B 89 02 00 E2 20 F0 03 4C EE 35")
        self.assert_source(0x7F35DC, "A9 03 8D D1 12 C2 20 A9 55 A0 8D CF 12 82 4F 00")
        self.assert_source(0x7F35EE, "B5 31 29 FB 95 31 B5 20 29 80 F0 18")
        self.assert_source(0x7F35FA, "A9 03 8D D1 12 C2 20 A9 27 A3 8D CF 12 82 31 00")
        self.assert_source(0x7F3619, "B5 19 F0 59")
        self.assert_source(0x7F3622, "B5 26 29 08 F0 04 5C 3B 36 7F")
        self.assert_source(0x7F362C, "C2 20 AD 84 1B 89 02 00 E2 20 F0 03 4C 50 36")
        self.assert_source(0x7F3661, "AD D3 1C 89 01 D0 0B BD CC 1C F0 06 A0 3F 03 20 B8 36")

    def test_retirement_detaches_children_then_clears_all_incoming_links(self):
        self.assert_source(0x7F336A, "22 25 34 7F 22 B2 33 7F 22 4F 34 7F C2 20 22 CB 34 7F")
        self.assert_source(0x7F3470, "B9 29 00 F0 17 C5 3C F0 03 A8 80 F4 B5 29 99 29 00")
        self.assert_source(0x7F34A4, "B4 29 B5 23 29 FB 95 23 C2 20 A9 00 00 95 06")
        self.assert_source(0x7F34B5, "B5 25 29 01 D0 04 5C C5 34 7F B5 25 09 08 95 25")
        self.assert_source(0x7F34CB, "DA 8A AE A8 12 D5 1C D0 02 74 1C D5 06 D0 02 74 06 B4 00 BB D0 EF")
        # Only after dependent services/link cleanup does the slot return to
        # the free-list head, preserving last-freed-first reuse.
        self.assert_source(0x7F339D, "AD AA 12 95 00 8E AA 12")

    def test_cleanup_samples_live_flags_and_saves_next_before_retirement(self):
        self.assert_source(0x7F4037, "AE A8 12 B5 25 29 08 D0 04 5C 54 40 7F")
        self.assert_source(0x7F4044, "C2 20 B5 00 48 E2 20 22 46 33 7F 7A BB 4C B3 40")
        self.assert_source(0x7F40B0, "9B B6 00 D0 85")

    def test_weapon_muzzle_uses_source_bank_then_pitch_yaw_and_word_scale(self):
        self.assert_source(0x03AB2A, "22 D2 2B 7F A9 00 99 16 00")
        self.assert_source(0x03AB6C, "B5 16 22 F0 3B 7F A5 04 85 02 A5 0A 85 08 A5 E4 85 97")
        self.assert_source(0x03AB7E, "B5 12 22 4E 3A 7F A5 04 85 02 A5 0A 85 08 A5 E4 85 97")
        self.assert_source(0x03AB90, "B5 14 22 A9 38 7F C2 20 A5 04 0A 0A 85 04 A5 0A 0A 0A 85 0A A5 E4 0A 0A 85 E4")
        self.assert_source(0x03ABE0, "22 C3 21 7F 18 6D B7 14 95 12 22 EB 21 7F 49 FF 1A 18 6D B6 14 95 14")
        self.assert_source(0x03ABFC, "B9 12 00 18 6D B7 14 99 12 00 B9 14 00 18 6D B6 14 99 14 00")
        self.assert_source(0x7F8B8B, "B5 25 09 08 95 25 4C FD 9E")

    def test_sound_queue_producers_wrap_sixteen_words_without_testing_full(self):
        self.assert_source(0x7FA43E, "DA AE 16 1D CC C3 12 F0 08 C0 3F 03 F0 03 09 00 80")
        self.assert_source(0x7FA44F, "9D F6 1C E2 20 AD 16 1D 1A 1A 29 1F 8D 16 1D FA 60")
        self.assert_source(0x7F6E09, "DA AE 16 1D 9D F6 1C E2 20 8A 1A 1A 29 1F 8D 16 1D C2 20 FA 6B")
        self.assert_source(0x7F0FA5, "AE 18 1D EC 16 1D F0 23 BD F6 1C")
        self.assert_source(0x7F0FBB, "AD 18 1D 1A 1A 29 1F 8D 18 1D")

    def test_path_sound_distance_branches_skip_angle_for_far_and_distance_only(self):
        self.assert_source(0x7FA4CE, "E0 20 03 90 27 80 32")
        self.assert_source(0x7FA4D5, "AD 34 1C E0 20 03 90 0E E0 14 05 90 07 09 60 8D 34 1C 80 11")
        self.assert_source(0x7FA4E9, "09 30 8D 34 1C AD 35 1C 3A F0 06 AE 37 1C 20 50 A5")
        self.assert_source(0x7F8C3A, "A9 00 00 8F 28 00 70 B9 10 00 38 F5 10 8F 2A 00 70")
        self.assert_source(0x7F8C50, "A9 01 A2 72 FB 22 7B 78 7F")

    def test_path_sound_bearing_uses_live_listener_and_half_open_rear_rejection(self):
        self.assert_source(0x7FA565, "22 58 1D 7F E2 20 EB 38 F9 15 00 C9 10 90 40 C9 F0 B0 3C")
        self.assert_source(0x7FA578, "48 AD 35 1C C9 03 F0 0B 68 C9 70 90 1B C9 90 90 2B 80 20")
        self.assert_source(0x7FA58B, "68 C9 40 90 10 C9 C0 B0 17 C2 20 A9 FF FF 8D 33 1C")
        self.assert_source(0x7FA5A0, "AD 34 1C 18 69 20 8D 34 1C 80 09 AD 34 1C 18 69 10 8D 34 1C")


    def test_contact_pool_capacity_lookup_insertion_and_refresh(self):
        self.assert_source(0x03A60D, "A9 9A 31 8D 81 12 A0 3C 00 AA 18 69 0B 00 9D 00 00 88 D0 F5 9E 00 00")
        self.assert_source(0x7F3ED9, "BC 1E 00 F0 0C BB DD 04 00 F0 5D BC 00 00 BB D0 F5")
        self.assert_source(0x7F3EF2, "AE 81 12 D0 04 BB 4C 55 3F")
        self.assert_source(0x7F3F10, "B9 00 00 9D 00 00 8A 99 00 00 98 9D 02 00")
        self.assert_source(0x7F3F34, "E2 20 A9 02 9D 09 00 9E 08 00 9E 0A 00 E2 20 BD 09 00 09 01 9D 09 00 FE 0A 00")

    def test_contact_callbacks_precede_reverse_then_original_lifo_release(self):
        self.assert_source(0x7F33BD, "B4 1E F0 0B B9 00 00 48 22 A7 3F 7F 7A D0 F5 B5 1E F0 04 22 06 80 00")
        self.assert_source(0x7F3FA7, "DA 5A 20 59 3F BE 04 00 B9 06 00 A8 20 59 3F")
        self.assert_source(0x7F3FE0, "AD 81 12 9D 00 00 8E 81 12 FA A5 08 95 1E 7A FA")
        self.assert_source(0x7F401A, "AD 81 12 9D 00 00 8E 81 12 FA A5 08 95 1E 6B")
        self.assert_source(0x7F4090, "B9 00 00 48 B9 09 00 89 01 00 D0 06 22 A7 3F 7F 80 09 E2 20 29 FE 99 09 00 C2 20 7A D0 E2")
        self.assert_source(0x7F4878, "A5 F5 99 08 00 A5 F6 9D 08 00")


    def test_hit_response_new_callback_replaces_continuing_and_parameter_is_other_actor_byte(self):
        self.assert_source(0x03A3B2, "B9 09 00 29 02 F0 42 B9 09 00 29 FD 99 09 00")
        self.assert_source(0x03A3D1, "A9 06 22 3B 23 7F C0 00 00 D0 04 5C FB A3 03")
        # The registered new handler returns directly to damage, not A3FB.
        self.assert_source(0x03A3F0, "7A 5A A9 03 48 F4 2C A4 DC CF 12")
        self.assert_source(0x03A3FB, "7A 5A A9 05 22 3B 23 7F")
        # Restore OTHER ACTOR from stack before loading its auxiliary byte.
        # This is not the similarly offset touch-count byte of the contact.
        self.assert_source(0x03A41C, "7A 5A B9 0A 00 8D 30 CF A9 03 48 F4 2C A4 DC CF 12")
        self.assert_source(0x03A42D, "7A B5 2D 38 ED 2F CF 95 2D 10 04 A9 00 95 2D")

    def test_hit_response_mutual_exemption_next_after_callback_and_pause_gate(self):
        self.assert_source(0x03A327, "B5 25 29 10 F0 04 5C 4C A4 03")
        self.assert_source(0x03A349, "B5 24 29 08 F0 04 5C 59 A3 03 B5 20 09 02 95 20")
        self.assert_source(0x03A38C, "B5 31 29 80 C9 80 D0 04 5C 3C A4 03")
        self.assert_source(0x03A398, "B9 2E 00 8D 2F CF B9 31 00 29 01 C9 01 F0 04 5C AE A3 03 9C 2F CF")
        self.assert_source(0x03A43C, "AC 2D CF C2 20 B9 00 00 A8 E2 20 F0 03 82 ED FE")
        self.assert_source(0x03A44C, "B5 26 29 08 F0 04 5C 66 A4 03 C2 20 AD 84 1B 89 02 00 E2 20 D0 03 4C 66 A4 6B 5C 8F 2B 7F")


    def test_collision_box_rotation_selector_and_zero_axis_bypasses(self):
        self.assert_source(0x7F4133, "89 F0 D0 03 82 6B 02 89 10 F0 03 82 E2 01 89 20 F0 03 82 58 01 89 40 F0 03 82 CE 00")
        self.assert_source(0x7F415D, "BF 16 00 00 F0 04 22 F0 3B 7F")
        self.assert_source(0x7F4172, "BF 12 00 00 F0 04 22 4E 3A 7F")
        self.assert_source(0x7F4184, "BF 14 00 00 F0 04 22 A9 38 7F")
        self.assert_source(0x7F4293, "B9 03 00 18 7F 0C 00 00 85 73")
        self.assert_source(0x7F4316, "B9 05 00 18 7F 0E 00 00 85 7B")
        # Candidate-side full rotation has the same individual bypasses.
        self.assert_source(0x7F456A, "B9 16 00 F0 04 22 F0 3B 7F")
        self.assert_source(0x7F457F, "B9 12 00 F0 04 22 4E 3A 7F")
        self.assert_source(0x7F4590, "B9 14 00 F0 04 22 A9 38 7F")

    def test_collision_box_animation_mask_and_word_absolute_overlap(self):
        self.assert_source(0x7F3302, "B9 08 00 9F 44 2F 7E B9 0A 00 9F 46 2F 7E B9 0C 00 9F 48 2F 7E B9 0E 00 9F 4A 2F 7E")
        self.assert_source(0x7F4100, "B9 02 00 F0 29 3A 85 02 BF CB 1C 7E 10 06 25 02 F0 1C 80 06 A5 02 25 C4 F0 14")
        self.assert_source(0x7F411A, "C2 20 29 7F 00 85 02 98 A4 02 18 69 12 00 88 D0 FA")
        self.assert_source(0x7F489A, "BD 4A 2F 18 65 5C 8D DE 12 A5 3E 38 E5 75 10 04 49 FF FF 1A 38 ED DE 12 30 03 4C 38 49")
        self.assert_source(0x7F48B7, "BD 46 2F 18 65 58 8D DE 12 A5 3A 38 E5 73 10 04 49 FF FF 1A 38 ED DE 12 30 03 4C 38 49")
        self.assert_source(0x7F4824, "E2 20 BF 10 00 7F 05 F5 85 F5 C2 20 BF 00 00 7F AA F0 03 82 C9 FC E2 20 A5 F5 F0 57")


if __name__ == "__main__":
    unittest.main()
