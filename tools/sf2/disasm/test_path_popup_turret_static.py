"""Popup turret source blocks, inspected as static bytes only."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PopupTurretStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start+len(expected)], expected)

    def test_layout_is_published_from_campaign_node_separately_from_location(self):
        self.assert_source(0x04B1FC, 'ae07dbbd0400c22029ff00e2208db51bbd08008da51bbd0a008df6d7')
        self.assert_source(0x08AF5C, '79a1b51b8a2aa1087b2f79a1a51b8a2aa10f7b2f487aa12e69a17b2f16712f')

    def test_masked_destination_keeps_all_working_writes_and_both_tables(self):
        self.assert_source(0x08AF8E, '58a10750398e503d929173fe06a1a35439a39183fe06a1a3543da3')
        self.assert_source(0x06FE73, '00fd0003000000fe0002000000fd00030003000300020000000000fe00fd00fd')

    def test_discharge_restores_captured_pose_on_every_shrink_and_fires_after_ten_visits(self):
        self.assert_source(0x098C4A, '50800c50820e508410f8758c4d0008cd5c391a000ff2fa7b610a079902500c80500e82501084ae1e44350f67a17c8c4c748c42')


if __name__ == '__main__':
    unittest.main()
