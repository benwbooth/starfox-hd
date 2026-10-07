"""Action-only player strategy initialization and its complete active body."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerActionWaitStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_initialization_clears_only_elapsed_then_installs_active_strategy(self):
        self.source(0x06837F, 'e220c210c220a99cbc9504e220207fdeb42bc220a9000099166ce220c220a99cbc9504e220a9048de51ca9008de61ca9008d741dc220a9c0839519e220a906951b')

    def test_active_strategy_disables_contacts_samples_selected_raw_controller_and_runs_action(self):
        self.source(0x0683C0, 'b521090195215ab5232940f0045cdb8306c220ad9612ac92128008c220ad9812ac94128d36198c3819e2207a22cfbc0d6b')

    def test_resetting_strategy_keeps_its_identity_and_visits_action_then_primary_palette(self):
        self.source(0x068362, 'e220c210a9048de51cad741d29af8d741db52109019521207fde4cbe84')
        self.source(0x0684BE, 'b42b5ab5232940f0045cd58406c220ad9612ac92128008c220ad9812ac94128d36198c3819e2207a22cfbc0d2267ea076b')


if __name__ == '__main__':
    unittest.main()
