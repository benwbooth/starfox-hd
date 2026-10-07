"""Free-flight plane selection preserves distinct player and renderer owners."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerSurfacePreparationStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_ordinary_contact_compares_retained_walker_height_and_copies_material(self):
        self.source(0x07E5E3, "b42bc220b9f76a38ed0f1ee22030045cffe507bdeb1c99826a4c74e6")
        self.source(0x07E5FF, "b42bad131e99826ac220ad0f1ee220d0045c74e607")

    def test_raised_material_adds_authored_offset_to_live_support_height(self):
        self.source(0x07E61F, "e2205ab42bbdeb1c99826a7ac901d0045c3be607c904f0045c74e607c220b9e41c18790e008508")

    def test_only_selected_planes_publish_clipping_and_carry(self):
        self.source(0x07E64F, "c220a5088fe02470")
        self.source(0x07E664, "b42bc220a508997d6ae220287afa386bb42bc220a90000997d6ae220287afa186b")


if __name__ == "__main__":
    unittest.main()
