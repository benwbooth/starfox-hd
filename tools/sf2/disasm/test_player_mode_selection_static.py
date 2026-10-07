"""Mode selection: gated new requests, independent pending work and byte ownership."""
from pathlib import Path
import unittest

from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerModeSelectionStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        offset = source_offset(address)
        self.assertEqual(self.rom[offset:offset + len(data)], data)

    def test_new_request_gate_jumps_to_pending_work_and_preserves_high_flags(self):
        self.source(0x0698D5, "b99b6b8940f0045c369906")
        self.source(0x06991C, "ad37198920f013b9ec6b2918d00cb9a16a29f0050299a16a8000")

    def test_original_mode_table_and_shared_phase_dispatch(self):
        self.source(0x069A10, "0000000010000500100007003000030020000000")
        self.source(0x7F3282, "8cca16bdc71cc22029ff000a0aa8")

    def test_transition_and_request_writes_preserve_adjacent_bytes(self):
        self.source(0x069962, "b9ec6b890800d03c891000f00929efff09080099ec6b")
        self.source(0x069981, "b9a16a29f0ff99a16aa5088502e220a5029dc71c287a186b")
        self.source(0x069999, "8502e220a5029dc71c287a386b")


if __name__ == "__main__":
    unittest.main()
