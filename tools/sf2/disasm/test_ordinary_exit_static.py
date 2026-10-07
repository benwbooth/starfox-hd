#!/usr/bin/env python3
"""Source-bound ordinary exit craft, view placement and handoff publications."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class OrdinaryExitStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_craft_speed_precedes_direction_read_and_only_table_selector_is_masked(self):
        self.source(0x07F808, '5a9ba9329518ad8f1d952ec220290700aae220bfb6fd0799d51cbfbefd0799e21cbfc6fd0799d71cbf9efd07991200bfa6fd07991400bfaefd07991600bfcefd07990a00c2208a0aaabf6efd0799cf1c18790c00990c00bf7efd0799d11c18790e00990e00bf8efd0799d31c18791000991000e220bb7a6b')

    def test_last_spawn_view_gets_two_distinct_heading_rotated_offsets_in_order(self):
        self.source(0x07F3E1, 'ac71d7a900850285048980f006a9ff850580026405a90a8508850a8980f006a9ff850b8002640ba950859785e48980f006a9ff85e5800264e5b51422a9387fc220a5040a0a0a0a0a0a8504a50a0a0a0a0a0a0a850aa5e40a0a0a0a0a0a85e4a504187d0c00990c00a5e4187d1000991000a50a187d0e00990e00e220daad8f1dc220290700aabff9f407e220fa8597ada91b991400a900991200991600a900850285048980f006a9ff850580026405a9008508850a8980f006a9ff850b8002640ba597859785e48980f006a9ff85e5800264e5b9140022a9387fc220a5040a0a0a0a0a0a8504a50a0a0a0a0a0a0a850aa5e40a0a0a0a0a0a85e4a50418790c00990c00a5e418791000991000a50a18790e00990e00e2206b')
        self.source(0x07F4F9, 'f1f6000a140a00f6')

    def test_fixed_view_chases_each_position_axis_once_without_writing_angles(self):
        self.source(0x7FC028, 'c2202020c7a8e220c220b90c00853ab50c22a3257f990c00e220c220b90e00853ab50e22a3257f990e00e220c220b91000853ab51022a3257f991000e2204cbeca')

    def test_layout_request_is_consumed_by_player_service_not_the_path_command(self):
        self.source(0x06A0F9, 'ad741d8901f00829fe8d741deea51b')
        for offset, body in [
            (0xE845, '89ad741d09018d741dc220a954e86b'),
            (0xCFF8, '89ad741d09048d741dc220a907d06b'),
            (0xD098, '89ad741d09088d741dc220a9a7d06b'),
        ]:
            data = bytes.fromhex(body)
            self.assertEqual(self.rom[0x40000 + offset:0x40000 + offset + len(data)], data)


if __name__ == '__main__':
    unittest.main()
