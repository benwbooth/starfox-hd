#!/usr/bin/env python3
"""ROM-backed static proof for the ordinary first-sortie follow camera."""

from __future__ import annotations

import unittest
from pathlib import Path

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


@unittest.skipUnless(Path(DEFAULT_ROM).is_file(), "retail SF2 ROM is not present")
class OpeningCameraStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_runtime_bytes(self, address: int, expected: str) -> None:
        payload = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset : offset + len(payload)], payload)

    def test_control_copies_pose_runs_three_follow_stages_and_publishes_anchor(
        self,
    ) -> None:
        # The retail routine copies all three player-position words into its
        # camera scratch anchor, runs state, horizontal, and vertical control,
        # adds the ambient height, writes the typed anchor components back,
        # and performs the final orientation step.
        self.assert_runtime_bytes(
            0x0784EC,
            """
            5A 08 E2 20 C2 10 B4 2B C2 20
            B5 0C 8D C2 1D B5 0E 8D C4 1D B5 10 8D C6 1D
            E2 20 20 66 8D 20 7A 8A 20 BF 88
            C2 20 AD C4 1D 18 79 E2 6A 8D C4 1D 99 C3 6A
            AD C2 1D 99 C1 6A AD C6 1D 99 C5 6A
            E2 20 20 D3 8B
            """,
        )

    def test_flight_wrapper_orders_control_recoil_orientation_anchor_and_output(
        self,
    ) -> None:
        # This is the live wrapper reached by the first sortie. The calls after
        # camera control apply recoil, compose orientation, publish the anchor,
        # run continuity helpers, and then expose the presentation output.
        self.assert_runtime_bytes(
            0x078089,
            """
            08 E2 20 C2 10 AD 9D 1D 8D 9D 1D 20 EC 84
            22 AB 9A 07 20 8B 96 20 F0 96 22 EF 9A 07
            22 88 94 07 22 89 94 07 22 F6 9D 07 28 60
            """,
        )

    def test_camera_motion_finishes_with_the_retail_five_stage_integrator(
        self,
    ) -> None:
        self.assert_runtime_bytes(
            0x0798BF,
            """
            20 A3 99 20 39 9A 20 60 99 20 D2 98 20 83 99
            28 7A FA 6B
            """,
        )


if __name__ == "__main__":
    unittest.main()
