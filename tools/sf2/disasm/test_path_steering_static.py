#!/usr/bin/env python3
"""Assembly contracts for native facing commands; no original execution."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathSteeringStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_immediate_selected_and_fixed_slots_have_distinct_target_selection(self):
        self.assert_source(0x7F872C, "AC 1F CF 9C 9D 14 22 A5 21 7F")
        self.assert_source(0x7F8755, "AC 1F CF CC C3 12 D0 05 A0 3F 03 80 03 A0 7E 03 9C 9D 14")

    def test_offset_aim_preparation_and_complete_consumer_preserve_signed_byte_geometry(self):
        self.assert_source(0x7FC346, "C2 20 20 20 C7 A8 E2 20 20 04 C5 99 00 00 4C A9 CA")
        self.assert_source(0x7FC1B8, "DA AE 1F CF AC D6 14 AD B1 16 85 02 85 04 89 80 F0 06 A9 FF 85 05 80 02 64 05 AD B3 16 85 08 85 0A 89 80 F0 06 A9 FF 85 0B 80 02 64 0B AD B5 16 85 97 85 E4 89 80 F0 06 A9 FF 85 E5 80 02 64 E5")
        self.assert_source(0x7FC1F8, "B5 14 22 A9 38 7F C2 20 A5 04 0A 0A 0A 0A 85 04 A5 0A 0A 0A 0A 0A 85 0A A5 E4 0A 0A 0A 0A 85 E4 A5 04 18 7D 0C 00 99 0C 00 A5 E4 18 7D 10 00 99 10 00 A5 0A 18 7D 0E 00 99 0E 00 E2 20 FA")
        self.assert_source(0x7FC236, "9C 9D 14 22 A5 21 7F E2 20 EB D5 12 F0 2D 38 F5 12 C9 00 30 08 C9 08 10 0A A9 08 80 06 C9 F8 30 02 A9 F8 C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 12 95 12")
        self.assert_source(0x7FC273, "22 88 21 7F E2 20 EB 49 FF 1A D5 14 F0 2D 38 F5 14 C9 00 30 08 C9 08 10 0A A9 08 80 06 C9 F8 30 02 A9 F8 C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 14 95 14 4C E8 CA")

    def test_selected_smoothing_clamps_numerator_then_takes_two_signed_halves(self):
        self.assert_source(0x7F879C, "38 F5 14 C9 00 30 08 C9 04 10 0A A9 04 80 06 C9 FC 30 02 A9 FC C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 14")
        self.assert_source(0x7F87DB, "38 F5 12 C9 00 30 08 C9 04 10 0A A9 04 80 06 C9 FC 30 02 A9 FC C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 12")

    def test_linked_smoothing_uses_three_halves_and_skips_absent_base_link(self):
        self.assert_source(0x7F8A72, "B4 06 D0 04 5C E8 CA 7F 9C 9D 14")
        self.assert_source(0x7F8A8C, "38 F5 12 C9 00 30 08 C9 08 10 0A A9 08 80 06 C9 F8 30 02 A9 F8 C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 12")

    def test_only_selected_variants_refresh_relative_yaw_and_not_pitch(self):
        self.assert_source(0x7F874D, "95 14 20 3F 88 4C E8 CA")
        self.assert_source(0x7F8783, "95 14 20 3F 88 4C E8 CA")
        self.assert_source(0x7F87C2, "95 14 20 3F 88 4C E8 CA")
        self.assert_source(0x7F8837, "95 14 20 3F 88 4C E8 CA")
        self.assert_source(0x7F883F, "B5 25 29 04 D0 04 5C 55 88 7F BC D8 1C B5 14 38 F9 14 00 9D D6 1C 60")

    def test_linked_alignment_counts_equality_before_assignment(self):
        self.assert_source(0x7F8B29, "EB D5 12 D0 04 5C 38 8B 7F 95 12 5C 3B 8B 7F EE 9D 14")
        self.assert_source(0x7F8B45, "D5 14 D0 04 5C 53 8B 7F 95 14 5C 56 8B 7F EE 9D 14")

    def test_radius_commands_sign_extend_literals_and_linked_form_does_not_load_variable(self):
        from path_semantics import PATH_SEMANTIC_BY_OPCODE
        self.assertEqual(PATH_SEMANTIC_BY_OPCODE[0x0AE].rust_name, "ContractSelectedRadius")
        self.assertEqual(PATH_SEMANTIC_BY_OPCODE[0x122].rust_name, "ContractLinkedRadius")
        self.assertEqual(PATH_SEMANTIC_BY_OPCODE[0x0AB].rust_name, "ContractLocalRadius")
        self.assert_source(0x7FADAF, "20 BC C4 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 85 02 B4 06 80 17")
        self.assert_source(0x7FADC7, "20 BC C4 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 85 02 AC 1F CF")

    def test_radial_scaling_subtracts_from_length_before_rescaling_all_axes(self):
        self.assert_source(0x7FAE0B, "A9 01 A2 72 FB 22 7B 78 7F C2 20 C2 10 AF 2E 00 70 38 E5 02 8F 68 00 70 E2 20 A9 01 A2 5A FC 22 7B 78 7F")
        self.assert_source(0x7FAE34, "AF 26 00 70 18 79 0C 00 95 0C AF 28 00 70 18 79 0E 00 95 0E AF 2A 00 70 18 79 10 00 95 10")

    def test_local_radius_uses_retained_vector_and_writes_every_local_axis(self):
        self.assert_source(0x7FAE9B, "BD CF 1C 8F 26 00 70 BD D1 1C 8F 28 00 70 BD D3 1C 8F 2A 00 70")
        self.assert_source(0x7FAEDB, "AF 26 00 70 9D CF 1C AF 28 00 70 9D D1 1C AF 2A 00 70 9D D3 1C")

    def test_yaw_orbits_use_geometry_matrix_and_publish_only_horizontal_axes(self):
        self.assert_source(0x7FABFB, "9C 8E 15 9C 92 15 A5 04 8D 90 15")
        self.assert_source(0x7FAC13, "22 27 8C 03")
        self.assert_source(0x7FAC4F, "22 85 8B 03")
        self.assert_source(0x7FAC6A, "A5 B7 9D CF 1C A5 BB 9D D3 1C 4C D3 CA")
        self.assert_source(0x7FAD53, "A5 B7 18 79 0C 00 95 0C A5 BB 18 79 10 00 95 10 4C D3 CA")

    def test_orbit_variable_samples_a_byte_while_literals_do_not_resolve_actor_fields(self):
        self.assert_source(0x7FABE8, "20bcc485056404c220bdcf1c8502bdd31c8597")
        self.assert_source(0x7FACA6, "20bcc42047cbb90000850564048018")
        self.assert_source(0x7FACC6, "20bcc485056404ac1fcf")
        self.assert_source(0x7FAE7C, "20bcc4c220898000f0050900ff800329ff008502")


if __name__ == "__main__":
    unittest.main()
