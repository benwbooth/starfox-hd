"""Scene exits, player main strategy phases 7/8 and the frame's collision pass."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerStrategyStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_wait_exit_marks_action_carry_scene_anchor_blend_and_restore_cue(self):
        self.source(0x0684EF, "b42bb9776b090199776bad731dc91dd00d5aa03f03b9210009209921007a"
                              "c220b50e99456be22022fb9707c220ade31c22096e7fe220")

    def test_shared_exit_installs_flight_at_the_configuration_phase(self):
        # Reticle, equipment ($06:D9FF), mode/HUD bits, visibility, the
        # death record, flags, strategy 9C27, the three contact records,
        # camera task clear, then the word-bounded configuration table.
        self.source(0x068525, "e220c210ad2f1e09808d2f1ea9808d301e8d311e20ffd9b42bc220a910001c841b"
                              "e220c220a904000c961be220b9776b090199776bb52329fd9523b526090895265a"
                              "a90c2260237fc220a9a4f399626ae220a90699646a7ab52129fe9521b52229fb9522"
                              "c220a9279c9519e220a906951b")
        self.source(0x0685E7, "ade21dc90a009003a900000aaabf5e9d06aaade31d898000d006bf0000068004"
                              "bf020006997b6b8502bf0400068d561ebf0600068d541ee220fa228695064c279c")
        self.source(0x06D9FF, "5a08b42be220c210add41d99066cadd31d99056cadd21d99046c287a60")

    def test_configuration_phase_records(self):
        self.source(0x069D5E, "729d7a9d829d8a9d929d9a9da29daa9d729db29d")
        self.source(0x069D72, "0500000000000000" "0500000000000000" "0500000000000000"
                              "0700030000010700" "0700030000010700" "0700030000000000"
                              "0500000000000000" "07000300fa000800" "0500000000000000")

    def test_retained_pitch_entry_and_visit(self):
        self.source(0x0687A9, "b42b5ab42ba9008dce1da91299a06a7aade01d09208de01dade11d09808de11d"
                              "2023dbad4d1b29f809008d4d1bc220a908008502e220228695065aa03f03b9"
                              "21000908992100b9210009109921007a203c88b52009089520b52429fd9524")
        self.source(0x068807, "5ab42bb9726a29ef99726a7ab52309089523225794062275900622eee20620269a"
                              "da22f89006c220bf929606fa9d0400e2204c7f87")
        self.source(0x06877F, "b42bb9726a098099726a225baf0722959106223692062"
                              "2e28f062238b00722ccd60722b2da075cba9d06")
        self.source(0x06883C, "b9776b8904f0045c6a880622ca9c07")

    def test_pilot_pair_shapes_and_flight_entry_reset(self):
        self.source(0x069692, "4cc2" "68c2" "e8c5" "14c9" "4cc9" "30c9")
        self.source(0x0690D4, "08c2305ab42bb9ff6b29ff00c906009003a900007aaa286b")
        self.source(0x0690F8, "08c23022d490068a29feffaa286b")
        self.source(0x06DB23, "5a08e220c210b42bc220b50c99ed6bb50e99ef6bb51099f16be220a90099606b"
                              "a90099616ba90099dc6aa90099de6a99ad6a99036c99ab6a99ac6a99eb6a99ed6a"
                              "99ee6a99ec6a99a36a99a46a99306b99626b99706a997a6a997b6a997c6a99e96a"
                              "99ea6a997f6a99176bc220a9000099756a990b6b990d6b990f6b99f16a99ef6a99"
                              "7d6a99056b99076b99096b99e06a99e26a99e46a99566b287a60")

    def test_consumable_gate_precedes_rapid_fire(self):
        self.source(0x07D78B, "a912227b2a7fc00000f0045ce4d707a916227b2a7fc00000f0045ce4d707b42b"
                              "c220b9136ce220f0045ce4d707b9616b290ff0045cdbd707ad36198940f01a5a"
                              "20a3d87ab9616b29f0090599616b8009dabb7ade616bdabb7a")

    def test_collision_pass_runs_at_frame_start_after_the_queue(self):
        self.source(0x038027, "22a1327f")
        self.source(0x0380AE, "22a1327f2280797f")
        self.source(0x7F7980, "e220ada61a8920d0088901d00420c27b6b")
        self.source(0x7F7BC2, "222d407f22bc117f60")

    def test_common_death_defers_to_a_registered_type_twelve_routine(self):
        self.source(0x03A055, "a90c223b237f")


if __name__ == "__main__":
    unittest.main()
