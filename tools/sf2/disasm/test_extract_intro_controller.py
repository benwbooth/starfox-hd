import unittest

from extract_intro_controller import TimingCondition, authored_scene_controller
from extract_map import DEFAULT_ROM


class IntroControllerTests(unittest.TestCase):
    def test_opening_companion_contains_the_missing_cues_and_palette_services(self):
        scene = authored_scene_controller(DEFAULT_ROM.read_bytes(), 6)
        self.assertEqual((scene.path_root, scene.script), (0xFA11, 0x0DBEDF))
        self.assertEqual(len(scene.commands), 14)
        cuts = [command for command in scene.commands if command.service == 0x0DC82F]
        self.assertEqual([command.start for command in cuts], [182, 249, 293, 327, 416])
        self.assertTrue(all(command.condition == TimingCondition.AT for command in cuts))
        for command in cuts:
            self.assertFalse(command.applies(command.start - 1))
            self.assertTrue(command.applies(command.start))
            self.assertFalse(command.applies(command.start + 1))
        double_restore = scene.commands[1]
        self.assertEqual(double_restore.service, 0x0DCF73)
        self.assertFalse(double_restore.applies(106))
        self.assertTrue(double_restore.applies(107))
        self.assertTrue(double_restore.applies(138))
        self.assertFalse(double_restore.applies(139))
        self.assertEqual((scene.commands[7].start, scene.commands[7].service), (441, 0x0DCA18))

    def test_source_increment_service_is_not_an_inferred_phase_assignment(self):
        rom = DEFAULT_ROM.read_bytes()
        self.assertEqual(rom[0x06C82F:0x06C83F], bytes.fromhex("08ee721dc220a90000991a6ce2202860"))

    def test_changed_pointer_loader_and_dispatch_comparisons_fail_closed(self):
        rom = DEFAULT_ROM.read_bytes()
        for offset in [0x03294A, 0x06BD5E, 0x06BD6E, 0x06BD7E, 0x06BD8E, 0x06BDA0]:
            changed = bytearray(rom)
            changed[offset] ^= 1
            with self.subTest(offset=offset), self.assertRaisesRegex(ValueError, "signature mismatch"):
                authored_scene_controller(bytes(changed), 6)

    def test_invalid_records_and_pointers_fail_closed(self):
        rom = DEFAULT_ROM.read_bytes()
        changed = bytearray(rom)
        changed[0x06BEDF] = 5
        with self.assertRaisesRegex(ValueError, "unknown timing condition"):
            authored_scene_controller(bytes(changed), 6)
        changed = bytearray(rom)
        record = 0x06D4C7 + 6 * 8
        changed[record + 2:record + 5] = bytes.fromhex("00007e")
        with self.assertRaisesRegex(ValueError, "not mapped ROM"):
            authored_scene_controller(bytes(changed), 6)
        changed[record + 2:record + 5] = bytes.fromhex("feff0d")
        changed[0x06FFFE:0x070000] = bytes([0, 0])
        with self.assertRaisesRegex(ValueError, "truncated controller record"):
            authored_scene_controller(bytes(changed), 6)
        with self.assertRaisesRegex(ValueError, "outside authored table"):
            authored_scene_controller(rom, 30)


if __name__ == "__main__":
    unittest.main()
