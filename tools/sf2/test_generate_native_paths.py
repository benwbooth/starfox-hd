#!/usr/bin/env python3
"""Complete native lowering tests using source data and synthetic byte edits."""

from pathlib import Path
import unittest
import re
from dataclasses import replace

from generate_native_paths import (
    DEFAULT_ROM, OUTPUT, PathAddress, PathExtractor, UnsupportedPath,
    byte_field, child_spawn_parameters, child_spawn_shape, generate, graph, lower_graph, word_field,
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
        # An unsupported independently spawned child rejects its parent too.
        changed = bytearray(self.rom)
        changed[0x4F582:0x4F585] = bytes.fromhex("00 06 12")
        with self.assertRaisesRegex(UnsupportedPath, "unsupported QueueSelectedMarkerClass1"):
            lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF561), 2)

    def test_spawned_sound_graph_is_complete_and_shared_with_standalone_entry(self):
        _, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xF561), 2)
        self.assertEqual(len(statements), 23)
        self.assertEqual(sum("Statement::Sound" in item for item in statements), 1)
        self.assertTrue(any("AuthoredCue::new(18, 0, PlayerTarget::Primary)" in item for item in statements))

    def test_sound_literals_decode_routing_offline_and_advance_past_escape(self):
        for record, cue in [
            ("00 04 ff", "255, 0, PlayerTarget::Primary"),
            ("00 0a 12 00", "18, 0, PlayerTarget::Primary"),
            ("00 0a 00 7f", "0, 127, PlayerTarget::Primary"),
            ("00 0a 81 80", "129, 0, PlayerTarget::Secondary"),
            ("00 0a ff ff", "255, 127, PlayerTarget::Secondary"),
        ]:
            statements = self.lower_record(record)
            self.assertIn(f"AuthoredCue::new({cue})", statements[0])
            self.assertIn("next: cursor(0, 1)", statements[0])
            self.assertEqual(statements[1], "Statement::Control(ControlCommand::End)")

    def test_callback_graph_has_semantic_action_and_deferred_redirection(self):
        _, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xF32C), 0)
        self.assertEqual(len(statements), 18)
        self.assertIn("Trigger { path: cursor(0, 12), kind: TriggerKind::Always, timer: 0 }", statements[0])
        self.assertIn("next: cursor(0, 1)", statements[0])
        self.assertEqual(statements[1], "Statement::RunWhenPaused { enabled: true, next: cursor(0, 2) }")
        self.assertEqual(statements[12], "Statement::LatchPrimaryViewFilter { next: cursor(0, 13) }")
        self.assertIn("ForceAfterCallbacks { target: cursor(0, 17), next: cursor(0, 16) }", statements[15])
        generated = generate(self.rom, (("CALLBACK", PathAddress(0xF32C)),))
        self.assertIn("use super::path_triggers::{Trigger, TriggerKind};", generated)
        self.assertNotIn("0xF348", generated)
        self.assertNotIn("Inline65816", generated)

    def test_unported_or_changed_inline_actions_are_rejected(self):
        with self.assertRaisesRegex(UnsupportedPath, "unported inline action"):
            lower_graph(PathExtractor(self.rom), PathAddress(0xF078), 0)
        changed = bytearray(self.rom)
        changed[0x4F350] = 0x40  # change the primary flag mask inside the action
        with self.assertRaisesRegex(ValueError, "inline signature mismatch"):
            lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF32C), 0)

    def test_spawn_lowering_uses_semantic_shape_and_child_cursor_with_separate_continuation(self):
        for record, rotation in (
            ("f5 98 bd 00 f6 81 fe 00 80 ff 7f ff ff ff", (0, 0, 0)),
            ("33 98 bd 00 f6 80 81 ff 81 fe 00 80 ff 7f ff ff ff", (128, 129, 255)),
        ):
            changed = bytearray(self.rom)
            program = bytes.fromhex(record) + b"\x0f"
            changed[0x4F536:0x4F536 + len(program)] = program
            changed[0x4F600] = 0x0F
            entry, statements = lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF536), 0)
            self.assertEqual(entry, 0)
            self.assertEqual(len(statements), 3)
            self.assertIn("kind: ObjectKind::Effect", statements[0])
            self.assertIn("ShapeId::from_catalog_index(9)", statements[0])
            self.assertIn("path: Some(cursor(0, 2))", statements[0])
            self.assertIn("next: cursor(0, 1)", statements[0])
            self.assertIn("x: -32768, y: 32767, z: -1", statements[0])
            for field, value in zip(("pitch", "yaw", "roll"), rotation):
                self.assertIn(f"{field}: Angle::from_units({value})", statements[0])
            self.assertIn("hit_points: 129, attack_power: 254, number: 255", statements[0])
            generated = generate(bytes(changed), (("SPAWNER", PathAddress(0xF536)),))
            self.assertIn("use super::path_spawn::ChildSpawn;", generated)
            self.assertIn("LOWERED_COMMAND_COUNT: usize = 3", generated)
            self.assertNotIn("0xBD98", generated)

    def test_spawn_null_path_is_absent_and_unknown_shapes_or_native_kinds_are_rejected(self):
        self.assertIn("path: None", self.lower_record("f5 98 bd 00 00 01 01 00 00 00 00 00 00 00")[0])
        for shape in [0, 0xBD99, 0xFBB8, 0xFFFF]:
            with self.assertRaisesRegex(UnsupportedPath, "not a catalog header"):
                child_spawn_shape(shape)
        with self.assertRaisesRegex(UnsupportedPath, "unreviewed native child kind"):
            child_spawn_shape(0xBC9C)

    def test_compact_child_parameters_use_literal_offsets_and_zero_rotation(self):
        extractor = PathExtractor(self.rom)
        command = extractor.decode_command(PathAddress(0xF56A))
        spawn = child_spawn_parameters(command)
        self.assertEqual(spawn.shape, 0xBD98)
        self.assertEqual(spawn.path, PathAddress(0xF57F))
        self.assertEqual(spawn.rotation, (0, 0, 0))
        self.assertEqual((spawn.hit_points, spawn.attack_power, spawn.number), (1, 1, 1))
        changed = replace(command, raw_hex="f5 34 12 78 56 81 fe 00 80 ff 7f ff ff ff")
        spawn = child_spawn_parameters(changed)
        self.assertEqual((spawn.shape, spawn.path.offset), (0x1234, 0x5678))
        self.assertEqual((spawn.hit_points, spawn.attack_power, spawn.number), (129, 254, 255))
        self.assertEqual(spawn.position, (-32768, 32767, -1))

    def test_extended_child_parameters_have_rotation_before_health_and_position(self):
        extractor = PathExtractor(self.rom)
        command = extractor.decode_command(PathAddress(0x2677))
        self.assertEqual(command.opcode, 0x33)
        spawn = child_spawn_parameters(command)
        self.assertEqual(spawn.rotation, (0, 0, 64))
        self.assertEqual((spawn.hit_points, spawn.attack_power), (100, 4))
        self.assertEqual(spawn.position, (960, 0, 0))
        changed = replace(command, raw_hex="33 34 12 78 56 80 81 ff fe fd 00 80 ff 7f ff ff 00")
        spawn = child_spawn_parameters(changed)
        self.assertEqual(spawn.rotation, (128, 129, 255))
        self.assertEqual((spawn.hit_points, spawn.attack_power, spawn.number), (254, 253, 0))
        self.assertEqual(spawn.position, (-32768, 32767, -1))

    def test_spawn_dependencies_are_closed_without_becoming_parent_control_edges(self):
        extractor = PathExtractor(self.rom)
        command = extractor.decode_command(PathAddress(0xF56A))
        self.assertEqual(command.successors, (PathAddress(0xF578),))
        found = {command.address: command for command in graph(extractor, PathAddress(0xF561))}
        self.assertIn(PathAddress(0xF57F), found)  # independently spawned child
        self.assertIn(PathAddress(0xE927), found)  # child's shared subroutine
        self.assertIn(PathAddress(0xF592), found)  # child's final END
        self.assertEqual(len(found), 23)

    def test_spawn_graph_deduplicates_recursive_children_and_skips_null_paths(self):
        for opcode, record in (
            (0xF5, "f5 34 12 36 f5 01 02 00 00 00 00 00 00 00"),
            (0x33, "33 34 12 36 f5 01 02 03 04 05 00 00 00 00 00 00 00"),
            (0x5D, "5d 34 12 36 f5 01 02"),
        ):
            for target in (0, 0xF536):
                with self.subTest(opcode=opcode, target=target):
                    changed = bytearray(self.rom)
                    program = bytearray.fromhex(record) + b"\x0f"
                    program[3:5] = target.to_bytes(2, "little")
                    changed[0x4F536:0x4F536 + len(program)] = program
                    commands = graph(PathExtractor(bytes(changed)), PathAddress(0xF536))
                    self.assertEqual(len(commands), 2)
                    self.assertEqual(commands[0].opcode, opcode)
                    self.assertEqual(commands[1].opcode, 0x0F)

    def test_child_parameter_decoder_rejects_other_handlers_and_malformed_records(self):
        extractor = PathExtractor(self.rom)
        command = extractor.decode_command(PathAddress(0xF56A))
        for invalid in (replace(command, raw_hex=command.raw_hex[:-2]),
                        replace(command, prefix_size=1),
                        replace(command, handler_address=0),
                        extractor.decode_command(PathAddress(0xF536))):
            with self.assertRaises(UnsupportedPath):
                child_spawn_parameters(invalid)

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
        self.assertEqual(statements[8], "Statement::SelectedAuxiliaryBranch { condition: SelectedAuxiliaryCondition::Continuation, taken: cursor(4, 10), next: cursor(4, 9) }")

    def test_detaching_sprite_preserves_numbered_child_and_aux_action_gate(self):
        entry, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xF540), 5)
        self.assertEqual(entry, 0)
        self.assertEqual(len(statements), 12)
        self.assertIn("size: 250", statements[0])
        self.assertIn("Assign(ByteOperand::Literal(0))", statements[1])
        self.assertIn("SelectedAuxiliaryCondition::ActionBit40", statements[6])
        self.assertIn("taken: cursor(5, 10), next: cursor(5, 7)", statements[6])
        self.assertIn("RelationshipCommand::UnlinkChild { number: 1 }", statements[9])
        self.assertIn("RelationshipCommand::UnlinkSelf", self.lower_record("65")[0])


if __name__ == "__main__":
    unittest.main()
