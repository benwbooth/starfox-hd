"""The strategic map simulation ($7F:537D): code and tables the native port embeds."""
import hashlib
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM
import cpu65816


# SHA-256 pins of the source ranges ported by strategic_sim.rs.
PINS = (
    ("simulation code", 0x7F535E, 0x7F6AE0, "9cc3f72c0414965f173447c3a15495d246b737e924af258f066758968b4f80f4"),
    ("simulation tables", 0x7F6C10, 0x7F6D30, "f3d7113425859cf4e5e1fa2c275195d010288dd3e22674d970ab0a9232008fa1"),
    ("satellite rests", 0x7F5504, 0x7F550A, "937da4e6f8c20c9540b51471610ce67a90e4ce352197f0c5ed36f8f5869b89ba"),
    ("missile timers", 0x7F5C27, 0x7F5C2F, "d33f84bbb1cf9b1fe9027f73c9ae00e21009324af07fa5954d51b3170ede3a11"),
    ("escort bases", 0x7F6491, 0x7F6499, "7afc7cce820bf277df47c308301274cbe75053a2c11a5fa7eb5f6f61a773d553"),
    ("sine table", 0x7F3D92, 0x7F3ED2, "7b3da0de0b1a651397e8b85b54ed5d7b6d3d35701714838f35b860fb019f4113"),
    ("spawn tables", 0x00B0F8, 0x00B2BC, "6b60d84fe2e8d7a65823221cb93d59a3e92eb5bc5a9c2f049bc4c6b214c1750e"),
    ("multiply", 0x7F76D8, 0x7F7763, "3044346ee941cbb0ec8b9d72498923a94b4717bd82a44c60b2b2556b5793d2b0"),
    ("atan entry", 0x7F1D58, 0x7F1D7E, "fed967b73624cc69bb4480867c15169864141133cf91525f0c4c0179a4d59980"),
)


def span(start, end, rom):
    if start >> 16 == 0x7F:
        return rom[source_offset(start):source_offset(end)]
    return rom[cpu65816.cpu_to_file(start):cpu65816.cpu_to_file(end)]


class StrategicSimulationStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def test_ported_ranges_are_pinned(self):
        for name, start, end, digest in PINS:
            with self.subTest(name):
                self.assertEqual(hashlib.sha256(span(start, end, self.rom)).hexdigest(), digest)

    def test_dispatch_tables(self):
        def words(address, count):
            data = span(address, address + 2 * count, self.rom)
            return [int.from_bytes(data[2 * i:2 * i + 2], "little") for i in range(count)]
        self.assertEqual(words(0x7F5626, 4), [0x562E, 0x5630, 0x5656, 0x5663])
        self.assertEqual(words(0x7F56BE, 9), [0x56D0, 0x56D1, 0x56DD, 0x56FC, 0x5720, 0x576E, 0x5778, 0x57F4, 0x57ED])
        self.assertEqual(words(0x7F55BF, 4), [0x55C7, 0x55E3, 0x55E8, 0x55F8])
        self.assertEqual(words(0x7F5961, 5), [0x596B, 0x596C, 0x5A3A, 0x5A47, 0x5A6D])
        self.assertEqual(words(0x7F5AB5, 10), [0x5ADB, 0x5B67, 0x5B8A, 0x5ADC, 0x5B03, 0x5B1F, 0x5B56, 0x5BC1, 0x5BE0, 0x5CB3])
        self.assertEqual(words(0x7F6700, 9), [0x6712, 0x6713, 0x681E, 0x6818, 0x6716, 0x6742, 0x6770, 0x6787, 0x67F8])
        self.assertEqual(words(0x7F53F5, 4), [0x53FD, 0x542E, 0x54D8, 0x54DE])
        self.assertEqual(words(0x7F5E49, 5), [0x5E71, 0x5E9F, 0x5EA6, 0x5E53, 0x5E6D])
        self.assertEqual(words(0x7F598D, 6), [0x5999, 0x599B, 0x59A1, 0x59BA, 0x59CF, 0x59DF])


if __name__ == "__main__":
    unittest.main()
