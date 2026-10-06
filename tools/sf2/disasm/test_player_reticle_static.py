"""The marker projector and asymmetric reticle-clamp/easing boundaries."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerReticleStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_individual_projector_calls_division_without_mesh_depth_dispatch(self):
        self.assert_source(0x01D50A,
            "60 3E DF 3F DF FA F0 03 3D A1 34 3D A2 16 3D A3 17 "
            "94 FF DB A2 01 3E A1 34 3E A2 16 00 01")
        self.assert_source(0x038F25, "A9 01 A2 0A D5 22 7B 78 7F")

    def test_vertical_lower_trigger_and_replacement_differ(self):
        self.assert_source(0x07A471,
            "C2 20 A9 F0 FF 18 65 79 10 05 A9 10 00 80 07 "
            "A9 D0 00 C5 79 B0 02 85 79 A9 E0 FF 18 65 7B "
            "10 05 A9 10 00 80 07 A9 B0 00 C5 7B B0 02 85 7B E2 20")

    def test_easing_writes_horizontal_before_reading_vertical_and_retention(self):
        self.assert_source(0x07A4C5,
            "C9 80 6A 10 02 69 00 18 6D 30 1E 8D 30 1E "
            "A5 7B 18 69 18 85 08 A5 08 CD 31 1E")
        self.assert_source(0x07A4F7,
            "C9 80 6A 10 02 69 00 18 6D 31 1E 8D 31 1E "
            "AE C3 12 B4 2B AD DD 1D")


if __name__ == "__main__":
    unittest.main()
