"""Four-turret encounter source contracts, without recorded execution."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class RadialTurretStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_path(self, address, expected):
        raw = bytes.fromhex(expected)
        self.assertEqual(self.rom[0x40000 + address:0x40000 + address + len(raw)], raw)

    def test_constructor_passes_four_fresh_low_selectors_to_child_attack(self):
        self.assert_path(0xF1D5, '41548d5c6c0e2e6104f50cc41bf21400000060ff0000017fa1089c7a2e089b6da145')

    def test_offsets_and_headings_are_two_word_tables_and_one_byte_table(self):
        start = source_offset(0x07FEA1)
        self.assertEqual(self.rom[start:start + 20], bytes.fromhex('000020030000e0fce0fc00002003000080c00040'))
        self.assert_path(0xF21B, 'f881f291a1fe072e8e91a9fe072e9290b1fe072e950b012e')

    def test_gate_saves_low_and_enables_collision_only_at_progress_255(self):
        self.assert_path(0xF281, '93a15c7aa12b8a2aa1ff91f25b4b81f295a142')

    def test_death_decrements_original_link_then_restores_it_after_nearest_search(self):
        self.assert_path(0xF252, '4b81f29a6f2d9b9406417d8c96060b012d5c1b014c3ff20b01ae42')
        self.assert_path(0x8C7D, '0dccbe99868c6da19b42')

    def test_release_is_a_health_one_wait_and_exit_marks_one_child(self):
        self.assert_path(0xF1F7, '2a2d01fff116f7f10004950004ba41628d0b01ae0c140034e20100030f0004cb0004cd0f')
        self.assert_path(0xF187, 'f821c6fe0800030a4aa9f101031e005b66010f')


if __name__ == '__main__':
    unittest.main()
