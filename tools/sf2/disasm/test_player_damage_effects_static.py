"""Sustained player particle flags, damage, puff speed and flame placement."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerDamageEffectsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_timer_wrap_and_roll_clear_precede_damage(self):
        self.source(0x07D1D0, "b9e46b89e0f014b9e56b1a99e56bc964f050b9dd6af004891ff047")

    def test_damage_uses_count_bits_but_shared_light_impact_then_zero_before_decrement(self):
        self.source(0x07D1F2, "a5c4291fd018b9e36b293fd011226faa06b9006cd004952d80013a99006c")

    def test_extinguish_preserves_bit_four_then_decrements_whole_flag_byte(self):
        self.source(0x07D232, "b9e46b291f090f99e46ba90099e56bb9e46b890ff0123a99e46b")

    def test_puff_has_independent_surface_gate_and_signed_half_plus_quarter_speed(self):
        self.source(0x07D068, "5ab42bb9946b7a2907c90590045cc2d007")
        self.source(0x07D0A4, "20f5beb518c9806a853ac9806a18653a991800c220a96d0022096e7fe220287afa6b")

    def test_flame_random_precedes_mode_and_has_independent_linked_gate(self):
        self.source(0x07D2DD, "5aa900850322d07b7f290f8502")
        self.source(0x07D32F, "5ab42bb9636b7a2980d0045c53d307")

    def test_flame_allocates_fresh_number_37_and_installs_real_deferred_path(self):
        self.source(0x07D353, "c220a9c4c0855fe22022172a7fb0045c9ad307a925223d2a7fdabb7ac220a91e7e9519e220a97f951b")
        self.source(0x07D3A3, "20f5bec220a940f5992b00e2207a60")


if __name__ == "__main__":
    unittest.main()
