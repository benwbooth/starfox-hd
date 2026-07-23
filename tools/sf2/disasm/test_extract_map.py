#!/usr/bin/env python3
"""ROM-backed regression tests for the clean-room SF2 map extractor."""

from __future__ import annotations

import unittest
from pathlib import Path

from extract_map import (
    DEFAULT_ROM,
    InlineBranchWordBits,
    InlineCall,
    InlineSelectGsuProgram,
    InlineSetPilotLinkedFlag,
    InlineWordBits,
    MapAddress,
    MapExtractor,
)


@unittest.skipUnless(Path(DEFAULT_ROM).is_file(), "retail SF2 ROM is not present")
class RetailMapExtractionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.extractor = MapExtractor(Path(DEFAULT_ROM).read_bytes())
        cls.result = cls.extractor.extract()

    def test_dispatch_table_and_spawn_handlers(self) -> None:
        table = self.extractor.handler_table()
        self.assertEqual(len(table), 83)
        self.assertEqual(table[0x78 // 2], 0x96E4)
        self.assertEqual(table[0x86 // 2], 0x9EC8)
        self.assertEqual(table[0x0A // 2], 0x9F57)

    def test_all_discovered_scripts_decode_without_guesses(self) -> None:
        self.assertEqual(len(self.result.roots), 25)
        self.assertEqual(len(self.result.commands), 4094)
        self.assertEqual(len(self.result.inline_exits), 262)
        self.assertEqual(len(self.result.phase_gates), 237)
        self.assertEqual(self.result.unresolved_inline_exits, [])
        self.assertEqual(self.result.invalid_opcodes, [])

    def test_reachable_spawn_roster_is_stable(self) -> None:
        self.assertEqual(len(self.result.spawns), 232)
        first = self.result.spawns[0]
        self.assertEqual(first.address, MapAddress(0x05, 0x0003))
        self.assertEqual((first.x, first.y, first.z), (400, -150, 0))
        self.assertEqual(first.shape, 0xBC9C)
        self.assertEqual((first.strategy_bank, first.strategy_addr), (0x06, 0x82F9))
        self.assertIn(
            (0x7F, 0x7E1E),
            {(spawn.strategy_bank, spawn.strategy_addr)
             for spawn in self.result.spawns},
        )

    def test_every_inline_routine_has_a_proven_typed_action(self) -> None:
        actions = list(self.result.inline_actions.values())
        self.assertEqual(len(actions), 262)
        self.assertEqual(sum(isinstance(a, InlineCall) for a in actions), 236)
        self.assertEqual(sum(isinstance(a, InlineWordBits) for a in actions), 7)
        self.assertEqual(sum(isinstance(a, InlineBranchWordBits) for a in actions), 4)
        self.assertEqual(sum(isinstance(a, InlineSetPilotLinkedFlag) for a in actions), 8)
        self.assertEqual(sum(isinstance(a, InlineSelectGsuProgram) for a in actions), 7)

    def test_live_oracle_phase_boundaries_are_recovered_exactly(self) -> None:
        gates = {
            (gate.hold, gate.parked, gate.continuation)
            for gate in self.result.phase_gates
        }
        self.assertIn(
            (
                MapAddress(0x05, 0x6052),
                MapAddress(0x05, 0x6055),
                MapAddress(0x05, 0x6059),
            ),
            gates,
        )
        self.assertIn(
            (
                MapAddress(0x05, 0x65B4),
                MapAddress(0x05, 0x65B7),
                MapAddress(0x05, 0x65BB),
            ),
            gates,
        )


if __name__ == "__main__":
    unittest.main()
