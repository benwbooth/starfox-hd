"""Transition clear preserves the final 81 bytes of the player allocation."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerMotionResetStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_complete_partial_clear_is_391_bytes_not_the_full_allocation(self):
        self.source(0x06DA38, 'da5a08e220c210b42ba900a2000099616ac8e8e08701d0f6287afa60')
        self.assertEqual(0x6A61 + 391, 0x6BE8)
        self.assertEqual(472 - 391, 81)

    def test_equipment_restore_is_a_distinct_following_call(self):
        self.source(0x06DEBD, 'b42b2038da20ffd9')
        self.source(0x06D9FF, '5a08b42be220c210add41d99066cadd31d99056cadd21d99046c287a60')

    def test_full_preparation_clears_ambient_low_byte_and_shared_steering_first(self):
        self.source(0x06DE7F, '5a08e220c210a55e29f7855e8f3a3000a9008fbc0170a55e0908855e8f3a3000c220a900008d361ee220c220a900008d381ee220c220a900008d3a1ee220')

    def test_motion_and_carry_clear_before_surface_kind_without_touching_velocity(self):
        self.source(0x06DEC5, 'c220a900009dc11c9dc31c9dc51c9539953b953de220b52129df9521ad4d1b29f809018d4d1b287a60')


if __name__ == '__main__':
    unittest.main()
