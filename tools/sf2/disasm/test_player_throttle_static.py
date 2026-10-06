"""Throttle callers, numbered installer contracts and authored consumers."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerThrottleStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_activity_gate_precedes_input_and_pressed_brake_wins(self):
        self.assert_source(0x06F017,
            "AD F4 D7 F0 3B AD 36 19 89 80 D0 0B AD 37 19 89 40 F0 09 "
            "A9 00 80 02 A9 01 99 78 6B")
        self.assert_source(0x06F033,
            "B9 78 6B D0 0D AD 39 19 89 40 F0 06 22 7B A4 06 80 16 "
            "AD 38 19 89 80 F0 06 22 C5 A4 06 80 09 A9 00 99 78 6B 22 1D A5 06")

    def test_brake_runs_both_installers_regardless_of_their_return_and_skips_retained_store(self):
        self.assert_source(0x06A4CB,
            "22 AD CE 07 B0 04 5C D5 A4 06 22 D7 CE 07 B0 04 5C DF A4 06")
        self.assert_source(0x06A504,
            "B9 77 6B 29 BF 89 20 D0 05 09 20 99 77 6B")

    def test_cancel_variants_clear_different_effect_bits_before_shared_decay(self):
        self.assert_source(0x06A525,
            "B9 79 6B 29 BF 99 79 6B B9 77 6B 29 9F 99 77 6B "
            "B9 7A 6B 29 3F 99 7A 6B B9 79 6B 29 3F D0 14 "
            "B9 7A 6B 89 0F F0 0D A5 C4 29 01 D0 07 B9 7A 6B 3A 99 7A 6B")
        self.assert_source(0x06A563, "B9 79 6B 29 7F 99 79 6B 80 C0")

    def test_pilot_pair_offsets_have_invalid_pilot_fallback_not_a_doubled_index(self):
        self.assert_source(0x0690DA,
            "B9 FF 6B 29 FF 00 C9 06 00 90 03 A9 00 00 7A AA 28 6B")
        self.assert_source(0x0690F8, "08 C2 30 22 D4 90 06 8A 29 FE FF AA 28 6B")
        self.assert_source(0x07CFA5, "0F 00 19 00 0F 00 00 00 0A 00 0A 00")

    def test_effect_allocations_use_real_shared_pool_and_deferred_authored_paths(self):
        self.assert_source(0x07CE5D, "C2 20 A9 B0 BE 85 5F E2 20 22 17 2A 7F")
        self.assert_source(0x07CF47, "C2 20 A9 B0 BE 85 5F E2 20 22 17 2A 7F")
        self.assert_source(0x07CE98, "C2 20 A9 2C F3 99 2B 00 E2 20 20 F5 BE")
        self.assert_source(0x07CF94, "C2 20 A9 6F F3 99 2B 00 E2 20 20 F5 BE")

    def test_full_pool_enters_nonreturning_fatal_display_not_the_apparent_failure_return(self):
        self.assert_source(0x7F2969,
            "A5 5E 29 E7 85 5E 8F 3A 30 00 22 32 80 00 18 6B")
        self.assert_source(0x008032, "C2 30 A9 E0 03 8F 2C 19 00 80 00")
        # The fatal-display loops do not return to the allocator's CLC/RTL.
        self.assert_source(0x0080B0,
            "9C 21 21 9C 22 21 9C 22 21 A2 0A 00 A0 00 00 88 D0 FD CA D0 F7 80 CE")


if __name__ == "__main__":
    unittest.main()
