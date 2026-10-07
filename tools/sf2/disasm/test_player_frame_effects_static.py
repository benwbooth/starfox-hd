"""Original ordered effects tail and linked protection installation."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerFrameEffectsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_appearance_then_damage_then_action_gate_and_contact_gate(self):
        self.source(0x069EE8, "221aaa0622a8d107ad721dd02cadf4d7f0132270cd07")

    def test_control_branch_converges_before_shared_eighth_clock_countdown(self):
        self.source(0x069EFE, "5ab42bb9026c7a295fd0045c0d9f06b9026c891ff00da5c42907d007b9026c3a99026c")

    def test_depth_publication_precedes_request_clear_and_recovery(self):
        self.source(0x069F21, "dac220b9a26a2903000aaabf1da306fa9dc81ce220ad1b1e9c1b1e8dae1df0131879006ccdd51d9003add51d99006c22c6d007")

    def test_protection_enable_then_count_then_existing_child(self):
        self.source(0x07CD76, "adf4d7d0045cc1cd07b42bb9026c891ff039a912227b2a7fc00000f0045cc1cd07")

    def test_protection_allocates_shape_then_number_formats_and_defers_real_path(self):
        self.source(0x07CD97, "c220a9ccc5855fe22022172a7fb0045cc1cd07a912223d2a7f20f5bec220a9b9f2992b00e220287a386b287a186b")


if __name__ == "__main__":
    unittest.main()
