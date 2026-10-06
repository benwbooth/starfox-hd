"""Source contracts for helper pickup allocation, wing repair and flash visits."""
import unittest

from test_strategy_source_contracts import instructions, source


class PickupSourceTests(unittest.TestCase):
    def test_repair_pickup_failure_retains_actor_and_success_defers_the_new_ship(self):
        body = source("STRAT/GASTRATS.ASM").split("\nitem4_Istrat", 1)[1].split(";********", 1)[0]
        self.assertEqual(instructions(body), [
            "s_start_strat", "s_set_alptrs x,item4_strat,0,0", "s_set_alsflag x,colldisable",
            "s_end_strat", "item4_strat", "s_start_strat", "s_remove_ifplayerdead x",
            "s_add_alvar B,x,al_roty,#4", "s_add_alvar B,x,al_rotx,#4",
            "s_jsr itemtorange_srou", "s_add_alvar W,x,al_worldz,#20", "s_set_objtobeplayer y",
            "s_jmp_Zdistmore x,y,#60*2,.nitem", "s_jmp_XYdistmore x,y,#50*2,.nitem",
            "s_make_obj #ripair_w,.badobj", "s_set_strat y,ripair_Istrat", "s_remove_obj x",
            ".badobj", ".nitem s_end_strat",
        ])

    def test_repair_ship_complete_entry_motion_and_catch(self):
        body = source("STRAT/GASTRATS.ASM").split("\nripair_Istrat", 1)[1].split("\nflashplayer_Istrat", 1)[0]
        self.assertEqual(instructions(body), [
            "s_start_strat", "s_set_alptrs x,ripair_strat,0,0", "s_set_alsflag x,colldisable",
            "s_set_alvar W,x,al_worldz,player_posz", "s_add_alvar W,x,al_worldz,#-200",
            "s_set_alvar W,x,al_worldx,player_posx", "s_add_alvar W,x,al_worldx,#500",
            "s_set_alvar W,x,al_worldy,player_posy", "s_set_alsflag x,shadow",
            "s_setnoremove_behind x", "s_set_vecs x,#0,#0,#30", "s_set_alvar B,x,al_rotz,#deg90",
            "trigse $8b", "s_set_alvar B,x,al_sbyte1,#30", "s_end_strat",
            "ripair_strat", "s_start_strat", "s_set_objtobeplayer y", "s_add_playerZ x",
            "s_add_alvars W,x,al_worldz,x,al_vz",
            "s_achase_alvar W,x,al_worldx,player_posx,3", "s_achase_alvar W,x,al_worldy,player_posy,3",
            "s_decbne_alvar B,x,al_sbyte1,.nreccoll", "s_set_alvar B,x,al_sbyte1,#1",
            "s_achase_alvar W,x,al_worldx,player_posx,1", "s_achase_alvar W,x,al_worldy,player_posy,1",
            "s_achase_alvar2alvar B,x,al_rotz,y,al_rotz,1", "s_jmp_Zdistmore x,y,#500,.npos",
            "s_set_alvar W,x,al_worldx,player_posx", "s_set_alvar W,x,al_worldy,player_posy",
            "s_copy_alvar2alvar B,x,al_rotz,y,al_rotz", ".npos", "s_set_alvar W,x,al_vz,#-40",
            ".iszv s_setremove_behind x", ".nspd", "s_jmp_XYdistmore x,y,#20,.nreccoll",
            "s_jmp_objinfront y,x,.catch", "s_jmp_Zdistmore x,y,#30,.nreccoll",
            ".catch phx", "ldx pcboxobj_LW", "s_jsl pLWing_Istrat", "ldx pcboxobj_RW",
            "s_jsl pRWing_Istrat", "plx",
            "s_and_var B,pshipflags,#~(psf_brkLwing!psf_Lwingcoll!psf_brkRwing!psf_Rwingcoll)",
            "s_copy_pos x,y", "TRIGSE $17", "s_set_strat x,flashplayer_Istrat", ".nreccoll", "s_end_strat",
        ])

    def test_repair_carrying_enemy_falls_through_and_its_death_defers_ship_entry(self):
        body = source("STRAT/GASTRATS.ASM").split("\nripman_Istrat", 1)[1].split(";********", 1)[0]
        self.assertEqual(instructions(body), [
            "s_start_strat", "s_set_alptrs x,ripman_strat,hitflash_Istrat,ripmanexp_Istrat",
            "s_set_aldata x,#ripmanHP,#ripmanAP", "s_set_alsflag x,shadow", "s_set_colltype x,enemyweap",
            "ripman_strat", "s_start_strat", "s_jmp_lower x,#-30,.gnd",
            "s_add_alvar B,x,al_roty,#16", "s_add_alvar W,x,al_worldy,#3",
            "s_add_alvar W,x,al_worldx,#4", "s_add_alvar W,x,al_worldz,#35", ".gnd s_end_strat",
            "ripmanexp_Istrat", "s_start_strat", "TRIGSE $0a", "s_make_obj #ripair_w,.badobj",
            "s_set_strat y,ripair_Istrat", ".badobj", "s_jmp explode_Istrat",
        ])

    def test_laser_pickup_restores_wing_actors_without_repair_flag_clears(self):
        score = source("INC/STRATLIB.INC").split("\ns_score\t", 1)[1].split("\tENDM", 1)[0]
        self.assertEqual(instructions(score), ["MACRO [score]", "MYNARG = NARG", "CHK_NARG 1"])
        body = source("STRAT/GASTRATS.ASM").split("\nitem7_Istrat", 1)[1].split(";********", 1)[0]
        self.assertEqual(instructions(body), [
            "s_set_alptrs x,item7_strat,0,0", "s_set_alsflag x,colldisable", "item7_strat",
            "s_start_strat", "s_remove_ifplayerdead x", "s_jmp_alvarNOTZERO B,x,al_sbyte1,.stop",
            "s_add_alvar W,x,al_worldz,#20", ".stop", "s_jsr itemtorange_srou",
            "s_add_alvar B,x,al_roty,#4", "s_add_alvar B,x,al_rotz,#4", "s_set_objtobeplayer y",
            "s_jmp_Zdistmore x,y,#60*2,.nitem", "s_jmp_XYdistmore x,y,#30*2,.nitem",
            "s_jmpNOT_varAND B,pshipflags,#psf_brkLwing!psf_brkRwing,.dlaser",
            "s_make_obj #ripair_w,.cont", "s_set_strat y,ripair_Istrat", "s_brl .cont", ".dlaser",
            "TRIGSE $15", "s_score #100", "phx", "ldx pcboxobj_LW", "s_jsl pLWing_Istrat",
            "ldx pcboxobj_RW", "s_jsl pRWing_Istrat", "plx",
            "s_jmp_varAND B,pshipflags2,#psf2_doublaser,.dbeam", "s_or_var B,pshipflags2,#psf2_doublaser",
            "s_brl .cont", ".dbeam", "s_or_var B,pshipflags3,#psf3_beamball", ".cont",
            "s_set_strat x,flashplayer_Istrat", "s_jmpto_strat x", ".nitem s_end_strat",
        ])

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
