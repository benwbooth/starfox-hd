import unittest
from unittest import mock

import verify_corneria_semantic_oracle as oracle


class CorneriaSemanticOracleTests(unittest.TestCase):
    def test_default_checkpoint_scenes_are_one_through_983(self):
        self.assertEqual(oracle.CHECKPOINT_SCENES, tuple(range(1, 984)))

    def test_parse_fields_rejects_duplicate_tokens(self):
        with self.assertRaises(RuntimeError):
            oracle.parse_fields("kind=semantic kind=other")

    def test_parse_fields_rejects_malformed_tokens(self):
        with self.assertRaises(RuntimeError):
            oracle.parse_fields("kind=semantic malformed")

    def test_compare_rejects_empty_scene_selection(self):
        with self.assertRaises(RuntimeError):
            oracle.compare("", "", checkpoint_scenes=())

    def test_compare_rejects_active_object_missing_on_both_sides(self):
        free = ",".join(str(slot) for slot in range(1, 70))
        artifact = (
            "kind=semantic scene=1 active_order=0 "
            f"free_order={free} background_source=3\n"
        )
        with self.assertRaises(RuntimeError):
            oracle.compare(artifact, artifact, checkpoint_scenes=(1,))

    def test_relocate_accepts_identity_only_for_identical_programs(self):
        program = b"retail-native path program"
        self.assertEqual(oracle.relocate_path_cursor(3, program, program), 3)

    def test_relocate_rejects_out_of_range_cursor(self):
        with self.assertRaises(RuntimeError):
            oracle.relocate_path_cursor(4, b"abcd", b"abcd")

    def test_relocate_rejects_different_program_with_same_local_window(self):
        retail = b"prefix-identical-window-suffix-retail"
        native = b"prefix-identical-window-suffix-native"
        with self.assertRaises(RuntimeError):
            oracle.relocate_path_cursor(10, retail, native)

    def test_run_native_sets_full_range_and_scrubs_ambient_controls(self):
        ambient = {
            "SF1_NATIVE_SEMANTIC_RANGE": "9-10",
            "SF1_NATIVE_SEMANTIC_SCENES": "9",
            "SF1_NATIVE_SEMANTIC_ROUTE": "alternate",
            "SF1_NATIVE_SEMANTIC_NO_OBJECTS": "1",
            "PATH": "/usr/bin",
        }
        completed = mock.Mock(stdout="native semantic output\n")
        with (
            mock.patch.object(oracle.os, "environ", ambient),
            mock.patch.object(oracle.subprocess, "run", return_value=completed) as run,
        ):
            result = oracle.run_native()

        self.assertEqual(result, "native semantic output\n")
        command = run.call_args.args[0]
        self.assertIn("sf1_native_semantic_probe", command)
        kwargs = run.call_args.kwargs
        self.assertEqual(kwargs["env"]["SF1_NATIVE_SEMANTIC_RANGE"], "1-983")
        for name in (
            "SF1_NATIVE_SEMANTIC_SCENES",
            "SF1_NATIVE_SEMANTIC_ROUTE",
            "SF1_NATIVE_SEMANTIC_NO_OBJECTS",
        ):
            self.assertNotIn(name, kwargs["env"])
        self.assertEqual(kwargs["cwd"], oracle.ROOT)


if __name__ == "__main__":
    unittest.main()
