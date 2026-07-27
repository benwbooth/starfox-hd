#!/usr/bin/env python3
"""ROM-backed static proof for the Astropolis entry and player handoff."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


PATH_DATA_FILE_OFFSET = 262_144


@unittest.skipUnless(Path(DEFAULT_ROM).is_file(), "retail SF2 ROM is not present")
class AstropolisEntryStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_runtime_bytes(self, address: int, expected: str) -> None:
        payload = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset : offset + len(payload)], payload)

    def assert_path_bytes(self, offset: int, expected: str) -> None:
        payload = bytes.fromhex(expected)
        start = PATH_DATA_FILE_OFFSET + offset
        self.assertEqual(self.rom[start : start + len(payload)], payload)

    def test_scripted_player_entry_waits_for_release(self) -> None:
        # The temporary player strategy preserves camera continuity, tests the
        # entry-phase latch, and returns through the scripted-input services
        # while that latch is nonzero. It cannot run normal flight control
        # before the Astropolis camera path releases it.
        self.assert_runtime_bytes(
            0x0684A2,
            """
            A0 3F 03 B4 2B 22 89 94 07
            C2 20 A9 10 00 1C 84 1B E2 20
            AD 72 1D D0 04 5C EF 84 06
            B4 2B 5A B5 23 29 40 F0 04 5C D5 84 06
            C2 20 AD 96 12 AC 92 12 80 08
            C2 20 AD 98 12 AC 94 12
            8D 36 19 8C 38 19 E2 20 7A
            22 CF BC 0D 22 67 EA 07 6B
            """,
        )

    def test_release_installs_and_enters_live_player_control(self) -> None:
        # The zero-latch branch installs the ordinary per-frame player
        # strategy, completes the one-shot craft setup, and tail-enters that
        # strategy in the same retail update.
        self.assert_runtime_bytes(
            0x068589,
            """
            C2 20 A9 27 9C 95 19 E2 20 A9 06 95 1B
            """,
        )
        self.assert_runtime_bytes(
            0x068621,
            """
            22 86 95 06 4C 27 9C
            """,
        )

    def test_entry_phase_service_advances_and_releases_the_latch(self) -> None:
        # The scene service advances the semantic entry phase and clears its
        # per-task timer. The adjacent release service resets the phase latch.
        self.assert_runtime_bytes(
            0x0DC82F,
            """
            08 EE 72 1D C2 20 A9 00 00 99 1A 6C E2 20 28 60
            08 9C 72 1D 28 60
            """,
        )

    def test_camera_path_encodes_named_phase_motion_and_live_target_aim(self) -> None:
        # Astropolis path $E132 waits for the entry latch, initializes two
        # camera-mover positions, advances them by fixed signed deltas, then
        # imports the live scene target. The scheduled callbacks copy the
        # mover into the presentation camera and aim it at the selected actor.
        self.assert_path_bytes(
            0xE132,
            """
            03 30
            5D 50 CF D8 E2 01 0A
            5D 50 CF D8 E2 01 0B
            5D 50 CF D8 E2 01 0C
            00 40 01 51 E1 16 49 E1
            0C F7 FF 8E 0C 7A FE 90 0C 09 F7 92
            16 60 E1
            00 40 02 68 E1 16 60 E1
            0C 37 00 8E 0C 78 00 90 0C 2B F8 92
            16 77 E1
            FD 54 00
            00 40 03 88 E1
            77 92 08 77 90 02 16 77 E1
            7C 8E E4 1D 7C 90 E6 1D 7C 92 E8 1D
            77 90 14 08 92 FA 00 77 8E CE
            16 A1 E1
            """,
        )
        self.assert_path_bytes(
            0xE1A1,
            """
            4B CB E1 FD 31 00 FC 44 1E 90 02
            00 4C 3F 03 77 92 0A 77 90 FC 77 8E 02
            00 40 04 E6 E1 16 AC E1
            00 4C 3F 03 00 38 3F 03 00 42
            00 4C 3F 03 00 38 3F 03 00 42
            00 4C 3F 03 00 38 3F 03 01 42
            16 E6 E1 4C E6 E1 42 0F
            """,
        )


if __name__ == "__main__":
    unittest.main()
