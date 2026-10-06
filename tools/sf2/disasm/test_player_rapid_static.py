#!/usr/bin/env python3
"""Source boundaries for the rapid tail, launch gate and flight helper."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerRapidStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_flight_mode_bypasses_the_alternate_pressed_and_shared_action_gates(self):
        self.assert_source(0x07D7E4, "B4 2B 5A B4 2B B9 A0 6A 7A 29 F0 C9 10 D0 04 5C 63 D8 07 AD 37 19 89 80 D0 04 5C 5F D8 07 AD 72 1D F0 04 5C 5F D8 07 C2 20 AD 84 1B 89 02 00 E2 20 F0 03 4C 5F D8")

    def test_alternate_parameters_and_selector_are_written_only_after_admission(self):
        self.assert_source(0x07D81A, "A0 3F 03 64 02 A5 02 49 FF 1A 85 02 A5 02 18 69 FC 85 02 A9 00 85 97 A9 00 8D B0 14 A9 00 8D B2 14 A5 97 8D B4 14 A5 02 8D B7 14 A9 00 8D B6 14 9C B8 14 9C B9 14 A9 04 22 9C A8 03 C0 00 00 D0 04 5C 5F D8 07 28 7A FA 6B")

    def test_complete_queue_uses_signed_compare_and_consumes_only_successful_requests(self):
        self.assert_source(0x07D863, "AD 37 19 89 80 F0 0D B9 60 6B C9 40 10 06 18 69 10 99 60 6B B9 60 6B 89 0F D0 18 89 F0 F0 1D 20 88 D9 90 18 B9 60 6B 29 F0 38 E9 10 09 01 99 60 6B 80 09 DA BB 7A DE 60 6B DA BB 7A 28 7A FA 6B")

    def test_flight_launch_publishes_owner_level_before_initializer_pause_gate(self):
        self.assert_source(0x07D988, "DA 5A 08 E2 20 C2 10 B4 2B A9 00 8D BC 1D B9 06 6C 8D B6 1D C2 20 AD 84 1B 89 02 00 E2 20 F0 03 4C 19 DA")
        self.assert_source(0x07D9AB, "5A B4 2B B9 63 6B 7A 89 40 F0 04 5C C2 D9 07 29 80 F0 04 5C D1 D9 07")

    def test_linked_origin_copies_only_xz_and_restores_only_after_a_returning_helper(self):
        self.assert_source(0x07D9D1, "AD B6 1D 3A F0 07 3A F0 04 A9 14 80 02 A9 EC 8D BC 1D A0 3F 03 C2 20 B5 0C 8D BA 1D B5 10 8D B8 1D B9 0C 00 95 0C B9 10 00 95 10 E2 20 20 1E DA C2 20 AD BA 1D 95 0C AD B8 1D 95 10 E2 20 C0 00 00 F0 05 28 7A FA 38 60 28 7A FA 18 60")

    def test_no_weapon_level_keeps_non_null_selection_that_wrapper_accepts(self):
        # Helper returns failure without replacing its selection for a
        # negative decremented level. The wrapper deliberately ignores that
        # failure flag, tests the retained selection and reports success.
        self.assert_source(0x07DA1E, "A9 00 8D B7 14 A9 00 8D B6 14 9C B8 14 9C B9 14 AD B6 1D 3A 10 04 5C B0 DA 07 D0 04 5C 4C DA 07 3A D0 04 5C 6E DA 07 3A D0 04 5C 8F DA 07")
        self.assert_source(0x07D9C2, "20 1E DA C0 00 00 D0 04 5C 19 DA 07 4C 14 DA")
        self.assert_source(0x07DAB0, "18 60")
        # A valid bound player record represents a successful owned auxiliary
        # allocation, not its failed/null initialization case.
        self.assert_source(0x068260, "22 A7 19 7F C2 20 A9 D8 01 22 4E 19 7F A8 E2 20 94 2B")
        self.assert_source(0x7F194E, "18 69 02 00 22 A7 18 7F C9 00 00 F0 0F 5A A8 BD DC 1C 99 61 6A 98 9D DC 1C 7A 1A 1A 6B")

    def test_each_valid_profile_resets_muzzle_and_calls_its_real_variant(self):
        for address, selection in [(0x07DA4C, 6), (0x07DA6E, 8), (0x07DA8F, 10)]:
            self.assert_source(address, f"A9 00 8D B0 14 A9 00 8D B2 14 AD BC 1D 8D B4 14 A9 {selection:02X} 22 9C A8 03 C0 00 00")

    def test_shot_count_rejection_preserves_selection_after_dispatcher_narrowing(self):
        # A page-aligned player allocation becomes null after narrowing.
        # The fixed linked view retains its nonzero low byte. No-weapon
        # levels bypass this dispatcher and keep the full auxiliary value.
        self.assert_source(0x03A89C, "86 3A E2 30")
        self.assert_source(0x06A9E6,
            "08 5A B4 2B B9 03 6C 7A C9 08 10 03 28 38 6B 28 18 6B")
        self.assert_source(0x0DE0CE, "6B")


if __name__ == "__main__":
    unittest.main()
