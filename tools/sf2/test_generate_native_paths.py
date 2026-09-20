#!/usr/bin/env python3
"""Complete native lowering tests using source data and synthetic byte edits."""

from pathlib import Path
import unittest
import re
import hashlib
from dataclasses import replace

from generate_native_paths import (
    DEFAULT_ROM, OUTPUT, PathAddress, PathExtractor, UnsupportedPath,
    banked_byte_values, banked_word_values, byte_field, child_spawn_parameters, independent_spawn_parameters, spawn_shape, generate, graph, lower_graph, lowering_units, SelectedOffsetAim, shape_index, trigger_kind, variable_bit_masks, word_field, rapid_shot_shapes,
)


class NativePathGenerationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def test_checked_in_catalog_is_exact_generated_output(self):
        from generate_native_paths import generate_reviewed_catalog
        self.assertEqual(OUTPUT.read_text(), generate_reviewed_catalog(self.rom))

    def test_completion_helpers_are_complete_callable_graphs_not_actor_roots(self):
        from generate_native_paths import SUBROUTINES, generate_reviewed_catalog
        extractor = PathExtractor(self.rom)
        expected = [(0x87D3, 7, '370ddda9625cae099be8b72846f1010e92173a4c683a7f7c38bebe29f019f7d4'),
                    (0x87E5, 14, '6eaa28af93dccdbcff0749d8240e0e13f4c1e31b125c1139e02c795f6001dcd6')]
        for address, count, digest in expected:
            root = PathAddress(address)
            self.assertNotIn(root, extractor.discover_roots())
            commands = graph(extractor, root)
            self.assertEqual(len(commands), count)
            self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(), digest)
            self.assertEqual(len(lower_graph(extractor, root, 0)[1]), count)
        source = generate_reviewed_catalog(self.rom)
        self.assertIn('LOWERED_ROOT_COUNT: usize = 145;', source)
        self.assertIn('LOWERED_SUBROUTINE_COUNT: usize = 7;', source)
        self.assertIn('LOWERED_SOURCE_COMMAND_COUNT: usize = 5481;', source)
        for _, _, _, callsite in SUBROUTINES:
            for delta in [0, 1, 2]:
                changed = bytearray(self.rom)
                changed[0x40000 + callsite.offset + delta] ^= 1
                with self.assertRaises(UnsupportedPath):
                    generate_reviewed_catalog(bytes(changed))

    def test_target_proxy_callback_is_a_complete_registered_graph(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x8D08)
        self.assertNotIn(root, extractor.discover_roots())
        commands = graph(extractor, root)
        self.assertEqual([c.raw_hex for c in commands], ['c6', '42'])
        entry, statements = lower_graph(extractor, root, 0)
        self.assertEqual(entry, 0)
        self.assertEqual(statements, ['Statement::ConsiderPrimaryTargetAndMarkSceneProxy { next: cursor(0, 1) }',
                                      'Statement::Control(ControlCommand::Return)'])
        self.assertIn('Statement::ConsiderPrimaryTarget {', self.lower_record('c7')[0])
        self.assertIn('Statement::ConsiderPrimaryTargetAndMarkSceneProxy {', self.lower_record('c6')[0])

    def test_protection_override_has_its_own_non_ifnot_branch_statement(self):
        self.assertEqual(self.lower_record('30 36 f5')[0],
                         'Statement::IfProtectionOverride { taken: cursor(0, 0), next: cursor(0, 1) }')

    def test_view_transition_commands_are_complete_paired_actions_not_simple_flags(self):
        self.assertEqual(self.lower_record('00 4f')[0],
                         'Statement::CopySelectedStoredRotation { next: cursor(0, 1) }')
        self.assertEqual(self.lower_record('00 4e')[0],
                         'Statement::CopySelectedStoredPosition { next: cursor(0, 1) }')
        self.assertEqual(self.lower_record('00 10')[0],
                         'Statement::MoveFixedView { snap: false, next: cursor(0, 1) }')
        self.assertEqual(self.lower_record('00 11')[0],
                         'Statement::MoveFixedView { snap: true, next: cursor(0, 1) }')
        self.assertEqual(self.lower_record('c2')[0],
                         'Statement::ViewTransition { enabled: true, next: cursor(0, 1) }')
        self.assertEqual(self.lower_record('c3')[0],
                         'Statement::ViewTransition { enabled: false, next: cursor(0, 1) }')

    def test_exit_placement_coordinates_are_typed_and_reject_other_word_mappings(self):
        self.assertIn('WordField::MotionScriptOverlap', self.lower_record('da a1 a2 36 f5')[0])
        self.assertIn('SceneByte::PlayerViewControl', self.lower_record('79 a2 e0 1d')[0])
        with self.assertRaises(UnsupportedPath):
            self.lower_record('7d a2 e0 1d')
        self.assertIn('ByteField::RadarMarker', self.lower_record('2a ad 86 36 f5')[0])
        for opcode, operation in [('7b', 'Import'), ('80', 'Export')]:
            for variable, index, coordinate, axis in [('0c', '0b', 'Primary', 'X'), ('10', '0d', 'Depth', 'Z')]:
                statement = self.lower_record(f'{opcode} {variable} {index}')[0]
                self.assertIn(f'PlacementCommand::{operation}', statement)
                self.assertIn(f'PlacementCoordinate::{coordinate}', statement)
                self.assertIn(f'WordField::Position(Axis::{axis})', statement)
            for record in [f'{opcode} 0c 0d', f'{opcode} 10 0b', f'{opcode} 0e 0d']:
                with self.assertRaises(UnsupportedPath):
                    self.lower_record(record)

    def test_scripted_exit_view_and_anchor_have_complete_source_bound_installers(self):
        extractor = PathExtractor(self.rom)
        for address, count, digest, installer, signature in [
            (0x78D2, 314, '142ebef793a2a234891bcab9cc2d7aa90b414122b4ed684e481a64e50581f6dd', 0x857B, '5d9cbcd2786400'),
            (0x7B8F, 2, '80836a35d66632e30a8ab79f35e56e3f6611b3e26d2173e19aafb05348145997', 0x8570, '5d28bd8f7b6400'),
        ]:
            root = PathAddress(address)
            self.assertNotIn(root, extractor.discover_roots())
            commands = graph(extractor, root)
            self.assertEqual(len(commands), count)
            self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(), digest)
            self.assertEqual(len(lower_graph(extractor, root, 0)[1]), count)
            source = extractor.decode_command(PathAddress(installer))
            self.assertIn(source, graph(extractor, PathAddress(0x8D82)))
            self.assertEqual(source.raw_hex, signature)
            changed = bytearray(self.rom)
            changed[0x40000 + installer + 3:0x40000 + installer + 5] = (address + 1).to_bytes(2, 'little')
            with self.assertRaisesRegex(UnsupportedPath, 'no verified child installer'):
                generate(bytes(changed), (('EXIT', root),))
        for shape, path, index in [(0xBD28, 0x7B8F, 5), (0xBC9C, 0x78D2, 0)]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, 'ObjectKind::Effect'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))

    def test_four_panel_objective_closes_panels_emitters_fighters_and_completion(self):
        root = PathAddress(0x5B96)
        extractor = PathExtractor(self.rom)
        self.assertIn(root, extractor.discover_roots())
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 396)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         '4757d51e3a06ca26f100f9f0de308e1d066e7dcbd52996e7dcf7b6c9d62ba567')
        self.assertEqual(len(lower_graph(extractor, root, 0)[1]), 396)
        for shape, path, index, kind in [(0xF2A4, 0x5AAC, 494, 'Enemy'),
                (0xF2DC, 0x5B00, 496, 'Enemy'), (0xF154, 0x5B61, 482, 'Effect'),
                (0xCC5C, 0x5E1C, 144, 'Effect'), (0xF288, 0x7F78, 493, 'Scenery')]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, f'ObjectKind::{kind}'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))

    def test_articulated_encounter_spawns_and_boundary_counter_are_source_bound(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0x8D82))
        for installer, shape, path, index, kind in [
                (0x8DB4, 0xBC9C, 0x910D, 0, 'Effect'),
                (0x9114, 0xE15C, 0x918D, 336, 'Effect'),
                (0x9191, 0xE15C, 0x91C8, 336, 'Effect'),
                (0x8F20, 0xEF5C, 0x52EC, 464, 'Effect'),
                (0x9224, 0xC8C0, 0x923E, 111, 'Effect'),
                (0x91CC, 0xE178, 0x9206, 337, 'Enemy')]:
            command = extractor.decode_command(PathAddress(installer))
            self.assertIn(command, commands)
            spawn = child_spawn_parameters(command) if command.opcode in (0x33, 0xF5) else independent_spawn_parameters(command)
            self.assertEqual((spawn.shape, spawn.path), (shape, PathAddress(path)))
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, f'ObjectKind::{kind}'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))
        for address, raw in [(0x8F74, '79993fd7'), (0x8FDD, 'fb3fd700'),
                             (0x900F, 'e53fd7'), (0x9097, 'e53fd7'),
                             (0x909F, 'e53fd7'), (0x90F1, 'e53fd7'), (0x90F9, 'e53fd7')]:
            self.assertEqual(extractor.decode_command(PathAddress(address)).raw_hex, raw)
            self.assertIn('CoordinationField::BoundaryCorrections', self.lower_record(raw)[0])
        for record, operation in [('7d 99 3f d7', 'Assign'), ('e7 3f d7', 'Decrement')]:
            statement = self.lower_record(record)[0]
            self.assertIn('CoordinationField::BoundaryCorrections', statement)
            self.assertIn(f'CoordinationCommand::{operation}', statement)
        for address in (0xD73E, 0xD740):
            with self.assertRaises(UnsupportedPath):
                self.lower_record('79 99 ' + address.to_bytes(2, 'little').hex(' '))

    def test_queen_dioray_complete_graph_and_linked_position_operand_forms(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x8D82)
        self.assertIn(root, extractor.discover_roots())
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 975)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         '5a315fba4a753aa1fa2ceb01fee0d024bb3687fdd66f9c104c380fc00940b724')
        self.assertEqual(len(lower_graph(extractor, root, 0)[1]), 975)
        self.assertEqual(extractor.decode_command(PathAddress(0x8DAF)).raw_hex, 'fc77d7b389')
        for value in range(256):
            self.assertIn(f'distance: ByteOperand::Literal({value})', self.lower_record(f'00 2e {value:02x}')[0])
        self.assertIn('distance: ByteOperand::Actor(ByteField::Rotation(Axis::X))', self.lower_record('00 2f 12')[0])
        with self.assertRaises(UnsupportedPath):
            self.lower_record('00 2f 06')

    def test_progress_gated_exits_are_complete_independently_installed_graphs(self):
        extractor = PathExtractor(self.rom)
        for address, count, digest in [
                (0x0AE7, 399, '0933ddb28ecc7cdda0095e4d9e6b69fde157f4e0ce53f2db5b1e6816a2606620'),
                (0x0AEA, 398, '5559c25d50e76c8f995fc3d4c5d2bf71d4e5e3a214c849376b12fb60c0e6aed6')]:
            root = PathAddress(address)
            self.assertIn(root, extractor.discover_roots())
            commands = graph(extractor, root)
            self.assertEqual(len(commands), count)
            self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(), digest)
            self.assertEqual(len(lower_graph(extractor, root, 0)[1]), count)
        self.assertEqual(extractor.decode_command(PathAddress(0x0AE7)).raw_hex, '4e142d')

    def test_child_auxiliary_link_thunks_and_identity_swap_are_typed_not_scalar(self):
        extractor = PathExtractor(self.rom)
        for address, continuation in [(0x2059, 0x2065), (0x9122, 0x912E),
                (0x919F, 0x91AB), (0x91DA, 0x91E6), (0xF9A1, 0xF9AD), (0xFDDC, 0xFDE8)]:
            raw = bytes.fromhex('ac71d7961cc220a9') + continuation.to_bytes(2, 'little') + b'\x6b'
            self.assertEqual(self.rom[0x40001 + address:0x40001 + address + len(raw)], raw)
            extractor.decode_command(PathAddress(address))
            # A synthetic END after the untouched thunk isolates its lowering.
            changed = bytearray(self.rom)
            changed[0x40000 + continuation] = 0x0F
            entry, statements = lower_graph(PathExtractor(bytes(changed)), PathAddress(address), 0)
            self.assertIn('Statement::LinkLastSpawnToSelf', statements[0])
            for offset in range(len(raw)):
                mutated = bytearray(changed)
                mutated[0x40001 + address + offset] ^= 1
                with self.assertRaisesRegex(ValueError, 'inline signature mismatch'):
                    lower_graph(PathExtractor(bytes(mutated)), PathAddress(address), 0)
        for record in ['00 78 1c 06', '00 78 06 1c']:
            self.assertIn('RelationshipCommand::SwapAttachmentAndAuxiliary', self.lower_record(record)[0])
        for record in ['00 78 1c 0c', '00 78 06 0c']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_scene_continuation_retains_the_current_instruction_then_ends(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x1682)
        self.assertNotIn(root, extractor.discover_roots())
        self.assertEqual([c.raw_hex for c in graph(extractor, root)], ['b6', '0f'])
        entry, statements = lower_graph(extractor, root, 0)
        self.assertEqual(entry, 0)
        self.assertEqual(statements, ['Statement::PreserveSceneContinuation { next: cursor(0, 1) }',
                                      'Statement::Control(ControlCommand::End)'])

    def test_all_authored_health_display_labels_are_data_bound_and_unknown_pointers_fail(self):
        expected = [(0x8941, 'ALGY'), (0x8946, 'PIGMA'), (0x894C, 'LEON'),
                    (0x8951, 'WOLF'), (0x8956, 'QUEEN DRAGOON'), (0x8964, 'KAMANTIS'),
                    (0x896D, 'H.FANTRON'), (0x8977, 'TEKTRON'), (0x897F, 'MIRAGE DRAGON'),
                    (0x898D, 'KNIGHT NACK'), (0x8999, 'KICK GUNNER'), (0x89A5, 'HEAVY CHARIOT'),
                    (0x89B3, 'QUEEN DIORAY'), (0x89C0, 'TAL KONG'), (0x89C9, 'SPACE BLADE'),
                    (0x89D5, 'KING DODORA')]
        actual_pointers = set()
        for command in PathExtractor(self.rom).extract().commands:
            raw = bytes.fromhex(command.raw_hex)
            if command.opcode == 0xFC and raw[1:3] == b'\x77\xd7':
                actual_pointers.add(int.from_bytes(raw[3:5], 'little'))
        self.assertEqual(actual_pointers, {pointer for pointer, _ in expected})
        for pointer, label in expected:
            record = bytes([0xFC, 0x77, 0xD7]) + pointer.to_bytes(2, 'little')
            self.assertEqual(self.lower_record(record.hex())[0],
                             f'Statement::SetHealthDisplayLabel {{ label: "{label}", next: cursor(0, 1) }}')
            # Every character and the terminator is certified, not just the start.
            for delta in range(len(label) + 1):
                changed = bytearray(self.rom)
                changed[0x10000 + pointer + delta] ^= 1
                changed[0x4F536:0x4F53C] = record + b'\x0f'
                with self.assertRaisesRegex(UnsupportedPath, 'unexpected .* display label'):
                    lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF536), 0)
        for pointer in [0, 0x8940, 0x8942, 0x89D6, 0x89E1, 0xFFFF]:
            with self.assertRaisesRegex(UnsupportedPath, 'unreviewed health display label'):
                self.lower_record((bytes([0xFC, 0x77, 0xD7]) + pointer.to_bytes(2, 'little')).hex())

    def test_completion_word_import_export_reject_unreviewed_neighbors_and_byte_views(self):
        self.assertIn('Statement::ImportObjectiveCompletion { destination: WordField::ScriptValue', self.lower_record('7b a3 43')[0])
        self.assertIn('Statement::ExportObjectiveCompletion { source: WordOperand::Actor(WordField::ScriptValue)', self.lower_record('80 a3 43')[0])
        for record in ['7b a3 42', '7b a3 44', '80 a3 42', '80 a3 44', '7a a1 43', '7f a1 43']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_transition_wait_is_a_complete_scratch_preserving_live_coordination_loop(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x872C)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 7)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         '09eb52f996a981262adb5947e2bf9f35f3b1981f1b923f178414d45bfeb4c764')
        entry, statements = lower_graph(extractor, root, 0)
        self.assertEqual(entry, 0)
        self.assertEqual(len(statements), 7)
        self.assertIn('SaveByte', statements[0])
        self.assertIn('CoordinationField::TransitionReady', statements[1])
        self.assertIn('ActorCondition::NonzeroByte', statements[2])
        self.assertIn('RestoreByte', statements[3])
        self.assertIn('ControlCommand::Goto { target: cursor(0, 0)', statements[4])
        self.assertIn('RestoreByte', statements[5])
        self.assertEqual(statements[6], 'Statement::Control(ControlCommand::Return)')
        self.assertIn('CoordinationField::TransitionReady', self.lower_record('fb d5 d7 01')[0])
        for record in ['7b a3 79', '80 a3 79', '7a a1 78', '7a a1 7a']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_map_spawned_core_defender_closes_head_gate_beam_and_progress_publication(self):
        from generate_native_paths import verified_map_spawn_installer
        root = PathAddress(0x5E68)
        extractor = PathExtractor(self.rom)
        self.assertNotIn(root, extractor.discover_roots())
        self.assertTrue(verified_map_spawn_installer(self.rom, root))
        self.assertFalse(verified_map_spawn_installer(self.rom, PathAddress(0x5E69)))
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 85)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         'f4a844b88662cb1742649c6d6023ba7d90dd711aa2aa5fc1520b2992533f18aa')
        statements = lower_graph(extractor, root, 0)[1]
        self.assertEqual(len(statements), 85)
        mapped = dict(zip((c.address.offset for c in commands), statements))
        self.assertIn('TriggerKind::PlayerContact', mapped[0x5E80])
        self.assertIn('part: BytePart::High', mapped[0x5EBB])
        self.assertIn('RetireChild { number: 11 }', mapped[0x5EC1])
        for shape, path, index in [(0xF330, 0x7F78, 499), (0xF34C, 0x6002, 500)]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, 'ObjectKind::Effect'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))
        for offset in [0x2AB4C, 0x2AB54, 0x2AB55, 0x2ABAC, 0x2CFBD, 0x2D299]:
            changed = bytearray(self.rom)
            changed[offset] ^= 1
            with self.assertRaises(UnsupportedPath):
                generate(bytes(changed), (('CORE_DEFENDER', root),))

    def test_planetary_core_closes_shield_health_phases_and_live_count_decrements(self):
        from generate_native_paths import core_beam_coordinates
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x5E1D)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 194)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         '9ddf24150b26862ce3034d8d7bff50944e83a68329de3a360c6964e5c145f6de')
        statements = lower_graph(extractor, root, 0)[1]
        self.assertEqual(len(statements), 194)
        self.assertEqual(sum('Statement::ObjectiveCounts' in s for s in statements), 3)
        self.assertEqual(sum('ActorCondition::EqualMaterial' in s for s in statements), 1)
        self.assertEqual(sum('Statement::SelectRelativeCoordinate' in s for s in statements), 2)
        self.assertEqual(core_beam_coordinates(self.rom, 0x06FBC9), (0, 0, 280, -280))
        self.assertEqual(core_beam_coordinates(self.rom, 0x06FBCD), (280, -280, 0, 0))
        for shape, path, index, kind in [(0xEB6C, 0x5EEB, 428, 'Enemy'),
                                       (0xF314, 0x5EC4, 498, 'Effect'), (0xEB6C, 0x6002, 428, 'Effect')]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, f'ObjectKind::{kind}'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))
        for offset in [0x45F3A, 0x45F3C, 0x45F4D, 0x45F4E, 0x45F54, 0x45F67]:
            changed = bytearray(self.rom)
            changed[offset] ^= 1
            with self.assertRaises(UnsupportedPath):
                core_beam_coordinates(bytes(changed), 0x06FBC9)
        for record in ['91 c9 fb 06 2d 8e', '91 c9 fb 06 2e 92', '91 cd fb 06 2e 8e']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_material_comparison_uses_asset_identity_not_a_word_operand(self):
        statement = '\n'.join(self.lower_record('2b 8c fe 82 00 00'))
        self.assertIn('ActorCondition::EqualMaterial(super::render::MaterialSetId::from_catalog_token(33534))', statement)
        for record in ['2b 8c fd 82 00 00', '2b 8c ff 82 00 00', '2a 8c fe 00 00']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_four_turret_encounter_closes_constructor_gate_and_saved_link_death(self):
        from generate_native_paths import radial_turret_coordinates
        extractor = PathExtractor(self.rom)
        root = PathAddress(0xF136)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 184)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         '94c13c56bb793f8df6c4c850495c33ef301e9c18d16b903ec69fbc1d0ecf6009')
        statements = lower_graph(extractor, root, 0)[1]
        self.assertEqual(len(statements), 184)
        self.assertEqual(sum('SelectRelativeCoordinate' in s for s in statements), 2)
        self.assertEqual(sum('StackValueCommand::SaveAttachment' in s for s in statements), 1)
        self.assertEqual(sum('StackValueCommand::RestoreAttachment' in s for s in statements), 1)
        self.assertEqual(radial_turret_coordinates(self.rom, 0x07FEA1), (0, 800, 0, -800))
        self.assertEqual(radial_turret_coordinates(self.rom, 0x07FEA9), (-800, 0, 800, 0))
        for shape, path, index, kind in [(0xC3F0, 0xF1D5, 67, 'Effect'),
                                       (0xC40C, 0xF21B, 68, 'Enemy'), (0xBECC, 0x5F69, 20, 'Effect')]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, f'ObjectKind::{kind}'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))
        for offset in [0x4F1DD, 0x4F1EE, 0x4F1F1, 0x4F1F5, 0x4F223, 0x4F22F]:
            changed = bytearray(self.rom)
            changed[offset] ^= 1
            with self.assertRaises(UnsupportedPath):
                radial_turret_coordinates(bytes(changed), 0x07FEA1)
        for record in ['91 a1 fe 07 2d 8e', '91 a1 fe 07 2e 92', '91 a9 fe 07 2e 8e']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_multipart_node_objective_closes_children_completion_and_handoff(self):
        from generate_native_paths import node_reveal_shapes
        from dump_runtime_routine import source_offset
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x546C)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 364)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         '9a00cbe314f85336729f05f9bdb125f174244c6c56c72a535b3b9c9acaf7d032')
        statements = lower_graph(extractor, root, 0)[1]
        self.assertEqual(len(statements), 364)
        self.assertEqual(sum('Statement::ExportActiveNodeFlags' in s for s in statements), 1)
        self.assertEqual(sum('Statement::EncounterHandoff' in s for s in statements), 4)
        self.assertEqual(node_reveal_shapes(self.rom), (276, 278, 262, 274))
        for dependency in [0x54DF, 0x54F6, 0x55D1, 0x55D9, 0x5604, 0x5624,
                           0x5667, 0x5682, 0x56F4, 0x5A02, 0x5A0D, 0x8A1B, 0x8B61]:
            self.assertIn(PathAddress(dependency), {c.address for c in commands})
        for shape, path, index in [(0xD714, 0x55D9, 242), (0xD784, 0x5682, 246),
                                    (0xD7A0, 0x56F4, 247), (0xC0A8, 0x830D, 37),
                                    (0xBC9C, 0x5624, 0), (0xBC9C, 0x55D1, 0),
                                    (0xBC9C, 0x5A0D, 0), (0xBC9C, 0x5A02, 0)]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, 'ObjectKind::Effect'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))
        for offset in [0x4562D, 0x4563E, 0x45646, 0x4564E, 0x45658, source_offset(0x06FC4D)]:
            changed = bytearray(self.rom)
            changed[offset] ^= 1
            with self.assertRaises(UnsupportedPath):
                node_reveal_shapes(bytes(changed))
        changed = bytearray(self.rom)
        changed[0x45507] ^= 1
        with self.assertRaises(UnsupportedPath):
            lower_graph(PathExtractor(bytes(changed)), root, 0)

    def test_direct_node_objective_is_an_independent_entry_not_the_forced_part_prologue(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x548E)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 355)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         'a4a5f4f3f21b80198c02b991bed54ba4e3615352069a3bebc502b5592df4a31f')
        self.assertTrue({c.address for c in commands}.issubset(
            {c.address for c in graph(extractor, PathAddress(0x546C))}))
        statements = lower_graph(extractor, root, 0)[1]
        self.assertIn('field: ByteField::ScriptParameter', statements[0])
        self.assertIn('ByteOperand::Actor(ByteField::Health)', statements[0])
        self.assertNotIn(PathAddress(0x548B), {c.address for c in commands})

    def test_encounter_gate_closes_five_parts_firing_and_handoff(self):
        from generate_native_paths import encounter_gate_shapes
        from dump_runtime_routine import source_offset
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x4D7E)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 87)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         'b57d166b9650bc0a8298d4ad3a95c5b73d8aa719aee26e5daeabc8fe8f1bbe44')
        statements = lower_graph(extractor, root, 0)[1]
        self.assertEqual(len(statements), 87)
        self.assertEqual(sum('Statement::EncounterHandoff' in s for s in statements), 4)
        self.assertEqual(sum('Statement::SpawnParameter' in s for s in statements), 3)
        mapped = {command.address.offset: statement for command, statement in zip(commands, statements)}
        self.assertIn('ActorCondition::ZeroByte', mapped[0x4D95])
        self.assertIn('ActorCondition::NonzeroByte', mapped[0x4DC2])
        self.assertEqual(encounter_gate_shapes(self.rom), (390, 384, 387, 393, 123))
        for dependency in [0x811B, 0x8985, 0x8D0A, 0xCFC8, 0xCFDE, 0x4DF1, 0x4DF9, 0x4E18]:
            self.assertIn(PathAddress(dependency), {c.address for c in commands})
        for shape, path, index, kind in [(0xC3B8, 0xCFC8, 65, 'Effect'), (0xE7EC, 0x4DF9, 396, 'Projectile')]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, f'ObjectKind::{kind}'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))
        # Shape count is established by this exact helper, not a permissive
        # unbounded source-memory view. Reject an altered count/index/stride.
        for offset in [0x4811E, 0x48120, 0x48132, 0x4813D, source_offset(0x06FC69)]:
            changed = bytearray(self.rom)
            changed[offset] ^= 1
            with self.assertRaises(UnsupportedPath):
                encounter_gate_shapes(bytes(changed))
        for offset in [0x44DE6, 0x44DEA, 0x44DEE]:
            changed = bytearray(self.rom)
            changed[offset] ^= 1
            with self.assertRaises(UnsupportedPath):
                lower_graph(PathExtractor(bytes(changed)), root, 0)

    def test_launch_transition_closes_camera_and_both_sprite_callbacks(self):
        from generate_native_paths import pilot_craft_appearances
        from dump_runtime_routine import source_offset
        extractor = PathExtractor(self.rom)
        root = PathAddress(0xDC42)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 74)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         '12cf4096a656c44f6bf616ab44e358d6900e244df45ecb368f4758b575bd1ff7')
        statements = lower_graph(extractor, root, 0)[1]
        self.assertEqual(len(statements), 74)
        for operation in ['SelectActivePilotCraft', 'AlignCameraHeading', 'PublishCameraTrackingTarget', 'UpdateLowShieldVisual']:
            self.assertEqual(sum(f'Statement::{operation}' in s for s in statements), 1)
        for dependency in [0xDC8F, 0xDCA7, 0xDCB1, 0xE937, 0xE927, 0xF32F, 0xF521]:
            self.assertIn(PathAddress(dependency), {c.address for c in commands})
        self.assertEqual(pilot_craft_appearances(self.rom),
                         ((52, 0x81F4), (52, 0x82FE), (53, 0x81F4), (53, 0x82FE), (85, 0x81F4), (85, 0x82FE)))
        for shape, path, index in [(0xBC9C, 0xDCB1, 0), (0xBEB0, 0xF32F, 19), (0xC08C, 0xF521, 36)]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, 'ObjectKind::Effect'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))
        for offset in [0x04DCBE, 0x04E93A]:
            # Path bank 44 comes from file 04xxxx, not a live CPU alias.
            changed = bytearray(self.rom)
            changed[offset] ^= 1
            with self.assertRaises((UnsupportedPath, ValueError)):
                lower_graph(PathExtractor(bytes(changed)), root, 0)
        for address in [0x068117, 0x068135]:
            changed = bytearray(self.rom)
            changed[source_offset(address)] ^= 1
            with self.assertRaises(UnsupportedPath):
                pilot_craft_appearances(bytes(changed))

    def test_heavy_chariot_closes_rotated_spawner_dependencies_and_reflection_callbacks(self):
        from generate_native_paths import offset_spawn_parameters
        e = PathExtractor(self.rom)
        root = PathAddress(0x0F7E)
        commands = graph(e, root)
        self.assertEqual(len(commands), 462)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         '1bc57c2cb3c5d0ef6b7943470f4ad47a05d11fecb4c8f7b512a834c2f392565e')
        statements = lower_graph(e, root, 0)[1]
        self.assertEqual(len(statements), 462)
        self.assertEqual(sum('Statement::SpawnOffset' in s for s in statements), 1)
        self.assertEqual(sum('Statement::ReflectContactShots' in s for s in statements), 1)
        self.assertTrue(any('label: "HEAVY CHARIOT"' in s for s in statements))
        for dependency in [0x12E5, 0xF5A1, 0x1109, 0x0A0D, 0x89E3]:
            self.assertIn(PathAddress(dependency), {c.address for c in commands})
        spawn = e.decode_command(PathAddress(0x105B))
        self.assertEqual(spawn.successors, (PathAddress(0x106B),))
        self.assertEqual(offset_spawn_parameters(spawn).offset, (0, -32, 32))
        for index, byte in enumerate([10, 12, 14]):
            for low in range(256):
                raw = bytearray.fromhex(spawn.raw_hex)
                raw[byte] = low
                raw[byte+1] = low ^ 255
                offset = offset_spawn_parameters(replace(spawn, raw_hex=raw.hex())).offset
                expected = [0, -32, 32]
                expected[index] = low if low < 128 else low - 256
                self.assertEqual(offset, tuple(expected))
        for mutation in [replace(spawn, raw_hex=spawn.raw_hex[:-2]), replace(spawn, prefix_size=1),
                         replace(spawn, handler_address=spawn.handler_address+1)]:
            with self.assertRaises(UnsupportedPath):
                offset_spawn_parameters(mutation)
        # Depend on the whole emitted actor path even without any other root
        # in the catalog, and reject a changed unsupported spawn dependency.
        changed = bytearray(self.rom)
        changed[0x4105E:0x41060] = b'\x00\x80'
        with self.assertRaises(UnsupportedPath):
            lower_graph(PathExtractor(bytes(changed)), root, 0)
        for shape in [0xCA9C, 0xCAB8]:
            self.assertEqual(spawn_shape(shape, PathAddress(0x1109))[1], 'ObjectKind::Enemy')
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(0x110A))

    def test_tal_kong_closes_controller_hands_and_death_presentation(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0xA2E6)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 431)
        self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(),
                         '1211c412109c035628e3fb00a2bd5c244122a4d7a8eca56e4d3f805ebda437cc')
        statements = lower_graph(extractor, root, 0)[1]
        self.assertEqual(len(statements), 431)
        self.assertEqual(sum('PublishEncounterCameraFocus' in s for s in statements), 2)
        self.assertEqual(sum('label: "TAL KONG"' in s for s in statements), 1)
        self.assertEqual(sum('Statement::HealthDisplay {' in s for s in statements), 3)
        for dependency in [0xA333, 0xA33E, 0xA34B, 0xA363, 0xA481, 0xA496,
                           0xA4ED, 0xAF1A, 0xAF2E, 0xAF35, 0xAF6F, 0xAFD8,
                           0x86DE, 0x8C8B, 0x44B2]:
            self.assertIn(PathAddress(dependency), {c.address for c in commands})
        for shape, path, index in [(0xE028, 0xA4ED, 325), (0xE044, 0xAF2E, 326)]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, 'ObjectKind::Effect'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))
        self.assertEqual(spawn_shape(0xDFF0, PathAddress(0xA481)), (323, 'ObjectKind::Enemy'))
        for offset, value, error in [(0x4A324, 0xC1, 'unreviewed health display label'),
                                     (0x189C0, 0x58, 'unexpected tal kong display label')]:
            changed = bytearray(self.rom)
            changed[offset] = value
            with self.assertRaisesRegex(UnsupportedPath, error):
                lower_graph(PathExtractor(bytes(changed)), root, 0)

    def test_kick_gunners_close_both_arena_graphs_with_decoded_health_and_route_state(self):
        extractor = PathExtractor(self.rom)
        for address, count, folded, checksum in [
            (0x32EF, 457, 11, '231da621345fbea3fef7cf92c0d6da4502173870c12466b4928abd34eff47148'),
            (0x348B, 419, 10, '8c6797a18db5629795c20dd25177abd98eebc0f744dd7161cbb8ed4a6fcd5b5b'),
        ]:
            root = PathAddress(address)
            commands = graph(extractor, root)
            self.assertEqual(len(commands), count)
            self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(), checksum)
            statements = lower_graph(extractor, root, 0)[1]
            self.assertEqual(len(statements), count - folded)
            for name, expected in [('ChooseGunnerRoute', 1), ('SetHealthDisplayLabel', 1),
                                   ('Statement::HealthDisplay {', 3), ('RequestPrimaryEncounterFeedback', 1)]:
                self.assertEqual(sum(name in s for s in statements), expected)
            self.assertTrue(any('label: "KICK GUNNER"' in s for s in statements))
            for dependency in [0x806F, 0x8082, 0x34D8, 0x8C36, 0x8C48, 0x8707, 0x86B4, 0x44B2]:
                self.assertIn(PathAddress(dependency), {c.address for c in commands})
        for offset, replacement, root, error in [
            (0x43453, b'\xff', 0x32EF, 'unexpected random gunner route block'),
            (0x4345B, b'\x43', 0x32EF, 'unexpected random gunner route block'),
            (0x43468, b'\x03', 0x32EF, 'unexpected random gunner route block'),
            (0x43587, b'\xff', 0x348B, 'unexpected random gunner route block'),
            (0x435B7, b'\x8e', 0x348B, 'unexpected random gunner route block'),
            (0x43303, b'\x54\x34', 0x32EF, 'external entry into random gunner route block'),
            (0x4349F, b'\x88\x35', 0x348B, 'external entry into random gunner route block'),
            (0x432FF, b'\xd8', 0x32EF, 'unreviewed external word store'),
            (0x4349C, b'\x9a', 0x348B, 'unreviewed health display label'),
            (0x18999, b'X', 0x348B, 'unexpected kick gunner display label'),
        ]:
            changed = bytearray(self.rom)
            changed[offset:offset + len(replacement)] = replacement
            with self.assertRaisesRegex(UnsupportedPath, error):
                lower_graph(PathExtractor(bytes(changed)), PathAddress(root), 0)

    def test_gunner_route_tables_keep_every_reachable_value_and_reject_unbounded_connections(self):
        from generate_native_paths import RandomGunnerRoute, source_offset
        for root, x_table, z_table in [(0x32EF, 0x06FE43, 0x06FE4B), (0x348B, 0x06FE37, 0x06FE35)]:
            def route_units(rom):
                return next(u for u in lowering_units(PathExtractor(rom), PathAddress(root)) if isinstance(u, RandomGunnerRoute)).routes
            baseline = route_units(self.rom)
            self.assertEqual([r[4] for r in baseline], [1, 3, 0, 2, 1, 3, 0, 2])
            self.assertEqual([r[5] for r in baseline], [0, 64, 128, 64, 192, 128, 192, 0])
            for axis, table in enumerate([x_table, z_table]):
                with self.assertRaises(UnsupportedPath):
                    banked_word_values(self.rom, table)
                for vertex in range(4):
                    changed = bytearray(self.rom)
                    start = source_offset(table) + 2 * vertex
                    changed[start:start+2] = b'\x00\x80'
                    routes = route_units(bytes(changed))
                    for choice, route in enumerate(routes):
                        if choice // 2 == vertex:
                            self.assertEqual(route[axis], -32768)
                        if route[4] == vertex:
                            self.assertEqual(route[axis + 2], -32768)
            for choice in range(8):
                changed = bytearray(self.rom)
                changed[source_offset(0x06FD3D) + choice] = 255
                self.assertEqual(route_units(bytes(changed))[choice][5], 255)
                changed = bytearray(self.rom)
                changed[source_offset(0x06FD35) + choice] = 4
                with self.assertRaisesRegex(UnsupportedPath, 'exceeds proven four-waypoint domain'):
                    route_units(bytes(changed))
            if root == 0x32EF:
                for vertex in range(4):
                    changed = bytearray(self.rom)
                    changed[source_offset(0x06FE3F) + vertex] = 255
                    routes = route_units(bytes(changed))
                    self.assertEqual(routes[vertex*2][6], 255)
                    self.assertEqual(routes[vertex*2+1][6], 255)

    def test_popup_turrets_close_discharge_graph_and_fold_only_masked_eight_choice_block(self):
        extractor = PathExtractor(self.rom)
        for address, count, checksum in [
            (0x2F11, 377, 'f2eac05282ea0e1607ea32fc904c8048f93f3fc82e5876bb72ae4cee93544222'),
            (0x2F1A, 378, 'c30bf7ee2b615d48dbc5674ddff34091fe9038beed9b9f68c2e8909f216a4bfb'),
        ]:
            root = PathAddress(address)
            commands = graph(extractor, root)
            self.assertEqual(len(commands), count)
            self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(), checksum)
            statements = lower_graph(extractor, root, 0)[1]
            self.assertEqual(len(statements), count - 6)
            self.assertEqual(sum('ChoosePatrolDestination' in s for s in statements), 1)
            self.assertEqual(sum('SceneByte::EncounterLayout' in s for s in statements), 1)
            for dependency in [0x8C4A, 0x8C75, 0x3016, 0x8402, 0x44B2]:
                self.assertIn(PathAddress(dependency), {c.address for c in commands})
        for offset, changed_bytes, expected in [
            (0x42F90, b'\xff', 'unexpected random patrol destination block'),
            (0x42F98, b'\x75', 'unexpected random patrol destination block'),
            (0x42FA5, b'\x80', 'unexpected random patrol destination block'),
            (0x42F18, b'\x91\x2f', 'external entry into random patrol destination block'),
        ]:
            changed = bytearray(self.rom)
            changed[offset:offset+len(changed_bytes)] = changed_bytes
            with self.assertRaisesRegex(UnsupportedPath, expected):
                lower_graph(PathExtractor(bytes(changed)), PathAddress(0x2F11), 0)
        with self.assertRaisesRegex(UnsupportedPath, 'external entry into random patrol destination block'):
            lower_graph(extractor, PathAddress(0x2F97), 0)
        self.assertEqual(spawn_shape(0xBECC, PathAddress(0x8C4A)), (20, 'ObjectKind::Effect'))
        with self.assertRaises(UnsupportedPath):
            spawn_shape(0xBECC, PathAddress(0x8C4B))

    def test_popup_destination_tables_remain_source_data_without_reading_unreachable_suffixes(self):
        from generate_native_paths import RandomPatrolDestination, source_offset
        for table in [0x06FE73, 0x06FE83]:
            with self.assertRaisesRegex(UnsupportedPath, 'unreviewed constant-word lookup window'):
                banked_word_values(self.rom, table)
            for choice in range(8):
                changed = bytearray(self.rom)
                start = source_offset(table) + choice * 2
                changed[start:start+2] = b'\x00\x80'
                unit = next(u for u in lowering_units(PathExtractor(bytes(changed)), PathAddress(0x2F1A))
                            if isinstance(u, RandomPatrolDestination))
                self.assertEqual(unit.offsets[choice][table == 0x06FE83], -32768)

    def test_rectangular_patrols_close_attachment_and_ballistic_effect_graphs(self):
        extractor = PathExtractor(self.rom)
        for address, count, checksum in [
            (0x1118, 451, 'e6cae018684014c8984e7c1338a4874daab33e4187d7215e7abf108289849898'),
            (0x111E, 451, '5cd9d1c400bd8dfb86a2cc9a8bb878a71f3dcf520201c5ac73a6d432d627b515'),
            (0x1124, 450, '93562e684b2ff93bc1d88b61b7c42848c78cc5a4f46564ff192e94b3b8a57eee'),
        ]:
            root = PathAddress(address)
            commands = graph(extractor, root)
            self.assertEqual(len(commands), count)
            self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(), checksum)
            statements = lower_graph(extractor, root, 0)[1]
            self.assertEqual(len(statements), count)
            self.assertEqual(sum('WordField::MotionDelta' in s for s in statements), 6)
            for dependency in [0x1283, 0x1290, 0x12A8, 0x12E5, 0x86B4, 0x8768, 0x44B2]:
                self.assertIn(PathAddress(dependency), {c.address for c in commands})
        for shape, index, path in [(0xF49C, 512, 0x1283), (0xCEE0, 167, 0x12E5)]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, 'ObjectKind::Effect'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))

    def test_motion_delta_word_and_byte_operands_share_all_three_typed_components(self):
        for encoded, axis in [(0x80, 'X'), (0x82, 'Y'), (0x84, 'Z')]:
            expected = f'WordField::MotionDelta(Axis::{axis})'
            self.assertEqual(word_field(encoded), expected)
            for offset, part in [(0, 'Low'), (1, 'High')]:
                self.assertEqual(byte_field(encoded + offset), f'ByteField::WordPart {{ field: {expected}, part: BytePart::{part} }}')

    def test_fighter_emitters_close_children_and_fold_only_complete_position_helpers(self):
        extractor = PathExtractor(self.rom)
        for address, count, checksum in [
            (0x4C68, 349, 'f95c8abc7a272569781a9584cb6cedeef01c843edeb48f153e947a580fce1e29'),
            (0x4C6A, 348, '04fccc0b98a56cd75b395d1be94f448428b0be63bf8fe91846e8aadbbcefb575'),
        ]:
            root = PathAddress(address)
            commands = graph(extractor, root)
            self.assertEqual(len(commands), count)
            self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(), checksum)
            statements = lower_graph(extractor, root, 0)[1]
            self.assertEqual(len(statements), count - 7)
            for name in ['CaptureWorldPosition', 'RestoreWorldPosition', 'FaceSelectedOffset']:
                self.assertEqual(sum(name in s for s in statements), 1)
            self.assertEqual(sum('CoordinationField::Phase' in s for s in statements), 3)
            for dependency in [0x4CCD, 0x4CF7, 0x89CA, 0x8692, 0x8C26, 0x44B2]:
                self.assertIn(PathAddress(dependency), {c.address for c in commands})
        for offset, changed_bytes, expected in [
            (0x48058, b'\x10', 'unexpected world-position transfer'),
            (0x48063, b'\x0b', 'unexpected world-position transfer'),
            (0x44CC6, b'\x57\x80', 'unported shared word'),
            (0x44CCA, b'\x61\x80', 'unported shared word'),
            (0x44CCA, b'\x57\x80', 'external entry into world-position transfer'),
        ]:
            changed = bytearray(self.rom)
            changed[offset:offset + len(changed_bytes)] = changed_bytes
            with self.assertRaisesRegex(UnsupportedPath, expected):
                lower_graph(PathExtractor(bytes(changed)), PathAddress(0x4C68), 0)
        for shape, index, path in [(0xCB98, 137, 0x4CCD), (0xCF6C, 172, 0x4CF7)]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, 'ObjectKind::Enemy'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(path + 1))

    def test_paired_patrol_graphs_include_detaching_parts_charge_launchers_and_every_drop(self):
        extractor = PathExtractor(self.rom)
        for address, count, checksum in [
            (0x3114, 372, 'e4869838b91d847ecd1f9a172c954a8ba687fddffdd4993cb486a3bea0298ba0'),
            (0x31B8, 367, '4867a5946972123f9c13b8dd2ef9f17a7f89e6d98a5bce0c32d3215b7c1ceffc'),
        ]:
            root = PathAddress(address)
            commands = graph(extractor, root)
            self.assertEqual(len(commands), count)
            self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(), checksum)
            statements = lower_graph(extractor, root, 0)[1]
            self.assertEqual(len(statements), count)
            self.assertEqual(sum('CoordinationField::RetiredActors' in s for s in statements), 3)
            for dependency in [0x32AD, 0x32B9, 0x8C36, 0x44B2, 0x44B4, 0x44B6, 0x44BA, 0x44BC]:
                self.assertIn(PathAddress(dependency), {c.address for c in commands})
        for shape, index, path in [(0xBC9C,0,0x32AD), (0xD12C,188,0x32B9),
                (0xD110,187,0x32B9), (0xBECC,20,0x8C36)]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (index, 'ObjectKind::Effect'))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(0x3114))
        for path in [0x44B2, 0x44B4, 0x44B6, 0x44BA, 0x44BC]:
            self.assertEqual(spawn_shape(0xF50C, PathAddress(path)), (516, 'ObjectKind::Effect'))

    def test_scenery_emitters_close_both_graphs_and_fold_only_the_private_surface_result(self):
        extractor = PathExtractor(self.rom)
        for address, count, checksum in [
            (0xB050, 49, '97b563ad4edfb07463fcee1a986f58253c9219b0644fee4a0abed70264cdd01b'),
            (0xB05E, 65, '34dfda2d624bc36c1b2a9b64199dfb1fceff2cf69f051b9591c12519cd45c232'),
        ]:
            root = PathAddress(address)
            commands = graph(extractor, root)
            self.assertEqual(len(commands), count)
            self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(), checksum)
            statements = lower_graph(extractor, root, 0)[1]
            self.assertEqual(len(statements), count - 1)
            self.assertEqual(sum('QuerySurfaceHeight' in s for s in statements), 1)
            self.assertEqual(sum('ImportSceneryPlacementHeight' in s for s in statements), 1)
            self.assertEqual(sum('SetSceneryPlacementHeight' in s for s in statements), 1)
            self.assertEqual(sum('MarkRemoval' in s for s in statements), 1)
        for offset, changed_bytes, expected in [
            (0x4B122, b'\xa1', 'unexpected scenery surface-height consumer'),
            (0x4B114, b'\x21\xb1', 'external entry into scenery surface-height import'),
        ]:
            changed = bytearray(self.rom)
            changed[offset:offset + len(changed_bytes)] = changed_bytes
            with self.assertRaisesRegex(UnsupportedPath, expected):
                lower_graph(PathExtractor(bytes(changed)), PathAddress(0xB05E), 0)
        self.assertIn('AttachLastSpawn', self.lower_record('7b0615')[0])
        for mask in range(256):
            self.assertIn(f'IncludeSelectedParticleFlags {{ mask: {mask}', self.lower_record(f'0016{mask:02x}')[0])
        # D767 has unrelated typed-pointer/shape consumers too; this is NOT
        # authorization for a shared numeric scratch-memory implementation.
        for record in ['fc67d70000', '7b0e0b', '7ca30800', '7b0415', '800615']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_linked_shot_count_and_retained_pair_suppression_are_typed(self):
        for opcode, command in [('32', 'Increment'), ('33', 'Decrement')]:
            self.assertIn(f'ShotCountCommand::{command}', self.lower_record(f'00 {opcode}')[0])
        self.assertIn('ImportPairSuppression', self.lower_record('79 a1 46 d7')[0])
        for record in ['7d a1 46 d7', 'fb 46 d7 01']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_rapid_shot_graphs_are_complete_with_bounded_decoded_shape_sequences(self):
        extractor = PathExtractor(self.rom)
        for root, count, checksum in [
            (0xE973, 166, '6fbb4a1118d31007dcf0e80e273d7b3e59a863a0bcaec70058626a592604dabf'),
            (0xEB1A, 150, '62add1d89f3ac58bfff290fa62e54fcfcba65df6f09d48311dc1b2e1d996d158'),
        ]:
            commands = graph(extractor, PathAddress(root))
            self.assertEqual(len(commands), count)
            self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(), checksum)
            statements = lower_graph(extractor, PathAddress(root), 0)[1]
            self.assertEqual(len(statements), count)
            self.assertEqual(sum('SelectShape' in s for s in statements), 3)
            self.assertEqual(sum('ShotCountCommand::Increment' in s for s in statements), 1)
            self.assertEqual(sum('ShotCountCommand::Decrement' in s for s in statements), 1)
        for address, expected in [
            (0x06FE93, (359,138,406,407,361,139,408,407,364,140,365,366,359,358,0,0,361,360,0,0,364,363,0,0)),
            (0x06FEC3, (364,140,365,366,26,27,28,29,364,363,0,0,26,25,0,0)),
        ]:
            self.assertEqual(rapid_shot_shapes(self.rom, address), expected)
            with self.assertRaisesRegex(UnsupportedPath, 'constant-word lookup window'):
                banked_word_values(self.rom, address)
            for group in range(0, len(expected), 4):
                self.assertEqual(len(expected[group:group + 4]), 4)
        for record in ['91 93 fe 06 26 04', '91 94 fe 06 27 04', '91 c3 fe 05 27 04']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)
        for record in ['7a a2 7c', 'fb d8 d7 01']:
            self.assertIn('ProjectileFlightOverride', self.lower_record(record)[0])

    def test_impact_material_producers_and_all_four_branch_edges(self):
        for opcode, operation in [('69', 'OrdinaryImpactMaterial'), ('6a', 'SuppressedImpactMaterial')]:
            for value in range(256):
                self.assertIn(f'ContactCommand::{operation}({value})', self.lower_record(f'00 {opcode} {value:02x}')[0])
        statements = self.lower_record('00 31 3e f5 3f f5 40 f5 0f 0f')
        self.assertEqual(statements[0],
            'Statement::ImpactBranch { first: cursor(0, 1), second: cursor(0, 2), third: cursor(0, 3), next: cursor(0, 1) }')
        for root in (0xE7DD, 0xE824):
            statements = lower_graph(PathExtractor(self.rom), PathAddress(root), 0)[1]
            self.assertTrue(any('ImportImpactMaterial' in statement for statement in statements))
        with self.assertRaisesRegex(UnsupportedPath, 'unported shared byte'):
            self.lower_record('79 a1 02 00')

    def test_aimed_impact_projectile_is_complete_through_both_cue_helpers_and_strategy_handoff(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0xF084)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 96)
        self.assertEqual(''.join(c.raw_hex for c in commands),
            '5c005ee4af0993a179a102002aa1061fe82aa1051ae82aa10415e82aa10310e8'
            '2aa1020be82aa10106e8fa711721e8fab31721e8fa801721e8fa441721e8fa65'
            '1721e8fabf1721e8fa6595a14293a179a102002aa10134e8fa9e1736e8fa23'
            '95a142005c8d7c06901d0004201b04007608007680f7ef79a24d1b2aa200a9f0'
            '063c0b280a17aef006780b280a4e94124e95144e9616f82cf161041cff080031'
            '2af119f125f152169644070af60c80ca04620a2a8a87dcf01c01084e12944e1495'
            '4e169620f7f00e00226400226400226406781702f106785432325434345436364e94'
            '124e95144e961600312af119f125f1005f2af1440f4741dde75d9cbcddaf0a0a0f'
            '474124e80f470f5812ff5814ff5816ff42')
        self.assertEqual(len(lower_graph(extractor, root, 0)[1]), 96)
        self.assertEqual(spawn_shape(0xBC9C, PathAddress(0xAFDD)), (0, 'ObjectKind::Effect'))
        self.assertIn('AttachPublishedHomingTarget', self.lower_record('7c 06 90 1d')[0])
        self.assertEqual(self.lower_record('00 5e e4 af 09'), ['Statement::InstallImpactBurst'])
        for record in ['7c a1 90 1d', '7c 06 91 1d', '00 5e e5 af 09']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_guidance_controller_entire_graph_includes_callbacks_and_delayed_reply(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x0591)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 134)
        self.assertEqual(''.join(c.raw_hex for c in commands),
            '48f7eff80c07fd4403799a701e2a9affda057b3b34683bb405553b9adf3b6c3b803b34'
            '030f7b3b367a99962a9902da05792fe21d2a2f09e1050b0599da993bda05033cfd5a07'
            'fd930019ed080ee803420b1099da993bda050314792fb51b8a2a2f08f8050b012f4a84'
            '8507fd0400192a2fff1d067b3b368a2a2f012106bc20060b10997b3b36d8993b803b36'
            '4c270642bd20061711064b84854b0006196aa3670669276706699467067a2e2a692e69'
            '067a2e98672e69060c1a00a3792ed01d672e570677a36055a39abd6206be6206dfa36c'
            'a30b03946f9442001271066ca3426ea32ba328007a06420b05997b3b36d8993b803b36'
            '4c8a06424b2e064b6a061969273f077aa99867a9490779a9841e67a94907dfa92a9aff'
            '390779275bdb2a270839072aa9104a072aa9114a070b3c277f272a6f27692749077d27'
            '841e427fa9085d9cbcb60764009c7aa1089b0b7827173f0779a95bdb2aa908d6074879'
            'a2701e033c2aa110ce070b06990799265299a2df990f6927a4856994a4857aa92a69a9'
            'a6850b3ca97fa92a0b38a952a99adfa90b03946f9442')
        self.assertEqual(len(lower_graph(extractor, root, 0)[1]), 134)
        child = PathAddress(0x07B6)
        self.assertTrue({c.address for c in graph(extractor, child)} <= {c.address for c in commands})
        self.assertEqual(spawn_shape(0xBC9C, child), (0, 'ObjectKind::Effect'))
        spawn = extractor.decode_command(PathAddress(0x074D))
        self.assertEqual(independent_spawn_parameters(spawn).path, child)
        self.assertIn(spawn, commands)

    def test_radio_event_low_byte_spawn_parameter_and_map_region_are_exact_typed_aliases(self):
        for record, operation in [('79 a9 84 1e', 'CopyTo(ByteField::Part)'),
                                  ('7d a9 84 1e', 'Assign(ByteOperand::Actor(ByteField::Part))')]:
            self.assertIn(f'RadioEventCommand::{operation}', self.lower_record(record)[0])
        for record, operation in [('7a a9 08', 'CopyTo(ByteField::Part)'),
                                  ('7f a9 08', 'Assign(ByteOperand::Actor(ByteField::Part))')]:
            self.assertIn(f'SpawnParameterCommand::{operation}', self.lower_record(record)[0])
        self.assertIn('SceneByte::MapRegion', self.lower_record('79 a9 5b db')[0])
        for record in ['79 a9 85 1e', '7d a9 5b db', '7a a9 0a', '7b a3 08', '7b a3 09']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_spawn_argument_mailboxes_have_separate_byte_only_identity(self):
        for index, address, argument in [('08', '64 d7', 'Primary'), ('09', '65 d7', 'Companion')]:
            for record, operation in [
                    (f'7a a9 {index}', 'CopyTo(ByteField::Part)'),
                    (f'7f a9 {index}', 'Assign(ByteOperand::Actor(ByteField::Part))'),
                    (f'fb {address} ff', 'Assign(ByteOperand::Literal(255))'),
                    (f'e5 {address}', 'Increment')]:
                statement = self.lower_record(record)[0]
                self.assertIn(f'SpawnArgument::{argument}', statement)
                self.assertIn(f'SpawnParameterCommand::{operation}', statement)

    def test_numbered_sprite_helpers_close_their_children_and_exact_argument_handoffs(self):
        extractor = PathExtractor(self.rom)
        for root, parent, callsite, installer, shape, index, digest in [
                (0x85C3, 0x23D4, 0x2404, 0x85CB, 0xBEB0, 19,
                 '64152accccaa12d8661b106eca343a4f61ef826d3ff58e8a9be706b1062c77e3'),
                (0x85F0, 0x6230, 0x6278, 0x85F8, 0xBEE8, 21,
                 'df7274e4b3c7b24bea9c9e845cc988580e972f4bb317f5f5c2deb490e7735b63')]:
            commands = graph(extractor, PathAddress(root))
            self.assertEqual(len(commands), 52)
            self.assertEqual(hashlib.sha256(bytes.fromhex(''.join(c.raw_hex for c in commands))).hexdigest(), digest)
            self.assertIn(PathAddress(parent), extractor.discover_roots())
            self.assertIn(extractor.decode_command(PathAddress(callsite)), graph(extractor, PathAddress(parent)))
            self.assertEqual(extractor.decode_command(PathAddress(callsite)).raw_hex, '41' + root.to_bytes(2, 'little').hex())
            spawn = child_spawn_parameters(extractor.decode_command(PathAddress(installer)))
            self.assertEqual(spawn.path, PathAddress(0x8488))
            self.assertEqual(spawn_shape(shape, spawn.path), (index, 'ObjectKind::Effect'))
            statements = lower_graph(extractor, PathAddress(root), 0)[1]
            self.assertEqual(len(statements), 52)
            self.assertEqual(sum('SpawnArgument::Companion' in s for s in statements), 2)
            self.assertEqual(sum('SpawnArgument::Primary' in s for s in statements), 2)
            self.assertEqual(sum('Statement::Placement' in s for s in statements), 2)
        for record, operation, field in [('80 a3 0b', 'Export', 'ScriptValue'),
                                          ('7b 92 0b', 'Import', 'RelativePosition(Axis::Z)')]:
            statement = self.lower_record(record)[0]
            self.assertIn('PlacementCoordinate::Primary', statement)
            self.assertIn(f'PlacementCommand::{operation}', statement)
            self.assertIn(f'WordField::{field}', statement)
        for record in ['7b a3 0b', '80 92 0b', '80 04 0b', '7b 04 0b']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_warning_and_cooldown_graphs_and_parent_installers_are_complete(self):
        extractor = PathExtractor(self.rom)
        parent = graph(extractor, PathAddress(0x22AA))
        for root, installer, count, source in [
            (0x888E, 0x8886, 21,
             '487aa12e0b03a2daa2a1cb887999701e2a99ffcb8851a39979a1d01d67a1b188'
             '77a36077a320dfa361037aa12ed8a2a17fa12e447aa12ed9a2a17fa12e0f'),
            (0x88DA, 0x88D2, 13,
             '487aa12e0b03a2daa2a1fb8861467aa12ed8a2a17fa12e447aa12ed9a2a17fa12e0f'),
        ]:
            commands = graph(extractor, PathAddress(root))
            self.assertEqual(len(commands), count)
            self.assertEqual(''.join(c.raw_hex for c in commands), source)
            self.assertEqual(len(lower_graph(extractor, PathAddress(root), 0)[1]), count)
            spawn = extractor.decode_command(PathAddress(installer))
            self.assertIn(spawn, parent)
            self.assertEqual(spawn.raw_hex, '5d9cbc' + root.to_bytes(2, 'little').hex() + '6400')
            self.assertEqual(independent_spawn_parameters(spawn).path, PathAddress(root))
            self.assertEqual(spawn_shape(0xBC9C, PathAddress(root)), (0, 'ObjectKind::Effect'))
            changed = bytearray(self.rom)
            changed[0x40000 + installer + 3:0x40000 + installer + 5] = (root + 1).to_bytes(2, 'little')
            with self.assertRaisesRegex(UnsupportedPath, 'no verified child installer'):
                generate(bytes(changed), (('WARNING', PathAddress(root)),))
        with self.assertRaises(UnsupportedPath):
            spawn_shape(0xBC9C, PathAddress(0x888F))

    def test_deferred_radio_word_and_live_scene_byte_imports_remain_distinct(self):
        for record, operation in [('7b a3 34', 'CopyTo(WordField::ScriptValue)'),
                                  ('80 a3 34', 'Assign(WordOperand::Actor(WordField::ScriptValue))')]:
            statement = self.lower_record(record)[0]
            self.assertIn(f'DeferredMessageCommand::{operation}', statement)
        for record, field in [('79 99 70 1e', 'WingmatePilot')]:
            self.assertIn(f'SceneByte::{field}', self.lower_record(record)[0])
        for record in ['7d a1 70 1e', 'fb 70 1e 00', '7b a3 35']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_objective_counts_import_and_mutate_the_same_live_record(self):
        for absolute, indexed, field in [('f4 d7', '98', 'Remaining'), ('a1 d7', '45', 'NodeRecord')]:
            for record, command in [
                (f'79 2d {absolute}', 'CopyTo(ByteField::Health)'),
                (f'7a 2d {indexed}', 'CopyTo(ByteField::Health)'),
                (f'7d 2d {absolute}', 'Assign(ByteOperand::Actor(ByteField::Health))'),
                (f'7f 2d {indexed}', 'Assign(ByteOperand::Actor(ByteField::Health))'),
                (f'fb {absolute} ff', 'Assign(ByteOperand::Literal(255))'),
                (f'e5 {absolute}', 'Increment'),
                (f'e7 {absolute}', 'Decrement'),
            ]:
                statement = self.lower_record(record)[0]
                self.assertIn('Statement::ObjectiveCounts', statement)
                self.assertIn(f'ObjectiveCountField::{field}', statement)
                self.assertIn(f'CoordinationCommand::{command}', statement)
        for record in ['79 2d f5 d7', '7b a3 98', '80 a3 45', 'e7 a2 d7', 'e5 f3 d7']:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_scene_reset_complete_graph_preserves_parameter_driven_initialization(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0x7BA0))
        self.assertEqual(len(commands), 15)
        self.assertEqual(''.join(c.raw_hex for c in commands),
            '487da1bb1b7fa12a7fa12b7fa12c7fa12d7fa12e7fa12f7fa13080a33261106da1d6a1450f')
        statements = lower_graph(extractor, PathAddress(0x7BA0), 0)[1]
        self.assertEqual(len(statements), 15)
        self.assertIn('RequestSoundBank', statements[1])
        self.assertIn('CountdownCommand::Assign', statements[2])
        for index, field in enumerate(['Progress', 'SecondaryProgress', 'CompletedParts', 'ActiveMessages', 'Handshake'], 3):
            self.assertIn(f'CoordinationField::{field}', statements[index])
            self.assertIn('CoordinationCommand::Assign(ByteOperand::Actor', statements[index])
        self.assertIn('SceneryDistance', statements[8])
        self.assertIn('PickupHistory', statements[9])
        self.assertIn('iterations: 16', statements[10])
        self.assertIn('ClearPathLatches', statements[12])
        self.assertIn('IndexedBitMask', statements[12])
        self.assertIn('End', statements[-1])

    def test_coordination_commands_decode_named_fields_and_reject_unreviewed_neighbors(self):
        for index, field in [*enumerate(['Progress', 'SecondaryProgress', 'CompletedParts', 'ActiveMessages', 'Handshake'], 0x2B), (0x31, 'RetiredActors'), (0x3E, 'Phase'), (0x79, 'TransitionReady')]:
            address = (0xD75C + index).to_bytes(2, 'little').hex(' ')
            for record, operation in [
                (f'7a a1 {index:02x}', 'CopyTo'), (f'7f a1 {index:02x}', 'Assign(ByteOperand::Actor'),
                (f'79 a1 {address}', 'CopyTo'), (f'7d a1 {address}', 'Assign(ByteOperand::Actor'),
                (f'fb {address} ff', 'Assign(ByteOperand::Literal(255))'),
                (f'e5 {address}', 'Increment'), (f'e7 {address}', 'Decrement'),
            ]:
                statement = self.lower_record(record)[0]
                self.assertIn(f'CoordinationField::{field}', statement)
                self.assertIn(f'CoordinationCommand::{operation}', statement)
        for address in [0xD785, 0xD78C]:
            with self.assertRaises(UnsupportedPath):
                self.lower_record('7d a1 ' + address.to_bytes(2, 'little').hex(' '))
        self.assertIn('RequestSoundBank', self.lower_record('7d 99 bb 1b')[0])
        self.assertIn('ClearPathLatches', self.lower_record('d6 a1')[0])
        with self.assertRaises(UnsupportedPath):
            self.lower_record('7d a1 bc 1b')

    def test_find_shape_decodes_zero_as_general_and_all_catalog_shapes_as_specific(self):
        from extract_shapes import SHAPE_HEADER_START, SHAPE_HEADER_SIZE, SHAPE_HEADER_COUNT
        for token, filter_ in [(0, "None")] + [
            (SHAPE_HEADER_START + index * SHAPE_HEADER_SIZE,
             f"Some(ShapeId::from_catalog_index({index}))")
            for index in range(SHAPE_HEADER_COUNT)
        ]:
            record = "0d " + token.to_bytes(2, "little").hex(" ")
            self.assertEqual(self.lower_record(record)[0],
                f"Statement::Relationship {{ command: RelationshipCommand::FindNearest {{ shape: {filter_} }}, next: cursor(0, 1) }}")
        for token in [1, SHAPE_HEADER_START - 1, SHAPE_HEADER_START + 1,
                      SHAPE_HEADER_START + SHAPE_HEADER_SIZE * SHAPE_HEADER_COUNT]:
            with self.assertRaises(UnsupportedPath):
                self.lower_record("0d " + token.to_bytes(2, "little").hex(" "))

    def test_nearest_shape_service_full_graph_and_reachable_installer(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0x5A02))
        self.assertEqual(len(commands), 5)
        self.assertEqual(''.join(c.raw_hex for c in commands), "0df0bc990c5a0bff2f9b0f")
        statements = lower_graph(extractor, PathAddress(0x5A02), 0)[1]
        self.assertEqual(len(statements), 5)
        self.assertIn("FindNearest { shape: Some(ShapeId::from_catalog_index(3)) }", statements[0])
        self.assertIn("LinkedOrBranch { missing: cursor(0, 4) }", statements[1])
        self.assertIn("field: ByteField::WeaponSelection", statements[2])
        self.assertIn("RestoreActor", statements[3])
        command = extractor.decode_command(PathAddress(0x5A0D))
        self.assertEqual(command.raw_hex, "5d9cbc025a0a0a")
        self.assertIn(command, graph(extractor, PathAddress(0x58B9)))
        self.assertEqual(independent_spawn_parameters(command).path, PathAddress(0x5A02))
        changed = bytearray(self.rom)
        changed[0x45A10:0x45A12] = (0x5A03).to_bytes(2, "little")
        with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
            generate(bytes(changed), (("SEARCH_SERVICE", PathAddress(0x5A02)),))

    def test_ballistic_effect_complete_arc_callbacks_and_installer(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x12E5)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 56)
        self.assertEqual(''.join(c.raw_hex for c in commands),
            "fd67004878495c00654a1311057b8e90578e548e0c61048f8e4550a38e61038f8e45578e"
            "548ea3578e7b9294579254921061048f924550a39261038f924557925492a35792000377fc"
            "06a10e0e78f7ef610d000377fc06a10e0e44780b32a1550ea11641135b42721000536213"
            "540c8e5410921a00005e13424c6613424c6813426c0e10")
        statements = lower_graph(extractor, root, 0)[1]
        self.assertEqual(len(statements), 56)
        self.assertIn("AtOrAboveSurface", statements[45])
        self.assertIn("GroundThreshold(0)", statements[48])
        self.assertEqual(statements[-1], "Statement::MarkForDeath")
        self.assertIn("WordOperation::Assign(WordOperand::Literal(0))", statements[-2])
        self.assertEqual(self.rom[0x37C77:0x37C85], bytes.fromhex("ced8e0e8eef4fc040c1218202832"))
        command = extractor.decode_command(PathAddress(0x12DA))
        self.assertEqual(command.raw_hex, "5de0cee5126404")
        self.assertIn(command, graph(extractor, PathAddress(0x1118)))
        self.assertEqual(independent_spawn_parameters(command).path, root)
        changed = bytearray(self.rom)
        changed[0x412DD:0x412DF] = (0x12E6).to_bytes(2, "little")
        with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
            generate(bytes(changed), (("BALLISTIC", root),))

    def test_height_staged_projectile_both_routes_and_shared_sprite_closure(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x6991)
        commands = graph(extractor, root)
        self.assertEqual(len(commands), 49)
        self.assertEqual(''.join(c.raw_hex for c in commands),
            "04005c0001f269fa740c0a0090610a540e90779005447ba392ef0ea3be69540e902b906400"
            "bb6977900516a769000502182d05f5b0be06f350010000000020fe01fd40006ca36ea32b"
            "a36400f16909a410e76916d76914c409ef6916d769031e100bc012f5b0be06f350010000"
            "000020fe01182d050a7ba392efa30ed26916066a7310424d001052992d0b642d4a28f30c"
            "895c1e01021621f30f4c27f342")
        self.assertEqual(len(lower_graph(extractor, root, 0)[1]), 49)
        self.assertTrue({c.address for c in graph(extractor, PathAddress(0xF306))} <=
                        {c.address for c in commands})
        command = extractor.decode_command(PathAddress(0x6918))
        self.assertEqual(command.raw_hex, "5d88c891690104")
        self.assertIn(command, graph(extractor, PathAddress(0x66EA)))
        self.assertEqual(independent_spawn_parameters(command).path, root)
        changed = bytearray(self.rom)
        changed[0x4691B:0x4691D] = (0x6992).to_bytes(2, "little")
        with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
            generate(bytes(changed), (("HEIGHT_STAGED", root),))

    def test_numbered_child_retirement_uses_literal_full_byte_and_immediate_continuation(self):
        for number in range(256):
            self.assertEqual(self.lower_record(f"66 {number:02x}")[0],
                f"Statement::Relationship {{ command: RelationshipCommand::RetireChild {{ number: {number} }}, next: cursor(0, 1) }}")

    def test_death_is_terminal_and_bouncing_part_keeps_all_commands_and_its_installer(self):
        self.assertEqual(self.lower_record("10"), ["Statement::MarkForDeath"])
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0xA481))
        self.assertEqual(len(commands), 33)
        self.assertEqual(''.join(c.raw_hex for c in commands),
            "2e0bffaf3f9aa40794081685a4e3010042fa8565e20100006596a4111e93126b12"
            "149001bea4142003b9a406ce17c0a406dd17c0a406ec95120b3ca16fa167a1e1a4"
            "71f07734041a1e00d8a416c5a4fa7657348f3416c5a410")
        self.assertEqual(len(lower_graph(extractor, PathAddress(0xA481), 0)[1]), 33)
        command = extractor.decode_command(PathAddress(0xA4F6))
        self.assertEqual(command.raw_hex, "f5f0df81a40a04d0008c0074ff01")
        self.assertIn(command, graph(extractor, PathAddress(0xA2E6)))
        self.assertEqual(child_spawn_parameters(command).path, PathAddress(0xA481))
        changed = bytearray(self.rom)
        changed[0x4A4F9:0x4A4FB] = (0xA482).to_bytes(2, "little")
        with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
            generate(bytes(changed), (("BOUNCING_PART", PathAddress(0xA481)),))

    def test_rotating_part_controller_shares_the_entire_child_graph_and_scopes_shape_kind(self):
        extractor = PathExtractor(self.rom)
        child = {c.address: c for c in graph(extractor, PathAddress(0xA481))}
        commands = graph(extractor, PathAddress(0xA4ED))
        complete = {c.address: c for c in commands}
        self.assertTrue(child.keys() <= complete.keys())
        for address, command in child.items():
            self.assertEqual(command, complete[address])
        self.assertEqual(''.join(c.raw_hex for c in commands if c.address not in child),
            "5c0bffaf2a130304a5f5f0df81a40a04d0008c0074ff013f0aa51604a5"
            "6119079404443d0183009417f1a4")
        self.assertEqual(len(commands), 45)
        statements = lower_graph(extractor, PathAddress(0xA4ED), 0)[1]
        self.assertEqual(len(statements), 45)
        self.assertEqual(spawn_shape(0xDFF0, PathAddress(0xA481)), (323, "ObjectKind::Enemy"))
        for path in [None, PathAddress(0xA482), PathAddress(0xA4ED)]:
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
                spawn_shape(0xDFF0, path)
        command = extractor.decode_command(PathAddress(0xA2FC))
        self.assertEqual(command.raw_hex, "f528e0eda40a0ad00074ffecff03")
        self.assertIn(command, graph(extractor, PathAddress(0xA2E6)))
        self.assertEqual(child_spawn_parameters(command).path, PathAddress(0xA4ED))
        changed = bytearray(self.rom)
        changed[0x4A2FF:0x4A301] = (0xA4EE).to_bytes(2, "little")
        with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
            generate(bytes(changed), (("ROTATING_CONTROLLER", PathAddress(0xA4ED)),))

    def test_actor_selection_and_restore_lower_to_immediate_typed_operations(self):
        for record, expected in [
            ("9c", "Statement::SelectActor { selection: ActorSelection::LastSpawn, next: cursor(0, 1) }"),
            ("9a", "Statement::SelectActor { selection: ActorSelection::Linked, next: cursor(0, 1) }"),
            ("99 36 f5", "Statement::SelectActor { selection: ActorSelection::LinkedOrBranch { missing: cursor(0, 0) }, next: cursor(0, 1) }"),
            ("9b", "Statement::RestoreActor { next: cursor(0, 1) }"),
        ]:
            self.assertEqual(self.lower_record(record)[0], expected)
            changed = bytearray(self.rom)
            program = bytes.fromhex(record + " 0f")
            changed[0x4F536:0x4F536 + len(program)] = program
            generated = generate(bytes(changed), (("CONTEXT", PathAddress(0xF536)),))
            self.assertEqual("use super::path_actor_context::ActorSelection;" in generated, record != "9b")

    def test_encounter_signals_decode_word_literals_and_direct_branch_edges(self):
        for mask in [0, 1, 255, 256, 0x8000, 0xABCD, 0xFFFF]:
            word = mask.to_bytes(2, "little").hex()
            for opcode, operation in [(0xE2, "Raise"), (0xE3, "Clear")]:
                self.assertEqual(self.lower_record(f"{opcode:02x} {word}")[0],
                    f"Statement::EncounterSignal {{ command: EncounterSignalCommand::{operation}({mask}), next: cursor(0, 1) }}")
            for opcode, condition in [(0xE0, "AnyRaised"), (0xE1, "AllClear")]:
                self.assertEqual(self.lower_record(f"{opcode:02x} {word} 36 f5")[0],
                    f"Statement::EncounterSignalBranch {{ condition: EncounterSignalCondition::{condition}({mask}), taken: cursor(0, 0), next: cursor(0, 1) }}")
        self.assertEqual(self.lower_record("e4")[0],
            "Statement::EncounterSignal { command: EncounterSignalCommand::Reset, next: cursor(0, 1) }")
        changed = bytearray(self.rom)
        code = bytes.fromhex("e4 e2 a5 ff e3 03 80 e0 00 80 36 f5 e1 ff ff 36 f5 0f")
        changed[0x4F536:0x4F536 + len(code)] = code
        output = generate(bytes(changed), (("SIGNALS", PathAddress(0xF536)),))
        self.assertIn("use super::path_program::EncounterSignalCommand;", output)
        self.assertIn("use super::path_program::EncounterSignalCondition;", output)

    def test_action_gate_and_group_operands_are_literals_or_typed_bytes_not_word_aliases(self):
        for value in range(256):
            self.assertEqual(self.lower_record(f"00 3c {value:02x}")[0],
                f"Statement::SetActionGate {{ value: {value}, next: cursor(0, 1) }}")
            for opcode, condition in [(0x40, "NotEqual"), (0x41, "Equal")]:
                self.assertEqual(self.lower_record(f"00 {opcode:02x} {value:02x} 36 f5")[0],
                    f"Statement::ActionGateBranch {{ condition: ActionGateCondition::{condition}({value}), taken: cursor(0, 0), next: cursor(0, 1) }}")
            self.assertIn(f"field: ByteField::SpawnGroup, operation: ByteOperation::Assign(ByteOperand::Literal({value}))",
                self.lower_record(f"0b {value:02x} af")[0])
        self.assertEqual(self.lower_record("00 3d")[0],
            "Statement::SetActionGate { value: 0, next: cursor(0, 1) }")
        self.assertEqual(self.lower_record("00 3f 36 f5")[0],
            "Statement::ActionGateBranch { condition: ActionGateCondition::Equal(0), taken: cursor(0, 0), next: cursor(0, 1) }")
        self.assertIn("ByteOperand::Actor(ByteField::SpawnGroup)", self.lower_record("4e 2d af")[0])
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand AF"):
            self.lower_record("0c 01 00 af")

    def test_numbered_actor_selection_keeps_full_literal_byte_and_decodes_field_before_switch(self):
        for number in range(256):
            self.assertEqual(self.lower_record(f"98 {number:02x} 36 f5")[0],
                f"Statement::SelectChild {{ number: ByteOperand::Literal({number}), missing: cursor(0, 0), next: cursor(0, 1) }}")
        for variable in (0x18, 0x27, 0x2D, 0x2E, 0xA1, 0xA2):
            self.assertEqual(self.lower_record(f"b7 {variable:02x} 36 f5")[0],
                f"Statement::SelectChild {{ number: ByteOperand::Actor({byte_field(variable)}), missing: cursor(0, 0), next: cursor(0, 1) }}")
        with self.assertRaisesRegex(UnsupportedPath, "unported byte operand 04"):
            self.lower_record("b7 04 36 f5")

    def test_pickup_roots_share_the_complete_collection_fallback_and_visibility_graph(self):
        extractor = PathExtractor(self.rom)
        shared = graph(extractor, PathAddress(0x44BC))
        spans = [
            (0x44B2, 0x45F5, "6da16da16da16da16da18a2aa104c744004703e24441618a4e2e146b126b146b16672efd447ba332da2ea3f94417fd446ba158a2072aa201bc442aa202ba442aa204b64417b244480ff7ef692d06450b642d8df68267a134452aa1012d452aa10326452aa10421451d0c1738451d08173845b9001d0a173845b9031d04173845b9011d0067a94d45c97927b51b8a2a27054d450b00ae411b8a5c0c0100a3f8ed4567a95f453f5f45165945792f4d1b8a2a2f006d450c28f504fd17008a2a2d64774519622d031e44622d4878495c440f69a99a458a2a2f00944597f401a04542979600a04542974600a045424ca445424b8445782aa104c5452aa105d145bb015f4567a9bb453e41df459d64000004450f41df459d6400001b0004430f41df450b08a1eb1b1ea19d01000f672eec457ba332d82ea380a332425389a357a3004bf242"),
            (0x8A1B, 0x8A34, "94397ba3920c18fc39efa3392a8a484a788a0f4a7d8a10963942"),
            (0x8A61, 0x8A70, "93a179a1b51b8a2aa1056e8ac995a142"),
            (0x8A78, 0x8A7E, "495c424842"),
        ]
        for lo, hi, expected in spans:
            self.assertEqual(''.join(c.raw_hex for c in shared if lo <= c.address.offset <= hi), expected)
        roots = []
        for root, installer in [(0x44B2, 0x8CD3), (0x44B4, 0x8CFB), (0x44B6, 0x8CDD), (0x44BA, 0x8CE7), (0x44BC, 0x8CF1)]:
            roots.append((f"PICKUP_{root:X}", PathAddress(root)))
            command = extractor.decode_command(PathAddress(installer))
            self.assertEqual(command.raw_hex, f"5d0cf5{root & 255:02x}440a00")
            self.assertIn(command, graph(extractor, PathAddress(0x0F7E)))
            spawn = independent_spawn_parameters(command)
            self.assertEqual((spawn.shape, spawn.path.offset, spawn.hit_points, spawn.attack_power), (0xF50C, root, 10, 0))
            self.assertEqual(shape_index(spawn.shape), 516)
            self.assertEqual(graph(extractor, PathAddress(root)), shared)
            entry, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(statements), 140)
            self.assertEqual(shared[entry].address.offset, root)
            mapped = dict(zip((c.address.offset for c in shared), statements))
            for source, fragment in [(0x456D, "TriggerKind::Always"), (0x45A4, "ControlCommand::Cancel"),
                (0x45A0, "ForceAfterCallbacks"), (0x45B2, "already_full: cursor(0, 59)"),
                (0x45CB, "UpgradeSelectedWeapon"), (0x45D7, "AccumulateShieldRecovery"),
                (0x45E9, "PickupHistoryCommand::Assign"), (0x8A1D, "ImportPlayerPosition"),
                (0x8A32, "RestoreWord(WordField::SavedPosition(Axis::X))")]:
                self.assertIn(fragment, mapped[source])
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = bytes.fromhex("b844")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed), (roots[-1],))
        output = generate(self.rom, tuple(roots))
        self.assertIn("LOWERED_ROOT_COUNT: usize = 5;", output)
        self.assertIn("LOWERED_COMMAND_COUNT: usize = 140;", output)
        self.assertIn("LOWERED_SOURCE_COMMAND_COUNT: usize = 140;", output)
        with self.assertRaisesRegex(UnsupportedPath, "no verified source installer"):
            generate(self.rom, (("FALLTHROUGH_ONLY", PathAddress(0x44B8)),))
        with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
            spawn_shape(0xF50C, PathAddress(0x44B8))

    def test_contact_projectile_children_preserve_complete_source_graphs_and_installation(self):
        extractor = PathExtractor(self.rom)
        cases = [
            (0xF9DD, 0xF5B4, 0xF7A1, "5d70c0ddf90104", 9, "4d0000f7ef007620005c00749903063c031e6b2d19"),
            (0x4DF9, 0x4D7E, 0x4DCF, "5dece7f94d6408", 20, "9312000f95128a141027074e000f005c0664fd13000c04848cfaaf00050f194b1e4e03050f5f224e424c184e42"),
            (0x6BCD, 0x6A15, 0x6BB9, "5dece7cd6b6404", 23, "040b642d0b042e29e66b0b10a14172895212a14172895214a1005c0664faaf611e0021f60af76b17f96b000f5ffe6b440f46fd6b29778956a142"),
            (0x9E9A, 0x9492, 0x9E46, "5df4de9a9e0a00", 27, "5c4ea12e0b012d0b042e52a1a10065eb9e11320065ce9e1107002c000f90f19e09a1a25214a290f29e09a1a25212a2065a03320f5bfd0408422aa201dc9e4cef9e42c1424cef9e420f"),
        ]
        for root, parent, installer, spawn_bytes, count, source in cases:
            commands = graph(extractor, PathAddress(root))
            self.assertEqual(''.join(c.raw_hex for c in commands), source)
            self.assertEqual(len(commands), count)
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(statements), count)
            command = extractor.decode_command(PathAddress(installer))
            self.assertIn(command, graph(extractor, PathAddress(parent)))
            self.assertEqual(command.raw_hex, spawn_bytes)
            self.assertEqual(independent_spawn_parameters(command).path.offset, root)
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = (root + 1).to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed), (("CHILD", PathAddress(root)),))
        _, statements = lower_graph(extractor, PathAddress(0x9E9A), 0)
        self.assertEqual(sum("TriggerKind::TimerPenultimate" in s for s in statements), 2)
        self.assertIn("TriggerKind::NewContact", statements[17])
        self.assertIn("ContactCommand::MarkHit", statements[22])
        # Eight authored yaw/pitch pairs; the offline decoder also retains
        # all reachable trailing bytes for noncanonical actor selectors.
        self.assertEqual(self.rom[0x49EF1:0x49F01].hex(), "f0001000001000f0f0f010f0f0101010")
        for source in [0x099EF1, 0x099EF2]:
            self.assertEqual(len(banked_byte_values(self.rom, source)), 256)

    def test_remaining_self_contained_children_preserve_graphs_and_exact_installers(self):
        extractor = PathExtractor(self.rom)
        cases = [
            (0x0A0D, 0x0691, 0x0958, "5dd4bc0d0a6400", 2, "480f"),
            (0x432A, 0x419B, 0x424C, "5dd4bc2a436400", 2, "480f"),
            (0x6BCB, 0x6A15, 0x6B7D, "f584c9cb6b64009001000060f009", 2, "5c19"),
            (0x7B8F, 0x0AE7, 0x8570, "5d28bd8f7b6400", 2, "030a0f"),
            (0x99E6, 0x9492, 0x990E, "33dcc1e6990000c00a0a00000000000052", 2, "5c19"),
            (0x9CDA, 0x9492, 0x9B90, "f584deda9c0a0a0000000000005e", 2, "5c19"),
            (0xACFE, 0xAA8A, 0xAB0B, "f5ace2feac0a0a00001400b0ff0a", 2, "2e19"),
            (0x8BF7, 0x7442, 0x77FA, "33f8ebf78b00e000640000000000000008", 4, "5c0b04ae8d19"),
            (0xF9F2, 0xF5B4, 0xF765, "5d9cbcf2f91e06", 10, "f7ef0076202e005c0b05ae780c84c904067f03140f"),
            (0x5A8D, 0x58B9, 0x592E, "33dcc18d5a800000640000000cfe00000a", 12, "00295c50a3906c3b3f9b5a16955a00049b82640090ef903baa5a169e5a3e0f"),
            (0x830D, 0x546C, 0x5604, "5da8c00d830a05", 16, "4d00025cc94ea12df81f8361191e0102440f75e2550ca25299a156a156a26d9942"),
            (0x5EC4, 0x5E1D, 0x5E3C, "f514f3c45e640400000000000002", 30, "415286fd040a194ccf5e42030fc90004950004ba61140795047790104441628d0004cb0004cd0f8d2e0b642d0b042e41548d5cccf7ef4289428942"),
            (0xB07C, 0xB05E, 0xB066, "5d84ec7cb00a0a", 33, "5c4d00181d0061081e0108440f7ba3922da318fce8038ab029a0b05cc9060a0b0aa1030a6fa167a19fb00021d03091b00f5c0c18fc0e0628045d04bf77920a0a0bd012fa897871088a2a1230b4b05d04bf77920a0afa880f"),
        ]
        for root, parent, installer, spawn_bytes, count, source in cases:
            commands = graph(extractor, PathAddress(root))
            self.assertEqual(''.join(c.raw_hex for c in commands), source)
            self.assertEqual(len(commands), count)
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(statements), count)
            command = extractor.decode_command(PathAddress(installer))
            self.assertIn(command, graph(extractor, PathAddress(parent)))
            self.assertEqual(command.raw_hex, spawn_bytes)
            spawn = independent_spawn_parameters(command) if command.opcode == 0x5D else child_spawn_parameters(command)
            self.assertEqual(spawn.path.offset, root)
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = (root + 1).to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed), (("CHILD", PathAddress(root)),))
        _, statements = lower_graph(extractor, PathAddress(0x5A8D), 0)
        self.assertIn("WordField::RelativePosition(Axis::Y)", statements[2])
        self.assertIn("operation: WordOperation::Chase(WordOperand::Literal(100))", statements[7])
        self.assertIn("ActorCondition::SecondWordLess", statements[8])
        _, statements = lower_graph(extractor, PathAddress(0x5EC4), 0)
        self.assertIn("ContactCommand::ShapeFootprintSearch(true)", statements[26])
        self.assertIn("ContactCommand::ShapeFootprintSearch(false)", statements[28])

    def test_shape_dead_lowers_to_attachment_presence_not_health_or_generic_predicate(self):
        statements = self.lower_record("20 36 f5")
        self.assertEqual(statements[0], "Statement::AttachmentAbsent { taken: cursor(0, 0), next: cursor(0, 1) }")

    def test_context_children_keep_complete_shared_graphs_and_exact_installers(self):
        extractor = PathExtractor(self.rom)
        cases = [
            (0x2271, 0x2102, 0x21DA, "f59cbc712264000000a0fb000001", 60, [0x83F9], "484ac28602195d08bef98364649cc407a9069b42"),
            (0x55D1, 0x546C, 0x556D, "f59cbcd1550a0a000090ff00000a", 40, [0x8285], "484a14870203220f5d7cbd858264009c6c0ec49b42"),
            (0x8294, 0x66EA, 0x6962, "f59cbc94820a0a00000000800203", 30, [0x82E3], "c4485d7cbde38264009cc49b030f169682"),
            (0x82B6, 0x00BC, 0x03F3, "5d7cbdb6826400", 77, [0x83F4, 0x82E3], "5d08bef483010a9cc49b17e382"),
            (0x82C3, 0x2BE9, 0x2CCF, "5d7cbdc3820200", 85, [0x83F4, 0x82E3], "c47b8e907b90927b9294622d870c8e870e90871092455d08bef483640a9cc49b"),
            (0x82D9, 0x2651, 0x82A7, "5d7cbdd9826400", 76, [0x83F4, 0x82E3], "5d08bef483640a9cc49b"),
            (0x9A4B, 0x9492, 0x96DB, "f5dcc14b9a0a0a0000000000000a", 16, [], "00295c0bc8940c90019200079b61107792ed44787792f6789a0c2cdf040b00ae9b19"),
        ]
        for root, parent, installer, spawn_bytes, count, shared_roots, new_bytes in cases:
            with self.subTest(root=f"{root:04X}"):
                commands = graph(extractor, PathAddress(root))
                shared = {c.address: c for shared_root in shared_roots for c in graph(extractor, PathAddress(shared_root))}
                actual = {c.address: c for c in commands}
                self.assertTrue(shared.keys() <= actual.keys())
                for address, command in shared.items():
                    self.assertEqual(actual[address], command)
                self.assertEqual(''.join(c.raw_hex for c in commands if c.address not in shared), new_bytes)
                self.assertEqual(len(commands), count)
                self.assertEqual(len(lower_graph(extractor, PathAddress(root), 0)[1]), count)
                command = extractor.decode_command(PathAddress(installer))
                self.assertIn(command, graph(extractor, PathAddress(parent)))
                self.assertEqual(command.raw_hex, spawn_bytes)
                spawn = independent_spawn_parameters(command) if command.opcode == 0x5D else child_spawn_parameters(command)
                self.assertEqual(spawn.path.offset, root)
                changed = bytearray(self.rom)
                changed[0x40003 + installer:0x40005 + installer] = (root + 1).to_bytes(2, "little")
                with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                    generate(bytes(changed), (("CONTEXT_CHILD", PathAddress(root)),))

    def test_signal_gated_children_keep_full_graphs_and_independent_installers(self):
        extractor = PathExtractor(self.rom)
        for root, parent, installer, record, count, source in [
            (0x9DA1, 0x9492, 0x9CF1, "f5f8c1a19d0a0a0000000000002d", 23,
             "5c00297790f6033261197790f64478e00001af9d611977900a442c16fa06c49d16bb9d61147790324478e08000ca9d61147790ce4416a79d"),
            (0xAE1E, 0xAA8A, 0xAAD8, "33c8e21eae0080000a0a2200ceff000003", 22,
             "2e5c8cf9fd22000b01a2e020003aae6ba22a8a8c37ae1c01101628ae69a241ae0b01a21628ae528aa28a2c8a8fa051ae078af08a2a8a865aae00048242"),
        ]:
            commands = graph(extractor, PathAddress(root))
            self.assertEqual(''.join(c.raw_hex for c in commands), source)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(lower_graph(extractor, PathAddress(root), 0)[1]), count)
            command = extractor.decode_command(PathAddress(installer))
            self.assertEqual(command.raw_hex, record)
            self.assertIn(command, graph(extractor, PathAddress(parent)))
            self.assertEqual(child_spawn_parameters(command).path.offset, root)
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = (root + 1).to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed), (("SIGNAL_CHILD", PathAddress(root)),))

    def test_action_gate_and_spawn_group_child_graphs_keep_every_source_command(self):
        extractor = PathExtractor(self.rom)
        for root, parent, installer, record, count, source in [
            (0xCFC8, 0x4D7E, 0x4D99, "f5b8c3c8cf0102000020fe000f06", 11,
             "f8dccf004000d4cf4adecf015c00290c03008719f942fa6b42"),
            (0xD399, 0xD27B, 0xD374, "5d2cca99d30101", 15,
             "0bffaf00295cf90328000492610a1c010b44030d002917dfd3004081e7d316dfd30f"),
            (0xD3B2, 0xD27B, 0xD2A4, "5d08e8b2d30101", 25,
             "006500dc01780bffaf080ed4fe5c6ba1fd190061106d954403280004b261107734076d9444030e0f52149542290bdc2909dcfa7042fa8b42"),
        ]:
            commands = graph(extractor, PathAddress(root))
            self.assertEqual(''.join(c.raw_hex for c in commands), source)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(lower_graph(extractor, PathAddress(root), 0)[1]), count)
            command = extractor.decode_command(PathAddress(installer))
            self.assertEqual(command.raw_hex, record)
            self.assertIn(command, graph(extractor, PathAddress(parent)))
            spawn = independent_spawn_parameters(command) if command.opcode == 0x5D else child_spawn_parameters(command)
            self.assertEqual(spawn.path.offset, root)
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = (root + 1).to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed), (("ACTION_CHILD", PathAddress(root)),))

    def test_weapon_level_branch_uses_literal_byte_not_an_actor_field_or_inverted_compare(self):
        for level in range(256):
            self.assertEqual(self.lower_record(f"00 47 {level:02x} 36 f5")[0],
                f"Statement::ActiveWeaponLevelEquals {{ expected: {level}, taken: cursor(0, 0), next: cursor(0, 1) }}")
        # The three source uses compare literals 2/3, not object bytes 2/3.
        extractor = PathExtractor(self.rom)
        for address, expected in [(0x44C2, "004703e244"), (0x730F, "0047022273"), (0x7314, "0047032b73")]:
            command = extractor.decode_command(PathAddress(address))
            self.assertEqual(command.raw_hex, expected)
            self.assertEqual(command.opcode, 0x147)

    def test_independent_sprite_roots_keep_nested_loops_callbacks_and_shared_fade(self):
        extractor = PathExtractor(self.rom)
        for root, count in [(0x82E3, 22), (0x8285, 30), (0x8458, 7)]:
            commands = graph(extractor, PathAddress(root))
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(statements), count)
            mapped = dict(zip((c.address.offset for c in commands), statements))
            expected = ([(0x82E7, "TriggerKind::Always"), (0x82EC, "iterations: 3"),
                         (0x82EE, "iterations: 4"), (0x82F4, "iterations: 4"),
                         (0x82F6, "amount: 255, period: 8"), (0x8303, "Literal(236)"),
                         (0x8305, "SignedByte(ByteOperand::Actor(ByteField::AttackPower))"),
                         (0x8308, "ByteOperation::Increment"), (0x830A, "ByteOperation::Increment"),
                         (0x830C, "ControlCommand::Return")]
                if root == 0x82E3 else
                [(0x8458, "DisableCollision"), (0x845B, "WaitOne"),
                 (0x845C, "iterations: 7"), (0x8462, "ControlCommand::End")])
            if root == 0x8285:
                expected += [(0x8285, "mask: 31"), (0x842D, "part: BytePart::High"),
                             (0x8432, "ByteField::Part"), (0x843A, "UnsignedByte"),
                             (0x843F, "immediate: true"), (0x844B, "immediate: true"),
                             (0x8457, "immediate: true")]
            for address, fragment in expected:
                self.assertIn(fragment, mapped[address])
            self.assertEqual(spawn_shape(0xBD7C, PathAddress(root)), (8, "ObjectKind::Effect"))
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
                spawn_shape(0xBD7C, PathAddress(root + 1))
        for source, target, root in [(0x48724, 0x82E6, 0x82E3), (0x48717, 0x8288, 0x8285), (0x408B6, 0x8459, 0x8458)]:
            changed = bytearray(self.rom)
            changed[source:source + 2] = target.to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed), (("CHILD_UNDER_TEST", PathAddress(root)),))

    def test_independent_spawn_parameters_validate_handler_record_and_literal_widths(self):
        extractor = PathExtractor(self.rom)
        for source, target, power in [(0x8721, 0x82E3, 241), (0x8714, 0x8285, 0), (0x08B3, 0x8458, 0)]:
            spawn = independent_spawn_parameters(extractor.decode_command(PathAddress(source)))
            self.assertEqual((spawn.shape, spawn.path.offset, spawn.hit_points, spawn.attack_power),
                             (0xBD7C, target, 100, power))
        command = extractor.decode_command(PathAddress(0x8721))
        spawn = independent_spawn_parameters(replace(command, raw_hex="5d 34 12 ff ff 80 ff"))
        self.assertEqual((spawn.shape, spawn.path.offset, spawn.hit_points, spawn.attack_power),
                         (0x1234, 65535, 128, 255))
        for invalid in (replace(command, raw_hex=command.raw_hex[:-2]),
                        replace(command, raw_hex=command.raw_hex + "00"),
                        replace(command, raw_hex="f5" + command.raw_hex[2:]),
                        replace(command, prefix_size=1), replace(command, handler_address=0),
                        extractor.decode_command(PathAddress(0xF56A))):
            with self.assertRaises(UnsupportedPath):
                independent_spawn_parameters(invalid)

    def test_sound_and_shrinking_sprite_children_have_reviewed_installers_and_complete_graphs(self):
        extractor = PathExtractor(self.rom)
        cases = [(0x8394, 11, 0x334E, 0xBF04, 22), (0x838F, 12, 0x33EC, 0xBF04, 22),
                 (0x83C2, 20, 0x73F3, 0xC0FC, 40), (0x832E, 20, 0x861D, 0xC08C, 36)]
        for root, count, installer, shape, index in cases:
            commands = graph(extractor, PathAddress(root))
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(statements), count)
            mapped = dict(zip((c.address.offset for c in commands), statements))
            if root in (0x8394, 0x838F):
                self.assertIn(f"id: {137 if root == 0x8394 else 136}", mapped[root])
                self.assertIn("UnsignedByte(ByteOperand::Actor(ByteField::Health))", mapped[0x83A2])
                self.assertIn("period: 8", mapped[0x83A4])
                self.assertEqual(sum("MarkerSound" in s for s in statements), 1)
            elif root == 0x83C2:
                self.assertIn("TriggerKind::Always", mapped[0x83CB])
                self.assertIn("Literal(13)", mapped[0x83E9])
                self.assertIn("part: BytePart::High", mapped[0x83EC])
                self.assertIn("SignedByte", mapped[0x83EE])
                self.assertIn("ByteOperation::Decrement", mapped[0x83F1])
                self.assertEqual(sum("mask: 63" in s for s in statements), 3)
            else:
                self.assertIn("ByteField::Part", mapped[0x8330])
                self.assertIn("period: 2", mapped[0x8346])
                self.assertIn("InvertNext", mapped[0x834B])
                self.assertIn("Literal(4)", mapped[0x834C])
                self.assertIn("MarkerRange::Near", mapped[0x8355])
                self.assertIn("id: 150", mapped[0x835B])
            self.assertEqual(spawn_shape(shape, PathAddress(root)), (index, "ObjectKind::Effect"))
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
                spawn_shape(shape, PathAddress(root + 1))
            changed = bytearray(self.rom)
            target = commands[1].address.offset
            changed[0x40003 + installer:0x40005 + installer] = target.to_bytes(2, "little")
            # The Gunner dust and four-panel break-sprite installers belong
            # to complete roots: reject their invalid pair during parent lowering.
            error = "unreviewed native spawn kind" if root in (0x8394, 0x838F, 0x832E) else "no verified child installer"
            with self.assertRaisesRegex(UnsupportedPath, error):
                generate(bytes(changed))

    def test_fixed_count_sprite_children_preserve_initialization_and_live_phase_reads(self):
        extractor = PathExtractor(self.rom)
        for root, count, installer, shape, index in [
            (0x90FD, 8, 0x8F63, 0xC0A8, 37), (0x9277, 7, 0x926F, 0xBF04, 22),
            (0xC520, 9, 0xC344, 0xBDD0, 11),
        ]:
            commands = graph(extractor, PathAddress(root))
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(statements), count)
            mapped = dict(zip((c.address.offset for c in commands), statements))
            if root == 0x90FD:
                for address, fragment in [(0x90FE, "Shadow(false)"), (0x90FF, "size: 32"),
                    (0x9102, "Literal(600)"), (0x9106, "iterations: 8"), (0x9108, "Literal(253)")]:
                    self.assertIn(fragment, mapped[address])
                self.assertFalse(any("Statement::Animation" in s for s in statements))
            elif root == 0x9277:
                self.assertIn("size: 24", mapped[0x9278])
                self.assertIn("AnimationCommand::Initialize", mapped[0x927B])
                self.assertIn("iterations: 8", mapped[0x927D])
            else:
                self.assertIn("part: BytePart::Low", mapped[0xC520])
                self.assertIn("ByteField::AttackPower", mapped[0xC525])
                self.assertIn("iterations: 8", mapped[0xC529])
                self.assertIn("ByteOperation::Add(ByteOperand::Actor(ByteField::WordPart", mapped[0xC52E])
                self.assertFalse(any("AnimationCommand::Initialize" in s for s in statements))
            self.assertEqual(spawn_shape(shape, PathAddress(root)), (index, "ObjectKind::Effect"))
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = commands[1].address.offset.to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed), (("SPRITE_CHILD", PathAddress(root)),))
        for shape, root in [(0xC0A8, 0x90FE), (0xBF04, 0x9278)]:
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
                spawn_shape(shape, PathAddress(root))

    def test_pickup_history_word_import_export_maps_only_the_reviewed_shared_record(self):
        self.assertEqual(self.lower_record("7b a3 32")[0],
            "Statement::PickupHistory { command: super::path_program::PickupHistoryCommand::CopyTo(WordField::ScriptValue), next: cursor(0, 1) }")
        self.assertEqual(self.lower_record("80 a3 32")[0],
            "Statement::PickupHistory { command: super::path_program::PickupHistoryCommand::Assign(WordOperand::Actor(WordField::ScriptValue)), next: cursor(0, 1) }")
        for index in (0x31, 0x33, 0x35):
            for opcode in (0x7B, 0x80):
                with self.assertRaisesRegex(UnsupportedPath, "unported shared word"):
                    self.lower_record(f"{opcode:02x} a3 {index:02x}")
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0x45DF))
        _, statements = lower_graph(extractor, PathAddress(0x45DF), 0)
        self.assertEqual(len(commands), 5)
        self.assertIn("ZeroByte", statements[0])
        self.assertIn("PickupHistoryCommand::CopyTo", statements[1])
        self.assertIn("Set", statements[2])
        self.assertIn("PickupHistoryCommand::Assign", statements[3])
        self.assertIn("Return", statements[4])

    def test_motion_fade_sprites_lower_complete_setup_jitter_audio_and_snapshot_callback(self):
        extractor = PathExtractor(self.rom)
        for root, count, installer, shape, index in [
            (0x83F4, 50, 0x25FD, 0xBE08, 13), (0x83F9, 51, 0x0443, 0xBE24, 14),
            (0x8402, 49, 0x3025, 0xBE08, 13),
        ]:
            commands = graph(extractor, PathAddress(root))
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(statements), count)
            mapped = dict(zip((c.address.offset for c in commands), statements))
            for address, fragment in [
                (0x8405, "AuxiliaryModeClass::Two"), (0x8408, "AuxiliaryModeClass::Three"),
                (0x840B, "TriggerKind::Always"), (0x8417, "Literal(17)"),
                (0x841A, "ByteField::Health"), (0x841F, "SecondByteLess"),
                (0x8424, "id: 139"), (0x842A, "id: 112"),
                (0x843A, "UnsignedByte"), (0x843F, "immediate: true"),
                (0x8463, "StackValueCommand::SaveByte"), (0x847D, "StackValueCommand::RestoreByte"),
                (0x8465, "ImportPlayerMotionByte { axis: Axis::X, part: BytePart::Low"),
                (0x8471, "ImportPlayerMotionByte { axis: Axis::Z, part: BytePart::Low"),
            ]:
                self.assertIn(fragment, mapped[address])
            if root == 0x83F4:
                self.assertIn("part: BytePart::High", mapped[root])
                self.assertIn("ByteOperation::Increment", mapped[root])
            elif root == 0x83F9:
                self.assertIn("mask: 31", mapped[root])
                self.assertIn("ByteField::AttackPower", mapped[0x83FC])
                self.assertIn("ByteOperation::Add", mapped[0x83FC])
            else:
                self.assertIn("ByteField::AttackPower, mask: 15", mapped[root])
            self.assertEqual(spawn_shape(shape, PathAddress(root)), (index, "ObjectKind::Effect"))
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = (root + 1).to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed))
        with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
            spawn_shape(0xBE24, PathAddress(0x83FA))

    def test_motion_snapshot_byte_imports_decode_axis_and_half_without_exporting_memory(self):
        for address in range(0x1E1C, 0x1E22):
            axis = ("X", "Y", "Z")[(address - 0x1E1C) // 2]
            part = "High" if address & 1 else "Low"
            self.assertEqual(self.lower_record(f"79 a1 {address & 255:02x} {address >> 8:02x}")[0],
                f"Statement::ImportPlayerMotionByte {{ axis: Axis::{axis}, part: BytePart::{part}, destination: ByteField::WordPart {{ field: WordField::MotionPhase, part: BytePart::Low }}, next: cursor(0, 1) }}")
            with self.assertRaisesRegex(UnsupportedPath, "unported shared byte"):
                self.lower_record(f"7d a1 {address & 255:02x} {address >> 8:02x}")
        for address in (0x1E1B, 0x1E22):
            with self.assertRaisesRegex(UnsupportedPath, "unported shared byte"):
                self.lower_record(f"79 a1 {address & 255:02x} {address >> 8:02x}")
        extractor = PathExtractor(self.rom)
        _, statements = lower_graph(extractor, PathAddress(0x8463), 0)
        self.assertEqual(len(statements), 11)
        self.assertIn("StackValueCommand::SaveByte", statements[0])
        self.assertIn("ImportPlayerMotionByte { axis: Axis::X, part: BytePart::Low", statements[1])
        self.assertIn("ByteOperation::Add(ByteOperand::Actor", statements[2])
        self.assertIn("ByteOperation::Negate", statements[3])
        self.assertIn("SignedByte", statements[4])
        self.assertIn("ImportPlayerMotionByte { axis: Axis::Z, part: BytePart::Low", statements[5])
        self.assertIn("StackValueCommand::RestoreByte", statements[9])

    def test_hit_cycled_shape_keeps_both_callbacks_and_all_loop_transition_commands(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0x20CD))
        _, statements = lower_graph(extractor, PathAddress(0x20CD), 0)
        self.assertEqual(len(commands), 27)
        self.assertEqual(len(statements), 27)
        mapped = dict(zip((c.address.offset for c in commands), statements))
        for address, fragment in [
            (0x20CD, "RunWhenPaused { enabled: true"), (0x20CE, "SuppressContactsNextEpoch(true)"),
            (0x20CF, "Initialize { channel: AnimationChannel::Shape, value: 0 }"),
            (0x20D0, "ConsumeHitEvent"), (0x20EE, "ConsumeHitEvent"),
            (0x20D4, "ForceAfterCallbacks"), (0x20F2, "ForceAfterCallbacks"),
            (0x20D8, "Cancel"), (0x20F6, "Cancel"),
            (0x20DB, "iterations: 7"), (0x20E2, "iterations: 7"), (0x20E8, "iterations: 7"),
            (0x20E1, "WaitOne"), (0x20F9, "iterations: 8"),
            (0x20FF, "Jump { target: cursor(0, 2) }"),
        ]:
            self.assertIn(fragment, mapped[address])
        self.assertEqual(sum("ControlCommand::Hold" in s for s in statements), 2)
        self.assertEqual(sum("amount: 255, period: 8" in s for s in statements), 2)
        self.assertEqual(sum("amount: 1, period: 8" in s for s in statements), 2)
        spawn = child_spawn_parameters(extractor.decode_command(PathAddress(0x1B34)))
        self.assertEqual((spawn.shape, spawn.path.offset, spawn.hit_points, spawn.attack_power,
                          spawn.position, spawn.number), (0xD014, 0x20CD, 100, 2, (280, 0, 0), 9))
        # Publishing the path does not guess this collision-enabled shape's
        # allocation category. Its parent/spawn integration is still unported.
        with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
            spawn_shape(0xD014, PathAddress(0x20CD))
        changed = bytearray(self.rom)
        changed[0x41B37:0x41B39] = (0x20CF).to_bytes(2, "little")
        with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
            generate(bytes(changed))

    def test_mesh_effect_children_keep_local_motion_timed_callback_and_loop_initialization(self):
        extractor = PathExtractor(self.rom)
        for root, count, installer, shape, index in [
            (0x9904, 4, 0x94D4, 0xDED8, 313), (0x9A3B, 6, 0x955D, 0xC1DC, 48),
            (0xF1BD, 8, 0xF161, 0xC63C, 88), (0xF1AE, 10, 0xF16F, 0xDF10, 315),
            (0xF1C9, 8, 0xF176, 0xCA2C, 124), (0xC1FF, 5, 0xC1D3, 0xE840, 399),
            (0xFA07, 4, 0xF76C, 0xC1DC, 48),
        ]:
            commands = graph(extractor, PathAddress(root))
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(statements), count)
            mapped = dict(zip((c.address.offset for c in commands), statements))
            expected = {
                0x9904: [(0x9907, "RelativeRotation(Axis::Y)"), (0x9907, "Literal(8)")],
                0x9A3B: [(0x9A3E, "RelativePosition(Axis::Y)"), (0x9A41, "RelativePosition(Axis::X)"),
                         (0x9A41, "SignedByte(ByteOperand::Literal(236))"), (0x9A44, "RelativeRotation(Axis::Z)")],
                0xF1BD: [(0x8D54, "ShapeFootprintSearch(true)"), (0xF1C0, "SuppressContactsNextEpoch(true)")],
                0xF1AE: [(0xF1AE, "Trigger::timed"), (0xF1AE, "Always, 30)"),
                         (0xF1B4, "Rotation(Axis::X)"), (0xF1BA, "WordField::Position(Axis::Y)"),
                         (0xF1BA, "SignedByte(ByteOperand::Literal(246))")],
                0xF1C9: [(0xF1CE, "Initialize { channel: AnimationChannel::Shape, value: 0 }"),
                         (0xF1D2, "Goto { target: cursor(0, 5) }")],
                0xC1FF: [(0xC200, "iterations: 6"), (0xC202, "amount: 1, period: 6"),
                         (0xC205, "immediate: false")],
                0xFA07: [(0xFA08, "FarSortBias(true)"), (0xFA0A, "Literal(20)")],
            }[root]
            for address, fragment in expected:
                self.assertIn(fragment, mapped[address])
            if root in (0xF1BD, 0xF1AE):
                self.assertIn("RelativeRotation(Axis::Y)", mapped[0xF1C3])
                self.assertIn("Literal(254)", mapped[0xF1C3])
            self.assertEqual(spawn_shape(shape, PathAddress(root)), (index, "ObjectKind::Effect"))
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
                spawn_shape(shape, PathAddress(root + 1))
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = (root + 1).to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed), (("CHILD_UNDER_TEST", PathAddress(root)),))

    def test_random_tumbling_mesh_keeps_draw_order_world_angles_and_complete_eighty_pass_loop(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0x98E5))
        _, statements = lower_graph(extractor, PathAddress(0x98E5), 0)
        self.assertEqual(len(commands), 13)
        self.assertEqual(len(statements), 13)
        mapped = dict(zip((c.address.offset for c in commands), statements))
        for address, fragments in [
            (0x98E6, ["Rotation(Axis::Y)", "mask: 255"]),
            (0x98E9, ["Rotation(Axis::X)", "mask: 255"]),
            (0x98EC, ["BytePart::Low", "mask: 31"]),
            (0x98EF, ["BytePart::Low", "Literal(240)"]),
            (0x98F2, ["BytePart::High", "mask: 31"]),
            (0x98F5, ["BytePart::High", "Literal(240)"]),
            (0x98F8, ["SetSpeed(30)"]), (0x98FA, ["iterations: 80"]),
            (0x98FC, ["Rotation(Axis::X)", "BytePart::Low"]),
            (0x98FF, ["Rotation(Axis::Y)", "BytePart::High"]),
            (0x9902, ["immediate: false"]), (0x9903, ["ControlCommand::End"]),
        ]:
            for fragment in fragments:
                self.assertIn(fragment, mapped[address])
        spawn = independent_spawn_parameters(extractor.decode_command(PathAddress(0x9880)))
        self.assertEqual((spawn.shape, spawn.path.offset, spawn.hit_points, spawn.attack_power),
                         (0xDE68, 0x98E5, 10, 10))
        self.assertEqual(spawn_shape(spawn.shape, spawn.path), (309, "ObjectKind::Effect"))
        with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
            spawn_shape(spawn.shape, PathAddress(0x98E6))
        changed = bytearray(self.rom)
        changed[0x49883:0x49885] = (0x98E6).to_bytes(2, "little")
        with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
            generate(bytes(changed))

    def test_rolling_contact_shape_and_distinct_held_shape_graphs_are_complete(self):
        extractor = PathExtractor(self.rom)
        for root, count, installer, shape in [
            (0x9A04, 9, 0x99FC, 0xBF58), (0x77C0, 6, 0x76AD, 0xEF5C),
            (0x7FA1, 5, 0x7791, 0xE664),
        ]:
            commands = graph(extractor, PathAddress(root))
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(statements), count)
            mapped = dict(zip((c.address.offset for c in commands), statements))
            fragments = {
                0x9A04: [(0x9A04, "Collision(true)"), (0x9A05, "SuppressContactsNextEpoch(true)"),
                         (0x9A06, "SetSpeed(80)"), (0x9A08, "ShapeId::from_catalog_index(47)"),
                         (0x9A0C, "TriggerKind::Always"), (0x9A0F, "Literal(12)"),
                         (0x9A11, "ControlCommand::End"), (0x9A12, "Rotation(Axis::Z)"),
                         (0x9A12, "Literal(16)"), (0x9A14, "ControlCommand::Return")],
                0x77C0: [(0x77C0, "DisableCollision"), (0x77C1, "Shadow(false)"),
                         (0x77C2, "Literal(100)"), (0x77C5, "RetainClass(ContactClassMask"),
                         (0x77C7, "AnimationChannel::Shape, value: 0"), (0x77C8, "ControlCommand::Hold")],
                0x7FA1: [(0x7FA1, "Literal(100)"), (0x7FA4, "DisableCollision"),
                         (0x7FA5, "MaximumDrawDistance(true)"), (0x7FA6, "Shadow(false)"),
                         (0x7FA7, "ControlCommand::Hold")],
            }[root]
            for address, fragment in fragments:
                self.assertIn(fragment, mapped[address])
            command = extractor.decode_command(PathAddress(installer))
            spawn = independent_spawn_parameters(command) if root == 0x9A04 else child_spawn_parameters(command)
            self.assertEqual((spawn.shape, spawn.path.offset), (shape, root))
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
                spawn_shape(shape, PathAddress(root))
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = (root + 1).to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed))

    def test_held_effect_children_preserve_full_depth_and_table_driven_color_callbacks(self):
        extractor = PathExtractor(self.rom)
        for root, count, installer, shape, index in [
            (0x4DF1, 4, 0x4DA7, 0xBE94, 18), (0xCFD4, 4, 0x8121, 0xBC9C, 0),
            (0x5667, 12, 0x562E, 0xBC9C, 0),
        ]:
            commands = graph(extractor, PathAddress(root))
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(statements), count)
            mapped = dict(zip((c.address.offset for c in commands), statements))
            if root == 0x4DF1:
                self.assertIn("size: 32", mapped[0x4DF2])
                self.assertIn("ByteOperation::Assign(ByteOperand::Literal(64))", mapped[0x4DF5])
            elif root == 0xCFD4:
                self.assertIn("FarSortBias(true)", mapped[0xCFD5])
                self.assertIn("WordField::DepthOffset", mapped[0xCFD7])
                self.assertIn("WordOperand::Literal(3)", mapped[0xCFD7])
            else:
                self.assertIn("value: 1", mapped[0x5667])
                self.assertIn("TriggerKind::Always", mapped[0x5669])
                self.assertIn("Literal(3)", mapped[0x566C])
                self.assertIn("Visibility(true)", mapped[0x566E])
                self.assertIn("DisableCollision", mapped[0x566F])
                self.assertIn("ByteField::Animation(AnimationChannel::Color)", mapped[0x5671])
                values = banked_byte_values(self.rom, 0x06FC2A)
                self.assertEqual(len(values), 256)
                self.assertEqual(values[:7], (128, 129, 130, 131, 130, 129, 128))
                self.assertIn("values: &[" + ", ".join(map(str, values)) + "]", mapped[0x5671])
                self.assertIn("Literal(7)", mapped[0x567A])
                self.assertIn("ByteOperation::Assign(ByteOperand::Literal(0))", mapped[0x567F])
            self.assertEqual(spawn_shape(shape, PathAddress(root)), (index, "ObjectKind::Effect"))
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
                spawn_shape(shape, PathAddress(root + 1))
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = commands[1].address.offset.to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed), (("CHILD_UNDER_TEST", PathAddress(root)),))

    def test_hit_toggle_sprite_entries_keep_both_hit_callbacks_and_counted_exit(self):
        extractor = PathExtractor(self.rom)
        for root, count in [(0x8488, 37), (0x8486, 38)]:
            commands = graph(extractor, PathAddress(root))
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(statements), count)
            mapped = dict(zip((c.address.offset for c in commands), statements))
            for address, fragment in [
                (0x8488, "RunWhenPaused"), (0x848C, "ByteField::AttackPower"),
                (0x8494, "TriggerKind::ConsumeHitEvent"), (0x849A, "BeginLoop"),
                (0x84A1, "NonzeroByte(ByteOperand::Actor(ByteField::ScriptParameter))"),
                (0x84AD, "Visibility(false)"), (0x84AE, "ForceAfterCallbacks"),
                (0x84B2, "Cancel"), (0x84B5, "ControlCommand::Jump"),
                (0x84B6, "TriggerKind::ConsumeHitEvent"), (0x84BA, "Hold"),
                (0x84BB, "Visibility(true)"), (0x84BC, "DisableCollision"),
                (0x84BD, "ForceAfterCallbacks"), (0x84C1, "Cancel"),
                (0x84C4, "ControlCommand::Jump"), (0x84C7, "ByteField::Health"),
                (0x84CF, "ControlCommand::End"),
            ]:
                self.assertIn(fragment, mapped[address])
            if root == 0x8486:
                self.assertIn("ByteField::ScriptParameter", mapped[root])
                self.assertIn("ByteOperation::Increment", mapped[root])
        # The independently reachable parent spawns are installation proof,
        # not a claim that either parent graph can be lowered in full.
        for source, target in [(0x401CD, 0x8489), (0x40980, 0x8487)]:
            changed = bytearray(self.rom)
            changed[source:source + 2] = target.to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed))
        for path in [0x8486, 0x8488]:
            self.assertEqual(spawn_shape(0xBEB0, PathAddress(path)), (19, "ObjectKind::Effect"))
        with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
            spawn_shape(0xBEB0, PathAddress(0x8489))

    def test_scene_material_child_has_complete_graph_and_verified_reachable_spawn(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0x7FAA)
        commands = graph(extractor, root)
        _, statements = lower_graph(extractor, root, 0)
        self.assertEqual(len(commands), 25)
        self.assertEqual(len(statements), 25)
        mapped = dict(zip((command.address.offset for command in commands), statements))
        for offset, fragment in [
            (0x7FAA, "ControlCommand::Call"), (0x7FC3, "ProximityWarningSource(true)"),
            (0x7FCC, "ControlCommand::SuspendAndMove"), (0x81B6, "SaveByte"),
            (0x81B8, "SceneByte::PlayerConfiguration"), (0x81C2, "SceneByte::EncounterLocation"),
            (0x81D3, "from_catalog_token(33944)"), (0x81DA, "from_catalog_token(33796)"),
            (0x81DE, "RestoreByte"), (0x81E0, "ControlCommand::Return"),
        ]:
            self.assertIn(fragment, mapped[offset])
        self.assertIn("SCENE_MATERIAL_SCENERY", generate(self.rom))
        changed = bytearray(self.rom)
        changed[0x4788A:0x4788C] = (0x7FAB).to_bytes(2, "little")
        with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
            generate(bytes(changed))

    def test_growing_sprite_and_shape_filtered_scenery_have_complete_installed_graphs(self):
        extractor = PathExtractor(self.rom)
        for root, count in [(0x0059, 8), (0x7F78, 31)]:
            commands = graph(extractor, PathAddress(root))
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(statements), count)
            mapped = dict(zip((c.address.offset for c in commands), statements))
            expected = ([(0x005A, "size: 16"), (0x005D, "iterations: 15"),
                         (0x005F, "ByteOperation::Add(ByteOperand::Literal(2))"),
                         (0x0064, "ShapeId::from_catalog_index(18)"), (0x0068, "Hold")]
                if root == 0x0059 else
                [(0x7F7B, "EqualShape(ShapeId::from_catalog_index(145))"),
                 (0x7F82, "EqualShape(ShapeId::from_catalog_index(201))"),
                 (0x7F88, "ProximityWarningSource(true)"), (0x7F8D, "Hold"),
                 (0x81B8, "SceneByte::PlayerConfiguration"),
                 (0x8653, "SuppressContactsNextEpoch(true)"),
                 (0x8D54, "ShapeFootprintSearch(true)")])
            for address, fragment in expected:
                self.assertIn(fragment, mapped[address])
        for source, target in [(0x40213, 0x005A), (0x45919, 0x7F7B)]:
            changed = bytearray(self.rom)
            changed[source:source + 2] = target.to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed))
        for shape, root, index, kind in [(0xBE5C, 0x0059, 16, "Effect"), (0xD6C0, 0x7F78, 239, "Scenery")]:
            self.assertEqual(spawn_shape(shape, PathAddress(root)), (index, f"ObjectKind::{kind}"))
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
                spawn_shape(shape, PathAddress(root + 1))

    def test_scene_imports_are_full_byte_reads_and_do_not_allow_unreviewed_writes(self):
        for address, source in [(0x1DE2, "PlayerConfiguration"), (0x1BB5, "EncounterLocation"), (0x1BA9, "EntryHeading")]:
            low, high = address.to_bytes(2, "little")
            self.assertIn(f"SceneByte::{source}", self.lower_record(f"79 a1 {low:02x} {high:02x}")[0])
            with self.assertRaisesRegex(UnsupportedPath, "unported shared byte"):
                self.lower_record(f"fb {low:02x} {high:02x} 09")

    def test_shared_scene_height_addition_decodes_destination_and_rejects_other_words(self):
        for variable, field in ((0x0E, "WordField::Position(Axis::Y)"),
                (0x90, "WordField::RelativePosition(Axis::Y)"), (0xA1, "WordField::MotionPhase")):
            self.assertEqual(self.lower_record(f"ea {variable:02x} d3 d7")[0],
                f"Statement::AddSceneHeightOffset {{ destination: {field}, next: cursor(0, 1) }}")
        for address in (0xD7D2, 0xD7D4, 0x1E0F, 0xFFFF):
            raw = address.to_bytes(2, 'little').hex(' ')
            with self.assertRaisesRegex(UnsupportedPath, "unported shared word addition"):
                self.lower_record(f"ea 0e {raw}")

    def test_distance_scenery_roots_keep_both_threshold_loops_and_nested_helpers(self):
        extractor = PathExtractor(self.rom)
        for root, count in [(0x7F27, 45), (0x7F24, 46)]:
            commands = graph(extractor, PathAddress(root))
            _, statements = lower_graph(extractor, PathAddress(root), 0)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(statements), count)
            mapped = dict(zip((command.address.offset for command in commands), statements))
            for offset, fragment in [
                (0x7F2A, "LowWord(WordField::Position(Axis::Y))"),
                (0x7F3E, "SelectedDistanceLess(512)"),
                (0x7F49, "SceneryDistanceCommand::CopyTo"),
                (0x7F4C, "WordOperation::ClearBits"),
                (0x7F4F, "SceneryDistanceCommand::Assign"),
                (0x7F52, "ControlCommand::Goto"),
                (0x7F55, "BranchCommand::InvertNext"),
                (0x7F56, "SelectedDistanceLess(600)"),
                (0x7F64, "WordOperation::SetBits"),
                (0x7F6A, "ControlCommand::Goto"),
                (0x8653, "SuppressContactsNextEpoch(true)"),
                (0x8D54, "ShapeFootprintSearch(true)"),
            ]:
                self.assertIn(fragment, mapped[offset])
            if root == 0x7F24:
                self.assertIn("Rotation(Axis::Y)", statements[0])
                self.assertIn("Actor(ByteField::Health)", statements[0])

    def test_footprint_helpers_require_every_source_byte_and_the_exact_continuation(self):
        for offset, enabled in [(0x8D54, "true"), (0x8D62, "false")]:
            _, statements = lower_graph(PathExtractor(self.rom), PathAddress(offset), 0)
            self.assertEqual(statements, [
                f"Statement::Contact {{ command: ContactCommand::ShapeFootprintSearch({enabled}), next: cursor(0, 1) }}",
                "Statement::Control(ControlCommand::Return)",
            ])
            for delta in range(1, 13):
                changed = bytearray(self.rom)
                changed[0x40000 + offset + delta] ^= 1
                with self.assertRaisesRegex(ValueError, "inline signature mismatch"):
                    lower_graph(PathExtractor(bytes(changed)), PathAddress(offset), 0)

    def test_scenery_mask_only_accepts_reviewed_indexed_byte_transfers(self):
        for opcode, fragment in [("7a", "CopyTo"), ("7f", "Assign")]:
            self.assertIn(f"SceneryDistanceCommand::{fragment}", self.lower_record(f"{opcode} a1 30")[0])
        for record in ["79 a1 8c d7", "7e a1 8c d7", "fb 8c d7 09", "7b a1 30", "80 a1 30"]:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_four_pulse_emitter_and_signal_guided_projectile_complete_graphs(self):
        extractor = PathExtractor(self.rom)
        for root, count, installer, spawn_hex in [
            (0x23D0, 61, 0x23C8, "5d50f2d0230a0a"),
            (0xA86F, 27, 0xA6DF, "5da4c16fa80104"),
        ]:
            commands = graph(extractor, PathAddress(root))
            statements = lower_graph(extractor, PathAddress(root), 0)[1]
            self.assertEqual((len(commands), len(statements)), (count, count))
            self.assertEqual(extractor.decode_command(PathAddress(installer)).raw_hex, spawn_hex)
            self.assertEqual(statements.count("Statement::MarkForDeath"), 1)
            if root == 0x23D0:
                self.assertEqual("".join(c.raw_hex for c in commands if c.address.offset < 0x2400), "41928610")
                self.assertEqual("".join(c.raw_hex for c in commands if c.address.offset >= 0x8692), "61045d24bef98364329c07a906c49b4442")
                self.assertEqual(sum("SpawnIndependent" in s for s in statements), 1)
                self.assertTrue(any("iterations: 4" in s for s in statements))
            else:
                self.assertEqual("".join(c.raw_hex for c in commands), "005c0004740005050b042e0b012d063204002c4d0008fd16080306fd150014e80395a8168da84b9fa80314104c95a842e10800a8a84c95a842141027afa80a420942")
                self.assertTrue(any("SelectedDistanceLess(10000)" in s for s in statements))
                self.assertTrue(any("EncounterSignalCondition::AllClear(8)" in s for s in statements))
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = (root + 1).to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed))

    def test_complete_debris_graphs_pin_installers_death_tails_and_arc_data(self):
        extractor = PathExtractor(self.rom)
        for root, count, installer, spawn_hex, graph_hex in [
            (0x6F36, 19, 0x6F25, "5d00ce366f6400", "5c58a1ff8ea158a2ff8ea258a9ff8ea958940f07941058950f0795d0610a550ca1550ea25510a95214945212954410"),
            (0x432C, 21, 0x42B8, "33bcfa2c43000a00640488ff0000d8ff01", "6da18cc92e5cc43f3c431c011016334361028f8e8f924550328e5036926ba16114000307b300a10e145212954410"),
            (0x432E, 20, 0x42E1, "33a0fa2e4300f600640478000000d8ff03", "8cc92e5cc43f3c431c011016334361028f8e8f924550328e5036926ba16114000307b300a10e145212954410"),
            (0x32B9, 22, 0x327C, "f52cd1b93264041e003c00000001", "5cc4fd3000fd040a194cc632424beb324bc2328f8e8f8e50328e58a23f07a2e00b02a1610f00031bb300a10e145214a244101c010842"),
        ]:
            commands = graph(extractor, PathAddress(root))
            statements = lower_graph(extractor, PathAddress(root), 0)[1]
            self.assertEqual((len(commands), len(statements)), (count, count))
            self.assertEqual("".join(c.raw_hex for c in commands), graph_hex)
            self.assertEqual(extractor.decode_command(PathAddress(installer)).raw_hex, spawn_hex)
            self.assertEqual(statements.count("Statement::MarkForDeath"), 1)
            changed = bytearray(self.rom)
            changed[0x40003 + installer:0x40005 + installer] = (root + 1).to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
                generate(bytes(changed), (("DEBRIS", PathAddress(root)),))
            with self.assertRaises(UnsupportedPath):
                generate(bytes(changed))
        self.assertEqual(self.rom[0x3307:0x331B].hex(), "9cafc0cfdce7f0f7fcff01040910192431405164")
        self.assertEqual(self.rom[0x331B:0x332F].hex(), "ced8e0e8eef4f8fcfe00000204080c1218202832")

    def test_targeting_upgrade_complete_parent_and_independent_children(self):
        statements = self.lower_record("00 7a 3c f5 00 79")
        self.assertIn("TargetingUpgradeOwned { taken: cursor(0, 2), next: cursor(0, 1)", statements[0])
        self.assertIn("AcquireTargetingUpgrade { next: cursor(0, 2)", statements[1])
        extractor = PathExtractor(self.rom)
        _, statements = lower_graph(extractor, PathAddress(0x81E1), 0)
        self.assertEqual(len(statements), 5)
        self.assertIn("DisableCollision", statements[0])
        self.assertIn("Initialize { channel: AnimationChannel::Color, value: 2 }", statements[1])
        self.assertIn("WaitOne", statements[2])
        self.assertIn("Initialize { channel: AnimationChannel::Color, value: 3 }", statements[3])
        self.assertIn("Goto", statements[4])
        source = generate(self.rom)
        self.assertIn("TARGETING_UPGRADE_GLOW", source)
        self.assertIn("TARGETING_UPGRADE_PICKUP", source)
        commands = graph(extractor, PathAddress(0x787D))
        self.assertEqual(len(commands), 49)
        self.assertEqual("".join(c.raw_hex for c in commands if 0x787D <= c.address.offset <= 0x78BF),
            "007abf78f6820b642d5cf5f8ddaa7f0a0a00000000000001f50cf5e1810a0a00000000000002004b97c800ad7816a37800044500794866016602031426d6033c26d70f")
        parent_statements = lower_graph(extractor, PathAddress(0x787D), 0)[1]
        self.assertTrue(any("RetireChild { number: 1 }" in statement for statement in parent_statements))
        self.assertTrue(any("RetireChild { number: 2 }" in statement for statement in parent_statements))
        changed = bytearray(self.rom)
        changed[0x47898:0x4789A] = (0x81E2).to_bytes(2, "little")
        with self.assertRaisesRegex(UnsupportedPath, "no verified child installer"):
            generate(bytes(changed))
        for shape, path, kind in [(0xDDF8, 0x7FAA, "Scenery"), (0xF50C, 0x81E1, "Effect")]:
            self.assertEqual(spawn_shape(shape, PathAddress(path)), (shape_index(shape), f"ObjectKind::{kind}"))
            with self.assertRaises(UnsupportedPath):
                spawn_shape(shape, PathAddress(0xF5A1))

    def test_offset_argument_preparation_folds_to_one_typed_statement(self):
        for value in range(256):
            signed = value if value < 128 else value - 256
            for axis in range(3):
                offsets = [0, 0, 0]
                offsets[axis] = value
                record = f"fb b1 16 {offsets[0]:02x} fb b3 16 {offsets[1]:02x} fb b5 16 {offsets[2]:02x} 00 30"
                statements = self.lower_record(record)
                expected = [0, 0, 0]
                expected[axis] = signed
                self.assertEqual(statements, [
                    "Statement::FaceSelectedOffset { offset: super::path_steering::AimOffset "
                    f"{{ x: {expected[0]}, y: {expected[1]}, z: {expected[2]} }}, next: cursor(0, 1) }}",
                    "Statement::Control(ControlCommand::End)",
                ])
        changed = bytearray(self.rom)
        record = bytes.fromhex("fb b1 16 00 fb b3 16 00 fb b5 16 40 00 30 0f")
        changed[0x4F536:0x4F536 + len(record)] = record
        extractor = PathExtractor(bytes(changed))
        self.assertEqual(len(graph(extractor, PathAddress(0xF536))), 5)
        self.assertIsInstance(lowering_units(extractor, PathAddress(0xF536))[0], SelectedOffsetAim)
        source = generate(bytes(changed), (("OFFSET", PathAddress(0xF536)),))
        self.assertIn("LOWERED_COMMAND_COUNT: usize = 2", source)
        self.assertIn("LOWERED_SOURCE_COMMAND_COUNT: usize = 5", source)
        wrapped = bytearray(self.rom)
        for index, value in enumerate(record):
            wrapped[0x40000 + ((0xFFF8 + index) & 0xFFFF)] = value
        extractor = PathExtractor(bytes(wrapped))
        units = lowering_units(extractor, PathAddress(0xFFF8))
        self.assertEqual([unit.address.offset for unit in units], [6, 0xFFF8])
        entry, statements = lower_graph(extractor, PathAddress(0xFFF8), 0)
        self.assertEqual(entry, 1)
        self.assertEqual(len(statements), 2)
        self.assertIn("next: cursor(0, 0)", statements[1])

    def test_offset_folding_rejects_partial_sequences_or_interior_control_entries(self):
        for record in [
            "fb b1 16 00", "fb b3 16 00 fb b5 16 40 00 30",
            "fb b1 16 00 fb b5 16 40 fb b3 16 00 00 30",
            "fb b1 16 00 fb b3 16 00 fb b5 16 40 09",
            "fb b1 16 00 fb b3 16 00 fb b5 16 40 78 00 30",
            "fb b1 16 00 fb b3 16 00 fb b5 16 40 00 30 17 3a f5",
            "fb b1 16 00 fb b3 16 00 fb b5 16 40 00 30 17 42 f5",
        ]:
            with self.subTest(record=record), self.assertRaises(UnsupportedPath):
                self.lower_record(record)
        changed = bytearray(self.rom)
        record = bytes.fromhex("fb b1 16 00 fb b3 16 00 fb b5 16 40 00 30 0f")
        changed[0x4F536:0x4F536 + len(record)] = record
        with self.assertRaises(UnsupportedPath):
            lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF53A), 0)

    def test_exhaust_graph_retains_all_five_statements_and_literal_operands(self):
        entry, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xF536), 0)
        self.assertEqual(entry, 0)
        self.assertEqual(len(statements), 5)
        self.assertIn("color: 0, size: 0", statements[0])
        self.assertIn("iterations: 3", statements[1])
        self.assertIn("amount: 1, period: 2", statements[2])
        self.assertIn("immediate: false", statements[3])
        self.assertEqual(statements[4], "Statement::Control(ControlCommand::End)")

    def test_variable_stack_commands_decode_fields_without_source_operand_leakage(self):
        for opcode, command in [("94 06", "SaveAttachment"), ("96 06", "RestoreAttachment")]:
            self.assertEqual(self.lower_record(opcode)[0],
                f"Statement::StackValue {{ command: super::path_commands::StackValueCommand::{command}, next: cursor(0, 1) }}")
        for opcode in ["93 06", "95 06", "94 07", "96 07"]:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(opcode)
        for opcode, command, field in [
            ("93 a1", "SaveByte", "ByteField::WordPart { field: WordField::MotionPhase, part: BytePart::Low }"),
            ("95 2d", "RestoreByte", "ByteField::Health"),
            ("94 a3", "SaveWord", "WordField::ScriptValue"),
            ("96 0c", "RestoreWord", "WordField::Position(Axis::X)"),
        ]:
            self.assertEqual(self.lower_record(opcode)[0],
                f"Statement::StackValue {{ command: super::path_commands::StackValueCommand::{command}({field}), next: cursor(0, 1) }}")
        for opcode in ("93", "94", "95", "96"):
            with self.assertRaises(UnsupportedPath):
                self.lower_record(f"{opcode} ff")

    def test_triggered_projectile_full_parent_child_graph_and_folded_aim(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0xF48B)
        self.assertEqual(len(graph(extractor, root)), 47)
        _, statements = lower_graph(extractor, root, 0)
        self.assertEqual(len(statements), 44)
        mapped = dict(zip((unit.address.offset for unit in lowering_units(extractor, root)), statements))
        for offset, fragment in [
            (0xF48B, "DisableCollision"), (0xF48C, "ObjectKind::Projectile"),
            (0xF48C, "x: 0, y: -10, z: 0"), (0xF48C, "Angle::from_units(231)"),
            (0xF49D, "LinkPrimaryCollisionExclusion"), (0xF4CA, "LinkPrimaryCollisionExclusion"),
            (0xF4A4, "iterations: 12"), (0xF4A6, "CopyTo(ByteField::AttackPower)"),
            (0xF4B2, "PopStackPair"), (0xF4B3, "Literal(20)"),
            (0xF4B6, "WorldPosition"), (0xF4B7, "x: 0, y: 0, z: 127"),
            (0xF4C5, "HalfTowardZero"), (0xF4C7, "Rotation(Axis::Z)"),
            (0xF4CE, "AuthoredCue::new(42"), (0xF4D1, "SuppressContactsNextEpoch(true)"),
            (0xF4D5, "SetSpeed(25)"), (0xF4D7, "GenerateVelocityEachStep(true)"),
            (0xF4DB, "TriggerKind::NewContact"), (0xF4E7, "OccupiedCell"),
            (0xF4EB, "AtOrAboveSurface"), (0xF4F2, "ForceAfterCallbacks"),
            (0xF4F6, "ControlCommand::Cancel"), (0xF4F9, "Assign(ByteOperand::Literal(1))"),
            (0xF4FD, "InitializePrimaryPitchRecoil { amount: 128"),
            (0xF500, "LockToProjectile"), (0xF50B, "AuthoredCue::new(43"),
            (0xF50E, "SetSpeed(0)"), (0xF510, "ShapeId::from_catalog_index(0)"),
            (0xF514, "RefreshOwnedOrigin"), (0xF51A, "RelativeRotation(Axis::X)"),
            (0xF51D, "WordField::Velocity(Axis::Y)"),
        ]:
            self.assertIn(fragment, mapped[offset])
        for offset in (0xF4BB, 0xF4BF, 0xF4C3):
            self.assertNotIn(offset, mapped)
        self.assertEqual(spawn_shape(0xBD60, PathAddress(0xF4CA)), (7, "ObjectKind::Projectile"))
        with self.assertRaises(UnsupportedPath):
            spawn_shape(0xBD60, PathAddress(0xF536))

    def test_primary_link_and_recoil_decode_to_semantic_state_not_numeric_pointer_fields(self):
        self.assertIn("LinkPrimaryCollisionExclusion", self.lower_record("7c 1c c3 12")[0])
        for encoded, expected in [(0, 0), (128, 128), (32767, 32767), (32768, -32768), (65535, -1)]:
            self.assertIn(f"amount: {expected},", self.lower_record(f"bf {encoded & 255:02x} {encoded >> 8:02x}")[0])
        for record in ("7c 0c c3 12", "7c 1c 1f cf", "7c 1c 1c 1e", "94 1c"):
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)
        self.assertIn("CopyTo(ByteField::AttackPower)", self.lower_record("79 2e 59 1e")[0])
        self.assertIn("Assign(ByteOperand::Literal(255))", self.lower_record("fb 59 1e ff")[0])
        for record in ("7b 2e 59 1e", "7c 0c 59 1e"):
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_projectile_inline_call_signature_is_required_for_lowering(self):
        for at in range(0x4F501, 0x4F50B):
            changed = bytearray(self.rom)
            changed[at] ^= 1
            with self.assertRaises((UnsupportedPath, ValueError)):
                lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF48B), 0)

    def test_attached_effect_inline_helpers_require_complete_signatures_and_exact_continuations(self):
        for address, continuation, action in ((0xF3F0, 0xF3FB, "Settle"),
                                               (0xF45B, 0xF466, "Tumble"),
                                               (0xF46E, 0xF479, "Center")):
            # Isolate each reviewed helper at its real authored location.
            # The replacement terminator is synthetic, not a complete-root claim.
            isolated = bytearray(self.rom)
            isolated[0x40000 + continuation] = 0x0F
            entry, statements = lower_graph(PathExtractor(bytes(isolated)), PathAddress(address), 0)
            self.assertEqual(entry, 0)
            self.assertEqual(statements, [
                f"Statement::AttachedEffectMotion {{ command: super::path_steering::AttachedEffectMotion::{action}, next: cursor(0, 1) }}",
                "Statement::Control(ControlCommand::End)",
            ])
            for offset in range(address + 1, continuation):
                for bit in range(8):
                    changed = bytearray(isolated)
                    changed[0x40000 + offset] ^= 1 << bit
                    with self.assertRaises(ValueError):
                        lower_graph(PathExtractor(bytes(changed)), PathAddress(address), 0)

    def test_attached_recovery_effect_lowers_complete_parent_children_and_timed_callbacks(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0xF3AD))
        _, statements = lower_graph(extractor, PathAddress(0xF3AD), 0)
        self.assertEqual(len(commands), 66)
        self.assertEqual(len(statements), 66)
        mapped = dict(zip((c.address.offset for c in commands), statements))
        for address, text in [
            (0xF3AD, "AuthoredCue::new(55"), (0xF3B0, "catalog_index(112)"),
            (0xF3BE, "catalog_index(113)"), (0xF3D4, "ByteOperand::Literal(3)"),
            (0xF3DE, "ByteOperand::Literal(8)"), (0xF3E5, "TriggerKind::Always, 35"),
            (0xF3F0, "AttachedEffectMotion::Settle"), (0xF3FC, "TriggerKind::Always, 25"),
            (0xF404, "RequestShieldRecovery"), (0xF40A, "RequestShieldRecovery"),
            (0xF410, "RequestShieldRecovery"), (0xF416, "WordField::DepthOffset"),
            (0xF41A, "ClockBitsSet { mask: 1"), (0xF435, "ImportEnvironmentPlaneHeight"),
            (0xF441, "UseSelfRelativeFrame"), (0xF45B, "AttachedEffectMotion::Tumble"),
            (0xF46E, "AttachedEffectMotion::Center"), (0xF47C, "ImportActionGate"),
            (0xF484, "ForceAfterCallbacks"), (0xF48A, "ControlCommand::End"),
        ]:
            self.assertIn(text, mapped[address])
        for shape, path in ((0xC8DC, 0xF3D4), (0xC8F8, 0xF3DE)):
            self.assertEqual(spawn_shape(shape, PathAddress(path))[1], "ObjectKind::Effect")
            for other in (None, PathAddress(0xF3AD), PathAddress(path ^ 10)):
                with self.assertRaises(UnsupportedPath):
                    spawn_shape(shape, other)

    def test_recovery_environment_and_clock_inputs_are_reviewed_access_forms_only(self):
        for value in range(256):
            self.assertIn(f"RequestShieldRecovery {{ amount: ByteOperand::Literal({value})",
                          self.lower_record(f"fb 1b 1e {value:02x}")[0])
            self.assertEqual(self.lower_record(f"00 2d {value:02x} 36 f5")[0],
                f"Statement::ClockBitsSet {{ mask: {value}, taken: cursor(0, 0), next: cursor(0, 1) }}")
        self.assertIn("ImportActionGate", self.lower_record("79 a1 72 1d")[0])
        self.assertIn("ImportEnvironmentPlaneHeight", self.lower_record("7c a3 0f 1e")[0])
        for record in ("79 a1 1b 1e", "7c a3 1b 1e", "fb 72 1d 01", "79 a1 0f 1e", "7c a3 10 1e"):
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_sf2_trail_opcode_is_radar_metadata_not_a_particle_emitter(self):
        for value in range(256):
            self.assertEqual(self.lower_record(f"f6 {value:02x}")[0],
                f"Statement::Appearance {{ command: AppearanceCommand::RadarMarker(super::radar::RadarMarker::from_packed({value})), next: cursor(0, 1) }}")

    def test_three_homing_projectile_roots_retain_full_shared_callbacks_and_effect(self):
        for address, count in [(0xEE2D, 82), (0xEE3B, 78), (0xEE4C, 82)]:
            extractor = PathExtractor(self.rom)
            commands = graph(extractor, PathAddress(address))
            _, statements = lower_graph(extractor, PathAddress(address), 0)
            self.assertEqual(len(statements), count)
            mapped = dict(zip((command.address.offset for command in commands), statements))
            for offset, fragment in [
                (0xEE77, "GenerateVelocityEachStep(true)"),
                (0xEE87, "SpawnIndependent"), (0xEE92, "InvertNext"),
                (0xEE9D, "SelectedDistanceLess(1000)"),
                (0xEEA2, "amount: 127"), (0xEEA4, "amount: 127"), (0xEEA6, "amount: 127"),
                (0xEEAA, "ControlCommand::Goto"), (0xEEB1, "ControlCommand::Jump"),
                (0xEEBC, "TriggerKind::Periodic(TriggerPeriod::Two)"),
                (0xEEC0, "TriggerKind::PlayerCrossing"), (0xEEC4, "iterations: 40"),
                (0xEEC8, "ForceAfterCallbacks"), (0xEECC, "ControlCommand::Cancel"),
                (0xEECF, "ControlCommand::Cancel"), (0xEED2, "Literal(15)"),
                (0xEED5, "SelectedWithinYawArc(32)"), (0xEEDA, "SelectedSmooth"),
                (0xEEDC, "OccupiedCell"), (0xEEE0, "WordOperation::Increment"),
                (0xEEE3, "WordOperand::Literal(60)"), (0xEEE9, "ForceAfterCallbacks"),
                (0xEF06, "ImportSurfaceMode"), (0xEF21, "AtOrAboveSurface"),
                (0xF018, "ForceAfterCallbacks"), (0xF5A1, "color: 0, size: 16"),
            ]:
                self.assertIn(fragment, mapped[offset])
            if address == 0xEE4C:
                self.assertIn("CampaignByte::Difficulty", mapped[0xEE4F])
                self.assertIn("SelectedDistanceLess(12000)", mapped[0xEE6E])
                self.assertNotIn(0xE78A, mapped)
            else:
                self.assertIn("InheritPrimaryHorizontalMotion", mapped[0xE78A])
                self.assertNotIn(0xEE6E, mapped)

    def test_offset_guided_projectile_folds_only_argument_preparation_and_keeps_full_graph(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0xECF7)
        self.assertEqual(len(graph(extractor, root)), 93)
        units = lowering_units(extractor, root)
        _, statements = lower_graph(extractor, root, 0)
        self.assertEqual(len(statements), 87)
        mapped = dict(zip((unit.address.offset for unit in units), statements))
        for offset in (0xED76, 0xED95):
            self.assertIn("FaceSelectedOffset", mapped[offset])
            self.assertIn("x: 0, y: 0, z: 64", mapped[offset])
            for interior in (offset + 4, offset + 8, offset + 12):
                self.assertNotIn(interior, mapped)
        for offset, fragment in [
            (0xECFA, "CampaignByte::Difficulty"),
            (0xED26, "GenerateVelocityEachStep(true)"),
            (0xED27, "GenerateVelocityEachStep(true)"),
            (0xED31, "ImportSurfaceMode"),
            (0xED3B, "values: &[1, 1, 1, 1, 2, 2, 3, 3"),
            (0xEDAE, "Trigger::timed"), (0xEDAE, "Always, 13)"),
            (0xEDB4, "ControlCommand::Hold"),
            (0xEDB5, "ControlCommand::Cancel"),
            (0xEDB8, "ControlCommand::Cancel"),
            (0xEDBD, "TriggerKind::Always"),
            (0xEDCA, "Literal(3)"), (0xEDCD, "Literal(50)"),
            (0xEDD0, "lower: 224, upper: 32"),
            (0xEDD9, "OccupiedCell"), (0xEDEA, "OccupiedCell"),
            (0xEDF0, "AtOrAboveSurface"),
            (0xEDF4, "GroundThreshold(0)"),
            (0xEDFA, "WordOperand::Literal(110)"),
            (0xF018, "ForceAfterCallbacks"),
        ]:
            self.assertIn(fragment, mapped[offset])

    def test_variant_projectile_covers_full_parent_and_death_suppressed_child_graph(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0xEF2D))
        _, statements = lower_graph(extractor, PathAddress(0xEF2D), 0)
        self.assertEqual(len(statements), 81)
        mapped = dict(zip((command.address.offset for command in commands), statements))
        for address, shape in [(0xEF58, 111), (0xEF5F, 109), (0xEF66, 110)]:
            self.assertIn(f"ShapeId::from_catalog_index({shape})", mapped[address])
        for address, depth, health in [(0xEF91, -120, 1), (0xEFA2, -480, 80), (0xEFB3, -60, 1)]:
            self.assertIn("ObjectKind::Effect", mapped[address])
            self.assertIn(f"z: {depth}", mapped[address])
            self.assertIn(f"hit_points: {health}", mapped[address])
        self.assertIn("ContactCommand::MarkHit", mapped[0xF003])
        self.assertIn("SuppressDeathEffects(true)", mapped[0xF313])
        self.assertIn("TriggerKind::ZeroHealth", mapped[0xF30F])
        self.assertIn("ControlCommand::Hold", mapped[0xEFD2])
        self.assertIn("iterations: 120", mapped[0xEFC4])
        self.assertIn("Literal(30)", mapped[0xF00F])
        self.assertIn("SelectedRelativeYawBetween { lower: 206, upper: 50 }", mapped[0xEFD4])
        self.assertIn("PlaneAxis::Forward", mapped[0xEFDA])
        for offset in (0xF314, 0xF318, 0xF31D):
            changed = bytearray(self.rom)
            changed[0x40000 + offset] ^= 1
            with self.assertRaisesRegex(ValueError, "inline signature mismatch"):
                lower_graph(PathExtractor(bytes(changed)), PathAddress(0xEF2D), 0)

    def test_linked_protection_effect_retains_both_inline_exits_and_full_flicker_table(self):
        extractor = PathExtractor(self.rom)
        root = PathAddress(0xF2B9)
        _, statements = lower_graph(extractor, root, 0)
        self.assertEqual(len(statements), 23)
        mapped = dict(zip((unit.address.offset for unit in lowering_units(extractor, root)), statements))
        for offset, fragment in [
            (0xF2BB, "SelectedTransformCommand::WorldPosition"),
            (0xF2C2, "ActivityCommand::CopyTo(ByteField::TextureScrollX)"),
            (0xF2CA, "AuthoredCue::new(20, 0, PlayerTarget::Primary)"),
            (0xF2CD, "iterations: 9"), (0xF2CF, "amount: 1, period: 10"),
            (0xF2D6, "ActivityCommand::Assign(ByteOperand::Literal(0))"),
            (0xF2DA, "value: 9"),
            (0xF2E4, "ordinary_return: cursor(0, 19), flicker: cursor(0, 20)"),
            (0xF2FD, "values: &[136, 135, 134, 133, 134, 135, 134, 133"),
            (0xF303, "ByteOperation::Increment"),
        ]:
            self.assertIn(fragment, mapped[offset])
        for offset in (0xF2E6, 0xF2ED, 0xF2F4, 0xF2FB):
            changed = bytearray(self.rom)
            changed[0x40000 + offset] ^= 1
            with self.assertRaisesRegex(ValueError, "inline signature mismatch"):
                lower_graph(PathExtractor(bytes(changed)), root, 0)

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

    def test_published_player_motion_imports_and_complete_counter_motion_root(self):
        for address, axis in [(0x1E1C, "X"), (0x1E1E, "Y"), (0x1E20, "Z")]:
            self.assertEqual(self.lower_record(f"7c a3 {address & 255:02x} {address >> 8:02x}")[0],
                f"Statement::ImportPlayerMotion {{ axis: Axis::{axis}, destination: WordField::ScriptValue, next: cursor(0, 1) }}")
        for address in [0x1E1B, 0x1E1D, 0x1E1F, 0x1E21, 0xD7ED]:
            with self.assertRaisesRegex(UnsupportedPath, "unported shared word"):
                self.lower_record(f"7c a3 {address & 255:02x} {address >> 8:02x}")
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand 04"):
            self.lower_record("7c 04 1c 1e")
        _, statements = lower_graph(PathExtractor(self.rom), PathAddress(0xBE65), 0)
        self.assertEqual(len(statements), 10)
        self.assertEqual(statements[0], "Statement::DisableCollision { next: cursor(0, 1) }")
        self.assertIn("FarSortBias(true)", statements[1])
        self.assertIn("axis: Axis::X, destination: WordField::Velocity(Axis::X)", statements[2])
        self.assertIn("axis: Axis::Z, destination: WordField::Velocity(Axis::Z)", statements[3])
        self.assertIn("WordOperation::Negate", statements[4])
        self.assertIn("WordOperation::Negate", statements[5])
        self.assertIn("SelectedTransformCommand::WorldRotation", statements[6])
        self.assertIn("ByteOperation::Assign(ByteOperand::Literal(64))", statements[7])
        self.assertIn("ByteOperation::Assign(ByteOperand::Literal(128))", statements[8])
        self.assertIn("target: cursor(0, 2)", statements[9])

    def test_charge_orb_complete_root_retains_live_threshold_and_callback(self):
        self.assertEqual(self.lower_record("79 a2 d6 1d")[0],
            "Statement::ImportChargeThreshold { destination: ByteField::WordPart { field: WordField::MotionPhase, part: BytePart::High }, next: cursor(0, 1) }")
        for record in ["79 a2 d5 1d", "79 a2 d7 1d", "7d a2 d6 1d", "fb d6 1d 19", "e5 d6 1d", "e7 d6 1d"]:
            with self.assertRaisesRegex(UnsupportedPath, "unported shared byte"):
                self.lower_record(record)
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0xF04F))
        _, statements = lower_graph(extractor, PathAddress(0xF04F), 0)
        self.assertEqual(len(statements), 17)
        mapped = dict(zip((command.address.offset for command in commands), statements))
        self.assertIn("Literal(2)", mapped[0xF050])
        self.assertIn("iterations: 8", mapped[0xF05C])
        self.assertIn("ImportChargeThreshold", mapped[0xF061])
        self.assertIn("EqualByte", mapped[0xF065])
        self.assertIn("RefreshSelectedChargeAttachment", mapped[0xF078])
        self.assertIn("ControlCommand::Hold", mapped[0xF077])
        self.assertIn("ControlCommand::Return", mapped[0xF083])
        for offset in (0xF07A, 0xF080):
            changed = bytearray(self.rom)
            changed[0x40000 + offset] ^= 1
            with self.assertRaisesRegex(ValueError, "inline signature mismatch"):
                lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF04F), 0)

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

    def test_suspension_is_terminal_movement_not_path_hold(self):
        self.assertEqual(self.lower_record("00 1e"),
                         ["Statement::Control(ControlCommand::SuspendAndMove)"])
        changed = bytearray(self.rom)
        changed[0x4F536:0x4F539] = bytes.fromhex("00 1e 0f")
        extractor = PathExtractor(bytes(changed))
        command = extractor.decode_command(PathAddress(0xF536))
        self.assertEqual(command.raw_hex, "001e")
        self.assertFalse(command.successors)
        broken = replace(command, successors=[PathAddress(0xF538)])
        extractor.decode_command = lambda address: broken
        with self.assertRaisesRegex(UnsupportedPath, "SetFlag26Bit40AndHold has outgoing edges"):
            lower_graph(extractor, command.address, 0)

    def test_proximity_warning_controls_change_candidate_membership(self):
        for opcode, enabled in [("67", "true"), ("68", "false")]:
            self.assertEqual(self.lower_record(f"00 {opcode}")[0],
                f"Statement::Appearance {{ command: AppearanceCommand::ProximityWarningSource({enabled}), next: cursor(0, 1) }}")

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
        for record in ("79 a9 85 d7", "7a a9 29", "7d a9 85 d7", "7f a9 29", "fb 85 d7 ff", "e5 85 d7", "e7 85 d7"):
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
        for record in ("90 55 fb 07 86 8a", "90 55 fb 07 a2 86"):
            with self.assertRaisesRegex(UnsupportedPath, "unported byte operand 86"):
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
        # Arbitrary shape-token destinations still need reviewed sequences.
        with self.assertRaisesRegex(UnsupportedPath, "unreviewed rapid-shot shape selector A1"):
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
            lower_graph(PathExtractor(self.rom), PathAddress(0xD253), 0)
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
            (0x18, "Speed"), (0x15, "RepeatCounter"), (0x28, "FriendHealthSlot"), (0x2D, "Health"), (0x2E, "AttackPower"),
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
            ("00 17", "CarrySelectedPlayer(true)"),
            ("00 18", "CarrySelectedPlayer(false)"),
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

    def test_contact_class_masks_decode_every_bit_including_weapon_formatted_class(self):
        for mask in range(256):
            for record, operation, retain in [(f"f7 {mask:02x}", "RetainClass", True),
                                               (f"00 76 {mask:02x}", "IncludeClass", False)]:
                statement = self.lower_record(record)[0]
                self.assertIn(f"ContactCommand::{operation}(ContactClassMask", statement)
                self.assertIn(f"groups: ExclusionGroups::from_authored_class({mask & 0xf8})", statement)
                self.assertIn(f"first_strategy_visit: {str(bool(mask & 4)).lower()}", statement)
                self.assertIn(f"weapon_formatted: {str(bool(mask & 2)).lower()}", statement)
                self.assertIn(f"suppress_attack_damage: {str(bool(mask & 1)).lower()}", statement)
                self.assertIn("next: cursor(0, 1)", statement)
        for record, operation in [("2e", "SuppressContactsNextEpoch(true)"),
                                  ("2f", "SuppressContactsNextEpoch(false)"),
                                  ("00 2c", "SuppressHitMarker(true)")]:
            self.assertEqual(self.lower_record(record)[0],
                f"Statement::Contact {{ command: ContactCommand::{operation}, next: cursor(0, 1) }}")

    def test_selected_transform_copies_remain_separate_immediate_statements(self):
        for record, operation in [("ed", "WorldPosition"), ("00 44", "WorldRotation"), ("a2", "RelativeFrame")]:
            self.assertEqual(self.lower_record(record)[0],
                f"Statement::CopySelectedTransform {{ command: SelectedTransformCommand::{operation}, next: cursor(0, 1) }}")
            changed = bytearray(self.rom)
            program = bytes.fromhex(record + " 0f")
            changed[0x4F536:0x4F536 + len(program)] = program
            self.assertIn("use super::path_relationships::SelectedTransformCommand;",
                          generate(bytes(changed), (("COPY", PathAddress(0xF536)),)))

    def test_relative_reference_controls_remain_distinct_from_unlinking(self):
        for record, operation in [("b4", "ClearRelativeReference"), ("b5", "UseSelfRelativeFrame")]:
            self.assertEqual(self.lower_record(record)[0],
                f"Statement::Relationship {{ command: RelationshipCommand::{operation}, next: cursor(0, 1) }}")

    def test_selected_mode_class_branches_use_reviewed_high_nibble_classes(self):
        for opcode, class_ in [("bc", "One"), ("bd", "Two"), ("be", "Three")]:
            self.assertEqual(self.lower_record(f"{opcode} 36 f5")[0],
                f"Statement::SelectedAuxiliaryBranch {{ condition: SelectedAuxiliaryCondition::ModeClass(super::path_conditions::AuxiliaryModeClass::{class_}), taken: cursor(0, 0), next: cursor(0, 1) }}")

    def test_source_noop_and_phase_high_alias_reuse_exact_existing_semantics(self):
        self.assertEqual(self.lower_record("b8")[0],
            "Statement::Control(ControlCommand::Jump { target: cursor(0, 1) })")
        for value in range(256):
            self.assertEqual(self.lower_record(f"b9 {value:02x}")[0],
                             self.lower_record(f"0b {value:02x} a2")[0])

    def test_indexed_add_and_advance_preserves_width_and_all_literal_periods(self):
        values = banked_byte_values(self.rom, 0x00B31B)
        for opcode, destination, kind, field in [
            ("92", "a2", "Byte", byte_field(0xA2)),
            ("00 03", "0e", "SignedWord", word_field(0x0E)),
        ]:
            for period in [0, 1, 20, 255]:
                statement = self.lower_record(f"{opcode} 1b b3 00 a1 {destination} {period:02x}")[0]
                self.assertIn(f"field: super::path_fields::IndexedAddField::{kind}({field})", statement)
                self.assertIn(f"values: &[{', '.join(map(str, values))}], period: {period}", statement)
                self.assertIn(f"selector: {byte_field(0xA1)}", statement)
                self.assertIn("next: cursor(0, 1)", statement)
        # A later wrap period does not prove the FIRST index is in range.
        # The full lookup window must still be immutable source data.
        for record in ["92 36 ff 06 a2 16 0a", "00 03 00 80 7e a1 0e 14"]:
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed constant-byte lookup window"):
                self.lower_record(record)
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand 04"):
            self.lower_record("00 03 1b b3 00 a1 04 14")

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

    def test_far_sort_bias_commands_use_the_same_appearance_boundary(self):
        for record, enabled in [("00 29", "true"), ("00 2a", "false")]:
            self.assertEqual(self.lower_record(record)[0],
                f"Statement::Appearance {{ command: AppearanceCommand::FarSortBias({enabled}), next: cursor(0, 1) }}")
            changed = bytearray(self.rom)
            changed[0x4F536:0x4F539] = bytes.fromhex(record + " 0f")
            generated = generate(bytes(changed), (("FAR_SORT", PathAddress(0xF536)),))
            self.assertIn("use super::path_appearance::AppearanceCommand;", generated)

    def test_scene_clipping_command_selects_first_plane_without_a_boolean_alias(self):
        self.assertEqual(self.lower_record("c9")[0],
            "Statement::Appearance { command: AppearanceCommand::ClippingPlane(super::render::ClippingPlaneSelection::FIRST), next: cursor(0, 1) }")
        changed = bytearray(self.rom)
        changed[0x4F536:0x4F538] = bytes.fromhex("c9 0f")
        generated = generate(bytes(changed), (("CLIPPED", PathAddress(0xF536)),))
        self.assertIn("use super::path_appearance::AppearanceCommand;", generated)
        self.assertEqual(byte_field(0xAE), "ByteField::ClippingPlane")
        for value in range(256):
            self.assertIn(f"field: ByteField::ClippingPlane, operation: ByteOperation::Assign(ByteOperand::Literal({value}))",
                          self.lower_record(f"0b {value:02x} ae")[0])
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand AE"):
            word_field(0xAE)

    def test_literal_material_override_only_accepts_reviewed_table_roots(self):
        for token in (0x8174, 0x81F4, 0x82FE, 0x8404, 0x8498):
            low, high = token.to_bytes(2, "little")
            self.assertEqual(self.lower_record(f"0c {low:02x} {high:02x} 8c")[0],
                f"Statement::Appearance {{ command: AppearanceCommand::MaterialSet(super::render::MaterialSetId::from_catalog_token({token})), next: cursor(0, 1) }}")
        for token in (0, 0x8403, 0x8405, 0x8497, 0x8499, 0xFFFF):
            low, high = token.to_bytes(2, "little")
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed material table"):
                self.lower_record(f"0c {low:02x} {high:02x} 8c")
        for record in ("08 8c 01 00", "07 8c 01", "0b 01 8c"):
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand 8C"):
            word_field(0x8C)
        changed = bytearray(self.rom)
        changed[0x4F536:0x4F53B] = bytes.fromhex("0c 04 84 8c 0f")
        generated = generate(bytes(changed), (("MATERIAL", PathAddress(0xF536)),))
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
        self.assertNotIn("VARIABLE_BIT_MASKS", generate(self.rom, (("EXHAUST", PathAddress(0xF536)),)))
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

    def test_occupancy_branch_keeps_exemption_and_world_query_in_native_dispatch(self):
        self.assertEqual(self.lower_record("00 5f 36 f5")[0],
            "Statement::OccupiedCell { taken: cursor(0, 0), next: cursor(0, 1) }")

    def test_surface_branch_keeps_object_query_and_mutation_in_native_dispatch(self):
        self.assertEqual(self.lower_record("00 53 36 f5")[0],
            "Statement::AtOrAboveSurface { taken: cursor(0, 0), next: cursor(0, 1) }")

    def test_surface_mode_byte_is_read_only_and_complete_root_keeps_all_callbacks(self):
        self.assertEqual(self.lower_record("79 a1 4d 1b")[0],
            "Statement::ImportSurfaceMode { destination: ByteField::WordPart { field: WordField::MotionPhase, part: BytePart::Low }, next: cursor(0, 1) }")
        for record in ["79 a1 4c 1b", "79 a1 4e 1b", "7d a1 4d 1b", "fb 4d 1b 01", "e5 4d 1b", "e7 4d 1b"]:
            with self.assertRaisesRegex(UnsupportedPath, "unported shared byte"):
                self.lower_record(record)
        with self.assertRaisesRegex(UnsupportedPath, "unported shared word"):
            self.lower_record("7c a1 4d 1b")
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0xEEED))
        _, statements = lower_graph(extractor, PathAddress(0xEEED), 0)
        self.assertEqual(len(statements), 27)
        mapped = dict(zip((command.address.offset for command in commands), statements))
        self.assertIn("EqualShape(ShapeId::from_catalog_index(363))", mapped[0xEEF5])
        self.assertIn("ImportSurfaceMode", mapped[0xEF06])
        self.assertIn("EqualByte", mapped[0xEF0A])
        self.assertIn("GroundThreshold(0)", mapped[0xEF1C])
        self.assertIn("AtOrAboveSurface", mapped[0xEF21])
        self.assertIn("FixedPlayerImmediate", mapped[0xEF2A])
        self.assertIn("ForceAfterCallbacks", mapped[0xEF26])
        self.assertIn("ForceAfterCallbacks", mapped[0xF018])

    def test_primary_motion_surface_root_includes_shared_subroutine_sound_and_full_loop(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0xEE10))
        _, statements = lower_graph(extractor, PathAddress(0xEE10), 0)
        self.assertEqual(len(statements), 31)
        mapped = dict(zip((command.address.offset for command in commands), statements))
        self.assertIn("InheritPrimaryHorizontalMotion", mapped[0xE78A])
        self.assertIn("ByteField::Health", mapped[0xEE10])
        self.assertIn("Literal(10)", mapped[0xEE10])
        self.assertIn("ByteField::AttackPower", mapped[0xEE13])
        self.assertIn("Literal(4)", mapped[0xEE13])
        self.assertIn("Literal(40)", mapped[0xEE16])
        self.assertIn("SetSpeed(80)", mapped[0xEE19])
        self.assertIn("ShapeId::from_catalog_index(31)", mapped[0xEE1E])
        self.assertIn("color: 0, size: 5", mapped[0xEE22])
        self.assertIn("SpatialLoop::from_authored_control(12)", mapped[0xEE25])
        self.assertIn("id: 115, mode: MarkerCueMode::DistanceBands(PathSoundClass::Positioned)", mapped[0xEE28])
        self.assertIn("AtOrAboveSurface", mapped[0xEF21])

    def test_independent_spawn_keeps_literal_bytes_independent_entry_and_no_child_fields(self):
        for health, power in [(0, 255), (129, 254), (255, 0)]:
            changed = bytearray(self.rom)
            changed[0x4F536:0x4F53E] = bytes([0x5D, 0x98, 0xBD, 0x00, 0xF6, health, power, 0x0F])
            changed[0x4F600] = 0x0F
            _, statements = lower_graph(PathExtractor(bytes(changed)), PathAddress(0xF536), 0)
            self.assertEqual(len(statements), 3)
            self.assertEqual(statements[0], f"Statement::SpawnIndependent {{ kind: ObjectKind::Effect, parameters: IndependentSpawn {{ shape: ShapeId::from_catalog_index(9), path: Some(cursor(0, 2)), hit_points: {health}, attack_power: {power} }}, next: cursor(0, 1) }}")
            generated = generate(bytes(changed), (("INDEPENDENT", PathAddress(0xF536)),))
            self.assertIn("use super::path_spawn::IndependentSpawn;", generated)
            self.assertIn("use super::{ObjectKind, ShapeId};", generated)
            self.assertNotIn("ChildSpawn", generated)
        self.assertIn("path: None", self.lower_record("5d 98 bd 00 00 a1 a3")[0])
        for record in ["5d 00 00 00 00 01 01", "5d 99 bd 00 00 01 01", "5d 9c bc 00 00 01 01"]:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_sprite_shape_metadata_requires_the_reviewed_transient_path(self):
        self.assertEqual(spawn_shape(0xBEB0, PathAddress(0xF5A1)), (19, "ObjectKind::Effect"))
        self.assertEqual(spawn_shape(0xBEB0, PathAddress(0xF306)), (19, "ObjectKind::Effect"))
        self.assertEqual(spawn_shape(0xBEB0, PathAddress(0xF32F)), (19, "ObjectKind::Effect"))
        for path in [None, PathAddress(0), PathAddress(0x8489), PathAddress(0xF330)]:
            with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
                spawn_shape(0xBEB0, path)
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0xEC98))
        _, statements = lower_graph(extractor, PathAddress(0xEC98), 0)
        self.assertEqual(len(statements), 42)
        mapped = dict(zip((command.address.offset for command in commands), statements))
        self.assertIn("ImportCampaignByte", mapped[0xEC98])
        self.assertIn("ObjectKind::Effect", mapped[0xECC1])
        self.assertIn("ShapeId::from_catalog_index(19)", mapped[0xECC1])
        self.assertIn("ByteOperand::Literal(253)", mapped[0xECD0])
        self.assertIn("QuadrupleVelocity(true)", mapped[0xECD2])
        self.assertIn("ByteOperand::Literal(3)", mapped[0xECD7])
        self.assertIn("AppearanceCommand::Collision(true)", mapped[0xECD9])
        self.assertIn("ByteOperand::Literal(50)", mapped[0xECE1])
        self.assertIn("OccupiedCell", mapped[0xECE4])
        self.assertIn("GroundThreshold(0)", mapped[0xECE8])
        self.assertIn("AtOrAboveSurface", mapped[0xECED])
        self.assertIn("color: 0, size: 16", mapped[0xF5A1])
        self.assertIn("DisableCollision", mapped[0xF5A4])
        self.assertIn("iterations: 3", mapped[0xF5A5])
        self.assertIn("channel: AnimationChannel::Color, amount: 1, period: 2", mapped[0xF5A7])

    def test_spawn_null_path_is_absent_and_unknown_shapes_or_native_kinds_are_rejected(self):
        self.assertIn("path: None", self.lower_record("f5 98 bd 00 00 01 01 00 00 00 00 00 00 00")[0])
        for shape in [0, 0xBD99, 0xFBB8, 0xFFFF]:
            with self.assertRaisesRegex(UnsupportedPath, "not a catalog header"):
                spawn_shape(shape)
        with self.assertRaisesRegex(UnsupportedPath, "unreviewed native spawn kind"):
            spawn_shape(0xBC9C)

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

    def test_child_signals_and_missing_branch_preserve_full_number_and_edges(self):
        for opcode in (0x3C, 0x3E):
            self.assertEqual(self.lower_record(f"{opcode:02x}")[0],
                "Statement::Relationship { command: RelationshipCommand::SignalLinked, next: cursor(0, 1) }")
        for number in range(256):
            self.assertEqual(self.lower_record(f"3d {number:02x}")[0],
                f"Statement::Relationship {{ command: RelationshipCommand::SignalChild {{ number: {number} }}, next: cursor(0, 1) }}")
            self.assertEqual(self.lower_record(f"3a {number:02x} 36 f5")[0],
                f"Statement::ChildMissing {{ number: {number}, taken: cursor(0, 0), next: cursor(0, 1) }}")

    def test_primary_target_update_uses_the_full_native_selection_service(self):
        self.assertEqual(self.lower_record("c7")[0],
            "Statement::ConsiderPrimaryTarget { next: cursor(0, 1) }")
        # C6 preserves request mode and marks a distinct scene-owned proxy.
        self.assertEqual(self.lower_record("c6")[0],
            "Statement::ConsiderPrimaryTargetAndMarkSceneProxy { next: cursor(0, 1) }")

    def test_selected_auxiliary_clear_action_gate_retains_both_branch_edges(self):
        self.assertEqual(self.lower_record("00 60 36 f5")[0],
            "Statement::SelectedAuxiliaryBranch { condition: SelectedAuxiliaryCondition::ActionBit04Clear, taken: cursor(0, 0), next: cursor(0, 1) }")

    def test_radio_request_literals_and_live_byte_fields_are_not_interchangeable(self):
        for number in range(256):
            self.assertEqual(self.lower_record(f"26 {number:02x}")[0],
                f"Statement::Message {{ number: ByteOperand::Literal({number}), next: cursor(0, 1) }}")
        self.assertEqual(self.lower_record("df a3")[0],
            f"Statement::Message {{ number: ByteOperand::Actor({byte_field(0xA3)}), next: cursor(0, 1) }}")
        with self.assertRaisesRegex(UnsupportedPath, "unported byte operand"):
            self.lower_record("df ff")

    def test_campaign_imports_are_read_only_byte_views_of_named_state(self):
        for record, source in [("79 a1 f2 d7", "Difficulty"), ("7a a1 96", "Difficulty"), ("79 a1 06 1c", "EncounterVariant")]:
            self.assertEqual(self.lower_record(record)[0],
                f"Statement::ImportCampaignByte {{ source: CampaignByte::{source}, destination: {byte_field(0xA1)}, next: cursor(0, 1) }}")
        for record in ["7d a1 f2 d7", "7f a1 96", "7d a1 06 1c", "fb 06 1c 04", "e5 f2 d7", "7b a3 96", "7c a3 06 1c"]:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_encounter_radio_graph_retains_both_difficulty_branches_and_timed_message_loop(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0x7BC5))
        _, statements = lower_graph(extractor, PathAddress(0x7BC5), 0)
        self.assertEqual(len(statements), 28)
        mapped = dict(zip((c.address.offset for c in commands), statements))
        self.assertIn("duration: ByteOperand::Literal(10)", mapped[0x7BC6])
        for offset in [0x7BC8, 0x7BDE, 0x7BED]:
            self.assertIn("CampaignByte::EncounterVariant", mapped[offset])
        self.assertIn("CampaignByte::Difficulty", mapped[0x7BD1])
        for offset, number in [(0x7BE7, 119), (0x7BEA, 117), (0x7BF6, 85), (0x7BF9, 120), (0x7BFC, 112)]:
            self.assertIn(f"number: ByteOperand::Literal({number})", mapped[offset])
        self.assertIn("ByteOperand::Literal(23)", mapped[0x7BFF])
        self.assertIn("iterations: 3", mapped[0x7C02])
        self.assertIn("Statement::Message { number: ByteOperand::Actor", mapped[0x7C04])
        self.assertIn("ByteOperation::Increment", mapped[0x7C06])
        self.assertIn("duration: ByteOperand::Literal(30)", mapped[0x7C08])
        self.assertIn("immediate: false", mapped[0x7C0A])
        self.assertIn("use super::path_program::CampaignByte;", generate(self.rom, (("RADIO", PathAddress(0x7BC5)),)))

    def test_word_swaps_decode_both_fields_without_exposing_unmapped_storage(self):
        fields = [0x0C, 0x0E, 0x10, 0x32, 0x34, 0x36, 0x8E, 0x90, 0x92, 0xA1, 0xA2, 0xA3]
        for first in fields:
            for second in fields:
                self.assertEqual(self.lower_record(f"00 78 {first:02x} {second:02x}")[0],
                    f"Statement::Mutate {{ mutation: Mutation::SwapWords {{ first: {word_field(first)}, second: {word_field(second)} }}, next: cursor(0, 1) }}")
        for invalid in [0x04, 0x0D, 0x2B, 0xA4]:
            for record in [f"00 78 {invalid:02x} a3", f"00 78 a3 {invalid:02x}"]:
                with self.assertRaisesRegex(UnsupportedPath, "unported word operand"):
                    self.lower_record(record)

    def test_published_position_imports_use_equivalent_absolute_and_indexed_coordinates(self):
        for index, address, axis in [(0x90, 0xD7EC, "X"), (0x92, 0xD7EE, "Y"), (0x94, 0xD7F0, "Z")]:
            expected = f"Statement::ImportPlayerPosition {{ axis: Axis::{axis}, destination: WordField::ScriptValue, next: cursor(0, 1) }}"
            self.assertEqual(self.lower_record(f"7b a3 {index:02x}")[0], expected)
            self.assertEqual(self.lower_record(f"7c a3 {address & 255:02x} {address >> 8:02x}")[0], expected)
            for record in [f"80 a3 {index:02x}", f"7e a3 {address & 255:02x} {address >> 8:02x}"]:
                with self.assertRaises(UnsupportedPath): self.lower_record(record)

    def test_saved_world_position_and_texture_y_operands_alias_existing_native_fields(self):
        for base, axis in [(0x39, "X"), (0x3B, "Y"), (0x3D, "Z")]:
            field = f"WordField::SavedPosition(Axis::{axis})"
            self.assertEqual(word_field(base), field)
            for part, offset in [("Low", 0), ("High", 1)]:
                self.assertEqual(byte_field(base + offset), f"ByteField::WordPart {{ field: {field}, part: BytePart::{part} }}")
            self.assertIn(f"StackValueCommand::SaveWord({field})", self.lower_record(f"94 {base:02x}")[0])
            self.assertIn(f"StackValueCommand::RestoreWord({field})", self.lower_record(f"96 {base:02x}")[0])
        self.assertEqual(byte_field(0x9A), "ByteField::TextureScrollY")
        for value in range(256):
            self.assertIn(f"field: ByteField::TextureScrollY, operation: ByteOperation::Assign(ByteOperand::Literal({value}))",
                self.lower_record(f"0b {value:02x} 9a")[0])
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand"):
            word_field(0x9A)

    def test_shield_pickup_adds_to_the_existing_recovery_request_with_a_byte_operand(self):
        self.assertEqual(self.lower_record("eb 1b 1e a1")[0],
            "Statement::AccumulateShieldRecovery { amount: ByteOperand::Actor(ByteField::WordPart { field: WordField::MotionPhase, part: BytePart::Low }), next: cursor(0, 1) }")
        for address in [0x1E1A, 0x1E1C, 0xD78C, 0xFFFF]:
            with self.assertRaisesRegex(UnsupportedPath, "unported shared byte addition"):
                self.lower_record(f"eb {address & 255:02x} {address >> 8:02x} a1")
        with self.assertRaisesRegex(UnsupportedPath, "unported byte operand"):
            self.lower_record("eb 1b 1e ff")
        self.assertEqual(PathExtractor(self.rom).decode_command(PathAddress(0x45D7)).raw_hex, "eb1b1ea1")

    def test_selected_score_award_is_a_literal_unsigned_word(self):
        for value in [0, 1, 100, 255, 256, 32767, 32768, 65534, 65535]:
            self.assertEqual(self.lower_record(f"9d {value & 255:02x} {value >> 8:02x}")[0],
                f"Statement::AwardSelectedScore {{ points: {value}, next: cursor(0, 1) }}")
        self.assertEqual(PathExtractor(self.rom).decode_command(PathAddress(0x45BE)).raw_hex, "9d6400")

    def test_selected_equipment_collection_and_upgrade_preserve_branch_edges_and_widths(self):
        for amount in range(256):
            for target, expected in [("36 f5", 0), ("3a f5", 1)]:
                self.assertEqual(self.lower_record(f"bb {amount:02x} {target}")[0],
                    f"Statement::CollectSelectedConsumables {{ amount: {amount}, already_full: cursor(0, {expected}), next: cursor(0, 1) }}")
        self.assertEqual(self.lower_record("00 1b")[0],
            "Statement::UpgradeSelectedWeapon { next: cursor(0, 1) }")
        extractor = PathExtractor(self.rom)
        for address, expected in [(0x45B2, "bb015f45"), (0x45CB, "001b")]:
            self.assertEqual(extractor.decode_command(PathAddress(address)).raw_hex, expected)

    def test_selected_auxiliary_updates_are_distinct_from_other_auxiliary_storage(self):
        for opcode, operation in [(0x6E, "SetModeLowNibbleOne"), (0x6F, "SetModeLowNibbleFour"), (0x62, "ClearActionBit01")]:
            self.assertEqual(self.lower_record(f"00 {opcode:02x}")[0],
                f"Statement::SelectedAuxiliary {{ command: SelectedAuxiliaryCommand::{operation}, next: cursor(0, 1) }}")
        # The similarly named OR operation targets a different field.
        self.assertEqual(self.lower_record("00 16 ff")[0],
            "Statement::IncludeSelectedParticleFlags { mask: 255, next: cursor(0, 1) }")
        changed = bytearray(self.rom)
        changed[0x4F536:0x4F53D] = bytes.fromhex("00 6e 00 6f 00 62 0f")
        generated = generate(bytes(changed), (("AUXILIARY", PathAddress(0xF536)),))
        self.assertIn("use super::path_program::SelectedAuxiliaryCommand;", generated)

    def test_random_branch_retains_both_edges_including_coincident_successors(self):
        self.assertEqual(self.lower_record("29 36 f5")[0],
            "Statement::RandomBranch { taken: cursor(0, 0), next: cursor(0, 1) }")
        self.assertEqual(self.lower_record("29 39 f5")[0],
            "Statement::RandomBranch { taken: cursor(0, 1), next: cursor(0, 1) }")

    def test_orbit_angles_and_radial_literals_retain_their_distinct_widths_and_centers(self):
        for value in range(256):
            for opcode, center in [("a5", "LocalOrigin"), ("a8", "Selected")]:
                self.assertEqual(self.lower_record(f"{opcode} {value:02x}")[0],
                    f"Statement::YawOrbit {{ center: OrbitCenter::{center}, angle: ByteOperand::Literal({value}), next: cursor(0, 1) }}")
            for opcode, center in [("ab", "LocalOrigin"), ("ae", "Selected"), ("00 22", "Linked")]:
                signed = value if value < 128 else value - 256
                self.assertEqual(self.lower_record(f"{opcode} {value:02x}")[0],
                    f"Statement::Radius {{ command: RadiusCommand {{ center: RadiusCenter::{center}, amount: {signed} }}, next: cursor(0, 1) }}")
        self.assertEqual(self.lower_record("a9 0c")[0],
            "Statement::YawOrbit { center: OrbitCenter::Selected, angle: ByteOperand::Actor(ByteField::WordPart { field: WordField::Position(Axis::X), part: BytePart::Low }), next: cursor(0, 1) }")
        with self.assertRaisesRegex(UnsupportedPath, "unported byte operand"):
            self.lower_record("a9 ff")
        changed = bytearray(self.rom)
        changed[0x4F536:0x4F53B] = bytes.fromhex("a5 81 ab 81 0f")
        generated = generate(bytes(changed), (("GEOMETRY", PathAddress(0xF536)),))
        self.assertIn("use super::path_program::OrbitCenter;", generated)
        self.assertIn("use super::path_steering::{RadiusCenter, RadiusCommand};", generated)

    def test_active_node_flags_import_requires_the_reviewed_live_word(self):
        self.assertEqual(self.lower_record("7b a3 9a")[0],
            "Statement::ImportActiveNodeFlags { destination: WordField::ScriptValue, next: cursor(0, 1) }")
        self.assertIn("destination: WordField::MotionPhase", self.lower_record("7b a1 9a")[0])
        for index in range(256):
            if index not in (0x32, 0x34, 0x36, 0x43, 0x90, 0x92, 0x94, 0x9A):
                with self.assertRaisesRegex(UnsupportedPath, "unported shared word"):
                    self.lower_record(f"7b a3 {index:02x}")
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand 04"):
            self.lower_record("7b 04 9a")
        self.assertIn("Statement::ExportActiveNodeFlags", self.lower_record("80 a3 9a")[0])
        # Other widths and guessed absolute-address aliases remain unreviewed.
        for record in ("78 a3 9a", "7a a3 9a", "7e a3 f6 d7", "7c a3 f6 d7"):
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_guidance_history_and_control_style_require_exact_reviewed_fields(self):
        self.assertEqual(self.lower_record("7b a3 36")[0],
            "Statement::Guidance { command: GuidanceCommand::CopyTo(WordField::ScriptValue), next: cursor(0, 1) }")
        self.assertEqual(self.lower_record("80 a3 36")[0],
            "Statement::Guidance { command: GuidanceCommand::Assign(WordOperand::Actor(WordField::ScriptValue)), next: cursor(0, 1) }")
        self.assertIn("Statement::ImportControlStyle", self.lower_record("79 a1 d0 1d")[0])
        self.assertIn("GuidanceCommand::CopyTo(WordField::MotionScriptOverlap)", self.lower_record("7b a2 36")[0])
        for index in range(256):
            if index not in (0x0B, 0x32, 0x34, 0x36, 0x43, 0x9A):
                with self.assertRaises(UnsupportedPath):
                    self.lower_record(f"80 a3 {index:02x}")
        for record in ["7b a4 36", "80 a4 36", "7a a3 36", "7f a3 36", "7c a3 92 d7", "7d a1 d0 1d", "7c a3 d0 1d", "fb d0 1d 01"]:
            with self.assertRaises(UnsupportedPath):
                self.lower_record(record)

    def test_first_control_guidance_retains_history_gate_layout_branch_and_timing(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0x04B5))
        _, statements = lower_graph(extractor, PathAddress(0x04B5), 0)
        self.assertEqual(len(statements), 27)
        mapped = dict(zip((c.address.offset for c in commands), statements))
        self.assertIn("CampaignByte::Difficulty", mapped[0x04B9])
        self.assertIn("InvertNext", mapped[0x04BC])
        self.assertIn("SetModeLowNibbleFour", mapped[0x04C4])
        self.assertIn("GuidanceCommand::CopyTo", mapped[0x04C6])
        self.assertIn("GuidanceCommand::Assign", mapped[0x04D4])
        self.assertIn("iterations: 5", mapped[0x04DD])
        self.assertIn("Statement::ImportControlStyle", mapped[0x04EA])
        self.assertIn("ActorCondition::ZeroByte", mapped[0x04EE])
        self.assertIn("number: ByteOperand::Literal(213)", mapped[0x04F2])
        self.assertIn("immediate: false", mapped[0x04FD])
        for offset, duration in [(0x04C2, 16), (0x04D7, 30), (0x04FB, 65)]:
            self.assertIn(f"duration: ByteOperand::Literal({duration})", mapped[offset])
        self.assertIn("use super::path_program::GuidanceCommand;", generate(self.rom, (("GUIDANCE", PathAddress(0x04B5)),)))

    def test_node_gated_target_root_lowers_all_edges_and_live_health_selector(self):
        extractor = PathExtractor(self.rom)
        commands = graph(extractor, PathAddress(0x545F))
        _, statements = lower_graph(extractor, PathAddress(0x545F), 0)
        self.assertEqual([command.address.offset for command in commands],
                         [0x545F, 0x5460, 0x5463, 0x5468, 0x5469, 0x8D53])
        self.assertEqual(statements, [
            "Statement::Appearance { command: AppearanceCommand::Visibility(false), next: cursor(0, 1) }",
            "Statement::ImportActiveNodeFlags { destination: WordField::ScriptValue, next: cursor(0, 2) }",
            "Statement::Compare { condition: ActorCondition::AnyWordBitsSet(WordOperand::Actor(WordField::ScriptValue), WordOperand::IndexedBitMask { selector: ByteOperand::Actor(ByteField::Health), masks: &VARIABLE_BIT_MASKS }), taken: cursor(0, 5), next: cursor(0, 3) }",
            "Statement::ConsiderPrimaryTarget { next: cursor(0, 4) }",
            "Statement::Control(ControlCommand::Goto { target: cursor(0, 1) })",
            "Statement::Control(ControlCommand::End)",
        ])
        generated = generate(self.rom, (("TARGET_SERVICE", PathAddress(0x545F)),))
        self.assertIn("const VARIABLE_BIT_MASKS: [u16; 128]", generated)
        self.assertIn("LOWERED_COMMAND_COUNT: usize = 6", generated)

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

    def test_weapon_selector_command_byte_alias_and_immediate_fire(self):
        self.assertEqual(byte_field(0x2F), "ByteField::WeaponSelection")
        for value in range(256):
            expected = f"Statement::Mutate {{ mutation: Mutation::Byte {{ field: ByteField::WeaponSelection, operation: ByteOperation::Assign(ByteOperand::Literal({value})) }}, next: cursor(0, 1) }}"
            self.assertEqual(self.lower_record(f"39 {value:02x}")[0], expected)
            self.assertEqual(self.lower_record(f"0b {value:02x} 2f")[0], expected)
        self.assertEqual(self.lower_record("79 2f 4d 1b")[0],
            "Statement::ImportSurfaceMode { destination: ByteField::WeaponSelection, next: cursor(0, 1) }")
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand 2F"):
            word_field(0x2F)
        self.assertEqual(self.lower_record("35")[0],
            "Statement::FireWeapon { next: cursor(0, 1) }")

    def test_dedicated_axis_adds_keep_byte_rotation_and_signed_word_displacement(self):
        for value in range(256):
            for index, axis in enumerate(("X", "Y", "Z")):
                with self.subTest(value=value, axis=axis):
                    self.assertEqual(self.lower_record(f"{0x71 + index:02x} {value:02x}")[0],
                        f"Statement::Mutate {{ mutation: Mutation::Byte {{ field: ByteField::Rotation(Axis::{axis}), operation: ByteOperation::Add(ByteOperand::Literal({value})) }}, next: cursor(0, 1) }}")
                    self.assertEqual(self.lower_record(f"{0x74 + index:02x} {value:02x}")[0],
                        f"Statement::Mutate {{ mutation: Mutation::Word {{ field: WordField::Position(Axis::{axis}), operation: WordOperation::Add(WordOperand::SignedByte(ByteOperand::Literal({value}))) }}, next: cursor(0, 1) }}")
            self.assertEqual(self.lower_record(f"77 a3 {value:02x}")[0],
                f"Statement::Mutate {{ mutation: Mutation::Word {{ field: WordField::ScriptValue, operation: WordOperation::Add(WordOperand::SignedByte(ByteOperand::Literal({value}))) }}, next: cursor(0, 1) }}")
        with self.assertRaisesRegex(UnsupportedPath, "unported word operand"):
            self.lower_record("77 04 01")

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
                field(0x86)

    def test_depth_word_and_byte_views_stop_before_animation_channels(self):
        self.assertEqual(word_field(0x87), "WordField::DepthOffset")
        for variable, part in ((0x87, "Low"), (0x88, "High")):
            self.assertEqual(byte_field(variable),
                             f"ByteField::WordPart {{ field: WordField::DepthOffset, part: BytePart::{part} }}")
            self.assertIn(byte_field(variable), self.lower_record(f"0b 03 {variable:02x}")[0])
        with self.assertRaises(UnsupportedPath):
            word_field(0x88)
        self.assertIn("WordField::DepthOffset", self.lower_record("0c 03 00 87")[0])

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
