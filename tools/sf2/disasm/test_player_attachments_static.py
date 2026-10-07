"""Source attachment-chain and temporary Walker publication contracts."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerAttachmentsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_contacts_disabled_still_refresh_and_only_walker_overrides(self):
        self.source(0x069F54, "5ab42bb9726a7a2910f0045c749f065ab42bb9a06a7a29f0c920d0045c7a9f062219237f8033")

    def test_walker_saves_pose_then_publishes_retained_height_and_wrapping_yaw(self):
        self.source(0x069F7A, "b42bc220b50e8dae1de220b5148db11dc220b9f36b950ee220b5141879ee6a95142219237f")

    def test_walker_restores_only_height_and_yaw(self):
        self.source(0x069F9F, "c220adae1d950ee220adb11d9514")

    def test_chain_advances_the_single_link_after_every_published_child(self):
        self.source(0x7F2322, "b429f007bb2229227f80f5")


if __name__ == "__main__":
    unittest.main()
