"""Tal Kong source contracts, checked without executing the original game."""
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class TalKongStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()

    def assert_source(self, address, expected):
        raw = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(raw)], raw)

    def test_camera_focus_copies_three_full_world_words_before_separate_camera_consumer(self):
        self.assert_source(0x7FC2B3, 'c220b50c8d011eb50e8d031eb5108d051e4ce8ca')
        self.assert_source(0x07A14A, '5adaacd614c220ad011e990c00ad031e990e00ad051e991000e2202288217fe220eb49ff1a991400853c')

    def test_label_and_asymmetric_limb_controllers(self):
        self.assert_source(0x0389C0, '54414c204b4f4e4700')
        self.assert_source(0x09A4ED, '5c0bffaf2a130304a5f5f0df81a40a04d0008c0074ff013f0aa51604a56119079404443d0183009417f1a4')

    def test_death_focus_is_republished_after_twenty_visits_then_two_bursts(self):
        self.assert_source(0x09AF2E, '4a35af0c176faf418b8ce587d742')
        self.assert_source(0x09AF6F, '003b5c4ad8af02c40314411aaf0057010010')
        self.assert_source(0x09AF1A, '003b6102ff41de8603024442')
        self.assert_source(0x09AFD8, 'c141de8642')


if __name__ == '__main__':
    unittest.main()
