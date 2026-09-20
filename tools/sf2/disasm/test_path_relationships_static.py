#!/usr/bin/env python3
"""Child attachment/lookup/detachment contracts from static source bytes only."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathRelationshipsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_selected_world_position_and_rotation_copy_only_their_named_components(self):
        self.assert_source(0x7FB8CD, "AC 1F CF C2 20 B9 0C 00 95 0C B9 0E 00 95 0E B9 10 00 95 10 4C E8 CA")
        self.assert_source(0x7FBF4E, "AC 1F CF 22 E2 2B 7F 4C E8 CA")
        self.assert_source(0x7F2BE2, "B9 12 00 95 12 B9 14 00 95 14 B9 16 00 95 16 6B")

    def test_refresh_linked_rotation_reads_live_link_and_subtracts_three_angle_bytes(self):
        self.assert_source(0x7FBACC, "B4 06 F0 1B B5 12 38 F9 12 00 9D D5 1C B5 14 38 F9 14 00 9D D6 1C B5 16 38 F9 16 00 9D D7 1C 4C E8 CA")

    def test_clear_and_self_reference_do_not_change_either_motion_gate(self):
        self.assert_source(0x7FAFA0,
            "C2 20 A9 00 00 9D D8 1C 4C E8 CA C2 20 8A 9D D8 1C A9 00 00 "
            "9D CF 1C 9D D1 1C 9D D3 1C E2 20 9D D5 1C 9D D6 1C 9D D7 1C 4C E8 CA")

    def test_selected_frame_capture_wraps_subtractions_and_enables_relative_coordinates(self):
        self.assert_source(0x7FAA5E,
            "A5 5E 29 E7 85 5E 8F 3A 30 00 AC 1F CF C2 20 B5 0C 38 F9 0C 00 9D CF 1C 85 02 "
            "B5 0E 38 F9 0E 00 9D D1 1C 85 08 B5 10 38 F9 10 00 9D D3 1C 85 97 98 9D D8 1C "
            "E2 20 B5 12 38 F9 12 00 9D D5 1C B9 12 00 49 FF 1A 8D 8F 15 9C 8E 15 "
            "B5 14 38 F9 14 00 9D D6 1C B9 14 00 49 FF 1A 8D 91 15 9C 90 15 "
            "B5 16 38 F9 16 00 9D D7 1C B9 16 00 49 FF 1A 8D 93 15 9C 92 15 "
            "DA 5A 22 BF 8B 03 C2 20 AD 3E 15 8D 7C 15 AD 40 15 8D 7E 15 AD 42 15 8D 80 15 "
            "AD 44 15 8D 82 15 AD 46 15 8D 84 15 AD 48 15 8D 86 15 AD 4A 15 8D 88 15 "
            "AD 4C 15 8D 8A 15 AD 4E 15 8D 8C 15 22 85 8B 03 C2 10 7A FA C2 20 "
            "A5 B7 9D CF 1C A5 B9 9D D1 1C A5 BB 9D D3 1C E2 20 B5 25 09 04 95 25 4C E8 CA")

    def test_selected_frame_uses_view_matrix_and_column_dot_products_not_attachment_order(self):
        self.assert_source(0x038BBF,
            "C2 20 AD 8E 15 8F 20 00 70 AD 90 15 8F 22 00 70 AD 92 15 8F 24 00 70 "
            "E2 20 C2 10 A9 01 A2 91 91 22 7B 78 7F")
        self.assert_source(0x038B85,
            "DA 5A 08 C2 30 A5 02 8F 68 00 70 A5 08 8F 2C 00 70 A5 97 8F 2E 00 70 "
            "E2 20 A9 01 A2 3A 91 22 7B 78 7F C2 20 AF 26 00 70 85 B7 AF 28 00 70 85 B9 "
            "AF 2A 00 70 85 BB 28 7A FA 6B")
        # Full matrix product/store suffix and complete point transform.
        self.assert_source(0x019266,
            "25 16 B4 9F 11 04 B3 9F 12 04 27 16 B4 9F 1D 04 B3 9F 1E 04 2C 16 BD 9F 04 52 "
            "39 D9 D9 B1 9F 04 6E 39 D9 D9 28 16 B4 9F 04 39 D9 D9 B7 9F 04 39 D9 D9 "
            "B5 9F 04 39 D9 D9 BC 4F D0 39 D9 D9 2C 16 BE 9F 04 61 39 D9 D9 B2 9F 04 5D "
            "39 D9 D9 28 16 B3 9F 04 39 9B 01")
        self.assert_source(0x01913A,
            "02 3D A1 34 3D A2 16 3D A3 17 3D A6 72 B1 9F 15 04 3D A6 75 B2 9F 04 25 50 "
            "3D A6 78 B3 9F 04 55 3E A0 13 3D A6 73 B1 9F 15 04 3D A6 76 B2 9F 04 25 50 "
            "3D A6 79 B3 9F 04 55 3E A0 14 3D A6 74 B1 9F 15 04 3D A6 77 B2 9F 04 25 50 "
            "3D A6 7A B3 9F 04 55 3E A0 15 00 01")

    def test_child_lookup_walks_in_order_and_compares_full_number_byte(self):
        self.assert_source(0x7F2A7B, "8D 2A 19 DA B4 29 F0 0B B9 13 00 CD 2A 19 F0 03 BB 80 F1 FA 6B")

    def test_linked_signals_are_identical_event_latches_without_motion_gate(self):
        expected = "B4 06 C0 00 00 D0 04 5C E8 CA 7F B9 23 00 09 08 99 23 00 4C E8 CA"
        self.assert_source(0x7F94DB, expected)
        self.assert_source(0x7F94F1, expected)

    def test_child_signal_uses_owner_flag_or_mother_and_only_sets_hit_event(self):
        self.assert_source(0x7F9400,
            "20 BC C4 8D B1 16 DA B5 23 29 10 F0 04 5C 19 94 7F B4 06 BB C0 00 00 F0 07 "
            "AD B1 16 22 7B 2A 7F FA C0 00 00 D0 04 5C D3 CA 7F B9 23 00 09 08 99 23 00 4C D3 CA")

    def test_child_missing_requires_parent_and_does_not_consume_ifnot(self):
        self.assert_source(0x7F938B,
            "20 BC C4 8D B1 16 DA B5 23 29 10 F0 04 5C A5 93 7F B4 06 BB D0 04 5C B8 93 7F "
            "5A AD B1 16 22 7B 2A 7F C0 00 00 D0 05 7A 5C BC 93 7F 7A FA 4C A9 CA FA 4C FF CA")
        self.assert_source(0x7FCAA9, "E2 20 C2 20 B5 2B 18 69 04 00 95 2B E2 20 4C 75 7E")
        self.assert_source(0x7FCAFF, "C2 20 20 4C C7 95 2B E2 20 4C 75 7E")

    def test_attach_sets_number_and_mother_appends_at_tail_then_sets_flags(self):
        self.assert_source(0x7F2A3D, "85 5F A5 5F 99 13 00 96 06 C2 20 A9 00 00 99 29 00 E2 20 C2 20 84 3A 8A A8 B9 29 00 D0 FA A5 3A 99 29 00 A4 3A E2 20 B5 23 09 10 95 23 B9 23 00 09 04 99 23 00 B9 25 00 09 01 99 25 00 6B")

    def test_child_spawn_selects_mother_only_when_attached_and_links_only_on_success(self):
        self.assert_source(0x7F90BB, "B5 23 29 04 D0 04 5C C8 90 7F B4 06 BB C2 20 AD B7 16 85 5F E2 20 22 17 2A 7F B0 04 5C 2B 91 7F AD B9 16 22 3D 2A 7F")

    def test_self_unlink_flag_gate_and_flags_clear_before_chain_search(self):
        self.assert_source(0x7F9435, "B5 23 29 04 D0 04 5C E8 CA 7F B4 06 B5 23 29 FB 95 23 B5 25 29 FE 95 25 C2 20 86 3C 5A B9 29 00 F0 17 C5 3C F0 03 A8 80 F4 B5 29 99 29 00 A9 00 00 95 06 95 29 E2 20 95 13 7A E2 20 4C E8 CA")

    def test_numbered_unlink_chooses_owner_or_mother_without_changing_selection(self):
        self.assert_source(0x7F9474, "20 BC C4 8D B1 16 DA B5 23 29 10 F0 04 5C 8D 94 7F B4 06 BB C0 00 00 F0 07 AD B1 16 22 7B 2A 7F FA C0 00 00 D0 04 5C D3 CA 7F DA B6 06 B9 23 00 29 FB 99 23 00 B9 25 00 29 FE 99 25 00 C2 20 84 3C DA B5 29 F0 1A C5 3C F0 03 AA 80 F5 B9 29 00 95 29 A9 00 00 99 06 00 99 29 00 E2 20 99 13 00 FA E2 20 FA 4C D3 CA")

    def test_aux_bit40_reads_action_byte_not_mode_and_preserves_ifnot(self):
        self.assert_source(0x7FB99E, "AC 1F CF DA BB 7A 5A B4 2B B9 77 6B 7A DA BB 7A 89 40 F0 04 5C B9 B9 7F 4C BE CA 4C F3 CA")


if __name__ == "__main__":
    unittest.main()
