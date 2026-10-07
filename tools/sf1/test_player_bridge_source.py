"""Bind bridge-clear timing and the replacement oracle to original source."""

import re
import unittest

from test_strategy_source_contracts import ROOT, instructions, source


class PlayerBridgeSourceTests(unittest.TestCase):
    def test_bridge_publishes_half_velocity_after_motion_without_unlocking(self):
        tail = re.split(r"\nplayeronBridge_strat[ \t]*\n", source("STRAT/PSTRATS.ASM"), maxsplit=1)[1]
        code = instructions(tail.split("\n\ts_end_strat", 1)[0] + "\n\ts_end_strat")
        self.assertEqual(code[:6], ["s_start_strat", "jsr do_player_bridge", "a16",
                                   "lda al_vy,x", "adiv2", "sta al_vy,x"])
        self.assertFalse(any("pshipflags" in line or "pstratflags" in line for line in code))
        self.assertIn("jsl perc62A_l", code)
        self.assertEqual(code[-5:], ["lda ViewCY", "sta pviewposy", "a8",
                                    "jsr viewmove_srou", "s_end_strat"])

    def test_bridge_movement_is_identical_to_half_height_movement(self):
        text = source("STRAT/PSTRATS.ASM")
        def routine(name):
            return instructions(text.split(f"\n{name}\n", 1)[1].split("\n\trts", 1)[0])
        self.assertEqual(routine("do_player_bridge"), routine("do_playerYvelD2"))

    def test_map_callback_only_installs_and_initializer_falls_through(self):
        text = source("STRAT/PCSTRATS.ASM")
        callback = text.split("\nset_playerClearbridge_l\n", 1)[1].split("\nplayerclearbridge_Istrat", 1)[0]
        self.assertEqual(instructions(callback), ["a8i16", "s_set_objtobeplayer x",
                         "s_set_strat x,playerClearbridge_Istrat", "rtl"])
        initializer = text.split("\nplayerclearbridge_Istrat\n", 1)[1].split("\nplayerclearbridge_strat", 1)[0]
        code = instructions(initializer)
        self.assertEqual(code[-1], "s_set_var B,psvar_byte1,#125+38")
        self.assertNotIn("s_end_strat", code)
        self.assertIn("s_playerctrl off", code)
        self.assertIn("s_or_var B,pstratflags,#pstf_inseq", code)

    def test_boost_handoff_issues_sound_without_replacing_retained_offset(self):
        text = source("STRAT/GCSTRATS.ASM")
        entry = text.split("\nclshipboost_Istrat\n", 1)[1].split("\nclshipboost_strat\n", 1)[0]
        code = instructions(entry)
        self.assertIn("trigse $32", code)
        self.assertIn("boost_sprite", code)
        self.assertEqual(code[-1], "s_set_speed x,#120")
        self.assertNotIn("boostZoff", entry)
        self.assertNotIn("al_snd2", entry)

    def test_replacement_retains_independent_original_and_scheduler_gates(self):
        oracle = (ROOT / "rust/sf-oracle/tests/sf1_player_bridge.rs").read_text()
        scheduler = (ROOT / "rust/sf-strat/tests/player_bridge_clear.rs").read_text()
        self.assertIn('load_built_rom().expect(', oracle)
        self.assertIn('original.names["REMOVEDEADAL_L"]', oracle)
        self.assertIn('original.names["SDPORT3"]', oracle)
        self.assertIn('original.compare_links', oracle)
        self.assertIn('game.run_strategies()', scheduler)
        self.assertIn('game.map_exec()', scheduler)
        self.assertNotRegex(oracle + scheduler, r"#\[ignore\b|SF_BLESS")


if __name__ == "__main__":
    unittest.main()
