#!/usr/bin/env python3
"""
Star Fox HD -- Shape Compiler

Reads SNES ASM shape files (SHAPES*.ASM, USHAPES.ASM, KSHAPES.ASM, PSHAPES.ASM)
and the shape ID registry (ISTRATS.ASM), then generates C data arrays for all
shapes at build time.

Usage:
    python3 tools/shape_compiler.py

Output:
    src/renderer/shape_data.h
"""

from __future__ import annotations

import ast
import os
import re
import sys
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

ISTRATS_PATH = os.path.join(REPO_ROOT, "reference/ultrastarfox/SF/STRAT/ISTRATS.ASM")

SHAPE_ASM_FILES = [
    "reference/ultrastarfox/SF/SHAPES/SHAPES.ASM",
    "reference/ultrastarfox/SF/SHAPES/SHAPES2.ASM",
    "reference/ultrastarfox/SF/SHAPES/SHAPES3.ASM",
    "reference/ultrastarfox/SF/SHAPES/SHAPES4.ASM",
    "reference/ultrastarfox/SF/SHAPES/SHAPES5.ASM",
    "reference/ultrastarfox/SF/SHAPES/SHAPES6.ASM",
    "reference/ultrastarfox/SF/SHAPES/USHAPES.ASM",
    "reference/ultrastarfox/SF/SHAPES/KSHAPES.ASM",
    "reference/ultrastarfox/SF/SHAPES/PSHAPES.ASM",
]

OUTPUT_PATH = os.path.join(REPO_ROOT, "src/renderer/shape_data.h")
RUST_OUTPUT_PATH = os.path.join(REPO_ROOT, "rust/sf-render/src/shape_data.rs")

# INC files providing global equates referenced by shape headers
# (e.g. bu_scale/bossN_scale shift constants live in STRATEQU.INC).
# Locally-defined symbols in a shape ASM file always take precedence.
INC_SYMBOL_FILES = [
    "reference/ultrastarfox/SF/INC/STRATEQU.INC",
]

# Shape IDs to skip in the output (using the "MACRO-counted" numbering
# where def_shape MACRO is ID 0, matching g_istrat_shape_defaults).
SKIP_SHAPE_IDS = {
    0,    # macro -- MACRO definition, not a real shape
    1,    # nullshape -- no geometry
}

# ---------------------------------------------------------------------------
# Extended-bank shapes.
#
# These meshes exist in SHAPES*.ASM / USHAPES.ASM but are NOT in the
# ISTRATS.ASM def_shape table, so they have no MACRO-counted id. The runtime
# reserves flat slots for them below MAX_SHAPES (512) in shapes.c:
#   - 508..510 are the SHAPE_ALIAS_OP_* slots that Shapes_ResolveShapeWord
#     already maps the raw 16-bit shape words 551/552/553 onto (the
#     launch-intro runway strips referenced by levels.c SH_OP_0..2).
#   - 256..287 is the extended-bank window owned by this compiler; the ids
#     are stable by definition in this table. 278 stays reserved for the
#     SHAPE_ALIAS_MOTHER1 slot from shapes.h (mother1 has no ASM geometry).
# Matching SHAPE_EXT_* macros are emitted into shape_data.h so game/strat
# code can retarget its current nullshape proxies by name.
EXTENDED_SHAPES = {
    # lowercase ASM label : fixed runtime shape id
    "op_0":       508,  # runway edge-light rails (wireframe, Face2 only)
    "op_1":       509,  # runway main strip
    "op_2":       510,  # runway end strip
    "mybase_0":   256,  # launch-intro base building (SHAPES5.ASM)
    "sea_0":      257,  # sea creature pose 0 (SHAPES.ASM)
    "sea_0_1":    258,  # sea creature pose 1 (SHAPES.ASM)
    "boss_2_0":   259,  # boss2 petal (SHAPES2.ASM)
    "boss_2_1":   260,  # boss2 pod (SHAPES2.ASM)
    "boss_2_3":   261,  # boss2 part (SHAPES2.ASM)
    "boss_2_4":   262,  # boss2 part (SHAPES2.ASM)
    "boss_2_5":   263,  # boss2 part (SHAPES2.ASM)
    "boss_8_1":   264,  # boss8 cover, open colbox variant (SHAPES2.ASM)
    "boss_8_1c":  265,  # boss8 cover, closed variant (SHAPES2.ASM)
    "sparklas":   266,  # boss8 rotating beam bolt (SHAPES2.ASM)
    "shrap1":     267,  # boss8 falling shrapnel (SHAPES2.ASM)
    "shyper":     268,  # boss8 death hyper ring (wireframe, Face2 only)
    "zaco_9":     269,  # boss8 kami homing missile body (SHAPES2.ASM)
    "boss_g_s":   270,  # boss_g shadow clone (SHAPES3.ASM)
    "f_fish":     271,  # flying fish (SHAPES3.ASM)
    "rockbeam":   272,  # rock beam sprite quad (USHAPES.ASM)
    "l2smoke":    273,  # large smoke sprite quad (USHAPES.ASM)
    "explosion5": 274,  # explosion sprite quad (USHAPES.ASM)
    "asteroid1":  275,  # asteroid sprite quad, level 1_2 belt (USHAPES.ASM)
    "asteroid3":  276,  # asteroid sprite quad (USHAPES.ASM)
    "asteroid4":  277,  # asteroid sprite quad (USHAPES.ASM)
    # 278 reserved: SHAPE_ALIAS_MOTHER1 (mother1 shapehdr has 0,0 geometry)
    "big_meteor": 279,  # big meteor sprite quad (USHAPES.ASM)
    "clasteroid": 280,  # clear-demo asteroid sprite quad (USHAPES.ASM)
    "whale":      281,  # whale, animated frame 0 (SHAPES5.ASM)
    "my_bird":    282,  # training bird (SHAPES5.ASM)
}


# ---------------------------------------------------------------------------
# ASM line stripping / parsing utilities
# ---------------------------------------------------------------------------

def strip_asm_comment(line: str) -> str:
    """Remove `;` comments, but NOT inside `<angle brackets>`."""
    in_angle = False
    for i, ch in enumerate(line):
        if ch == '<':
            in_angle = True
        elif ch == '>':
            in_angle = False
        elif ch == ';' and not in_angle:
            return line[:i]
    return line


def split_asm_args(args_str: str) -> List[str]:
    """Split comma-separated ASM arguments, respecting parens and angle brackets."""
    parts: List[str] = []
    paren_depth = 0
    angle_depth = 0
    start = 0
    for i, ch in enumerate(args_str):
        if ch == '(':
            paren_depth += 1
        elif ch == ')' and paren_depth > 0:
            paren_depth -= 1
        elif ch == '<':
            angle_depth += 1
        elif ch == '>' and angle_depth > 0:
            angle_depth -= 1
        elif ch == ',' and paren_depth == 0 and angle_depth == 0:
            parts.append(args_str[start:i].strip())
            start = i + 1
    parts.append(args_str[start:].strip())
    return parts


# ---------------------------------------------------------------------------
# Simple expression evaluator for ASM integer expressions
# ---------------------------------------------------------------------------

class ExprEval(ast.NodeVisitor):
    """Evaluate simple integer arithmetic expressions using Python's AST."""

    def __init__(self, symbols: Dict[str, int]):
        self.symbols = symbols

    def visit_Expression(self, node: ast.Expression) -> int:
        return self.visit(node.body)

    def visit_BinOp(self, node: ast.BinOp) -> int:
        left = self.visit(node.left)
        right = self.visit(node.right)
        if isinstance(node.op, ast.Add):
            return left + right
        if isinstance(node.op, ast.Sub):
            return left - right
        if isinstance(node.op, ast.Mult):
            return left * right
        if isinstance(node.op, (ast.Div, ast.FloorDiv)):
            return 0 if right == 0 else (left // right)
        if isinstance(node.op, ast.Mod):
            return 0 if right == 0 else (left % right)
        if isinstance(node.op, ast.BitAnd):
            return left & right
        if isinstance(node.op, ast.BitOr):
            return left | right
        if isinstance(node.op, ast.BitXor):
            return left ^ right
        if isinstance(node.op, ast.LShift):
            return left << right
        if isinstance(node.op, ast.RShift):
            return left >> right
        raise ValueError("unsupported binop")

    def visit_UnaryOp(self, node: ast.UnaryOp) -> int:
        v = self.visit(node.operand)
        if isinstance(node.op, ast.UAdd):
            return +v
        if isinstance(node.op, ast.USub):
            return -v
        if isinstance(node.op, ast.Invert):
            return ~v
        raise ValueError("unsupported unary op")

    def visit_Constant(self, node: ast.Constant) -> int:
        if isinstance(node.value, int):
            return node.value
        raise ValueError("unsupported constant")

    def visit_Name(self, node: ast.Name) -> int:
        return self.symbols.get(node.id.lower(), 0)

    def generic_visit(self, node: ast.AST) -> int:
        raise ValueError(f"unsupported node {type(node).__name__}")


def eval_expr(expr_str: str, symbols: Dict[str, int]) -> Optional[int]:
    """Evaluate an integer expression string. Returns None on failure."""
    expr_str = expr_str.strip()
    if not expr_str:
        return None

    # Handle hex literals like $FF
    expr_str = re.sub(r'\$([0-9A-Fa-f]+)', r'0x\1', expr_str)

    # Replace & (used as bitwise AND in some assemblers) only when not part of &&
    # Actually in 65816 ASM, & is used for address masking, handle it carefully
    # We rely on Python's AST which uses & naturally

    try:
        tree = ast.parse(expr_str, mode='eval')
        return ExprEval(symbols).visit(tree)
    except Exception:
        # Try as plain integer
        try:
            return int(expr_str, 0)
        except ValueError:
            return None


# ---------------------------------------------------------------------------
# ASM line representation
# ---------------------------------------------------------------------------

@dataclass
class AsmLine:
    label: str = ""     # label on this line (if any)
    op: str = ""        # opcode/directive (lowercase)
    args: str = ""      # raw argument string
    raw: str = ""       # original line text (for debugging)


KNOWN_OPS = {
    "shapehdr", "shapehdr_s", "pointsb", "pointsw", "pointsxb", "pointsxw",
    "pb", "pw", "pbd2", "pby2", "frames", "jumptab", "jump",
    "endpoints", "faces", "face2", "face3", "face4", "face5", "face6",
    "face7", "face8", "face9", "face10", "face11", "face12",
    "aface3", "aface4", "fend", "fendq",
    "endshape", "vizis", "viz", "bspinit", "bsp", "bspe", "bspnull", "bspend",
    "datahdr", "db", "dbh", "collite", "coldepth", "colnorm", "colanim",
    "coltext", "colsmooth", "rept", "endr", "ifeq", "ifne", "elseif", "endc",
    "public", "extern", "incfile", "incpublics", "start_shapes",
    "s_sprite", "equ",
}


def parse_asm_line(raw_line: str) -> AsmLine:
    """Parse a single ASM line into label, opcode, and args."""
    stripped = strip_asm_comment(raw_line).strip()
    if not stripped:
        return AsmLine(raw=raw_line)

    result = AsmLine(raw=raw_line)

    # Handle assignment: `name = expr`
    eq_match = re.match(r'^(\w+)\s*=\s*(.*)$', stripped)
    if eq_match:
        result.label = eq_match.group(1)
        result.op = "="
        result.args = eq_match.group(2).strip()
        return result

    # Handle `name equ expr`
    equ_match = re.match(r'^(\w+)\s+(?:equ)\s+(.*)$', stripped, re.IGNORECASE)
    if equ_match:
        result.label = equ_match.group(1)
        result.op = "equ"
        result.args = equ_match.group(2).strip()
        return result

    # General: possibly a label, then an opcode, then args
    # Labels start at column 0 (no leading whitespace) in most assemblers,
    # but in these files labels can be anywhere. A token is a label if the
    # next token is a known op.
    tokens = stripped.split(None, 2)
    if not tokens:
        return result

    first = tokens[0]

    # If only one token
    if len(tokens) == 1:
        if first.lower() in KNOWN_OPS:
            result.op = first.lower()
        else:
            result.label = first
        return result

    second = tokens[1]
    rest = tokens[2] if len(tokens) > 2 else ""

    # Check if second token is `=`
    if second == '=' or second.lower() == 'equ':
        result.label = first
        result.op = second.lower() if second.lower() == 'equ' else '='
        result.args = rest.strip()
        return result

    # If first token is a known op
    if first.lower() in KNOWN_OPS:
        result.op = first.lower()
        # Rejoin second + rest as args
        result.args = (second + " " + rest).strip() if rest else second
        return result

    # If second token is a known op
    if second.lower() in KNOWN_OPS:
        result.label = first
        result.op = second.lower()
        result.args = rest.strip()
        return result

    # Heuristic: if first is not a known op and second is not either,
    # treat first as label and second as op
    if not first.lower().startswith('.') and re.match(r'^[A-Za-z_]', first):
        result.label = first
        result.op = second.lower()
        result.args = rest.strip()
    else:
        result.op = first.lower()
        result.args = (second + " " + rest).strip() if rest else second

    return result


# ---------------------------------------------------------------------------
# ASM file representation
# ---------------------------------------------------------------------------

@dataclass
class AsmFile:
    path: str
    lines: List[AsmLine] = field(default_factory=list)
    symbols: Dict[str, str] = field(default_factory=dict)  # name -> expr string
    resolved: Dict[str, int] = field(default_factory=dict)  # name -> int value


def load_asm_file(path: str) -> AsmFile:
    """Load and parse an ASM file."""
    af = AsmFile(path=path)
    with open(path, 'r', errors='replace') as f:
        for raw in f:
            af.lines.append(parse_asm_line(raw))
    # Collect symbol definitions (equ and =)
    for line in af.lines:
        if line.label and line.op in ('equ', '=') and line.args:
            af.symbols[line.label.lower()] = line.args
    return af


def resolve_symbol(af: AsmFile, name: str, _resolving: Optional[set] = None) -> int:
    """Resolve a symbol to an integer value."""
    key = name.lower()
    if key in af.resolved:
        return af.resolved[key]
    if _resolving is None:
        _resolving = set()
    if key in _resolving:
        return 0  # circular
    _resolving.add(key)
    expr_str = af.symbols.get(key)
    if expr_str is None:
        return 0
    # Build a symbols dict for the evaluator that can resolve recursively
    val = eval_expr(expr_str, af.resolved)
    if val is not None:
        af.resolved[key] = val
    else:
        af.resolved[key] = 0
        val = 0
    _resolving.discard(key)
    return val


def resolve_all_symbols(af: AsmFile) -> None:
    """Resolve all symbols in an ASM file."""
    for name in list(af.symbols.keys()):
        resolve_symbol(af, name)


def eval_in_file(af: AsmFile, expr_str: str) -> Optional[int]:
    """Evaluate an expression in the context of a file's symbols."""
    return eval_expr(expr_str, af.resolved)


# ---------------------------------------------------------------------------
# Shape data structures
# ---------------------------------------------------------------------------

@dataclass
class Vertex:
    x: float
    y: float
    z: float


@dataclass
class Face:
    vertex_indices: List[int]
    color_index: int


@dataclass
class ShapeHeader:
    label: str              # shape name (from the label on the shapehdr line)
    points_label: str       # label for vertex data
    faces_label: str        # label for face data
    shift: int              # coordinate shift
    color_table: str        # color table name


@dataclass
class ShapeData:
    shape_id: int
    name: str
    vertices: List[Vertex]
    faces: List[Face]


# ---------------------------------------------------------------------------
# Parse ISTRATS.ASM for def_shape entries
# ---------------------------------------------------------------------------

def parse_def_shapes(istrats_path: str) -> List[Tuple[int, str]]:
    """Parse def_shape entries from ISTRATS.ASM.
    Returns list of (id, name) tuples.
    """
    shapes: List[Tuple[int, str]] = []
    shape_id = 0

    with open(istrats_path, 'r', errors='replace') as f:
        for raw_line in f:
            line = strip_asm_comment(raw_line).strip()
            if not line:
                continue
            # Match def_shape lines (including the MACRO definition line,
            # which gets counted as ID 0 to match the istrat table numbering
            # used by g_istrat_shape_defaults in istrat_shapes.c).
            m = re.match(r'^def_shape\s+(\w+)', line, re.IGNORECASE)
            if m:
                name = m.group(1)
                shapes.append((shape_id, name.lower()))
                shape_id += 1

    return shapes


# ---------------------------------------------------------------------------
# Find labels in ASM file
# ---------------------------------------------------------------------------

def find_label_line(af: AsmFile, label: str) -> int:
    """Find the line index where a label is defined. Returns -1 if not found."""
    target = label.lower()
    for i, line in enumerate(af.lines):
        if line.label and line.label.lower() == target:
            return i
    return -1


# ---------------------------------------------------------------------------
# Parse vertices (frame 0 only for animated shapes)
# ---------------------------------------------------------------------------

def parse_vertices(af: AsmFile, points_label: str, shift: int) -> List[Vertex]:
    """Parse vertex data starting from points_label."""
    start = find_label_line(af, points_label)
    if start < 0:
        return []

    # Find the EndPoints marker
    end = -1
    for i in range(start, len(af.lines)):
        if af.lines[i].op == 'endpoints':
            end = i
            break
    if end < 0:
        return []

    # Check for animation frames -- if present, follow frame 0 only
    vertices: List[Vertex] = []
    pc = start
    pending_count = 0
    pending_mirror = False
    guard = 0
    max_guard = (end - start) * 64

    while pc >= start and pc <= end and guard < max_guard:
        guard += 1
        line = af.lines[pc]

        if line.op == 'endpoints':
            break

        if line.op in ('pointsb', 'pointsw', 'pointsxb', 'pointsxw'):
            val = eval_in_file(af, line.args)
            pending_count = val if val is not None else 0
            pending_mirror = line.op in ('pointsxb', 'pointsxw')
            pc += 1
            continue

        if line.op in ('pb', 'pw', 'pbd2', 'pby2'):
            if pending_count > 0:
                args = split_asm_args(line.args)
                if len(args) >= 3:
                    x_val = eval_in_file(af, args[0])
                    y_val = eval_in_file(af, args[1])
                    z_val = eval_in_file(af, args[2])
                    x = x_val if x_val is not None else 0
                    y = y_val if y_val is not None else 0
                    z = z_val if z_val is not None else 0

                    if line.op == 'pbd2':
                        x //= 2
                        y //= 2
                        z //= 2
                    elif line.op == 'pby2':
                        y *= 2

                    vertices.append(Vertex(float(x), float(y), float(z)))
                    if pending_mirror:
                        vertices.append(Vertex(float(-x), float(y), float(z)))

                pending_count -= 1
            pc += 1
            continue

        if line.op == 'frames':
            # Animated shape: follow frame 0 via jumptab
            val = eval_in_file(af, line.args)
            frame_count = val if val is not None and val > 0 else 1
            # Find the first jumptab after this line
            scan = pc + 1
            while scan < end:
                jline = af.lines[scan]
                if jline.op == 'jumptab':
                    # Follow this label (frame 0)
                    jump_target = jline.args.strip().strip('<>').strip()
                    target_line = find_label_line_range(af, jump_target, start, end)
                    if target_line >= 0:
                        pc = target_line
                    else:
                        pc = scan + 1
                    break
                scan += 1
            else:
                pc += 1
            continue

        if line.op == 'jump':
            # Follow jump to another label within the vertex section
            jump_target = line.args.strip().strip('<>').strip()
            target_line = find_label_line_range(af, jump_target, start, end)
            if target_line >= 0:
                pc = target_line
                continue
            pc += 1
            continue

        # Skip everything else (datahdr, rept, endr, etc.)
        pc += 1

    # Apply shift scaling
    if shift != 0:
        if shift > 0:
            scale = float(1 << shift)
        else:
            scale = 1.0 / float(1 << (-shift))
        for v in vertices:
            v.x *= scale
            v.y *= scale
            v.z *= scale

    return vertices


def find_label_line_range(af: AsmFile, label: str, start: int, end: int) -> int:
    """Find a label within a range of lines."""
    target = label.lower()
    # Handle local labels (starting with .)
    for i in range(start, min(end + 1, len(af.lines))):
        line = af.lines[i]
        if line.label and line.label.lower() == target:
            # If this line has no op, the actual content is on the next line
            if not line.op and i + 1 < len(af.lines):
                return i + 1
            return i
    return -1


# ---------------------------------------------------------------------------
# Parse faces
# ---------------------------------------------------------------------------

def parse_faces(af: AsmFile, faces_label: str) -> List[Face]:
    """Parse face data starting from faces_label."""
    start = find_label_line(af, faces_label)
    if start < 0:
        return []

    faces: List[Face] = []

    for i in range(start, len(af.lines)):
        line = af.lines[i]

        if line.op == 'endshape':
            break

        # Match faceN where N >= 2. Face2 is a wireframe line segment
        # (args: color, vis, nx, ny, nz, v0, v1); it is emitted with
        # num_verts == 2 and rendered via the GL_LINES path.
        if line.op.startswith('face') and line.op != 'faces':
            # Skip aface3, aface4 (animated faces)
            if line.op.startswith('aface'):
                continue

            try:
                nverts = int(line.op[4:])
            except ValueError:
                continue

            if nverts < 2 or nverts > 12:
                continue

            args = split_asm_args(line.args)
            # Args: color_idx, vis_idx, nx, ny, nz, v0..v(N-1)
            # We need at least nverts + 5 args
            if len(args) < nverts + 5:
                continue

            color_val = eval_in_file(af, args[0])
            color_index = color_val if color_val is not None else 0

            # Vertex indices are the last N args
            base = len(args) - nverts
            vindices: List[int] = []
            ok = True
            for vi in range(nverts):
                val = eval_in_file(af, args[base + vi])
                if val is not None:
                    vindices.append(val)
                else:
                    vindices.append(0)

            faces.append(Face(vertex_indices=vindices, color_index=color_index))

    return faces


# ---------------------------------------------------------------------------
# Parse shape headers from all ASM files
# ---------------------------------------------------------------------------

def parse_shape_headers(af: AsmFile) -> List[ShapeHeader]:
    """Extract all shapehdr / shapehdr_s entries from an ASM file."""
    headers: List[ShapeHeader] = []

    for i, line in enumerate(af.lines):
        if line.op not in ('shapehdr', 'shapehdr_s'):
            continue

        args = split_asm_args(line.args)
        if len(args) < 14:
            continue

        # Determine the shape label(s).  Some shapes have multiple bare
        # label aliases stacked before the shapehdr (e.g. boss_9_5 / boss_9_6).
        # Collect ALL of them so every alias can be matched to a def_shape ID.
        shape_labels: List[str] = []
        if line.label:
            shape_labels.append(line.label.lower())
        # Look backwards for bare labels
        for j in range(i - 1, max(i - 10, -1), -1):
            prev = af.lines[j]
            if prev.label and not prev.op:
                shape_labels.append(prev.label.lower())
            elif prev.op:
                break

        if not shape_labels:
            continue

        # Strip angle brackets from last arg (display name)
        points_label = args[0].strip()
        faces_label = args[2].strip()

        # Parse shift value (args[7])
        shift_val = eval_in_file(af, args[7])
        shift = shift_val if shift_val is not None else 0

        # Color table name (args[13]) -- strip angle brackets if present
        # But first strip any trailing <display_name> from the entire args
        color_table = args[13].strip() if len(args) > 13 else "0"

        for lbl in shape_labels:
            headers.append(ShapeHeader(
                label=lbl,
                points_label=points_label,
                faces_label=faces_label,
                shift=shift,
                color_table=color_table,
            ))

    return headers


# ---------------------------------------------------------------------------
# Rust emitter (rust/sf-render/src/shape_data.rs)
#
# Emits the exact same shape set as the C emitter (same skip rules, same
# vertex Y-negation / -0.0 avoidance, same %.1f float formatting, same
# 12-slot index padding) so the two outputs stay numerically identical.
# The C emitter above is the reference; do not change one without the other.
# ---------------------------------------------------------------------------

def emit_rust(sorted_shapes: List[ShapeData], ext_compiled: Dict[str, int]) -> None:
    out: List[str] = []
    out.append("// Auto-generated by tools/shape_compiler.py -- do not edit")
    out.append("//! Shape mesh data compiled from SHAPES*.ASM / USHAPES.ASM /")
    out.append("//! KSHAPES.ASM / PSHAPES.ASM via ISTRATS.ASM def_shape numbering.")
    out.append("//! Mirrors the C output in src/renderer/shape_data.h; both emitters")
    out.append("//! live in tools/shape_compiler.py (single source of truth).")
    out.append("")

    if ext_compiled:
        out.append("// Extended-bank shape slots (meshes that are not in the ISTRATS")
        out.append("// def_shape catalog). Slot numbers are fixed by tools/shape_compiler.py;")
        out.append("// 508-510 match the SHAPE_ALIAS_OP_* aliases in shapes.h.")
        for ext_name in sorted(ext_compiled, key=lambda n: ext_compiled[n]):
            out.append(
                f"pub const SHAPE_EXT_{ext_name.upper()}: u16 = "
                f"{ext_compiled[ext_name]};")
        out.append("")

    out.append("/// Mesh vertex, matching C `ShapeVertex` (src/renderer/shapes.h).")
    out.append("#[derive(Debug, Clone, Copy, PartialEq)]")
    out.append("pub struct ShapeVertex {")
    out.append("    pub x: f32,")
    out.append("    pub y: f32,")
    out.append("    pub z: f32,")
    out.append("}")
    out.append("")
    out.append("/// Polygon face, matching C `ShapeFace` (src/renderer/shapes.h).")
    out.append("/// `num_verts == 2` is a Face2 wireframe line segment.")
    out.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
    out.append("pub struct ShapeFace {")
    out.append("    pub vertex_indices: [u16; 12],")
    out.append("    pub num_verts: u8,")
    out.append("    pub color_index: u8,")
    out.append("}")
    out.append("")
    out.append("/// One compiled shape, matching C `ShapeDataEntry` (shape_data.h).")
    out.append("#[derive(Debug, Clone, Copy)]")
    out.append("pub struct ShapeDataEntry {")
    out.append("    pub shape_id: u16,")
    out.append("    pub vertices: &'static [ShapeVertex],")
    out.append("    pub faces: &'static [ShapeFace],")
    out.append("    pub name: &'static str,")
    out.append("}")
    out.append("")
    out.append("const fn v(x: f32, y: f32, z: f32) -> ShapeVertex {")
    out.append("    ShapeVertex { x, y, z }")
    out.append("}")
    out.append("")
    out.append("const fn f(vertex_indices: [u16; 12], num_verts: u8, color_index: u8) -> ShapeFace {")
    out.append("    ShapeFace { vertex_indices, num_verts, color_index }")
    out.append("}")
    out.append("")

    for shape in sorted_shapes:
        out.append(f"// Shape {shape.shape_id}: {shape.name}")

        # Vertices -- same Y-negation / -0.0 avoidance as the C emitter.
        out.append(
            f"static SHAPE_{shape.shape_id}_VERTS: "
            f"[ShapeVertex; {len(shape.vertices)}] = [")
        for vert in shape.vertices:
            neg_y = -vert.y if vert.y != 0.0 else 0.0
            vx = vert.x if vert.x != 0.0 else 0.0
            vz = vert.z if vert.z != 0.0 else 0.0
            out.append(f"    v({vx:.1f}, {neg_y:.1f}, {vz:.1f}),")
        out.append("];")

        # Faces -- indices padded to 12, same as the C emitter.
        out.append(
            f"static SHAPE_{shape.shape_id}_FACES: "
            f"[ShapeFace; {len(shape.faces)}] = [")
        for face in shape.faces:
            padded = list(face.vertex_indices) + [0] * (12 - len(face.vertex_indices))
            indices_str = ", ".join(str(idx) for idx in padded)
            out.append(
                f"    f([{indices_str}], {len(face.vertex_indices)}, "
                f"{face.color_index}),")
        out.append("];")
        out.append("")

    out.append(f"pub const SHAPE_DATA_COUNT: usize = {len(sorted_shapes)};")
    out.append("")
    out.append("pub static SHAPE_DATA: [ShapeDataEntry; SHAPE_DATA_COUNT] = [")
    for shape in sorted_shapes:
        out.append(
            f"    ShapeDataEntry {{ shape_id: {shape.shape_id}, "
            f"vertices: &SHAPE_{shape.shape_id}_VERTS, "
            f"faces: &SHAPE_{shape.shape_id}_FACES, "
            f'name: "{shape.name}" }},')
    out.append("];")
    out.append("")

    os.makedirs(os.path.dirname(RUST_OUTPUT_PATH), exist_ok=True)
    with open(RUST_OUTPUT_PATH, 'w') as f:
        f.write("\n".join(out))

    print(f"  Wrote {RUST_OUTPUT_PATH} ({len(sorted_shapes)} shapes)",
          file=sys.stderr)


# ---------------------------------------------------------------------------
# Main compilation
# ---------------------------------------------------------------------------

def main() -> int:
    print(f"Shape compiler: reading ISTRATS.ASM ...", file=sys.stderr)

    # 1. Parse shape name -> ID mapping
    def_shapes = parse_def_shapes(ISTRATS_PATH)
    name_to_id: Dict[str, int] = {}
    for sid, sname in def_shapes:
        name_to_id[sname] = sid

    print(f"  Found {len(def_shapes)} def_shape entries", file=sys.stderr)

    # 2a. Load global equates from INC files (raw expression strings; they
    # are resolved per shape file after local symbols are merged in).
    inc_symbols: Dict[str, str] = {}
    for rel_path in INC_SYMBOL_FILES:
        full_path = os.path.join(REPO_ROOT, rel_path)
        if not os.path.exists(full_path):
            print(f"  WARNING: {rel_path} not found, skipping", file=sys.stderr)
            continue
        inc_af = load_asm_file(full_path)
        inc_symbols.update(inc_af.symbols)
        print(f"  Loaded {rel_path}: {len(inc_af.symbols)} equates", file=sys.stderr)

    # 2. Load all shape ASM files
    asm_files: List[AsmFile] = []
    for rel_path in SHAPE_ASM_FILES:
        full_path = os.path.join(REPO_ROOT, rel_path)
        if not os.path.exists(full_path):
            print(f"  WARNING: {rel_path} not found, skipping", file=sys.stderr)
            continue
        af = load_asm_file(full_path)
        # Seed global INC equates (shift/scale constants); local defs win.
        for sym_name, sym_expr in inc_symbols.items():
            af.symbols.setdefault(sym_name, sym_expr)
        resolve_all_symbols(af)
        asm_files.append(af)
        print(f"  Loaded {rel_path}: {len(af.lines)} lines, {len(af.symbols)} symbols", file=sys.stderr)

    # 3. Collect all shape headers across all files
    all_headers: List[Tuple[ShapeHeader, AsmFile]] = []
    for af in asm_files:
        for hdr in parse_shape_headers(af):
            all_headers.append((hdr, af))

    print(f"  Found {len(all_headers)} shape headers", file=sys.stderr)

    # 4. Build shape data: match headers to def_shape IDs
    # Track which geometry (points_label, faces_label) we've already processed
    # to handle multiple shapehdr entries sharing the same geometry.
    compiled_shapes: Dict[int, ShapeData] = {}  # shape_id -> ShapeData
    processed_geometry: set = set()  # (file_path, points_lower, faces_lower)

    for hdr, af in all_headers:
        shape_id = name_to_id.get(hdr.label)
        if shape_id is None:
            continue

        if shape_id in SKIP_SHAPE_IDS:
            continue

        if shape_id in compiled_shapes:
            continue  # already have this shape

        # Skip shapes with no geometry
        if hdr.points_label == '0' or hdr.faces_label == '0':
            continue

        # Check if this geometry was already processed (different color variant)
        geom_key = (af.path, hdr.points_label.lower(), hdr.faces_label.lower())

        # Parse vertices and faces -- try the shape's own file first,
        # then fall back to searching all loaded files (cross-file refs).
        vertices = parse_vertices(af, hdr.points_label, hdr.shift)
        faces = parse_faces(af, hdr.faces_label)

        if not vertices:
            for other_af in asm_files:
                if other_af is not af:
                    vertices = parse_vertices(other_af, hdr.points_label, hdr.shift)
                    if vertices:
                        break

        if not faces:
            for other_af in asm_files:
                if other_af is not af:
                    faces = parse_faces(other_af, hdr.faces_label)
                    if faces:
                        break

        if not vertices or not faces:
            continue

        compiled_shapes[shape_id] = ShapeData(
            shape_id=shape_id,
            name=hdr.label,
            vertices=vertices,
            faces=faces,
        )
        processed_geometry.add(geom_key)

    print(f"  Compiled {len(compiled_shapes)} shapes with geometry", file=sys.stderr)

    # 4b. Extended-bank pass: shapes outside the ISTRATS def_shape catalog,
    # compiled into the fixed runtime slots defined by EXTENDED_SHAPES.
    ext_compiled: Dict[str, int] = {}  # name -> id (for macro emission)
    for hdr, af in all_headers:
        ext_id = EXTENDED_SHAPES.get(hdr.label)
        if ext_id is None:
            continue
        if ext_id in compiled_shapes:
            continue  # already have this slot
        if hdr.points_label == '0' or hdr.faces_label == '0':
            continue  # header-only shape (no geometry)

        vertices = parse_vertices(af, hdr.points_label, hdr.shift)
        faces = parse_faces(af, hdr.faces_label)

        if not vertices:
            for other_af in asm_files:
                if other_af is not af:
                    vertices = parse_vertices(other_af, hdr.points_label,
                                              hdr.shift)
                    if vertices:
                        break

        if not faces:
            for other_af in asm_files:
                if other_af is not af:
                    faces = parse_faces(other_af, hdr.faces_label)
                    if faces:
                        break

        if not vertices or not faces:
            print(f"  WARNING: extended shape {hdr.label} (id {ext_id}) has "
                  f"no geometry (points={hdr.points_label}, "
                  f"faces={hdr.faces_label})", file=sys.stderr)
            continue

        compiled_shapes[ext_id] = ShapeData(
            shape_id=ext_id,
            name=hdr.label,
            vertices=vertices,
            faces=faces,
        )
        ext_compiled[hdr.label] = ext_id

    missing_ext = sorted(set(EXTENDED_SHAPES) - set(ext_compiled))
    print(f"  Compiled {len(ext_compiled)} extended-bank shapes"
          + (f" (missing: {', '.join(missing_ext)})" if missing_ext else ""),
          file=sys.stderr)

    # 5. Generate output
    sorted_shapes = sorted(compiled_shapes.values(), key=lambda s: s.shape_id)

    out_lines: List[str] = []
    out_lines.append("// Auto-generated by tools/shape_compiler.py -- do not edit")
    out_lines.append("#ifndef STARFOX_RENDERER_SHAPE_DATA_H")
    out_lines.append("#define STARFOX_RENDERER_SHAPE_DATA_H")
    out_lines.append("")
    out_lines.append('#include "../types.h"')
    out_lines.append('#include "shapes.h"')
    out_lines.append("")

    if ext_compiled:
        out_lines.append("// Extended-bank shape slots (meshes that are not in the ISTRATS")
        out_lines.append("// def_shape catalog). Slot numbers are fixed by tools/shape_compiler.py;")
        out_lines.append("// 508-510 match the SHAPE_ALIAS_OP_* aliases in shapes.h.")
        for ext_name in sorted(ext_compiled, key=lambda n: ext_compiled[n]):
            out_lines.append(f"#define SHAPE_EXT_{ext_name.upper()} {ext_compiled[ext_name]}")
        out_lines.append("")

    for shape in sorted_shapes:
        out_lines.append(f"// Shape {shape.shape_id}: {shape.name}")

        # Vertices -- negate Y for OpenGL coordinate system
        out_lines.append(f"static const ShapeVertex shape_{shape.shape_id}_verts[] = {{")
        for vi, v in enumerate(shape.vertices):
            neg_y = -v.y if v.y != 0.0 else 0.0  # Negate Y: Star Fox Y-down -> OpenGL Y-up
            vx = v.x if v.x != 0.0 else 0.0      # Avoid -0.0
            vz = v.z if v.z != 0.0 else 0.0
            comma = "," if vi < len(shape.vertices) - 1 else ""
            out_lines.append(f"    {{ {vx:.1f}f, {neg_y:.1f}f, {vz:.1f}f }}{comma}")
        out_lines.append("};")

        # Faces
        out_lines.append(f"static const ShapeFace shape_{shape.shape_id}_faces[] = {{")
        for fi, face in enumerate(shape.faces):
            indices_str = ", ".join(str(idx) for idx in face.vertex_indices)
            # Pad to 12 indices
            pad_count = 12 - len(face.vertex_indices)
            if pad_count > 0:
                indices_str += ", " + ", ".join(["0"] * pad_count)
            comma = "," if fi < len(shape.faces) - 1 else ""
            out_lines.append(
                f"    {{ .vertex_indices = {{{indices_str}}}, "
                f".num_verts = {len(face.vertex_indices)}, "
                f".color_index = {face.color_index} }}{comma}"
            )
        out_lines.append("};")
        out_lines.append("")

    # Shape data table
    out_lines.append("typedef struct {")
    out_lines.append("    uint16 shape_id;")
    out_lines.append("    const ShapeVertex *vertices;")
    out_lines.append("    int num_vertices;")
    out_lines.append("    const ShapeFace *faces;")
    out_lines.append("    int num_faces;")
    out_lines.append("    const char *name;")
    out_lines.append("} ShapeDataEntry;")
    out_lines.append("")

    out_lines.append("static const ShapeDataEntry g_shape_data[] = {")
    for si, shape in enumerate(sorted_shapes):
        comma = "," if si < len(sorted_shapes) - 1 else ""
        verts_arr = f"shape_{shape.shape_id}_verts"
        faces_arr = f"shape_{shape.shape_id}_faces"
        out_lines.append(
            f"    {{ {shape.shape_id}, {verts_arr}, "
            f"sizeof({verts_arr})/sizeof({verts_arr}[0]), "
            f"{faces_arr}, "
            f"sizeof({faces_arr})/sizeof({faces_arr}[0]), "
            f'"{shape.name}" }}{comma}'
        )
    out_lines.append("};")
    out_lines.append("")
    out_lines.append("#define SHAPE_DATA_COUNT (sizeof(g_shape_data) / sizeof(g_shape_data[0]))")
    out_lines.append("")
    out_lines.append("#endif // STARFOX_RENDERER_SHAPE_DATA_H")
    out_lines.append("")

    # Write output
    os.makedirs(os.path.dirname(OUTPUT_PATH), exist_ok=True)
    with open(OUTPUT_PATH, 'w') as f:
        f.write("\n".join(out_lines))

    print(f"  Wrote {OUTPUT_PATH} ({len(sorted_shapes)} shapes)", file=sys.stderr)

    # 6. Generate the Rust mirror (rust/sf-render/src/shape_data.rs).
    emit_rust(sorted_shapes, ext_compiled)
    return 0


if __name__ == "__main__":
    sys.exit(main())
