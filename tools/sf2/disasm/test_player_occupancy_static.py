"""Source-bound contracts for complete player occupancy response."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


@unittest.skipUnless(Path(DEFAULT_ROM).is_file(), "retail SF2 ROM is not present")
class PlayerOccupancyStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_gate_before_clear_before_exemption_before_camera_task(self):
        self.source(0x07E685, "da5a08e220c210ad721df0045c76e707b42bb9e66b297f99e66bb9eb6b8980f0045c76e707c220b99d6ac90000e220f0045c76e707")

    def test_actual_fixed_view_query_precedes_owner_query(self):
        self.source(0x07E6BA, "5aa03f03c220b90c008502b9100085972271db0de220a502d003820b007ab9e66b098099e66b80017ac220b50c8502b51085972271db0d")

    def test_unblocked_history_replaces_only_changed_axes(self):
        self.source(0x07E70A, "b9296bcdbe1df003992c6bb92a6bcdba1df003992d6badbe1d99296badba1d992a6bb9e66b29bf99e66b4c76e7")

    def test_blocked_contact_sets_event_only_on_transition(self):
        self.source(0x07E737, "b9e66b8940d0020910094099e66b")

    def test_diagonal_tie_keeps_unused_x_distance_and_retained_comparison(self):
        self.source(0x07E7D3, "c220adc01d203ce838f50c100449ffff1a853aadbc1d203ce8100449ffff1ac53ee2209008adc01d8dbe1d8006adbc1d8dba1d")

    def test_neighbors_publish_proxy_x_and_z_and_preserve_height(self):
        self.source(0x07E808, "acd614c220a53a203ce8990c00a53e203ce8991000e220c220b90c008502b9100085972271db0de220a502f00382020018603860eb2900ff0a09000160")

    def test_heading_and_edge_tables_are_source_data(self):
        self.source(0x07E8AF, "e0 00 20 00 c0 ff 40 00 a0 80 60 00 00 00 00 00")
        self.source(0x07E8BF, "c0 40 e0 60 00 80 20 a0 40 c0 60 e0 80 00 a0 20")
        self.source(0x07EA0D, "0406020a08090105")

    def test_heading_return_clears_only_low_shared_yaw_byte(self):
        self.source(0x07E8EF, "b9ce6ad05fa9008d381eb9bc6a48c220b9bb6a853aadb41d22a3257f99bb6ae2206838f9bc6a0a0a0a49ff1a8597b9da6a853aa5972282277f99da6a")

    def test_correction_commits_actual_position_and_horizontal_history(self):
        self.source(0x07E9DA, "c220a50299f96a8502a59799fb6a8597800ac220a5028502a5978597a50218750c950c99ed6ba597187510951099f16b287a60")


if __name__ == "__main__":
    unittest.main()
