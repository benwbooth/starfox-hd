#!/usr/bin/env python3
"""Assembly-backed checks for native path control; no gameplay execution."""

from pathlib import Path
import hashlib
import re
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM
from extract_path import PathAddress, PathExtractor
from path_semantics import PATH_SEMANTIC_BY_OPCODE


ROOT = Path(__file__).resolve().parents[3]


@unittest.skipUnless(Path(DEFAULT_ROM).is_file(), "retail SF2 ROM is not present")
class PathControlStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()
        cls.extractor = PathExtractor(cls.rom)

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_hit_toggle_sprite_graph_and_both_independent_parent_spawns(self):
        self.assert_source(0x098486,
            "6D 27 C4 4D 00 08 52 99 2E C4 5C CD F7 EF 4A AD 84 0A "
            "1D 00 61 03 1E 01 04 44 78 69 27 C7 84 1D 00 78 1D 01 16 A5 84 "
            "48 4C B2 84 42 4B AD 84 B8 4A BB 84 0A 19 49 5C 4C C1 84 42 "
            "4B BB 84 17 94 84 62 2D 1D 00 78 1D 01 44 0F")
        self.assert_source(0x0881CA, "F5 B0 BE 88 84 64 46 10 FF 28 00 A0 00 0A")
        self.assert_source(0x08897D, "F5 B0 BE 86 84 04 00 00 00 00 00 E2 FF 01")
        # The apparent NOP at 84B5 is a verified RTS helper, not a stub.
        self.assert_source(0x7FAFF6, "20 FC AF 4C E8 CA 60")

    def test_growing_sprite_and_shape_filtered_scenery_entries_and_spawns(self):
        self.assert_source(0x088059, "5C 4D 00 10 61 0F 07 99 02 44 78 0C 94 BE 04 19")
        self.assert_source(0x088210, "F5 5C BE 59 00 0A 0A 00 00 78 00 78 00 01")
        self.assert_source(0x08FF78, "41 B6 81 2B 04 78 CC 88 7F 8A 2B 04 98 D2 8A 7F 00 67 41 52 86 19")
        self.assert_source(0x08D916, "F5 C0 D6 78 7F 64 00 00 00 00 00 00 00 09")

    def test_independent_pulse_jitter_and_fade_sprite_graphs_and_installers(self):
        self.assert_source(0x0982E3,
            "4D 00 10 5C FD 1C 00 1D 00 61 03 61 04 1E 01 08 44 61 04 "
            "1E FF 08 44 44 78 61 07 1E 01 08 44 0F 75 EC 55 0C 2E 6D 99 6D 99 42")
        self.assert_source(0x098285, "58 2E 1F 5C F7 EF 4D 00 08 52 99 2E 17 2D 84")
        self.assert_source(0x09842D,
            "2A A2 01 58 84 6D A9 58 A1 7F 07 A1 C0 62 A9 55 0C A1 45 "
            "58 A1 7F 07 A1 C0 62 A9 55 0E A1 45 58 A1 7F 07 A1 C0 62 A9 "
            "55 10 A1 45 5C 1D 00 78 61 07 1E 01 08 44 0F")
        self.assert_source(0x098721, "5D 7C BD E3 82 64 F1")
        self.assert_source(0x098714, "5D 7C BD 85 82 64 00")
        self.assert_source(0x0888B3, "5D 7C BD 58 84 64 00")

    def test_sound_and_shrinking_sprite_graph_bytes_and_independent_spawn_records(self):
        self.assert_source(0x09838F,
            "FA 88 17 96 83 FA 89 4D 00 08 5C F7 EF 52 99 2E 1D 00 78 62 2D 1E 01 08 44 0F")
        self.assert_source(0x0983C2,
            "4D 00 08 52 99 2E 5C F7 EF F8 EC 83 58 A1 3F 07 A1 E0 55 0C A1 "
            "58 A1 3F 07 A1 E0 55 0E A1 58 A1 3F 07 A1 E0 55 10 A1 03 0D 0F 6D A2 55 0E A2 6F 99 42")
        self.assert_source(0x09832E,
            "FA 96 6D A9 17 38 83 00 09 83 4D 00 08 5C F7 EF 52 99 2E 1D 00 78 "
            "62 2D 1E 01 02 6D A1 8A 2A A1 04 5D 83 69 A9 5B 83 00 09 83 17 5D 83 FA 96 44 0F")
        self.assert_source(0x08B34E, "5D 04 BF 94 83 08 32")
        self.assert_source(0x08B3EC, "5D 04 BF 8F 83 08 32")
        self.assert_source(0x08F3F3, "5D FC C0 C2 83 64 18")
        self.assert_source(0x09861D, "5D 8C C0 2E 83 0A 08")

    def test_fixed_count_sprite_graph_bytes_and_independent_spawn_records(self):
        self.assert_source(0x0990FD, "5C 8D 4D 00 20 08 0E 58 02 61 08 07 99 FD 44 0F")
        self.assert_source(0x099277, "5C 4D 00 18 1D 00 61 08 1E 01 08 44 0F")
        self.assert_source(0x09C520, "6B A1 4D 00 00 4E 99 2E 5C 61 08 1E 01 08 52 99 A1 44 0F")
        self.assert_source(0x098F63, "5D A8 C0 FD 90 0A 0A")
        self.assert_source(0x09926F, "5D 04 BF 77 92 0A 0A")
        self.assert_source(0x09C344, "5D D0 BD 20 C5 01 01")

    def test_held_effect_graphs_installers_and_entire_byte_indexed_color_table(self):
        self.assert_source(0x08CDF1, "5C 4D 00 20 0B 40 99 19")
        self.assert_source(0x09CFD4, "5C 00 29 0C 03 00 87 19")
        self.assert_source(0x08D667,
            "1D 01 FD 08 00 03 03 49 5C 19 90 2A FC 06 A1 89 6D A1 8A 2A A1 07 81 56 6B A1 42")
        self.assert_source(0x08CDA7, "F5 94 BE F1 4D 01 01 00 00 40 FC 40 15 07")
        self.assert_source(0x098121, "F5 9C BC D4 CF 01 01 00 00 00 00 00 00 01")
        self.assert_source(0x08D62E, "F5 9C BC 67 56 0A 0A 00 00 00 00 00 00 02")
        start = source_offset(0x06FC2A)
        self.assertEqual(hashlib.sha256(self.rom[start:start + 256]).hexdigest(),
                         "541f238f2bc0ac7b2f91a6cf6ce778a756b3ed903ac0afbc55bd6fe037d99b03")

    def test_pickup_history_import_export_and_collection_lifecycle(self):
        self.assert_source(0x7F9F75, "20 E7 9F C2 20 AD B7 16 99 00 00 4C BE CA")
        self.assert_source(0x7F9FE7,
            "20 E0 C4 C2 20 29 FF 00 A8 B9 5C D7 8D B7 16 E2 20 20 BC C4 20 47 CB 60")
        self.assert_source(0x7F9FAF, "20 BF 9F 29 FF 00 A8 AD B7 16 99 5C D7 4C BE CA")
        self.assert_source(0x7F9FBF, "20 BC C4 20 47 CB C2 20 B9 00 00 8D B7 16 20 4C C7 A8 60")
        # Authored yaw is copied to pickup identity. An already-collected
        # bit hides/ends the actor; successful collection imports, sets and
        # exports that same full mask. Zero identity bypasses both checks.
        self.assert_source(0x08C4CA,
            "4E 2E 14 6B 12 6B 14 6B 16 67 2E FD 44 7B A3 32 DA 2E A3 F9 44 17 FD 44")
        self.assert_source(0x08C4F9, "48 0F")
        self.assert_source(0x08C5DF, "67 2E EC 45 7B A3 32 D8 2E A3 80 A3 32 42")
        self.assert_source(0x08C5BB, "41 DF 45 9D 64 00 00 04 45 0F")
        self.assert_source(0x08C5C5, "41 DF 45 9D 64 00 00 1B 00 04 43 0F")
        self.assert_source(0x08C5D1, "41 DF 45 0B 08 A1 EB 1B 1E A1 9D 01 00 0F")
        self.assert_source(0x04B1C0,
            "8B 08 E2 20 A9 7E 48 AB C2 30 9C 86 D7 9C 88 D7 9C 8A D7 9C 8C D7 9C 8E D7")
        self.assert_source(0x08FBBA, "80 A3 32")

    def test_clipping_plane_setter_draw_publication_and_non_boolean_producers(self):
        self.assertEqual(PATH_SEMANTIC_BY_OPCODE[0xC9].rust_name, "SetObject1cef")
        self.assert_source(0x7FB4D7, "A9 01 9D EF 1C 4C E8 CA")
        self.assert_source(0x7FB511, "A9 00 9D EF 1C 4C E8 CA")
        self.assert_source(0x7F141E, "B9 EF 1C 9D 1E 00")
        # Independent producer assigns two, and its other arm clears. This
        # selection is not a collision/visibility flag or a boolean toggle.
        self.assert_source(0x07C096, "A9 02 9D EF 1C 80 0B A9 00 9D EF 1C")
        self.assert_source(0x08C53C,
            "C9 79 27 B5 1B 8A 2A 27 05 4D 45 0B 00 AE 41 1B 8A")
        # The operand resolver adds 1C41 only for extension encodings, so
        # AE aliases exactly the same 1CEF byte used by the fixed command.
        self.assert_source(0x7FCB47,
            "08 C2 20 8E B1 16 29 FF 00 89 80 00 F0 04 18 69 41 1C 18 6D B1 16 A8 28 60")

    def test_motion_fade_sprite_setup_and_independent_installers(self):
        # The full shared jitter/fade and saved-byte callback are pinned by
        # their own tests. These three entry prefixes join that exact tail.
        self.assert_source(0x0983F4,
            "6D A2 17 05 84 58 A1 1F 52 2E A1 17 05 84 58 2E 0F BD 0E 84 BE 0E 84 "
            "F8 63 84 5C F7 EF 4D 00 08 52 99 2E 0B 11 A1 2A 2D 01 2D 84 "
            "EE 99 A1 2A 84 00 06 8B 17 2D 84 00 06 70")
        self.assert_source(0x08A5FD, "5D 08 BE F4 83 64 77")
        self.assert_source(0x088443, "5D 24 BE F9 83 64 32")
        self.assert_source(0x08B025, "5D 08 BE 02 84 64 20")

    def test_hit_cycled_shape_complete_graph_and_child_installation(self):
        self.assert_source(0x08A0CD,
            "C4 2E F9 FD 04 0A 19 4C D8 20 42 4B D4 20 61 07 1C 01 08 44 78 "
            "61 07 1C FF 08 44 61 07 1C 01 08 44 FD 04 0A 19 4C F6 20 42 "
            "4B F2 20 61 08 1C FF 08 44 17 CF 20")
        self.assert_source(0x089B34, "F5 14 D0 CD 20 64 02 18 01 00 00 00 00 09")

    def test_mesh_effect_graph_bytes_and_installed_child_records(self):
        self.assert_source(0x099904, "5C 00 29 07 95 08 16 07 99")
        self.assert_source(0x099A3B, "00 29 5C 77 90 EC 77 8E EC 07 96 04 16 3E 9A")
        self.assert_source(0x09F1AE,
            "00 65 BA F1 00 1E 0B 80 12 17 C0 F1 75 F6 42 41 54 8D 2E 5C 8D 07 95 FE 16 C3 F1 "
            "8D 5C 41 54 8D F9 07 95 FE 16 CE F1")
        self.assert_source(0x098D54, "89")
        self.assert_source(0x098D61, "42")
        self.assert_source(0x09C1FF, "5C 61 06 1C 01 06 44 0F")
        self.assert_source(0x09FA07, "5C 00 29 03 14 0F")
        for address, record in [
            (0x0994D4, "F5 D8 DE 04 99 0A 0A 00 00 D4 FE 00 00 61"),
            (0x09955D, "F5 DC C1 3B 9A 0A 0A 00 00 2C 01 00 00 0B"),
            (0x09F161, "F5 3C C6 BD F1 01 01 00 00 2C 01 00 00 01"),
            (0x09F16F, "5D 10 DF AE F1 01 01"),
            (0x09F176, "F5 2C CA C9 F1 01 01 00 00 34 01 00 00 01"),
            (0x09C1D3, "33 40 E8 FF C1 00 80 00 01 01 05 00 F6 FF F6 FF 01"),
            (0x09F76C, "33 DC C1 07 FA C0 00 00 01 01 00 00 00 00 3C 00 01"),
        ]:
            self.assert_source(address, record)

    def test_published_weapon_level_branch_uses_literal_operand_and_direct_targets(self):
        self.assert_source(0x7FBF58, "20 BC C4 CD D4 1D D0 03 4C FF CA 4C A9 CA")
        self.assert_source(0x7FC4BC,
            "84 79 A0 01 00 A5 5E 29 EF 85 5E 8F 3A 30 00 B7 F9 8D 11 19 "
            "A5 5E 09 10 85 5E 8F 3A 30 00 AD 11 19 A4 79 60")
        self.assert_source(0x7FCAA9, "E2 20 C2 20 B5 2B 18 69 04 00 95 2B E2 20 4C 75 7E")
        self.assert_source(0x7FCAFF, "C2 20 20 4C C7 95 2B E2 20 4C 75 7E")
        self.assert_source(0x7FC74C,
            "84 79 A0 02 00 E2 20 A5 5E 29 EF 85 5E 8F 3A 30 00 C2 20 B7 F9 "
            "8D 11 19 E2 20 A5 5E 09 10 85 5E 8F 3A 30 00 C2 20 AD 11 19 A4 79 60")
        # Player publication and pilot exchange are separate from the path's
        # selected actor. The compared byte must not be recomputed there.
        self.assert_source(0x069CE1, "B9 06 6C 8D D4 1D")
        self.assert_source(0x06A38B, "AD D4 1D 48 AD DA 1D 8D D4 1D 68 8D DA 1D")
        self.assert_source(0x06DA07, "AD D4 1D 99 06 6C")
        # Weapon use masks the per-player level, but this path branch does
        # not. The separately scheduled upgrade saturates levels at three.
        self.assert_source(0x0DDDC0, "B4 2B B9 06 6C 29 03 D0 04 5C CE E0 0D")
        self.assert_source(0x7FBA7C, "B4 2B B9 06 6C C9 03 B0 04 1A 99 06 6C")

    def test_suspension_enters_movement_and_both_strategy_passes_skip_it(self):
        self.assert_source(0x7FBC80, "B5 26 09 40 95 26 4C DE 9D")
        self.assert_source(0x7F9DDE, "A5 5E 09 18 85 5E 8F 3A 30 00 B5 0B")
        # Callbacks run unconditionally, even with the suspension bit set.
        self.assert_source(0x7F9E66, "B5 22 29 01 D0 04 5C 70 9E 7F 20 A8 9A")
        # Exit cleanup clears contact bits, never the suspension bit.
        self.assert_source(0x7F9F09, "B5 26 29 FD 95 26 B5 26 29 FB 95 26 6B")
        # Each skip leads directly to the next live actor, bypassing both
        # strategy execution (and its sound service) and immediate retirement.
        self.assert_source(0x7F3519,
            "B5 26 29 40 F0 04 5C 2F 35 7F 9C D6 12 22 96 35 7F "
            "AD D6 12 D0 07 B4 00 BB D0 DD")
        self.assert_source(0x7F3565,
            "B5 26 29 40 F0 04 5C 7B 35 7F 9C D6 12 22 96 35 7F "
            "AD D6 12 D0 07 B4 00 BB D0 E5")

    def test_forward_projection_host_rotation_order_and_byte_inputs(self):
        self.assert_source(0x0DB751, "E2 20 64 02 64 08 A9 7F 85 97")
        self.assert_source(0x0DB7C0, "B5 16 22 F0 3B 7F")
        self.assert_source(0x0DB7D2, "B5 12 22 4E 3A 7F")
        self.assert_source(0x0DB7E4, "B5 14 22 A9 38 7F")
        # Each rotated byte becomes the high byte of a signed coefficient.
        self.assert_source(0x0DB810, "C2 20 B9 0C 00 EB 29 00 FF 8F 68 00 70")
        self.assert_source(0x0DB821, "B9 0E 00 EB 29 00 FF 8F 2C 00 70")
        self.assert_source(0x0DB830, "B9 10 00 EB 29 00 FF 8F 2E 00 70")
        self.assert_source(0x0DB863, "A9 01 A2 86 FC 22 7B 78 7F")

    def test_geometry_dot_product_doubles_each_word_before_signed_high_product(self):
        # Three ADD-self/FMUL pairs, separate high-word truncations, and
        # wrapping word sums. A combined wide dot product is not equivalent.
        self.assert_source(
            0x01FC86,
            "60 3F DF 3E DF 3D A0 13 3D A6 34 50 11 9F "
            "3D A0 14 3D A6 16 50 9F 11 51 "
            "3D A0 15 3D A6 17 50 9F 51 3E A0 55 00 01",
        )

    def test_square_root_uses_sixteen_word_remainder_iterations(self):
        self.assert_source(
            0x01FA64,
            "A6 00 A8 00 AC 10 FD 6E FA 25 55 24 04 28 04 "
            "25 55 24 04 28 04 26 56 B6 17 56 B7 68 0D 04 "
            "28 3D 67 D6 3C 25 9B 01",
        )

    def test_normalization_division_preconditions_sign_bits_and_bounds_remainder(self):
        self.assert_source(
            0x01FBAB,
            "20 B0 0B 06 01 26 B6 0A 06 01 26 03 03 24 97 "
            "AC 10 24 54 04 2F 1D 66 0C 07 01 24 04 3C 04 "
            "9B 03 56 24 54 3C 04 9B 03",
        )
        self.assert_source(
            0x01FC0D,
            "26 10 A9 00 03 09 04 01 05 FA D9 60 F4 FF 7F "
            "2C B9 09 07 01 2F 1D 24 54 3C 04 26 16 94 FF AB FB 01 24 16",
        )
        self.assert_source(
            0x01FC70,
            "3D A6 34 26 56 B1 9F 3E A0 13 B2 9F 3E A0 14 B3 9F 3E A0 15 00 01",
        )

    def test_shared_byte_trig_tables_match_the_sf2_rotation_callees(self):
        source = (ROOT / "rust/sf-core/src/snes_trig.rs").read_text()
        for name, address in (
            ("SINTAB", 0x7F3D92), ("COSTAB", 0x7F3DD2),
            ("SINTAB", 0x008E26), ("COSTAB", 0x008E66),
        ):
            match = re.search(rf"pub static {name}: \[i8; 256\] = \[(.*?)\];", source, re.S)
            self.assertIsNotNone(match)
            values = [int(value.strip()) & 255 for value in match[1].split(",") if value.strip()]
            self.assertEqual(len(values), 256)
            offset = source_offset(address)
            self.assertEqual(bytes(values), self.rom[offset:offset + 256])

    def test_crossing_trigger_latches_and_tests_both_players_in_priority_order(self):
        self.assert_source(0x7F9C0C, "B5 24 29 40 F0 04 5C 52 9C 7F")
        self.assert_source(0x7F9C16, "B5 24 09 40 95 24 AC C3 12")
        self.assert_source(0x7F9C37, "AC C5 12 F0 16 22 51 B7 0D")
        self.assert_source(0x7F9C52, "AC C3 12 F0 1F 22 51 B7 0D 89 80")
        self.assert_source(0x7F9C76, "AC C5 12 F0 1F 22 51 B7 0D 89 80")
        self.assert_source(0x7F9CA9, "B5 24 29 BF 95 24")
        self.assert_source(0x7F9CCA, "B5 24 29 BF 95 24")
        # Callback selection also changes which player later path ops target.
        self.assert_source(0x7F9CBA, "AC C3 12 8C 1F CF B5 24 29 7F 95 24")
        self.assert_source(0x7F9CDB, "AC C5 12 8C 1F CF B5 24 09 80 95 24")

    def test_loop_wait_and_periodic_clock_branch_instructions(self):
        self.assert_source(0x7F96C4, "B9 61 6A 3A F0 11 99 61 6A")
        self.assert_source(0x7F84FB, "20 BC C4 D5 17 D0 04 5C CF CA 7F F6 17 4C DE 9D")
        self.assert_source(0x7FCACF, "A9 00 95 17")
        self.assert_source(0x7F9D46, "A9 01 80 18 A9 03 80 14 A9 07 80 10")
        self.assert_source(0x7F9D52, "A9 0F 80 0C A9 1F 80 08 A9 3F 80 04 A9 7F 80 00")
        self.assert_source(0x7F9D62, "25 C4 F0 02 80 20")

    def test_variable_loops_snapshot_unsigned_count_after_pushing_body_cursor(self):
        self.assert_source(0x7F95CA, "C2 20 B5 2B 1A 1A E2 20 C2 20 8D 69 B2 BD DE 1C 22 0F 1A 7F 9D DE 1C E2 20 20 BC C4 20 47 CB C2 20 B9 00 00 29 FF 00 E2 20 C2 20 8D 69 B2 BD DE 1C 22 0F 1A 7F 9D DE 1C E2 20 4C D3 CA")
        self.assert_source(0x7F9607, "C2 20 B5 2B 1A 1A E2 20 C2 20 8D 69 B2 BD DE 1C 22 0F 1A 7F 9D DE 1C E2 20 20 BC C4 20 47 CB C2 20 B9 00 00 E2 20 C2 20 8D 69 B2 BD DE 1C 22 0F 1A 7F 9D DE 1C E2 20 4C D3 CA")

    def test_variable_wait_rereads_byte_and_rewinds_escaped_opcode_until_equal(self):
        self.assert_source(0x7FBCD8, "20 BC C4 20 47 CB B9 00 00 D5 17 D0 04 5C CF CA 7F F6 17 C2 20 D6 2B E2 20 4C DE 9D")

    def test_hostile_laser_authored_control_flow(self):
        expected = {
            0xEE4C: ("SetByte", "0b0a2d"),
            0xEE4F: ("ImportByteIndexed", "7a2e96"),
            0xEE52: ("IfSameByte", "2a2e0068ee"),
            0xEE57: ("IfSameByte", "2a2e0262ee"),
            0xEE5C: ("SetByte", "0b032e"),
            0xEE62: ("SetByte", "0b042e"),
            0xEE68: ("SetByte", "0b022e"),
            0xEE6E: ("IfSelectedDistanceLess", "14e02e74ee"),
            0xEE73: ("End", "0f"),
            0xEE74: ("QueueSelectedMarkerClass2", "fa72"),
            0xEE81: ("SetVelocity", "061e"),
            0xEE9B: ("SetVelocity", "063f"),
            0xEE9D: ("IfSelectedDistanceLess", "14e803adee"),
            0xEEA2: ("ContractSelectedRadius", "ae7f"),
            0xEEA4: ("ContractSelectedRadius", "ae7f"),
            0xEEA6: ("ContractSelectedRadius", "ae7f"),
            0xEEA8: ("FaceSelectedImmediate", "000f"),
            0xEEAA: ("Goto", "169dee"),
            0xEEAD: ("FaceSelectedImmediate", "000f"),
            0xEEAF: ("SetVelocity", "063f"),
            0xEEB1: ("GotoImmediate", "17bcee"),
            0xEEBC: ("ScheduleTrigger", "4ad5ee01"),
            0xEEC0: ("ScheduleTrigger", "4ac8ee0d"),
            0xEEC4: ("DoQueue", "6128"),
            0xEEC6: ("Next", "44"),
            0xEEC7: ("End", "0f"),
            0xEEC8: ("ForceTriggerPath", "4cccee"),
            0xEECC: ("CancelTrigger", "4bd5ee"),
            0xEECF: ("CancelTrigger", "4bc8ee"),
            0xEED2: ("Wait", "030f"),
            0xEED4: ("End", "0f"),
            0xEED5: ("IfSelectedWithinYawArc", "a420daee"),
            0xEEDA: ("FaceSelectedSmooth", "09"),
            0xEF18: ("DoVariableByte", "620a"),
            0xEF1A: ("Next", "44"),
            0xEF1B: ("End", "0f"),
            0xEF1C: ("IfHitGround", "1a000026ef"),
            0xEF21: ("IfCurrentAtOrAboveCollisionTarget", "005326ef"),
            0xEF26: ("ForceTriggerPath", "4c1bef"),
            0xF018: ("ForceTriggerPath", "4c1cf0"),
            0xF01B: ("Return", "42"),
            0xF01C: ("End", "0f"),
        }
        for address, (semantic, raw) in expected.items():
            command = self.extractor.decode_command(PathAddress(address))
            self.assertEqual(PATH_SEMANTIC_BY_OPCODE[command.opcode].rust_name, semantic)
            self.assertEqual(command.raw_hex, raw)

    def test_movement_acceleration_and_bank_rounding(self):
        self.assert_source(
            0x7F9DF0,
            "B5 18 D5 0A 30 0B 38 F5 0B D5 0A 10 13 B5 0A 80 09 "
            "18 75 0B D5 0A 30 08 B5 0A 95 18 74 0B 80 02 95 18",
        )
        self.assert_source(
            0x7F9E29,
            "B5 16 C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 14 95 14",
        )

    def test_ordinary_integration_precedes_callbacks_but_attached_motion_follows(self):
        self.assert_source(0x7F9E62, "22 24 2C 7F")
        self.assert_source(0x7F9E70, "20 A8 9A")
        self.assert_source(
            0x7F2C24,
            "C2 20 B5 0C 18 75 32 95 0C B5 0E 18 75 34 95 0E "
            "B5 10 18 75 36 95 10 E2 20 6B",
        )
        self.assert_source(0x7F9E9F, "C2 20 BD CF 1C 18 7D 32 00 9D CF 1C")
        self.assert_source(0x7F9EAD, "C2 20 BD D1 1C 18 7D 34 00 9D D1 1C")
        self.assert_source(0x7F9EBB, "C2 20 BD D3 1C 18 7D 36 00 9D D3 1C")
        # The forced-path post-callback hook has no additional side effects.
        self.assert_source(0x7FAFFC, "60")

    def test_selected_displacement_skips_only_horizontal_axis(self):
        self.assert_source(0x7F9F30, "89 04 F0 04 5C 42 9F 7F")
        self.assert_source(0x7F9F38, "C2 20 AD 1C 1E 18 75 0C 95 0C")
        self.assert_source(0x7F9F42, "C2 20 AD 20 1E 18 75 10 95 10 E2 20 60")

    def test_direction_velocity_keeps_doubled_byte_products_and_world_scaling(self):
        self.assert_source(0x7F3078, "AD 12 15 49 FF 1A A8 AD 11 15 AA")
        self.assert_source(0x7F30A0, "A5 85 30 18 0A 8F 02 42 00")
        self.assert_source(0x7F30BC, "49 FF 1A 0A 8F 02 42 00")
        self.assert_source(0x7F30F5, "A5 02 30 18 0A 8F 02 42 00")
        self.assert_source(0x7F85FC, "C2 20 16 32 16 32 16 34 16 34 16 36 16 36 E2 20 60")

    def test_selected_distance_uses_wrapped_horizontal_deltas_and_geometry_length(self):
        self.assert_source(0x7F8C30, "B9 0C 00 38 F5 0C 8F 26 00 70")
        self.assert_source(0x7F8C3A, "A9 00 00 8F 28 00 70")
        self.assert_source(0x7F8C41, "B9 10 00 38 F5 10 8F 2A 00 70")
        self.assert_source(0x7F8C50, "A9 01 A2 72 FB 22 7B 78 7F")

    def test_smooth_face_clamps_small_differences_then_rounds_two_halves(self):
        self.assert_source(
            0x7F87DB,
            "38 F5 12 C9 00 30 08 C9 04 10 0A A9 04 80 06 C9 FC 30 02 A9 FC "
            "C9 80 6A 10 02 69 00 C9 80 6A 10 02 69 00 18 75 12",
        )
        self.assert_source(0x7F880A, "49 FF 1A D5 14")
        self.assert_source(0x7FAB69, "85 97 06 97 18 75 14 18 65 02 C5 97 B0 04")

    def test_hit_callback_and_surface_comparisons(self):
        self.assert_source(0x7F9D38, "FA DA B5 22 29 02 D0 04 5C 88 9D 7F")
        self.assert_source(0x7FBF9C, "C2 20 B5 0E 38 E5 08 30 03 4C F3 CA")
        self.assert_source(0x7F8CA3, "C2 20 20 20 C7 18 75 0E")

    def test_axis_adds_wrap_rotation_bytes_and_sign_extend_position_operands(self):
        for address, field in [(0x7F863A, 0x12), (0x7F8645, 0x14), (0x7F8650, 0x16)]:
            self.assert_source(address, f"20 BC C4 18 75 {field:02X} 95 {field:02X} 4C D3 CA")
        for address, field in [(0x7F865B, 0x0C), (0x7F8675, 0x0E), (0x7F868F, 0x10)]:
            self.assert_source(address, f"20 BC C4 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 18 75 {field:02X} 95 {field:02X} 4C D3 CA")
        self.assert_source(0x7F86A9, "20 BC C4 20 47 CB 20 E0 C4 C2 20 89 80 00 F0 05 09 00 FF 80 03 29 FF 00 18 79 00 00 99 00 00 E2 20 4C BE CA")

    def test_surface_path_preserves_group_and_keeps_support_and_flags_on_both_edges(self):
        self.assert_source(0x7FBF86, "A5 5E 29 E7 85 5E 8F 3A 30 00 BD EA 1C 48 22 3A AF 0D 68 9D EA 1C C2 20 B5 0E 38 E5 08 30 03 4C F3 CA 4C BE CA")
        self.assert_source(0x0DB20B, "AD 5F 19 9F E8 1C 7E F0 56")
        self.assert_source(0x0DB26A, "E2 20 AD 61 19 9F EA 1C 7E AF 8D 1A 00 9F EB 1C 7E 20 23 AF 7A AB 28 6B")
        self.assert_source(0x7FCAF3, "C2 20 20 20 C7 95 2B E2 20 4C 75 7E")
        self.assert_source(0x7FCABE, "E2 20 C2 20 B5 2B 18 69 03 00 95 2B E2 20 4C 75 7E")

    def test_surface_object_order_eligibility_and_unmasked_automatic_animation(self):
        self.assert_source(0x0DAF4E, "22 68 1B 7F AE A8 12 D0 03 82 9B 02 22 F0 1B 7F")
        self.assert_source(0x7F1BFE, "E4 3A F0 6E B5 31 29 04 00 D0 67 B5 24 29 04 00 D0 60 B4 04")
        self.assert_source(0x0DAF86, "BF CB 1C 7E 10 04 29 7F 80 04 A5 C4 29 FF 85 3E 64 3F")
        self.assert_source(0x7F1B76, "AD 4D 1B 29 07 00 C9 00 00 F0 05 A9 00 20 80 03 A9 00 40 8D 5D 19 9C 5F 19 9C 61 19 9C 49 19 A9 00 00 8D 8B 1A 8D 8D 1A")

    def test_surface_mode_writers_preserve_high_bits_and_complete_authored_path(self):
        self.assert_source(0x068638, "AD 4D 1B 29 F8 09 01 8D 4D 1B")
        self.assert_source(0x068656, "AD 4D 1B 29 F8 09 02 8D 4D 1B")
        self.assert_source(0x0687CC, "AD 4D 1B 29 F8 09 00 8D 4D 1B")
        self.assert_source(0x09EEED, "00 2c 0b 0a 0a 06 32 8a 2b 04 50 e4 fe ee f8 2a ef 8d 00 76 08 4a 18 f0 08 79 a1 4d 1b 2a a1 00 15 ef f8 1c ef 17 18 ef f8 21 ef 62 0a 44 0f 1a 00 00 26 ef 00 53 26 ef 42 4c 1b ef 42 00 4b 42")

    def test_primary_motion_surface_path_initializes_combat_sound_and_shared_callbacks(self):
        self.assert_source(0x09EE10, "0b 0a 2d 0b 04 2e 0b 28 0a 06 50 41 8a e7 0c 00 c0 04 4d 00 05 00 05 0c fa 73 17 fe ee")

    def test_occupancy_surface_path_and_its_independent_three_frame_effect(self):
        self.assert_source(0x09EC98, "7a 2e 96 2a 2e 00 b1 ec 2a 2e 02 ab ec 0b 04 2e 17 b4 ec 0b 06 2e 17 b4 ec 0b 02 2e 0b 78 2d 0c 6c c1 04 00 76 08 4d 00 05 5d b0 be a1 f5 01 01 fa 75 00 05 03 bc d2 ec 71 fd 00 5c 06 14 5c 03 03 5b 4a 18 f0 08 f8 e4 ec 03 32 0f 00 5f f2 ec 1a 00 00 f2 ec 00 53 f2 ec 42 4c f6 ec 42 0f")
        self.assert_source(0x09F5A1, "4d 00 10 5c 61 03 1e 01 02 44 0f")
        self.assert_source(0x00BEB0, "00 00 00 00 00 00 00 02 00 00 10 00 10 00 10 00 10 00 fd 85 b0 be b0 be b0 be b0 be")

    def test_collision_rotation_scales_input_words_and_preserves_product_carries(self):
        self.assert_source(0x01FD6D, "3D A0 34 50 11 50 3D A0 17 50 13 50")
        self.assert_source(0x01FE78, "F0 66 8E 1E 52 EF 15 4D F0 26 8E 1E 52 EF 19 4D")
        self.assert_source(
            0x01FE88,
            "21 16 B9 18 3D 9F 24 17 23 16 B5 3D 9F 27 54 3D 58 12 96 "
            "23 16 B9 18 3D 9F 24 17 21 16 B5 3D 9F 27 64 3D 68 13 96",
        )

    def test_collision_polygon_uses_full_signed_cross_product_and_inclusive_edges(self):
        self.assert_source(
            0x01FD2F,
            "B9 13 67 BA 15 68 B1 17 67 B2 18 68 28 16 23 3D 9F "
            "24 18 25 16 27 3D 9F 24 15 25 68 27 3D 63 0B 0B",
        )
        self.assert_source(0x01FD50, "29 17 2A 18 3C 01 A0 00 05 03 01 A0 FF 3E A0 0B 00 01")

    def test_collision_plane_truncates_before_doubling_and_signed_division(self):
        self.assert_source(
            0x01FA39,
            "3D A0 13 3D A6 55 9F 11 50 3D A0 15 3D A6 57 9F 50 11 51 "
            "3D A0 14 21 60 21 15 A4 00 3D A6 56 94 FF 8A FA 01 24 96 3E A4 14",
        )
        self.assert_source(
            0x01FA8A,
            "02 B5 17 3D C6 60 26 B6 0A 02 16 66 25 B5 0A 05 "
            "14 64 15 3D 65 24 54 B5 04 AC 10 2F 1D 66 0C 08 01 "
            "24 04 3C 04 05 06 01 56 24 54 3C 04 27 B7 0A 03 24 4F D4 9B 01",
        )

    def test_surface_admission_uses_no_extra_footprint_margin(self):
        self.assert_source(0x0DB07E, "BD 05 00 4A 18 7D 01 00 38 E5 02 DD 05 00 B0 10")
        self.assert_source(0x0DB08E, "BD 07 00 4A 18 7D 03 00 38 E5 97 DD 07 00 90 09")
        # The +2 is only the later vertical admission margin.
        self.assert_source(0x0DB157, "A5 08 38 79 0C 00 1A D5 0E 10 03 82 6D 00")
        self.assert_source(0x0DB165, "A5 08 CD 5D 19 30 03 82 63 00")
        # Both failed and accepted surface candidates resume at the NEXT
        # OBJECT link, not the compound-group loop at $B09E.
        self.assert_source(0x0DB1D8, "C2 20 AC 49 19 B6 00 F0 14")
        self.assert_source(0x0DB1FE, "AD 5D 19 C9 00 20 D0 03 A9 00 00 85 08")

    def test_world_occupancy_quantization_and_bit_masks(self):
        self.assert_source(0x00B063, "01 00 02 00 04 00 08 00 10 00 20 00 40 00 80 00")
        self.assert_source(0x0DDAD1, "A5 02 EB 4A 29 7F 00 85 5F")
        self.assert_source(0x0DDAED, "A5 97 29 00 FE EB 0A 0A 0A 18 65 5F 85 5F")
        self.assert_source(0x0DDAFB, "A5 04 18 69 FF 01 EB 4A 29 7F 00 85 04")
        self.assert_source(0x0DDB08, "A5 E4 18 69 FF 01 EB 4A 29 7F 00 85 E4")
        self.assert_source(0x0DDB75, "A5 02 EB 4A 29 7F 00 85 5F")
        self.assert_source(0x0DDB91, "A5 97 29 00 FE EB 0A 0A 0A 18 65 5F AA")
        self.assert_source(0x0DDBA0, "BD 36 CF 25 0A 85 02")

    def test_occupancy_path_handler_short_circuits_and_never_reads_ifnot(self):
        self.assert_source(0x7FB73E, "AC 1F CF DA BB 7A 5A B4 2B B9 EB 6B 7A DA BB 7A 89 80 F0 04 5C BE CA 7F A5 5E 29 E7 85 5E 8F 3A 30 00 C2 20 B5 0C 85 02 B5 10 85 97 22 71 DB 0D E2 20 A5 02 F0 03 82 7C 13 4C BE CA")
        self.assert_source(0x7FCAF3, "C2 20 20 20 C7 95 2B E2 20 4C 75 7E")
        self.assert_source(0x7FCABE, "E2 20 C2 20 B5 2B 18 69 03 00 95 2B E2 20 4C 75 7E")

    def test_world_marker_half_row_boundary_and_countdown_widths(self):
        # At every eighth BYTE boundary, subtract a full 16-byte row.
        self.assert_source(
            0x0DDB36,
            "18 26 0A 90 10 26 0A E8 C2 20 8A 89 07 00 D0 05 38 E9 10 00 AA",
        )
        # Width decrements in byte mode; depth decrements in word mode.
        self.assert_source(0x0DDB4B, "E2 20 C6 04 D0 CF")
        self.assert_source(0x0DDB59, "A5 5F 18 69 10 00 C9 00 08 90 04 38 E9 00 08 85 5F C6 E4 D0 A7")
        # Selected auxiliary exemption short-circuits the occupancy test.
        self.assert_source(0x7FB745, "B4 2B B9 EB 6B 7A DA BB 7A 89 80 F0 04 5C BE CA 7F")

    def test_shared_allocator_marks_effects_only_after_consuming_final_slot(self):
        self.assert_source(0x7F2925, "C2 20 9B AE AA 12 D0 06 E2 20 BB 4C 69 29")
        self.assert_source(0x7F295D, "DA AE AA 12 D0 03 20 79 29 FA 38 6B")
        self.assert_source(0x7F2979, "DA 5A AE A8 12 B4 00 F0 37 5A B5 20 29 10")
        self.assert_source(
            0x7F298D,
            "C2 20 B5 04 C9 98 BD F0 16 C9 B4 BD F0 11 C9 D0 BD F0 0C "
            "C9 EC BD F0 07 C9 08 BE F0 02 80 08 E2 20 B5 25 09 08 95 25",
        )
        source = (ROOT / "rust/sf2-data/src/shape_data.rs").read_text()
        for index, shape in enumerate((0xBD98, 0xBDB4, 0xBDD0, 0xBDEC, 0xBE08), start=9):
            self.assertRegex(source, rf"header_index: {index},\s+shape_id: 0x{shape:04X},")
        # The new allocation is cleared after the sweep, including its
        # retirement flag, before the requested shape is installed.
        self.assert_source(0x7F2A1C, "22 25 29 7F")
        self.assert_source(0x7F2A2E, "22 BC 29 7F C2 20 A5 5F 99 04 00")
        self.assert_source(0x7F29C4, "A9 00 5A DA A0 3B 00 95 04 E8 88 D0 FA")

    def test_weapon_allocation_temporarily_changes_pressure_head_and_clears_bank(self):
        self.assert_source(0x0DE019, "AD A8 12 48 E2 20 8E A8 12")
        self.assert_source(0x0DE02B, "22 17 2A 7F B0 04 5C 9A E0 0D")
        self.assert_source(0x0DE035, "C2 20 68 8D A8 12")
        self.assert_source(0x03AB2A, "22 D2 2B 7F A9 00 99 16 00")
        self.assert_source(0x03AC1D, "B5 18 99 17 00 96 1C 94 1C 6B")

    def test_strategy_epoch_increments_word_clock_once_and_saves_live_cursor(self):
        self.assert_source(0x7F3504, "E6 C4 D0 02 E6 C5")
        self.assert_source(0x7F350E, "AE A8 12 AF 30 30 00 29 20 F0 25")
        self.assert_source(0x7F3519, "B5 26 29 40 F0 04 5C 2F 35 7F")
        self.assert_source(0x7F3523, "9C D6 12 22 96 35 7F AD D6 12 D0 07")
        self.assert_source(0x7F3536, "B4 00 22 56 33 7F 80 F3 8E 42 19")
        # Remainder loads the saved next actor and never increments the clock.
        self.assert_source(0x7F3560, "AE 42 19 F0 25 B5 26 29 40 F0 04 5C 7B 35 7F")
        self.assert_source(0x7F356F, "9C D6 12 22 96 35 7F AD D6 12 D0 07")
        self.assert_source(0x7F3582, "B4 00 22 56 33 7F 80 F3 8E 42 19")

    def test_strategy_selection_death_hit_pause_and_post_service_precedence(self):
        self.assert_source(0x7F35A8, "EC D6 14 D0 03 82 A0 00")
        self.assert_source(0x7F35B5, "B5 2D D0 35 B5 31 29 04 F0 04 5C EE 35 7F")
        self.assert_source(0x7F35C3, "B5 26 29 08 F0 04 5C DC 35 7F")
        self.assert_source(0x7F35CD, "C2 20 AD 84 1B 89 02 00 E2 20 F0 03 4C EE 35")
        self.assert_source(0x7F35DC, "A9 03 8D D1 12 C2 20 A9 55 A0 8D CF 12 82 4F 00")
        self.assert_source(0x7F35EE, "B5 31 29 FB 95 31 B5 20 29 80 F0 18")
        self.assert_source(0x7F35FA, "A9 03 8D D1 12 C2 20 A9 27 A3 8D CF 12 82 31 00")
        self.assert_source(0x7F3619, "B5 19 F0 59")
        self.assert_source(0x7F3622, "B5 26 29 08 F0 04 5C 3B 36 7F")
        self.assert_source(0x7F362C, "C2 20 AD 84 1B 89 02 00 E2 20 F0 03 4C 50 36")
        self.assert_source(0x7F3661, "AD D3 1C 89 01 D0 0B BD CC 1C F0 06 A0 3F 03 20 B8 36")

    def test_retirement_detaches_children_then_clears_all_incoming_links(self):
        self.assert_source(0x7F336A, "22 25 34 7F 22 B2 33 7F 22 4F 34 7F C2 20 22 CB 34 7F")
        self.assert_source(0x7F3470, "B9 29 00 F0 17 C5 3C F0 03 A8 80 F4 B5 29 99 29 00")
        self.assert_source(0x7F34A4, "B4 29 B5 23 29 FB 95 23 C2 20 A9 00 00 95 06")
        self.assert_source(0x7F34B5, "B5 25 29 01 D0 04 5C C5 34 7F B5 25 09 08 95 25")
        self.assert_source(0x7F34CB, "DA 8A AE A8 12 D5 1C D0 02 74 1C D5 06 D0 02 74 06 B4 00 BB D0 EF")
        # Only after dependent services/link cleanup does the slot return to
        # the free-list head, preserving last-freed-first reuse.
        self.assert_source(0x7F339D, "AD AA 12 95 00 8E AA 12")

    def test_cleanup_samples_live_flags_and_saves_next_before_retirement(self):
        self.assert_source(0x7F4037, "AE A8 12 B5 25 29 08 D0 04 5C 54 40 7F")
        self.assert_source(0x7F4044, "C2 20 B5 00 48 E2 20 22 46 33 7F 7A BB 4C B3 40")
        self.assert_source(0x7F40B0, "9B B6 00 D0 85")

    def test_weapon_muzzle_uses_source_bank_then_pitch_yaw_and_word_scale(self):
        self.assert_source(0x03AB2A, "22 D2 2B 7F A9 00 99 16 00")
        self.assert_source(0x03AB6C, "B5 16 22 F0 3B 7F A5 04 85 02 A5 0A 85 08 A5 E4 85 97")
        self.assert_source(0x03AB7E, "B5 12 22 4E 3A 7F A5 04 85 02 A5 0A 85 08 A5 E4 85 97")
        self.assert_source(0x03AB90, "B5 14 22 A9 38 7F C2 20 A5 04 0A 0A 85 04 A5 0A 0A 0A 85 0A A5 E4 0A 0A 85 E4")
        self.assert_source(0x03ABE0, "22 C3 21 7F 18 6D B7 14 95 12 22 EB 21 7F 49 FF 1A 18 6D B6 14 95 14")
        self.assert_source(0x03ABFC, "B9 12 00 18 6D B7 14 99 12 00 B9 14 00 18 6D B6 14 99 14 00")
        self.assert_source(0x7F8B8B, "B5 25 09 08 95 25 4C FD 9E")

    def test_sound_queue_producers_wrap_sixteen_words_without_testing_full(self):
        self.assert_source(0x7FA43E, "DA AE 16 1D CC C3 12 F0 08 C0 3F 03 F0 03 09 00 80")
        self.assert_source(0x7FA44F, "9D F6 1C E2 20 AD 16 1D 1A 1A 29 1F 8D 16 1D FA 60")
        self.assert_source(0x7F6E09, "DA AE 16 1D 9D F6 1C E2 20 8A 1A 1A 29 1F 8D 16 1D C2 20 FA 6B")
        self.assert_source(0x7F0FA5, "AE 18 1D EC 16 1D F0 23 BD F6 1C")
        self.assert_source(0x7F0FBB, "AD 18 1D 1A 1A 29 1F 8D 18 1D")

    def test_path_sound_distance_branches_skip_angle_for_far_and_distance_only(self):
        self.assert_source(0x7FA4CE, "E0 20 03 90 27 80 32")
        self.assert_source(0x7FA4D5, "AD 34 1C E0 20 03 90 0E E0 14 05 90 07 09 60 8D 34 1C 80 11")
        self.assert_source(0x7FA4E9, "09 30 8D 34 1C AD 35 1C 3A F0 06 AE 37 1C 20 50 A5")
        self.assert_source(0x7F8C3A, "A9 00 00 8F 28 00 70 B9 10 00 38 F5 10 8F 2A 00 70")
        self.assert_source(0x7F8C50, "A9 01 A2 72 FB 22 7B 78 7F")

    def test_path_sound_bearing_uses_live_listener_and_half_open_rear_rejection(self):
        self.assert_source(0x7FA565, "22 58 1D 7F E2 20 EB 38 F9 15 00 C9 10 90 40 C9 F0 B0 3C")
        self.assert_source(0x7FA578, "48 AD 35 1C C9 03 F0 0B 68 C9 70 90 1B C9 90 90 2B 80 20")
        self.assert_source(0x7FA58B, "68 C9 40 90 10 C9 C0 B0 17 C2 20 A9 FF FF 8D 33 1C")
        self.assert_source(0x7FA5A0, "AD 34 1C 18 69 20 8D 34 1C 80 09 AD 34 1C 18 69 10 8D 34 1C")


    def test_contact_pool_capacity_lookup_insertion_and_refresh(self):
        self.assert_source(0x03A60D, "A9 9A 31 8D 81 12 A0 3C 00 AA 18 69 0B 00 9D 00 00 88 D0 F5 9E 00 00")
        self.assert_source(0x7F3ED9, "BC 1E 00 F0 0C BB DD 04 00 F0 5D BC 00 00 BB D0 F5")
        self.assert_source(0x7F3EF2, "AE 81 12 D0 04 BB 4C 55 3F")
        self.assert_source(0x7F3F10, "B9 00 00 9D 00 00 8A 99 00 00 98 9D 02 00")
        self.assert_source(0x7F3F34, "E2 20 A9 02 9D 09 00 9E 08 00 9E 0A 00 E2 20 BD 09 00 09 01 9D 09 00 FE 0A 00")

    def test_contact_callbacks_precede_reverse_then_original_lifo_release(self):
        self.assert_source(0x7F33BD, "B4 1E F0 0B B9 00 00 48 22 A7 3F 7F 7A D0 F5 B5 1E F0 04 22 06 80 00")
        self.assert_source(0x7F3FA7, "DA 5A 20 59 3F BE 04 00 B9 06 00 A8 20 59 3F")
        self.assert_source(0x7F3FE0, "AD 81 12 9D 00 00 8E 81 12 FA A5 08 95 1E 7A FA")
        self.assert_source(0x7F401A, "AD 81 12 9D 00 00 8E 81 12 FA A5 08 95 1E 6B")
        self.assert_source(0x7F4090, "B9 00 00 48 B9 09 00 89 01 00 D0 06 22 A7 3F 7F 80 09 E2 20 29 FE 99 09 00 C2 20 7A D0 E2")
        self.assert_source(0x7F4878, "A5 F5 99 08 00 A5 F6 9D 08 00")


    def test_hit_response_new_callback_replaces_continuing_and_parameter_is_other_actor_byte(self):
        self.assert_source(0x03A3B2, "B9 09 00 29 02 F0 42 B9 09 00 29 FD 99 09 00")
        self.assert_source(0x03A3D1, "A9 06 22 3B 23 7F C0 00 00 D0 04 5C FB A3 03")
        # The registered new handler returns directly to damage, not A3FB.
        self.assert_source(0x03A3F0, "7A 5A A9 03 48 F4 2C A4 DC CF 12")
        self.assert_source(0x03A3FB, "7A 5A A9 05 22 3B 23 7F")
        # Restore OTHER ACTOR from stack before loading its auxiliary byte.
        # This is not the similarly offset touch-count byte of the contact.
        self.assert_source(0x03A41C, "7A 5A B9 0A 00 8D 30 CF A9 03 48 F4 2C A4 DC CF 12")
        self.assert_source(0x03A42D, "7A B5 2D 38 ED 2F CF 95 2D 10 04 A9 00 95 2D")

    def test_hit_response_mutual_exemption_next_after_callback_and_pause_gate(self):
        self.assert_source(0x03A327, "B5 25 29 10 F0 04 5C 4C A4 03")
        self.assert_source(0x03A349, "B5 24 29 08 F0 04 5C 59 A3 03 B5 20 09 02 95 20")
        self.assert_source(0x03A38C, "B5 31 29 80 C9 80 D0 04 5C 3C A4 03")
        self.assert_source(0x03A398, "B9 2E 00 8D 2F CF B9 31 00 29 01 C9 01 F0 04 5C AE A3 03 9C 2F CF")
        self.assert_source(0x03A43C, "AC 2D CF C2 20 B9 00 00 A8 E2 20 F0 03 82 ED FE")
        self.assert_source(0x03A44C, "B5 26 29 08 F0 04 5C 66 A4 03 C2 20 AD 84 1B 89 02 00 E2 20 D0 03 4C 66 A4 6B 5C 8F 2B 7F")


    def test_collision_box_rotation_selector_and_zero_axis_bypasses(self):
        self.assert_source(0x7F4133, "89 F0 D0 03 82 6B 02 89 10 F0 03 82 E2 01 89 20 F0 03 82 58 01 89 40 F0 03 82 CE 00")
        self.assert_source(0x7F415D, "BF 16 00 00 F0 04 22 F0 3B 7F")
        self.assert_source(0x7F4172, "BF 12 00 00 F0 04 22 4E 3A 7F")
        self.assert_source(0x7F4184, "BF 14 00 00 F0 04 22 A9 38 7F")
        self.assert_source(0x7F4293, "B9 03 00 18 7F 0C 00 00 85 73")
        self.assert_source(0x7F4316, "B9 05 00 18 7F 0E 00 00 85 7B")
        # Candidate-side full rotation has the same individual bypasses.
        self.assert_source(0x7F456A, "B9 16 00 F0 04 22 F0 3B 7F")
        self.assert_source(0x7F457F, "B9 12 00 F0 04 22 4E 3A 7F")
        self.assert_source(0x7F4590, "B9 14 00 F0 04 22 A9 38 7F")

    def test_collision_box_animation_mask_and_word_absolute_overlap(self):
        self.assert_source(0x7F3302, "B9 08 00 9F 44 2F 7E B9 0A 00 9F 46 2F 7E B9 0C 00 9F 48 2F 7E B9 0E 00 9F 4A 2F 7E")
        self.assert_source(0x7F4100, "B9 02 00 F0 29 3A 85 02 BF CB 1C 7E 10 06 25 02 F0 1C 80 06 A5 02 25 C4 F0 14")
        self.assert_source(0x7F411A, "C2 20 29 7F 00 85 02 98 A4 02 18 69 12 00 88 D0 FA")
        self.assert_source(0x7F489A, "BD 4A 2F 18 65 5C 8D DE 12 A5 3E 38 E5 75 10 04 49 FF FF 1A 38 ED DE 12 30 03 4C 38 49")
        self.assert_source(0x7F48B7, "BD 46 2F 18 65 58 8D DE 12 A5 3A 38 E5 73 10 04 49 FF FF 1A 38 ED DE 12 30 03 4C 38 49")
        self.assert_source(0x7F4824, "E2 20 BF 10 00 7F 05 F5 85 F5 C2 20 BF 00 00 7F AA F0 03 82 C9 FC E2 20 A5 F5 F0 57")


    def test_reflection_offsets_scatter_and_zero_muzzle_parameters(self):
        self.assert_source(0x07F1E7, "B5 14 18 69 80 85 08 B5 12 49 FF 1A 85 02 DA A6 04 A5 08 38 F5 14 85 08")
        self.assert_source(0x07F1FF, "EC C3 12 F0 05 EC C5 12 D0 0F 5A B4 2B B9 02 6C 7A 29 40 D0 04 5C 3E F2 07")
        self.assert_source(0x07F218, "22 D0 7B 7F 29 3F 18 6D 02 00 85 02 22 D0 7B 7F 29 3F 18 6D 08 00 85 08 A5 02 18 69 E0 85 02 A5 08 18 69 E0 85 08")
        self.assert_source(0x07F23E, "A5 02 8D B7 14 A5 08 8D B6 14 9C B8 14 9C B9 14 A9 00 8D B0 14 A9 00 8D B2 14 A9 00 8D B4 14 A9 02 22 9C A8 03")

    def test_homing_projectile_entry_variants_and_complete_shared_control_bytes(self):
        self.assert_source(0x09EE2D, "06 4B 54 32 32 54 34 34 54 36 36 17 3D EE 06 5A 0B 0A 2D 0B 02 2E 0B 21 0A 41 8A E7 17 74 EE")
        self.assert_source(0x09EE4C, "0B 0A 2D 7A 2E 96 2A 2E 00 68 EE 2A 2E 02 62 EE 0B 03 2E 17 6B EE 0B 04 2E 17 6B EE 0B 02 2E 0B 2D 0A 14 E0 2E 74 EE 0F")
        self.assert_source(0x09EE74, "FA 72 8D 04 00 76 08 4A 18 F0 08 00 5C 06 1E 0C A8 E3 04 5D B0 BE A1 F5 01 01 79 A1 4D 1B 8A 2A A1 00 FE EE F8 DC EE 06 3F")
        self.assert_source(0x09EE9D, "14 E8 03 AD EE AE 7F AE 7F AE 7F 00 0F 16 9D EE 00 0F 06 3F 17 BC EE")
        self.assert_source(0x09EEBB, "0F 4A D5 EE 01 4A C8 EE 0D 61 28 44 0F 4C CC EE 42 4B D5 EE 4B C8 EE 03 0F 0F A4 20 DA EE 42 09 42 00 5F E9 EE 6E A3 8A 2B A3 3C 00 EC EE 4C BB EE 42")

    def test_offset_guided_projectile_complete_graph_and_pitch_data(self):
        self.assert_source(0x09ECF7, "0b 01 2d 7a 2e 96 2a 2e 00 13 ed 2a 2e 02 0d ed 0b 04 2e 17 16 ed 0b 06 2e 17 16 ed 0b 02 2e 00 05 0c fa 73 8d 00 76 08 0c 00 c0 04 4d 00 05 04 04 00 5c 4a 18 f0 08 58 a2 07 79 a1 4d 1b 8a 2a a1 00 bd ed")
        self.assert_source(0x09ED3B, "90 04 ee 09 a2 94 58 95 07 07 95 04 14 88 13 6b ed 14 4c 1d 66 ed 14 10 27 61 ed 14 c8 32 5c ed 0f 06 2d 17 6d ed 06 29 17 6d ed 06 26 17 6d ed 06 23 58 a9 03 58 a2 ff f8 d9 ed")
        self.assert_source(0x09ED76, "fb b1 16 00 fb b3 16 00 fb b5 16 40 00 30 14 a0 0f 8c ed 16 76 ed 2c 18 00 26 95 ed 18 26 02 fb b1 16 00 fb b3 16 00 fb b5 16 40 00 30 14 dc 05 ab ed 16 95 ed 18 19 02 00 65 d0 ed 00 0d 19")
        self.assert_source(0x09EDB5, "4b d9 ed 4b 18 f0 b8 0f f8 ea ed bc c5 ed 71 fd 00 5c 06 1e 5c 03 03 5b 03 32 0f 00 21 e0 20 d7 ed 42 09 42 00 5f 00 ee 6e a3 8a 2b a3 6e 00 e9 ed 4c b5 ed 42 00 5f 00 ee 6e a3 00 53 00 ee 1a 00 00 00 ee 8a 2b a3 6e 00 03 ee 4c b5 ed 42")
        self.assert_source(0x09EE04, "01 01 01 01 02 02 03 03")

    def test_variant_guided_projectile_complete_branches_and_attached_effect(self):
        self.assert_source(0x09EF2D, "0B 01 2D 0B 06 2E 14 10 27 39 EF 0F 00 05 02 F7 F7 00 04 74 4A 14 F0 08 00 5C 06 02 04 79 A1 4D 1B 2A 27 01 66 EF 2A 27 02 5F EF 0C C0 C8 04 17 6A EF 0C 88 C8 04 17 6A EF 0C A4 C8 04")
        self.assert_source(0x09EF6A, "8A 2A 27 01 77 EF 61 0A 75 0A 73 05 44 8A 2A 27 02 84 EF 00 5C 06 2D 17 87 EF 18 14 02 2A 27 01 B3 EF 2A 27 02 A2 EF")
        self.assert_source(0x09EF91, "F5 B0 BE 06 F3 01 01 00 00 00 00 88 FF 01 17 C1 EF F5 B0 BE 06 F3 50 01 00 00 00 00 20 FE 01 17 C1 EF F5 B0 BE 06 F3 01 01 00 00 00 00 C4 FF 01 F8 D3 EF 61 78 44 4B D3 EF 14 E8 03 D0 EF 0F 6B 2D 19")
        self.assert_source(0x09EFD3, "5B 00 21 CE 32 DE EF 5F 04 F0 5C 2A 27 02 FB EF 14 F4 01 08 F0 00 0F 00 53 03 F0 2A A1 00 F8 EF 1A 00 00 03 F0 73 05 42 14 64 00 08 F0 17 E8 EF C1 4C C7 EF 42 4C 0C F0 42 4B D3 EF 03 1E 17 CA EF 4C C7 EF 42")
        self.assert_source(0x09F306, "4D 00 10 52 99 2D 0B 64 2D 4A 28 F3 0C 89 B5 25 09 02 95 25 C2 20 A9 20 F3 6B 5C 1E 01 02 16 21 F3 0F 4C 27 F3 42")
        self.assert_source(0x7FB245, "B5 20 09 02 95 20 4C E8 CA")
        # Suppression skips the ordinary death visuals, not common cleanup.
        self.assert_source(0x03A08B, "B5 25 29 02 F0 04 5C 6A A2 03")
        self.assert_source(0x03A26A, "22 A4 2A 7F 22 D6 33 7F B5 25 09 08 95 25 6B")

    def test_hostile_launch_primary_yaw_gate_and_wrapping_counters(self):
        self.assert_source(0x0DDE38, "DA AE C3 12 B5 14 FA 18 69 80 38 F9 14 00 18 69 40 C9 80 90 1A")
        self.assert_source(0x0DDE4D, "22 D0 7B 7F 29 03 85 3A A5 3A F0 0B B9 21 00 09 01 99 21 00 EE 69 1D EE 6B 1D")
        self.assert_source(0x0DDE67, "B9 31 00 09 50 99 31 00 B9 31 00 09 10 99 31 00 60")


if __name__ == "__main__":
    unittest.main()
