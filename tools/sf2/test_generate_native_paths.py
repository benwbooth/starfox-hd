#!/usr/bin/env python3
"""Complete native lowering tests using source data and synthetic byte edits."""

from pathlib import Path
import unittest
import re
from dataclasses import replace

from generate_native_paths import (
    DEFAULT_ROM, OUTPUT, PathAddress, PathExtractor, UnsupportedPath,
    banked_byte_values, banked_word_values, byte_field, child_spawn_parameters, child_spawn_shape, generate, graph, lower_graph, shape_index, trigger_kind, variable_bit_masks, word_field,
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

    def test_primary_motion_ground_limited_root_includes_inline_callee_and_contact_callback(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0xF029))
        _, statements = lower_graph(extractor, PathAddress(0xF029), 0)
        self.assertEqual(len(commands), 20)
        self.assertEqual(len(statements), 20)
        mapped = dict(zip((command.address.offset for command in commands), statements))
        self.assertIn("InheritPrimaryHorizontalMotion", mapped[0xE78A])
        self.assertIn("ContactCommand::IncludeClass", mapped[0xF033])
        self.assertIn("from_authored_class(8)", mapped[0xF033])
        self.assertIn("from_authored_class(128)", mapped[0xF036])
        self.assertIn("from_authored_class(232)", mapped[0xF039])
        self.assertIn("TriggerKind::NewContact", mapped[0xF040])
        self.assertIn("SuppressHitMarker(true)", mapped[0xF044])
        self.assertIn("UnsignedByte(ByteOperand::Actor(ByteField::TargetSpeed))", mapped[0xF046])
        self.assertIn("SpatialCondition::GroundThreshold(0)", mapped[0xF048])
        self.assertIn("ForceAfterCallbacks", mapped[0xF018])
        generated = generate(self.rom, (("PROJECTILE", PathAddress(0xF029)),))
        self.assertIn("use super::path_contact::ContactClassMask;", generated)
        self.assertIn("use super::collision_pass::ExclusionGroups;", generated)

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

    def test_hold_is_terminal_and_does_not_publish_an_unreachable_end(self):
        self.assertEqual(self.lower_record("19"), ["Statement::Control(ControlCommand::Hold)"])
        changed = bytearray(self.rom)
        changed[0x4F536] = 0x19
        extractor = PathExtractor(bytes(changed))
        command = extractor.decode_command(PathAddress(0xF536))
        self.assertFalse(command.successors)
        broken = replace(command, successors=[PathAddress(0xF537)])
        extractor.decode_command = lambda address: broken if address == broken.address else command
        with self.assertRaisesRegex(UnsupportedPath, "PathHold has outgoing edges"):
            lower_graph(extractor, PathAddress(0xF536), 0)

    def test_script_parameter_decodes_to_a_separate_named_byte(self):
        self.assertEqual(byte_field(0x27), "ByteField::ScriptParameter")
        for record, fragment in [
            ("4e 27 2d", "ByteOperation::Assign(ByteOperand::Actor(ByteField::Health))"),
            ("4f 27 0e", "ByteOperation::Assign(ByteOperand::LowWord(WordField::Position(Axis::Y)))"),
            ("6f 27", "ByteOperation::Decrement"),
        ]:
            statement = self.lower_record(record)[0]
            self.assertIn("field: ByteField::ScriptParameter", statement)
            self.assertIn(fragment, statement)
        for opcode in ("d8", "d9"):
            self.assertIn("selector: ByteOperand::Actor(ByteField::ScriptParameter)",
                          self.lower_record(f"{opcode} 27 a1")[0])
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand 27"):
            word_field(0x27)

    def test_facing_lowering_distinguishes_live_selection_fixed_players_and_linked_actor(self):
        for record, command in [
            ("00 0f", "SelectedImmediate"), ("09", "SelectedSmooth"),
            ("0a", "SelectedYaw"), ("00 4b", "FixedPlayerImmediate"),
            ("0e", "LinkedSmooth"), ("00 0e", "LinkedImmediate"),
        ]:
            self.assertEqual(self.lower_record(record)[0],
                f"Statement::Facing {{ command: FacingCommand::{command}, next: cursor(0, 1) }}")
            changed = bytearray(self.rom)
            program = bytes.fromhex(record + " 0f")
            changed[0x4F536:0x4F536 + len(program)] = program
            generated = generate(bytes(changed), (("FACING", PathAddress(0xF536)),))
            self.assertIn("use super::path_steering::FacingCommand;", generated)

    def test_script_working_word_and_its_byte_views_share_one_typed_field(self):
        self.assertEqual(word_field(0xA3), "WordField::ScriptValue")
        for variable, part in [(0xA3, "Low"), (0xA4, "High")]:
            self.assertEqual(byte_field(variable),
                f"ByteField::WordPart {{ field: WordField::ScriptValue, part: BytePart::{part} }}")
        self.assertIn("field: WordField::ScriptValue, operation: WordOperation::Assign(WordOperand::Literal(65296))",
                      self.lower_record("0c 10 ff a3")[0])
        self.assertIn("WordOperand::Actor(WordField::ScriptValue)", self.lower_record("68 a3 36 f5")[0])
        self.assertIn("WordOperand::Actor(WordField::ScriptValue)", self.lower_record("da 27 a3 36 f5")[0])
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand A4"):
            word_field(0xA4)

    def test_unsupported_complete_root_is_rejected_not_partially_published(self):
        # An unsupported independently spawned child rejects its parent too.
        changed = bytearray(self.rom)
        changed[0x4F582:0x4F585] = bytes.fromhex("e5 ff 1f")
        with self.assertRaisesRegex(UnsupportedPath, "unported shared byte 1FFF"):
            lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF561), 2)

    def test_shared_countdown_service_is_complete_and_global_aliases_remain_scoped(self):
        extractor = PathExtractor(self.rom)
        _, statements = lower_graph(extractor, PathAddress(0x04FF), 0)
        self.assertEqual(len(statements), 8)
        self.assertIn("ControlCommand::Jump", statements[0])
        self.assertIn("AppearanceCommand::Visibility(false)", statements[2])
        self.assertIn("RunWhenPaused { enabled: true", statements[3])
        self.assertIn("CountdownCommand::CopyTo(ByteField::Part)", statements[4])
        self.assertIn("CountdownCommand::Decrement", statements[6])
        self.assertIn("ControlCommand::Goto", statements[7])
        for record, operation in (
            ("79 a9 86 d7", "CopyTo(ByteField::Part)"),
            ("7a a9 2a", "CopyTo(ByteField::Part)"),
            ("7d a9 86 d7", "Assign(ByteOperand::Actor(ByteField::Part))"),
            ("7f a9 2a", "Assign(ByteOperand::Actor(ByteField::Part))"),
            ("fb 86 d7 ff", "Assign(ByteOperand::Literal(255))"),
            ("e5 86 d7", "Increment"), ("e7 86 d7", "Decrement"),
        ):
            statement = self.lower_record(record)[0]
            self.assertIn(f"CountdownCommand::{operation}", statement)
            self.assertIn("next: cursor(0, 1)", statement)
        for record in ("79 a9 87 d7", "7a a9 2b", "7d a9 85 d7", "7f a9 29", "fb 87 d7 ff", "e5 85 d7", "e7 87 d7"):
            with self.assertRaisesRegex(UnsupportedPath, "unported shared byte"):
                self.lower_record(record)

    def test_positional_loop_control_preserves_every_authored_byte(self):
        for value in range(256):
            self.assertEqual(self.lower_record(f"00 05 {value:02x}")[0],
                f"Statement::SpatialLoop {{ sound: super::SpatialLoop::from_authored_control({value}), next: cursor(0, 1) }}")

    def test_banked_byte_table_decodes_all_indices_and_live_field_roles(self):
        statement = self.lower_record("90 55 fb 07 a2 8a")[0]
        self.assertTrue(statement.startswith("Statement::Mutate { mutation: Mutation::Byte"))
        self.assertIn("field: ByteField::Animation(AnimationChannel::Shape)", statement)
        self.assertIn("selector: ByteField::WordPart { field: WordField::MotionPhase, part: BytePart::High }", statement)
        values = tuple(map(int, re.search(r"values: &\[([^]]+)\]", statement)[1].split(", ")))
        self.assertEqual(values, tuple(self.rom[0x3FB55:0x3FC55]))
        self.assertIn("next: cursor(0, 1)", statement)
        # A source-data edit must reach every decoder, including index 255.
        changed = bytearray(self.rom)
        changed[0x3FC54] ^= 255
        self.assertEqual(banked_byte_values(changed, 0x07FB55)[255], self.rom[0x3FC54] ^ 255)
        for value in range(256):
            self.assertEqual(banked_byte_values(self.rom, 0x07FB55)[value], self.rom[0x3FB55 + value])
        self.assertEqual(banked_byte_values(self.rom, 0x07FF00), tuple(self.rom[0x3FF00:0x40000]))
        # A table's suffix can be instructions read as DATA, never executed.
        self.assertEqual(len(values), 256)

    def test_banked_byte_lookup_rejects_mutable_windows_and_truncated_data(self):
        for address in (0x071234, 0x077FFF, 0x07FF01, 0x07FFFF, 0x407FFF, 0x7E8000):
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed constant-byte lookup window"):
                banked_byte_values(self.rom, address)
        with self.assertRaisesRegex(UnsupportedPath, "truncated constant-byte lookup"):
            banked_byte_values(self.rom[:0x3FC54], 0x07FB55)
        for record in ("90 55 fb 07 80 8a", "90 55 fb 07 a2 80"):
            with self.assertRaisesRegex(UnsupportedPath, "unported byte operand 80"):
                self.lower_record(record)

    def test_banked_word_lookup_doubles_the_unsigned_byte_index_at_word_width(self):
        statement = self.lower_record("91 55 fc 06 a1 92")[0]
        self.assertIn("Mutation::Word { field: WordField::RelativePosition(Axis::Z)", statement)
        self.assertIn("WordOperation::Assign(WordOperand::Lookup", statement)
        self.assertIn("selector: ByteField::WordPart { field: WordField::MotionPhase, part: BytePart::Low }", statement)
        values = tuple(map(int, re.search(r"values: &\[([^]]+)\]", statement)[1].split(", ")))
        expected = tuple(self.rom[0x37C55 + 2 * i] | (self.rom[0x37C56 + 2 * i] << 8) for i in range(256))
        self.assertEqual(values, expected)
        changed = bytearray(self.rom)
        for index in range(256):
            changed[0x37C55 + 2 * index:0x37C57 + 2 * index] = bytes((index, index ^ 0xA5))
        self.assertEqual(banked_word_values(changed, 0x06FC55),
                         tuple(i | ((i ^ 0xA5) << 8) for i in range(256)))
        self.assertIn("next: cursor(0, 1)", statement)
        # Ending exactly at the last ROM byte is valid; crossing is not.
        self.assertEqual(len(banked_word_values(self.rom, 0x06FE00)), 256)
        with self.assertRaisesRegex(UnsupportedPath, "unreviewed constant-word lookup window"):
            banked_word_values(self.rom, 0x06FE01)
        for address in (0x061234, 0x067FFF, 0x7E8000):
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed constant-byte lookup window"):
                banked_word_values(self.rom, address)
        with self.assertRaisesRegex(UnsupportedPath, "truncated constant-byte lookup"):
            banked_word_values(self.rom[:0x37E54], 0x06FC55)
        # Shape-token destinations still need semantic decoding, not u16 writes.
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand 04"):
            self.lower_record("91 55 fc 06 a1 04")

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

    def test_marker_sound_classes_and_ranges_are_decoded_without_runtime_addresses(self):
        for opcode, mode, extra_import in [
            ("00 06", "DistanceBands(PathSoundClass::DistanceOnly)", "PathSoundClass"),
            ("fa", "DistanceBands(PathSoundClass::Positioned)", "PathSoundClass"),
            ("00 07", "RangeLimited(MarkerRange::Wide)", "MarkerRange"),
            ("00 09", "RangeLimited(MarkerRange::Near)", "MarkerRange"),
        ]:
            for cue in (0, 18, 255):
                record = f"{opcode} {cue:02x}"
                self.assertEqual(self.lower_record(record)[0],
                    f"Statement::MarkerSound {{ id: {cue}, mode: MarkerCueMode::{mode}, next: cursor(0, 1) }}")
                changed = bytearray(self.rom)
                program = bytes.fromhex(record + " 0f")
                changed[0x4F536:0x4F536 + len(program)] = program
                generated = generate(bytes(changed), (("SOUND", PathAddress(0xF536)),))
                self.assertIn("use super::path_sound::MarkerCueMode;", generated)
                self.assertIn(f"use super::path_sound::{extra_import};", generated)

    def test_linked_rotation_refresh_has_no_operand_or_selected_actor_dependency(self):
        self.assertEqual(self.lower_record("f2")[0],
            "Statement::Relationship { command: RelationshipCommand::RefreshLinkedRotation, next: cursor(0, 1) }")

    def test_short_zero_frame_is_the_existing_typed_manual_shape_initializer(self):
        self.assertEqual(self.lower_record("f9"), self.lower_record("1b 00"))
        self.assertEqual(self.lower_record("f9")[0],
            "Statement::Animation { command: AnimationCommand::Initialize { channel: AnimationChannel::Shape, value: 0 }, next: cursor(0, 1) }")

    def test_primary_target_control_literals_and_complete_follower_root(self):
        for word, value in [(0, 0), (32767, 32767), (32768, -32768), (65535, -1)]:
            self.assertIn(f"PlayerControlCommand::Configure({value})", self.lower_record(f"fe {word & 255:02x} {word >> 8:02x}")[0])
        self.assertIn("PlayerControlCommand::RefreshOwnedOrigin", self.lower_record("00 56")[0])
        _, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xF38A), 0)
        self.assertEqual(len(statements), 9)
        self.assertIn("Configure(-8)", statements[2])
        self.assertIn("LockForLinkedMode", statements[3])
        self.assertIn("iterations: 8", statements[4])
        self.assertIn("FollowPrimaryPosition", statements[5])
        self.assertIn("RefreshOwnedOrigin", statements[6])
        self.assertIn("ControlCommand::Next", statements[7])
        self.assertIn("ControlCommand::End", statements[8])
        generated = generate(self.rom, (("FOLLOWER", PathAddress(0xF38A)),))
        self.assertIn("use super::path_player_control::PlayerControlCommand;", generated)
        for offset in (0xF393, 0xF398, 0xF3A0, 0xF3A5):
            changed = bytearray(self.rom)
            changed[0x40000 + offset] ^= 1
            with self.assertRaisesRegex(ValueError, "inline signature mismatch"):
                lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF38A), 0)

    def test_primary_target_configuration_variants_retain_full_word_before_native_low_byte_transform(self):
        for opcode, operation in [("00 57", "ConfigureDoubledLowByte"), ("00 58", "ConfigureAlternateAxes")]:
            for word in (0, 127, 128, 255, 256, 32767, 32768, 65528, 65535):
                value = int.from_bytes(word.to_bytes(2, "little"), "little", signed=True)
                self.assertEqual(self.lower_record(f"{opcode} {word & 255:02x} {word >> 8:02x}")[0],
                    f"Statement::PlayerControl {{ command: PlayerControlCommand::{operation}({value}), next: cursor(0, 1) }}")

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

    def test_primary_horizontal_inheritance_inline_has_reviewed_target_and_return(self):
        _, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xE78A), 0)
        self.assertEqual(statements, [
            "Statement::InheritPrimaryHorizontalMotion { next: cursor(0, 1) }",
            "Statement::Control(ControlCommand::Return)",
        ])
        for offset in (0x4E78C, 0x4E792):  # change callee or returned path
            changed = bytearray(self.rom)
            changed[offset] ^= 1
            with self.assertRaisesRegex(ValueError, "inline signature mismatch"):
                lower_graph(PathExtractor(bytes(changed)), PathAddress(0xE78A), 0)

    def test_trigger_condition_table_decodes_all_eighteen_and_rejects_other_selectors(self):
        expected = ["Always", *(f"Periodic(TriggerPeriod::{period})" for period in (
            "Two", "Four", "Eight", "Sixteen", "ThirtyTwo", "SixtyFour", "OneTwentyEight")),
            "NewContact", "PlayerContact", "ConsumeHitEvent", "Detached", "ZeroHealth",
            "PlayerCrossing", "PlayerPartTarget", "ControlledAuxFlagHigh", "ControlledAuxFlagLow", "TimerPenultimate"]
        for index, kind in enumerate(expected):
            self.assertEqual(trigger_kind(index), f"TriggerKind::{kind}")
            statements = self.lower_record(f"4a 36 f5 {index:02x}")
            self.assertIn(f"kind: TriggerKind::{kind}, timer: 0", statements[0])
            self.assertIn("path: cursor(0, 0)", statements[0])
            self.assertIn("next: cursor(0, 1)", statements[0])
        for index in range(18, 256):
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed trigger condition"):
                self.lower_record(f"4a 36 f5 {index:02x}")

    def test_timed_trigger_keeps_condition_and_duration_separate_including_byte_wrap(self):
        for condition in range(18):
            for duration in [0, 1, 254, 255]:
                statement = self.lower_record(f"00 65 36 f5 {condition:02x} {duration:02x}")[0]
                self.assertIn(f"Trigger::timed(cursor(0, 0), {trigger_kind(condition)}, {duration})", statement)
                self.assertIn("next: cursor(0, 1)", statement)

    def test_relative_trigger_uses_unsigned_displacement_and_wraps_at_path_boundary(self):
        for start, delta in [(0xF536, 0), (0xF536, 3), (0xF536, 128), (0xF536, 255), (0xFFFE, 3)]:
            changed = bytearray(self.rom)
            for index, value in enumerate(bytes([0xFD, delta, 7, 0x0F])):
                changed[0x40000 + ((start + index) & 0xFFFF)] = value
            target = (start + delta) & 0xFFFF
            if delta:
                changed[0x40000 + target] = 0x0F
            _, statements = lower_graph(PathExtractor(bytes(changed)), PathAddress(start), 0)
            statement = next(item for item in statements if "ControlCommand::Register" in item)
            addresses = sorted({start, (start + 3) & 0xFFFF, target})
            self.assertIn(f"path: cursor(0, {addresses.index(target)})", statement)
            self.assertIn("TriggerKind::Periodic(TriggerPeriod::OneTwentyEight), timer: 0", statement)
            if start == 0xF536:  # publishing still requires a verified installer
                generated = generate(bytes(changed), (("RELATIVE", PathAddress(start)),))
                self.assertIn("use super::path_control::TriggerPeriod;", generated)

    def test_cancel_uses_existing_identity_without_claiming_unknown_targets_and_clear_is_separate(self):
        self.assertIn("ControlCommand::Cancel { path: cursor(0, 0), next: cursor(0, 1) }",
            self.lower_record("4b 36 f5")[0])
        with self.assertRaisesRegex(UnsupportedPath, "cancel target lacks catalog identity"):
            self.lower_record("4b 00 00")
        self.assertEqual(self.lower_record("00 7b")[0], "Statement::Control(ControlCommand::Clear { next: cursor(0, 1) })")

    def test_variable_copy_and_add_decode_destination_first_and_width_conversion(self):
        cases = [
            ("4e 0a 0b", "ByteField::TargetSpeed", "ByteOperation::Assign(ByteOperand::Actor(ByteField::Acceleration))"),
            ("4f 0a a1", "ByteField::TargetSpeed", "ByteOperation::Assign(ByteOperand::LowWord(WordField::MotionPhase))"),
            ("50 0c a1", "WordField::Position(Axis::X)", "WordOperation::Assign(WordOperand::Actor(WordField::MotionPhase))"),
            ("51 0c 0a", "WordField::Position(Axis::X)", "WordOperation::Assign(WordOperand::SignedByte(ByteOperand::Actor(ByteField::TargetSpeed)))"),
            ("52 0a 0b", "ByteField::TargetSpeed", "ByteOperation::Add(ByteOperand::Actor(ByteField::Acceleration))"),
            ("53 0a 0b", "ByteField::TargetSpeed", "ByteOperation::Add(ByteOperand::Actor(ByteField::Acceleration))"),
            ("54 0c a1", "WordField::Position(Axis::X)", "WordOperation::Add(WordOperand::Actor(WordField::MotionPhase))"),
            ("55 0c 0a", "WordField::Position(Axis::X)", "WordOperation::Add(WordOperand::SignedByte(ByteOperand::Actor(ByteField::TargetSpeed)))"),
        ]
        for record, destination, value in cases:
            with self.subTest(record=record):
                statements = self.lower_record(record)
                self.assertIn(f"field: {destination}, operation: {value}", statements[0])
                self.assertIn("next: cursor(0, 1)", statements[0])
                self.assertEqual(statements[1], "Statement::Control(ControlCommand::End)")
        self.assertEqual(self.lower_record("52 0a 0b"), self.lower_record("53 0a 0b"))

    def test_reviewed_scalar_and_relative_fields_decode_without_raw_operand_access(self):
        for variable, field in [
            (0x18, "Speed"), (0x28, "RepeatCounter"), (0x2D, "Health"), (0x2E, "AttackPower"),
            (0x94, "RelativeRotation(Axis::X)"), (0x95, "RelativeRotation(Axis::Y)"),
            (0x96, "RelativeRotation(Axis::Z)"), (0xA9, "Part"),
        ]:
            self.assertEqual(byte_field(variable), f"ByteField::{field}")
            self.assertIn(f"field: ByteField::{field}", self.lower_record(f"0b ff {variable:02x}")[0])
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand 94"):
            self.lower_record("50 0c 94")

    def test_variable_bits_decode_selector_first_and_word_destination_second(self):
        for opcode, operation in [(0xD8, "SetBits"), (0xD9, "ClearBits")]:
            statements = self.lower_record(f"{opcode:02x} 2d a1")
            self.assertIn(f"field: WordField::MotionPhase, operation: WordOperation::{operation}", statements[0])
            self.assertIn("selector: ByteOperand::Actor(ByteField::Health), masks: &VARIABLE_BIT_MASKS", statements[0])
            self.assertIn("next: cursor(0, 1)", statements[0])
            self.assertEqual(statements[1], "Statement::Control(ControlCommand::End)")
        statement = self.lower_record("da 2d a1 36 f5")[0]
        self.assertIn("ActorCondition::AnyWordBitsSet(WordOperand::Actor(WordField::MotionPhase)", statement)
        self.assertIn("ByteField::Health", statement)
        self.assertIn("taken: cursor(0, 0), next: cursor(0, 1)", statement)
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand 04"):
            self.lower_record("d8 2d 04")

    def test_motion_flags_and_speed_map_to_distinct_reviewed_services(self):
        for record, operation in [
            ("01", "FollowPlayerDisplacement(true)"),
            ("02", "FollowPlayerDisplacement(false)"),
            ("04", "GenerateVelocityEachStep(true)"),
            ("05", "GenerateVelocityEachStep(false)"),
            ("12", "BankTurn(true)"), ("13", "BankTurn(false)"),
            ("00 5c", "QuadrupleVelocity(true)"),
            ("06 ff", "SetSpeed(255)"),
        ]:
            statements = self.lower_record(record)
            self.assertEqual(statements[0], f"Statement::Motion {{ command: MotionCommand::{operation}, next: cursor(0, 1) }}")
            self.assertEqual(statements[1], "Statement::Control(ControlCommand::End)")
        # A separate source flag cannot be substituted based on its bit number.
        for opcode in ["2e", "2f"]:
            statement = self.lower_record(opcode)[0]
            self.assertIn("ContactCommand::SuppressContactsNextEpoch", statement)
            self.assertNotIn("QuadrupleVelocity", statement)

    def test_contact_class_masks_decode_every_reviewed_bit_and_reject_unknown_mutations(self):
        for mask in range(256):
            for record, operation, retain in [(f"f7 {mask:02x}", "RetainClass", True),
                                               (f"00 76 {mask:02x}", "IncludeClass", False)]:
                if bool(mask & 2) != retain:
                    with self.assertRaisesRegex(UnsupportedPath, "unreviewed contact class bit 02"):
                        self.lower_record(record)
                    continue
                statement = self.lower_record(record)[0]
                self.assertIn(f"ContactCommand::{operation}(ContactClassMask", statement)
                self.assertIn(f"groups: ExclusionGroups::from_authored_class({mask & 0xf8})", statement)
                self.assertIn(f"first_strategy_visit: {str(bool(mask & 4)).lower()}", statement)
                self.assertIn(f"suppress_attack_damage: {str(bool(mask & 1)).lower()}", statement)
                self.assertIn("next: cursor(0, 1)", statement)
        for record, operation in [("2e", "SuppressContactsNextEpoch(true)"),
                                  ("2f", "SuppressContactsNextEpoch(false)"),
                                  ("00 2c", "SuppressHitMarker(true)")]:
            self.assertEqual(self.lower_record(record)[0],
                f"Statement::Contact {{ command: ContactCommand::{operation}, next: cursor(0, 1) }}")

    def test_selected_transform_copies_remain_separate_immediate_statements(self):
        for record, operation in [("ed", "WorldPosition"), ("00 44", "WorldRotation")]:
            self.assertEqual(self.lower_record(record)[0],
                f"Statement::CopySelectedTransform {{ command: SelectedTransformCommand::{operation}, next: cursor(0, 1) }}")
            changed = bytearray(self.rom)
            program = bytes.fromhex(record + " 0f")
            changed[0x4F536:0x4F536 + len(program)] = program
            self.assertIn("use super::path_relationships::SelectedTransformCommand;",
                          generate(bytes(changed), (("COPY", PathAddress(0xF536)),)))

    def test_arithmetic_chase_preserves_literal_and_variable_operand_order_and_waiting(self):
        for record, fragment in [
            ("81 ff 27", "field: ByteField::ScriptParameter, operation: ByteOperation::Chase(ByteOperand::Literal(255))"),
            ("82 01 80 a3", "field: WordField::ScriptValue, operation: WordOperation::Chase(WordOperand::Literal(32769))"),
            ("85 27 2d", "field: ByteField::ScriptParameter, operation: ByteOperation::Chase(ByteOperand::Actor(ByteField::Health))"),
            ("87 a3 a1", "field: WordField::ScriptValue, operation: WordOperation::Chase(WordOperand::Actor(WordField::MotionPhase))"),
        ]:
            self.assertIn(fragment, self.lower_record(record)[0])
        self.assertEqual(self.lower_record("83 ff 27")[0],
            "Statement::WaitChase { field: ByteField::ScriptParameter, target: ByteOperand::Literal(255), next: cursor(0, 1) }")
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand 04"):
            self.lower_record("82 00 00 04")

    def test_halves_preserve_width_and_signed_vs_unsigned_operation(self):
        for record, kind, field, operation in [
            ("8e 18", "Byte", "ByteField::Speed", "HalfTowardZero"),
            ("8f a1", "Word", "WordField::MotionPhase", "HalfTowardZero"),
            ("00 54 18", "Byte", "ByteField::Speed", "LogicalHalf"),
        ]:
            statement = self.lower_record(record)[0]
            self.assertIn(f"Mutation::{kind} {{ field: {field}, operation: {kind}Operation::{operation} }}", statement)
            self.assertIn("next: cursor(0, 1)", statement)

    def test_visibility_shadow_and_draw_distance_use_named_controls(self):
        for record, command in [
            ("48", "Visibility(false)"), ("49", "Visibility(true)"),
            ("5b", "Collision(true)"), ("8c", "Shadow(true)"), ("8d", "Shadow(false)"),
            ("cd", "MaximumDrawDistance(true)"), ("cc", "MaximumDrawDistance(false)"),
        ]:
            self.assertEqual(self.lower_record(record)[0],
                f"Statement::Appearance {{ command: AppearanceCommand::{command}, next: cursor(0, 1) }}")
            changed = bytearray(self.rom)
            changed[0x4F536:0x4F538] = bytes.fromhex(record + " 0f")
            generated = generate(bytes(changed), (("APPEARANCE", PathAddress(0xF536)),))
            self.assertIn("use super::path_appearance::AppearanceCommand;", generated)

    def test_literal_shape_assignment_and_equality_decode_every_catalog_header(self):
        for index in range(577):
            shape = 0xBC9C + index * 28
            self.assertEqual(shape_index(shape), index)
            low, high = shape.to_bytes(2, "little")
            assigned = self.lower_record(f"0c {low:02x} {high:02x} 04")[0]
            self.assertEqual(assigned, f"Statement::Appearance {{ command: AppearanceCommand::Shape(ShapeId::from_catalog_index({index})), next: cursor(0, 1) }}")
            compared = self.lower_record(f"2b 04 {low:02x} {high:02x} 36 f5")[0]
            self.assertEqual(compared, f"Statement::Compare {{ condition: ActorCondition::EqualShape(ShapeId::from_catalog_index({index})), taken: cursor(0, 0), next: cursor(0, 1) }}")
        for shape in [0, 0xBC9B, 0xBC9D, 0xFBB8, 0xFFFF]:
            low, high = shape.to_bytes(2, "little")
            for record in [f"0c {low:02x} {high:02x} 04", f"2b 04 {low:02x} {high:02x} 36 f5"]:
                with self.assertRaisesRegex(UnsupportedPath, "not a catalog header"):
                    self.lower_record(record)
        # Generic arithmetic and variable copies cannot expose a shape pointer.
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand 04"):
            self.lower_record("08 04 01 00")
        for record in ["0c 40 be 04", "2b 04 40 be 36 f5"]:
            changed = bytearray(self.rom)
            program = bytes.fromhex(record + " 0f")
            changed[0x4F536:0x4F536 + len(program)] = program
            generated = generate(bytes(changed), (("SHAPE", PathAddress(0xF536)),))
            self.assertIn("use super::ShapeId;", generated)
            self.assertNotIn("48704", generated)
            self.assertNotIn("0xBE40", generated)

    def test_word_literal_equality_keeps_full_width_and_branch_edges(self):
        for value in [0, 1, 255, 256, 32767, 32768, 65535]:
            low, high = value.to_bytes(2, "little")
            self.assertEqual(self.lower_record(f"2b a1 {low:02x} {high:02x} 36 f5")[0],
                f"Statement::Compare {{ condition: ActorCondition::EqualWord(WordOperand::Actor(WordField::MotionPhase), WordOperand::Literal({value})), taken: cursor(0, 0), next: cursor(0, 1) }}")

    def test_between_literals_keep_encoded_order_width_and_wrapped_bounds(self):
        for opcode, variable, kind, field, boundaries in [
            (0x2C, 0x18, "Byte", "ByteField::Speed", [(0, 255), (255, 0), (128, 128)]),
            (0x2D, 0xA1, "Word", "WordField::MotionPhase", [(0, 65535), (65535, 0), (32768, 32768)]),
        ]:
            for lower, upper in boundaries:
                width = 2 if kind == "Word" else 1
                record = bytes([opcode, variable]) + lower.to_bytes(width, "little") + upper.to_bytes(width, "little") + bytes.fromhex("36 f5")
                self.assertEqual(self.lower_record(record.hex())[0],
                    f"Statement::Compare {{ condition: ActorCondition::Between{kind} {{ value: {kind}Operand::Actor({field}), lower: {kind}Operand::Literal({lower}), upper: {kind}Operand::Literal({upper}) }}, taken: cursor(0, 0), next: cursor(0, 1) }}")

    def test_variable_comparisons_keep_first_second_order_and_width(self):
        for record, condition in [
            ("ee 18 2d", "SecondByteLess(ByteOperand::Actor(ByteField::Speed), ByteOperand::Actor(ByteField::Health))"),
            ("ef a1 0c", "SecondWordLess(WordOperand::Actor(WordField::MotionPhase), WordOperand::Actor(WordField::Position(Axis::X)))"),
            ("f0 18 2d", "EqualByte(ByteOperand::Actor(ByteField::Speed), ByteOperand::Actor(ByteField::Health))"),
            ("f1 a1 0c", "EqualWord(WordOperand::Actor(WordField::MotionPhase), WordOperand::Actor(WordField::Position(Axis::X)))"),
        ]:
            self.assertEqual(self.lower_record(record + " 36 f5")[0],
                f"Statement::Compare {{ condition: ActorCondition::{condition}, taken: cursor(0, 0), next: cursor(0, 1) }}")

    def test_spatial_conditions_decode_literals_and_plane_axes_without_sampling_world(self):
        for record, condition in [
            ("14 ff ff", "SelectedDistanceLess(65535)"),
            ("15 00 80", "LinkedDistanceLess(32768)"),
            ("1a 00 80", "GroundThreshold(-32768)"),
            ("1a ff ff", "GroundThreshold(-1)"),
            ("97 00 80", "WithinSelectedRange(32768)"),
            ("a4 ff", "SelectedWithinYawArc(255)"),
            ("00 21 ff 01", "SelectedRelativeYawBetween { lower: 255, upper: 1 }"),
            ("00 01", "SelectedAbove"),
            ("24", "SelectedAtOrBelow"),
            ("23", "NegativeSelectedPlane(PlaneAxis::Right)"),
            ("5f", "NegativeSelectedPlane(PlaneAxis::Forward)"),
        ]:
            self.assertEqual(self.lower_record(record + " 36 f5")[0],
                f"Statement::Spatial {{ condition: SpatialCondition::{condition}, taken: cursor(0, 0), next: cursor(0, 1) }}")
            changed = bytearray(self.rom)
            program = bytes.fromhex(record + " 36 f5 0f")
            changed[0x4F536:0x4F536 + len(program)] = program
            generated = generate(bytes(changed), (("SPATIAL", PathAddress(0xF536)),))
            self.assertIn("use super::path_conditions::SpatialCondition;", generated)
            self.assertEqual("use super::path_control::PlaneAxis;" in generated, "PlaneAxis" in condition)

    def test_break_pair_discard_and_consuming_hit_branches_keep_their_edges(self):
        self.assertEqual(self.lower_record("46 36 f5")[0],
            "Statement::Control(ControlCommand::Break { target: cursor(0, 0) })")
        self.assertEqual(self.lower_record("47")[0],
            "Statement::Control(ControlCommand::PopStackPair { next: cursor(0, 1) })")
        for mask in [0, 1, 128, 255]:
            self.assertEqual(self.lower_record(f"5a 36 f5 {mask:02x}")[0],
                f"Statement::Branch(BranchCommand::HitFlags {{ mask: {mask}, taken: cursor(0, 0), next: cursor(0, 1) }})")
        self.assertEqual(self.lower_record("3f 36 f5")[0],
            "Statement::Branch(BranchCommand::HitEvent { taken: cursor(0, 0), next: cursor(0, 1) })")

    def test_full_bit_mask_data_is_decoded_offline_and_only_emitted_when_required(self):
        masks = variable_bit_masks(self.rom)
        self.assertEqual(len(masks), 128)
        self.assertEqual(masks[:16], tuple(1 << bit for bit in range(16)))
        self.assertEqual(masks[16], 0xE020)
        self.assertEqual(masks[-1], 0x9902)
        self.assertNotIn("VARIABLE_BIT_MASKS", generate(self.rom))
        changed = bytearray(self.rom)
        changed[0x4F536:0x4F53A] = bytes.fromhex("d8 2d a1 0f")
        generated = generate(bytes(changed), (("BITS", PathAddress(0xF536)),))
        table = re.search(r"const VARIABLE_BIT_MASKS: \[u16; 128\] = \[(.*?)\];", generated, re.S)
        self.assertIsNotNone(table)
        self.assertEqual(tuple(int(value, 16) for value in re.findall(r"0x[0-9A-F]+", table.group(1))), masks)
        self.assertNotIn("7FB5CF", generated)
        changed[0x537CF + 254:0x537CF + 256] = bytes.fromhex("34 12")
        self.assertEqual(variable_bit_masks(bytes(changed))[-1], 0x1234)
        with self.assertRaisesRegex(UnsupportedPath, "truncated"):
            variable_bit_masks(b"")

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

    def test_animation_byte_operands_alias_controls_without_authorizing_word_views(self):
        for variable, channel in ((0x89, "Color"), (0x8A, "Shape")):
            field = f"ByteField::Animation(AnimationChannel::{channel})"
            self.assertEqual(byte_field(variable), field)
            with self.assertRaises(UnsupportedPath):
                word_field(variable)
            for record in (f"0b 00 {variable:02x}", f"6d {variable:02x}",
                           f"4e {variable:02x} 2d", f"52 a1 {variable:02x}"):
                self.assertIn(field, self.lower_record(record)[0])
        self.assertIn("ByteOperation::Add(ByteOperand::Actor(ByteField::WordPart { field: WordField::ScriptValue, part: BytePart::Low }))",
                      self.lower_record("53 89 a3")[0])

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
