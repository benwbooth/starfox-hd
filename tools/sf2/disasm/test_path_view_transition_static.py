"""Fixed-view base copy and projectile invalidation are source-derived."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM
from extract_path import PathExtractor


class ViewTransitionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_cleanup_traverses_all_active_actors_and_changes_only_three_fields(self):
        self.assert_source(0x03A6A5,
            '085adae220c210aea812f02db400b5312950c950d0045ccba603'
            'b5312908c908f0045cdba603b52609089526b52109019521a900952d'
            'bbd0d3fa7a286b')

    def test_chase_targets_fixed_view_with_double_depth_then_distance_and_word_angles(self):
        self.assertEqual(PathExtractor(self.rom).handler_entry(0x110).handler_address, 0x7FB376)
        self.assert_source(0x7FB376,
            'a03f03c220b90c00853ab50c22a3257f990c00e220c220b90e00853ab50e22a3257f990e00e220c2'
            '20b91000853ab51022a3257f991000e220c220b91000853ab51022a3257f991000e220c220b92900'
            '853aa9000022a3257f992900e220c220b51229ff00eb49ffff1a8db116b51429ff00eb49ffff1a8d'
            'b316b51629ff00eb49ffff1a8db516e220c220b91200853aadb11622a3257f991200e220c220b914'
            '00853aadb31622a3257f991400e220c220b91600853aadb51622a3257f991600e2204ce8ca')

    def test_selected_position_reads_auxiliary_record_not_current_actor_coordinates(self):
        self.assertEqual(PathExtractor(self.rom).handler_entry(0x14E).handler_address, 0x7FC107)
        self.assert_source(0x7FC107,
            'ac1fcfdab62b9bfac220b9c16a950cb9c36a950eb9c56a9510e2204ce8ca')

    def test_snap_computes_all_negated_coarse_angle_targets_before_any_word_angle_store(self):
        self.assertEqual(PathExtractor(self.rom).handler_entry(0x111).handler_address, 0x7FB43B)
        self.assert_source(0x7FB43B,
            'a03f03c220b50c990c00e220c220b50e990e00e220c220b510991000e220c220b510991000e220c2'
            '20a90000992900e220c220b51229ff00eb49ffff1a8db116b51429ff00eb49ffff1a8db316b51629'
            'ff00eb49ffff1a8db516e220c220adb116991200e220c220adb316991400e220c220adb516991600'
            'e2204ce8ca')

    def test_save_runs_cleanup_before_copying_exactly_sixty_three_base_bytes(self):
        extractor = PathExtractor(self.rom)
        self.assertEqual(extractor.handler_entry(0xC2).handler_address, 0x7FB295)
        self.assert_source(0x7FB295,
            'c220a902000c841be220b52609089526a55e29e7855e8f3a3000'
            '22a5a603c220a93f00224e197fa8e220dac220981869616aa8e220'
            'a23f0320f7b2fa')
        self.assert_source(0x7FB2F7,
            'da5aa93f8db116bd0000990000e8c8ceb116d0f37afa60')

    def test_restore_uses_same_fixed_base_and_frees_payload_but_not_auxiliary_entry(self):
        self.assertEqual(PathExtractor(self.rom).handler_entry(0xC3).handler_address, 0x7FB320)
        self.assert_source(0x7FB320,
            'c220a902001c841be220b52629f79526a908223b237fc00000d0045c6eb37f'
            'c220b9626aa8e220c220981869616aa8e220dabba03f0320f7b29b'
            'c2209838e9616aa8e220fac22098226b197fe220a9f720b8a34ce8ca')

    def test_auxiliary_lookup_walks_four_byte_entries_and_grows_before_publishing(self):
        self.assert_source(0x7F233B,
            'e2208d2acfbcec1cf01ab9616a8d29cfc8b9616acd2acff00b'
            'c8c8c8ce29cfd0efa000006b')
        self.assert_source(0x7F2360,
            'e2208d2acfbcec1cf031b9616a8d29cfc8b9616acd2acff058'
            'c8c8c8ce29cfd0efbcec1cb9616a1ac22029ff000a0a1ac220'
            '22001b7fa8e2208011c220a90500224e197fa8e220a90099616a'
            'c220989dec1ce220b9616a1a99616a3ac22029ff000a0a187dec1c'
            '1aa8e220ad2acf99616a6b')
        self.assert_source(0x7FB2D1,
            'c2209838e9616aa8e220c2209848e220a9082260237fc2206899626a'
            'e220a9f820b8a34ce8ca')

    def test_paired_cues_always_route_via_primary_pointer_not_selected_marker(self):
        self.assert_source(0x7FA3B8, '5a8d311c9c321cacc3122039a47a60')
        self.assert_source(0x7FA439, 'c220ad311cdaae161dccc312f008c03f03f0030900809df61c')
        self.assert_source(0x7FCAE8, 'e220c220f62be2204c757e')


if __name__ == '__main__':
    unittest.main()
