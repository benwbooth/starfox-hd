#!/usr/bin/env python3
"""
Star Fox HD -- Shape Compiler

Reads the original shape files and shape catalog, then generates the native
Rust mesh table used by the renderer.

Usage:
    python3 tools/shape_compiler.py

Output:
    rust/sf-render/src/shape_data.rs
"""

from __future__ import annotations

import ast
import math
import os
import re
import subprocess
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

# Some public labels are intentionally redefined by alternate/demo shape
# banks. The live player catalog uses PSHAPES; choosing the first textual
# header instead compiled SHAPES6's 36-point demo craft into runtime id 2,
# which is why the legacy renderer had to overwrite it by hand.
PREFERRED_HEADER_FILES = {
    "myship_4": "PSHAPES.ASM",
    "bmyship_4": "PSHAPES.ASM",
}

OUTPUT_PATH = os.path.join(REPO_ROOT, "src/renderer/shape_data.h")
RUST_OUTPUT_PATH = os.path.join(REPO_ROOT, "rust/sf-render/src/shape_data.rs")
RUST_METRICS_OUTPUT_PATH = os.path.join(
    REPO_ROOT, "rust/sf-core/src/sf1_shape_metrics.rs")

# INC files providing global equates referenced by shape headers
# (e.g. bu_scale/bossN_scale shift constants live in STRATEQU.INC).
# Locally-defined symbols in a shape ASM file always take precedence.
INC_SYMBOL_FILES = [
    "reference/ultrastarfox/SF/INC/STRATEQU.INC",
]

# Shape IDs to skip in the output. The source catalog starts with nullshape at
# zero; it deliberately has no geometry.
SKIP_SHAPE_IDS = {
    0,    # nullshape -- no geometry
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
    "zaco_8p":    283,  # szaco2 debris piece (SHAPES2.ASM; not in def_shape)
    # Map-visible meshes referenced directly by 16-bit shape words rather
    # than through ISTRATS.ASM's def_shape table.  The old Rust maps replaced
    # all of these with nullshape, leaving invisible tunnel walls, colony
    # pipes, scenery and credits glyphs even though the source geometry is
    # present and parseable.
    "bou_1":      284,
    "pipe_8_0":   285,
    "pipe_8":     286,
    "bou_1b":     287,
    "paper_1":    288,
    "paper_3":    289,
    "pole_0":     290,
    "slot_0":     291,
    "font_t2":    292,
    "font_h2":    293,
    "font_e2":    294,
    "font_e3":    295,
    "font_n2":    296,
    "font_d2":    297,
    "pilon":      298,
    "mine_0":     299,
    # Great Commander / Transformer child meshes.  The map-visible airship
    # (`boss_f_4`) and its feet (`boss_f_3`) already have def_shape ids 95 and
    # 82; these remaining SHAPES4 components need stable extended slots.
    "boss_f_1":   305,  # head
    "boss_f_2":   306,  # torso
    "boss_f_5":   307,  # transformed torso
    "boss_f_6":   308,  # right arm
    "boss_f_7":   309,  # left arm
    # bossH is placed by a direct strategy address and its component meshes
    # therefore never receive def_shape ids from ISTRATS.ASM.
    "boss_h_0":   300,  # walking boss body (SHAPES.ASM)
    "boss_h_1":   301,  # leg, normal animation range
    "boss_h_1a":  302,  # leg, alternate animation range
    "boss_h_2":   303,  # rotating weapon top
    "boss_h_3":   304,  # teleport effect
    "pipe_9_0":   310,
    "pipe_9":     311,
    "deboss_1":   312,
    # WASHENT.ASM pipe-run pieces. These labels are intentionally commented
    # out of ISTRATS.ASM's def_shape catalog but are complete meshes in
    # SHAPES3.ASM and are referenced directly by the map macros.
    "pipe_0":     313,
    "pipe_1":     314,
    "pipe_2":     315,
    "pipe_3":     316,
    "pipe_4":     317,
    "pipe_5":     318,
    "pipe_6":     319,
    # FINALMAP.ASM's route-specific last-base obstacles. Like the pipe-run
    # pieces above, these complete meshes are addressed directly by the map
    # and therefore never receive an ISTRATS def_shape slot.
    "d_pilar":    320,
    "half_d":     321,
    # Reachable runtime-only meshes.  These are selected directly by strategy
    # code (rather than `def_shape` map rows), so leaving them out forced the
    # Rust port to substitute unrelated extended shapes or nullshape.  Keep
    # this range stable: sf-strat uses the generated SHAPE_EXT_* constants.
    "cockpit":      322,
    "old_type":     323,
    "item_0":       324,
    "r_but_2":      325,
    "walk_4_0":     326,
    "arm":          327,
    "bulge":        328,
    "boss_e_0":     329,
    "boss_e_1":     330,
    "boss_e_1a":    331,
    "boss_e_3":     332,
    "boss_e_4":     333,
    "ringlaser":    334,
    "snake_0":      335,
    "snake_3":      336,
    "snake_4":      337,
    "smark":        338,
    "mmark":        339,
    "lmark":        340,
    "escapee":      341,
    "lfdie":        342,
    "andross":      343,
    "androsscube":  344,
    "face_0_1":     345,
    "face_1":       346,
    "face_box":     347,
    "sface_b":      348,
    "sface2_b":     349,
    "para_1":       350,
    "my_w":         351,
    "my_r_w":       352,
    "my_l_w":       353,
    "my_b_w":       354,
    "up1_man":      355,
    "f_dra_1":      356,
    "fire":         357,
    "smoke":        358,
    "ssplash":      359,
    "splash":       360,
    "pexplod":      361,
    "boostshape":   362,
    "firebreath":   363,
    "lsmoke":       364,
    "folsmoke":     365,
    "androsshole":  366,
    "spexplod":     367,
    # Complete retail player-shape table plus the scrape spark. The intact
    # Arwing remains def_shape id 2; these variants are selected at runtime.
    "myship_r":     368,
    "myship_l":     369,
    "myship_b":     370,
    "my_up":        371,
    "bmyship_4":    372,
    "bmyship_r":    373,
    "bmyship_l":    374,
    "bmyship_b":    375,
    "myzoom_4":     376,
    "myzoom_r":     377,
    "myzoom_l":     378,
    "myzoom_b":     379,
    "line":         380,
    # Remaining reachable combat/cutscene meshes that were still represented
    # by nullshape or by unrelated ids in the Rust strategy ports.
    "boss_d_0":     381,
    "boss_d_2":     382,
    "neck":         383,
    "grabber":      384,
    "grabber2":     385,
    "egg":          386,
    "boss_d_8":     387,
    "boss_d_9":     388,
    "boss_d_6":     389,
    "boss_d_7":     390,
    "boss_9_0":     391,
    "barrier":      392,
    "fireface_b":   393,
    "boss_a_3":     394,
    "boss_a_4":     395,
    "boss_a_5":     396,
    "boss_b_l":     397,
    "boss_b_r":     398,
    "boss_b_h":     399,
    "round0p":      400,
    "ripair_w":     401,
    "fireball":     402,
    "missile":      403,
    "ironball":     404,
    "bouncyball":   405,
    "shelpball":    406,
    "nuke":         407,
    "hyper":        408,
    "hou_3":        409,
    "my_demobs":    410,
    "my_demos":     411,
    "big_m":        412,
    "boss_f_b":     413,
    "walker_r":     414,
    "playerbeam":   415,
    "ovalbeam":     416,
    "c_miss":       417,
    # Black Hole's shape-shuffling object selects these two meshes directly.
    # Neither is part of the map shape catalog, so they need stable native
    # slots alongside the other runtime-only meshes.
    "zaco_0":       418,
    "zaco_7p":      419,
    "robot_0":      420,
    # Great Commander animated hull pieces. The renderer retains hand-tuned
    # frame overlays for these ids, but compiling their complete source face
    # programs here supplies exact visibility metadata instead of treating
    # every polygon as two-sided.
    "boss_7_0":     421,
    "boss_7_1o":    422,
    "boss_7_2":     423,
    "boss_7_3":     424,
    "boss_7_4":     425,
    # Player laser. Keep its complete nine-frame source vertex stream and
    # exact per-face visibility triangles in the typed runtime catalog.
    "elaser2":      511,
    "boss_a_6":     426,
    "boss_f_8":     427,
    "boss_f_9":     428,
    "boss_f_8a":    429,
    "boss_f_9a":    430,
    "face_0":       431,
    # Directly selected boss/runtime meshes that were previously assigned
    # unrelated catalog ids in the Rust port.  These labels are real source
    # ShapeHdrs but do not have def_shape rows, so reserve stable native slots.
    "boss_0_0":     432,
    "boss_0_0a":    433,
    "boss_0_2":     434,
    "boss_0_3":     435,
    "boss_1_0":     436,
    "boss_1_1":     437,
    "amoeba1":      438,
    "rpillar3":     439,
    # Intro Great Commander side shells. The center shell (`deboss_1`) is a
    # map-visible extended shape at 312; these two are native child objects
    # created by boss7intro_Istrat and need their own stable catalog slots.
    "deboss_0":      440,
    "deboss_2":      441,
    # Path-only actors selected by assembled PATHDATA operands. These are
    # complete ShapeHdr meshes, but neither has an ISTRATS def_shape row.
    "flower":         442,
    "big_bird":       443,
    "leaf":            444,
    "walk_4_l":        445,
    "walk_4_r":        446,
    "tow_1":           447,
    "slot_1":          448,
    "slot_2":          449,
    "slot_3":          450,
    "slot_4":          451,
    "pillar3_ns":      452,
    "laserline":       453,
    # GA2STRAT's warp fighter materializes through these three intermediate
    # ShapeHdrs before reaching the catalog `warp` mesh (id 133).
    "warp_1":          454,
    "warp_2":          455,
    "warp_3":          456,
    "wall_l":          457,
    "wall_r":          458,
    "iris_1":          459,
    # Sprouting-tree body mesh selected after each animated `stalk` head
    # finishes growing. It is a complete source ShapeHdr but has no
    # ISTRATS `def_shape` row of its own.
    "stalk_1":         460,
    # Runtime explosion meshes selected by EXPSTRAT.ASM from the destroyed
    # object's ShapeHdr size. The first four are scaled texture quads; the
    # final three are the polygon-debris bodies.
    "explosion":       461,
    "explosion2":      462,
    "explosion3":      463,
    "explosion4":      464,
    "expl_4":          465,
    "expl_6":          466,
    "expl_8":          467,
    # Andross's walking kick pose and detached foot are selected directly by
    # GB3STRAT rather than through map def_shape rows.
    "boss_b_6":        468,
    "boss_b_7":        469,
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
    "pb", "pw", "pbd2", "pby2", "pipe8pb", "pipe9pb", "pipepb", "tbpb",
    "mlaser",
    "frames", "jumptab", "jump",
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
    # Authored lighting/explosion normal from the Face record. Components are
    # signed source bytes; the Rust emitter converts Y into GL-up coordinates.
    normal: Tuple[int, int, int]
    # Resolved `vizis` triangle used by the source hidden-surface test.
    # `None` is the source selector -1: draw from either side (wire lines and
    # deliberately two-sided polygons). Keeping the resolved indices avoids
    # recreating the shape bytecode/BSP interpreter in the shipping renderer.
    visibility_vertices: Optional[Tuple[int, int, int]]


@dataclass
class ShapeHeader:
    label: str              # shape name (from the label on the shapehdr line)
    points_label: str       # label for vertex data
    faces_label: str        # label for face data
    shift: int              # coordinate shift
    visual_extent: int      # assembled ShapeHdr sh_size
    color_table: str        # color table name


@dataclass
class ShapeData:
    shape_id: int
    name: str
    vertices: List[Vertex]
    # Complete vertex streams selected by the Shape `Frames` bytecode for
    # each animation frame. Static shapes leave this empty; animated shapes
    # include frame zero as the first entry.
    animation_frames: List[List[Vertex]]
    faces: List[Face]
    # ShapeHdr sh_col_ptr.  Zero in a live object's al_coltab means "use
    # this header table"; retaining it is required for sprite-textured and
    # per-shape animated materials (asteroids, explosions, lasers, etc.).
    color_table: str
    visual_extent: int
    coordinate_shift: int


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
            # Match catalog entries, but not the `def_shape MACRO`
            # declaration. The source counter is initialized to zero and the
            # declaration does not invoke the macro; nullshape is therefore
            # exactly id 0, exitlight id 1, and myship_4 id 2.
            m = re.match(r'^def_shape\s+(\w+)', line, re.IGNORECASE)
            if m:
                name = m.group(1)
                if name.lower() == "macro":
                    continue
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
# Parse vertices and animation-frame jump tables
# ---------------------------------------------------------------------------

def vertex_section_bounds(af: AsmFile, points_label: str) -> Tuple[int, int]:
    """Return the inclusive points-language range, or ``(-1, -1)``."""
    start = find_label_line(af, points_label)
    if start < 0:
        return -1, -1

    for i in range(start, len(af.lines)):
        if af.lines[i].op == 'endpoints':
            return start, i
    return -1, -1


def animation_period(af: AsmFile, points_label: str) -> int:
    """Return the complete period of all ``Frames`` tables in a shape.

    The GSU applies the same object animation counter independently at every
    Frames opcode and wraps it by that table's row count. Their least common
    multiple therefore describes the complete vertex-stream period.
    """
    start, end = vertex_section_bounds(af, points_label)
    if start < 0:
        return 0
    period = 1
    animated = False
    for line_index, line in enumerate(af.lines[start:end + 1], start):
        if line.op != 'frames':
            continue
        declared_count = eval_in_file(af, line.args)
        if declared_count is None or declared_count <= 0:
            continue
        row_count = 0
        while (
                line_index + 1 + row_count <= end
                and af.lines[line_index + 1 + row_count].op == 'jumptab'
        ):
            row_count += 1
        # paper_1 declares Frames 32 but contains 20 rows, and every live path
        # explicitly clamps its counter to 0..19. Use the concrete typed rows
        # rather than compiling the twelve following point records as offsets.
        count = row_count if row_count > 0 else declared_count
        animated = True
        period = math.lcm(period, count)
    return period if animated else 0


def parse_vertices(
        af: AsmFile,
        points_label: str,
        shift: int,
        animation_frame: int = 0,
) -> List[Vertex]:
    """Parse one complete vertex stream starting from ``points_label``."""
    start, end = vertex_section_bounds(af, points_label)
    if start < 0:
        return []

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

        if line.op in ('pb', 'pw', 'pbd2', 'pby2', 'pipe8pb', 'pipe9pb', 'pipepb', 'tbpb'):
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
                    elif line.op in ('pipe8pb', 'pipe9pb'):
                        # SHAPES2/5 local macro: pb x/8,y/8,z/8.  The
                        # matching ShapeHdr has shift=3, so the final mesh
                        # returns to the authored coordinates.
                        x //= 8
                        y //= 8
                        z //= 8
                    elif line.op == 'pipepb':
                        # SHAPES3 pipe macro.
                        y *= 2
                        z = (z * 125) // 100
                    elif line.op == 'tbpb':
                        # SHAPES.ASM t_bool helper.
                        x = (x * 125) // 100
                        y = (y * 125) // 100
                        z = (z * 125) // 100

                    vertices.append(Vertex(float(x), float(y), float(z)))
                    if pending_mirror:
                        vertices.append(Vertex(float(-x), float(y), float(z)))

                pending_count -= 1
            pc += 1
            continue

        if line.op == 'mlaser':
            # USHAPES player-laser point macro. Each selected animation row
            # expands to a complete six-point stream and its own EndPoints.
            args = split_asm_args(line.args)
            values = [eval_in_file(af, arg) for arg in args]
            if len(values) != 4 or any(value is None for value in values):
                raise ValueError(
                    f"invalid mlaser row at {af.path}:{pc + 1}")
            nose, middle, tail, radius = values
            vertices.extend([
                Vertex(0.0, 0.0, float(tail)),
                Vertex(float(-radius), 0.0, float(middle)),
                Vertex(0.0, 0.0, float(nose)),
                Vertex(float(radius), 0.0, float(middle)),
                Vertex(0.0, -2.0, float(middle)),
                Vertex(0.0, 2.0, float(middle)),
            ])
            break

        if line.op == 'frames':
            # The GSU repeatedly subtracts the table size, i.e. selects the
            # object animation counter modulo this Frames table's row count.
            table: List[AsmLine] = []
            scan = pc + 1
            while scan <= end and af.lines[scan].op == 'jumptab':
                table.append(af.lines[scan])
                scan += 1
            if not table:
                pc += 1
                continue
            frame_count = len(table)
            selected = animation_frame % frame_count
            jump_target = table[selected].args.strip().strip('<>').strip()
            target_line = find_label_line_range(af, jump_target, start, end)
            pc = target_line if target_line >= 0 else pc + 1
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


def parse_vertex_frames(
        af: AsmFile,
        points_label: str,
        shift: int,
) -> List[List[Vertex]]:
    """Parse every distinct animation frame, or one stream for static data."""
    period = animation_period(af, points_label)
    count = period if period > 0 else 1
    return [
        parse_vertices(af, points_label, shift, frame)
        for frame in range(count)
    ]


def validate_shape_geometry(
        name: str,
        vertex_frames: List[List[Vertex]],
        faces: List[Face],
) -> None:
    """Reject a typed mesh whose faces cannot address every animation frame."""
    if not vertex_frames:
        raise ValueError(f"shape {name} has no vertex frames")
    referenced_indices = [
        index
        for face in faces
        for index in (
            face.vertex_indices
            + (list(face.visibility_vertices)
               if face.visibility_vertices is not None else [])
        )
    ]
    if not referenced_indices:
        return
    max_index = max(referenced_indices)
    for frame_index, frame in enumerate(vertex_frames):
        if max_index >= len(frame):
            raise ValueError(
                f"shape {name} frame {frame_index} has {len(frame)} vertices "
                f"but its faces reference vertex {max_index}")


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
    visibility_tests: List[Tuple[int, int, int]] = []
    visibility_tests_remaining = 0

    for i in range(start, len(af.lines)):
        line = af.lines[i]

        if line.op == 'endshape':
            break

        if line.op == 'vizis':
            count = eval_in_file(af, line.args)
            if count is None or count < 0:
                raise ValueError(
                    f"invalid visibility-test count at {af.path}:{i + 1}")
            visibility_tests = []
            visibility_tests_remaining = count
            continue

        if line.op == 'viz' and visibility_tests_remaining > 0:
            args = split_asm_args(line.args)
            if len(args) < 3:
                raise ValueError(
                    f"truncated visibility test at {af.path}:{i + 1}")
            indices = tuple(eval_in_file(af, arg) for arg in args[:3])
            if any(index is None for index in indices):
                raise ValueError(
                    f"unresolved visibility test at {af.path}:{i + 1}")
            visibility_tests.append(indices)  # type: ignore[arg-type]
            visibility_tests_remaining -= 1
            continue

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

            normal_values = [eval_in_file(af, arg) for arg in args[2:5]]
            if any(value is None for value in normal_values):
                raise ValueError(
                    f"unresolved face normal at {af.path}:{i + 1}")
            # Face records store each component as one signed byte.
            normal = tuple(
                ((int(value) & 0xFF) - 256
                 if (int(value) & 0xFF) >= 128 else (int(value) & 0xFF))
                for value in normal_values
            )

            visibility_val = eval_in_file(af, args[1])
            if visibility_val is None:
                raise ValueError(
                    f"unresolved face visibility at {af.path}:{i + 1}")
            if visibility_val == -1:
                visibility_vertices = None
            elif 0 <= visibility_val < len(visibility_tests):
                visibility_vertices = visibility_tests[visibility_val]
            else:
                raise ValueError(
                    f"face visibility {visibility_val} outside the "
                    f"{len(visibility_tests)} tests at {af.path}:{i + 1}")

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

            faces.append(Face(
                vertex_indices=vindices,
                color_index=color_index,
                normal=normal,  # type: ignore[arg-type]
                visibility_vertices=visibility_vertices,
            ))

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

        # SHMACS.INC assembles `sh_size` as the semantic size operand shifted
        # by the header coordinate scale. Preserve that finished domain value
        # without carrying a ShapeHdr address into the Rust port.
        size_val = eval_in_file(af, args[12])
        unshifted_size = size_val if size_val is not None else 0
        visual_extent = (unshifted_size << shift) & 0xFFFF

        # Color table name (args[13]) -- strip angle brackets if present
        # But first strip any trailing <display_name> from the entire args
        color_table = args[13].strip() if len(args) > 13 else "0"

        for lbl in shape_labels:
            headers.append(ShapeHeader(
                label=lbl,
                points_label=points_label,
                faces_label=faces_label,
                shift=shift,
                visual_extent=visual_extent,
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
    out.append("    /// Authored face normal in the shared GL-up coordinate model.")
    out.append("    pub normal: [i16; 3],")
    out.append("    /// Source `vizis` triangle; `None` means deliberately two-sided.")
    out.append("    pub visibility_vertices: Option<[u16; 3]>,")
    out.append("}")
    out.append("")
    out.append("/// One compiled shape, matching C `ShapeDataEntry` (shape_data.h).")
    out.append("#[derive(Debug, Clone, Copy)]")
    out.append("pub struct ShapeDataEntry {")
    out.append("    pub shape_id: u16,")
    out.append("    pub vertices: &'static [ShapeVertex],")
    out.append("    pub animation_frames: &'static [&'static [ShapeVertex]],")
    out.append("    pub faces: &'static [ShapeFace],")
    out.append("    pub default_color_table: &'static str,")
    out.append("    pub name: &'static str,")
    out.append("}")
    out.append("")
    out.append("const fn v(x: f32, y: f32, z: f32) -> ShapeVertex {")
    out.append("    ShapeVertex { x, y, z }")
    out.append("}")
    out.append("")
    out.append("const fn f(")
    out.append("    vertex_indices: [u16; 12],")
    out.append("    num_verts: u8,")
    out.append("    color_index: u8,")
    out.append("    normal: [i16; 3],")
    out.append("    visibility_vertices: Option<[u16; 3]>,")
    out.append(") -> ShapeFace {")
    out.append("    ShapeFace { vertex_indices, num_verts, color_index, normal, visibility_vertices }")
    out.append("}")
    out.append("")

    for shape in sorted_shapes:
        out.append(f"// Shape {shape.shape_id}: {shape.name}")

        # Vertices -- same Y-negation / -0.0 avoidance as the C emitter.
        # Animated entries contain complete streams, not sparse overlays, so
        # the renderer can select a typed frame without recreating the source
        # shape-language VM.
        frames = shape.animation_frames or [shape.vertices]
        for frame_index, vertices in enumerate(frames):
            suffix = "" if frame_index == 0 else f"_FRAME_{frame_index}"
            out.append(
                f"static SHAPE_{shape.shape_id}{suffix}_VERTS: "
                f"[ShapeVertex; {len(vertices)}] = [")
            for vert in vertices:
                neg_y = -vert.y if vert.y != 0.0 else 0.0
                vx = vert.x if vert.x != 0.0 else 0.0
                vz = vert.z if vert.z != 0.0 else 0.0
                out.append(f"    v({vx:.1f}, {neg_y:.1f}, {vz:.1f}),")
            out.append("];")
        if shape.animation_frames:
            out.append(
                f"static SHAPE_{shape.shape_id}_ANIMATION_FRAMES: "
                f"[&[ShapeVertex]; {len(shape.animation_frames)}] = [")
            for frame_index in range(len(shape.animation_frames)):
                suffix = "" if frame_index == 0 else f"_FRAME_{frame_index}"
                out.append(f"    &SHAPE_{shape.shape_id}{suffix}_VERTS,")
            out.append("];")

        # Faces -- indices padded to 12, same as the C emitter.
        out.append(
            f"static SHAPE_{shape.shape_id}_FACES: "
            f"[ShapeFace; {len(shape.faces)}] = [")
        for face in shape.faces:
            padded = list(face.vertex_indices) + [0] * (12 - len(face.vertex_indices))
            indices_str = ", ".join(str(idx) for idx in padded)
            visibility = (
                f"Some([{', '.join(str(idx) for idx in face.visibility_vertices)}])"
                if face.visibility_vertices is not None else "None"
            )
            normal = [face.normal[0], -face.normal[1], face.normal[2]]
            normal_str = ", ".join(str(component) for component in normal)
            out.append(
                f"    f([{indices_str}], {len(face.vertex_indices)}, "
                f"{face.color_index}, [{normal_str}], {visibility}),")
        out.append("];")
        out.append("")

    out.append(f"pub const SHAPE_DATA_COUNT: usize = {len(sorted_shapes)};")
    out.append("")
    out.append("pub static SHAPE_DATA: [ShapeDataEntry; SHAPE_DATA_COUNT] = [")
    for shape in sorted_shapes:
        animation_frames = (
            f"&SHAPE_{shape.shape_id}_ANIMATION_FRAMES"
            if shape.animation_frames else "&[]")
        out.append(
            f"    ShapeDataEntry {{ shape_id: {shape.shape_id}, "
            f"vertices: &SHAPE_{shape.shape_id}_VERTS, "
            f"animation_frames: {animation_frames}, "
            f"faces: &SHAPE_{shape.shape_id}_FACES, "
            f'default_color_table: "{shape.color_table.lower()}", '
            f'name: "{shape.name}" }},')
    out.append("];")
    out.append("")

    os.makedirs(os.path.dirname(RUST_OUTPUT_PATH), exist_ok=True)
    with open(RUST_OUTPUT_PATH, 'w') as f:
        f.write("\n".join(out))
    subprocess.run(
        ["rustfmt", "--edition", "2021", RUST_OUTPUT_PATH],
        check=True,
    )

    print(f"  Wrote {RUST_OUTPUT_PATH} ({len(sorted_shapes)} shapes)",
          file=sys.stderr)


def emit_rust_metrics(sorted_shapes: List[ShapeData]) -> None:
    """Emit gameplay-facing ShapeHdr metrics without source addresses."""
    out: List[str] = []
    out.append("// Auto-generated by tools/shape_compiler.py -- do not edit.")
    out.append("//! Flat semantic ShapeHdr metrics used by SF1 gameplay.")
    out.append("")
    out.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
    out.append("pub struct Sf1ShapeMetrics {")
    out.append("    pub visual_extent: u16,")
    out.append("    pub coordinate_shift: u8,")
    out.append("}")
    out.append("")
    out.append("pub const SF1_SHAPE_METRICS: &[(u16, Sf1ShapeMetrics)] = &[")
    out.append("    (0, Sf1ShapeMetrics { visual_extent: 0, coordinate_shift: 0 }), // nullshape")
    for shape in sorted_shapes:
        out.append(
            "    ("
            f"{shape.shape_id}, Sf1ShapeMetrics {{ visual_extent: "
            f"{shape.visual_extent}, coordinate_shift: "
            f"{shape.coordinate_shift} }}), // {shape.name}")
    out.append("];")
    out.append("")
    out.append("pub fn sf1_shape_metrics(shape_id: u16) -> Option<Sf1ShapeMetrics> {")
    out.append("    SF1_SHAPE_METRICS")
    out.append("        .binary_search_by_key(&shape_id, |&(id, _)| id)")
    out.append("        .ok()")
    out.append("        .map(|index| SF1_SHAPE_METRICS[index].1)")
    out.append("}")
    out.append("")

    os.makedirs(os.path.dirname(RUST_METRICS_OUTPUT_PATH), exist_ok=True)
    with open(RUST_METRICS_OUTPUT_PATH, 'w') as f:
        f.write("\n".join(out))
    subprocess.run(
        ["rustfmt", "--edition", "2021", RUST_METRICS_OUTPUT_PATH],
        check=True,
    )
    print(
        f"  Wrote {RUST_METRICS_OUTPUT_PATH} ({len(sorted_shapes)} shapes)",
        file=sys.stderr,
    )


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
        preferred_file = PREFERRED_HEADER_FILES.get(hdr.label)
        if preferred_file is not None and os.path.basename(af.path) != preferred_file:
            continue
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
        vertex_frames = parse_vertex_frames(af, hdr.points_label, hdr.shift)
        vertices = vertex_frames[0] if vertex_frames else []
        faces = parse_faces(af, hdr.faces_label)

        if not vertices:
            for other_af in asm_files:
                if other_af is not af:
                    vertex_frames = parse_vertex_frames(
                        other_af, hdr.points_label, hdr.shift)
                    vertices = vertex_frames[0] if vertex_frames else []
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

        if any(len(frame) != len(vertices) for frame in vertex_frames):
            lengths = sorted({len(frame) for frame in vertex_frames})
            raise ValueError(
                f"animated shape {hdr.label} has inconsistent vertex counts: "
                f"{lengths}")
        validate_shape_geometry(hdr.label, vertex_frames, faces)

        compiled_shapes[shape_id] = ShapeData(
            shape_id=shape_id,
            name=hdr.label,
            vertices=vertices,
            animation_frames=vertex_frames if len(vertex_frames) > 1 else [],
            faces=faces,
            color_table=hdr.color_table,
            visual_extent=hdr.visual_extent,
            coordinate_shift=hdr.shift,
        )
        processed_geometry.add(geom_key)

    print(f"  Compiled {len(compiled_shapes)} shapes with geometry", file=sys.stderr)

    # 4b. Extended-bank pass: shapes outside the ISTRATS def_shape catalog,
    # compiled into the fixed runtime slots defined by EXTENDED_SHAPES.
    ext_compiled: Dict[str, int] = {}  # name -> id (for macro emission)
    for hdr, af in all_headers:
        preferred_file = PREFERRED_HEADER_FILES.get(hdr.label)
        if preferred_file is not None and os.path.basename(af.path) != preferred_file:
            continue
        ext_id = EXTENDED_SHAPES.get(hdr.label)
        if ext_id is None:
            continue
        if ext_id in compiled_shapes:
            continue  # already have this slot
        if hdr.points_label == '0' or hdr.faces_label == '0':
            continue  # header-only shape (no geometry)

        vertex_frames = parse_vertex_frames(af, hdr.points_label, hdr.shift)
        vertices = vertex_frames[0] if vertex_frames else []
        faces = parse_faces(af, hdr.faces_label)

        if not vertices:
            for other_af in asm_files:
                if other_af is not af:
                    vertex_frames = parse_vertex_frames(
                        other_af, hdr.points_label, hdr.shift)
                    vertices = vertex_frames[0] if vertex_frames else []
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

        if any(len(frame) != len(vertices) for frame in vertex_frames):
            lengths = sorted({len(frame) for frame in vertex_frames})
            raise ValueError(
                f"animated shape {hdr.label} has inconsistent vertex counts: "
                f"{lengths}")
        validate_shape_geometry(hdr.label, vertex_frames, faces)

        compiled_shapes[ext_id] = ShapeData(
            shape_id=ext_id,
            name=hdr.label,
            vertices=vertices,
            animation_frames=vertex_frames if len(vertex_frames) > 1 else [],
            faces=faces,
            color_table=hdr.color_table,
            visual_extent=hdr.visual_extent,
            coordinate_shift=hdr.shift,
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

    # C header path is legacy — the C/C++ tree was removed; Rust is the
    # sole shape-data consumer. Keep emit for local experiments only when
    # the destination directory already exists.
    out_dir = os.path.dirname(OUTPUT_PATH)
    if os.path.isdir(out_dir):
        with open(OUTPUT_PATH, 'w') as f:
            f.write("\n".join(out_lines))
        print(f"  Wrote {OUTPUT_PATH} ({len(sorted_shapes)} shapes)", file=sys.stderr)
    else:
        print(f"  Skipped C header ({out_dir} absent; Rust-only build)", file=sys.stderr)

    # 6. Generate the Rust mirror (rust/sf-render/src/shape_data.rs).
    emit_rust(sorted_shapes, ext_compiled)
    emit_rust_metrics(sorted_shapes)
    return 0


if __name__ == "__main__":
    sys.exit(main())
