"""Static evidence for rotated spawning and contact-shot reflection."""
import unittest
import ast
import re
from pathlib import Path
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM
from extract_path import PathExtractor, PathAddress


class ChariotStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start+len(raw)], raw)

    def test_offset_spawn_operand_layout_and_full_dependency_closure(self):
        self.assert_source(0x08905B, '31e0cee51200000064040000e0ff2000')
        extractor = PathExtractor(self.rom)
        extractor.discover_roots = lambda: [PathAddress(0x105B)]
        extracted = extractor.extract()
        self.assertIn(PathAddress(0x12E5), {c.address for c in extracted.commands})
        self.assertNotIn(PathAddress(0x12E5), extractor.decode_command(PathAddress(0x105B)).successors)

    def test_offset_spawn_rotation_sign_extends_low_bytes_and_scales_after_rotation(self):
        self.assert_source(0x7F92CA, 'adb116850285048980f006a9ff850580026405adb3168508850a8980f006a9ff850b8002640badb516859785e48980f006a9ff85e5800264e5')
        self.assert_source(0x7F9303, 'b51622f03b7fa5048502a50a8508a5e48597b512224e3a7fa5048502a50a8508a5e48597b51422a9387f')
        self.assert_source(0x7F932D, 'c220a5040a0a8504a50a0a0a850aa5e40a0a85e4a504187d0c00990c00a5e4187d1000991000a50a187d0e00990e00e22060')
        self.assert_source(0x7F935F, 'c220688da812e220a0000060')

    def test_reflection_source_body_includes_scatter_full_pool_sprite_and_list_gates(self):
        self.assert_source(0x7FBF75, 'a55e29e7855e8f3a300022aef1074ce8ca')
        self.assert_source(0x07F1AE, 'da5a08860464e4b5252910d0045ceaf207b41ed0045ceaf207be04005ab5312908c908f0045cccf207b5212901f0045cccf207b52109019521b5141869808508b51249ff1a8502daa604a50838f5148508ecc312f005ecc512d00f5ab42bb9026c7a2940d0045c3ef20722d07b7f293f186d0200850222d07b7f293f186d08008508a5021869e08502a5081869e08508a5028db714a5088db6149cb8149cb914a9008db014a9008db214a9008db414a902229ca803c00000d003acd614fa5aa90a223b237fc00000d0045c8bf207c220b9626a8502e220c220a5028004c220b5047a990400e220c220bdcd1c99cd1ce22022aa2b7fb5202920d0045cc5f207b920000920992000bdc81c99c81cbdda1c99da1ca93c9518a5e418690185e47aada61a8902d008a5e4f0045ceaf207c220b90000a8e220f0045cc7f107287afa6b')

    def test_heavy_chariot_label_is_source_text(self):
        self.assert_source(0x0389A5, '48454156592043484152494f5400')

    def test_shared_native_rotation_tables_match_source_byte_coefficients(self):
        source = (Path(__file__).resolve().parents[3] / 'rust/sf-core/src/snes_trig.rs').read_text()
        for name, address in [('SINTAB', 0x7F3D92), ('COSTAB', 0x7F3DD2)]:
            values = ast.literal_eval(re.search(rf'pub static {name}: \[i8; 256\] = (\[.*?\]);', source, re.S).group(1))
            start = source_offset(address)
            self.assertEqual(bytes(v & 255 for v in values), self.rom[start:start+256])


if __name__ == '__main__':
    unittest.main()
