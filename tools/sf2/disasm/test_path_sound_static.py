#!/usr/bin/env python3
"""Static-only provenance for decoded authored cue enqueue operations."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathSoundStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_direct_cue_zeroes_parameter_and_pair_reads_two_literals(self):
        self.assert_source(0x7FA412, "20 BC C4 8D 31 1C 9C 32 1C AC 1F CF 20 39 A4 4C D3 CA")
        self.assert_source(0x7FA424, "20 BC C4 8D 31 1C 20 E0 C4 8D 32 1C AC 1F CF 20 39 A4 4C BE CA")

    def test_routing_ors_secondary_except_for_primary_actor_and_fixed_fallback(self):
        self.assert_source(0x7FA439, "C2 20 AD 31 1C DA AE 16 1D CC C3 12 F0 08 C0 3F 03 F0 03 09 00 80 9D F6 1C E2 20 AD 16 1D 1A 1A 29 1F 8D 16 1D FA 60")

    def test_literal_readers_do_not_sample_actor_fields(self):
        for address, offset in ((0x7FC4BC, 1), (0x7FC4E0, 2)):
            self.assert_source(address, f"84 79 A0 {offset:02X} 00 A5 5E 29 EF 85 5E 8F 3A 30 00 B7 F9 8D 11 19 A5 5E 09 10 85 5E 8F 3A 30 00 AD 11 19 A4 79 60")

    def test_advances_preserve_wait_and_continue_immediately(self):
        for address, count in ((0x7FCAD3, 2), (0x7FCABE, 3)):
            self.assert_source(address, f"E2 20 C2 20 B5 2B 18 69 {count:02X} 00 95 2B E2 20 4C 75 7E")

    def test_marker_selection_uses_selected_view_side_not_path_owner_selection(self):
        self.assert_source(0x7FA3FB, "AC 1F CF B9 23 00 29 40 F0 04 5C 0E A4 7F A0 3F 03 80 03 A0 7E 03 60")

    def test_banded_handlers_read_one_literal_and_resolve_fixed_marker(self):
        self.assert_source(0x7FA472, "20 BC C4 8D 31 1C A9 01 8D 35 1C 20 FB A3 20 AE A4 4C D3 CA 20 BC C4 8D 31 1C A9 02 8D 35 1C 20 FB A3 20 AE A4 4C D3 CA")

    def test_banded_core_retains_thresholds_far_angle_bypass_and_suppression(self):
        self.assert_source(0x7FA4AE, "08 DA AD 31 1C 8D 33 1C 9C 34 1C 8E 37 1C 20 25 8C C2 20 AD B5 16 AA E2 20 AD 35 1C F0 02 80 07 E0 20 03 90 27 80 32 AD 34 1C E0 20 03 90 0E E0 14 05 90 07 09 60 8D 34 1C 80 11 09 30 8D 34 1C AD 35 1C 3A F0 06 AE 37 1C 20 50 A5 C2 20 AD 33 1C C9 FF FF F0 03 20 3E A4 FA 28 60")

    def test_range_limited_entries_use_wrapped_sign_comparison_without_distance_attenuation(self):
        self.assert_source(0x7FA50A, "5A A0 00 14 80 04 5A A0 20 03 20 BC C4 8D 31 1C 9C 35 1C 8C 37 1C 20 FB A3 20 2A A5 7A 4C D3 CA 08 AD 31 1C 8D 33 1C 9C 34 1C 20 25 8C C2 20 AD B5 16 CD 37 1C 10 0D E2 20 20 50 A5 C2 20 AD 33 1C 20 3E A4 28 60")

    def test_angle_uses_marker_bearing_byte_and_adds_stereo_parameter(self):
        self.assert_source(0x7FA550, "08 C2 20 BD 0C 00 38 F9 0C 00 85 02 BD 10 00 38 F9 10 00 85 08 22 58 1D 7F E2 20 EB 38 F9 15 00 C9 10 90 40 C9 F0 B0 3C 48 AD 35 1C C9 03 F0 0B 68 C9 70 90 1B C9 90 90 2B 80 20 68 C9 40 90 10 C9 C0 B0 17 C2 20 A9 FF FF 8D 33 1C E2 20 80 14 AD 34 1C 18 69 20 8D 34 1C 80 09 AD 34 1C 18 69 10 8D 34 1C 28 60")

    def test_distance_service_ignores_height_and_calls_reviewed_word_length_kernel(self):
        self.assert_source(0x7F8C25, "E2 20 A5 5E 48 22 FE 77 7F C2 20 B9 0C 00 38 F5 0C 8F 26 00 70 A9 00 00 8F 28 00 70 B9 10 00 38 F5 10 8F 2A 00 70 E2 20 DA E2 20 A9 01 A2 72 FB 22 7B 78 7F E2 20 C2 10 FA C2 20 AF 2E 00 70 8D B5 16 E2 20 68 85 5E 8F 3A 30 00 22 4B 78 7F 60")


if __name__ == "__main__":
    unittest.main()
