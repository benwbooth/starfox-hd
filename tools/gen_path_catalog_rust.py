#!/usr/bin/env python3
"""Transcribe the build_path_catalog() body of src/path/path_literals.c
into rust/sf-path/src/catalog_data.rs (mechanical pb_* -> builder-method
translation). The Rust output must emit byte-identical data."""
import re, sys

SRC = '/home/ben/src/starfox-hd/src/path/path_literals.c'
DST = '/home/ben/src/starfox-hd/rust/sf-path/src/catalog_data.rs'

lines = open(SRC).read().splitlines()

# Region: first statement after the s_*_ip resets to the line before pb_resolve(&b);
start = None
end = None
for i, l in enumerate(lines):
    if start is None and l.strip().startswith('// Missing/unported path scripts'):
        start = i
    if l.strip() == 'pb_resolve(&b);':
        end = i
        break
assert start is not None and end is not None, (start, end)
region = lines[start:end]

HEADER = '''\
//! Generated transcription of `build_path_catalog()`.
//!
//! C origin: `src/path/path_literals.c` build_path_catalog (the literal
//! PATHDATA.ASM / DPATHDAT.ASM / KPATHDAT.ASM / map-lane path scripts).
//! Mechanically translated from the C emit calls by
//! `scripts` transcription (pb_xxx(&b, ...) -> b.xxx(...)); comments —
//! including the ASM heritage line references — are carried over verbatim.
//!
//! Do not hand-edit the emit sequences without diffing against the C oracle;
//! the fixture test in `tests/catalog_bytes.rs` enforces byte equality.
#![allow(clippy::too_many_lines)]

use crate::builder::*;
use crate::ids::*;
use crate::literals::InlineIps;
use crate::opcodes::*;

// ---------------------------------------------------------------------------
// Constant environment of build_path_catalog (C origin noted per group).
// Typed i32 so values flow through the builder's C-style argument conversion.
// ---------------------------------------------------------------------------

// src/path/path_literals.c PATH_EXT_* (WRAM-mapped path-external addresses).
pub const PATH_EXT_SINTAB: i32 = 0x2200;
pub const PATH_EXT_GWORD1: i32 = 0x2300;
pub const PATH_EXT_EROLL1: i32 = 0x2302;
pub const PATH_EXT_EBYTE2: i32 = 0x2303;
pub const PATH_EXT_EFLAG1: i32 = 0x2304;
pub const PATH_EXT_CTYPE: i32 = 0x2305;

// src/path/path_literals.c ON/OFF trail-colour arguments.
const ON: i32 = 1;
#[allow(dead_code)]
const OFF: i32 = 0;

// Source path shapes expressed as native flat ids for the ROM-less catalog.
const SH_NULLSHAPE: i32 = 0;
const SH_PILLAR3: i32 = 27;
const SH_BOM_WING: i32 = 48;
const SH_R_BU_7: i32 = 102;
const SH_ROBOT_0: i32 = 420;
const SH_B_HOU_0: i32 = 164;
const SH_S_HOU_0: i32 = SH_B_HOU_0;
const SH_WALKER_2: i32 = 164;
const SH_GATE_2: i32 = 210;
const SH_ZACO_A: i32 = 217;
const SH_ZACO_B: i32 = 224;
const SH_FRIENDSHIP_4: i32 = 218;
const SH_FLOWER: i32 = 442;
const SH_BOSS_7_0: i32 = 240;
const SH_BOSS_7_1: i32 = 241;
const SH_BOSS_7_1O: i32 = 242;
const SH_BOSS_7_2: i32 = 243;
const SH_BOSS_7_3: i32 = 244;
const SH_BOSS_7_4: i32 = 245;
const SH_ARCH_0: i32 = 228;
const SH_TOW_0: i32 = 247;
const SH_PILLAR3_NS: i32 = SH_PILLAR3;
// `mediumshape` is a source collision/explosion envelope with no vertices or
// faces; null is its exact visual representation in the native renderer.
const SH_MEDIUMSHAPE: i32 = SH_NULLSHAPE;
const SH_ASTEROID1: i32 = 275;
const SH_EGG: i32 = 386;
const SH_BOSS_D_8: i32 = 387;
const SH_BOSS_D_9: i32 = 388;
const SH_BIG_BIRD: i32 = 443;

// src/path/path_literals.c COLTAB_ID_1_C (renderer currently ignores coltab).
const COLTAB_ID_1_C: i32 = 1;

// src/path/path_literals.c PATH_TRIGGER_*_VALUE (paths.c trigger enum values).
const PATH_TRIGGER_ALWAYS_VALUE: i32 = 0;
const PATH_TRIGGER_32_VALUE: i32 = 5;
const PATH_TRIGGER_WHENHIT_VALUE: i32 = 8;
const PATH_TRIGGER_WHENHITBYPLAYER_VALUE: i32 = 9;
const PATH_TRIGGER_WHENSHAPEDEAD_VALUE: i32 = 11;
const PATH_TRIGGER_WHENDEAD_VALUE: i32 = 12;

// src/path/path_literals.c WEAPON_* (flat weapon strat ids).
const WEAPON_REBELASER: i32 = 2;
const WEAPON_FRIENDELASER: i32 = 6;
const WEAPON_HPLASMA: i32 = 38;
const WEAPON_RINGLASER: i32 = 24;
const WEAPON_RELOVALBEAM: i32 = 50;
const WEAPON_RELSLOWELASER: i32 = 12;
const WEAPON_RELBEAMBALL: i32 = 56;

// src/path/path_literals.c STRAT_ID_* (flat strategy table ids).
const STRAT_ID_BREAK_METEOR: i32 = 235;
const STRAT_ID_GATE2: i32 = 207;

// src/variables.h DEG* (256-unit SNES angles).
const DEG90: i32 = 64;
const DEG270: i32 = 192;
const DEG45: i32 = 32;
const DEG22: i32 = 16;
const DEG5: i32 = 4;
const DEG11: i32 = 8;
const DEG180: i32 = 128;
// src/game/world.h level-exit codes.
const LE_ENTERSPEC: i32 = 16;
const DEG0: i32 = 0;

// src/variables.h FRIEND_* wingman ids.
const FRIEND_RABBIT: i32 = 1;
const FRIEND_FALCON: i32 = 2;
const FRIEND_FROG: i32 = 3;
const FRIEND_ANYONE: i32 = 6;

/// C `(uint8)` cast on an emit argument — wrap into byte range.
fn path_u8(value: i32) -> i32 {
    value as u8 as i32
}

/// C `PATH_I8` macro: `((int8)(uint8)(value))` — wrap into signed-byte range.
fn path_i8(value: i32) -> i32 {
    value as u8 as i8 as i32
}

/// Body of C `build_path_catalog()` between the builder prologue and
/// `pb_resolve` — every path script emit sequence, in C order.
pub(crate) fn emit_all(b: &mut PathLiteralBuilder, ips: &mut InlineIps) {
'''

FOOTER = '''\
}
'''

def transform_stmt(stmt: str) -> str:
    s = stmt
    # start65816 with out-ip
    m = re.match(r'pb_emit_start65816\(&b,\s*&s_(\w+)_ip,\s*("(?:[^"]*)")\)$', s)
    if m:
        return f'ips.{m.group(1)} = b.emit_start65816({m.group(2)})'
    # generic pb_ call
    m = re.match(r'pb_(\w+)\(&b\s*(?:,\s*)?(.*)\)$', s, re.S)
    assert m, s
    name, args = m.group(1), m.group(2)
    args = re.sub(r'\s+', ' ', args).strip()
    # casts
    args = re.sub(r'\(uint8\)\(int8\)\((-?\w+)\)', r'path_u8(path_i8(\1))', args)
    args = re.sub(r'\(uint8\)\(([^()]+)\)', r'path_u8(\1)', args)
    args = re.sub(r'\(uint8\)(-?\w+)', r'path_u8(\1)', args)
    args = re.sub(r'\(int8\)\(([^()]+)\)', r'path_i8(\1)', args)
    args = re.sub(r'\(int8\)(-?\w+)', r'path_i8(\1)', args)
    args = args.replace('PATH_I8(', 'path_i8(')
    return f'b.{name}({args})'

out = [HEADER]
i = 0
n = len(region)
while i < n:
    line = region[i]
    stripped = line.strip()
    if stripped == '' :
        out.append('')
        i += 1
        continue
    if stripped.startswith('//'):
        out.append('    ' + stripped)
        i += 1
        continue
    if stripped.startswith('#define ENDOFF'):
        out.append('    // (C: #define ENDOFF 1 — Andross-route ending offset)')
        out.append('    const ENDOFF: i32 = 1;')
        i += 1
        continue
    if stripped.startswith('#undef ENDOFF'):
        out.append('    // (C: #undef ENDOFF)')
        i += 1
        continue
    # statement: accumulate until ';' at depth 0 (strings have no ';')
    buf = ''
    trail_comments = []
    while i < n:
        l = region[i]
        code = l
        cm = re.search(r'//.*$', l)
        comment = None
        if cm and '"' not in l[:cm.start()].split('//')[0].rsplit('"', 1)[-1]:
            # trailing comment (labels never contain //)
            comment = cm.group(0)
            code = l[:cm.start()]
        buf += ' ' + code.strip()
        if comment:
            trail_comments.append(comment)
        i += 1
        if code.count('(') == code.count(')') and code.rstrip().endswith(';'):
            break
        if ';' in code and code.count('(') == code.count(')'):
            break
    stmt = buf.strip()
    assert stmt.endswith(';'), stmt
    # A source line may hold several statements; split at top-level ';'.
    parts = []
    depth = 0
    cur = ''
    for ch in stmt:
        if ch == '(':
            depth += 1
        elif ch == ')':
            depth -= 1
        if ch == ';' and depth == 0:
            parts.append(cur.strip())
            cur = ''
        else:
            cur += ch
    if cur.strip():
        parts.append(cur.strip())
    tc = ('  ' + ' '.join(trail_comments)) if trail_comments else ''
    for k, part in enumerate(parts):
        rust = transform_stmt(part)
        out.append(f'    {rust};{tc if k == len(parts) - 1 else ""}')

out.append(FOOTER)
open(DST, 'w').write('\n'.join(out))
print('wrote', DST, 'lines:', len(out))
