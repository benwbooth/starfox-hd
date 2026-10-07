"""Original camera caller, aliases, and complete seven-bit bob-data domain."""
from pathlib import Path
import re
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerCameraGroundStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_animation_uses_raw_seven_bit_channel_and_all_readable_bias_bytes(self):
        self.source(0x07845B, "bdcb1c297faabfdc8407")
        root = Path(__file__).resolve().parents[3]
        native = (root / "rust/sf2-game/src/native/player_camera_ground.rs").read_text()
        table = re.search(r"const FRAME_PITCH_BIAS: \[u8; 128\] = \[(.*?)\];", native, re.S)
        self.assertIsNotNone(table)
        values = bytes(int(value) for value in re.findall(r"\d+", table.group(1)))
        offset = source_offset(0x0784DC)
        self.assertEqual(len(values), 128)
        self.assertEqual(values, self.rom[offset:offset + 128])

    def test_ground_protection_reset_writes_bob_then_pitch(self):
        self.source(0x078286, "c220a9000099376be220c220a9000099316be2204cd984")

    def test_walker_target_uses_distinct_carried_y_and_real_fine_yaw_high_byte(self):
        self.source(0x079754, "c220b9ed6b950cb9f36b950eb9f16b9510e220b9bc6a")

    def test_aim_uses_live_fixed_view_and_clears_shared_axis_control(self):
        self.source(0x0797CF, "daa23f03acd6149c9d1422a5217f")

    def test_yaw_difference_reads_the_value_just_published(self):
        self.source(0x078631, "c220b9bb6a49ffff1a8dae1de220c220adae1d99336be220c220adae1d38f9336b99396b")

    def test_common_caller_selects_terrain_pitch_only_for_family_thirty(self):
        self.source(0x078530, "b9a06a29f0c930d0045c42850720b786800422f18107")


if __name__ == "__main__":
    unittest.main()
