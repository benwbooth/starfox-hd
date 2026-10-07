"""Source byte contracts for the complete player engine-sound composer."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerEngineSoundStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        expected = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(expected)], expected)

    def test_gate_and_shared_sound_publication_are_not_actor_indexed(self):
        self.source(0x06923F, "c220b9136cc9c9c1e220d0045c639206")
        self.source(0x06924F, "adf4d7f00fade51c8dae1d206792adae1d8de51c")

    def test_flight_uses_original_pilot_speed_and_signed_motion_comparisons(self):
        self.source(0x06DBC0, "282a201e3032")
        self.source(0x069320, "460246024602c220a508c502e2201004")
        self.source(0x069364, "b9dd6a100349ff1a7ac90a3004")

    def test_walker_final_selection_replaces_the_temporary_masked_value(self):
        self.source(0x06942F, "b9ea6a7a8980d007a9008dae1d8005a92c8dae1d")
        self.source(0x069443, "b9e96a8908d0045c569406adae1d09348dae1d60")

    def test_surface_checks_live_carry_and_not_flight_protection(self):
        self.source(0x0693E1, "b42badae1d29808dae1dad131ec901f0045c569406b5212920d0045c569406adae1d09408dae1d804c")


if __name__ == "__main__":
    unittest.main()
