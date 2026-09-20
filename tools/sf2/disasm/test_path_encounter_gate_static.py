"""Encounter gate publications verified from static source, never execution."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM, MapExtractor


class EncounterGateStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_map_installs_gate_and_helper_uses_five_decoded_shape_position_pairs(self):
        commands = MapExtractor(self.rom).extract().commands
        self.assertTrue(any(c.address.bank == 5 and c.address.offset == 28314
                            and c.opcode == 0x8C and c.raw_hex == '8c7e4d' for c in commands))
        self.assert_source(0x06FC5F, 'e0fc000f2004c0f400fb44e79ce6f0e698e710ca')

    def test_handoff_or_preserves_flags_and_consumers_choose_their_own_height(self):
        self.assert_source(0x7FC1AD, 'ad741d09408d741d4ce8ca')
        self.assert_source(0x079E34, 'acd614c220ad881d990c00a9ceff990e00ad8c1d991000ad8e1d991400')
        self.assert_source(0x07A22F, 'da5a08aed614c220ad881d950cad8c1d9510a90000950ee220ad8e1d9514a90095129516')

    def test_node_mode_is_separate_from_layout_and_initialized_then_set_from_flags(self):
        self.assert_source(0x04B474, 'e2209c9bd7c220')
        self.assert_source(0x04B05A, 'a902008d7ddab91200aabd1c000980009d1c00bd1e00890200f007e220a9018d9bd7')


if __name__ == '__main__':
    unittest.main()
