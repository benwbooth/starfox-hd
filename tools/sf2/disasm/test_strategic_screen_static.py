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


if __name__ == "__main__":
    unittest.main()
