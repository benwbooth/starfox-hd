"""Camera distance uses live weapon flags, shared requests and exact profiles."""
from pathlib import Path
import struct
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerViewDistanceStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_preliminary_stance_test_converges_without_gating_the_transition(self):
        self.source(0x079B2A, "2907c903b0045c349b07b9636b2920d0045c5c9b07")
        self.source(0x079B3F, "b9636b29df99636b5aa03f03b921000908992100b9210009109921007a")

    def test_toggle_updates_shared_muzzle_flags_and_publishes_actual_new_view(self):
        self.source(0x079C02, "b9636b498099636bb9636b094099636b9c42f5b9636b8980f003ee42f5")
        self.source(0x079CB7, "b9636b29bf99636bb9636b092099636b287a6b")

    def test_all_authored_profiles_and_initializer_store_order(self):
        offset = source_offset(0x079DC0)
        self.assertEqual(struct.unpack_from("<21h", self.rom, offset), (
            -210, -20, 20, 20, 0, 0, 0,
            -240, -50, 40, 0, 0, 0, 0,
            -160, 0, 0, 0, 0, 0, 0,
        ))
        self.source(0x079CFC, "c220bf00000799676b996f6bbf02000799696bbf040007996b6b")


if __name__ == "__main__":
    unittest.main()
