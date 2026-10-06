"""Source branches behind the body/wing collision differential tests."""
import unittest

from test_strategy_source_contracts import instructions, source


class PlayerCollisionSourceTests(unittest.TestCase):
    def test_damage_clamps_by_wrapped_sign_and_scaling_is_arithmetic(self):
        body = source("STRAT/STRATROU.ASM").split("\ndo_coll_l\n", 1)[1].split(";********", 1)[0]
        self.assertEqual(instructions(body), [
            "s_decbne_alvar B,x,al_collcount,.skip", ".f2",
            "s_jmpNOT_varAND B,pshipflags3,#psf3_intunnel,.ntun",
            "lda x1", "cmp #hardAP", "bne .nhard", "asra", "sta x1",
            ".nhard", ".ntun", "LDA AL_HP,X", "BMI .o2c", "SEC", "SBC X1",
            "BPL .nnhc", "LDA #0", ".nnhc STA AL_HP,X",
            ".o2c s_set_alvar B,x,al_collcount,tpa", ".skip", "rtl",
        ])
        shift = source("INC/MACROS.INC").split("\nasra\tmacro", 1)[1].split("\tendm", 1)[0]
        self.assertEqual(instructions(shift), ["cmp #(longa&$7f80)+128", "ror a"])
        macro = source("INC/STRATMAC.INC").split("\ns_docoll\t", 1)[1].split("\tENDM", 1)[0]
        self.assertIn("REPT \\3\nasra\nENDR", "\n".join(instructions(macro)))

    def test_broken_wing_wall_uses_explicit_body_damage_and_common_tail(self):
        body = source("STRAT/PSTRATS.ASM").split("\nbrkpwingcol", 1)[1].split("\npwingcol", 1)[0]
        self.assertEqual(instructions(body), [
            "ldy pcboxobj_B", "s_copy_alvar2alvar.w W,y,al_collobjptr,x,al_collobjptr",
            "s_set_alsflag y,collide", "s_jmp_alvarZERO W,x,al_collobjptr,.bodywallcol",
            "brl no_pwingcol", ".bodywallcol", "s_docollAP y,#framesperAP,#4",
            "brl no_pwingcol",
        ])
        tail = source("STRAT/PSTRATS.ASM").split("\nno_pwingcol", 1)[1].split(";********", 1)[0]
        self.assertEqual(instructions(tail), [
            "s_do_strat x", "s_set_objtobealvar y,x,al_sword1",
            "s_chk_objptr y,.badobj", "s_copy_pos y,x", "jsr sgenspark_srou",
            ".badobj", "s_end_strat",
        ])

    def test_wing_entry_skips_object_recoil_on_wall_and_impact_cue_when_wire(self):
        player = source("STRAT/PSTRATS.ASM")
        for side in ["L", "R"]:
            body = player.split(f"\npcol{side}W_Istrat", 1)[1].split(f"\npcol{side}W_strat", 1)[0]
            ops = instructions(body)
            # Wire branch goes past the impact sounds, while wall/null goes
            # past both the AP check and the entire object-only recoil block.
            self.assertLess(ops.index("s_jmp_varAND B,pshipflags2,#psf2_wireship,.dsn"), ops.index("TRIGSE $04"))
            self.assertGreater(ops.index(".dsn"), ops.index("TRIGSE $04"))
            null = ops.index("s_chk_objptr y,.bady")
            recoil = ops.index("s_set_objtobeplayer y")
            ready = ops.index(".bady")
            self.assertLess(null, recoil)
            self.assertLess(recoil, ready)
            self.assertEqual(ops[ready + 1], f"s_set_collstrat x,pcol{side}W_strat")


if __name__ == "__main__":
    unittest.main()
