"""The stage loop's scene setup: display, clocks, layout and first visit."""
import unittest

from extract_map import DEFAULT_ROM
import cpu65816


# Byte pins of the source routines ported by stage_setup.rs.
PINS = (
    ("scene setup", 0x038325,
     "088be220a90048ab78e2209c0c42a980228487038d002164f364f4a9008f871a00a96e8d3e1622578f03e220c210a964"
     "8d3e1622578f03e220c210a9808d491b9c4719c230a9e02e8f39d77ea9f0d88d111ea901008fbc24709c321b9cca1a9c"
     "cc1a9c571d9c0e169c55169c59169c5b1664cba901008fea03709c571de220c2108ba97e48abc220ad841b890100e220"
     "d03ba9018d701bc220a910000c841ba900011c841ba904001c961ba9ffff8d2ee89c63daa9f0008d61daa904008d67da"
     "9c0a1ce220a9038d36e8823400a27a008e0a1c22"
     ),
    ("scene setup tail", 0x0384F9,
     "08e230c220a934128fe80170a901008fea01702258ad03e220a98085a0a9ff8d281564005c688a03"
     ),
    ("layout dispatch", 0x03B19D,
     "0820a3b1286be230aea51a7cabb1dbb26ab39ab42fb5beb54db6bdb14cb2"
     ),
    ("layout 0", 0x03B2DB,
     "c230a9c0008d16193a85301ac900806a85d2642c642aa9c0008d1419a9e0008d18193a852e1ac900806a85d0a992248d"
     "ea18a9aa2a8dec18a9a0028d2819a902008d1a19a902008d1e19a9003c8d2219a900208d2619a900008d341ba900308d"
     "361ba9002c8d381ba900708d3a1ba9005c8d3c1ba900648d421ba9802f8d441ba9c02f8d461ba902088da61a4ce5b6"
     ),
    ("object pool reset", 0x0385AE,
     "08c2309ca812a9bd038daa12a03c00aa18693f00950088d0f67400286b"
     ),
    ("proxy and first visit", 0x03A55D,
     "c2209c1f189c21189c23189cb018a900008fac0170e220c2108ba97e48abaea812c220a99cbc855fe22022172a7fb004"
     "5cb3a5038cd614b9090029f7990900b9220029fb992200b922000940992200a9ff99f01c8004220680008cc3128cc512"
     "aea8129cd6122296357f"
     ),
)


class StageSetupStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def test_ported_routines_are_pinned(self):
        for name, address, expected in PINS:
            with self.subTest(name):
                data = bytes.fromhex(expected)
                start = cpu65816.cpu_to_file(address)
                self.assertEqual(self.rom[start:start + len(data)], data)


if __name__ == "__main__":
    unittest.main()
