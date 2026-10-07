"""Complete actor-selection and proxy-drain boundary used by scene entry."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class SceneClearStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_selection_releases_proxy_before_marking_without_retiring_actor(self):
        self.source(0x03AC81, '08aea812f01abc0000b5222904d0045c9eac0322d6337fb52509089525bbd0e6')

    def test_drain_repeatedly_releases_active_head_to_lifo_free_list(self):
        self.source(0x03ACA1, 'ae8512f034c220bc0200d00ebc00008c8512a900009902008012dabd0000aa990000c90000f004989d0200faad83129d00008e8312e22080c7286b')

    def test_scene_entry_calls_cleanup_before_other_reset_publications(self):
        self.source(0x0683F8, '2281ac03a9008d1019a9ff8d0e198d0f19225fda0d')


if __name__ == '__main__':
    unittest.main()
