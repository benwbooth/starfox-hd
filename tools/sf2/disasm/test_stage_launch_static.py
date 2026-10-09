"""The mission stage launch: modes, director request, location map."""
import unittest

from extract_map import DEFAULT_ROM
import cpu65816


# Byte pins of the source routines ported by stage_launch.rs.
PINS = (
    ("mission launch", 0x03B90E,
     "a9008da51aa9048d9e1ba9028da21bc220a901001c841ba910000c861ba902000c9c1ba904000c9c1ba908000c9c1be2"
     "20a9080cb81cc220e220a9008fd8d77e22178c0b22668c0bc2208d6e1ce220c220a902000c671ce220c220ad8a1b8980"
     "00f006a900400c961bad861b892000f018a900400c961ba901008ff4d77eaff2d77e8da51b22c0b104e220adb51b1a8d"
     "b71bc220af5bda7e8f5dda7ea901001c881badb51bc906003006a901000c881be220aeb51bbf4fba038db81bc2208a0a"
     "0aaabf19ba038d6e1badb51bc90700d02fada51bc90800f018c90a00f00bc90b00d01da900200c961ba973018d6e1b80"
     "0faf87e07e890004f006a962018d6e1be2202212e3037c17ba"
     ),
    ("location table", 0x03BA17,
     "5dba62007bba76009fba8a00bdba9e00e1bab20003bbc3008dbb1501a5bb2901bdbb3d018dbb510121bbd4003fbbe200"
     "57bbf90075bb0701"
     ),
    ("location classes", 0x03BA4F,
     "0304050100020000000000000000"
     ),
    ("interception location", 0x03BBA5,
     "a905a295698d2e198e5716c220a900021c841be2204c74be"
     ),
    ("director clear", 0x0B8C17,
     "08c230a2be1f7400286b"
     ),
    ("stage-start request", 0x0B8C66,
     "20b08c031e"
     ),
)


class StageLaunchStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def test_ported_routines_are_pinned(self):
        for name, address, expected in PINS:
            with self.subTest(name):
                data = bytes.fromhex(expected)
                start = cpu65816.cpu_to_file(address)
                self.assertEqual(self.rom[start:start + len(data)], data)

    def test_location_scene_numbers(self):
        start = cpu65816.cpu_to_file(0x03BA19)
        scenes = [int.from_bytes(self.rom[start + 4 * i:start + 4 * i + 2], "little") for i in range(14)]
        self.assertEqual(scenes, [0x62, 0x76, 0x8A, 0x9E, 0xB2, 0xC3, 0x115, 0x129, 0x13D, 0x151, 0xD4, 0xE2, 0xF9, 0x107])


if __name__ == "__main__":
    unittest.main()
