"""The strategic map screen's frame ($04:B5DA) and scripts ($04:CD47): code
and tables the native port embeds."""
import hashlib
import re
import unittest
from pathlib import Path

from extract_map import DEFAULT_ROM
from test_strategic_sim_static import span

NATIVE = Path(__file__).resolve().parents[3] / "rust/sf2-game/src/native"

# SHA-256 pins of the source ranges ported by strategic_screen.rs and
# strategic_script.rs.
PINS = (
    ("screen frame", 0x04B5DA, 0x04B9A3, "6d5a90985aad0226d0fc22158cba3c8139ecf935fabcc56d631b20625778bf97"),
    ("menu, scripts and services", 0x04CC54, 0x04D4D8, "4bcfe1e933369f236d3b55c86064e2724a8e26f5d6cd824902b9c294c0233e58"),
    ("travel", 0x04D56B, 0x04DAC2, "3288dc9db7b42f730aef65ba7a97a2b187ab62cf6d4e4dccb54adc240a7f0b45"),
    ("markers, guards and waves", 0x04E245, 0x04E6F2, "340ad15aa8f39beae6d7c3b3daf86b9d24cd4a4a34f8206d18466318aaedaa3e"),
    ("wave lists", 0x04E6F2, 0x04E73B, "3b660bdc5e2f41ec09a176b2c333134feb62a33b271562e06eb14d4a4fddd36a"),
    ("marker kinds", 0x04EE52, 0x04EF1E, "c08790d137a6ccf07893454d26e67f41eca3e9aaacffa74ce6e642e38e5cb409"),
    ("cursor and hover", 0x04F013, 0x04F323, "092c2820742d8d464870b3539668056444793bd53bba68914c78782bd5502ef3"),
    ("escort starts", 0x048E0A, 0x048E12, "c52246b9717fb85b1bf6f3942251f97cf5af5d3acf45b76022a7bee51bf15e02"),
    ("event cues", 0x7F6D30, 0x7F6D40, "7d59935bae8eb9823ea34e8d681dcbf566edc538a267de0fee94abdbe79f8694"),
    ("threat cues", 0x04B789, 0x04B7A1, "e5238d1e3fb9cb824a344b15d6c963d8928de5db53171e0b5785c6547bd5350a"),
    # strategic_director.rs ($04:B9A3).
    ("fleet and mark clearing", 0x04B42C, 0x04B4D7, "1488410be4296fafd10f3cc60299c7caf53e94184ddb11c2d58c1e48ddd146e7"),
    ("ship stop", 0x04B88D, 0x04B8A0, "56df88357368a997df61f8f78499d806538018ecd2ce1d9334f0debd0ee60f80"),
    ("program frame and aftermath", 0x04B9A3, 0x04C0E7, "8c817ba341c80a3656435540331e31aeac15ad67c25686eef244fe0b7ca0055a"),
    ("timeline, recapture, ambush, threats, sortie", 0x04C0E7, 0x04CC7E, "4f0b3d46f9e1397849fad5b50a25596ef336e1cf08a72873d23ace3c82e2f05c"),
    ("pulse starts", 0x04D4D8, 0x04D56B, "af767faee8d6f3a689cfe3b64de3a7f2ea98a5b2c8228712729cf810cea42aef"),
    ("clearing and exit", 0x04DAC2, 0x04DCBC, "62af28eea3656d9efb2b762cf88885498ccfb24b6a2834c640393cd3e23119ad"),
    ("place and unit release", 0x04E082, 0x04E0DC, "468d26040d0e2e6025363e368be90a4852c9cfe691b6303af34f8216d2f9a90c"),
    ("missile salvo", 0x04E7CD, 0x04E88E, "3ad674723ac344ea16e54ee21fc8e216154f8ec8294743eb50207c68cc04b0c5"),
    ("schedule", 0x04EF74, 0x04F002, "7096f0155e61b6aae2944b7bba9969e39448c6fc886ee1473b3cee66c3663731"),
    ("threat rescan", 0x7F5856, 0x7F58A6, "7fdb4b68cad4913860d794441adb210e15ffced929c273359e6627fbcb1488d1"),
)

# Rust byte tables and the ROM ranges they copy.
TABLES = (
    ("strategic_script.rs", "SCRIPTS", 0x04CD6E),
    ("strategic_script.rs", "WAVES", 0x04E6F2),
    ("strategic_script.rs", "MARKER_KINDS", 0x04EE52),
    ("strategic_script.rs", "MARKER_TIMERS", 0x04E2FE),
    ("strategic_script.rs", "ESCORT_STARTS", 0x048E0A),
    ("strategic_script.rs", "MARKERS_BY_DIFFICULTY", 0x04CE8A),
    ("strategic_screen.rs", "EVENT_CUES", 0x7F6D30),
    ("strategic_director.rs", "SCHEDULE", 0x04EF74),
    ("strategic_director.rs", "PULSE_STARTS", 0x04D533),
    ("strategic_director.rs", "SALVO_COLUMNS", 0x04E88A),
    ("strategic_director.rs", "MISSILE_KIND_OFFSETS", 0x04C36A),
    ("strategic_director.rs", "SATELLITE_ALERTS", 0x04C32B),
)


def rust_bytes(file, name):
    text = (NATIVE / file).read_text()
    body = re.search(r"const %s: \[u8; [^\]]+\] = \[(.*?)\];" % name, text, re.S).group(1)
    return bytes(int(v, 16) for v in re.findall(r"0x([0-9A-Fa-f]{2})", body))


class StrategicScreenStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def test_ported_ranges_are_pinned(self):
        for name, start, end, digest in PINS:
            with self.subTest(name):
                self.assertEqual(hashlib.sha256(span(start, end, self.rom)).hexdigest(), digest)

    def test_tables_copy_the_rom(self):
        for file, name, start in TABLES:
            with self.subTest(name):
                data = rust_bytes(file, name)
                self.assertEqual(data, span(start, start + len(data), self.rom))

    def test_dispatch_tables(self):
        def words(address, count):
            data = span(address, address + 2 * count, self.rom)
            return [int.from_bytes(data[2 * i:2 * i + 2], "little") for i in range(count)]
        # The frame's service table is all returns; Select's handler table.
        self.assertEqual(words(0x04B618, 8), [0xB628] * 8)
        self.assertEqual(words(0x04B8D4, 3), [0xB8E1, 0xB912, 0xB918])
        self.assertEqual(words(0x04D37E, 7), [0xD38C, 0xD3CB, 0xD3D3, 0xD3A4, 0xD405, 0xD38D, 0xD3EE])
        # The program frame's state tables.
        self.assertEqual(words(0x04DC87, 3), [0xDC8D, 0xDC8E, 0xDCA7])
        self.assertEqual(words(0x04B9F5, 10)[:3], [0xBA09, 0xBA0A, 0xBA0A])
        self.assertEqual(words(0x04C731, 6), [0xC73D, 0xC782, 0xC7DD, 0xC7E6, 0xC7FE, 0xC804])
        self.assertEqual(words(0x04C8E0, 4), [0xC8E8, 0xC8E9, 0xC91D, 0xC926])
        self.assertEqual(words(0x04C6D9, 5), [0xC6E3, 0xC6E4, 0xC6E4, 0xC6E4, 0xC6F6])
        self.assertEqual(words(0x04CB47, 5), [0xCB51, 0xCB66, 0xCBD8, 0xCC15, 0xCC32])
        self.assertEqual(words(0x04C9C7, 8), [0xC9D7, 0xC9D8, 0xC9E2, 0xCA0D, 0xCA1F, 0xCA38, 0xCA70, 0xCA9B])
        self.assertEqual(words(0x04DAD1, 10), [0xDAE5, 0xDBF5, 0xDC20, 0xDC3E, 0xDC57, 0xDAE6, 0xDAEB, 0xDAF0, 0xDAF5, 0xDAFA])


if __name__ == "__main__":
    unittest.main()
