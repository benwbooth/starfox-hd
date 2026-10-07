"""Mission progress and forced-retreat admission source boundaries."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerMissionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_special_configuration_compares_full_location_before_progress(self):
        self.source(0x069FAF, "ade21dc909d040c220adb51bc90b00e220f007ad87d7c9fef01b")

    def test_progress_flags_are_independent_and_set_before_music_control(self):
        self.source(0x069FC9, "ad87d7c9ffd026b9716a8940d01f094099716aa90322f86d7f8012b9716a8980d00b098099716aa90722f86d7f")

    def test_retreat_requires_objective_low_byte_inhibition_and_inactive_action(self):
        self.source(0x069FF6, "adf4d7f04ac220ad961b890001e220f03ec220b9136ce220f0045c45a006")

    def test_installer_resets_local_clocks_clears_only_trigger_and_skips_exit_controller(self):
        self.source(0x06A014, "5ab42ba90dd9156cf00399156cc220a963bfd9136cf00c99136ca9000099166c99186ce2207ab9776b29fe99776b4c16a3")


if __name__ == "__main__":
    unittest.main()
