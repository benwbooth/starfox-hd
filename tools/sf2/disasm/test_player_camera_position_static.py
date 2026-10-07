"""Byte-bound camera position scheduling and retained shared-response fields."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerCameraPositionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_common_position_caller_orders_height_distance_lateral_ambient_and_boost(self):
        self.source(0x078507, "20668d207a8a20bf88c220adc41d1879e26a8dc41d99c36aadc21d99c16aadc61d99c56ae22020d38b")

    def test_lateral_smoothing_uses_shared_motion_impulse_with_signed_byte_extension(self):
        self.source(0x07898B, "c220b9ad6a898000f0050900ff800329ff0018653a853a")
        self.source(0x0789E4, "c220b94a6b853ab9e76a2267257f994a6b")

    def test_surface_distance_and_boost_share_longitudinal_offset_but_use_different_rates(self):
        self.source(0x078B70, "c220b9526b853aa5972236267f99526b")
        self.source(0x078C7A, "c220b9526b853aadc01d22a3257f99526b")

    def test_boost_target_precedes_timer_decay_and_projection_reads_updated_impulse(self):
        self.source(0x078C3E, "1879566b8dc01de220b9586bf0193a99586b")
        self.source(0x078C8D, "c220b9526b0a1879566b8dc01de220207a9d")


if __name__ == "__main__":
    unittest.main()
