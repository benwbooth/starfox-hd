#!/usr/bin/env python3
"""ROM-backed regression tests for the clean-room SF2 path extractor."""

from __future__ import annotations

import unittest
from pathlib import Path

from extract_map import DEFAULT_ROM
from extract_path import FlowEffect, PathAddress, PathExtractor
from path_semantics import PATH_SEMANTICS


@unittest.skipUnless(Path(DEFAULT_ROM).is_file(), "retail SF2 ROM is not present")
class RetailPathExtractionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.extractor = PathExtractor(Path(DEFAULT_ROM).read_bytes())
        cls.result = cls.extractor.extract()

    def test_reset_copy_translation_and_rts_dispatch_entries(self) -> None:
        self.assertEqual(self.extractor.runtime_file(0x7F7E00), 0x050000)
        self.assertEqual(self.extractor.runtime_file(0x7FCBFF), 0x054DFF)
        self.assertEqual(self.extractor.source_cpu_to_runtime(0x0A8000), 0x7F7E00)
        self.assertEqual(self.extractor.source_cpu_to_runtime(0x0ACDFF), 0x7FCBFF)

        primary = self.extractor.handler_entry(0x001)
        self.assertEqual(primary.table_address, 0x7F7EEC)
        self.assertEqual(primary.stored_address, 0x84E8)
        self.assertEqual(primary.handler_address, 0x7F84E9)
        self.assertEqual(primary.bank, 0x7F)

        extended = self.extractor.handler_entry(0x100)
        self.assertEqual(extended.table_address, 0x7F82E8)
        self.assertEqual(extended.stored_address, 0x860C)
        self.assertEqual(extended.handler_address, 0x7F860D)

    def test_high_extended_dispatch_slot_alias_is_not_reachable_bytecode(self) -> None:
        # Logical $180 computes a slot at $84E8.  That address is already in
        # handler code, so only its low word is meaningful to PHA/RTS.
        aliased = self.extractor.handler_entry(0x180)
        self.assertEqual(aliased.table_address, 0x7F84E8)
        self.assertEqual(aliased.stored_address, 0xB5CA)
        self.assertEqual(aliased.handler_address, 0x7FB5CB)
        self.assertNotEqual(aliased.bank, 0x7F)

        self.assertNotIn(
            0x180,
            {command.opcode for command in self.result.commands},
        )

    def test_reachable_path_graph_is_stable_and_fully_resolved(self) -> None:
        self.assertEqual(len(self.result.roots), 106)
        self.assertEqual(
            [root.offset for root in self.result.roots],
            [
                0x00BC, 0x04B5, 0x04FF, 0x0502, 0x0591, 0x0691, 0x0AE7,
                0x0AEA, 0x0F7E, 0x1118, 0x111E, 0x1124, 0x1369, 0x1AFF,
                0x2102, 0x22AA, 0x23D4, 0x2651, 0x295E, 0x2961, 0x2BE9,
                0x2F11, 0x2F1A, 0x3114, 0x31B8, 0x32EF, 0x348B, 0x360A,
                0x37AE, 0x37B6, 0x3FC1, 0x419B, 0x4839, 0x4A95, 0x4C68,
                0x4C6A, 0x4D7E, 0x4E26, 0x4F06, 0x4F72, 0x545F, 0x546C,
                0x548E, 0x5742, 0x58B9, 0x5B96, 0x5E1D, 0x6003, 0x60F1,
                0x6230, 0x6550, 0x66EA, 0x6A15, 0x6C01, 0x6F65, 0x6F75,
                0x720D, 0x72BB, 0x7442, 0x787D, 0x7BA0, 0x7BC5, 0x7F24,
                0x7F27, 0x8D82, 0x9492, 0xA2E6, 0xA552, 0xAA8A, 0xB050,
                0xB05E, 0xB8C5, 0xBE65, 0xCF18, 0xD1CB, 0xD207, 0xD27B,
                0xDC42, 0xE973, 0xEB1A, 0xEC98, 0xECF7, 0xEE10, 0xEE2D,
                0xEE3B, 0xEE4C, 0xEEED, 0xEF2D, 0xF029, 0xF04F, 0xF084,
                0xF136, 0xF294, 0xF2B9, 0xF32C, 0xF36F, 0xF38A, 0xF3AD,
                0xF48B, 0xF521, 0xF536, 0xF540, 0xF561, 0xF582, 0xF593,
                0xF5B4,
            ],
        )
        self.assertEqual(len(self.result.commands), 14220)
        self.assertEqual(len(self.result.handlers), 279)
        self.assertEqual(self.result.invalid_opcodes, [])
        self.assertEqual(self.result.unresolved_handlers, [])

    def test_wait_and_direct_pointer_handlers_have_proven_flow(self) -> None:
        wait = self.result.handlers[0x003]
        self.assertEqual(
            wait.effects,
            (
                FlowEffect("advance", 2, resets_counter=True),
                FlowEffect("hold", yields=True),
            ),
        )
        self.assertEqual(
            self.result.handlers[0x016].effects,
            (FlowEffect("jump", 1, yields=True),),
        )
        self.assertEqual(
            self.result.handlers[0x078].effects,
            (FlowEffect("advance", 1, yields=True),),
        )

    def test_internal_calls_and_indexed_state_dispatch_are_followed(self) -> None:
        # $7F/$80 call $9FBF, which leaves A 16-bit.  Decoding the caller with
        # its entry width would see a false BRK; interprocedural flow proves
        # both are ordinary three-byte records.
        self.assertEqual(
            self.result.handlers[0x07F].effects,
            (FlowEffect("advance", 3),),
        )
        self.assertEqual(
            self.result.handlers[0x080].effects,
            (FlowEffect("advance", 3),),
        )

        stateful = self.result.handlers[0x042]
        self.assertEqual(stateful.effects, (FlowEffect("return"),))
        self.assertEqual(stateful.unresolved_targets, ())

    def test_gosub_and_return_are_interprocedural_bytecode_edges(self) -> None:
        gosub = self.extractor.decode_command(PathAddress(0x7450))
        self.assertEqual(gosub.raw_hex, "415874")
        self.assertEqual(
            gosub.successors,
            (PathAddress(0x7453), PathAddress(0x7458)),
        )
        self.assertEqual(gosub.effects, (FlowEffect("call", 1),))

        returned = self.extractor.decode_command(PathAddress(0x7679))
        self.assertEqual(returned.raw_hex, "42")
        self.assertEqual(returned.successors, ())
        self.assertEqual(returned.effects, (FlowEffect("return"),))

        # The shared child-spawn handler branches on the saved logical opcode.
        # Its two record formats must not be unioned: doing so interprets spawn
        # operands as bytecode and invents overlapping command starts.
        spawn_alias = self.extractor.decode_command(PathAddress(0x42B8))
        self.assertEqual(spawn_alias.opcode, 0x033)
        self.assertEqual(len(bytes.fromhex(spawn_alias.raw_hex)), 17)
        self.assertEqual(spawn_alias.successors, (PathAddress(0x42C9),))
        spawn = self.extractor.decode_command(PathAddress(0x421F))
        self.assertEqual(spawn.opcode, 0x0F5)
        self.assertEqual(len(bytes.fromhex(spawn.raw_hex)), 14)
        self.assertEqual(spawn.successors, (PathAddress(0x422D),))

        # Spawned objects start independent path VMs.  Their literal targets
        # must be included in the reachable catalog even though they are not
        # same-object control-flow successors.
        starts = {command.address.offset for command in self.result.commands}
        for command in self.result.commands:
            if command.opcode in (0x033, 0x05D, 0x0F5):
                raw = bytes.fromhex(command.raw_hex)
                operand = command.prefix_size + 3
                target = int.from_bytes(raw[operand:operand + 2], "little")
                if target:
                    self.assertIn(target, starts)

        overlaps = {}
        for command in self.result.commands:
            start = command.address.offset
            nested = tuple(sorted(
                candidate
                for candidate in starts
                if start < candidate < start + len(bytes.fromhex(command.raw_hex))
            ))
            if nested:
                overlaps[start] = nested

        # The expanded graph contains deliberate multi-entry bytecode.  These
        # are exact independently reachable roots/branches (including the
        # signature-gated inline dispatch), not width-decoder accidents.
        self.assertEqual(
            overlaps,
            {
                # The late inline dispatch enters `$040B` inside the indexed
                # movement record at `$0407`; its resulting GOTO spans the
                # independent `$040E` return reached by the movement path.
                0x0407: (0x040B, 0x040C),
                0x040C: (0x040E,),
                0x07FD: (0x07FE, 0x07FF),
                0x4209: (0x420A,),
                0x532F: (0x5330,),
                0x619C: (0x61A1,),
                0x7A2D: (0x7A2E,),
                0x7A2E: (0x7A2F,),
                0x7A2F: (0x7A31,),
                0x7A31: (0x7A32, 0x7A34),
                0x7A34: (0x7A36,),
                0x96A0: (0x96A1,),
                0xA266: (0xA26B, 0xA26C, 0xA26D, 0xA272),
                0xA272: (0xA274, 0xA275),
            },
        )

    def test_long_relative_branches_recover_both_bytecode_successors(self) -> None:
        # These comparison handlers use BRL for their taken branch.  Omitting
        # BRL from the CPU branch set used to walk the handler fallthrough and
        # infer a false record width.
        for address, opcode, successors in [
            (0x0144, 0x0EE, (0x0149, 0x014C)),
            (0x01A1, 0x0EF, (0x01A6, 0x01BC)),
        ]:
            command = self.extractor.decode_command(PathAddress(address))
            self.assertEqual(command.opcode, opcode)
            self.assertEqual(
                tuple(item.offset for item in command.successors),
                successors,
            )

    def test_inline_65816_blocks_rejoin_the_path_graph(self) -> None:
        direct = self.extractor.decode_command(PathAddress(0x8D54))
        self.assertEqual(direct.effects, (FlowEffect("inline"),))
        self.assertEqual(direct.successors, (PathAddress(0x8D61),))

        table = self.extractor.decode_command(PathAddress(0xAB2A))
        self.assertEqual(
            tuple(item.offset for item in table.successors),
            (0x8D81, 0xA8C6, 0xA8CA, 0xA8D5, 0xA8DC,
             0xA915, 0xA95C, 0xA964, 0xAA3A),
        )
        self.assertIn(0xB116, {command.address.offset for command in self.result.commands})
        self.assertIn(0xB129, {command.address.offset for command in self.result.commands})

    def test_trigger_builders_recover_full_records_and_scheduled_edges(self) -> None:
        cases = [
            (0x0594, "f80c07", (0x0597, 0x070C)),
            (0x06AF, "fd5d00", (0x06B2, 0x070C)),
            (0x3FEB, "4a728703", (0x3FEF, 0x8772)),
            (0x0502, "00651f051178", (0x0508, 0x051F)),
        ]
        for address, raw, successors in cases:
            command = self.extractor.decode_command(PathAddress(address))
            self.assertEqual(command.raw_hex, raw)
            self.assertEqual(
                tuple(item.offset for item in command.successors),
                successors,
            )

        # FORCE_TRIGGER_PATH starts its literal target immediately while the
        # caller continues.  Both edges are part of the retail graph; omitting
        # the scheduled edge used to hide Meteor's controller sequence.
        forced = self.extractor.decode_command(PathAddress(0x54F2))
        self.assertEqual(forced.opcode, 0x04C)
        self.assertEqual(forced.raw_hex, "4cf654")
        self.assertEqual(
            tuple(item.offset for item in forced.successors),
            (0x54F5, 0x54F6),
        )

        starts = {command.address.offset for command in self.result.commands}
        for command in self.result.commands:
            if command.opcode == 0x04C:
                raw = bytes.fromhex(command.raw_hex)
                operand = command.prefix_size + 1
                target = int.from_bytes(raw[operand:operand + 2], "little")
                self.assertIn(target, starts)

    def test_handler_disassembly_preserves_width_resolved_cfg(self) -> None:
        instructions = self.extractor.handler_instructions(0x02A)
        states = {(instruction.cpu, instruction.m, instruction.x) for instruction in instructions}
        self.assertTrue(states)
        self.assertEqual(len(states), len(instructions))
        self.assertIn((0x0A9140, 1, 0), states)
        self.assertTrue(any(instruction.m == 0 for instruction in instructions))

    def test_reviewed_semantics_are_unique_and_pinned_to_retail_handlers(self) -> None:
        self.assertEqual(len(PATH_SEMANTICS), 279)
        self.assertEqual(len({spec.opcode for spec in PATH_SEMANTICS}), 279)
        self.assertEqual(len({spec.rust_name for spec in PATH_SEMANTICS}), 279)
        for spec in PATH_SEMANTICS:
            self.assertIn(spec.opcode, self.result.handlers)
            self.assertEqual(
                self.result.handlers[spec.opcode].handler_address,
                spec.handler_address,
                spec.rust_name,
            )

        # The allow-list now covers every reachable retail handler.  This is
        # exact equality, not a lower bound that could hide a new opcode.
        named = {spec.opcode for spec in PATH_SEMANTICS}
        self.assertEqual(named, set(self.result.handlers))


if __name__ == "__main__":
    unittest.main()
