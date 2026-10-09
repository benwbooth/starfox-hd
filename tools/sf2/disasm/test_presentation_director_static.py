"""The bank-0B presentation director: state dispatch, step lists, stage-start iris."""
import unittest

from extract_map import DEFAULT_ROM
import cpu65816


# Byte pins of the source routines ported by presentation_director.rs.
PINS = (
    ("director frame", 0x0B8C21,
     "8bf47e7eabab08c23020818ceeccf528ab6b"
     ),
    ("director lane run", 0x0B8C81,
     "a9be1f8da81f2023ac808c"
     ),
    ("stage start state table", 0x0B8C8A,
     "808c5b915b9169925492ce8e4c8f7594e79349954995019c109c5b911a954a9526961e9ca58ec220"
     ),
    ("holding request", 0x0B8CB0,
     "c220a920000c841ba900008f3ef57e8009c220a901008f3ef57ee220fa8b4baba0be1fbd0100990000a900990100a900"
     "ebbd0200ab6b"
     ),
    ("state dispatch", 0x0BAC23,
     "aea81fb500f01a300d950209008095007404740874063868750275029baa7c00006860"
     ),
    ("step timer", 0x0BAC51,
     "aea81fb406f009d606f608d002d60860aea81f95067408f60460"
     ),
    ("step end", 0x0BAC78,
     "aea81fa9000095007402740460"
     ),
    ("step interpreter", 0x0BAC85,
     "aea81f9b740a6838750475047504aabf00000b29ff00f02548bf03000b297f0048fc01007a68898000d004984c51ac29"
     "0f000aaa7cbcacc5acc9acd3ac4c78ac984c61acaea81ff608d002d60860aea81ff608d002d608b50af004984c61ac60"
     ),
    ("stage start iris", 0x0B9269,
     "2085ac80648e80849280b29211bb9280348e02e7c780698e812c8eaea81fda221ea80bfa8ea81fad9c1b890800d00d20"
     "0c8dffff0f0000030210e060200c8dffff0f0000030250e060a901008f3ef57e80d520998d20f49220349320c89260"
     ),
    ("stage start", 0x0BA81E,
     "8b4bab08c230a900008f32f57ead681bc90100d00c9cd21fa901008dd21f2050c828ab6b"
     ),
    ("window fills", 0x0B8E2C,
     "bb74047402740060e220c220ad2c1429fdff8d2c14e220c210c220ad2e1429fdff8d2e14e220c210c220a920001c841b"
     "60855085529b8010a9807f8003a900ffa8855084"
     ),
)


class PresentationDirectorStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def test_ported_routines_are_pinned(self):
        for name, address, expected in PINS:
            with self.subTest(name):
                data = bytes.fromhex(expected)
                start = cpu65816.cpu_to_file(address)
                self.assertEqual(self.rom[start:start + len(data)], data)

    def test_stage_start_iris_steps(self):
        start = cpu65816.cpu_to_file(0x0B926C)
        steps = [(self.rom[start + 3 * i], int.from_bytes(self.rom[start + 3 * i + 1:start + 3 * i + 3], "little")) for i in range(8)]
        self.assertEqual(steps, [(0x80, 0x8E64), (0x80, 0x9284), (0x80, 0x92B2), (0x11, 0x92BB), (0x80, 0x8E34), (0x02, 0xC7E7), (0x80, 0x8E69), (0x81, 0x8E2C)])


if __name__ == "__main__":
    unittest.main()
