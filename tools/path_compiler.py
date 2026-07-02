#!/usr/bin/env python3
"""
Compile SF PATHDATA ASM into a flat C bytecode blob + path offset table.

This is intentionally strict about opcode IDs (taken from PATHS.ASM mode_table),
but partial about macro coverage: unsupported macros are skipped with diagnostics.
"""

from __future__ import annotations

import argparse
import ast
import os
import re
import sys
from dataclasses import dataclass
from typing import Dict, List, Tuple


@dataclass
class Fixup:
    pos: int
    label: str
    source: str


class ExprEval(ast.NodeVisitor):
    def __init__(self, symbols: Dict[str, int]):
        self.symbols = symbols

    def visit_Expression(self, node):
        return self.visit(node.body)

    def visit_BinOp(self, node):
        l = self.visit(node.left)
        r = self.visit(node.right)
        if isinstance(node.op, ast.Add):
            return l + r
        if isinstance(node.op, ast.Sub):
            return l - r
        if isinstance(node.op, ast.Mult):
            return l * r
        if isinstance(node.op, (ast.Div, ast.FloorDiv)):
            return 0 if r == 0 else (l // r)
        if isinstance(node.op, ast.Mod):
            return 0 if r == 0 else (l % r)
        if isinstance(node.op, ast.BitAnd):
            return l & r
        if isinstance(node.op, ast.BitOr):
            return l | r
        if isinstance(node.op, ast.BitXor):
            return l ^ r
        if isinstance(node.op, ast.LShift):
            return l << r
        if isinstance(node.op, ast.RShift):
            return l >> r
        raise ValueError("unsupported binop")

    def visit_UnaryOp(self, node):
        v = self.visit(node.operand)
        if isinstance(node.op, ast.UAdd):
            return +v
        if isinstance(node.op, ast.USub):
            return -v
        if isinstance(node.op, ast.Invert):
            return ~v
        raise ValueError("unsupported unary op")

    def visit_Constant(self, node):
        if isinstance(node.value, int):
            return node.value
        raise ValueError("unsupported constant")

    def visit_Name(self, node):
        return int(self.symbols.get(node.id.lower(), 0))

    def generic_visit(self, node):
        raise ValueError(f"unsupported node {type(node).__name__}")


class PathCompiler:
    def __init__(self, repo_root: str, strict: bool = False):
        self.repo_root = repo_root
        self.strict = strict
        self.diagnostics: List[str] = []
        self.symbols: Dict[str, int] = {
            "on": 1,
            "off": 0,
            "deg0": 0,
            "deg22": 16,
            "deg45": 32,
            "deg90": 64,
            "deg180": 128,
            "friend_fox": 0,
            "friend_rabbit": 1,
            "friend_bunny": 1,
            "friend_falcon": 2,
            "friend_cock": 2,
            "friend_frog": 3,
            "friend_anyone": 6,
        }
        self.opcodes = self._load_mode_table()
        self.shape_ids = self._load_shape_ids()
        self._load_equ_symbols()
        for k, v in self.shape_ids.items():
            self.symbols.setdefault(k, v)
        self.labels: Dict[str, int] = {}
        self.fixups: List[Fixup] = []
        self.path_names: List[str] = []
        self.path_offsets: List[int] = []
        self.data = bytearray()

        # SNES al_ offsets (base) and alx offsets (encoded with bit7 set).
        self.base_byte = {
            "flags": 8, "type": 9, "count": 10, "count1": 11,
            "rotx": 18, "roty": 19, "rotz": 20, "vel": 21,
            "sflags": 29, "sflags2": 30, "sflags3": 31, "sflags4": 32,
            "skidy": 33, "sbyte1": 34, "sbyte2": 35, "sbyte3": 36, "sbyte4": 37,
            "hp": 42, "ap": 43,
        }
        self.base_word = {
            "shape": 4, "ptr": 6, "worldx": 12, "worldy": 14, "worldz": 16,
            "immuneptr": 25, "collobjptr": 27, "sword1": 38, "sword2": 40,
        }
        self.alx_byte = {"pbyte1": 50, "pbyte2": 51}
        self.alx_word = {"swpx1": 0, "swpy1": 2, "swpz1": 4, "depthoffset": 21, "pword1": 52}

    def _diag(self, msg: str):
        self.diagnostics.append(msg)

    def _load_mode_table(self) -> Dict[str, int]:
        out: Dict[str, int] = {}
        path = os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "PATH", "PATHS.ASM")
        inside = False
        idx = 0
        with open(path, "r", encoding="latin1", errors="ignore") as f:
            for raw in f:
                line = raw.split(";", 1)[0].strip().lower()
                if not line:
                    continue
                if line.startswith("s_mode_table"):
                    inside = True
                    continue
                if inside and line.startswith("s_mode_table_end"):
                    break
                if not inside:
                    continue
                m = re.match(r"^s_mode_entry\s+[^,]+,\s*([a-z0-9_]+)", line)
                if not m:
                    continue
                out[m.group(1)] = idx
                idx += 1
        return out

    def _load_shape_ids(self) -> Dict[str, int]:
        out: Dict[str, int] = {}
        path = os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "STRAT", "ISTRATS.ASM")
        idx = 0
        with open(path, "r", encoding="latin1", errors="ignore") as f:
            for raw in f:
                line = raw.split(";", 1)[0].strip().lower()
                m = re.match(r"^def_shape\s+([a-z0-9_]+)", line)
                if not m:
                    continue
                out[m.group(1)] = idx
                idx += 1
        return out

    def _load_equ_symbols(self):
        files = [
            os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "INC", "VARS.INC"),
            os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "INC", "STRATEQU.INC"),
            os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "INC", "SOUNDEQU.INC"),
        ]
        for path in files:
            if not os.path.exists(path):
                continue
            with open(path, "r", encoding="latin1", errors="ignore") as f:
                for raw in f:
                    line = raw.split(";", 1)[0].strip()
                    if not line:
                        continue
                    m = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)\s*(?:equ|=)\s*(.+)$", line, re.IGNORECASE)
                    if not m:
                        continue
                    name = m.group(1).lower()
                    val = self.eval_expr(m.group(2), quiet=True)
                    self.symbols[name] = val

    def eval_expr(self, expr: str, quiet: bool = False) -> int:
        text = expr.strip()
        if not text:
            return 0
        text = re.sub(r"\$([0-9A-Fa-f]+)", lambda m: f"0x{m.group(1)}", text)
        text = text.replace("!", "|")
        text = text.replace("&WM", "")
        text = re.sub(r"\bpath_([a-zA-Z0-9_]+)\b", r"\1", text)
        try:
            node = ast.parse(text, mode="eval")
            return int(ExprEval(self.symbols).visit(node))
        except Exception:
            if not quiet:
                self._diag(f"expr unresolved '{expr}' -> 0")
            return 0

    def split_args(self, s: str) -> List[str]:
        out: List[str] = []
        cur = []
        depth = 0
        for ch in s:
            if ch == "," and depth == 0:
                out.append("".join(cur).strip())
                cur = []
                continue
            if ch == "(":
                depth += 1
            elif ch == ")" and depth > 0:
                depth -= 1
            cur.append(ch)
        tail = "".join(cur).strip()
        if tail:
            out.append(tail)
        return out

    def op(self, name: str) -> int:
        key = name.lower()
        if key not in self.opcodes:
            self._diag(f"missing opcode symbol '{name}'")
            return 0
        return self.opcodes[key]

    def emit8(self, v: int):
        self.data.append(v & 0xFF)

    def emit16(self, v: int):
        self.data.append(v & 0xFF)
        self.data.append((v >> 8) & 0xFF)

    def emit16_fix(self, label: str, source: str):
        pos = len(self.data)
        self.emit16(0)
        self.fixups.append(Fixup(pos, label, source))

    def resolve_label(self, tok: str, cur_path: str) -> str:
        t = self.normalize_label_token(tok).strip().lower()
        if t.startswith("."):
            return f"{cur_path}{t}"
        if "." in t:
            head, tail = t.split(".", 1)
            # Common source form uses p<path>.label for locals in path <path>.
            if head.startswith("p"):
                return f"{head[1:]}.{tail}"
        return t

    def normalize_label_token(self, tok: str) -> str:
        # Some sources pass split path/local pieces separated by whitespace:
        # e.g. "endin .wait" should become "endin.wait".
        parts = tok.strip().split()
        if len(parts) >= 2 and parts[1].startswith("."):
            return f"{parts[0]}.{parts[1][1:]}"
        return tok

    def parse_var(self, tok: str) -> Tuple[int, bool]:
        t = tok.strip().lower()
        m = re.match(r"^([a-z_][a-z0-9_]*)([+-]\d+)?$", t)
        base = t
        delta = 0
        if m:
            base = m.group(1)
            if m.group(2):
                delta = int(m.group(2), 10)
        if base in self.base_byte:
            return ((self.base_byte[base] + delta) & 0xFF, False)
        if base in self.base_word:
            return ((self.base_word[base] + delta) & 0xFF, True)
        if base in self.alx_byte:
            return ((0x80 | ((self.alx_byte[base] + delta) & 0x7F)), False)
        if base in self.alx_word:
            return ((0x80 | ((self.alx_word[base] + delta) & 0x7F)), True)
        self._diag(f"unknown path var '{tok}', using sbyte1")
        return (34, False)

    def _onoff(self, tok: str) -> bool:
        return self.eval_expr(tok, quiet=True) != 0 or tok.strip().lower() in ("on", "yes", "true")

    def _emit_cond_jump(self, opcode: int, cur_path: str, args: List[str], size: int):
        if len(args) < size:
            self._diag("conditional jump missing args")
            return
        self.emit8(opcode)
        if size == 2:
            label = self.resolve_label(args[1], cur_path)
            self.emit16_fix(label, "cond")
        else:
            label = self.resolve_label(args[2], cur_path)
            self.emit16_fix(label, "cond")

    def compile_file(self, path: str):
        cur_path = ""
        with open(path, "r", encoding="latin1", errors="ignore") as f:
            for lineno, raw in enumerate(f, start=1):
                src = f"{os.path.basename(path)}:{lineno}"
                line = raw.split(";", 1)[0].strip().replace("\x1a", "")
                if not line:
                    continue

                m_start = re.match(r"(?i)^start_path\s+([a-z0-9_]+)$", line)
                if m_start:
                    name = m_start.group(1).lower()
                    cur_path = name
                    if name not in self.labels:
                        self.path_names.append(name)
                        self.path_offsets.append(len(self.data))
                    self.labels[name] = len(self.data)
                    self.labels[f"p{name}"] = len(self.data)
                    self.symbols[f"path_{name}"] = len(self.data)
                    continue

                if not cur_path:
                    continue

                m_local = re.match(
                    r"^(\.[A-Za-z0-9_]+)(?:\s+LOCAL)?(?:\s+(.*))?$",
                    line,
                    re.IGNORECASE,
                )
                if m_local:
                    self.labels[f"{cur_path}{m_local.group(1).lower()}"] = len(self.data)
                    rest = (m_local.group(2) or "").strip()
                    if not rest:
                        continue
                    line = rest

                m_global = re.match(
                    r"^([A-Za-z_][A-Za-z0-9_\.]*)(?:\s+LOCAL)?(?:\s+(.*))?$",
                    line,
                )
                if m_global:
                    key = m_global.group(1).lower()
                    rest = (m_global.group(2) or "").strip()
                    if (
                        not key.startswith("p_")
                        and not key.upper().startswith("P_")
                        and not key.startswith("start_path")
                        and not key.startswith("if")
                        and key not in ("db", "dw", "defs", "public", "extern", "run", "printf", "fopen", "fclose")
                        and not rest.startswith("=")
                        and not rest.lower().startswith("equ ")
                    ):
                        self.labels[key] = len(self.data)
                        if not rest:
                            continue
                        line = rest

                m_op = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)\s*(.*)$", line)
                if not m_op:
                    continue
                op = m_op.group(1).upper()
                if not op.startswith("P_"):
                    continue
                args = self.split_args(m_op.group(2))

                try:
                    if op == "P_RELTOPLAYER":
                        self.emit8(self.op("p_reltoplayeron" if self._onoff(args[0]) else "p_reltoplayeroff"))
                    elif op == "P_ALWAYSGENVECS":
                        self.emit8(self.op("p_alwaysgenvecson" if self._onoff(args[0]) else "p_alwaysgenvecsoff"))
                    elif op == "P_SPACESHIP":
                        self.emit8(self.op("p_spaceshipon" if self._onoff(args[0]) else "p_spaceshipoff"))
                    elif op == "P_HELICOPTER":
                        self.emit8(self.op("p_zacoon" if self._onoff(args[0]) else "p_zacooff"))
                    elif op == "P_WAIT":
                        n = self.eval_expr(args[0])
                        if n == 1:
                            self.emit8(self.op("p_wait1"))
                        else:
                            self.emit8(self.op("p_wait"))
                            self.emit8(n)
                    elif op == "P_SETVEL":
                        self.emit8(self.op("p_setvel"))
                        self.emit8(self.eval_expr(args[0]))
                    elif op == "P_LOOP":
                        self.emit8(self.op("p_loop"))
                        self.emit8(self.eval_expr(args[0]))
                        self.emit16_fix(self.resolve_label(args[1], cur_path), src)
                    elif op == "P_ADD":
                        var_off, is_word = self.parse_var(args[0])
                        v = self.eval_expr(args[1])
                        self.emit8(self.op("p_addw" if is_word else "p_addb"))
                        self.emit8(var_off)
                        if is_word:
                            self.emit16(v)
                        else:
                            self.emit8(v)
                    elif op == "P_SET":
                        var_off, is_word = self.parse_var(args[0])
                        v = self.eval_expr(args[1])
                        self.emit8(self.op("p_setw" if is_word else "p_setb"))
                        if is_word:
                            self.emit16(v)
                        else:
                            self.emit8(v)
                        self.emit8(var_off)
                    elif op == "P_ZERO":
                        var_off, is_word = self.parse_var(args[0])
                        self.emit8(self.op("p_set0w" if is_word else "p_set0b"))
                        self.emit8(var_off)
                    elif op == "P_INC":
                        var_off, is_word = self.parse_var(args[0])
                        self.emit8(self.op("p_incw" if is_word else "p_incb"))
                        self.emit8(var_off)
                    elif op == "P_DEC":
                        var_off, is_word = self.parse_var(args[0])
                        self.emit8(self.op("p_decw" if is_word else "p_decb"))
                        self.emit8(var_off)
                    elif op == "P_FACEPLAYER":
                        self.emit8(self.op("p_faceplayer"))
                    elif op == "P_WAITFACEPLAYER":
                        self.emit8(self.op("p_waitfaceplayer"))
                    elif op in ("P_CHASE", "P_WAITCHASE"):
                        var_off, is_word = self.parse_var(args[0])
                        v = self.eval_expr(args[1])
                        if op == "P_CHASE":
                            self.emit8(self.op("p_achasew" if is_word else "p_achaseb"))
                        else:
                            self.emit8(self.op("p_waitachasew" if is_word else "p_waitachaseb"))
                        if is_word:
                            self.emit16(v)
                        else:
                            self.emit8(v)
                        self.emit8(var_off)
                    elif op == "P_END":
                        self.emit8(self.op("p_end"))
                    elif op == "P_REMOVE":
                        if args:
                            self.emit8(self.op("p_removechild"))
                            self.emit8(self.eval_expr(args[0]))
                        else:
                            self.emit8(self.op("p_remove"))
                    elif op == "P_GOTO":
                        self.emit8(self.op("p_goto"))
                        self.emit16_fix(self.resolve_label(args[0], cur_path), src)
                    elif op == "P_IGOTO":
                        self.emit8(self.op("p_igoto"))
                        self.emit16_fix(self.resolve_label(args[0], cur_path), src)
                    elif op == "P_GOSUB":
                        self.emit8(self.op("p_gosub"))
                        self.emit16_fix(self.resolve_label(args[0], cur_path), src)
                    elif op == "P_RETURN":
                        self.emit8(self.op("p_return"))
                    elif op == "P_ACCEL":
                        self.emit8(self.op("p_accel"))
                        self.emit8(self.eval_expr(args[0]))
                        self.emit8(self.eval_expr(args[1]))
                    elif op == "P_DISTLESS":
                        self.emit8(self.op("p_distless"))
                        self.emit16(self.eval_expr(args[0]))
                        self.emit16_fix(self.resolve_label(args[1], cur_path), src)
                    elif op == "P_DISTMORE":
                        self.emit8(self.op("p_ifnot"))
                        self.emit8(self.op("p_distless"))
                        self.emit16(self.eval_expr(args[0]))
                        self.emit16_fix(self.resolve_label(args[1], cur_path), src)
                    elif op == "P_NOTFRIEND":
                        self.emit8(self.op("p_notfriend"))
                        self.emit8(self.eval_expr(f"friend_{args[0]}"))
                        self.emit16_fix(self.resolve_label(args[1], cur_path), src)
                    elif op == "P_IFFRIEND":
                        self.emit8(self.op("p_ifnot"))
                        self.emit8(self.op("p_notfriend"))
                        self.emit8(self.eval_expr(f"friend_{args[0]}"))
                        self.emit16_fix(self.resolve_label(args[1], cur_path), src)
                    elif op == "P_IFSAME":
                        var_off, is_word = self.parse_var(args[0])
                        self.emit8(self.op("p_ifsamew" if is_word else "p_ifsameb"))
                        self.emit8(var_off)
                        if is_word:
                            self.emit16(self.eval_expr(args[1]))
                        else:
                            self.emit8(self.eval_expr(args[1]))
                        self.emit16_fix(self.resolve_label(args[2], cur_path), src)
                    elif op == "P_IFNOTSAME":
                        self.emit8(self.op("p_ifnot"))
                        var_off, is_word = self.parse_var(args[0])
                        self.emit8(self.op("p_ifsamew" if is_word else "p_ifsameb"))
                        self.emit8(var_off)
                        if is_word:
                            self.emit16(self.eval_expr(args[1]))
                        else:
                            self.emit8(self.eval_expr(args[1]))
                        self.emit16_fix(self.resolve_label(args[2], cur_path), src)
                    elif op == "P_IFBETWEEN":
                        _, var_tok, _, _ = args
                        var_off, is_word = self.parse_var(var_tok)
                        self.emit8(self.op("p_ifbetweenw" if is_word else "p_ifbetweenb"))
                        self.emit8(var_off)
                        if is_word:
                            self.emit16(self.eval_expr(args[0]))
                            self.emit16(self.eval_expr(args[2]))
                        else:
                            self.emit8(self.eval_expr(args[0]))
                            self.emit8(self.eval_expr(args[2]))
                        self.emit16_fix(self.resolve_label(args[3], cur_path), src)
                    elif op == "P_IFNOTBETWEEN":
                        self.emit8(self.op("p_ifnot"))
                        _, var_tok, _, _ = args
                        var_off, is_word = self.parse_var(var_tok)
                        self.emit8(self.op("p_ifbetweenw" if is_word else "p_ifbetweenb"))
                        self.emit8(var_off)
                        if is_word:
                            self.emit16(self.eval_expr(args[0]))
                            self.emit16(self.eval_expr(args[2]))
                        else:
                            self.emit8(self.eval_expr(args[0]))
                            self.emit8(self.eval_expr(args[2]))
                        self.emit16_fix(self.resolve_label(args[3], cur_path), src)
                    elif op == "P_IFZERO":
                        var_off, is_word = self.parse_var(args[0])
                        self.emit8(self.op("p_ifzerow" if is_word else "p_ifzerob"))
                        self.emit8(var_off)
                        self.emit16_fix(self.resolve_label(args[1], cur_path), src)
                    elif op == "P_IFNOTZERO":
                        var_off, is_word = self.parse_var(args[0])
                        self.emit8(self.op("p_ifnotzerow" if is_word else "p_ifnotzerob"))
                        self.emit8(var_off)
                        self.emit16_fix(self.resolve_label(args[1], cur_path), src)
                    elif op == "P_INVINCIBLE":
                        self.emit8(self.op("p_invincibleon" if self._onoff(args[0]) else "p_invincibleoff"))
                    elif op == "P_ZREMOVE":
                        self.emit8(self.op("p_zremoveon" if self._onoff(args[0]) else "p_zremoveoff"))
                    elif op == "P_INVISIBLE":
                        self.emit8(self.op("p_invisibleon" if self._onoff(args[0]) else "p_invisibleoff"))
                    elif op == "P_COLLISIONS":
                        self.emit8(self.op("p_collisionson" if self._onoff(args[0]) else "p_collisionsoff"))
                    elif op == "P_SMOKE":
                        self.emit8(self.op("p_smokeon" if self._onoff(args[0]) else "p_smokeoff"))
                    elif op == "P_WEAPON":
                        self.emit8(self.op("p_weapon"))
                        w = args[0].strip().lower()
                        self.emit8(self.eval_expr(f"weapon_{w}", quiet=True))
                    elif op == "P_FIRE":
                        joined = ",".join(a.strip().lower() for a in args)
                        self.emit8(self.op("p_firecanhit" if "canhit" in joined else "p_fire"))
                    elif op == "P_IFFLAG":
                        self.emit8(self.op("p_ifflag"))
                        self.emit16_fix(self.resolve_label(args[0], cur_path), src)
                    elif op == "P_MSG":
                        self.emit8(self.op("p_msg"))
                        self.emit8(self.eval_expr(args[0]))
                    elif op == "P_RANDOMGOTO":
                        self.emit8(self.op("p_randomgoto"))
                        self.emit16_fix(self.resolve_label(args[0], cur_path), src)
                    elif op == "P_DO":
                        tok = args[0].strip()
                        m_var = re.match(r"^([a-z_][a-z0-9_]*)([+-]\d+)?$", tok.lower())
                        is_var = False
                        if m_var:
                            base = m_var.group(1)
                            is_var = (
                                base in self.base_byte
                                or base in self.base_word
                                or base in self.alx_byte
                                or base in self.alx_word
                            )
                        if is_var:
                            var_off, is_word = self.parse_var(tok)
                            self.emit8(self.op("p_doalvarw" if is_word else "p_doalvarb"))
                            self.emit8(var_off)
                        else:
                            n = self.eval_expr(args[0])
                            if 0 <= n < 256:
                                self.emit8(self.op("p_doq"))
                                self.emit8(n)
                            else:
                                self.emit8(self.op("p_do"))
                                self.emit16(n)
                    elif op == "P_NEXT":
                        self.emit8(self.op("p_next"))
                    elif op == "P_INEXT":
                        self.emit8(self.op("p_inext"))
                    elif op == "P_BREAK":
                        if args:
                            self.emit8(self.op("p_break"))
                            self.emit16_fix(self.resolve_label(args[0], cur_path), src)
                        else:
                            self.emit8(self.op("p_breakc"))
                    else:
                        self._diag(f"{src}: unsupported macro {op} (skipped)")
                        if self.strict:
                            raise RuntimeError(f"unsupported macro {op}")
                except Exception as e:
                    self._diag(f"{src}: compile error for {op}: {e}")
                    if self.strict:
                        raise

    def resolve_fixups(self):
        for fx in self.fixups:
            if fx.label not in self.labels:
                self._diag(f"{fx.source}: unresolved label '{fx.label}', using 0")
                target = 0
            else:
                target = self.labels[fx.label]
            self.data[fx.pos] = target & 0xFF
            self.data[fx.pos + 1] = (target >> 8) & 0xFF

    def compile(self, files: List[str]):
        for rel in files:
            p = os.path.join(self.repo_root, rel)
            self.compile_file(p)
        self.resolve_fixups()


def emit_header(comp: PathCompiler, array_name: str) -> str:
    lines = []
    lines.append("// Auto-generated by tools/path_compiler.py")
    lines.append("#ifndef STARFOX_PATH_PATHS_DATA_H")
    lines.append("#define STARFOX_PATH_PATHS_DATA_H")
    lines.append("")
    lines.append('#include "../types.h"')
    lines.append("")
    lines.append(f"#define PATH_DATA_COUNT {len(comp.path_offsets)}u")
    lines.append(f"#define PATH_DATA_SIZE  {len(comp.data)}u")
    lines.append("")
    lines.append("static const uint16 path_data_offsets[PATH_DATA_COUNT] = {")
    for i, off in enumerate(comp.path_offsets):
        tail = "," if i + 1 < len(comp.path_offsets) else ""
        lines.append(f"    {off}{tail}")
    lines.append("};")
    lines.append("")
    lines.append(f"static const uint8 {array_name}[PATH_DATA_SIZE] = {{")
    row = []
    for i, b in enumerate(comp.data):
        row.append(f"0x{b:02X}")
        if len(row) == 16:
            lines.append("    " + ", ".join(row) + ",")
            row = []
    if row:
        lines.append("    " + ", ".join(row))
    lines.append("};")
    lines.append("")
    lines.append("#endif // STARFOX_PATH_PATHS_DATA_H")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--strict", action="store_true")
    parser.add_argument("--array-name", default="path_data")
    parser.add_argument("--source", action="append", default=[])
    args = parser.parse_args()

    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    sources = args.source or [
        "reference/ultrastarfox/SF/PATH/PATHDATA.ASM",
        "reference/ultrastarfox/SF/PATH/DPATHDAT.ASM",
        "reference/ultrastarfox/SF/PATH/KPATHDAT.ASM",
    ]

    comp = PathCompiler(repo_root=repo_root, strict=args.strict)
    comp.compile(sources)
    sys.stdout.write(emit_header(comp, args.array_name))

    for d in comp.diagnostics:
        print(f"[path-compiler] {d}", file=sys.stderr)


if __name__ == "__main__":
    main()
