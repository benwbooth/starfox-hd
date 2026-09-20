#!/usr/bin/env python3
"""Complete attached-projectile graph and the shared trigger's producers."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class TriggeredProjectileStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_full_parent_child_and_callbacks_with_inline_continuation(self):
        self.assert_source(0x09F48B, "5c 33 60 bd ca f4 e7 00 00 01 01 00 00 f6 ff 00 00 01 7c 1c c3 12 f8 b6 f4 61 0c 79 2e 59 1e 69 2e b2 f4 44 17 b3 f4 47 03 14 0f ed fb b1 16 00 fb b3 16 00 fb b5 16 7f 00 30 8e 12 6b 16 42 7c 1c c3 12 00 04 2a 2e 4d 00 00 06 19 04 f8 1a f5 4a f2 f4 08 79 2e 59 1e 69 2e f6 f4 00 5f f6 f4 00 53 f6 f4 16 df f4 4c f6 f4 42 4b f2 f4 fb 59 1e 01 bf 80 00 89 22 7d b6 07 c2 20 a9 0b f5 6b 00 04 2b 06 00 0c 9c bc 04 00 56 16 14 f5 19 07 94 04 54 90 34 42")

    def test_primary_weapon_installer_and_independent_trigger_producer(self):
        self.assert_source(0x07D145, "c2 20 a9 8b f4 99 2b 00 e2 20 20 f5 be b9 26 00 29 f7 99 26 00 28 7a 6b")
        self.assert_source(0x07DD0C, "22 1d d1 07 9c 59 1e")
        self.assert_source(0x0DC69D, "08 a9 01 8d 59 1e 28 60")
        self.assert_source(0x0DC679, "08 ad 59 1e d0 07 ad 36 19 89 40 f0 0b c2 20 a9 0c 00 99 16 6c 8d a5 1d e2 20 28 60")

    def test_primary_link_is_object_identity_used_by_pair_exclusion(self):
        self.assert_source(0x7F9F67, "20 d2 9f c2 20 ad b7 16 99 00 00 4c a9 ca")
        self.assert_source(0x7F444D, "98 d5 1c d0 03 82 e3 04 8a bb d5 1c d0 03 82 da 04 9b aa")


if __name__ == "__main__":
    unittest.main()
