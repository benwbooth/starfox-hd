"""Caller ordering for free-flight input/camera preparation, before movement."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerFlightPrepareStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_contact_event_input_and_shoulders_precede_camera_selection(self):
        self.source(0x06871A, '5ab42bb9726a29ef99726a7ab523090895232257940622759006')

    def test_only_normal_camera_bypasses_ground_pitch_before_movement(self):
        self.source(0x068734, 'b99c6a8502c220b99a6a8504e220a502c907f0045c5b8706c220a504c94880e220d0045c5f870622f18107222ce306')


if __name__ == '__main__':
    unittest.main()
