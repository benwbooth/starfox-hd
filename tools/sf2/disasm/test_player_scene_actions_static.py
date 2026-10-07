"""Source-complete timed/empty scene streams and their actual publications."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_intro_controller import TimingCondition, authored_scene_controller
from extract_map import DEFAULT_ROM


class PlayerSceneActionsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def test_whole_streams_have_only_the_lowered_services_in_authored_order(self):
        expected = [
            (3, 0x0DC4F3, [(180, 0x0DCA18)]),
            (4, 0x0DBEB4, [(144, 0x0DCA18)]),
            (5, 0x0DBED3, [(0, 0x0DC6E1), (227, 0x0DCA18)]),
            (9, 0x0DC191, []),
            (25, 0x0DBEBB, [(124, 0x0DCA18)]),
        ]
        for scene, pointer, commands in expected:
            with self.subTest(scene=scene):
                stream = authored_scene_controller(self.rom, scene)
                self.assertEqual(stream.script, pointer)
                self.assertEqual([(item.start, item.service) for item in stream.commands], commands)
                self.assertTrue(all(item.condition == TimingCondition.AT for item in stream.commands))
                end = source_offset(pointer) + 5 * len(commands)
                self.assertEqual(self.rom[end:end + 2], b'\xff\xff')

    def test_services_write_only_scene_exit_and_projection_correction_bits(self):
        for address, value in [
            (0x0DCA18, '08c220a910000c961be2202860'),
            (0x0DC6E1, '08b9656b094099656b2860'),
        ]:
            expected = bytes.fromhex(value)
            start = source_offset(address)
            self.assertEqual(self.rom[start:start + len(expected)], expected)


if __name__ == '__main__':
    unittest.main()
