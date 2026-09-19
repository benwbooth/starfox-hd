#!/usr/bin/env python3
"""Source-byte checks for scene proxies and ordered actor retirement."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class SceneProxyStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_pool_has_512_entries_with_distinct_head_and_free_list(self):
        self.assert_source(0x0DD5C1, "9C 85 12 A9 2E 34 8D 83 12 A0 00 02 AA 18 69 19 00 9D 00 00 88 D0 F5 9E 00 00")

    def test_capture_inserts_after_head_and_exhaustion_skips_capture(self):
        self.assert_source(0x7FAF03, "AE 85 12 9B AE 83 12 D0 04 BB 4C 9A AF")
        self.assert_source(0x7FAF27, "B9 00 00 9D 00 00 8A 99 00 00 98 9D 02 00")
        self.assert_source(0x7FAF9A, "E2 20 FA 4C E8 CA")

    def test_capture_snapshots_pose_and_uses_type_three_or_next_instruction(self):
        self.assert_source(0x7FAF40, "B5 0C 99 04 00 B5 0E 99 06 00 B5 10 99 08 00 B5 04 99 0D 00 8A 99 13 00 98 9D E6 1C")
        self.assert_source(0x7FAF5C, "5A A9 03 00 22 3B 23 7F C0 00 00 D0 06 C2 20 5C 7A AF 7F C2 20 B9 62 6A E2 20 C2 20 80 03 B5 2B 1A 7A 99 0F 00")
        self.assert_source(0x7FAF83, "A9 03 99 12 00 B5 12 99 0A 00 B5 14 99 0B 00 B5 16 99 0C 00")
        self.assert_source(0x7FAFCB, "BC E6 1C F0 0A C2 20 B5 2B 99 0F 00 4C E8 CA")

    def test_retirement_detaches_proxy_without_releasing_its_slot(self):
        self.assert_source(0x7F342F, "BC E6 1C F0 18 9E E6 1C 9E E7 1C B9 12 00 29 FE 09 10 99 12 00 A9 00 99 13 00 99 14 00 7A AB 6B")

    def test_immediate_release_clears_actor_handle_and_pushes_proxy_on_free_list(self):
        self.assert_source(0x7F33E0, "BC E6 1C F0 3D 9E E6 1C 9E E7 1C")
        self.assert_source(0x7F3415, "AD 83 12 9D 00 00 8E 83 12 E2 20 7A FA 7A AB 6B")

    def test_world_retirement_orders_scene_contacts_relationships_programs_then_pool(self):
        self.assert_source(0x7F336A, "22 25 34 7F 22 B2 33 7F 22 4F 34 7F C2 20 22 CB 34 7F B4 02")
        self.assert_source(0x7F34E1, "FA 22 A7 19 7F 6B")
        self.assert_source(0x7F19B3, "BC DC 1C F0 10 DA BE 61 6A 98 22 77 17 7F 9B D0 F5 FA 9E DC 1C 9E EC 1C 9E DE 1C 9E E0 1C")
        self.assert_source(0x7F339D, "AD AA 12 95 00 8E AA 12")


if __name__ == "__main__":
    unittest.main()
