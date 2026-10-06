"""Source scene-event publication, transition consumption and reset contract."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class SceneEventStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def test_scene_entry_clears_the_whole_event_word(self):
        start = source_offset(0x04DECD)
        self.assertEqual(self.rom[start:start + 6], bytes.fromhex('a9ffff1c881b'))

    def test_transition_consumer_observes_the_announced_bit_and_clears_only_its_pair(self):
        for address, source in [
            (0x04B588, 'ad881b890002d027'),
            (0x04B5B7, 'a900421c881ba932008d0fda60'),
            (0x04B53C, 'a900021c881b'),
        ]:
            start = source_offset(address)
            expected = bytes.fromhex(source)
            self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_announcement_preserves_arguments_and_publishes_before_optional_message(self):
        expected = bytes.fromhex('94a393a1932f7ca3881b0b0aa1d8a1a37ea3881b792f701e2a2fff2d800c9200a355a32fdfa3952f95a196a342')
        self.assertEqual(self.rom[0x48007:0x48007 + len(expected)], expected)


if __name__ == '__main__':
    unittest.main()
