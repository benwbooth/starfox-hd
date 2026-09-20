"""Scene re-entry preservation is source-derived, not recorded gameplay."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class SceneContinuationStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_preservation_updates_proxy_or_retains_current_not_next_path_in_auxiliary_three(self):
        self.assert_source(0x7FAFCB, 'bce61cf00ac220b52b990f004ce8cac220b52bc22048e220a9032260237fc2206899626ae220c2204ce8ca')
        self.assertEqual(self.rom[0x4167E:0x41684], bytes.fromhex('4c821642b60f'))

    def test_capture_copies_retained_value_without_consuming_it(self):
        self.assert_source(0x7FAF5C, '5aa90300223b237fc00000d006c2205c7aaf7fc220b9626ae220c2208003b52b1a7a990f00')
        self.assert_source(0x7F2360, 'e2208d2acf bcec1c f031 b9616a8d29cfc8b9616acd2acff058')
        self.assert_source(0x7F23CB, 'ad2acf99616a6b')


if __name__ == '__main__':
    unittest.main()
