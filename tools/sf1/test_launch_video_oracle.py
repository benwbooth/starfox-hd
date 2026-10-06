"""The independent launch runner must reject changed or incomplete evidence."""

import tempfile
from pathlib import Path
import unittest

from run_launch_video_oracle import capture_identity, verify_repeated_captures


class RepeatedLaunchCaptures(unittest.TestCase):
    def test_no_images_is_not_success(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / "launch_display.txt").write_text("video=1\n")
            with self.assertRaisesRegex(RuntimeError, "omitted"):
                capture_identity(path)

    def test_both_payloads_and_inventory_must_match(self):
        with tempfile.TemporaryDirectory() as directory:
            first, second = (Path(directory) / name for name in ("first", "second"))
            for path in (first, second):
                path.mkdir()
                (path / "launch_display.txt").write_text("video=1\n")
                (path / "launch_0001.ppm").write_bytes(b"original pixels")
                (path / "launch_0001.vram").write_bytes(b"original bitmap")
                (path / "launch_edges_0004.bin").write_bytes(b"original edges")
            verify_repeated_captures(first, second)
            (second / "launch_0001.vram").write_bytes(b"different bitmap")
            with self.assertRaisesRegex(RuntimeError, "launch_0001.vram"):
                verify_repeated_captures(first, second)
            (second / "launch_0001.vram").write_bytes(b"original bitmap")
            (second / "launch_edges_0005.bin").write_bytes(b"unexpected extra record")
            with self.assertRaisesRegex(RuntimeError, "launch_edges_0005.bin"):
                verify_repeated_captures(first, second)

    def test_partial_weapon_capture_is_not_repeatable_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            first, second = (Path(directory) / name for name in ("first", "second"))
            for path in (first, second):
                path.mkdir()
                (path / "weapon_display.txt").write_text("video=1\n")
                (path / "weapon_0001.ppm").write_bytes(b"original pixels")
                with self.assertRaisesRegex(RuntimeError, "omitted paired weapon"):
                    capture_identity(path, "weapon")
                (path / "weapon_0001.vram").write_bytes(b"original bitmap")
            verify_repeated_captures(first, second, "weapon")
            (second / "weapon_display.txt").write_text("video=2\n")
            with self.assertRaisesRegex(RuntimeError, "weapon_display.txt"):
                verify_repeated_captures(first, second, "weapon")


if __name__ == "__main__":
    unittest.main()
