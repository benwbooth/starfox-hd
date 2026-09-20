"""Source-only completion-word ownership and callable helper contracts."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class ObjectiveCompletionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_campaign_load_and_writeback_copy_full_objective_word(self):
        self.assert_source(0x04B2A4, 'c220a63abda2d78d9fd7e22060')
        self.assert_source(0x04B2E6, '8a0aaaad9fd79da2d728fa60')

    def test_query_and_record_preserve_complete_source_control_flow(self):
        self.assertEqual(self.rom[0x487D3:0x487E5], bytes.fromhex('7ba3436d27da27a3e0876ca3420c6400a342'))
        self.assertEqual(self.rom[0x487E5:0x4880B], bytes.fromhex('94a393a17ba3437aa145d827a380a3432c270008018807a1f01703886fa17fa14595a196a342'))

    def test_variable_selector_wraps_before_word_lookup_and_bit_predicate_bypasses_ifnot(self):
        self.assert_source(0x7FB5FB, '20bcc42047cbb900003a0ac22029ff008eb116aabfcfb57faeb11660')
        self.assert_source(0x7FB652, '20efb5390000f00382ae144c94ca')


if __name__ == '__main__':
    unittest.main()
