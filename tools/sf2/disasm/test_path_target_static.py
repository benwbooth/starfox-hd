#!/usr/bin/env python3
"""Target selection contracts read only from the disassembled source/data."""

from pathlib import Path
import re
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathTargetStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def assert_source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_wrapper_primary_slot_lock_and_display_side_effect_before_distance_test(self):
        self.assert_source(0x7FB262, "A5 5E 29 E7 85 5E 8F 3A 30 00 22 EA B1 07 4C E8 CA")
        self.assert_source(0x07B1EA,
            "8B DA 5A 08 A9 7E 48 AB E2 20 C2 10 A9 08 8D B2 1D 80 0F 8B DA 5A 08 A9 7E 48 AB E2 20 C2 10 9C "
            "B2 1D 8E C0 1D AC C3 12 DA B6 2B 9B FA B9 C2 6B 89 10 F0 04 5C AA B3 07 B9 B6 6B 29 7F 99 B6 6B "
            "C2 20 B9 BA 6B 8D DE 12 E2 20 A0 3F 03 22 9F 24 7F C2 20 AD DE 12 8D B4 1D E2 20 20 90 AE C2 20 "
            "AD B0 1D 8D BA 1D AD AE 1D 8D B8 1D E2 20")

    def test_forced_owner_or_strict_unsigned_distance_then_publication(self):
        self.assert_source(0x07B32F,
            "AE C3 12 C2 20 B4 2B B9 CA 6B F0 13 CD C0 1D D0 0E E2 20 B9 C2 6B 09 10 99 C2 6B C2 20 80 0E AD "
            "B4 1D 10 04 49 FF FF 1A D9 BC 6B B0 4E AD B4 1D 99 BC 6B AD DE 12 99 BA 6B DA AE C0 1D 8A 99 B8 "
            "6B BD 0C 00 99 CC 6B BD 0E 00 99 CE 6B BD 10 00 99 D0 6B FA AD BA 1D 99 BE 6B AD B8 1D 99 C0 6B "
            "E2 20 A5 79 99 AD 6B A5 7B 99 AF 6B AD B0 1D 99 C5 6B B9 C2 6B 0D B2 1D 99 C2 6B 28 7A FA AB 6B")

    def test_angles_half_each_coordinate_first_and_replace_only_distance_low_byte(self):
        self.assert_source(0x07AE90,
            "08 C2 30 B9 0C 00 C9 00 80 6A 85 3A B5 0C C9 00 80 6A 38 E5 3A 10 04 49 FF FF 1A 8D DE 12 B9 10 "
            "00 C9 00 80 6A 85 3E B5 10 C9 00 80 6A 38 E5 3E 10 04 49 FF FF 1A 18 6D DE 12 8D DE 12 64 3A 89 "
            "00 F0 F0 05 4A E6 3A 80 F6 8D DE 12 DA BB 7A DA 5A C2 20 9C 9D 14 AD DE 12 85 08 B5 0E C9 00 80 "
            "6A 85 3C B9 0E 00 C9 00 80 6A 38 E5 3C C6 3A 30 06 C9 00 80 6A 80 F6 85 02 22 58 1D 7F C2 30 7A "
            "FA 18 75 12 8D AE 1D E2 20 22 88 21 7F C2 20 49 FF FF 1A 18 75 14 8D B0 1D E2 20 B9 0C 00 38 F5 "
            "0C 10 03 49 FF 1A 8D DE 12 B9 10 00 38 F5 10 10 03 49 FF 1A 18 6D DE 12 8D DE 12 DA BB 7A 28 60")

    def test_projection_retains_yaw_fraction_coupling_and_individual_arithmetic_shifts(self):
        self.assert_source(0x07B258,
            "DA C2 20 A2 00 00 AD BA 1D 8D BA 1D 30 0F DF 81 B4 07 90 18 EE B0 1D BF 7D B4 07 80 30 "
            "DF 83 B4 07 B0 09 EE B0 1D BF 7F B4 07 80 21 AD B9 1D 85 3C AD BB 1D 89 80 00 F0 07 "
            "26 3C 20 61 B4 80 09 26 3C 20 56 B4 49 FF FF 1A 18 69 70 00 85 79 A2 00 00 AD BA 1D "
            "10 0A C9 00 EE 10 0D A2 10 00 80 08 C9 00 15 90 03 A2 08 00 AD B8 1D 30 0F DF 89 B4 07 "
            "90 18 EE B0 1D BF 85 B4 07 80 57 DF 8B B4 07 B0 09 EE B0 1D BF 87 B4 07 80 48 AD B9 1D "
            "85 3A AD B9 1D 89 80 00 F0 0B 26 3A 20 61 B4 49 FF FF 1A 80 05 26 3A 20 56 B4 85 3A "
            "C9 00 80 6A C9 00 80 6A 48 18 65 3A 85 3A 68 C9 00 80 6A 49 FF FF 1A 48 18 65 3A "
            "85 3A 68 C9 00 80 6A 18 65 3A 18 69 60 00 85 7B FA")
        self.assert_source(0x07B456,
            "DA 08 29 1F 00 AA 28 B0 15 80 0D DA 08 49 FF FF 1A 29 1F 00 AA 28 90 06 BF BD B4 07 80 04 BF DD B4 07 29 FF 00 FA 60")

    def test_authored_edge_limits_and_native_curves_are_exact_immutable_data(self):
        self.assert_source(0x07B47D,
            "10 00 D0 00 00 18 00 E8 B0 00 20 00 00 14 00 EE 9A 00 20 00 00 0E 00 EC 84 00 3C 00 00 0D 00 F3")
        native = (Path(__file__).resolve().parents[3] / "rust/sf2-game/src/native/path_target.rs").read_text()
        for name, address in [("WHOLE_CURVE", 0x07B4BD), ("HALF_CURVE", 0x07B4DD)]:
            values = re.search(rf"const {name}: \[u8; 32\] = \[(.*?)\];", native, re.S)[1]
            actual = bytes(int(value) for value in re.findall(r"\d+", values))
            start = source_offset(address)
            self.assertEqual(actual, self.rom[start:start + 32])

    def test_range_helper_keeps_wrapped_signed_halves_and_bearing_uses_full_deltas(self):
        self.assert_source(0x7F249F,
            "C2 20 B9 0C 00 38 F5 0C 10 04 49 FF FF 1A 85 02 B9 10 00 38 F5 10 10 04 49 FF FF 1A "
            "85 08 C2 20 A5 02 C9 00 80 6A 85 02 A5 08 C9 00 80 6A 85 08 A5 08 18 65 02 0A 8D DE 12 "
            "A5 08 C5 02 30 04 A5 08 80 02 A5 02 18 6D DE 12 85 02 85 08 A5 02 C9 00 80 6A 18 65 08 "
            "C9 00 80 6A C9 00 80 6A 8D DE 12 E2 20 6B")
        self.assert_source(0x7F2188,
            "DA 5A C2 20 B9 0C 00 38 F5 0C 85 02 B9 10 00 38 F5 10 85 08 22 58 1D 7F C2 30 7A FA 6B")


    def test_find_shape_sets_zero_lower_and_strict_7000_upper_then_replaces_only_link(self):
        self.assert_source(0x7F89EF,
            "c2202020c78db116e220aca8128c3614c220a90000853ea9581b853aadb11622f81e7f"
            "c00000e220d0045c228a7f94064cbeca740674074cbeca")

    def test_nearest_search_skips_owner_preserves_first_tie_and_has_distinct_filters(self):
        self.assert_source(0x7F1EF8,
            "8604ae3614d003823900c90000f03a850a643ce404f01db504c50ad017a404229f247f"
            "c220adde12c53a1008c53e3004853a863cb400bbd0dab4008c3614a43ca6046b"
            "a00000a6046b643ce404f01eb522290400f017a404229f247fc220adde12c53a1008"
            "c53e3004853a863cb400bbd0d98c3614a43ca6046b")


if __name__ == "__main__":
    unittest.main()
