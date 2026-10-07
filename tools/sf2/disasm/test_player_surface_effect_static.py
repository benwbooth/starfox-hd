"""Protected surface effects retain distinct origins and real actor lifetimes."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerSurfaceEffectStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_child_limit_and_walker_gate_precede_real_allocation(self):
        self.source(0x07C480, "b429f005e63abb80f7fa7aa53ac90590045c1ac607")
        self.source(0x07C4A6, "5ab42bb9946b7a2907c905b0045c1ac607")
        self.source(0x07C4C0, "22172a7fb0045c1ac607a90f223d2a7f")

    def test_formatter_precedes_separate_origin_and_retained_steering_response(self):
        self.source(0x07C4F4, "22608006c220a504992b00e220daa604")
        self.source(0x07C53A, "b512224e3a7fa5048502a50a8508a5e48597b51422a9387f")
        self.source(0x07C566, "a50a187d0e00990e00e220fa")

    def test_initial_visit_defers_motion_and_crossing_precedes_lifetime_decrement(self):
        self.source(0x07C61E, "c220a931c69519e220a907951ba9fb9dda1c6b")
        self.source(0x07C67D, "b42bc220b50c853ab90c002267257f950ce220")
        self.source(0x07C6CA, "c220b50edde41ce22010045ce1c607d60af0045ce7c607b525090895256b")


if __name__ == "__main__":
    unittest.main()
