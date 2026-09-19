#!/usr/bin/env python3
"""Source-byte evidence for queue ordering and epoch latches; no recordings."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class CollisionPassStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_queue_filters_and_shape_header_snapshot(self):
        self.assert_source(0x7F32B3, "9C D9 18 AD 96 1B 89 00 01 D0 7D")
        self.assert_source(0x7F32C7, "B5 31 29 04 00 D0 67 B5 21 29 01 00 D0 60 B5 25 29 08 00 D0 59 B5 2D 29 FF 00 F0 52 B5 08 89 01 00 D0 4B")
        self.assert_source(0x7F3302, "B9 08 00 9F 44 2F 7E B9 0A 00 9F 46 2F 7E B9 0C 00 9F 48 2F 7E B9 0E 00 9F 4A 2F 7E")
        self.assert_source(0x7F3335, "FA B5 00 AA D0 8B")

    def test_cleanup_copies_skip_and_hit_latches_before_clearing_pending(self):
        self.assert_source(0x7F4054, "B5 22 29 08 F0 04 5C 66 40 7F B5 25 29 EF 95 25 80 06 B5 25 09 10 95 25")
        self.assert_source(0x7F406C, "B5 20 29 80 F0 04 5C 7E 40 7F B5 21 29 FD 95 21 80 06 B5 21 09 02 95 21 B5 20 29 7F 95 20")
        # Retired actor saves its successor before the lifecycle call.
        self.assert_source(0x7F4044, "C2 20 B5 00 48 E2 20 22 46 33 7F 7A BB 4C B3 40")
        # Survivor instead reads its link after directional contact cleanup.
        self.assert_source(0x7F40B0, "9B B6 00 D0 85")

    def test_candidates_begin_after_probe_and_are_restarted_for_each_box(self):
        self.assert_source(0x7F43E5, "20 24 44")
        self.assert_source(0x7F43F4, "7A FA C2 20 B9 00 00 F0 03 82 FB FC")
        self.assert_source(0x7F4424, "AD D9 18 8D DB 18 A4 7F 8C DD 18 4C 38 49")
        self.assert_source(0x7F4938, "C2 20 AD DD 18 18 69 0A 00 A8 CE DB 18 F0 03 4C 32 44 E2 20 60")
        self.assert_source(0x7F40BD, "DA 20 D4 40 FA C2 20 8A 18 69 0A 00 AA E2 20 CE D9 18 D0 EC")

    def test_live_pair_filters_groups_links_and_bilateral_same_shape_opt_in(self):
        self.assert_source(0x7F443E, "A4 7D B9 31 00 35 31 29 F8 00 F0 03 82 EB 04")
        self.assert_source(0x7F444D, "98 D5 1C D0 03 82 E3 04 8A BB D5 1C D0 03 82 DA 04")
        self.assert_source(0x7F446C, "B5 22 29 20 00 F0 0D B9 22 00 29 20 00 F0 05 B5 04 9B 80 0B B5 04 9B CD E1 18 D0 03 82 AD 04")

    def test_compound_flags_and_simple_directional_preservation(self):
        self.assert_source(0x7F483C, "A5 F5 F0 57")
        self.assert_source(0x7F4850, "B5 20 09 80 95 20 B9 20 00 09 80 99 20 00")
        self.assert_source(0x7F4878, "A5 F5 99 08 00 A5 F6 9D 08 00 7A B9 38 00 05 F5 99 38 00 A6 7D B5 38 05 F6 95 38")
        self.assert_source(0x7F4928, "E2 20 A5 F6 9D 08 00 A6 7D B5 38 05 F6 95 38 FA")

    def test_both_frame_branches_detect_before_strategy_and_strategy_reads_pending(self):
        self.assert_source(0x038027, "22 A1 32 7F A5 00 22 1E 7A 7F")
        self.assert_source(0x7F7B07, "20 BD 7B")
        self.assert_source(0x7F7BBD, "22 2D 40 7F 60")
        self.assert_source(0x038080, "22 E7 34 7F 22 4A 35 7F")
        self.assert_source(0x0380AE, "22 A1 32 7F 22 80 79 7F")
        self.assert_source(0x7F79E5, "20 C2 7B")
        self.assert_source(0x7F7BC2, "22 2D 40 7F 22 BC 11 7F 60")
        self.assert_source(0x0380D5, "22 E7 34 7F")
        self.assert_source(0x7F35EE, "B5 31 29 FB 95 31 B5 20 29 80 F0 18 A9 03 8D D1 12")


if __name__ == "__main__":
    unittest.main()
