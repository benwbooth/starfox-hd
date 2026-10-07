"""Entry cleanup silences output independently from retained sound identity."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerEntryResetStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_action_termination_retains_total_update_count(self):
        self.source(0x068413, '5ab42bc220a9000099136c99166c99186ce22099156c7a')

    def test_entry_tail_orders_shape_audio_handoff_collision_camera_and_launch_counts(self):
        self.source(0x06842A, 'c220a99cbc9504e220a9048de51ca9008de61cad741d29af8d741db521090195215ab42bc220a90000999a6ae220999c6a7ac220a900008d691d8d6b1d8d6d1de220')
        self.source(0x06846C, 'ad721dd010')

    def test_audio_publisher_keeps_output_separate_from_nearest_actor_accumulator(self):
        self.source(0x03815A, 'c220ad841b890100e220d02cadec1c8de61caded1c8de71caeee1c8ee81caef41c8eea1cc220adf41c1ad00c9ce61c9ce71ca200008ee81ce220')


if __name__ == '__main__':
    unittest.main()
