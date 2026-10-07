"""Source bounds for map-installed auxiliary cameras and their shared owners."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerCameraAuxiliaryStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_footer_increments_whole_counter_before_optional_task(self):
        self.source(0x079DFD, "a03f03c220b9e41c1a99e41ce220b42bc220b99d6af01c")

    def test_all_map_task_installers(self):
        for address, target in [(0x0DC75C, "179f"), (0x0DC783, "349e"),
                                (0x0DC795, "449f"), (0x0DC7BC, "f79e"),
                                (0x0DC7E3, "ca9e")]:
            self.source(address, "08a907999f6ac220a9" + target + "999d6ae220")
        self.source(0x0DC80A, "08c220a90000999d6ae220999f6a")
        self.source(0x0DC791, "e2202860")  # Handoff has no blend request.
        for address in [0x0DC770, 0x0DC7A9, 0x0DC7D0, 0x0DC7F7]:
            self.source(address, "b921000908992100b921000910992100")

    def test_initialization_changes_task_then_falls_into_first_update(self):
        for address, target in [(0x079ECA, "e09e"), (0x079EF7, "0d9f"),
                                (0x079F17, "2d9f")]:
            self.source(address, "200da15ab42ba907999f6ac220a9" + target + "999d6ae2207a20")
        self.source(0x079F44, "5ab42bc220a9b0ff99546be2207a200da1")
        self.source(0x07A10D, "5a8eff1db42ba03f03b9150049ff1a99e21cc220a9000099e41c")

    def test_orientation_restores_previous_full_angles_then_resets_steering(self):
        self.source(0x079F78, "c220bdc11c9512bdc31c9514bdc51c9516e220acff1d9c9d1422a5217f")
        self.source(0x079FAB, "a504c900806a8504")

    def test_focus_copies_velocity_before_position_but_other_tasks_after_aim(self):
        self.source(0x079EE0, "202ba1204aa120729f")
        self.source(0x079F0D, "20e29f20729f202ba16b")
        self.source(0x079F2D, "2028a020729f202ba1")
        self.source(0x079F68, "2012a020729f202ba16b")

    def test_focus_framing_reads_real_timer_and_published_player_position(self):
        self.source(0x07A1BA, "adecd7990c00adeed7990e00adf0d7991000")
        self.source(0x07A1D0, "ad0a1c89fefff003a20400")
        self.source(0x07A227, "140088ffceff6aff")
        # Actual scene-clock producer, not a presentation-frame inference.
        self.source(0x03C5AF, "ad0a1cc96300f01b9c63daa9f0008d61daee0a1c")

    def test_handoff_heading_word_and_saved_view_position_are_distinct(self):
        self.source(0x079E4B, "ad8e1d991400e220")
        self.source(0x079EAF, "8cff1d20729f202fa2")
        self.source(0x07A2D8, "b93900990c00b93b00990e00b93d00991000")


if __name__ == "__main__":
    unittest.main()
