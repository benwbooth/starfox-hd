"""The Super FX decompressor against the GSU output hashes the Mesen oracle
(mesen_decompress_oracle.lua) recorded for the title screen's streams."""
import unittest

from decompress import decompress
from rom import load_rom

# (bank, stream end, output length, FNV-1a of the output).
STREAMS = (
    (0x16, 0xBFB8, 0x2020, 0x68444FCF),
    (0x16, 0xC4E4, 0x1000, 0x9B84A6AD),
    (0x18, 0xAF24, 0x24C0, 0x3E4A3761),
    (0x16, 0xEF70, 0x1000, 0x76C12B24),
    (0x16, 0xFD7C, 0x0800, 0xA2FFB62F),
    (0x19, 0x9F9C, 0x2020, 0xC78AFF13),
)


def fnv1a(data: bytes) -> int:
    value = 0x811C9DC5
    for byte in data:
        value = ((value ^ byte) * 0x01000193) & 0xFFFFFFFF
    return value


class DecompressTests(unittest.TestCase):
    def test_streams_match_the_gsu(self):
        rom = load_rom()
        for bank, end, length, digest in STREAMS:
            with self.subTest(f"{bank:02X}:{end:04X}"):
                data = decompress(rom, bank, end)
                self.assertEqual(len(data), length)
                self.assertEqual(fnv1a(data), digest)


if __name__ == "__main__":
    unittest.main()
