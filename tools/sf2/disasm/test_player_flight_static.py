"""Shared flight order and collision-traversal handoff provenance."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerFlightStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_roll_ambient_throttle_surface_speed_pose_and_translation_order(self):
        self.source(0x06E258, "08e220c21020ffe720f7f22010f0226fdd07205df020b0ec200aee")

    def test_damage_precedes_shared_boundary_gate_and_ordered_corridor_grid(self):
        self.source(0x06E2D0, "7a228ee107ada61a8902d0045cece20622f3e2072285e60722f2e2072860")
        self.source(0x07E2F2, "6b")

    def test_traversed_surface_candidate_clears_bias_before_profile_selection(self):
        self.source(0x0DAF86, "bfcb1c7e1004297f8004a5c429ff853e643fc220a900008f591900845fbe1600e45fd058")


if __name__ == "__main__":
    unittest.main()
