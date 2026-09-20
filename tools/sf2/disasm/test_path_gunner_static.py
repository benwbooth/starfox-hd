"""Gunner source evidence; no original-program execution or recordings."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class GunnerStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        expected = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start+len(expected)], expected)

    def test_primary_feedback_service_gates_full_target_mode_and_live_player_state(self):
        self.assert_source(0x7FC19C, 'a55e29e7855e8f3a3000224bb6074ce8ca')
        self.assert_source(0x07B64B, 'da5a08e220c210aec312b42bc220b91c6cc90800e220d016b9006cd0045c79b607a90499116cb9126c092499126c287afa6b')

    def test_display_words_are_text_references_not_callbacks_and_health_clamp_is_later(self):
        self.assert_source(0x038999, '4b49434b2047554e4e455200')
        self.assert_source(0x02FAB3, 'af77d77e8f642870af79d77e8f662870')
        self.assert_source(0x02FA92, 'af73d77e8980f006a9008f73d77e0a29ffcfac01709004afac01708fae0170')
        self.assert_source(0x09806F, '932d072dd88e2d7f2d197f2d17952dfd040842932d072dd88a2a2d018f806d2d8e2d7f2d17952d42')

    def test_route_tables_are_four_points_and_two_connections_each(self):
        self.assert_source(0x06FD35, '010300020103000200408040c080c000')
        self.assert_source(0x06FE35, '00fb0005000500fb00fba0e020600000000000fc00fc00fc0000000000fc')

    def test_gunner_jump_and_bounce_arcs_keep_signed_bytes_and_exact_loop_sizes(self):
        self.assert_source(0x00B307, '9cafc0cfdce7f0f7fcff01040910192431405164')
        self.assert_source(0x06FD0B, '080c04fcec')
        self.assert_source(0x08B5F8, '6ba1610500030bfd06a10e051c010c447842')
        self.assert_source(0x08B51B, '0a9024fd06a18a000307b300a10e146fa1000307b300a10e1444060078')


if __name__ == '__main__':
    unittest.main()
