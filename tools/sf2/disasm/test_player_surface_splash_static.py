"""Crossing splashes have their own deferred movement and manual lifetime."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerSurfaceSplashStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_short_entry_clears_inherited_frame_and_size(self):
        self.source(0x07CBDC, "a954c0855fe22022172a7fb0045c6acc07a90499e31ca9008508a900850a4c1ccc")

    def test_long_entry_preserves_inherited_frame_and_size(self):
        self.source(0x07CC06, "a904bf855fe22022172a7fb0045c6acc07a90899e31c")

    def test_attachment_precedes_random_identity_and_formatting(self):
        self.source(0x07CC1C, "a910223d2a7fc2209899d81ce220b92600090899260022d07b7f991300")
        self.source(0x07CC42, "c220a96ecc991900e220a907991b0022608006")

    def test_first_visit_moves_horizontal_axes_once_then_falls_into_animation(self):
        self.source(0x07CC6E, "c220a995cc9519e220a907951bb406c220b50c18793200950ce220c220b510187936009510e220")

    def test_animation_keeps_manual_bit_including_terminal_frame(self):
        self.source(0x07CC95, "bde31c8502bdca1c1869013003186502297fc502900ba5023a09809dca1c4cbccc09809dca1c6bb525090895256b")

    def test_placement_clamps_wrapped_height_then_rotates_roll_and_yaw_without_pitch(self):
        self.source(0x07CCDC, "a50838f50e100ac981ffb00da981ff8008c97f009003a97f00")
        self.source(0x07CD2F, "b51622f03b7fa5048502a50a8508a5e48597b51422a9387f")
        self.source(0x07CD66, "fab92200090499220060")


if __name__ == "__main__":
    unittest.main()
