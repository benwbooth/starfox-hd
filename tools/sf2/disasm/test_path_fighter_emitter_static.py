"""Fighter controller source contracts, using static bytes only."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class FighterEmitterStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(expected)], expected)

    def test_emitter_limit_is_second_minus_first_sign_not_unsigned_less_or_equal(self):
        self.assert_source(0x08CC68, '6d27fb9ad70048f6860b0494780d44e720744cfd46007aa12feea194b44c2a2701984c5d6ccff74c0104e58bd717a74c1410278b4c5d98cbcd4c6404e58bd70319672eb94c622e7844167e4c035017a74c030a297e4c16b94c')
        self.assert_source(0x7FB8E4, '20bcc42047cbb900008db71620e0c42047cbb90000cdb7161003820a124c94ca')

    def test_position_helpers_transfer_all_three_words_and_missing_link_still_restores(self):
        self.assert_source(0x08CCC1, '0a99c94c4154809b415e8042')
        self.assert_source(0x098054, '800c0b800e0d80100f427b0c0b7b0e0d7b100f42')
        self.assert_source(0x7F9F75, '20e79fc220adb7169900004cbeca')
        self.assert_source(0x7F9FAF, '20bf9f29ff00a8adb716995cd74cbeca')

    def test_rolling_expiry_decrements_but_phase_abort_does_not(self):
        self.assert_source(0x08CCCD, 'f686005c006706322efd0f0058a10f07a1f80364e78bd7107aa23e67a2ef4c4cf34c5216a1424be54c0f')

    def test_guided_contact_death_and_phase_abort_have_different_side_effects(self):
        self.assert_source(0x08CD6C, '7aa13e67a1774d5c4c784d424b6c4d031e0f')
        self.assert_source(0x0989C6, '4cca89424bc68941878c9d3200e78bd741928610')
        self.assert_source(0x098692, '61045d24bef98364329c07a906c49b4442')


if __name__ == '__main__':
    unittest.main()
