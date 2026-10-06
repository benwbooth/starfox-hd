import hashlib
import unittest

from generate_native_paths import DEFAULT_ROM, ROOTS, PathAddress, PathExtractor, graph
from path_backlog import inventory


class PathBacklogTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()
        cls.report = inventory(cls.rom)

    def test_discovered_roots_are_partitioned_without_counting_children_twice(self):
        report = self.report
        extractor = PathExtractor(self.rom)
        self.assertEqual({row['root'] for row in report['roots']},
                         {root.label() for root in extractor.discover_roots()})
        self.assertEqual(report['discovered_roots'],
                         report['registered_discovered_roots'] + report['blocked_roots']
                         + report['lowerable_unregistered_roots'])
        self.assertEqual(len(report['roots']), len({row['root'] for row in report['roots']}))
        registered = {root.label() for _, root in ROOTS}
        self.assertEqual({row['root'] for row in report['roots'] if row['registered_name']}
                         | {row['root'] for row in report['additional_registered_roots']}, registered)
        self.assertTrue({row['root'] for row in report['additional_registered_roots']}
                        .isdisjoint(row['root'] for row in report['roots']))

    def test_blockers_are_explicit_and_prevent_false_completion(self):
        report = self.report
        self.assertFalse(report['discovered_catalog_complete'])
        self.assertEqual(report['shipping_integration'], 'not established by this report')
        self.assertTrue(any(row['status'] == 'blocked' for row in report['roots']))
        for row in report['roots']:
            self.assertLessEqual(row['commands_in_registered_graphs'], row['source_commands'])
            if row['status'] == 'blocked':
                self.assertTrue(row['first_blocker'])
                self.assertIsNone(row['lowered_statements'])
            else:
                self.assertIsNone(row['first_blocker'])
                self.assertGreater(row['lowered_statements'], 0)

    def test_complete_encounter_has_source_digest_and_distinct_shipping_status(self):
        root = PathAddress(0x6550)
        row = next(row for row in self.report['roots'] if row['root'] == root.label())
        commands = graph(PathExtractor(self.rom), root)
        self.assertEqual(row['status'], 'registered')
        self.assertEqual(row['registered_name'], 'TARGET_GATED_PULSE_ATTACKER')
        self.assertEqual(row['source_commands'], 546)
        self.assertEqual(row['commands_in_registered_graphs'], 546)
        self.assertEqual(row['lowered_statements'], 540)
        self.assertEqual(row['source_sha256'], hashlib.sha256(bytes.fromhex(
            ''.join(command.raw_hex for command in commands))).hexdigest())
        self.assertEqual(self.report['source_rom_sha256'], hashlib.sha256(self.rom).hexdigest())


if __name__ == '__main__':
    unittest.main()
