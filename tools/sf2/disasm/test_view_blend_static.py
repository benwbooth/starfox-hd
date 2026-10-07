"""Fixed-view continuity uses shared base flags and final pose snapshots."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class ViewBlendStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_capture_discard_clears_only_the_three_shared_request_bits(self):
        self.source(0x079837, "b5212920d0045c569807b52129df9521b52129f79521b52129ef95214cbf98")

    def test_position_and_angle_decay_precede_rotation_then_position_and_snapshot(self):
        self.source(0x0798BF, "20a39920399a20609920d298208399287afa6b")
        self.source(0x0799B4, "a9000022e8257f")
        self.source(0x079A05, "a900002234257f")

    def test_final_snapshot_retains_whole_angles_and_base_position(self):
        self.source(0x079983, "c220b50c9539b50e953bb510953db5129dc11cb5149dc31cb5169dc51ce22060")
        self.source(0x079A99, "bdd51c1dd61c1dd71cd006b521297f952160")


if __name__ == "__main__":
    unittest.main()
