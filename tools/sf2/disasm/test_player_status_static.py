"""Player-frame status callers and their canonical cross-service aliases."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerStatusStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_pending_display_skips_chase_but_not_actual_shield_warning(self):
        self.source(0x07AF64, "b9386c3028b9006cd9386cf020")
        self.source(0x07AF8C, "098099386cb9386c209baf")

    def test_warning_uses_actual_life_unsigned_loss_and_preserves_dead_history(self):
        self.source(0x07AFA0, "b52df042b9006cc928f035d90a6cb030c90db02cc9059015")
        self.source(0x07AFE0, "b9006c990a6c2860")

    def test_filter_caller_chases_the_existing_heading_bank_after_both_filters(self):
        self.source(0x069195, "5ab42b22ac9106b9da6a853aa90022b5277f99da6a7a6b")

    def test_transformation_countdown_and_new_request_use_the_same_byte(self):
        self.source(0x068FE8, "b42bb99b6b893fd0045c7290063a999b6b8980")
        self.source(0x069003, "b99b6b29bf999b6b")
        self.source(0x069039, "b99b6b29bf999b6b")

    def test_proximity_warning_observes_live_boost_and_brake_action_bits(self):
        self.source(0x06A66B, "b9776b8920f0045c34a806")
        self.source(0x06A67F, "b9776b8940d0045c34a806")


if __name__ == "__main__":
    unittest.main()
