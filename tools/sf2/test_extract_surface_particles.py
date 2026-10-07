"""Exact ambient color pairs and source selection/range proof."""
from pathlib import Path
import unittest

import extract_surface_particles as particles
from rom import ROM_PATH, RUST_SRC


class SurfaceParticlesExtractionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(ROM_PATH).read_bytes()

    def test_generated_assets_match_all_reachable_palette_pairs(self):
        decoded = particles.decode(self.rom)
        self.assertEqual(tuple(map(len, decoded)), (8, 8))
        self.assertNotEqual(*decoded)
        self.assertEqual(Path(RUST_SRC, "surface_particles.rs").read_text(),
                         particles.render_rust(decoded))

    def test_source_color_index_is_random_low_two_bits_plus_zero_or_four(self):
        self.assertEqual(self.rom[0xC7DE:0xC7E2], bytes.fromhex("a0193fdf"))
        self.assertEqual(self.rom[0xC973:0xC975], bytes.fromhex("a204"))
        self.assertEqual(self.rom[0xC95D:0xC95F], bytes.fromhex("a200"))
        self.assertEqual(self.rom[0xC98B:0xC996], bytes.fromhex("273e733df05c28571e52ef"))

    def test_truncated_table_is_rejected(self):
        with self.assertRaises(ValueError):
            particles.decode(self.rom[:particles.STARTS[-1] + 7])


if __name__ == "__main__":
    unittest.main()
