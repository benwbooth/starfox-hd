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

    def test_child_lookup_walks_in_order_and_compares_full_number_byte(self):
        self.assert_source(0x7F2A7B, "8D 2A 19 DA B4 29 F0 0B B9 13 00 CD 2A 19 F0 03 BB 80 F1 FA 6B")

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
