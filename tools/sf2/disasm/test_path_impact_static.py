"""Projectile impact contracts from static source bytes; no CPU execution."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PathImpactStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_pair_precedence_ground_wrapping_surface_group_restore_and_signed_height_compare(self):
        self.assert_source(0x0DDE78,
            'da5ab5202980f0045c1adf0db5212902f0045c1adf0dad4d1b2907c900f010'
            'c220b50e18691400e22030045c3adf0dbdea1c48223aaf0d689dea1cbce81c'
            'f057c220b50ec508e22010045c0fdf0db5312980c980f0045cfade0d')
        self.assert_source(0x0DDED2,
            'b5312908c908f0045cfade0db5242980f0045cf2de0db9260009029926008008'
            'b926000904992600b926002901d0045c1adf0db92200090299220080047afa186b'
            'bce81c84088029b41ec220b904008508e2205aa4089c46d7b922002908f005a901'
            '8d46d77a8009c220a900008508e2207afa386b')

    def test_classifier_zero_health_players_and_distinct_optional_material_records(self):
        self.assert_source(0x0DDF47,
            '2278de0db0046402186bda5a0864036402c220a508e220d0045cd2df0da408'
            'ccc312d0045cd4df0dccc512d0045cd4df0db92d00d0045cd4df0db922002908'
            'f0045cafdf0d5adabb7aa90b223b237fc00000d007dabb7a5caadf0db9626a'
            'dabb7a85027aa90180255adabb7aa90d223b237fc00000d007dabb7a5ccddf0d'
            'b9626adabb7a85027aa9028002a900287afa386b')
        self.assert_source(0x7F233B,
            'e2208d2acfbcec1cf01ab9616a8d29cfc8b9616acd2acff00bc8c8c8ce29cfd0efa000006b')
        self.assert_source(0x7FC384,
            '20bcc448a90d2260237f6899626a4cd3ca20bcc448a90b2260237f6899626a4cd3ca')

    def test_all_source_pool_slots_explain_typed_four_slot_player_branch_pattern(self):
        self.assert_source(0x0385AE, '08c2309ca812a9bd038daa12a03c00aa18693f00950088d0f67400286b')
        self.assert_source(0x7FBDF8, 'a55e29e7855e8f3a30002247df0d900e3a3008f0034c23cb4c0bcb4cf3ca4c72ca')
        for slot in range(60):
            returned = (0x03BD + 0x003F * slot) & 255
            decremented = (returned - 1) & 255
            branch = 0 if decremented & 128 else 1 if decremented == 0 else 2
            self.assertEqual(branch, 0 if slot % 4 < 2 else 2)

    def test_target_lock_publication_copies_retained_target_and_cancellation_clears_it(self):
        self.assert_source(0x07A5E3, 'c220b9ca6b8d901de220c220b9ca6b99c86be220d0699c901d9c911d')
        self.assert_source(0x07A653, 'c220b9ca6b8d901de220a90a99c76b')
        self.assert_source(0x07A550, 'c220a900008d901de220')
        self.assert_source(0x07A56D, 'c220a9000099c86b9c901de220')

    def test_strategy_handoff_clears_path_and_render_parameter_then_enters_real_movement(self):
        self.assert_source(0x7F99F6, 'c2202020c79519e2202004c5951ba9009dc71cc220a90000952be2204cde9d')
        self.assert_source(0x09AFE4,
            'b52009109520a9079517a9059513a91e9515b52029f79520b52629fe9526b526'
            '09089526b52409049524b52609109526c220a925b09519e220a909951ba901950a'
            'b50a3a950a101cc9f6f019b52009109520a9009517a9009513a9009515b52029f7'
            '95206bb525090895256b')
        # Flag 20:10 is the transient actor's allocation-pressure retirement
        # class, not scaled-sprite rendering (which has separate controls).
        self.assert_source(0x7F2983, 'b5202910f0045cac297f')


if __name__ == '__main__':
    unittest.main()
