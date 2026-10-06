"""Common death's real callers, allocation order, and complete branch contracts."""

import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class CommonDestructionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(raw)], raw)

    def test_auxiliary_override_tail_dispatch_precedes_map_counts_and_effect_gate(self):
        self.assert_source(0x03A055,
            'a90c223b237fc00000d0045c77a003c220b9626a8dcf12e220b9646a8dd112dccf12')
        self.assert_source(0x03A077,
            'b5252940d0045c8ba003ceca1aeecc1a225f9103b5252902f0045c6aa203')
        self.assert_source(0x03915F,
            '085ac220ac321bf0188a800e8ad9ce1af012bde61cd9ce1af00a888888888810eb7a286b')

    def test_secondary_then_primary_distance_cues_precede_flags_and_allocation(self):
        self.assert_source(0x03A095,
            'ada61a8902d0042207800622008006b52409049524b52609089526'
            'c220a99cbc855fe22022172a7fb0045c6aa203')
        self.assert_source(0x068000, '5a08a03f0380055a08a07e03')
        self.assert_source(0x06800C,
            'c220229f247fc220adde12c91405e22030045c528006')
        self.assert_source(0x068024,
            '229f247fc220adde12c92003e22030045c458006')
        # Both marker variants use the same unmodified primary-route words.
        self.assert_source(0x068038,
            'c220a9700022096e7fe2208018c220a9703022096e7fe220800b'
            'c220a9706022096e7fe220287a6b')
        self.assert_source(0x7F2A17,
            '863aaea8122225297fb007a00000a63a186b9ba63ae22022bc297f')

    def test_profile_uses_shape_xy_bounds_doubled_then_unsigned_thresholds(self):
        self.assert_source(0x03A0F5,
            'c2205ab404dabb9c8614bf0a0000df0c0000b004bf0c0000'
            '0ac94000b00382d500c98000b003828300c90001b003823000')
        for address, limit, shifts in [(0x03A156, '0001', 5),
                                      (0x03A1A1, '8000', 4),
                                      (0x03A1EB, '4000', 3)]:
            self.assert_source(address,
                '38e9' + limit + '8502bf07000029ff00f00546023ad0fb'
                'a502fa7a' + '4a' * shifts + 'e22099da1c')
        self.assert_source(0x03A126, 'fae220202aa67aa908990b00c220a9ecbd990400')

    def test_effect_and_companion_defaults_preserve_distinct_health_and_animation(self):
        self.assert_source(0x03A0C3,
            'b921000901992100b920000920992000a90099c81ca90099da1c'
            'b926000908992600b924000904992400b926000910992600')
        self.assert_source(0x03A230,
            'a900990a00a900098099ca1c22aa2b7fa901992d00b921000901992100'
            'c220a979a2991900e220a903991b00c220b90e0038ed8614990e00e220')
        self.assert_source(0x03A62A,
            '5ac220a99cbc855fe22022172a7fb0045ca3a603b920000910992000'
            'a907991700a91e991300a91e991500b9200029f7992000'
            'b926000908992600b924000904992400b926000910992600'
            'a902990b00b921000901992100c220a979a2991900e220a903991b00'
            '22aa2b7fc220b90e0038ed8614990e00e2207a60')

    def test_default_death_detaches_children_releases_proxy_then_marks_deferred_cleanup(self):
        self.assert_source(0x03A26A, '22a42a7f22d6337fb525090895256b')
        self.assert_source(0x7F2AA4,
            '5adae220b5232910d0045c8c2b7fb52329ef9523b429bbd00382cc00'
            'b4295ab52329fb9523c220a900009506e220b5252901d0045c852b7f829c00')
        self.assert_source(0x7F2B7B, 'b52109019521a900952d7abbf0038234fffa7a6b')

    def test_animation_primary_mode_scroll_signed_age_color_and_delayed_retirement(self):
        self.assert_source(0x03A279,
            'acc312dabb7a5ab42bb9a06a7adabb7a29f0c910f0045cb1a203'
            'c220ad1c1e0a49ffff1a18750c950cad201e0a49ffff1a1875109510e220')
        self.assert_source(0x03A2B1,
            'f60ab50add0b0030045cd8a203bdca1c1869013003186908297fc908900338e90809809dca1c6b')
        self.assert_source(0x03A2D8,
            'b5202910d0045c16a303b52009109520a9009517a9009513a9009515'
            'b52029f79520c220a90ba39519e220a903951ba940950a'
            'b50ad0045c16a303d60a6b5c1aa303b52309029523b525090895256b')


if __name__ == '__main__':
    unittest.main()
