"""Exact lighting catalog extraction and source loader selections."""

from pathlib import Path
import unittest

import extract_lighting as lighting
from rom import ROM_PATH, RUST_SRC


@unittest.skipUnless(Path(ROM_PATH).is_file(), "retail SF2 ROM is not present")
class LightingExtractionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(ROM_PATH).read_bytes()

    def test_checked_in_catalog_is_exact_and_all_families_are_retained(self):
        families, shades, thresholds = lighting.decode(self.rom)
        self.assertEqual(len(families), 5)
        self.assertEqual(len(shades), 4)
        self.assertEqual(len(thresholds), 14)
        self.assertGreater(len({tuple(sum(family, [])) for family in families}), 1)
        self.assertEqual(
            Path(RUST_SRC, "lighting.rs").read_text(),
            lighting.render_rust(families, shades, thresholds),
        )

    def test_truncated_source_and_invalid_catalog_pointer_fail_closed(self):
        with self.assertRaises(ValueError):
            lighting.decode(self.rom[: lighting.DEPTH_THRESHOLDS_START + 55])
        changed = bytearray(self.rom)
        changed[lighting.LIGHT_POINTERS_START] ^= 1
        with self.assertRaises(AssertionError):
            lighting.decode(changed)
        changed = bytearray(self.rom)
        changed[lighting.DEPTH_THRESHOLDS_START + 3] = 1
        with self.assertRaises(AssertionError):
            lighting.decode(changed)

    def test_setup_and_main_handoff_select_distinct_threshold_records(self):
        # Source setup changes thresholds without selecting a colour family.
        self.assertEqual(self.rom[0x10BBF:0x10BCA], bytes.fromhex(
            "C2 20 A9 2C 8F 8F 50 00 70 E2 20"
        ))
        # The main-loop tail runs only after the sprite palette service.
        self.assertEqual(self.rom[0x1D52C:0x1D537], bytes.fromhex(
            "C2 20 A9 40 8F 8F 50 00 70 E2 20"
        ))
        self.assertEqual(self.rom[0x1D559:0x1D565], bytes.fromhex(
            "A9 0C 8F 4E 00 70 A9 8B 8F 4F 00 70"
        ))
        self.assertEqual(self.rom[0x1D565:0x1D570], bytes.fromhex(
            "AD 9B 12 8D 0C 42 A5 00 D0 FC 6B"
        ))


if __name__ == "__main__":
    unittest.main()
