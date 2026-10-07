"""Whole free-flight mode ordering and source-specific transformation cue."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerFreeFlightStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_plane_then_optional_effect_precede_input_history(self):
        self.source(0x06E334, "22c8e507b97d6b8980d0045c47e3062275c40722d9f206")

    def test_live_flight_vertical_and_shared_frame_precede_walker_request(self):
        self.source(0x06E36E, "20b1e420f7e320f6e92058e2a904850222cd9806")

    def test_only_successful_transformation_entry_queues_a_side_specific_cue(self):
        self.source(0x06E382, "9013c220a91c00ecc312f00309008022096e7fe220287a6b")


if __name__ == "__main__":
    unittest.main()
