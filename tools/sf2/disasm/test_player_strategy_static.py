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

    def test_scene_player_death_routine(self):
        # Throttle cancel, the smoke child numbered by direct-page $00 on
        # path F561, primary target rates, 1E0D bit 01, the defeat action,
        # the defeat cue, falling velocity, the countdown, the follow-up
        # routine F512, view bits, child refresh, depth and zero reserve.
        self.source(0x06F3A4,
                    "e220c210221da506c220a99cbc855fe22022172a7fb0045cd8f306a500223d2a7f"
                    "dabb7ac220a91e7e9519e220a97f951bdabb7ac00000f028c220a91e7e991900e220"
                    "a97f991b00c220a961f5992b00e220a901992d00a901992e00a932990a00b42ba901"
                    "853aa901853ca901853e229bb807ad0d1e09018d0d1ea9008de51c5ab42ba90dd915"
                    "6cf00399156cc220a985c5d9136cf00c99136ca9000099166c99186ce2207a5ab42b"
                    "b9a06a7a29f0c910f0045c70f406c220a91100ecc312f00309008022096e7fe22080"
                    "13c220a91500ecc312f00309008022096e7fe220a91e9518a9009513b5188585b514"
                    "8d1215b5138d1115221f2d7f22959106c220a900009de41ce220c220a900009de41c"
                    "e220ada61a8902f0045ccaf406c220a91e009de41ce2205aa90c2260237fc220a912"
                    "f599626ae220a90699646a7ab9ea6a291f99ea6a5aa03f03b9210009089921"
                    "00b9210009109921007a2219237fc220a901009dc81ce220a90099006c6b")

    def test_defeated_player_routine_and_blast_actor(self):
        f512 = ("e220c210b42ba90099006cc220bde41cc9ffffe220d0045c3bf506c220bde41c186901009de41ce22022cfbc0d"
                "b52529df9525b9eb6b8980f0045c83f506c220b50c8502b51085972271db0de220a502d003821c00c220bde41c"
                "c93200e220f00630045c83f506c220a932009de41ce220c220bde41cc92500e220d0045ccaf506c220bde41cc9"
                "3200e220d0045c09f606c220bde41cc92600e220d0045c4ef606c220bde41cc93200e22010045c52f606229aec"
                "074c4ef6c220a90000853ae220a903853ca901853e229fb707a91f853aa91f853ca91f853e2261b807a904853a"
                "a908853ca908853e229bb807b98c6a0980998c6a8045b98c6a297f998c6ac220a90200853ae220a903853ca900"
                "853e229fb707a91f853aa91f853ca91f853e2261b807a900853aa902853ca902853e229bb807b98c6a0980998c"
                "6a2233b807a916227b2a7fc00000d0045c71f6065aa916227b2a7fb9250009089925007a5ab42bb9a06a7a29f0"
                "c920f0045c97f606c220a900009dc11c9dc51c95329536e2202281d706b512853aa90022b5277f95122238b007"
                "22e28f062200800722fb9707225baf07c220bde41cc93200e220f00630045c69f706c220bde41cc92800e22010"
                "045cfbf606c220bde41cc92e00e22010045cfbf606a5c42901f0045cfbf606b5200902952080005ac220bde41c"
                "c93200e220f0045c68f706c220a99cbc855fe22022172a7fb0045c68f706c00000f044c220a9c180991900e220"
                "a906991b00a901992d00a901992e00b92100090199210022aa2b7fb920000910992000a906991700a93c991300"
                "a97f991500b9200029f79920007a5ab42bb9a06a7a29f0c920d0045c96f7065ab42bb9a06a7a29f0c930d0045c"
                "96f706b516186905951622242c7fc220bde41cc93200e220f0045c0ef806c220a90000953295349536e2205ac2"
                "20a908be855fe22022172a7fb0045ceef706c220a91e7e991900e220a97f991b00c220a993f5992b00e220a90a"
                "992d00a90a992e0022aa2b7f7ac220a91000ecc312f00309008022096e7fe2202025f8c220a99cbc9504e220c2"
                "20bde41cc93100e220f00630045c24f8062219237f6b")
        self.source(0x06F512, f512)
        self.source(0x0680C1, "b52209089522a900950ac220a9d8809519e220a906951bb50ac96410045cf58006c220a9f5"
                              "809519e220a906951ba90095178000f60aacc31222be2b7fb50ac97f10045c0e8106b525"
                              "090895256b")
        # Target-control origin (the shared configuration's first step).
        self.source(0x07B79F, "5a08e220c210acc312dab62b9bfab98c6a29bf998c6ab98c6a2980f0045cfab707c220a902"
                              "00991c6cb50c99926ab50e99946ab51099966aa9ff0099246ca53a99906a1003a9010099"
                              "266c8a99986ae220a53c99296ca53e99286c287a6b")

    def test_defeat_action_stream_and_its_services(self):
        self.source(0x0DC585, "0000008cc8" "040000c6c8" "010000c0c9" "0008005cc7" "00000076c8"
                              "000000eacb" "001400cdcb" "004b007ac9" "ffff")
        self.source(0x0DC75C, "08a907999f6ac220a9179f999d6ae2205aa03f03b921000908992100b9210009"
                              "109921007a2860")
        self.source(0x0DCBEA, "08a90e22f86d7f2860")
        self.source(0x0DCBCD, "08ada61a8902d0045cdfcb0da90422f86d7f2860")
        self.source(0x0DC97A, "08ada61a8902d0045c8bc90da9028d781b2860")


if __name__ == "__main__":
    unittest.main()
