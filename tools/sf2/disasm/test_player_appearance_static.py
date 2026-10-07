"""Shared pilot material, low-shield appearance, and carry/depth ownership."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerAppearanceStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_pilot_side_material_precedes_reserve_shield_test(self):
        self.source(0x06AA20, "b42bb9ff6b2901d00cc220a9f4819dcd1ce220800ac220a9fe829dcd1ce220b9006cc90d")

    def test_particle_call_precedes_whole_override_and_recovery_clears_it(self):
        self.source(0x06AA44, "b01a22b1cf07a98099a26aa5c48904d0178901d013a98399a26a800cb9a26a8980f005a90099a26a287a6b")

    def test_damage_emission_uses_real_contact_flag_linked_mode_and_clock(self):
        self.source(0x07CFB3, "c220b504c99cbce220d0045c45d007b5232902f0045c45d007b52dd0045c45d007")
        self.source(0x07CFD4, "5ab42bb9726a7a2910f0045c45d0075ab42bb9636b7a8940f0045cfacf072980f0045c45d007a5c42901f0045c45d007")

    def test_damage_allocation_retains_caller_number_and_delays_real_path(self):
        self.source(0x07D004, "c220a98cc0855fe22022172a7fb0045c2fd007a500223d2a7fc220a91e7e991900e220a97f991b00")
        self.source(0x07D038, "20f5bec220a921f5992b00e220287a6b")

    def test_surface_preserves_override_before_carry_read_and_publishes_full_depth(self):
        self.source(0x07C32F, "b9a26a3021ad131ec901f0045c50c307b5212920d0045c50c307a90299a26a8005a90099a26a")
        self.source(0x069F21, "dac220b9a26a2903000aaabf1da306fa9dc81ce220")
        self.source(0x06A31D, "0000010002000300")


if __name__ == "__main__":
    unittest.main()
