"""Regression checks for continuing, signed clipping-plane shape commands."""

import struct
import unittest

from extract_shapes import (
    ClipPlane, MVAL_CLIP_PLANE, ShapeParseError, Vertex, extract_shapes, parse_faces,
    parse_face_program,
    parse_point_stream,
)
from rom import load_rom


FACE_PROGRAM_ADDRESS = 0x008000
PLANE_DIRECTION = 4095


class PointBlockExtraction(unittest.TestCase):
    def test_changed_point_dispatch_is_rejected(self):
        data = bytearray(load_rom())
        data[0x8005] ^= 1
        with self.assertRaisesRegex(ShapeParseError, "point dispatch signature"):
            extract_shapes(data)

    def test_mixed_formats_preserve_mirror_boundaries(self):
        stream = bytes([4, 1, 1, 2, 3, 0x34, 1]) + struct.pack("<hhh", -32768, 9, -7) + bytes([0x0C])
        vertices, counts, blocks = parse_point_stream(stream, 0x8000, 0)
        self.assertEqual(vertices, (Vertex(1, 2, 3), Vertex(-32768, 9, -7), Vertex(-32768, 9, -7)))
        self.assertEqual(counts, ())
        self.assertEqual([(b.source_address, b.words, b.mirrored, b.first_vertex, b.count) for b in blocks],
                         [(0x8000, False, False, 0, 1), (0x8005, True, True, 1, 1)])

    def test_truncated_and_zero_blocks_fail(self):
        for opcode, stride in [(4, 3), (8, 6), (0x34, 6), (0x38, 3)]:
            with self.assertRaisesRegex(ShapeParseError, "zero-count"):
                parse_point_stream(bytes([opcode, 0, 0x0C]), 0x8000, 0)
            for length in range(1, 2 + stride):
                with self.subTest(opcode=opcode, length=length):
                    with self.assertRaisesRegex(ShapeParseError, "truncated point"):
                        parse_point_stream((bytes([opcode, 1]) + bytes(stride))[:length], 0x8000, 0)

    def test_retail_blocks_cover_every_decoded_frame_without_gaps(self):
        for shape in extract_shapes(load_rom()):
            frames = shape.animation_frames or (shape.vertices,)
            self.assertEqual(len(frames), len(shape.point_frames))
            for vertices, blocks in zip(frames, shape.point_frames):
                cursor = 0
                for block in blocks:
                    self.assertEqual(block.first_vertex, cursor)
                    cursor += block.count * (2 if block.mirrored else 1)
                self.assertEqual(cursor, len(vertices))


def plane_record(slot, origin=(0, 0, 0), direction=(0, -PLANE_DIRECTION, 0)):
    return struct.pack("<BBhhhhhh", MVAL_CLIP_PLANE, slot, *origin, *direction)


class ClippingPlaneExtraction(unittest.TestCase):
    def test_both_opposing_planes_survive(self):
        stream = plane_record(4) + plane_record(5, direction=(0, PLANE_DIRECTION, 0)) + b"\x00"
        faces, planes = parse_faces(stream, FACE_PROGRAM_ADDRESS)
        self.assertEqual(faces, ())
        self.assertEqual(planes, (
            ClipPlane(4, Vertex(0, 0, 0), Vertex(0, -PLANE_DIRECTION, 0)),
            ClipPlane(5, Vertex(0, 0, 0), Vertex(0, PLANE_DIRECTION, 0)),
        ))

    def test_two_points_remain_signed_and_distinct(self):
        origin = (-32768, 32767, -11)
        direction = (32767, -32768, 29)
        _, planes = parse_faces(plane_record(8, origin, direction) + b"\x00", FACE_PROGRAM_ADDRESS)
        self.assertEqual(planes, (ClipPlane(8, Vertex(*origin), Vertex(*direction)),))

    def test_unknown_command_after_plane_fails(self):
        with self.assertRaisesRegex(ShapeParseError, "unknown face opcode"):
            parse_faces(plane_record(4) + b"\x69", FACE_PROGRAM_ADDRESS)

    def test_regular_faces_after_plane_are_not_lost(self):
        # A two-sided line, followed by the end-of-shape face terminator.
        line = bytes((0x14, 2, 0xFF, 7, 0, 0, 0, 0, 1, 0xFF))
        faces, planes = parse_faces(plane_record(4) + line, FACE_PROGRAM_ADDRESS)
        self.assertEqual(len(planes), 1)
        self.assertEqual(len(faces), 1)
        self.assertEqual(faces[0].vertex_indices, (0, 1))
        self.assertEqual(faces[0].color_index, 7)

    def test_every_truncation_fails(self):
        record = plane_record(4)
        for length in range(1, len(record)):
            with self.subTest(length=length):
                with self.assertRaisesRegex(ShapeParseError, "truncated clipping plane"):
                    parse_faces(record[:length], FACE_PROGRAM_ADDRESS)

    def test_missing_terminator_fails(self):
        with self.assertRaisesRegex(ShapeParseError, "unterminated face program"):
            parse_faces(plane_record(4), FACE_PROGRAM_ADDRESS)

    def test_invalid_slots_fail(self):
        for slot in (0, 9, 255):
            with self.subTest(slot=slot):
                with self.assertRaisesRegex(ShapeParseError, "invalid clipping-plane slot"):
                    parse_faces(plane_record(slot) + b"\x00", FACE_PROGRAM_ADDRESS)


class RetailClippingPlanes(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = load_rom()

    def test_complete_catalog_keeps_both_pairs_and_all_geometry(self):
        shapes = extract_shapes(self.rom)
        clipped = [shape for shape in shapes if shape.clipping_planes]
        self.assertEqual([shape.header.index for shape in clipped], [48, 49])
        for shape, slots in zip(clipped, ((4, 5), (6, 7))):
            self.assertEqual(shape.faces, ())
            self.assertEqual(shape.clipping_planes, tuple(
                ClipPlane(slot, Vertex(0, 0, 0), Vertex(0, sign * PLANE_DIRECTION, 0))
                for slot, sign in zip(slots, (-1, 1))
            ))
        self.assertEqual(len(shapes), 577)
        self.assertEqual(sum(len(shape.vertices) for shape in shapes), 11860)
        self.assertEqual(sum(len(shape.faces) for shape in shapes), 10524)
        self.assertEqual(sum(len(shape.animation_frames) for shape in shapes), 1342)

    def test_dispatch_change_is_not_silently_accepted(self):
        changed = bytearray(self.rom)
        changed[0x8069] ^= 1
        with self.assertRaisesRegex(ShapeParseError, "dispatch signature mismatch"):
            extract_shapes(changed)


def line_record(color):
    return bytes((0x14, 2, 0xFF, color, 0, 0, 0, 0, 1, 0xFF))


class FaceProgramTopology(unittest.TestCase):
    def bsp_program(self, right_offset):
        data = bytearray(256)
        data[:6] = bytes((0x30, 1, 0, 1, 2, 0x3C))
        data[6:11] = bytes((0x28, 0, 7, 0, right_offset))
        data[11:14] = bytes((0x44, 13, 0))
        data[16:26] = line_record(1)
        data[26:36] = line_record(2)
        if right_offset:
            right = 10 + right_offset
            data[right:right + 3] = bytes((0x44, 8, 0))
            data[right + 10:right + 20] = line_record(3)
        return bytes(data)

    def test_bsp_null_child_does_not_create_a_phantom_command(self):
        faces, _, nodes = parse_face_program(self.bsp_program(0), FACE_PROGRAM_ADDRESS)
        by_address = {node.address: node for node in nodes}
        self.assertEqual(by_address[6].targets, (16, 11, None))
        self.assertEqual(by_address[6].arguments, (0,))
        self.assertEqual(by_address[11].operation, "BspLeaf")
        self.assertEqual(by_address[11].targets, (26,))
        self.assertNotIn(10, by_address)
        self.assertEqual({face.color_index for face in faces}, {1, 2})

    def test_bsp_right_offset_is_unsigned(self):
        faces, _, nodes = parse_face_program(self.bsp_program(128), FACE_PROGRAM_ADDRESS)
        by_address = {node.address: node for node in nodes}
        self.assertEqual(by_address[6].targets, (16, 11, 138))
        self.assertEqual(by_address[138].targets, (148,))
        self.assertEqual({face.color_index for face in faces}, {1, 2, 3})

    def test_bsp_requires_the_active_visibility_table(self):
        data = bytearray(self.bsp_program(0))
        data[7] = 1
        with self.assertRaisesRegex(ShapeParseError, "BSP visibility 1"):
            parse_face_program(data, FACE_PROGRAM_ADDRESS)

    def test_group_depth_points_precede_the_pointer_table(self):
        data = bytearray(64)
        data[:8] = bytes((0x10, 2, 5, 7, 0x10, 0x80, 0x20, 0x80))
        data[16:26] = line_record(1)
        data[32:42] = line_record(2)
        faces, _, nodes = parse_face_program(data, FACE_PROGRAM_ADDRESS)
        self.assertEqual(nodes[0].arguments, (5, 7))
        self.assertEqual(nodes[0].targets, (16, 32))
        self.assertEqual({face.color_index for face in faces}, {1, 2})

    def test_continuing_and_quitting_face_lists_stay_distinct(self):
        stream = line_record(1)[:-1] + b"\xFE" + line_record(2)
        faces, _, nodes = parse_face_program(stream, FACE_PROGRAM_ADDRESS)
        self.assertEqual(len(faces), 2)
        self.assertEqual(nodes[0].arguments, (0, 1))
        self.assertEqual(nodes[0].targets, (10,))
        self.assertEqual(nodes[1].arguments, (1, 1))
        self.assertEqual(nodes[1].targets, (None,))

    def test_truncated_command_records_fail_cleanly(self):
        for record in (bytes((0x30, 1, 0, 1, 2)),
                       bytes((0x10, 2, 5, 7, 0x10, 0x80, 0x20, 0x80)),
                       bytes((0x50, 1, 2, 3)), bytes((0x54, 1, 2, 3, 4)),
                       bytes((0x44, 0, 0)), line_record(1)):
            for length in range(1, len(record)):
                with self.subTest(record=record, length=length):
                    with self.assertRaises(ShapeParseError):
                        parse_face_program(record[:length], FACE_PROGRAM_ADDRESS)

    def test_retail_graph_covers_each_face_and_resolves_every_edge(self):
        shapes = extract_shapes(load_rom())
        self.assertEqual(sum(len(shape.face_program) for shape in shapes), 4037)
        for shape in shapes:
            nodes = shape.face_program
            addresses = {node.address for node in nodes}
            self.assertEqual(len(addresses), len(nodes))
            covered = []
            for node in nodes:
                for edge in node.targets:
                    if edge is not None:
                        self.assertIn(edge, addresses)
                if node.operation == "Faces":
                    first, count = node.arguments
                    covered.extend(range(first, first + count))
            self.assertEqual(sorted(covered), list(range(len(shape.faces))))


if __name__ == "__main__":
    unittest.main()
