"""Completion counter width/reset and packed argument decoding from source."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class CompletionTotalsStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def test_increment_handler_updates_a_full_word_and_scene_entry_clears_both_totals(self):
        for address, source in [
                (0x7FB817, 'c2202020c7a8b900001a9900004cbeca'),
                (0x04DF8D, 'a200008e16d88e18d88e5bda8ee4d78ee6d7')]:
            start = source_offset(address)
            expected = bytes.fromhex(source)
            self.assertEqual(self.rom[start:start+len(expected)], expected)

    def test_completion_variants_order_bit_record_signals_totals_and_remaining_count(self):
        for address, source in [
                (0x80B1, 'e6e4d741e587e7f4d742'),
                (0x80BB, 'fb9ad7ffff41e587003be6e6d7e7f4d742'),
                (0x8839, '93a10b05a14e272e6104d9a12e6da1005427456d2795a142')]:
            start = 0x40000 + address
            expected = bytes.fromhex(source)
            self.assertEqual(self.rom[start:start+len(expected)], expected)

    def test_variable_bit_selector_is_one_based_and_clear_preserves_the_companion_byte(self):
        for address, source in [
                (0x7FB5FB, '20bcc42047cbb900003a0ac22029ff008eb116aabfcfb57faeb11660'),
                (0x7FB643, '20efb549ffff3900009900004cbeca'),
                (0x7FB5D7, '1000200040008000')]:
            start = source_offset(address)
            expected = bytes.fromhex(source)
            self.assertEqual(self.rom[start:start+len(expected)], expected)


if __name__ == '__main__':
    unittest.main()
