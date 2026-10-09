"""The strategic map screen's frame ($04:B5DA) and scripts ($04:CD47): code
and tables the native port embeds."""
import hashlib
import re
import sys
import unittest
from pathlib import Path

from extract_map import DEFAULT_ROM
import cpu65816
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
    # strategic_sprites.rs, strategic_hud.rs and strategic_visit.rs.
    ("sprite pass head", 0x048301, 0x0483B3, "37397c2367c8e9b3d2414eeaf61c314758f16e2253934901c17ed8abc5369380"),
    ("sprite pass map mode", 0x0487A9, 0x049389, "a6861ae5ed0649750f247d17fd02dc074710df1c915f4bb2d12817b5a750db55"),
    ("shield gauge", 0x0495DA, 0x049664, "27c6f510af73b644bddbc2672f2697a54ce69e2288074e21c55d5a4480ca95fd"),
    ("sprite composers", 0x049EDE, 0x04A357, "eb30c6622f5aea1b12bcbb99fbedf33a7cf62146f8e2f1f7169e434229863494"),
    ("size bit tables", 0x04A3B6, 0x04A3ED, "94cff4c1308de4759bb2259dc564246ccaad8adefad5b6858fefcd5b196c1246"),
    ("effects and animated sprites", 0x04F431, 0x04F932, "a42101ac6cc9c88279a98f3aa7764549b4595c0cc3fa070f1dfe5e91c7f551fc"),
    ("hud layout service", 0x04A3ED, 0x04A501, "b6871cadd99c670987a61d91430bff6f6827b05a154779931a83a4f58dca7a84"),
    ("palette cycles", 0x04AB07, 0x04AC5B, "cc16c13e2a7b19c36005948929dfa0545cc6486bce631ded7814df6766d46e0b"),
    ("planet drain", 0x7F5FA0, 0x7F600A, "cb428eea5ecb36fc4a57936205e7b6e47c19e77c6a0db4d2af296c7161354457"),
    ("frame timers and random word", 0x7F0516, 0x7F05A0, "9f71a34ed0c85ce19c93172604d6e9497f7078f72019b984c0e423ba75b2f48c"),
    # strategic_radio.rs: the CPU side, the GSU message box step and the
    # text engine's measure.
    ("map radio call", 0x04E99B, 0x04E9AF, "6a9f73dbf659b85a8218420bb344b88fb79752b4db5f2d6bdf954fd7f62d12cc"),
    ("radio entry", 0x0B9F87, 0x0B9FB2, "f37f70a4322425fe4d6f6cd351a2204046e11d2042b81e655dd0146e1607c096"),
    ("radio services and script machine", 0x0BA029, 0x0BA609, "01bef960af57ea33d2c7208985f5cafb04814413586d1509b574d1fddd301dfe"),
    ("radio alerts", 0x0BFB8D, 0x0BFBE3, "1c9942a043890e0f6ec062de20d6644aec4b72210fa378592c67522ecb8793cb"),
    ("place messages", 0x0BFBE3, 0x0BFBEF, "1c23d1786e2c8ee9c9bb84a920124105770a61013cd0f9754e10d80fdef40517"),
    ("box styles", 0x0B85CA, 0x0B85DA, "0c9ec6321e72907229e676da651417365a877beb78a83814f1755b989e7efc38"),
    ("message box step", 0x0B8234, 0x0B84D0, "33532aca7d725aab3ceddb64d2704428e0bed89f49a46b3814505a1dd5f77ca9"),
    ("word wrap", 0x01ED91, 0x01EE48, "14a2065f14741d750ed2d631c823212f8c774544f41715bceeb1f33c39dac88c"),
    ("text height", 0x01EF10, 0x01EF48, "9564c4f9c9d81344ec73526587f4592c945e5e218d61fea1b07a825b55ece201"),
    ("message pointers", 0x00AEB3, 0x00B063, "27a757528b59cad379bfb0ab711b9793256feaa046c04f30071a0cc7b3c1722a"),
    # strategic_entry.rs: the map entry and the campaign setup.
    ("map program head", 0x04AE4F, 0x04AE92, "f17520126b53670467453cf061cfb8e092cb5fd74f80f4cbf68a479ab5eac264"),
    ("map entry", 0x04DCBC, 0x04DE71, "906021f7fdd272b0f7c75b74d3612c4ab301ff8cfb6461bd9e51235b20ae4fe4"),
    ("ship placement", 0x04DE71, 0x04DEA9, "def81952ffe87acceec3efdb6ada99772b1b43bdaa5c0fa46e9b8b1b89ddbec6"),
    ("campaign setup", 0x04DEA9, 0x04E079, "696cc3df43a91960ce7e6e6142adb9acdf2af0dae60f44a1c3e318452e6c0795"),
    ("campaign tables", 0x04E079, 0x04E082, "0460d40d441e91316ce41edc28c1e42a986f64589a64a8388534eac9e7a94284"),
    ("pool and place setup", 0x04E0DC, 0x04E245, "a04f16332007ae57d3216df9868c3ccaf6c78b2d0a5463d23cfff5a7e1dd60e5"),
    ("planet setup", 0x04E30C, 0x04E3B9, "cf4bda2967fe0f521b72654cb059e2845f43e48b3c49cf1eaa7a414139c0bb73"),
    ("new place", 0x04E4DA, 0x04E571, "f12d5e1328b7b4e7bb9c95cc9ab8c5b4eb25601c714ab45fbe7101605fc56500"),
    ("station setup", 0x04E73B, 0x04E7CD, "659ffd4ad96f5786acb4d2f5172610f9edeebf326fcc81d6c90f488d2b9aedf9"),
    ("base setup", 0x04E88E, 0x04E920, "d5d175d29369d741779773a576b59434377ba05c627427bbed932c335b9f7daf"),
    ("escort numbers", 0x04EE29, 0x04EE2C, "9c87f6be6cf25fabbb27e2be33c3afbb0f9e1316d9ff4130e8ddc0d66d0eb44f"),
    ("marked places", 0x04EF1E, 0x04EF36, "977f87fe8900a9f28e4618324fe1e221c9528b11849e29d1f2c95a227998f4d1"),
    ("map layers", 0x04EA8C, 0x04EAF6, "57a68ecdc17d935f5b48b265b2aeb2030c2e3bda76caf4cce1936c19f3d902b3"),
    ("terrain grid", 0x04EB80, 0x04EC1A, "540ea7d71cb084f63bbd1d853d907049077f22365615aa5512978de0b3aaa891"),
    ("palette shadow", 0x04EC76, 0x04ED33, "9f622e1f9cf90213b2b48b06350ef80f38b22fcde897f1c56a671e1b25237374"),
    ("sprite buffer setup", 0x048001, 0x048059, "5f71fed5d69aeca86595832c6f5b13be787192e41341766ed015a08b1eadd8ec"),
    ("sprite buffer fill", 0x049D3C, 0x049D79, "fbb37fe498f8c2bf9714c47d5276d9c377b1e9af794703413bf4266bc788cbff"),
    ("sprite high table", 0x04A109, 0x04A12E, "8846766a2cf128e8091577b4221c647319b9509bb6cb979c85192826730ad874"),
    ("radio reset", 0x0B9F4C, 0x0B9F87, "902be6b577e7590b6d574383577369ada1edc95f086bbd5b4d59465b1453e7a2"),
    ("radio reset helpers", 0x0B9FB2, 0x0BA029, "1fa0f861414af4e80ef14102441160c28026d0d813882900d36f974627a86d55"),
    ("box base", 0x0BA05A, 0x0BA084, "e6c3cda81664be28f762c1245af2054d80dbd0beddf7b5482d317520ccfc81d3"),
    ("generator", 0x7F7BD4, 0x7F7BE8, "5cd430174268f0283a09e1ce2fbb887a1fe9e0fdf9fc7d0826f3d9114cde0880"),
    ("decompressor", 0x01D9FF, 0x01DBDD, "3e340f6a14720ba017492ed38b3ca6cfe9f052f9a999ccfb5e46a7bb162b4eec"),
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
    ("strategic_radio.rs", "SCRIPTS", 0x0BA609),
    ("strategic_radio.rs", "PLACE_MESSAGES", 0x0BFBE3),
    ("strategic_radio.rs", "STYLE_TOP", 0x0B85D2),
    ("strategic_radio.rs", "STYLE_CUES", 0x0B85CA),
    ("strategic_entry.rs", "MARKS", 0x04E079),
    ("strategic_entry.rs", "FLEETS", 0x04E07C),
    ("strategic_entry.rs", "ENEMIES", 0x04E07F),
    ("strategic_entry.rs", "ESCORT_NUMBERS", 0x04EE29),
    ("strategic_entry.rs", "SCHEDULE_STARTS", 0x04E04C),
    ("strategic_entry.rs", "MISSILES", 0x04E052),
    ("strategic_entry.rs", "PLANET_TALLIES", 0x04E3B6),
    ("strategic_entry.rs", "MARKED_PLACES", 0x04EF1E),
    ("strategic_entry.rs", "SATELLITE_WAITS", 0x04E7C7),
    ("strategic_entry.rs", "MARKER_HEADER", 0x04EE52),
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

    def test_sprite_catalog_copies_the_rom(self):
        path = NATIVE.parents[2] / "sf2-data/src/map_sprites.rs"
        body = re.search(r"MAP_SPRITES: \[u8; [^\]]+\] = \[(.*?)\];", path.read_text(), re.S).group(1)
        data = bytes(int(v, 16) for v in re.findall(r"0x([0-9A-Fa-f]{2})", body))
        start = cpu65816.cpu_to_file(0x18AF24)
        catalog = self.rom[start : start + 0x50DC]
        self.assertEqual(data, catalog)
        self.assertEqual(
            hashlib.sha256(catalog).hexdigest(), "2cafc506fca19528a8737ff3a0011b85cca6a89c5564c4a868dbd3681fd886e6"
        )

    def test_radio_alerts_copy_the_rom(self):
        text = (NATIVE / "strategic_radio.rs").read_text()
        body = re.search(r"const ALERTS: \[[^\]]+\] = \[(.*?)\];", text, re.S).group(1)
        rows = re.findall(r"\((0x[0-9A-F]+), (0x[0-9A-F]+), (0x[0-9A-F]+), (0x[0-9A-F]+), (0x[0-9A-F]+)\)", body)
        data = b"".join(
            int(bit, 16).to_bytes(2, "little") + bytes(int(v, 16) for v in rest) for bit, *rest in rows
        )
        # Fourteen entries, then the zero bit that ends the table.
        self.assertEqual(data + b"\0\0", span(0x0BFB8D, 0x0BFBE3, self.rom))

    def test_messages_and_font_copy_the_rom(self):
        text = (NATIVE.parents[2] / "sf2-data/src/messages.rs").read_text()

        def table(name, pattern):
            body = re.search(r"%s: \[u(?:8|16); [^\]]+\] = \[(.*?)\];" % name, text, re.S).group(1)
            return [int(v, 16) for v in re.findall(pattern, body)]

        pointers = table("MESSAGE_POINTERS", r"0x([0-9A-Fa-f]{4})")
        self.assertEqual(b"".join(p.to_bytes(2, "little") for p in pointers), span(0x00AEB3, 0x00B063, self.rom))
        self.assertEqual(bytes(table("MESSAGES", r"0x([0-9A-Fa-f]{2})")), span(0x009168, 0x00AEB3, self.rom))
        self.assertEqual(bytes(table("FONT", r"0x([0-9A-Fa-f]{2})")), span(0x0DE1FB, 0x0DE472, self.rom))

    def test_map_layers_are_the_decompressed_streams(self):
        sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
        import extract_map_layers

        text = (NATIVE.parents[2] / "sf2-data/src/map_layers.rs").read_text()
        for name, data in (("MAP_TERRAIN", extract_map_layers.terrain(self.rom)), ("MAP_PALETTE", extract_map_layers.palette(self.rom))):
            with self.subTest(name):
                body = re.search(r"%s: \[u8; [^\]]+\] = \[(.*?)\];" % name, text, re.S).group(1)
                self.assertEqual(bytes(int(v, 16) for v in re.findall(r"0x([0-9A-Fa-f]{2})", body)), data)

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
