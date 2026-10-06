#!/usr/bin/env python3
"""Source-bound consumable dispatch, effect installers and shared field owners."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerConsumableStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_pause_count_and_byte_doubled_kind_precede_type_specific_inputs(self):
        self.assert_source(0x07DC8B, "DA 5A C2 20 AD 84 1B 89 02 00 E2 20 F0 03 4C 6B DD B4 2B B9 04 6C 89 0F D0 04 5C 6B DD 07")

    def test_dispatch_table_and_original_empty_branches_are_complete(self):
        self.assert_source(0x07DCA3, "D0 04 5C 6B DD 07 B9 05 6C 0A C9 0A 90 04 5C 16 DD 07 DA C2 20 29 FF 00 AA BF C8 DC 07 85 02 E2 20 FA 6C 02 00 3D DD D2 DC 19 DD 1E DD 16 DD")
        self.assert_source(0x07DD16, "4C 5E DD 4C 5E DD 80 40")
        self.assert_source(0x07DD5E, "B4 2B B9 04 6C 3A 99 04 6C 7A FA 38 6B 7A FA 18 6B")

    def test_triggered_installation_gates_then_starts_action_before_allocation_and_clear(self):
        self.assert_source(0x07DCD2, "C2 20 B9 13 6C E2 20 F0 04 5C 6B DD 07 B9 E9 6B 89 18 F0 04 5C 6B DD 07 A9 0D D9 15 6C F0 03 99 15 6C C2 20 A9 DA BD D9 13 6C F0 0C 99 13 6C A9 00 00 99 16 6C 99 18 6C E2 20 22 1D D1 07 9C 59 1E 4C 5E DD")

    def test_deflection_checks_active_count_and_projectile_bit_but_preserves_high_tags(self):
        self.assert_source(0x07DD1E, "B9 02 6C 29 5F F0 04 5C 6B DD 07 B9 02 6C 29 E0 99 02 6C A9 1F 29 1F 19 02 6C 99 02 6C 80 21")

    def test_recovery_tests_caller_reserve_equality_not_active_shield_or_less_than(self):
        self.assert_source(0x07DD3D, "B9 7D 6B 89 40 F0 04 5C 6B DD 07 5A B4 2B B9 00 6C 7A CD D5 1D D0 04 5C 6B DD 07 22 5D D1 07 90 0D")

    def test_triggered_installer_does_not_attach_or_number_and_clears_pause_after_formatting(self):
        self.assert_source(0x07D11D, "5A 08 E2 20 C2 10 C2 20 A9 9C BC 85 5F E2 20 22 17 2A 7F B0 04 5C 5A D1 07 C2 20 A9 1E 7E 99 19 00 E2 20 A9 7F 99 1B 00 C2 20 A9 8B F4 99 2B 00 E2 20 20 F5 BE B9 26 00 29 F7 99 26 00 28 7A 6B")

    def test_recovery_installer_owns_number_22_and_self_relative_frame(self):
        self.assert_source(0x07D15D, "5A 08 E2 20 C2 10 A9 16 22 7B 2A 7F C0 00 00 F0 04 5C A4 D1 07 C2 20 A9 9C BC 85 5F E2 20 22 17 2A 7F B0 04 5C A4 D1 07 A9 16 22 3D 2A 7F C2 20 A9 AD F3 99 2B 00 E2 20 20 F5 BE C2 20 98 99 D8 1C E2 20 28 7A 38 6B 28 7A 18 6B")
        self.assert_source(0x7FAFAB, "C2 20 8A 9D D8 1C A9 00 00 9D CF 1C 9D D1 1C 9D D3 1C E2 20 9D D5 1C 9D D6 1C 9D D7 1C 4C E8 CA")

    def test_shared_formatter_sets_named_fields_without_copying_links_or_auxiliary(self):
        self.assert_source(0x07BEF5, "08 E2 20 C2 10 C2 20 A9 1E 7E 99 19 00 E2 20 A9 7F 99 1B 00 A9 FF 99 F0 1C A9 01 99 2D 00 A9 01 99 2E 00 B9 22 00 09 04 99 22 00 22 AA 2B 7F 22 D2 2B 7F B9 26 00 09 08 99 26 00 B9 31 00 29 EF 99 31 00 B9 21 00 09 01 99 21 00 28 60")

    def test_contact_and_projectile_recoil_share_the_same_live_player_word(self):
        self.assert_source(0x06AAC6, "C2 20 B9 3B 6B C9 00 00 E2 20 F0 04 5C E0 AA 06 C2 20 A9 80 00 99 3B 6B E2 20")
        self.assert_source(0x7FB1EF, "5A B4 2B C2 20 B9 3B 6B C9 00 00 E2 20 F0 04 5C 0B B2 7F C2 20 A5 02 99 3B 6B E2 20")

    def test_recovery_feedback_installer_preserves_existing_child_before_allocation(self):
        # Number 24 is distinct from the healing emitter's number 22. A found
        # child returns immediately. The apparent carry-clear return after
        # allocation cannot execute: exhaustion enters non-returning $008032.
        self.assert_source(0x07D0C6,
            "5A 08 E2 20 C2 10 A9 18 22 7B 2A 7F C0 00 00 F0 04 5C 19 D1 07 C2 20 A9 9C BC "
            "85 5F E2 20 22 17 2A 7F B0 04 5C 07 D1 07 A9 18 22 3D 2A 7F DA BB 7A C2 20 "
            "A9 1E 7E 95 19 E2 20 A9 7F 95 1B DA BB 7A C0 00 00 F0 0D C2 20 A9 8A F3 "
            "99 2B 00 E2 20 20 F5 BE 28 7A 38 6B")

    def test_recovery_feedback_path_locks_and_follows_primary_for_eight_updates(self):
        self.assert_source(0x09F38A,
            "5C 00 04 32 FE F8 FF 89 22 EC F5 07 C2 20 A9 9C F3 6B 61 08 "
            "89 22 34 F6 07 C2 20 A9 A9 F3 6B 00 56 44 0F")


if __name__ == "__main__":
    unittest.main()
