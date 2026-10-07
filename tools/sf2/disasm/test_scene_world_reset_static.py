"""World clear callers disable region scanning without erasing definitions."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class SceneWorldResetStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_action_cleanup_preserves_both_region_selections(self):
        self.source(0x0DC956, '08da5a082281ac03a9008d1019225fda0d287afa2860')

    def test_player_entry_clears_count_and_both_groups_before_occupancy(self):
        self.source(0x0683F1, 'e220c210da5a082281ac03a9008d1019a9ff8d0e198d0f19225fda0d287afa')

    def test_full_occupancy_fill_is_exactly_2048_bytes(self):
        self.source(0x0DDA5F, '088be220c210a97e48aba9ffa200009d36cfe8e00008d0f7ab286b')


if __name__ == '__main__':
    unittest.main()
