"""Complete player-side node-exit birth and objective-clear source boundary."""
from pathlib import Path
import unittest
from dump_runtime_routine import source_offset
from extract_map import DEFAULT_ROM


class PlayerNodeExitStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = Path(DEFAULT_ROM).read_bytes()

    def source(self, address, expected):
        data = bytes.fromhex(expected)
        start = source_offset(address)
        self.assertEqual(self.rom[start:start + len(data)], data)

    def test_nonzero_objective_low_byte_and_requested_uncreated_presentation(self):
        self.source(0x06A045, 'adf4d7f051ad081e8980f04a8940d046')

    def test_birth_has_no_pose_copy_attachment_or_immediate_path_dispatch(self):
        self.source(0x06A055, 'c220a99cbc855fe22022172a7fb0045c9ba006c220a91e7e991900e220a97f991b00a901992d00a901992e00b921000901992100c220a9c5b8992b00e220ad081e09408d081e')

    def test_exact_completion_code_clears_only_objective_low_byte_after_birth(self):
        self.source(0x06A09B, 'ad171ec901d0039cf4d7')

    def test_constructor_uses_active_head_not_current_player_and_shared_reset_is_ordered(self):
        self.source(0x7F2A17, '863aaea8122225297fb007a00000a63a186b9ba63ae22022bc297fc220a55f990400e220386b')
        self.source(0x0695F1, '9c081e9c4d1b')
        self.source(0x06960E, '9c741d9c7a1da000008c7b1d9c161e9c171e')


if __name__ == '__main__':
    unittest.main()
