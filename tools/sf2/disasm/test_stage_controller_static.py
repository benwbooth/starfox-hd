"""The flight stage controller: transition machine, clocks and exit fade."""
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM
import cpu65816


# Byte pins of the source routines ported by stage_controller.rs.
PINS = (
    ("stage kind 1 dispatch", 0x03C500,
     "220ac7032209c50360"
     ),
    ("transition machine", 0x03C509,
     "8ba97e48ab20e4c6c220ad671c892000f003829401ad671c890400f003827f01ad781b0aaa7c31c541c5e1c51fc650c6"
     "92c678c6efc53cc6ad841b891200f003829401ad721d29ff00f003828901adf4d729ff00d003827e01ad32f5f0038276"
     "01ad881b890001f012a903008d781ba930000c841ba900020c8a1ba908001c961bce67dad00ca904008d67daa908000c"
     "961b22395f7fad961b890080d028ee63dace61dad020ad0a1cc96300f01b9c63daa9f0008d61daee0a1cad5bdac9e703"
     "f0041a8d5bda820e01a9ef008d63daa900800c961b82ff00ce7c1bd006ad7a1b8d781b82f100a900080c961ba910000c"
     "841ba907008d7a1ba93c008d7c1ba904008dd21fa901008d781be220a9080cd31cc22082c100a902001c881ba904000c"
     "861ba903008d6a1be220a9800c10d8c2208064a900400c881ba902001c881ba907008d6a1b8050a9080022f86d7fa9f1"
     "0022096e7fa914008d761ba904008d6a1ba908000c861ba904000c671c8030a900020c861ba900200c861ba91c008d76"
     "1ba904008d6a1b800ea907008d6a1ba902000c881b8000a920000c671c800a2254e103c2209030800822fce003c22090"
     "26ad841b890800f006a90d008d6a1ba9ffff1c961ba900008d701ba900008d781ba9ffff1c0e1ce220ab6b"
     ),
    ("score publication", 0x03C6E4,
     "088be220a97e48abacc312c220b92b00a8e220c220b9336c8d16d8e220b9356c8d18d8ab2860"
     ),
    ("pause trigger", 0x03C70A,
     "c220ad841b893200f003824b00aff4d77ed003824200ad841b890100d03dad9612890010f0328000a920000c841ba900"
     "100c961b22708c0b225ac00be220ad701b8d721bad9e1b8da01ba9028d701ba9068d9e1b68686860e2206bad96128900"
     "10f008a910000c981b800ead9812890010f0e5a920000c981be220ad701b8d721bad9e1b8da01ba9028d701ba90f8d9e"
     "1b80c1"
     ),
    ("planet damage", 0x7F5F39,
     "ad8a1b890002d005ad4bdbd013adb9d9f00c3a8db9d9d006a900201c9c1b186bce4bdbce47dba900100c881ba900200c"
     "9c1ba932008db9d9a9640038ed47db8d49dbc96400d01ea903008d781ba902008d21daa900010c881ba930000c841ba9"
     "00020c8a1b386b"
     ),
    ("scene exit fade", 0x03E0FC,
     "c220ad671c892000f04aad671c891000d016a910000c671ca9090022f86d7fe220a9010cd31cc220adbe1b3a1003a900"
     "008dbe1bd01ee220a5f4f006a9fe85f38012c220a920001c671ca910001c671ce220386be220186b"
     ),
    ("stage kind dispatch", 0x03C193,
     "a2ffff8ef41c229dc70322a0c703c220ad681b0aaae220fcaec16b8dc200c57dc4b9c1dcc160"
     ),
    ("stage loop frame", 0x03C182,
     "2281dd0322218c0b220480032293c10360"
     ),
    ("scene fade service", 0x7F0E79,
     "adbb181af00da98f8f7c007f8f7e007f827e00a5f3d0038277001035c9fef028c9fdf01da5f4f0053af0028039a98f8f"
     "7c007f8f7e007f8f80007f64f364f48050a5c42901f0dd60a5f4f0e13af0de80d5c902f021c903f02ba5f4c90ff0031a"
     "800464f3a90f85f48f7c007f8f7e007f8f80007f801ba5f4c90ff0e61ac90ff0e51a80e2ce591cd008ad5a1c8d591c80"
     "c860"
     ),
    ("render fade call", 0x7F0846,
     "20790e"
     ),
    ("blank hold", 0x03DD81,
     "08e220c210adbb181af003cebb18286b"
     ),
)


def offset(address):
    if address >> 16 == 0x7F:
        return source_offset(address)
    return cpu65816.cpu_to_file(address)


class StageControllerStaticTests(unittest.TestCase):
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
