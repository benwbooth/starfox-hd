#!/usr/bin/env python3
"""Original source contracts for the triggered consumable's parallel stream."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerActionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_pause_precedes_auxiliary_lookup_and_inactive_skips_completion(self):
        self.assert_source(0x0DBCD0, "DA 5A 08 E2 20 C2 10 C2 20 AD 84 1B 89 02 00 E2 20 F0 03 4C D3 BD E2 20 C2 10 B4 2B 8C A3 1D C2 20 B9 16 6C 8D A5 1D B9 13 6C 8D 9E 1D E2 20 D0 04 5C D3 BD 0D")

    def test_complete_authored_stream_keeps_unsorted_order(self):
        self.assert_source(0x0DBDDA, "03 02 00 0B 00 79 C6 00 00 00 C8 CF 00 28 00 63 C6 00 0E 00 24 D4 00 0C 00 9D C6 FF FF")

    def test_timing_reads_live_decision_time_and_iteration_does_not_reload_action(self):
        self.assert_source(0x0DBD58, "A9 05 00 8D A1 1D AD A5 1D C7 02 D0 4F 4C A0 BD")
        self.assert_source(0x0DBD88, "A9 07 00 8D A1 1D AD A5 1D C7 02 90 1F E6 02 E6 02 C7 02 B0 17 4C A0 BD")
        self.assert_source(0x0DBDA0, "E6 02 E6 02 A7 02 85 3A A9 B2 BD 3A 48 E2 20 6C 3A 00 C2 20 AD 9E 1D 18 6D A1 1D 8D 9E 1D 4C 0B BD")

    def test_early_jump_changes_both_clocks_and_later_detonation_sets_full_byte(self):
        self.assert_source(0x0DC679, "08 AD 59 1E D0 07 AD 36 19 89 40 F0 0B C2 20 A9 0C 00 99 16 6C 8D A5 1D E2 20 28 60")
        self.assert_source(0x0DC69D, "08 A9 01 8D 59 1E 28 60")

    def test_stop_preserves_total_age_and_epilogue_increments_after_stop(self):
        self.assert_source(0x0DC663, "08 C2 20 A9 00 00 99 13 6C 99 16 6C 99 18 6C E2 20 99 15 6C 28 60")
        self.assert_source(0x0DBDC1, "C2 20 B9 16 6C 1A F0 0A 99 16 6C B9 1A 6C 1A 99 1A 6C E2 20 28 7A FA AB 6B")

    def test_palette_snapshot_copies_128_full_words_and_restore_sets_only_one_bit(self):
        self.assert_source(0x0DCFC8, "08 AD E2 1D C9 09 F0 04 22 B8 EB 07 28 60")
        self.assert_source(0x07EBB8, "DA 5A 08 E2 20 C2 10 B4 2B C2 20 A2 FE 00 BF E5 EF 7E 9F E5 F2 7E CA CA 10 F4 28 7A FA 6B")
        self.assert_source(0x0DD424, "08 AD 0D 1E 09 10 8D 0D 1E 28 60")


if __name__ == "__main__":
    unittest.main()
