#!/usr/bin/env python3
"""Complete native lowering tests using source data and synthetic byte edits."""

from pathlib import Path
import unittest

from generate_native_paths import (
    DEFAULT_ROM, OUTPUT, PathAddress, PathExtractor, UnsupportedPath,
    generate, lower_graph,
)


class NativePathGenerationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def test_checked_in_catalog_is_exact_generated_output(self):
        self.assertEqual(OUTPUT.read_text(), generate(self.rom))

    def test_exhaust_graph_retains_all_five_statements_and_literal_operands(self):
        entry, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xF536), 0)
        self.assertEqual(entry, 0)
        self.assertEqual(len(statements), 5)
        self.assertIn("color: 0, size: 0", statements[0])
        self.assertIn("iterations: 3", statements[1])
        self.assertIn("amount: 1, period: 2", statements[2])
        self.assertIn("immediate: false", statements[3])
        self.assertEqual(statements[4], "Statement::Control(ControlCommand::End)")

    def test_lowering_reads_authored_parameters_instead_of_hardcoding_fixture(self):
        changed = bytearray(self.rom)
        changed[0x40000 + 0xF537] = 17
        changed[0x40000 + 0xF538] = 201
        _, statements = lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF536), 0)
        self.assertIn("color: 17, size: 201", statements[0])

    def test_unsupported_complete_root_is_rejected_not_partially_published(self):
        with self.assertRaisesRegex(UnsupportedPath, "unsupported"):
            lower_graph(PathExtractor(self.rom), PathAddress(0xF593), 1)


if __name__ == "__main__":
    unittest.main()
