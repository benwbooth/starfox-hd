#!/usr/bin/env python3
"""ROM-backed static proof for the first-sortie follow and return cameras."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

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

    def test_follow_lead_uses_retail_settle_chase_decay_and_depth_composition(
        self,
    ) -> None:
        # While the settle timer is live the lead chases the selected target;
        # afterward it decays to zero. The follow depth is chased separately,
        # doubled, combined with the lead, and then applied to the anchor.
        self.assert_runtime_bytes(
            0x078C47,
            """
            B9 58 6B F0 19 3A 99 58 6B C2 20
            B9 56 6B 85 3A AD B8 1D 22 34 25 7F 99 56 6B
            E2 20 80 13 C2 20 B9 56 6B 85 3A A9 00 00
            22 E8 25 7F 99 56 6B E2 20
            E2 20 C2 20 B9 52 6B 85 3A AD C0 1D
            22 A3 25 7F 99 52 6B E2 20
            C2 20 B9 52 6B 0A 18 79 56 6B 8D C0 1D E2 20
            20 7A 9D
            """,
        )

    def test_return_initializes_orbit_and_installs_the_three_stage_task(
        self,
    ) -> None:
        # The return camera starts at a depth of -80, derives the orbit yaw,
        # installs its update task, and orders depth, aim, and motion stages.
        self.assert_runtime_bytes(
            0x079F44,
            """
            5A B4 2B C2 20 A9 B0 FF 99 54 6B E2 20 7A
            20 0D A1 5A B4 2B A9 07 99 9F 6A C2 20 A9 68 9F
            99 9D 6A E2 20 7A
            20 12 A0 20 72 9F 20 2B A1 6B
            """,
        )

    def test_return_depth_advances_by_five_before_orbit_positioning(self) -> None:
        self.assert_runtime_bytes(
            0x07A012,
            """
            5A B4 2B C2 20 B9 54 6B 18 69 FB FF
            85 97 99 54 6B E2 20 7A 80 09
            """,
        )

    def test_orbit_yaw_steps_once_and_is_published_to_the_camera(self) -> None:
        self.assert_runtime_bytes(
            0x07A069,
            """
            5A DA AC D6 14 B5 14 85 3C A2 3F 03 AD E2 1D C9 09
            F0 09 BD E2 1C 38 E9 01 9D E2 1C BD E2 1C 99 14 00
            A9 00 99 12 00 FA
            """,
        )

    def test_return_aim_uses_retail_atan_half_pitch_and_minimum_chase(
        self,
    ) -> None:
        # The live target delta is converted into desired pitch and yaw. Pitch
        # is negated and halved, then both axes use the retail chase helper.
        self.assert_runtime_bytes(
            0x079F72,
            """
            5A 08 DA A2 3F 03 C2 20
            BD C1 1C 95 12 BD C3 1C 95 14 BD C5 1C 95 16
            E2 20 AC FF 1D 9C 9D 14 22 A5 21 7F
            C2 20 49 FF FF 1A 85 04 E2 20 22 88 21 7F
            C2 20 85 0A E2 20 C2 20 A5 04 C9 00 80 6A 85 04
            E2 20 C2 20 B5 12 85 3A A5 04 22 34 25 7F 95 12
            E2 20 C2 20 B5 14 85 3A A5 0A 22 34 25 7F 95 14
            E2 20 C2 20 A9 00 00 95 16 E2 20
            """,
        )

    def test_return_inherits_all_three_player_motion_components(self) -> None:
        self.assert_runtime_bytes(
            0x07A12B,
            """
            DA 5A 08 B4 2B A2 3F 03 C2 20
            B9 0B 6B 95 32 B9 0D 6B 95 34 B9 0F 6B 95 36
            E2 20 28 7A FA 60
            """,
        )


if __name__ == "__main__":
    unittest.main()
