"""Source contracts for typed attachment save/restore; no CPU execution."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class AttachmentStackStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_word_save_and_restore_share_the_actor_path_stack(self):
        self.assert_source(0x7FA771, '20bcc42047cbe220c220b900008d69b2bdde1c220f1a7f9dde1ce2204cd3ca')
        self.assert_source(0x7FA7AF, '20bcc42047cbe220c220bdde1c22c11a7f9dde1cad69b2990000e2204cd3ca')

    def test_authored_death_callback_saves_attachment_across_explosion_helper(self):
        self.assertEqual(self.rom[0x4F259:0x4F260], bytes.fromhex('9406417d8c9606'))


if __name__ == '__main__':
    unittest.main()
