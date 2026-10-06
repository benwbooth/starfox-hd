"""Source-layout migration audit for retained C traces, not gameplay parity.

Only the listed actor roles can change. This deliberately does not reinterpret
every first-byte bit 4: the boss2 particle emitter must keep its real partobj bit.
The reversible hashes certify every other byte against the pre-migration files.
No expected value is obtained from the native implementation.
"""

from collections import Counter
import hashlib
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "rust/sf-strat/tests/fixtures"
SOURCE = ROOT / "reference/ultrastarfox/SF"

# These masks are derived from STRATEQU.INC make_sflag ordering. The legacy C
# layout used first-byte 0x10/0x40, colliding with partobj/textobj. Actual source
# colldisable is second-byte 0x01 and nohitaffect is third-byte 0x20.
COLLISION_SHAPES = {
    "ea_boss1": {"16", "0", "462"},
    "eb_bossa": {"56"},
    "eb_spacepilon": {"615"},
    "bo_boss2": {"255", "260"},
}
IMMUNE_SHAPES = {
    "ea_boss1": {"437"},
    "eb_boss7": {"56", "421", "424", "425"},
    "eb_bossa": {"56", "426"},
    "eb_bossf": {"107"},
}
EXPECTED_CHANGES = {
    "ea_boss1": {"collision": 164, "immunity": 524},
    "eb_boss7": {"immunity": 482},
    "eb_bossa": {"collision": 450, "immunity": 900},
    "eb_bossf": {"immunity": 150},
    "eb_spacepilon": {"collision": 39},
    "bo_boss2": {"collision": 300},
    "sp_player_trace": {"nucleus": 100},
}
PRE_LAYOUT_HASHES = {
    "ea_boss1": "a9cea38870e3366b7a7b7f206cd99b3d8ab1a03cbbda863457cebd60df6daed7",
    "eb_boss7": "202cfda6e27fe838d80b354f0b5caa663ea54136e7c8e21a0201cdd703cb5fcb",
    "eb_bossa": "c82eab8bbe54ea2ea7f646167d64d38b923d6ff436a3a15e4f77bbd719cf1cb3",
    "eb_bossf": "3a725a35eccb3f4ee9e019f469dbf2307a464b885d86853f00bf40f981f1b3bd",
    "eb_spacepilon": "6b5ac2587fa9cfc0669ee74cd3ff23f6f464c21fa63f7ffe5c3d6feec5805f48",
    "bo_boss2": "d30ba3fe87f877ada6e676169d519e89f52e01055ba33b73dbd9b893deaae35a",
    "sp_player_trace": "373a3079b0002ea978bde904ffa72e998dfa0efdce9006d946744cfeaad66654",
}


def change_field(line, name, value):
    return re.sub(rf"(?<= ){name}=\d+\b", f"{name}={value}", line)


def migrate_flag_layout(name, fixture, *, undo=False):
    """Translate reviewed flag fields only, retaining all other trace bytes."""
    changed = Counter()
    output = []
    scenario = None
    for line in fixture.splitlines(keepends=True):
        if line.startswith("== "):
            scenario = line.strip()
        if name == "sp_player_trace" and scenario == "== nucleus ==" and line.startswith("T"):
            # The player is already collision-disabled by its movement setup;
            # playerEscapeNucleus_Istrat must not add a first-byte particle bit.
            before, after = (8, 24) if undo else (24, 8)
            columns = line.split(" ")
            assert (int(columns[12]), int(columns[13])) == (before, 1)
            columns[12] = str(after)
            line = " ".join(columns)
            changed["nucleus"] += 1
        elif re.match(r"(?:O \d+ |T\d+ A\d+ )", line):
            values = dict(re.findall(r"(\w+)=(\S+)", line))
            shape = values["sh"]
            prefix = "s" if name.startswith("bo_") else "sf"
            first, second, third = (int(values[key]) for key in ("sf", f"{prefix}2", f"{prefix}3"))
            collision_role = shape in COLLISION_SHAPES.get(name, set())
            if name == "ea_boss1" and shape == "0":
                # Exclude ordinary stayrel camera anchors, whose second-byte
                # colldisable was already correctly represented. The retained
                # delayed-circle child has AFEXP, while the copied circle
                # countdown has realobj cleared. This audit does not certify
                # that legacy delayed child as a complete particle producer.
                collision_role = int(values["fl"]) & 1 != 0 or third == 0
            if name == "ea_boss1" and shape == "462":
                # This explosion sprite copies the dying boss's flags and
                # already had second-byte colldisable before the migration.
                collision_role = second & 0xC0 == 0xC0
            if collision_role and (second & 1 if undo else first & 0x10):
                inherited_collision = name == "ea_boss1" and shape == "462"
                if undo:
                    assert first & 0x10 == 0
                    first |= 0x10
                    if not inherited_collision:
                        second &= ~1
                else:
                    assert second & 1 == int(inherited_collision)
                    first &= ~0x10
                    second |= 1
                changed["collision"] += 1
            if shape in IMMUNE_SHAPES.get(name, set()) and (third & 0x20 if undo else first & 0x40):
                if undo:
                    assert first & 0x40 == 0
                    first |= 0x40
                    third &= ~0x20
                else:
                    assert third & 0x20 == 0
                    first &= ~0x40
                    third |= 0x20
                changed["immunity"] += 1
            for key, value in [("sf", first), (f"{prefix}2", second), (f"{prefix}3", third)]:
                line = change_field(line, key, value)
        output.append(line)
    assert dict(changed) == EXPECTED_CHANGES.get(name, {}), (name, changed)
    return "".join(output)


def undo_flag_layout(name, fixture):
    return migrate_flag_layout(name, fixture, undo=True)


class StrategyFlagLayoutTests(unittest.TestCase):
    def test_every_nonflag_byte_survives_the_layout_migration(self):
        for name, expected in PRE_LAYOUT_HASHES.items():
            with self.subTest(fixture=name):
                current = (FIXTURES / f"{name}.txt").read_text()
                previous = undo_flag_layout(name, current)
                self.assertEqual(hashlib.sha256(previous.encode()).hexdigest(), expected)
                self.assertEqual(migrate_flag_layout(name, previous), current)

    def test_reviewed_flag_writers_remain_in_the_original_routines(self):
        def instructions(text):
            return [re.sub(r"\s+", " ", line.split(";", 1)[0].strip())
                    for line in text.splitlines() if line.split(";", 1)[0].strip()]

        for path, start, end, flag in [
            ("STRAT/GBSTRATS.ASM", "boss1_Istrat", "boss1up_strat", "colldisable"),
            ("STRAT/GBSTRATS.ASM", "boss1turret_init", "boss1turretL_strat", "nohitaffect"),
            ("STRAT/GBSTRATS.ASM", "boss2_Istrat", "boss2_strat", "colldisable"),
            ("STRAT/GBSTRATS.ASM", "boss2top_Istrat", "boss2top_strat", "colldisable"),
            ("STRAT/GB2STRAT.ASM", "bossF_Istrat", "bossFC_strat", "nohitaffect"),
            ("STRAT/GB3STRAT.ASM", "boss7_Istrat", "boss7a_strat", "nohitaffect"),
            ("STRAT/GB3STRAT.ASM", "boss7hatch_strat", "boss7hatchcol_Istrat", "nohitaffect"),
            ("STRAT/GB3STRAT.ASM", "boss7launcher_cont", "boss7launchercol_Istrat", "nohitaffect"),
            ("STRAT/GB3STRAT.ASM", "bossAcupper_Icont", ".bossAcupper_strat", "nohitaffect"),
            ("STRAT/GB3STRAT.ASM", "bossAturret_Icont", "bossAturretR_strat", "colldisable"),
            ("STRAT/GB3STRAT.ASM", "bossAturret_Icont", "bossAturretR_strat", "nohitaffect"),
            ("STRAT/GA3STRAT.ASM", "spacepilon_strat", "spacepiloncol_Istrat", "colldisable"),
            ("STRAT/EXPSTRAT.ASM", "circdelayexplode_Istrat", "circdelayexplode_strat", "colldisable"),
            ("STRAT/PCSTRATS.ASM", "playerEscapeNucleus_Istrat", "playerEscapeNucleus_strat", "colldisable"),
        ]:
            code = (SOURCE / path).read_text()
            body = re.split(rf"^{re.escape(start)}\b", code, maxsplit=1, flags=re.M)[1]
            body = re.split(rf"^{re.escape(end)}\b", body, maxsplit=1, flags=re.M)[0]
            self.assertIn(f"s_set_alsflag x,{flag}", instructions(body), (path, start))


if __name__ == "__main__":
    unittest.main()
