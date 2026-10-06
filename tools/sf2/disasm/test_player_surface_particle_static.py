#!/usr/bin/env python3
"""Particle lifetime, attachment identity, exact animation and motion owner."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerSurfaceParticleStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        start = source_offset(address)
        expected = bytes.fromhex(expected)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_distinct_shapes_install_custom_strategies_not_paths(self):
        self.assert_source(0x07C6E8, 'da5a08e220c210c220a954c0855fe22022172a7fb0045c23c807c220a914c7991900e220a907991b004c61c7')
        self.assert_source(0x07C738, 'da5a08e220c210c220a904bf855fe22022172a7fb0045c23c807c220a927c8991900e220a907991b00')

    def test_fresh_child_sixteen_has_self_extension_parent_pause_exemption_and_shared_formatter(self):
        self.assert_source(0x07C761, 'a910223d2a7fc2209899d81ce220b92600090899260022608006b920000920992000a90099c81ca50a99da1ca508098099ca1c')

    def test_player_retained_translation_is_read_after_rotation_and_parent_size_is_written_last(self):
        self.assert_source(0x07C801, 'c220da5ab42bbb7abd0b6b993200a90000993400bd0f6b993600e220faa9049dda1c287afa6b')
        # Flight publishes thrust velocity, then adds base-speed velocity.
        self.assert_source(0x06EE6C, 'c220b532990b6bb9616b1005a900008002b534990d6bb536990f6b')
        self.assert_source(0x06EE90, 'c220b53218790b6b990b6bb53418790d6b990d6bb53618790f6b990f6b')

    def test_both_terminal_frames_skip_integration_and_only_request_deferred_removal(self):
        self.assert_source(0x07C714, 'bdca1c1869013003186904297fc904900ba9043a09809dca1c4c61c809809dca1c4c48c8')
        self.assert_source(0x07C827, 'bdca1c1869013003186908297fc908900ba9083a09809dca1c4c61c809809dca1c')
        self.assert_source(0x07C848, '22242c7fc220b53222c2a9039532b53622c2a9039536e2206bb525090895256b')

    def test_horizontal_damping_adds_independently_rounded_half_and_eighth(self):
        self.assert_source(0x03A9C2, 'c900806a853ac900806ac900806a18653a6b')


if __name__ == '__main__':
    unittest.main()
