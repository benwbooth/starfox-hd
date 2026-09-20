"""Fixed-view base copy and projectile invalidation are source-derived."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM
from extract_path import PathExtractor


class ViewTransitionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_cleanup_traverses_all_active_actors_and_changes_only_three_fields(self):
        self.assert_source(0x03A6A5,
            '085adae220c210aea812f02db400b5312950c950d0045ccba603'
            'b5312908c908f0045cdba603b52609089526b52109019521a900952d'
            'bbd0d3fa7a286b')

    def test_save_runs_cleanup_before_copying_exactly_sixty_three_base_bytes(self):
        extractor = PathExtractor(self.rom)
        self.assertEqual(extractor.handler_entry(0xC2).handler_address, 0x7FB295)
        self.assert_source(0x7FB295,
            'c220a902000c841be220b52609089526a55e29e7855e8f3a3000'
            '22a5a603c220a93f00224e197fa8e220dac220981869616aa8e220'
            'a23f0320f7b2fa')
        self.assert_source(0x7FB2F7,
            'da5aa93f8db116bd0000990000e8c8ceb116d0f37afa60')

    def test_restore_uses_same_fixed_base_and_frees_payload_but_not_auxiliary_entry(self):
        self.assertEqual(PathExtractor(self.rom).handler_entry(0xC3).handler_address, 0x7FB320)
        self.assert_source(0x7FB320,
            'c220a902001c841be220b52629f79526a908223b237fc00000d0045c6eb37f'
            'c220b9626aa8e220c220981869616aa8e220dabba03f0320f7b29b'
            'c2209838e9616aa8e220fac22098226b197fe220a9f720b8a34ce8ca')


if __name__ == '__main__':
    unittest.main()
