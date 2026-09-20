"""Scenery-placement source contracts, without executing original code."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class SceneryEmitterStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_selected_copy_probe_and_deferred_removal_are_distinct_immediate_actions(self):
        self.assert_source(0x09B0CC, 'ac1fcfc220b90c00950cb910009510b90e00950ec220a9e6b06b')
        self.assert_source(0x09B117, '223aaf0dc220a921b16b')
        self.assert_source(0x09B121, '7ca3080068a336b189b52509089525c220a936b16b7b0e0b9b42')

    def test_surface_query_exports_height_and_only_normalizes_the_reduced_sentinel(self):
        self.assert_source(0x7F1B76, 'ad4d1b290700c90000f005a900208003a900408d5d199c5f199c6119')
        self.assert_source(0x0DB1FE, 'ad5d19c90020d003a900008508ad5f199fe81c7e')

    def test_selected_particle_or_is_distinct_from_the_action_byte_and_keeps_all_bits(self):
        self.assert_source(0x7FBAA5, 'ac1fcfdab62b9bfa20bcc419e46b99e46b4cd3ca')
        self.assert_source(0x7FB783, 'b9776b29fe99776b')
        self.assert_source(0x07D2C2, 'b9e46b2920d0045cd9d207c220a90000853ae22020ddd2')
        self.assert_source(0x07D355, 'a9c4c0855fe22022172a7f')

    def test_emitter_stores_are_heights_and_last_spawn_import_is_an_attachment(self):
        self.assert_source(0x09B050, '480310fc67d700004118a51751b0480308fc67d70cfe5d84ec7cb00a0a7b061541c7b0165fb0')
        self.assert_source(0x09A518, '5dc4c024a50a0afa8841c7b05c4d0000f9fd180075c4782d0e18fc0cfe3ea55dc4c024a50a0a03070f6f991c01049750004ea51751a500162042')


if __name__ == '__main__':
    unittest.main()
