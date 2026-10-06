"""Source-bound birth-visit correction for the retained Houdai C regression.

No native output is an input. The legacy fixture already contains each shot's
source-proven constant velocity and the scenario's constant scroll (65).
Restoring l_add and initializer fall-through moves each shot once sooner and
retires it once sooner. The inverse recovers every original record and order.
"""

from pathlib import Path
import re
import unittest

from test_strategy_source_contracts import instructions, source

ROOT = Path(__file__).resolve().parents[2]


def frames(text):
    return [frame.splitlines(keepends=True)
            for frame in re.split(r"(?=^T )", text, flags=re.M) if frame]


def fields(line):
    return {key: int(value) for key, value in re.findall(r"(\w+)=(-?\d+)", line)}


def change(line, values):
    for field, value in values.items():
        line, count = re.subn(rf"(?<= ){field}=-?\d+\b", f"{field}={value}", line)
        assert count == 1, field
    return line


def shift_position(state, direction):
    return {axis: (state[axis] + direction * (state[velocity] + scroll) + 32768)
            % 65536 - 32768
            for axis, velocity, scroll in [("x", "vx", 0), ("y", "vy", 0), ("z", "vz", 65)]}


def advance_houdai_birth_visit(text):
    result = []
    for frame in frames(text):
        shots, others = [], []
        for line in frame[1:]:
            state = fields(line)
            if state["sh"] != 405:
                others.append(line)
                continue
            if state["hp"] == 0:
                assert state["cn"] == 0
                continue  # Source retirement happened one visit earlier.
            assert state["hp"] == 1 and 1 <= state["cn"] <= 30
            dead = state["cn"] == 1
            shots.append(change(line, {
                **shift_position(state, 1), "cn": state["cn"] - 1,
                "hp": 0 if dead else 1,
                "sf2": state["sf2"] | (1 if dead else 0), "cf": state["cf"] & ~4,
            }))
        assert [line.split()[1] for line in others] == ["2", "1", "0"]
        result.extend([frame[0], *others[:2], *shots, others[2]])
    return "".join(result)


def undo_houdai_birth_visit(text):
    result, previous_terminal = [], []
    for frame in frames(text):
        shots, others, terminal = [], [], []
        for line in frame[1:]:
            state = fields(line)
            if state["sh"] != 405:
                others.append(line)
                continue
            assert 0 <= state["cn"] <= 29
            if state["hp"] == 0:
                assert state["cn"] == 0 and state["sf2"] & 1
                terminal.append(line)
            shots.append(change(line, {
                **shift_position(state, -1), "cn": state["cn"] + 1,
                "hp": 1, "sf2": state["sf2"] & ~1,
                "cf": state["cf"] | (4 if state["cn"] == 29 else 0),
            }))
        # Legacy allocation inserted new shots at the head. Its extra terminal
        # record is exactly the source terminal record from the preceding visit.
        result.extend([frame[0], *shots, *previous_terminal, *others])
        previous_terminal = terminal
    return "".join(result)


class WeaponEntrySourceTests(unittest.TestCase):
    def test_flat_initializers_fall_through_without_a_return(self):
        weapons = source("STRAT/GSTRATS.ASM")
        for stem in ["relflatmiss", "flatmiss"]:
            body = weapons.split(f"\n{stem}_Istrat", 1)[1].split(f"\n{stem}_strat", 1)[0]
            self.assertEqual(instructions(body), [
                "s_start_strat", f"s_set_strat x,{stem}_strat",
                "s_copy_alvar2alvar B,x,al_sbyte1,x,al_roty",
                "s_copy_alvar2alvar B,x,al_sbyte2,x,al_rotx",
                "s_gen_3dvecs x,al_sbyte1,al_sbyte2,al_vel", "set_sound2 x,#6",
            ])
        relative = weapons.split("\nrelflatmiss_strat\n", 1)[1].split(";********", 1)[0]
        self.assertEqual(instructions(relative), [
            "s_start_strat", "s_add_playerZ x", "s_add_vecs2pos x",
            "s_dec_lifecnt x,1", "s_brl miss_end",
        ])
        fixed = weapons.split("\nflatmiss_strat\n", 1)[1].split(";********", 1)[0]
        self.assertEqual(instructions(fixed), [
            "s_start_strat", "s_rots_flat x", "s_add_vecs2pos x",
            "s_dec_lifecnt x,1", "s_brl miss_end",
        ])

    def test_constructors_install_initializers_and_insert_after_current(self):
        weapons = source("STRAT/GSTRATS.ASM")
        for fire, init in [
            ("plasma", "relflatmiss"), ("beamball", "flatmiss"),
            ("relovalbeam", "relflatmiss"), ("relringlaser", "relflatmiss"),
            ("ovalbeam", "flatmiss"), ("ringlaser", "flatmiss"),
            ("shortplasma", "relflatmiss"),
        ]:
            body = weapons.split(f"\nfire_{fire}", 1)[1].split(";********", 1)[0]
            self.assertIn(f"s_set_alptrs y,{init}_Istrat,weapcollide_Istrat,remove_Istrat",
                          instructions(body))
            self.assertIn("jsr gen_weapon", instructions(body))
            self.assertNotRegex(body, rf"\b(?:jsl|jsr|s_jmp)\s+{init}_Istrat")
        routines = source("STRAT/STRATROU.ASM")
        make = routines.split("\nsr_make_obj\t", 1)[1].split(";********", 1)[0]
        self.assertIn("jsl makeobj_l", instructions(make))
        allocator = routines.split("\nmakeobj_l\n", 1)[1].split(";********", 1)[0]
        self.assertIn("l_add allst,alfreelst,.badobj", instructions(allocator))
        add = source("INC/MACROS.INC").split("\nl_add\t", 1)[1].split("\n\tendm", 1)[0]
        self.assertIn(".NotStart\\@ lda.w _next,y", instructions(add))
        self.assertIn("stx _next,y", instructions(add))

    def test_houdai_correction_round_trips_without_native_data(self):
        corrected = (ROOT / "rust/sf-strat/tests/fixtures/ea_houdai.txt").read_text()
        legacy = undo_houdai_birth_visit(corrected)
        self.assertEqual(advance_houdai_birth_visit(legacy), corrected)
        # The four completed lifetimes retire a visit earlier. The fifth shot
        # remains live at the end of this bounded 120-visit scenario.
        self.assertEqual(sum(" sh=405 " in row for row in corrected.splitlines()), 141)
        self.assertEqual(sum(" sh=405 " in row for row in legacy.splitlines()), 145)


if __name__ == "__main__":
    unittest.main()
