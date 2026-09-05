import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from extract_intro_paths import (
    authored_intro_root, authored_scene_roots, cursor_is_reached, decode,
    installed_scene_roots, main, trace_paths,
)
from extract_map import DEFAULT_ROM
from extract_path import PATH_DATA_FILE, PathAddress, PathExtractor


class IntroPathTraceTests(unittest.TestCase):
    def test_authored_intro_root_comes_from_the_indexed_scene_installer(self):
        self.assertEqual(authored_intro_root(DEFAULT_ROM.read_bytes()), 0xFA11)

    def test_changed_scene_installer_is_not_silently_accepted(self):
        source = bytearray(DEFAULT_ROM.read_bytes())
        source[0x032968] ^= 1
        with self.assertRaisesRegex(ValueError, "scene installer signature mismatch"):
            authored_intro_root(bytes(source))

    def test_installed_scenes_are_validated_against_the_indexed_table(self):
        rom = DEFAULT_ROM.read_bytes()
        table = authored_scene_roots(rom)
        self.assertEqual(len(table), 30)
        self.assertEqual(len(set(table)), 24)
        installations = self.write_trace(
            "frame=156 selector=6 index=6 root=FA11\n"
            "frame=2034 selector=7 index=7 root=B65B\n"
        )
        self.assertEqual(installed_scene_roots(rom, installations), [0xB65B, 0xFA11])

    def test_installer_clamps_the_selector_but_not_a_forged_root(self):
        rom = DEFAULT_ROM.read_bytes()
        table = authored_scene_roots(rom)
        clamped = self.write_trace(f"frame=1 selector=255 index=0 root={table[0]:04X}\n")
        self.assertEqual(installed_scene_roots(rom, clamped), [table[0]])
        for line in [
            "frame=1 selector=6 index=6 root=B65B\n",
            "frame=1 selector=30 index=30 root=FA11\n",
            "frame=1 selector=256 index=0 root=FA11\n",
            "frame=2 selector=6 index=6 root=FA11\nframe=1 selector=7 index=7 root=B65B\n",
            "frame=1 root=FA11\n",
            "",
        ]:
            with self.subTest(line=line), self.assertRaises(ValueError):
                installed_scene_roots(rom, self.write_trace(line))

    def test_actual_attract_installer_roots_reach_the_previously_missing_family(self):
        commands, failures = decode(DEFAULT_ROM, [0xFA11, 0xB65B])
        self.assertEqual(failures, [])
        self.assertEqual(len(commands), 903)
        for cursor in [0xB6F7, 0xB741, 0xB7E3, 0xB866, 0xB868, 0xDB2E]:
            self.assertTrue(cursor_is_reached(commands, cursor), f"{cursor:04X}")

    def test_both_inline_phase_branches_are_source_signature_checked(self):
        rom = DEFAULT_ROM.read_bytes()
        extractor = PathExtractor(rom)
        for root, successors in [(0xB796, [0xB7A4, 0xB7AB]), (0xB869, [0xB877, 0xB87E])]:
            command = extractor.decode_command(PathAddress(root))
            self.assertEqual(command.successors, tuple(PathAddress(offset) for offset in successors))
            changed = bytearray(rom)
            changed[PATH_DATA_FILE + successors[1] - 2] ^= 1
            with self.assertRaisesRegex(ValueError, "signature mismatch"):
                PathExtractor(bytes(changed)).decode_command(PathAddress(root))

    def test_consumed_escape_is_not_misclassified_as_a_new_path_root(self):
        extractor = PathExtractor(DEFAULT_ROM.read_bytes())
        command = extractor.decode_command(PathAddress(0xFBB5))
        commands = {command.address: command}
        self.assertTrue(cursor_is_reached(commands, 0xFBB5))
        self.assertTrue(cursor_is_reached(commands, 0xFBB6))
        self.assertFalse(cursor_is_reached(commands, 0xFBB7))

    def write_trace(self, body: str) -> Path:
        handle = tempfile.NamedTemporaryFile("w", delete=False)
        handle.write(body)
        handle.close()
        self.addCleanup(lambda: Path(handle.name).unlink(missing_ok=True))
        return Path(handle.name)

    def test_filters_non_generic_strategy(self):
        trace = self.write_trace(
            "frame=1 objects=BC9C,06845C,0,0,0,0,0,0,123;"
            "E530,7F7E53,0,0,0,0,0,0,64089 draws=\n"
        )
        self.assertEqual(trace_paths(trace), [64089])

    def test_malformed_record_fails_closed(self):
        trace = self.write_trace("frame=1 objects=BC9C,7F7E53,broken draws=\n")
        with self.assertRaises(ValueError):
            trace_paths(trace)

    def test_empty_trace_fails_closed(self):
        trace = self.write_trace("frame=1 mode=4 objects= draws=\n")
        with self.assertRaises(ValueError):
            trace_paths(trace)

    def test_cli_rejects_an_observed_cursor_outside_the_authored_graph(self):
        trace = self.write_trace(
            "frame=1 objects=BC9C,7F7E53,0,0,0,0,0,0,46839 draws=\n"
        )
        output = StringIO()
        with patch("sys.argv", ["extract_intro_paths.py", str(trace), "--summary"]):
            with redirect_stdout(output):
                self.assertEqual(main(), 1)
        self.assertIn("UNREACHED_OBSERVED 44:B6F7", output.getvalue())

    def test_semantic_gate_does_not_equate_decoding_with_reviewed_behavior(self):
        trace = self.write_trace(
            "frame=1 objects=BC9C,7F7E53,0,0,0,0,0,0,64017 draws=\n"
        )
        output = StringIO()
        with patch("sys.argv", ["extract_intro_paths.py", str(trace), "--summary",
                                "--require-reviewed-semantics"]):
            with redirect_stdout(output):
                self.assertEqual(main(), 1)
        self.assertIn("missing_observed=0", output.getvalue())
        self.assertIn("UNREVIEWED_SEMANTIC opcode=145", output.getvalue())

    def test_child_spawn_path_is_reachable(self):
        commands, failures = decode(DEFAULT_ROM, [0xFCF9])
        self.assertFalse(failures)
        self.assertIn(next(address for address in commands if address.offset == 0xFD52), commands)

    def test_intro_inline_continuations_match_reviewed_rom_bytes(self):
        extractor = PathExtractor(DEFAULT_ROM.read_bytes())
        for root, continuation in [(0xFCB9, 0xFCC4), (0xFDDC, 0xFDE8)]:
            command = extractor.decode_command(PathAddress(root))
            self.assertEqual(command.successors, (PathAddress(continuation),))

    def test_changed_inline_signature_is_rejected(self):
        source = bytearray(DEFAULT_ROM.read_bytes())
        source[PATH_DATA_FILE + 0xFCBA] ^= 1
        extractor = PathExtractor(bytes(source))
        with self.assertRaisesRegex(ValueError, "signature mismatch"):
            extractor.decode_command(PathAddress(0xFCB9))

    def test_unreviewed_inline_root_is_rejected(self):
        source = bytearray(DEFAULT_ROM.read_bytes())
        unreviewed_root = 0xFFFF
        source[PATH_DATA_FILE + unreviewed_root] = 0x89
        extractor = PathExtractor(bytes(source))
        with self.assertRaisesRegex(ValueError, "unreviewed path inline"):
            extractor.decode_command(PathAddress(unreviewed_root))


if __name__ == "__main__":
    unittest.main()
