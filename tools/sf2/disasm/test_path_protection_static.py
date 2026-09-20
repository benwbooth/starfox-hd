#!/usr/bin/env python3
"""Linked protection effect: source graph, inline branches and exact gates."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathProtectionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_complete_protection_path_and_inline_branches(self):
        self.assert_source(0x09F2B9, "5c 8d ed f9 0b 01 a1 6b a2 79 99 df 1d 69 99 d3 f2 00 04 14 61 09 1c 01 0a 44 f8 e4 f2 fb df 1d 00 1b 09 69 a1 e1 f2 0f 16 d6 f2")
        self.assert_source(0x09F2E4, "89 22 8d f5 07 bd e2 1c 89 fe f0 07 c2 20 a9 f6 f2 6b 42 c2 20 a9 fd f2 6b 90 55 fb 07 a2 8a 6d a2 42")
        self.assert_source(0x07FB55, "88 87 86 85 86 87 86 85 84 83 82 81 82 83 82 81")

    def test_complete_effect_refresh_keeps_linked_owner_and_inverted_contact_gate(self):
        self.assert_source(0x07F58D, "bd d5 1c 18 69 08 9d d5 1c bd d7 1c 18 69 06 9d d7 1c ad e2 1d c9 09 f0 09 ad 4d 1b 29 07 c9 00 d0 05 ad 72 1d d0 32 b4 06 da b6 2b 9b fa ad 0d 1e 29 01 d0 05 ad f4 d7 d0 0c b9 02 6c 89 fe f0 05 a9 01 99 02 6c b9 02 6c 29 1f 9d e2 1c d0 08 b9 02 6c 29 df 99 02 6c 6b a9 00 9d e2 1c 6b")

    def test_other_linked_effect_producers_publish_shared_activity(self):
        self.assert_source(0x06F903, "c2 20 a5 04 99 06 00 e2 20 a9 01 8d df 1d")
        self.assert_source(0x06F99A, "c2 20 a9 ba f9 99 19 00 e2 20 a9 06 99 1b 00 a9 01 8d df 1d")

    def test_countdown_subsection_and_live_protection_effect_installation(self):
        self.assert_source(0x069F0D, "b9 02 6c 89 1f f0 0d a5 c4 29 07 d0 07 b9 02 6c 3a 99 02 6c")
        self.assert_source(0x07CD7F, "b4 2b b9 02 6c 89 1f f0 39 a9 12 22 7b 2a 7f")
        self.assert_source(0x07CDB0, "20 f5 be c2 20 a9 b9 f2 99 2b 00")


if __name__ == "__main__":
    unittest.main()
