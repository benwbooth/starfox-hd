"""Static launch transition source leaves; no source-machine execution."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class LaunchStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_craft_launch_installer_and_fixed_pilot_appearance_selector(self):
        self.assert_source(0x0DCE1C, 'c220a942dc992b00e22022aa2b7f22d22b7f')
        self.assert_source(0x7FBE19, 'a55e29e7855e8f3a3000ad141e220f81064ce8ca')
        self.assert_source(0x06810F, 'da5a08c2309b290f00c906009003a905000a0aaabf358106990400bf37810699cd1c287afa6b')
        self.assert_source(0x068135, '4cc2f4814cc2fe8268c2f48168c2fe82e8c5f481e8c5fe82')

    def test_camera_alignment_uses_high_heading_byte_and_three_signed_halves(self):
        self.assert_source(0x07F3B6, 'a03f03b9150049ff1a38f514c9806ac9806ac9806a18751495146b')
        self.assert_source(0x7FBE2D, '8eff1d4ce8ca')

    def test_published_shield_clears_complete_depth_word_and_only_low_phase_byte(self):
        self.assert_source(0x07F6D6, 'add11d48a9009de21cc220a900009dc81ce22068c90db019a9019de21ca5c48904d00e8901d00ac220a903009dc81ce2206b')


if __name__ == '__main__':
    unittest.main()
