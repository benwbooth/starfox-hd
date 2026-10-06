import hashlib
import unittest

import cpu65816 as cpu
from extract_scene_loaders import (
    DEFAULT_ROM, PACKET_FIXTURE, SOURCE_GATES, TABLE, TABLE_END,
    artwork_packet, extract_scene_loaders, footprint_digest, footprint_spans,
    load_sequence, render_packet_fixture, rom_bytes,
)


class SceneLoaderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = DEFAULT_ROM.read_bytes()
        cls.result = extract_scene_loaders(cls.rom)

    def test_authored_requests_and_complete_graph_are_retained(self):
        self.assertEqual(len(self.result.requests), 19)
        self.assertEqual(sum(len(load.requested_by) for load in self.result.requests), 31)
        self.assertEqual(len({root for load in self.result.requests for root in load.routines}), 17)
        self.assertEqual(len(self.result.nodes), 840)
        packets = [node.packet for node in self.result.nodes if node.packet]
        self.assertEqual(len(packets), 42)
        self.assertEqual(len({packet.compressed_end for packet in packets}), 32)
        for node in self.result.nodes:
            self.assertTrue(node.successors or node.mnemonic == "RTL")
        self.assertEqual(PACKET_FIXTURE.read_text(), render_packet_fixture(self.result))

    def test_bank_first_table_and_null_entries_remain_distinct(self):
        self.assertEqual(load_sequence(self.rom, 0), ())
        self.assertEqual(load_sequence(self.rom, 3), (0x03C7B3,))
        self.assertEqual(load_sequence(self.rom, 0x15), (0x03CB1C,))
        self.assertEqual(load_sequence(self.rom, 0x8D), (0x03C80B,))
        changed = bytearray(self.rom)
        at = cpu.cpu_to_file(TABLE + 6)
        changed[at:at + 3] = bytes.fromhex("031ccb")
        self.assertEqual(load_sequence(changed, 3), (0x03C7B3, 0x03CB1C, 0x03D31A))
        for offset in [-3, 1, 4, TABLE_END - TABLE]:
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                load_sequence(self.rom, offset)

    def test_inline_data_is_never_decoded_as_instructions(self):
        by_address = {node.address: node for node in self.result.nodes}
        for node in self.result.nodes:
            if not node.packet:
                continue
            self.assertEqual(node.successors, (node.address + 10,))
            for byte in range(node.address + 3, node.address + 10):
                self.assertNotIn(byte, by_address)
        intro = [node.packet for node in self.result.reachable(0x03CB1C) if node.packet]
        self.assertEqual([p.compressed_end for p in intro],
                         [0x1990E4, 0x19B678, 0x199238, 0x19B774])
        self.assertEqual([p.transfer_bytes for p in intro], [2048, 2048, 4096, 4096])
        self.assertEqual([p.decoded_bytes for p in intro], [2048, 2048, 4096, 8192])
        self.assertTrue(all(p.destination is None for p in intro))

    def test_all_conditional_artwork_and_shared_tails_are_reachable(self):
        nodes = self.result.reachable(0x03D0CE)
        packets = [node.packet.compressed_end for node in nodes if node.packet]
        self.assertEqual(packets, [0x16BFB8, 0x16D230, 0x15C9BC,
                                  0x16C930, 0x16BFB8, 0x16C4E4])
        self.assertTrue(any(node.address == 0x03D52C for node in nodes))
        startup = {node.address for node in self.result.reachable(0x03C80B)}
        self.assertTrue({0x03C85F, 0x03C879, 0x03C893} <= startup)

    def test_width_changes_and_polling_loops_are_not_flattened(self):
        nodes = {node.address: node for node in self.result.nodes}
        self.assertEqual(nodes[0x03CB9A].accumulator_bytes, 2)
        self.assertEqual(nodes[0x03CB9A].operand, 0x8F4C)
        self.assertEqual(nodes[0x03CB5B].successors, (0x03CB5D, 0x03CB59))
        self.assertEqual(nodes[0x03D4B1].successors, (0x03D4B3, 0x03D4D8))
        self.assertEqual(nodes[0x03D4D7].successors, ())
        self.assertEqual(nodes[0x03D4FC].successors, ())

    def test_full_footprint_digest_covers_original_code_and_payloads(self):
        original = b"".join(rom_bytes(self.rom, address, length)
                            for address, length in footprint_spans(self.result))
        self.assertEqual(hashlib.sha256(original).hexdigest(), footprint_digest(self.result))

    def test_helper_changes_and_unreviewed_calls_fail_closed(self):
        for start, _, _ in SOURCE_GATES:
            changed = bytearray(self.rom)
            changed[start] ^= 1
            with self.subTest(start=start), self.assertRaisesRegex(ValueError, "signature mismatch"):
                extract_scene_loaders(changed)
        changed = bytearray(self.rom)
        at = cpu.cpu_to_file(0x03CB30)
        changed[at + 1:at + 3] = bytes.fromhex("71d5")
        with self.assertRaisesRegex(ValueError, "unreviewed loader call"):
            extract_scene_loaders(changed)

    def test_bad_table_targets_and_packet_sizes_fail_closed(self):
        changed = bytearray(self.rom)
        at = cpu.cpu_to_file(TABLE + 3)
        changed[at] = 0x7E
        with self.assertRaisesRegex(ValueError, "outside reviewed code"):
            extract_scene_loaders(changed)
        changed = bytearray(self.rom)
        at = cpu.cpu_to_file(0x03CB36)
        changed[at:at + 3] = bytes.fromhex("00007e")
        with self.assertRaisesRegex(ValueError, "not mapped ROM"):
            extract_scene_loaders(changed)
        changed = bytearray(self.rom)
        at = cpu.cpu_to_file(0x03CB36)
        changed[at + 5:at + 7] = bytes.fromhex("ffff")
        with self.assertRaisesRegex(ValueError, "exceeds decoded stream"):
            extract_scene_loaders(changed)
        with self.assertRaisesRegex(ValueError, "truncated ROM record"):
            artwork_packet(self.rom[:0x1CB3C], 0x03CB33, 0x03D674)

    def test_branches_into_packet_bytes_fail_closed(self):
        changed = bytearray(self.rom)
        at = cpu.cpu_to_file(0x03CB5B)
        changed[at + 1] = (0x03CB36 - 0x03CB5D) & 255
        with self.assertRaisesRegex(ValueError, "overlapping instruction or inline packet"):
            extract_scene_loaders(changed)


if __name__ == "__main__":
    unittest.main()
