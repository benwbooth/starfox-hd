"""Source bounds for surface-camera placement and shared plane transitions."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerCameraSurfaceStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_surface_yaw_consumes_difference_before_its_decay(self):
        self.source(0x078158, "c220b9336b38f9396b99336be220c220b9396b")

    def test_surface_publishes_prepared_xz_but_uses_separate_retained_height(self):
        self.source(0x0781D4, "207a8a20bf88c220adc21d99c16aadc61d99c56ae22020a390")

    def test_standing_clears_only_follow_hold_and_recovery(self):
        self.source(0x07911A, "b97d6b29a7997d6b4c2b94")
        self.source(0x07915D, "b97d6b29a7997d6b")

    def test_finished_plane_return_clears_only_secondary_protection_and_crossing(self):
        self.source(0x07930C, "b97d6b295f997d6b4c8594")

    def test_height_bounding_is_followed_by_conditional_eighth_chase(self):
        self.source(0x07943D, "c220a9320085a5e22022bab906")
        self.source(0x079466, "c220b9c36a853aadae1d22a3257f99c36a")

    def test_bounded_half_helper_clamps_distance_before_smoothing(self):
        self.source(0x06B9BA, "c220a50238e504300e38e5a59017a50238e5a58504800e1865a5b009a5021865a585048000")


if __name__ == "__main__":
    unittest.main()
