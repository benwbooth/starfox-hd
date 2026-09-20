"""Four-panel objective evidence from authored data, without gameplay recordings."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class FourPanelStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_path(self, address, expected):
        data = bytes.fromhex(expected)
        self.assertEqual(self.rom[0x40000 + address:0x40000 + address + len(data)], data)

    def test_map_entry_and_four_child_constructor_publish_numbered_coordinates(self):
        self.assertEqual(self.rom[0x2FBD6:0x2FBD9], bytes.fromhex('8c965b'))
        self.assert_path(0x5B96, 'fb9ad700fb64d7006104f5a4f2ac5a6404000060000000019c7aa10891d5fb06a18e91d1fb06a19290ddfb06a1956da14e13a1e564d79b45')
        offset = source_offset(0x06FBD1)
        self.assertEqual(self.rom[offset:offset + 16], bytes.fromhex('000000006000a0ff00000000c0400080'))

    def test_panel_height_gate_contact_threshold_break_and_parent_publication(self):
        self.assert_path(0x5AAC, 'c98d5c0cc0ffa3fd1008fd040019efa30ec15a5c425b420b50a1ee2da1ce5a4ccf5a424bc35a4bba5a5c41b4865d5ccc1c5e64000cc0f2049a6da97fa9089b3e7aa1080b19a262a107a2fb454ea1a24a0f5e02')

    def test_emitter_is_signal_gated_and_preserves_full_byte_count_arithmetic(self):
        self.assert_path(0x5B61, '5cb58dfd040a194c6c5b420b20a97aa13e0b01a2eea1a2915b7fa9085ddcf2005b0104e59ad79c7aa908c9080ef0009b56a9030f19')

    def test_five_contacts_start_lift_and_four_broken_parts_enter_shared_completion(self):
        self.assert_path(0x5C20, 'fd07092f6ba219')
        self.assert_path(0x5C27, '0b642d6da28a2aa205355c4c365c424b275c2e000495611975f844')
        self.assert_path(0x5C65, '2aa902795c2aa9037e5c8a2aa904785c4c865c424aa78505424ba7854aaf8504420004cb4b655c173f5d')
        self.assert_path(0x5D3F, 'f6009de8030057010000042be7f4d7e7a1d75cff000500')


if __name__ == '__main__':
    unittest.main()
