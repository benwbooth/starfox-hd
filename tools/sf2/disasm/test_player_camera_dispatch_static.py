"""Complete camera-mode call order, gates, and retained publications."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerCameraDispatchStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_scripted_action_and_absent_primary_have_different_tails(self):
        self.source(0x078008, "ad841b890200e220f0034c3880ad721dd024")
        self.source(0x07801A, "c230b42bb99a6af018")
        self.source(0x078038, "20dc96287a6b20dc9622f69d07287a6b")

    def test_common_modes_publish_before_view_selection_and_auxiliary(self):
        self.source(0x07806E, "20ec8422ab9a07208b9620f09622ef9a072288940722f69d072860")
        self.source(0x078094, "20ec8422ab9a07208b9620f09622ef9a07228894072289940722f69d072860")
        self.source(0x079488, "6b")  # Actual empty source routine, not a stub.

    def test_surface_selects_view_before_preparation_and_publishes_after_occupancy(self):
        self.source(0x0780DD, "22ef9a07202c8122ab9a07")
        self.source(0x0780F6, "a23f03c220b50c8502b51085972271db0d")
        self.source(0x078110, "b9636b09028005b9636b29fd99636bfa208b9620f09622f69d072860")

    def test_horizon_requests_only_set_bit_without_clearing_existing_state(self):
        self.source(0x07805E, "ad9d1d09808d9d1d8006ad9d1d8d9d1d")
        self.source(0x0780CD, "ad9d1d09808d9d1d8006ad9d1d8d9d1d")
        # Consumer selects the zero-filled horizon/roll table.
        self.source(0x07BD78, "ad9d1d298000f008a2af19207fbe")
        self.source(0x07BE8A, "a900009f0000009f000000")

    def test_projection_gates_preserve_prior_value(self):
        self.source(0x079491, "ad841b890200e220f0034c0b95")
        self.source(0x07949E, "5ab42bb9656b7a8940f0045c0b9507")

    def test_midpoint_rounding_precedes_wrapped_floor_and_clamp(self):
        self.source(0x0794B2, "ad321e186d341ec900806a100369000049ffff1a18790e00")
        self.source(0x0794CA, "c900806ac900806ac900806ac900806a")
        self.source(0x079503, "adae1d8d521e")

    def test_enclosing_player_pairs_dispatch_and_continuity_before_separate_plane_clamp(self):
        self.source(0x069A26, "2200800722fb970760")
        self.source(0x079D66, "a9f6ff186d0f1ed90e001003990e00")

    def test_free_flight_installer_uses_profile_capture_and_shared_surface_flag(self):
        self.source(0x0686C9, "c220b99d6ac90000e220f0045c1a8706b97d6b8980")
        self.source(0x0686F9, "22ca9c075ab42be220a907999c6ab9646b29f799646bc220a94880999a6a")


if __name__ == "__main__":
    unittest.main()
