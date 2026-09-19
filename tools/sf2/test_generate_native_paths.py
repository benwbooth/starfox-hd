#!/usr/bin/env python3
"""Complete native lowering tests using source data and synthetic byte edits."""

from pathlib import Path
import unittest
import re

from generate_native_paths import (
    DEFAULT_ROM, OUTPUT, PathAddress, PathExtractor, UnsupportedPath,
    byte_field, generate, lower_graph, word_field,
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

    def test_color_cycle_effect_preserves_initial_wait_and_seven_count_loop(self):
        entry, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xF593), 1)
        self.assertEqual(entry, 0)
        self.assertEqual(len(statements), 8)
        self.assertIn("DisableCollision", statements[0])
        self.assertIn("color: 0, size: 10", statements[1])
        self.assertIn("Initialize", statements[2])
        self.assertIn("value: 0", statements[2])
        self.assertIn("WaitOne", statements[3])
        self.assertIn("iterations: 7", statements[4])
        self.assertIn("amount: 1, period: 8", statements[5])
        self.assertIn("immediate: false", statements[6])
        self.assertEqual(statements[7], "Statement::Control(ControlCommand::End)")

    def test_unsupported_complete_root_is_rejected_not_partially_published(self):
        with self.assertRaisesRegex(UnsupportedPath, "unsupported SpawnChild"):
            lower_graph(PathExtractor(self.rom), PathAddress(0xF561), 2)

    def lower_record(self, record):
        changed = bytearray(self.rom)
        program = bytes.fromhex(record) + bytes([0x0F])
        changed[0x4F536:0x4F536 + len(program)] = program
        return lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF536), 0)[1]

    def test_literal_sets_and_adds_have_distinct_source_operand_orders(self):
        for record, fragment in (
            ("0b 05 a1", "ByteOperation::Assign(ByteOperand::Literal(5))"),
            ("0c 12 ab a1", "WordOperation::Assign(WordOperand::Literal(43794))"),
            ("07 a1 05", "ByteOperation::Add(ByteOperand::Literal(5))"),
            ("08 a1 12 ab", "WordOperation::Add(WordOperand::Literal(43794))"),
            ("6b a1", "ByteOperation::Assign(ByteOperand::Literal(0))"),
            ("6c a1", "WordOperation::Assign(WordOperand::Literal(0))"),
        ):
            with self.subTest(record=record):
                statements = self.lower_record(record)
                self.assertEqual(len(statements), 2)
                self.assertIn("WordField::MotionPhase", statements[0])
                self.assertIn(fragment, statements[0])

    def test_unary_arithmetic_width_is_preserved(self):
        for opcode, width, operation in (
            (0x6D, "Byte", "Increment"), (0x6E, "Word", "Increment"),
            (0x6F, "Byte", "Decrement"), (0x70, "Word", "Decrement"),
            (0x56, "Byte", "Negate"), (0x57, "Word", "Negate"),
        ):
            with self.subTest(opcode=opcode):
                statement = self.lower_record(f"{opcode:02x} a1")[0]
                self.assertIn(f"Mutation::{width}", statement)
                self.assertIn(f"{width}Operation::{operation}", statement)

    def test_variable_byte_loop_is_unsigned_and_word_loop_uses_full_word(self):
        self.assertIn("WordOperand::UnsignedByte(ByteOperand::Actor(ByteField::TargetSpeed))", self.lower_record("62 0a")[0])
        self.assertIn("WordOperand::Actor(WordField::MotionPhase)", self.lower_record("63 a1")[0])

    def test_zero_conditions_use_real_target_order_and_noninverting_predicates(self):
        for opcode, condition, width in (
            (0x67, "ZeroByte", "Byte"), (0x68, "ZeroWord", "Word"),
            (0x69, "NonzeroByte", "Byte"), (0x6A, "NonzeroWord", "Word"),
        ):
            # Branch back to the statement; fallthrough is the appended END.
            statement = self.lower_record(f"{opcode:02x} a1 36 f5")[0]
            self.assertIn(f"ActorCondition::{condition}({width}Operand::Actor(", statement)
            self.assertIn("taken: cursor(0, 0), next: cursor(0, 1)", statement)

    def test_wait_reads_literal_or_live_byte_and_has_only_one_continuation(self):
        self.assertIn("Statement::Wait { duration: ByteOperand::Literal(129)", self.lower_record("03 81")[0])
        self.assertIn("Statement::Wait { duration: ByteOperand::Actor(ByteField::TargetSpeed)", self.lower_record("00 28 0a")[0])

    def test_particle_fields_are_named_and_unknown_encodings_are_rejected(self):
        self.assertEqual(byte_field(0x99), "ByteField::TextureScrollX")
        self.assertEqual(word_field(0x34), "WordField::Velocity(Axis::Y)")
        self.assertIn("WordField::MotionPhase", byte_field(0xA1))
        self.assertIn("BytePart::Low", byte_field(0xA1))
        self.assertIn("BytePart::High", byte_field(0xA2))
        self.assertEqual(word_field(0x8E), "WordField::RelativePosition(Axis::X)")
        self.assertIn("RelativePosition(Axis::Z)", byte_field(0x93))
        with self.assertRaises(UnsupportedPath):
            word_field(0x8D)
        for field in (word_field, byte_field):
            with self.assertRaises(UnsupportedPath):
                field(0x80)

    def test_particle_branch_uses_literal_target_not_sorted_successor_order(self):
        entry, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xF294), 2)
        self.assertEqual(entry, 0)
        self.assertEqual(len(statements), 12)
        self.assertIn("taken: cursor(2, 11), next: cursor(2, 9)", statements[8])
        self.assertIn("MotionCommand::AccelerateTo { target: 0, amount: 5 }", statements[5])
        self.assertIn("WordField::Velocity(Axis::Y)", statements[9])

    def test_subroutine_before_root_in_layout_has_real_return_and_continuation(self):
        entry, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xF521), 3)
        self.assertEqual(entry, 4)  # the shared subroutine is earlier in source
        self.assertEqual(len(statements), 12)
        self.assertIn("RelativePosition(Axis::X)", statements[0])
        self.assertEqual(statements[3], "Statement::Control(ControlCommand::Return)")
        self.assertIn("target: cursor(3, 0), next: cursor(3, 6)", statements[5])
        self.assertIn("BranchCommand::InvertNext", statements[8])
        self.assertIn("ControlCommand::Goto { target: cursor(3, 6)", statements[11])

    def test_shared_source_statements_have_one_identity_across_entry_aliases(self):
        source = generate(self.rom, (("FIRST", PathAddress(0xF521)), ("SECOND", PathAddress(0xF521))))
        first = re.search(r"pub const FIRST: PathCursor = (.*);", source)[1]
        second = re.search(r"pub const SECOND: PathCursor = (.*);", source)[1]
        self.assertEqual(first, second)
        self.assertIn("LOWERED_COMMAND_COUNT: usize = 12", source)
        self.assertEqual(source.count("ControlCommand::Return"), 1)

    def test_auxiliary_sprite_preserves_wrapped_size_add_and_dynamic_loop_edges(self):
        entry, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xF36F), 4)
        self.assertEqual(entry, 0)
        self.assertEqual(len(statements), 11)
        self.assertIn("size: 255", statements[0])
        self.assertIn("ByteField::TextureScrollX", statements[4])
        self.assertIn("ByteOperation::Add(ByteOperand::Literal(2))", statements[4])
        self.assertEqual(statements[8], "Statement::SelectedAuxiliaryBranch { taken: cursor(4, 10), next: cursor(4, 9) }")


if __name__ == "__main__":
    unittest.main()
