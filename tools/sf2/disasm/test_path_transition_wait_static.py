"""Source-only encounter transition-readiness publication and wait loop."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class TransitionWaitStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_encounter_reset_and_transition_publication_use_the_same_byte(self):
        self.assert_source(0x04B1F1, 'e2209c9ad79cd5d728ab6b')
        self.assertEqual(self.rom[0x4B926:0x4B935], bytes.fromhex('67a135b9fbd5d7010044004d3f030f'))

    def test_wait_loop_restores_scratch_on_both_yield_and_return(self):
        self.assertEqual(self.rom[0x4872C:0x4873D], bytes.fromhex('93a17aa17969a13a8795a1162c8795a142'))
        # Neither branch exit enters an IFNOT-consuming route.
        self.assert_source(0x7F8F1A, '20bcc42047cbb90000d0045ca9ca7f4cffca')


if __name__ == '__main__':
    unittest.main()
