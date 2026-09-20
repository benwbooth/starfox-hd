"""Source-only defender gate, player-contact death and unsafe-null cleanup."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM, MapExtractor


class CoreDefenderStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_ten_map_spawns_use_the_same_defender_mesh_and_independent_entry(self):
        commands = MapExtractor(self.rom).extract().commands
        installs = [c for c in commands if c.raw_hex.endswith('4cf3685e')]
        self.assertEqual([(c.address.bank, c.address.offset) for c in installs],
                         [(5, n) for n in [0x2B4B, 0x2B57, 0x2BA0, 0x2BAC, 0x4FBC,
                                           0x4FC8, 0x5270, 0x527C, 0x5288, 0x5294]])

    def test_gate_uses_full_progress_byte_and_player_contact_callback_not_new_contact(self):
        self.assertEqual(self.rom[0x480E5:0x480FB], bytes.fromhex('2e0c74818c7aa22b2aa2fff58016ea800cf4818c2f42'))
        self.assertEqual(self.rom[0x45E80:0x45E8A], bytes.fromhex('fd1b095b03147829865e'))
        self.assertEqual(self.rom[0x45E9B:0x45EA7], bytes.fromhex('0b5fa1ee2da1a65e4ca75e42'))

    def test_death_increments_the_found_controller_high_byte_before_unguarded_cleanup(self):
        self.assertEqual(self.rom[0x45EA7:0x45EC4], bytes.fromhex('4b9b5e5d4cf3026064000c68f3040df8f299be5e6da29bf6005c660b19'))
        # Retire-child has no null-result guard: retain the native diagnostic
        # rather than inventing a child or skipping the source command.
        self.assert_source(0x7F8B7B, '227b2a7fb925000908992500fa4cd3ca')


if __name__ == '__main__':
    unittest.main()
