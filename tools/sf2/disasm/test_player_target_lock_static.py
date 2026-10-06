"""Target retention belongs to the display service and its actual primary."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerTargetLockStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_display_call_sites_and_primary_reload_before_retention(self):
        self.assert_source(0x038053, "22 26 A3 07 22 9C 53 7F 22 01 83 04")
        self.assert_source(0x03810E, "22 26 A3 07 AD A6 1A 89 02 D0 08")
        self.assert_source(0x07A326,
            "8B DA 5A 08 E2 20 C2 10 A9 7E 48 AB AE C3 12 80 01 6B B4 2B")
        self.assert_source(0x07A505,
            "AE C3 12 B4 2B AD DD 1D 89 80 D0 04 5C F9 A5 07")

    def test_cancel_preserves_publication_unless_its_distinct_clear_prefix_runs(self):
        self.assert_source(0x07A5F9,
            "9C 90 1D 9C 91 1D C2 20 B9 B8 6B 99 C8 6B E2 20 "
            "C2 20 A9 00 00 99 CA 6B E2 20 A9 00 99 C6 6B A9 00 99 C7 6B 80 43")
        self.assert_source(0x07A56D,
            "C2 20 A9 00 00 99 C8 6B 9C 90 1D E2 20 "
            "B9 C2 6B 29 20 89 20 F0 04 5C FF A5 07")

    def test_acquisition_queues_literal_cue_before_publication_and_boundary_precedes_drawing(self):
        self.assert_source(0x07A62C,
            "B9 B7 6B 29 F0 99 B7 6B C2 20 A9 3B 00 22 09 6E 7F E2 20 "
            "C2 20 B9 B8 6B 99 CA 6B E2 20 C2 20 B9 B8 6B 99 C8 6B E2 20 "
            "C2 20 B9 CA 6B 8D 90 1D E2 20 A9 0A 99 C7 6B "
            "B9 C2 6B 29 EF 29 F7 99 C2 6B A5 5E 29 F7 85 5E 8F 3A 30 00")


if __name__ == "__main__":
    unittest.main()
