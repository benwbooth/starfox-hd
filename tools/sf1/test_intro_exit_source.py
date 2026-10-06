"""Source contract for the intro's controller/scripted exit and transfer fade."""

from pathlib import Path
import unittest

SOURCE = Path(__file__).resolve().parents[2] / "reference/ultrastarfox/SF/ASM"


def instructions(text):
    return [" ".join(line.split(";", 1)[0].split()).lower()
            for line in text.splitlines() if line.split(";", 1)[0].strip()]


def contains_in_order(test, lines, expected):
    previous = -1
    for instruction in expected:
        previous = lines.index(instruction, previous + 1)
    test.assertGreaterEqual(previous, 0)


class IntroExitSource(unittest.TestCase):
    def test_low_byte_frame_gate_precedes_scripted_and_controller_requests(self):
        source = (SOURCE / "ENDSEQ.ASM").read_text(encoding="latin1")
        intro = source.split("\nintro_l\n", 1)[1].split("\ntit_istrat", 1)[0]
        loop = instructions(intro.split(".lp3", 1)[1])
        contains_in_order(self, loop, [
            "jsl setblack_l", "jsl transfer_l", "a8", "lda gameframe", "cmp #30",
            "bcc .lp3", "lda exitintro", "bne .waitup", "lda cont0", "ora contl0",
            "beq .lp3", ".waitup",
        ])

    def test_seed_follows_previous_transfer_and_completion_is_checked_after_the_next(self):
        source = (SOURCE / "ENDSEQ.ASM").read_text(encoding="latin1")
        exit_loop = source.split("\nintro_l\n", 1)[1].split("\n.waitup\n", 1)[1]
        exit_loop = exit_loop.split("\ntit_istrat", 1)[0]
        contains_in_order(self, instructions(exit_loop), [
            "lda #-2", "sta fadedir", "lda #11", "sta fade", ".finishlp",
            "jsl setblack_l", "jsl transfer_l", "a8", "lda fadedir",
            "bne .finishlp", "rtl",
        ])
        self.assertNotIn("wm_val", exit_loop)

    def test_quick_fade_uses_two_brightness_decrements_and_forces_black_at_zero(self):
        source = (SOURCE / "IRQ.ASM").read_text(encoding="latin1")
        fade = source.split("setinidisp\tlda", 1)[1].split("\ngetcont0", 1)[0]
        contains_in_order(self, instructions(fade), [
            ".qfadedown lda fade", "beq .off", "dec a", "beq .off", "bra .setdown",
        ])
        contains_in_order(self, instructions(fade), [
            ".setdown beq .off", "dec a", "beq .off", "sta.l xinidisp1",
            "sta.l xinidisp2", "sta.l xinidisp1a", "sta fade", "bra .done",
            ".off lda #$80", "sta.l xinidisp1", "sta.l xinidisp2",
            "sta.l xinidisp1a", "stz fadedir", "stz fade",
        ])


if __name__ == "__main__":
    unittest.main()
