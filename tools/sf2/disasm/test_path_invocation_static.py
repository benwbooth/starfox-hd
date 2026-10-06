"""Source boundaries required by the native path invocation coordinator."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathInvocationStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_common_entry_refreshes_selection_but_immediate_dispatch_does_not(self):
        self.assert_source(0x7F7E5D,
            "B5 24 29 80 F0 04 5C 6F 7E 7F AC C3 12 8C 1F CF 80 06 "
            "AC C5 12 8C 1F CF C2 20 B5 2B 85 F9 E2 20")

    def test_carry_resolves_selection_after_callbacks_and_before_contact_cleanup(self):
        self.assert_source(0x7F9E70, "20 A8 9A")
        self.assert_source(0x7F9EC9,
            "B5 21 29 20 D0 04 5C FD 9E 7F AC 1F CF B9 24 00 29 02")
        self.assert_source(0x7F9EF2,
            "BD C1 1C F0 03 20 1C BB 20 F7 BA "
            "B5 22 29 FD 95 22 B5 21 29 7F 95 21 B5 26 29 FD 95 26 B5 26 29 FB 95 26 6B")

    def test_strategy_footer_and_both_passes_preserve_the_returned_actor(self):
        # The footer saves/restores the RETURNED identity around positional
        # sound; it does not reload the entry identity saved at 12C7.
        self.assert_source(0x7F365A,
            "DA 5A 08 E2 20 C2 10 AD D3 1C 89 01 D0 0B BD CC 1C F0 06 "
            "A0 3F 03 20 B8 36 28 7A FA E2 20 68 85 5E 8F 3A 30 00 28 6B")
        for address, expected in [
            (0x7F3526, "22 96 35 7F AD D6 12 D0 07 B4 00 BB D0 DD 80 08 B4 00 22 56 33 7F 80 F3"),
            (0x7F3572, "22 96 35 7F AD D6 12 D0 07 B4 00 BB D0 E5 80 08 B4 00 22 56 33 7F 80 F3"),
        ]:
            self.assert_source(address, expected)

    def test_hit_response_tail_does_not_replace_the_assigned_strategys_returned_actor(self):
        self.assert_source(0x03A44C,
            "B5 26 29 08 F0 04 5C 66 A4 03 C2 20 AD 84 1B 89 02 00 E2 20 "
            "D0 03 4C 66 A4 6B 5C 8F 2B 7F")
        self.assert_source(0x7F2B8F,
            "B5 1B 48 C2 20 D0 09 B5 19 F0 0C 3A 48 E2 20 6B "
            "B5 19 3A 48 E2 20 6B")


if __name__ == "__main__":
    unittest.main()
