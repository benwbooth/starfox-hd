#!/usr/bin/env python3
"""
Star Fox Shape (3D Model) Parser

Parses the Star Fox SHAPES format from extracted ROM data.
Star Fox shapes consist of:
- Vertex list (3D points in fixed-point)
- Face list (polygon indices + color/texture info)
- LOD chain (multiple detail levels)
- Collision boxes (AABB per shape)
- Animation data (for animated shapes)

Usage: python3 shape_tool.py data/shapes.bin [--dump] [--obj shape_id]
"""

import sys
import os
import struct
import json


class ShapeVertex:
    def __init__(self, x, y, z):
        self.x = x
        self.y = y
        self.z = z

    def to_dict(self):
        return {"x": self.x, "y": self.y, "z": self.z}

    def to_obj(self, scale=1.0):
        return f"v {self.x * scale} {self.y * scale} {self.z * scale}"


class ShapeFace:
    def __init__(self, indices, color_index, normal=None):
        self.indices = indices
        self.color_index = color_index
        self.normal = normal

    def to_dict(self):
        d = {"indices": self.indices, "color": self.color_index}
        if self.normal:
            d["normal"] = self.normal
        return d

    def to_obj(self):
        # OBJ format uses 1-based indices
        idx_str = " ".join(str(i + 1) for i in self.indices)
        return f"f {idx_str}"


class Shape:
    def __init__(self, shape_id):
        self.id = shape_id
        self.vertices = []
        self.faces = []
        self.collision_box = None  # (min_x, min_y, min_z, max_x, max_y, max_z)
        self.lod_shapes = []      # IDs of lower-detail versions

    def to_dict(self):
        return {
            "id": self.id,
            "vertices": [v.to_dict() for v in self.vertices],
            "faces": [f.to_dict() for f in self.faces],
            "collision_box": self.collision_box,
            "lod_shapes": self.lod_shapes,
        }

    def to_obj(self, scale=0.01):
        lines = [f"# Star Fox Shape {self.id}"]
        lines.append(f"# {len(self.vertices)} vertices, {len(self.faces)} faces")
        lines.append(f"o shape_{self.id}")
        for v in self.vertices:
            lines.append(v.to_obj(scale))
        for f in self.faces:
            lines.append(f.to_obj())
        return "\n".join(lines)


def parse_shape_header(data, offset):
    """Parse a shape header from binary data.

    Star Fox shape format (from SHMACS.INC):
    - Number of vertices (byte)
    - Number of faces (byte)
    - Vertex data: list of (x, y, z) as signed bytes or words
    - Face data: list of (num_verts, vertex_indices..., color_index)

    Note: The exact format needs verification against the ROM data.
    This is a best-effort parser based on the assembly macros.
    """
    if offset + 2 > len(data):
        return None, offset

    num_verts = data[offset]
    num_faces = data[offset + 1]
    offset += 2

    if num_verts == 0 or num_verts > 200:
        return None, offset

    shape = Shape(0)

    # Parse vertices (signed 16-bit x, y, z)
    for _ in range(num_verts):
        if offset + 6 > len(data):
            break
        x = struct.unpack_from("<h", data, offset)[0]
        y = struct.unpack_from("<h", data, offset + 2)[0]
        z = struct.unpack_from("<h", data, offset + 4)[0]
        shape.vertices.append(ShapeVertex(x, y, z))
        offset += 6

    # Parse faces
    for _ in range(num_faces):
        if offset + 1 > len(data):
            break
        nv = data[offset]
        offset += 1
        if nv == 0 or nv > 8:
            break

        if offset + nv + 1 > len(data):
            break

        indices = []
        for _ in range(nv):
            indices.append(data[offset])
            offset += 1
        color_idx = data[offset]
        offset += 1

        shape.faces.append(ShapeFace(indices, color_idx))

    return shape, offset


def scan_shapes(data, max_shapes=512):
    """Scan binary data for valid shapes."""
    shapes = []
    offset = 0

    while offset < len(data) - 4 and len(shapes) < max_shapes:
        shape, new_offset = parse_shape_header(data, offset)
        if shape and len(shape.vertices) >= 3 and len(shape.faces) >= 1:
            shape.id = len(shapes)
            shapes.append(shape)
            offset = new_offset
        else:
            offset += 1  # Skip byte and try again

    return shapes


def export_json(shapes, output_path):
    """Export shapes as JSON."""
    data = {"shapes": [s.to_dict() for s in shapes]}
    with open(output_path, "w") as f:
        json.dump(data, f, indent=2)
    print(f"Exported {len(shapes)} shapes to {output_path}")


def export_obj(shape, output_path):
    """Export a single shape as OBJ."""
    with open(output_path, "w") as f:
        f.write(shape.to_obj())
    print(f"Exported shape {shape.id} to {output_path}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python3 shape_tool.py <shapes.bin> [options]")
        print("  --dump          Print shape summary")
        print("  --json <file>   Export all shapes as JSON")
        print("  --obj <id>      Export shape as OBJ file")
        sys.exit(1)

    shapes_path = sys.argv[1]
    if not os.path.exists(shapes_path):
        print(f"Error: file not found: {shapes_path}")
        sys.exit(1)

    with open(shapes_path, "rb") as f:
        data = f.read()

    print(f"Loaded {len(data)} bytes from {shapes_path}")
    shapes = scan_shapes(data)
    print(f"Found {len(shapes)} shapes")

    if "--dump" in sys.argv:
        for s in shapes:
            print(f"  Shape {s.id}: {len(s.vertices)} verts, {len(s.faces)} faces")

    if "--json" in sys.argv:
        idx = sys.argv.index("--json")
        out = sys.argv[idx + 1] if idx + 1 < len(sys.argv) else "shapes.json"
        export_json(shapes, out)

    if "--obj" in sys.argv:
        idx = sys.argv.index("--obj")
        shape_id = int(sys.argv[idx + 1]) if idx + 1 < len(sys.argv) else 0
        if shape_id < len(shapes):
            export_obj(shapes[shape_id], f"shape_{shape_id}.obj")
        else:
            print(f"Shape {shape_id} not found (max: {len(shapes) - 1})")
