"""Regression checks for continuing, signed clipping-plane shape commands."""

import struct
import unittest

from extract_shapes import (
    ClipPlane, MVAL_CLIP_PLANE, ShapeParseError, Vertex, extract_shapes, parse_faces,
)
from rom import load_rom


FACE_PROGRAM_ADDRESS = 0x008000
PLANE_DIRECTION = 4095


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


if __name__ == "__main__":
    unittest.main()
