"""Independent source contract for the narrow ponpon trace correction.

The trace began as a retired C regression fixture, not cartridge evidence.
Only the ordinary-fire collision class is corrected here. Reversing that
single source-derived field must recover its exact pre-correction hash.
No native trace output is used to manufacture expectations.
"""

import hashlib
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "reference/ultrastarfox/SF"


class PathFireSourceTests(unittest.TestCase):
    def test_source_handlers_only_ordinary_fire_adds_enemy1(self):
        source = (SOURCE / "PATH/PATHS.ASM").read_text()
        for ordinary, shared in [("fire", "fireit"), ("fireatplayer", "fireitatplayer"),
                                 ("fireatshape", "fireitatshape")]:
            for suffix in ["", "canhit"]:
                body = source.split(f".{ordinary}{suffix}\tSHORTA\n", 1)[1].split(";**********", 1)[0]
                instructions = [re.sub(r"\s+", " ", line.strip()) for line in body.splitlines()
                                if line.strip()]
                expected = [f"jsr .{shared}"]
                if not suffix:
                    expected.append("s_set_colltype y,ENEMY1")
                expected.append("jmp .add1")
                self.assertEqual(instructions, expected)
        equates = (SOURCE / "INC/STRATEQU.INC").read_text()
        self.assertRegex(equates, r"acf_colltype2\s+equ\s+16\b")
        self.assertRegex(equates, r"colltype_enemy1\s+equ\s+acf_colltype2\b")

    def test_ponpon_uses_ordinary_fire(self):
        source = (SOURCE / "PATH/PATHDATA.ASM").read_text()
        body = source.split("START_PATH\tponpon", 1)[1].split("START_PATH", 1)[0]
        fire = [line.strip() for line in body.splitlines()
                if re.match(r"\s*P_FIRE\b", line)]
        self.assertEqual(fire, ["P_FIRE", "P_FIRE", "P_FIRE"])

    def test_fixture_changes_only_post_constructor_projectile_class(self):
        path = ROOT / "rust/sf-path/tests/fixtures/pi_ponpon.txt"
        lines = path.read_text().splitlines(keepends=True)
        original = []
        changed = 0
        for line in lines:
            if line.startswith("A 3 "):
                self.assertIn(" cf=80 ", line)
                line = line.replace(" cf=80 ", " cf=64 ")
                changed += 1
            original.append(line)
        self.assertEqual(changed, 225)  # T15 through T239; the host never ticks shots.
        self.assertEqual(
            hashlib.sha256("".join(original).encode()).hexdigest(),
            "9b9c4bfe9826d5d5bef75c96605eb56765184d658dde965aec580d6bb87bfa3b",
        )


if __name__ == "__main__":
    unittest.main()
