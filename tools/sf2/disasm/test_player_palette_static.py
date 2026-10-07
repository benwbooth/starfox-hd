"""Exact primary-player palette dispatch and all three reachable color loops."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerPaletteStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_primary_selection_delay_mutation_and_branch_precedence(self):
        self.source(0x07EA67, '8bda5a08e220c210a97e48abaec312b42bb9ea6bf0153a99ea6bf007b9e96b09188005b9e96b29f799e96bb9e96b8908d01a8910d01c8920d0068940d01d801fa5c48918d01522d6eb078013229aec07800d22b2ee07287afaab6b22b2ee07287afaab6b')

    def test_pulse_preserves_red_and_decreases_green_one_blue_two(self):
        self.source(0x07EBD6, 'da5a08b42be220c210ad581e09808d581ec230a2de00bd05f048291f001003a900008504684829e00338e920001003a90000050485046829007c38e9000438e900041003a9000005049d05f0caca10c6287afa6b')

    def test_flash_skips_each_bank_first_color_and_uses_distinct_component_caps(self):
        self.source(0x07EC9A, 'da5a08e220ad581e09808d581ec230b42ba20400863ea28000a98003850aa9007085a7a91f008508a903008597a9000485e4e220ad581e09808d581ec2209cae1d8eb01da43ee220ad581e09808d581ec2208a291f00f03bbde5ef48291f00186597c5089002a5088504684829e00318692000c50a9002a50a050485046829007c1865e4c5a79002a5a70504850405049de5ef88f0098a18692000aa4ce0ecadae1d1a1a8dae1dc92000f008186db01daa4cdeec287afa6b')

    def test_restore_includes_bank_first_colors_and_tests_full_words_before_approach(self):
        self.source(0x07EEB2, 'da5a08c230b42ba20400863ea28000e220ad581e09808d581ec230643a9cae1d8eb01da43ebde5efdde5f2d0045c45ef0748291f008502684829e00385086829007c8597bde5f248291f00c502f0083004e6028002c602684829e003c508f0123005a920008003a9e0ff18650829e00385086829007cc597f0103005a900048003a900fc18659729007c050805029de5efe63a88f0098a18692000aa4cd7eeadae1d1a1a8dae1dc92000f008186db01daa4cd5ee287afa5a08c220a53ae220d013b42bb9e96b29ef99e96b8920d00529bf99e96b287a6b')


if __name__ == '__main__':
    unittest.main()
