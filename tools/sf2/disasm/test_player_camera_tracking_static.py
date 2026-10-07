"""Byte-bound branch order and rounding for retained camera height tracking."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerCameraTrackingStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_linked_reset_writes_tracking_and_position_before_height_flags(self):
        self.source(0x078DC7, "c220a9000099476ba9ecff994e6bb50e99c36a99456be220b9306b29c099306bb512")

    def test_direction_requests_have_distinct_retained_bit_masks(self):
        self.source(0x078EDC, "b9306b8901d00229c509058023")
        self.source(0x078EFA, "b9306b8908d00229e809288005b9306b29e499306b")

    def test_vertical_offset_uses_signed_floor_halves_not_toward_zero(self):
        self.source(0x07907A, "c220adae1dc900806a853ac900806ac900806ac900806a49ffff1a18653a49ffff1a994e6b287afa60")


if __name__ == "__main__":
    unittest.main()
