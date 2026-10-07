"""Complete retained-pitch mode ordering, history and unsided cue."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerFlightModeStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_input_history_precedes_camera_bank_and_three_shared_target_resets(self):
        self.source(0x06E2EE, "5a08e220c210b42b22d9f206a90099c06aa000008c361e8c381e8c3a1e")
        self.source(0x06F2D9, "5a08c230b42bb9896b4d381919876b2d381999876bad381999896b287a6b")

    def test_held_pitch_and_hard_limits_precede_the_whole_shared_flight_call(self):
        self.source(0x06E30B, "20b1e4209ae32010ea2058e2")

    def test_select_edge_cue_has_no_player_side_test_or_transition_substitute(self):
        self.source(0x06E317, "ad37198920f00bc220a9360022096e7fe220287a6b")


if __name__ == "__main__":
    unittest.main()
