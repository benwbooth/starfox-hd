#!/usr/bin/env python3
"""Literal action gate commands and spawn-group field aliases, source only."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM
from extract_path import PathExtractor
from path_semantics import PATH_SEMANTICS


class PathActionGateStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_complete_gate_handlers_and_table_entries(self):
        extractor = PathExtractor(self.rom)
        specs = {s.opcode: s for s in PATH_SEMANTICS}
        for opcode, address, source in [
            (0x13C, 0x7FBF05, "20bcc48d721d4cd3ca"),
            (0x13D, 0x7FBF35, "a9008d721d4ce8ca"),
            (0x13F, 0x7FBF0E, "ad721dd0034cf3ca4cbeca"),
            (0x140, 0x7FBF19, "20bcc4cd721df0034cffca4ca9ca"),
            (0x141, 0x7FBF27, "20bcc4cd721dd0034cffca4ca9ca"),
        ]:
            self.assert_source(address, source)
            self.assertEqual(extractor.handler_entry(opcode).handler_address, address)
            self.assertEqual(specs[opcode].handler_address, address)

    def test_gate_continuations_preserve_ifnot_wait_and_immediate_player_selection(self):
        self.assert_source(0x7FCAD3, "e220c220b52b18690200952be2204c757e")
        self.assert_source(0x7FCAE8, "e220c220f62be2204c757e")
        self.assert_source(0x7FCABE, "e220c220b52b18690300952be2204c757e")
        self.assert_source(0x7FCAA9, "e220c220b52b18690400952be2204c757e")
        self.assert_source(0x7FCAF3, "c2202020c7952be2204c757e")
        self.assert_source(0x7FCAFF, "c220204cc7952be2204c757e")

    def test_spawn_group_operand_aliases_the_actual_initializer_and_retirement_byte(self):
        self.assert_source(0x7FCB47, "08c2208eb11629ff00898000f0041869411c186db116a82860")
        # AF + 1C41 = 1CF0: the initialized/inherited group, not a new counter.
        self.assert_source(0x0DD8EC, "b400bdf01cc55fd00ea9009de61c9de71cb52509089525bbd0e6")
        self.assert_source(0x7F2A0C, "ad0e199df01cabdabb7a6b")
        for address in [0x4A482, 0x4A4EE, 0x4D2FB, 0x4D332, 0x4D36F, 0x4D399, 0x4D3B8]:
            self.assertEqual(self.rom[address:address + 3], bytes.fromhex("0bffaf"))


if __name__ == "__main__":
    unittest.main()
