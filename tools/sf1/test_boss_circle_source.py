"""Reversible, source-bounded correction of the retired-C boss-circle records.

This is not a whole-boss certification. Unrelated barrage and boss records stay
byte-identical (apart from the forced allocation-slot permutation). In particular
the separate boss-delay initializer still needs its own source-entry audit.
No native output is read by this transformation or its tests.
"""
import difflib
import hashlib
from pathlib import Path
import re
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "rust/sf-strat/tests/fixtures/ea_boss1.txt"
PRE_CIRCLE_HASH = "5009921170562501671c65543c78922a1088ef4593a7838ecc75bf3f951bb4e4"


def frames(text):
    result = []
    for line in text.splitlines(keepends=True):
        if line.startswith("T "):
            assert int(line.split()[1]) == len(result)
            result.append([line, []])
        else:
            assert line.startswith("O ")
            result[-1][1].append(line)
    assert len(result) == 120
    return result


def identity(line):
    return int(line.split()[1])


def edit(line, slot=None, **values):
    if slot is not None:
        line = re.sub(r"^O \d+", f"O {slot}", line)
    for name, value in values.items():
        line, count = re.subn(rf"(?<= ){name}=-?\d+\b", f"{name}={value}", line)
        assert count == 1, (name, line)
    return line


def record(rows, slot):
    matches = [line for line in rows if identity(line) == slot]
    assert len(matches) == 1, (slot, rows)
    return matches[0]


def migrate_circle(text, *, undo=False):
    trace = frames(text)
    # Source bossexplode creates count=35-20 at frame 70. Insert-after-current
    # and initializer fallthrough visit it at birth: 14..0, then expire at 85.
    birth, expiry, scroll, origin = 70, 85, 65, 7215
    copied_z = origin + scroll * (expiry - birth)
    assert "cn=14 " in record(trace[birth][1], 18) if undo else "cn=15 " in record(trace[birth][1], 18)
    proxy_template = record(trace[birth][1], 18)
    if not undo:
        # The old child/anchor at 86 supply only the unchanged zero/default
        # fields. Count, position, payload, flags and visit state come from
        # EXPSTRAT and the source allocation/dispatch contracts.
        emitter_template = record(trace[86][1], 19)
        anchor_template = record(trace[86][1], 11)
    output = []
    for tick, (header, rows) in enumerate(trace):
        if birth <= tick < expiry:
            proxy = record(rows, 18)
            rows = [line for line in rows if identity(line) != 18]
            if undo:
                proxy = edit(proxy, cn=85-tick, z=origin+scroll*(tick-birth), cf=4 if tick == birth else 0)
                rows.insert(0, proxy)
            else:
                proxy = edit(proxy, cn=84-tick, z=origin+scroll*(tick-birth+1), cf=0)
                rows.insert(next(i for i, line in enumerate(rows) if identity(line) == 1) + 1, proxy)
        elif tick >= expiry:
            if undo:
                emitter, anchor = record(rows, 11), record(rows, 12)
                rows = [line for line in rows if identity(line) not in {11, 12}]
                rows = [edit(line, slot={18: 12, 19: 18}.get(identity(line), identity(line))) for line in rows]
                if tick == expiry:
                    rows.insert(0, edit(proxy_template, cn=0, z=copied_z, cf=0))
                else:
                    emitter = edit(emitter, slot=19, cn=196-tick, z=copied_z+scroll*(tick-86), sf=0, cf=4 if tick == 86 else 0, sb1=0, sb2=0, sb3=0)
                    anchor = edit(anchor, slot=11, z=copied_z+scroll*(tick-86), sf2=0 if tick == 86 else 1, cf=4 if tick == 86 else 0)
                    rows = [emitter, anchor] + rows
            else:
                if tick == expiry:
                    rows = [line for line in rows if identity(line) != 18]
                    emitter, anchor = emitter_template, anchor_template
                else:
                    emitter, anchor = record(rows, 19), record(rows, 11)
                    rows = [line for line in rows if identity(line) not in {19, 11}]
                # Source l_add pops 12 then 11 before circle parent 18 retires.
                # The next burst sprite reuses 18 (was 12); the following one
                # takes 19 (was 18). These roles recycle in this permutation
                # throughout the remaining retained trace.
                rows = [edit(line, slot={12: 18, 18: 19}.get(identity(line), identity(line))) for line in rows]
                emitter = edit(emitter, slot=11, cn=tick-expiry, z=copied_z+scroll*(tick-expiry), sf=16, cf=0, sb1=255 if tick == expiry else 0, sb2=100 if tick == expiry else 0, sb3=4 if tick == expiry else 0)
                anchor = edit(anchor, slot=12, z=copied_z+scroll*(tick-expiry+1), sf2=1, cf=0)
                # Both are successors of boss 1. Its preceding sprite 3 stays
                # their predecessor after boss 1 finally retires at frame 116.
                predecessor = 1 if tick < 116 else 3
                insertion = next(i for i, line in enumerate(rows) if identity(line) == predecessor) + 1
                rows[insertion:insertion] = [emitter, anchor]
        output.extend([header, *rows])
    return "".join(output)


def undo_circle(name, fixture):
    return migrate_circle(fixture, undo=True) if name == "ea_boss1" else fixture


class BossCircleSourceTests(unittest.TestCase):
    def test_correction_is_reversible_to_the_complete_previous_fixture(self):
        current = FIXTURE.read_text()
        previous = migrate_circle(current, undo=True)
        self.assertEqual(hashlib.sha256(previous.encode()).hexdigest(), PRE_CIRCLE_HASH)
        self.assertEqual(migrate_circle(previous), current)

    def test_original_circle_entry_and_scheduling_contract(self):
        code = (ROOT / "reference/ultrastarfox/SF/STRAT/EXPSTRAT.ASM").read_text()
        body = code.split("\ncircdelayexplode_Istrat", 1)[1].split(";********", 1)[0]
        instructions = [re.sub(r"\s+", " ", line.strip()) for line in body.splitlines() if line.strip() and not line.lstrip().startswith(";")]
        self.assertEqual(instructions, [
            "s_start_strat", "s_hardvars x", "s_set_alptrs x,circdelayexplode_strat,0,0",
            "s_set_alsflag x,colldisable", "circdelayexplode_strat", "s_start_strat",
            "s_decbpl_lifecnt x,.nd", "s_jsl makebosscircexp_srou",
            "s_jmpNOT_alsflag x,sflag1,.badobj", "s_make_obj #nullshape,.badobj",
            "s_set_strat y,BIGparticleexplode_Istrat", "s_copy_pos y,x",
            "s_set_alsflag y,relexplode", ".badobj", "s_remove_obj x", ".nd",
            "s_add_playerZ x", "s_end_strat",
        ])
        macros = (ROOT / "reference/ultrastarfox/SF/INC/MACROS.INC").read_text()
        self.assertIn(";Add item to linked list after X", macros)
        particle = code.split("\nBIGparticleexplode_Istrat", 1)[1].split("\nBIGparticleexplode_strat", 1)[0]
        self.assertIn("s_particle_data\tx,4,255,100", particle)


if __name__ == "__main__":
    if sys.argv[1:] == ["--patch"]:
        previous = FIXTURE.read_text()
        assert hashlib.sha256(previous.encode()).hexdigest() == PRE_CIRCLE_HASH
        updated = migrate_circle(previous)
        assert migrate_circle(updated, undo=True) == previous
        diff = list(difflib.unified_diff(previous.splitlines(keepends=True), updated.splitlines(keepends=True)))
        print("*** Begin Patch\n*** Update File: " + str(FIXTURE))
        print("".join("@@\n" if line.startswith("@@") else line for line in diff[2:]), end="")
        print("*** End Patch")
    else:
        unittest.main()
