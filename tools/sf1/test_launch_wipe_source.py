"""Contracts for map-owned launch apertures and their transfer boundaries."""

from pathlib import Path
import unittest

SOURCE = Path(__file__).resolve().parents[2] / "reference/ultrastarfox/SF"


def source(relative):
    return (SOURCE / relative).read_text(encoding="latin1")


def instructions(text):
    return [" ".join(line.split(";", 1)[0].split()).lower()
            for line in text.splitlines() if line.split(";", 1)[0].strip()]


def contains_in_order(test, lines, expected):
    previous = -1
    for instruction in expected:
        previous = lines.index(instruction, previous + 1)
    test.assertGreaterEqual(previous, 0)


class LaunchWipeSource(unittest.TestCase):
    def test_launch_initializer_remainder_is_map_owned_and_has_no_stage_waits(self):
        macros = source("INC/MAPMACS.INC")
        wipe = macros.split("wipein\tmacro", 1)[1].split("\tendm", 1)[0]
        contains_in_order(self, instructions(wipe), [
            "mapcode_jsl initblack_l", "setvar.b stayblack,30", "mapwait 300",
            r"setvar.w circleanim,\1", "setvar.b stayblack,-1",
        ])
        initializer = macros.split("initlevel\tMACRO", 1)[1]
        launch = initializer.split(r"IFEQ\til_\2-il_nofadenostage".replace("\\t", "\t"), 1)[1]
        launch = launch.split("\tMEXIT", 1)[0]
        contains_in_order(self, instructions(launch), [
            r"wipein \3", "mapcode_jsl initblack_l", "setvar.b stayblack,2",
        ])
        self.assertNotIn("mapwait", launch)
        self.assertNotIn("setstage", launch)
        for filename in ["LEVEL1_1.ASM", "LEVEL2_1.ASM", "LEVEL3_1.ASM"]:
            text = source("MAPS/" + filename)
            self.assertIn("nofadenostage,mstarwipe_circle", text)
            self.assertIn("wipein\tmscramwipe_circle", text)

    def test_transfer_prepares_before_strategies_and_renders_after_sprites(self):
        transfer = source("ASM/TRANS.ASM").split("transfer_l\n", 1)[1]
        contains_in_order(self, instructions(transfer), [
            "jsr do_circle_explosion", "jsr dostrats", "jsl build_drawlist_l",
            "jsl do_sprites_l", ".skip jsl do_3d_display_l", "jsr do_window_wipe",
        ])
        cleanup = source("ASM/TRANS.ASM").split("wipeend_do\tstz", 1)[1].split("\trts", 1)[0]
        self.assertIn("circleanim", cleanup)
        self.assertIn("dealloc_window wipe", " ".join(cleanup.split()))
        self.assertNotIn("doingwipe", cleanup)
        idle = source("ASM/TRANS.ASM").split("do_circle_explosion\n", 2)[2].split("circle_com\ta16", 1)[0]
        self.assertIn("stz\tdoingwipe", idle)

    def test_gameplay_initializer_preserves_the_controller_once_flag(self):
        controller = instructions(source("ASM/CONT.ASM"))
        contains_in_order(self, controller, ["lda #1", "sta.l oncewipe"])
        main = source("ASM/MAIN.ASM")
        initializer = main.split("initgame_l", 1)[1].split("initgame3d_l", 1)[0]
        self.assertNotIn("oncewipe", initializer)

    def test_aperture_has_priority_over_black_color_window(self):
        windows = instructions(source("INC/STRUCTS.INC"))
        self.assertLess(windows.index("defwindow wipe"), windows.index("defwindow blackfade"))
        transfer = source("ASM/TRANS.ASM").split("find_window_pri\n", 2)[2].split("calcbg2voffsets_l", 1)[0]
        contains_in_order(self, instructions(transfer), ["ror tpa", "bcs .found"])
        irq = source("ASM/IRQ.ASM").split("\n.wipe\n", 1)[1].split("\trts", 1)[0]
        contains_in_order(self, instructions(irq), ["lda #%00001011", "sta wobjsel"])
        self.assertNotIn("sta\tcoldata", irq)


if __name__ == "__main__":
    unittest.main()
