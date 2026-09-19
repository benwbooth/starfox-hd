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
        for name, address in (("SINTAB", 0x7F3D92), ("COSTAB", 0x7F3DD2)):
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
            0xEE6E: ("IfSelectedDistanceLess", "14e02e74ee"),
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


if __name__ == "__main__":
    unittest.main()
