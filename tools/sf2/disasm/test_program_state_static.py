#!/usr/bin/env python3
"""Assembly checks for actor-owned path stack and loop operations."""

from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class ProgramStateStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_stack_initial_cost_and_each_eighth_entry_resize(self):
        self.assert_source(0x7F1A1D, "A8 D0 13 A9 21 00 22 4E 19 7F A8 E2 20 A9 01 99 61 6A C2 20 80 27")
        self.assert_source(0x7F1A33, "B9 61 6A 29 FF 00 1A E2 20 99 61 6A 89 07 C2 20 D0 15 18 69 08 00 0A 0A 1A 22 00 1B 7F")

    def test_entry_has_four_units_and_count_is_a_byte(self):
        self.assert_source(0x7F1A5B, "B9 61 6A 29 FF 00 0A 0A 8C 61 B2 38 6D 61 B2 A8 AD 69 B2 99 5D 6A AD 6B B2 99 5F 6A")

    def test_next_decrements_before_repeat_and_loads_prior_entry(self):
        self.assert_source(0x7F96C4, "B9 61 6A 3A F0 11 99 61 6A 88 88 88 88 B9 61 6A 95 2B E2 20 4C DE 9D")
        self.assert_source(0x7F971E, "B9 61 6A 3A F0 B7 99 61 6A 88 88 88 88 B9 61 6A 95 2B E2 20 4C 53 7E")

    def test_loop_completion_pops_twice_without_free(self):
        self.assert_source(0x7F96DF, "BD DE 1C 22 C1 1A 7F 9D DE 1C AD 69 B2 8D B1 16 E2 20 C2 20 BD DE 1C 22 C1 1A 7F 9D DE 1C AD 69 B2 8D B1 16 E2 20 4C E8 CA")

    def test_call_and_normal_return_use_the_same_owned_stack(self):
        self.assert_source(0x7F956F, "B5 2B 8D 69 B2 BD DE 1C 22 0F 1A 7F 9D DE 1C")
        self.assert_source(0x7F9592, "B5 2B 8D 69 B2 BD DE 1C 22 0F 1A 7F 9D DE 1C")
        self.assert_source(0x7F95B4, "C2 20 BD DE 1C 22 C1 1A 7F 9D DE 1C AD 69 B2 95 2B E2 20 4C BE CA")

    def test_word_count_handler_pushes_continuation_then_full_variable_word(self):
        from path_semantics import PATH_SEMANTIC_BY_OPCODE
        self.assertEqual(PATH_SEMANTIC_BY_OPCODE[0x063].rust_name, "DoVariableWord")
        self.assert_source(0x7F9607, "C2 20 B5 2B 1A 1A E2 20 C2 20 8D 69 B2 BD DE 1C 22 0F 1A 7F 9D DE 1C E2 20 20 BC C4 20 47 CB C2 20 B9 00 00 E2 20 C2 20 8D 69 B2 BD DE 1C 22 0F 1A 7F 9D DE 1C E2 20 4C D3 CA")


if __name__ == "__main__":
    unittest.main()
