"""Stage services that reach the strategic map: radar region and announcer."""
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM
import cpu65816


# Byte pins of the source routines ported by strategic_service.rs and stage_announcer.rs.
PINS = (
    ("radar region service", 0x7F539C,
     "a55e48a55e0918855e8f3a3000c230ad841b891a00d024ad961b890400f01cad961b890800f014ad861b892000d00ce2"
     "208ba97e48ab225e537fabe22068855e8f3a30006b"
     ),
    ("map tick dispatch", 0x7F535E,
     "08c230ae081cfc6953286b79537953795379537953795379537953207d5360"
     ),
    ("announcer wrapper", 0x7F7034,
     "8bf47f7fababa55e48a55e0918855e8f3a3000c2302263777f205b70e22068855e8f3a3000ab6b"
     ),
    ("announcer", 0x7F7763,
     "8bf47e7eabab08c230ad681b29ff00c90100d008adbe1fc90300f03bad841ed036a20000bfd4777ff02d1c33dbd0088a"
     "18690400aa80edbfd6777f29ff00d00320bd778d841ebfd7777f29ff008f66f57e9c5ef59c60f528ab6ba91100ac47db"
     "c00f00900ba91000c032009003a90f0060"
     ),
    ("announcer events", 0x7F77D4,
     "02006e03040000020800120201001302100051002000500040005200800087000001d1020002c502"
     ),
    ("display service stage branch", 0x04FD87,
     "2234707f60"
     ),
)


def offset(address):
    if address >> 16 == 0x7F:
        return source_offset(address)
    return cpu65816.cpu_to_file(address)


class StageMapServicesStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def test_ported_routines_are_pinned(self):
        for name, address, expected in PINS:
            with self.subTest(name):
                data = bytes.fromhex(expected)
                start = offset(address)
                self.assertEqual(self.rom[start:start + len(data)], data)


if __name__ == "__main__":
    unittest.main()
