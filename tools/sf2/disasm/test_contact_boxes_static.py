#!/usr/bin/env python3
"""ROM-data checks for object-contact boxes; no execution or recordings."""

from pathlib import Path
import sys
import subprocess
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from extract_contact_boxes import parse, render, COPIED_BANK_FILE_BASE
from rom import load_rom


class ContactBoxStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = load_rom()
        cls.roots, cls.groups = parse(cls.rom)

    def test_all_shape_roots_and_every_selectable_chain_are_closed(self):
        self.assertEqual(len(self.roots), 61)
        self.assertEqual(len(set(self.roots.values())), 37)
        self.assertEqual(len(self.groups), 94)
        self.assertEqual(sum(len(group.variants) for group in self.groups.values()), 94)
        self.assertTrue(all(group.variant_mask == 0 for group in self.groups.values()))
        for group in self.groups.values():
            for box in group.variants:
                self.assertTrue(box.next_group == 0 or box.next_group in self.groups)

    def test_laser_profile_and_player_wing_boxes_come_from_shape_headers(self):
        self.assertEqual(self.roots[357], 0x4B9F)
        laser = []
        cursor = self.roots[357]
        while cursor:
            box = self.groups[cursor].variants[0]
            laser.append(box)
            cursor = box.next_group
        self.assertEqual([box.center for box in laser], [(0, 0, 0), (0, 0, -20), (0, 0, -40)])
        self.assertTrue(all(box.rotation_flags == 0x82 and box.half_extents == (40, 40, 40) and box.hit_flags == 7 for box in laser))
        self.assertEqual(self.roots[52], 0x494D)
        wings = self.groups[0x495F].variants[0], self.groups[0x4971].variants[0]
        self.assertEqual([box.center for box in wings], [(-25, 10, -15), (25, 10, -15)])
        self.assertEqual([box.hit_flags for box in wings], [2, 4])

    def test_cycles_are_rejected_instead_of_truncated(self):
        data = bytearray(self.rom)
        root = self.roots[357]
        offset = COPIED_BANK_FILE_BASE + root
        data[offset:offset + 2] = root.to_bytes(2, "little")
        with self.assertRaisesRegex(ValueError, "cyclic contact-box chain"):
            parse(data)

    def test_generated_native_catalog_is_reproducible_from_rom_alone(self):
        generated = subprocess.run(
            ["rustfmt", "--edition", "2021", "--emit", "stdout"],
            input=render(self.rom), text=True, capture_output=True, check=True,
        ).stdout
        root = Path(__file__).resolve().parents[3]
        self.assertEqual(generated, (root / "rust/sf2-data/src/contact_box_data.rs").read_text())


if __name__ == "__main__":
    unittest.main()
