"""Bind native anchor choices to the active original strategy bodies."""

import re
import unittest

from test_strategy_source_contracts import ROOT, instructions, source


def body(name):
    tail = source("STRAT/PSTRATS.ASM").split(f"\n{name}\n", 1)[1]
    return tail.split("\n\ts_end_strat", 1)[0] + "\n\ts_end_strat"


class PlayerViewAnchorSourceTests(unittest.TestCase):
    def test_space_selects_only_inside_and_uses_immediate_not_live_center(self):
        code = instructions(body("playerinspace_strat"))
        self.assertEqual(code[0:2], ["s_start_strat", "jsr do_player_limitX"])
        self.assertEqual([line for line in code if line.startswith("s_jmp_varNE")], [
            "s_jmp_varNE B,splayerflymode,#spfm_inside,.ninsidex",
            "s_jmp_varNE B,splayerflymode,#spfm_inside,.ninsidey",
        ])
        self.assertEqual([line for line in code if line.startswith("jsl perc")], [
            "jsl perc75A_l", "jsl perc62A_l",
        ])
        self.assertEqual([line for line in code if line.startswith(("sbc ", "adc "))], [
            "sbc #Space_ViewCY", "adc #Space_ViewCY",
        ])
        self.assertEqual(code[-2:], ["jsr viewmove_srou", "s_end_strat"])

    def test_water_half_width_fixed_height_block_is_not_assembled(self):
        water = body("playeronwater_strat")
        disabled, active = water.split("\tifeq\t1", 1)[1].split("\tendc", 1)
        self.assertIn("asra", instructions(disabled))
        self.assertIn("lda #planet_ViewCY", instructions(disabled))
        # The compiled tail is byte-for-byte equivalent to the planet tail,
        # including the unconditional jump over the old close-view branch.
        self.assertEqual(instructions(active), instructions(body("playeronplanet_strat"))[1:])
        code = instructions(active)
        self.assertEqual(code[0], "jsr do_player_Yvel125")
        self.assertIn("jsl perc87A_l", code)
        self.assertEqual(code[code.index("jsl perc87A_l") + 1], "bra .donex")
        self.assertEqual([line for line in code if line.startswith(("sbc ", "adc "))], [
            "sbc viewcy", "adc viewcy",
        ])

    def test_underground_uses_live_fixed_center_and_scaled_horizontal_position(self):
        code = instructions(body("playerundergnd_strat"))
        self.assertEqual(code[:4], ["s_start_strat", "jsr do_playerYvelD2", "a16", "lda al_worldx,x"])
        self.assertIn("jsl perc87A_l", code)
        self.assertEqual(code[-5:], [
            "lda viewCY", "sta pviewposy", "a8", "jsr viewmove_srou", "s_end_strat",
        ])

    def test_percentage_helpers_keep_separately_rounded_arithmetic_shifts(self):
        code = source("STRAT/STRATROU.ASM")
        for name, next_name, expected in [
            ("perc62A_l", "perc75A_l", ["asra", "sta tpx", "asra", "asra", "clc", "adc tpx", "rtl"]),
            ("perc75A_l", "perc87A_l", ["asra", "sta tpx", "asra", "clc", "adc tpx", "rtl"]),
            ("perc87A_l", "perc93A_l", ["asra", "sta tpx", "asra", "sta tpy", "asra", "clc", "adc tpx", "clc", "adc tpy", "rtl"]),
        ]:
            self.assertEqual(instructions(code.split(f"\n{name}\n", 1)[1].split(f"\n{next_name}\n", 1)[0]), expected)

    def test_shipping_strategies_keep_movement_anchor_then_shared_depth_order(self):
        code = (ROOT / "rust/sf-strat/src/player.rs").read_text()
        for name, movement, mode in [
            ("player_in_space_strat", "do_player_limit_x", "Space"),
            ("player_on_water_strat", "do_player_yvel125", "Surface"),
            ("player_on_planet_body", "do_player_yvel125", "Surface"),
            ("player_undergnd_strat", "do_player_yvel_d2", "Underground"),
        ]:
            native = code.split(f"fn {name}(g: &mut Game, idx: u16) {{", 1)[1].split("\n}", 1)[0]
            self.assertEqual(re.sub(r"\s+", " ", native).strip(),
                             f"{movement}(g, idx); apply_flight_view_anchor(g, idx, FlightViewAnchor::{mode}); viewmove_srou(g, idx);")


if __name__ == "__main__":
    unittest.main()
