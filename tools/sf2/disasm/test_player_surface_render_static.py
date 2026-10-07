"""Surface palette, shading and environment publication boundaries."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerSurfaceRenderStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_palette_entries_differ_in_refresh_and_source_row(self):
        self.source(0x07EB7F, "bf2c8a019dc5f0bf2c8a019dc5f3caca10ee")
        self.source(0x07EB99, "ad581e09808d581ec230a21e00bf6c8a019dc5f09dc5f3caca10f2")

    def test_shading_uses_carry_flag_not_the_separate_view_height_flag(self):
        self.source(0x07C355, "ad4d1b2907c902d06b5ab42bb9646b7a8902")
        self.source(0x07C377, "a98c8f4e0070a98b8f4f0070c220a9388f8f500070")
        self.source(0x07C3A6, "a90c8f4e0070a98b8f4f0070c220a93c8f8f500070")

    def test_negative_view_height_replaces_plane_gate_and_entire_ambient_control(self):
        self.source(0x07C3DD, "5ab42bb9646b7a8904f0045c0ec407")
        self.source(0x07C3EC, "c220a924fa8d111ead0f1e8fb91800a900008fbc0170a95dff8f5c2870a91000")

    def test_nonnegative_view_height_preserves_gate_and_other_ambient_flags(self):
        self.source(0x07C40E, "c220a900008fb91800afbc01700920000902008fbc0170a96dff8f5c2870a910008f5e2870")


if __name__ == "__main__":
    unittest.main()
