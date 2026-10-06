"""Training's strategy handoff must not repeat the movement initializer."""

import unittest

from test_strategy_source_contracts import instructions, source


class TrainingHandoffSourceTests(unittest.TestCase):
    def test_training_queues_strategy_for_background_request_to_install(self):
        background = source("ASM/BGS.ASM").split("\nbg_training_1\t", 1)[1]
        self.assertEqual(instructions(background)[:7], [
            "init_bg mode2", "bg2chr stp", "bg2scr stp", "palette 2d",
            "gamepal blue", "set_bg", "pstrat playeronplanet,a,ab",
        ])
        macro = source("INC/BGMACS.INC").split("\npstrat\tmacro", 1)[1].split("\tendm", 1)[0]
        body = instructions(macro)[1:]
        self.assertEqual(body[:4], [
            "ldx #(\\1_istrat)&WM", "stx newplayerstrat",
            "lda #(\\1_istrat)>>16", "sta newplayerstrat+2",
        ])
        self.assertEqual([line for line in body if line.startswith(("jsl ", "jsr "))], [
            "jsl changeviewmode_l",
        ])
        install = source("ASM/WORLD.ASM").split("\nsetbginforeq_l\n", 1)[1].split("\n.nowrite", 1)[0]
        self.assertEqual(instructions(install), [
            "php", "ai16", "ldx playpt", "beq .nowrite",
            "lda newplayerstrat", "ora newplayerstrat+1", "beq .con534", "a8",
            "lda pshipflags3", "bit #psf3_keeppstrat", "bne .con534", "a16",
            "lda newplayerstrat", "sta al_stratptr,x", "lda newplayerstrat+1",
            "sta al_stratptr+1,x", "bra .con534",
        ])

    def test_movement_initialization_belongs_to_the_base_player_entry(self):
        text = source("STRAT/PSTRATS.ASM")
        initializer = text.split("\nplayer_Istrat\n", 1)[1].split(";********", 1)[0]
        body = instructions(initializer)
        self.assertEqual(body[:3], [
            "s_start_strat", "s_jsr playermove_init", "s_jsl playercred_Istrat",
        ])
        planet = text.split("\nplayeronplanet_Istrat\n", 1)[1].split("\nplayeronplanet_strat\n", 1)[0]
        self.assertEqual(instructions(planet), [
            "s_start_strat", "s_playerctrl on", "s_playerfly_mode planet",
            "s_set_alptrs x,playeronplanet_strat,playercoll_Istrat,playerdead_Istrat",
        ])


if __name__ == "__main__":
    unittest.main()
