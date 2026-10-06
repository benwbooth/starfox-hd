"""Source contracts for helper pickup allocation, wing repair and flash visits."""
import unittest

from test_strategy_source_contracts import instructions, source


class PickupSourceTests(unittest.TestCase):
    def test_death_marks_removal_without_returning(self):
        macros = source("INC/STRATMAC.INC")
        remove = macros.split("\ns_remove_ifplayerdead\t", 1)[1].split("\tENDM", 1)[0]
        self.assertEqual(instructions(remove)[4:], [
            "lda pshipflags2", "and #psf2_playerHP0", "beq .ok\\@",
            "s_remove_obj \\1", ".ok\\@",
        ])
        marker = macros.split("\ns_doremove_obj\t", 1)[1].split("\tENDM", 1)[0]
        self.assertIn("inc aldead", instructions(marker))
        fire = macros.split("\ns_remove_fire\t", 1)[1].split("\tENDM", 1)[0]
        body = instructions(fire)
        self.assertLess(body.index("beq .bf\\@"), body.index("s_clr_alflag x,onfire"))
        self.assertLess(body.index("sta.l alx_fireobjptr,x"), body.index("jsl removedeadal_l"))

    def test_complete_helpball_pickup_orders_failure_and_inline_flash(self):
        body = source("STRAT/GASTRATS.ASM").split("\nitem7a_Istrat", 1)[1].split(";********", 1)[0]
        self.assertEqual(instructions(body), [
            "s_set_alptrs x,item7a_strat,0,0", "s_set_alsflag x,colldisable",
            "item7a_strat", "s_start_strat", "s_remove_ifplayerdead x",
            "s_jmp_alvarNOTZERO B,x,al_sbyte1,.stop", "s_add_alvar W,x,al_worldz,#20",
            ".stop", "s_jsr itemtorange_srou", "s_add_alvar B,x,al_roty,#4",
            "s_add_alvar B,x,al_rotz,#4", "s_set_objtobeplayer y",
            "s_jmp_Zdistmore x,y,#60*2,.nitem", "s_jmp_XYdistmore x,y,#30*2,.nitem",
            "s_make_obj #helpball,.bad1", "s_set_strat y,helpball_Istrat", "TRIGSE $10",
            "phx", "ldx pcboxobj_LW", "s_jsl pLWing_Istrat", "ldx pcboxobj_RW",
            "s_jsl pRWing_Istrat", "plx", ".bad1", "s_set_strat x,flashplayer_Istrat",
            "s_jmpto_strat x", ".nitem s_end_strat",
        ])

    def test_wing_initializers_preserve_flags_and_do_not_consume_transfer_scratch(self):
        player = source("STRAT/PSTRATS.ASM")
        for side in ["L", "R"]:
            body = player.split(f"\np{side}Wing_Istrat\n", 1)[1].split(f"\np{side}Wing_strat", 1)[0]
            expected = ["s_start_strat", "s_setnoremove_behind x",
                f"s_set_alptrs x,p{side}Wing_strat,pcol{side}W_Istrat,P{side}Wbrk_Istrat",
                "s_set_aldata x,#playerW_HP,#playerW_AP"]
            if side == "L":
                expected += ["s_set_state x,0"]
            expected += [f"s_set_vartobeobj pcboxobj_{side}W,x",
                         "s_set_alsflag x,colldisable", "s_end_strat"]
            self.assertEqual(instructions(body), expected)
        # That lone assignment loads address zero (= trans_flag), rather
        # than immediate #0. It has no consumer in the complete player or
        # common strategy graph; the native port deliberately omits it.
        self.assertEqual([op for op in instructions(player) if "state" in op.lower()],
                         ["s_set_state x,0"])
        self.assertNotIn("al_stratstate", source("STRAT/STRATROU.ASM"))
        self.assertIn("zalc trans_flag,1", instructions(source("INC/ALCS.INC")))

    def test_complete_flash_fallthrough_animation_and_lifetime_order(self):
        body = source("STRAT/GASTRATS.ASM").split("\nflashplayer_Istrat\n", 1)[1].split("\nitemtorange_srou", 1)[0]
        self.assertEqual(instructions(body), [
            "s_start_strat", "s_jmp_varEQ B,splayerflymode,#spfm_inside,remove_Istrat",
            "s_set_lifecnt x,#20", "s_set_alsflag x,colldisable",
            "s_set_strat x,flashplayer_strat", "flashplayer_strat", "s_start_strat",
            "s_set_objtobeplayer y", "s_copy_rots x,y", "s_copy_pos x,y",
            "s_remove_ifplayerdead x", "s_jmp_notdelay 1,.dowire",
            "s_set_alvar W,x,al_shape,#nullshape", "brl .endpfl", ".dowire",
            "exg_xy", "s_ldajsl B,#pshipnum_wire,setYplayershape_l", "exg_xy",
            "s_add_colanim x,#1,#4", ".endpfl", "s_dec_lifecnt x", "s_end_strat",
        ])
        repair = source("STRAT/GASTRATS.ASM").split("\n.catch", 1)[1].split("\nflashplayer_Istrat\n", 1)[0]
        self.assertEqual(instructions(repair)[-3:], [
            "s_set_strat x,flashplayer_Istrat", ".nreccoll", "s_end_strat"])


if __name__ == "__main__":
    unittest.main()
