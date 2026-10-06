"""Source assertions and reversible audits for retired-C strategy fixtures.

These tests are not whole-game parity. The fixture hashes preserve every
unreviewed record: only the explicitly sourced fields below may differ from
the seven pre-audit files. Expected data is never taken from native output.
"""

import hashlib
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "reference/ultrastarfox/SF"
FIXTURES = ROOT / "rust/sf-strat/tests/fixtures"
ORIGINAL_HASHES = {
    "ea_rader0": "f660df3f335eb3bb9ce6cb863ff584a16e8f286606ff54746e7a3208d5dd4839",
    "ea_zaco1": "1492e70668859a1e560d0abae4260dee74da93d2bdb5d0baed5b3c7bd55c123e",
    "ea_houdai": "55ce8d639a1b5abcb3704e2c34f4e4f18e38053cf1e7e20ff92fa47cfce644c7",
    "ea_boss1": "717830516323c3e40fe1a70b1994c2b75aa544d9d166fc436167af5facd6f092",
    "eb_bossf": "3f6fbd6beaabf8c503d26036fea01454ab2a4176f15450f8ac59d9da4bc88305",
    "eb_spacepilon": "644d98f5c5034bb7ff598a172fb74d190c2b450cec52572be4d8aacfbf695045",
    "bo_boss2": "7c5540286cdbcc4fdcf9618963fcd367a0d3f3713649780782cc357675db45b6",
}


def source(path):
    return (SOURCE / path).read_text()


def instructions(text):
    return [re.sub(r"\s+", " ", line.split(";", 1)[0].strip())
            for line in text.splitlines() if line.split(";", 1)[0].strip()]


def replace_field(line, name, transform):
    return re.sub(rf"(?<= ){name}=(\d+)\b",
                  lambda match: f"{name}={transform(int(match[1]))}", line)


class StrategySourceContractTests(unittest.TestCase):
    def test_source_flag_byte_and_mask_assignments(self):
        flags = re.findall(r"^[ \t]+make_sflag[ \t]+(\w+)", source("INC/STRATEQU.INC"), re.M)
        self.assertEqual(len(flags), 32)
        for name, byte, mask in [("ssprite", 1, 32), ("colldisable", 2, 1),
                                 ("smflag1", 2, 4), ("nopolyexp", 3, 64),
                                 ("relexplode", 4, 4)]:
            index = flags.index(name)
            self.assertEqual((index // 8 + 1, 1 << (index % 8)), (byte, mask), name)
        sprite = source("INC/STRATLIB.INC").split("s_sprite_obj\tMACRO", 1)[1].split("ENDM", 1)[0]
        self.assertIn("s_set_alsflag {obj},ssprite", instructions(sprite))

    def test_radar_initializer_falls_through_to_rotation(self):
        radar = source("STRAT/GASTRATS.ASM").split("\nrader0_Istrat\n", 1)[1]
        radar = radar.split(";*****************************************************************************", 1)[0]
        self.assertEqual(instructions(radar), [
            "s_start_strat",
            "s_set_alptrs x,rader0_strat,hitflash_Istrat,explode_Istrat",
            "s_set_aldata x,#raderHP,#raderAP",
            "s_set_colltype x,enemy1", "s_set_colltype x,Zenemy",
            "rader0_strat", "s_start_strat", "s_add_alvar B,x,al_roty,#8", "s_end_strat",
        ])

    def test_homing_constructor_preserves_firer_speed(self):
        weapons = source("STRAT/GSTRATS.ASM")
        constructor = weapons.split("\nfire_relslowElaserHome ", 1)[1].split(";********", 1)[0]
        self.assertIn("jsr gen_weapon", instructions(constructor))
        common = weapons.split("\ngen_weapon\n", 1)[1].split("\n\trts", 1)[0]
        self.assertIn("s_copy_alvar2alvar B,y,al_sbyte3,x,al_vel", instructions(common))
        # This laser retains the field but does not add mother speed to motion.
        homing = weapons.split("\nrelelaserhome_strat\n", 1)[1].split(";********", 1)[0]
        self.assertNotIn("al_sbyte3", "\n".join(instructions(homing)))

    def test_flat_projectile_and_muzzle_flash_are_source_sprites(self):
        weapons = source("STRAT/GSTRATS.ASM")
        for start, end in [("fire_plasma", "fire_beamball"),
                           ("flash_Istrat", "flash_strat")]:
            body = weapons.split(f"\n{start}", 1)[1].split(f"\n{end}", 1)[0]
            self.assertRegex("\n".join(instructions(body)), r"s_sprite_obj [xy],#0")

    def test_lifetime_decrement_stores_zero_and_kill_does_not_return(self):
        macros = source("INC/STRATMAC.INC")
        lifetime = macros.split("\ns_dec_lifecnt\t", 1)[1].split("ENDM", 1)[0]
        self.assertIn("s_decbne_alvar B,\\1,al_count,.\\@", instructions(lifetime))
        decrement = macros.split("\ns_decbne_alvar\t", 1)[1].split("ENDM", 1)[0]
        self.assertLess(decrement.index("dec\t\\3,x"), decrement.index("rlbne\t\\4"))
        kill = macros.split("\ns_kill_obj\t", 1)[1].split("ENDM", 1)[0]
        self.assertEqual(instructions(kill)[-4:], [
            "s_set_alsflag \\1,colldisable", "lda #0", "s_sta.w al_HP,\\1", "stratmac_end",
        ])

    def test_explosion_sprite_preserves_fourth_flag_byte(self):
        explode = source("STRAT/EXPSTRAT.ASM").split("\nexplode_Icont\n", 1)[1]
        copy = explode.split("s_copy_sflags", 1)[1].split("s_sprite_obj", 1)[0]
        self.assertEqual(instructions(copy), [
            "y,x", "s_clr_alsflag y,special", "s_clr_alsflag y,hitflash",
            "s_clr_alsflag y,realobj", "s_clr_alsflag y,shadow",
        ])
        macro = source("INC/STRATMAC.INC").split("\ns_copy_sflags\t", 1)[1].split("ENDM", 1)[0]
        self.assertIn("s_sta.w al_sflags4,\\1", instructions(macro))

    def test_reviewed_regression_tests_cannot_bless_native_output(self):
        for relative in ["sf-strat/tests/ea_parity.rs", "sf-strat/tests/eb_parity.rs",
                         "sf-strat/tests/bo_parity.rs", "sf-path/tests/interp_trace.rs"]:
            code = (ROOT / "rust" / relative).read_text()
            self.assertNotIn('var_os("SF_BLESS_FIXTURES")', code, relative)
            self.assertNotIn("std::fs::write", code, relative)

    def test_fixture_audit_recovers_every_original_byte(self):
        for name, original_hash in ORIGINAL_HASHES.items():
            with self.subTest(fixture=name):
                original = []
                for line in (FIXTURES / f"{name}.txt").read_text().splitlines(keepends=True):
                    if re.match(r"(?:O \d+ |T\d+ A\d+ )", line):
                        fields = dict(re.findall(r"(\w+)=(\S+)", line))
                        prefix = "s" if name.startswith("bo_") else "sf"
                        # All legacy bit-4 byte-2 uses in these seven fixtures
                        # are relative weapons/effects, not macro facing latches.
                        if int(fields[f"{prefix}4"]) & 4:
                            line = replace_field(line, f"{prefix}4", lambda value: value & ~4)
                            line = replace_field(line, f"{prefix}2", lambda value: value | 4)
                        if fields["sh"] in {"405", "479"}:
                            self.assertTrue(int(fields["sf"]) & 32)
                            line = replace_field(line, "sf", lambda value: value & ~32)
                        if name == "ea_zaco1" and fields["sh"] == "478":
                            self.assertEqual(fields["sb3"], "60")
                            line = replace_field(line, "sb3", lambda _: 0)
                        if name == "ea_houdai" and fields["sh"] == "405" and fields["hp"] == "0":
                            self.assertEqual(fields["cn"], "0")
                            line = replace_field(line, "cn", lambda _: 1)
                        if name == "ea_rader0" and fields["sh"] == "15":
                            line = replace_field(line, "ry", lambda value: (value - 8) & 255)
                    original.append(line)
                self.assertEqual(hashlib.sha256("".join(original).encode()).hexdigest(), original_hash)


if __name__ == "__main__":
    unittest.main()
