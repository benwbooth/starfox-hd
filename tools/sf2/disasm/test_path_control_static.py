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
        }
        for address, (semantic, raw) in expected.items():
            command = self.extractor.decode_command(PathAddress(address))
            self.assertEqual(PATH_SEMANTIC_BY_OPCODE[command.opcode].rust_name, semantic)
            self.assertEqual(command.raw_hex, raw)


if __name__ == "__main__":
    unittest.main()
