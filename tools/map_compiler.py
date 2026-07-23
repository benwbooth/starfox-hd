#!/usr/bin/env python3
"""
Star Fox HD — Map Script Compiler

Compiles level data from Python definitions into C byte arrays
compatible with the world.c map executor.

Usage:
    python3 tools/map_compiler.py > src/levels/level1_1_data.h
"""

import argparse
import ast
import os
import re
import struct
import sys

# ============================================================
# Shape IDs — extracted from ISTRATS.ASM def_shape order
# (cs counter, starting from 0)
# ============================================================
SH_NULLSHAPE    = 0
SH_EXITLIGHT    = 1
SH_MYSHIP_4     = 2
SH_IRIS         = 3
SH_TRUCK        = 4
SH_RAIL_4       = 5
SH_M_TANK       = 6
SH_GATE_0       = 7
SH_MISS_1_2     = 8
SH_KAMIKAZE     = 9
SH_LARGEPLASMA  = 10
SH_FLINGBOSS    = 11
SH_SHARK        = 12
SH_D_HEAD_0     = 13
SH_D_BODY_0     = 14
SH_CAMELEON     = 15
SH_BEEANIM      = 16
SH_ROUND_0      = 17
SH_BIG_M        = 18
SH_BOSS_1_2     = 19
SH_SHIP_1       = 20
SH_SHIP_3       = 21
SH_SHIP_0_C     = 22
SH_SSHIP_0_C    = 23
SH_WALL1        = 24
SH_WALL2        = 25
SH_WALKER_0     = 26
SH_PILLAR3      = 27
SH_MWIREEXIT    = 28
SH_LWIREEXIT    = 29
SH_BLTUNNELFACE = 30
SH_SEA_0_0      = 31
SH_EXIT_1       = 32
SH_LBLACKFACE   = 33
SH_LCUBE        = 34
SH_RAIL_0       = 35
SH_MISS_1_1     = 36
SH_EXITFACE     = 37
SH_MEXITFACE    = 38
SH_BSHIPEXITFACE = 39
SH_SHIP3EXITFACE = 40
SH_PILLAR2      = 41
SH_SPILLAR2     = 42
SH_BOSS_8_5     = 43
SH_HOU_4        = 44
SH_BOSS_8_4     = 45
SH_BOSS_8_0     = 46
SH_WIRE_MAN     = 47
SH_BOM_WING     = 48
SH_W_L          = 49
SH_RADER_0      = 50
SH_RADER_1      = 51
SH_ZACO_6       = 52
SH_ZACO_5       = 53
SH_HOUDAI_0     = 54
SH_BOSS_7_1     = 55
SH_BOSS_A_1     = 56
SH_BOSS_A_2     = 57
SH_TOWER_2      = 58
SH_PARA_0       = 59
SH_BU_0         = 60
SH_BU_1         = 61
SH_BU_2         = 62
SH_BU_3         = 63
SH_BU_4         = 64
SH_BU_5         = 65
SH_BU_6         = 66
SH_BU_7         = 67
SH_BU_8         = 68
SH_BOSS_2_2     = 69
SH_MYBASE_1     = 124
SH_MYBASE_0     = 0   # mybase_0 doesn't appear in def_shape list, use nullshape
SH_ZACO_A       = 217
SH_FRIENDSHIP_4 = 218
SH_ITEM_5       = 158
SH_ITEM_7       = 160
SH_WALKER_2     = 164
SH_ARCH_0       = 228
SH_BIG_GATE     = 233
SH_ROBOT_0      = 420
SH_R_BU_1       = 96
SH_CARRIER      = 114
SH_TOW_0        = 247

# ============================================================
# Strategy IDs — extracted from ISTRATS.ASM def_istrat order
# (ci counter, starting from 0)
# ============================================================
IS_PLAYER           = 0
IS_NOCOLL           = 10
IS_GND              = 11
IS_IRIS             = 47
IS_TRUCK            = 48
IS_GATE             = 52
IS_BEE1             = 64
IS_FIGHTER          = 65
IS_WINDMILL         = 66
IS_BOSS1            = 68
IS_FLYPILLARS       = 73
IS_PILLAR3          = 78
IS_BOMWING          = 88
IS_RADER0           = 91
IS_RADER1           = 92
IS_ZACOS            = 93
IS_ZACO1L           = 94
IS_ZACO1R           = 95
IS_HOUDAI           = 96
IS_BOSS7            = 98
IS_ZACO3            = 99
IS_TOWER0           = 100
IS_ZACO0            = 101
IS_ZACO4            = 102
IS_HARDENEMY1       = 103
IS_HARD180YR        = 104
IS_HARD180YRNZR     = 105
IS_FRIEND1          = 108
IS_HARD90YR         = 126
IS_STAYRELHARD180YR = 136
IS_FRIENDEXITBASE   = 151
IS_PATH             = 156
IS_ITEM5            = 174
IS_ITEM7            = 176
IS_WALKER2          = 179
IS_HARD180YRFOG     = 180
IS_HARD             = 225
IS_SHIPINTRO        = 238
IS_BOSS7INTRO       = 239
IS_SKILLFLY         = 240

# ============================================================
# SNES Alien struct field offsets (from STRUCTS.INC + GILESAL.INC)
# Offset from start of alien block (includes 4-byte linked list header)
# ============================================================
AL_SHAPE     = 4
AL_PTR       = 6
AL_FLAGS     = 8
AL_TYPE      = 9
AL_COUNT     = 10
AL_COUNT1    = 11
AL_WORLDX    = 12
AL_WORLDY    = 14
AL_WORLDZ    = 16
AL_ROTX      = 18
AL_ROTY      = 19
AL_ROTZ      = 20
AL_VEL       = 21
AL_STRATPTR  = 22   # 3 bytes
AL_IMMUNEPTR = 25
AL_COLLOBJPTR= 27
AL_SFLAGS    = 29
AL_SFLAGS2   = 30
AL_SFLAGS3   = 31
AL_SFLAGS4   = 32
AL_SKIDY     = 33
AL_SBYTE1    = 34
AL_SBYTE2    = 35
AL_SBYTE3    = 36
AL_SBYTE4    = 37
AL_SWORD1    = 38
AL_SWORD2    = 40
AL_HP        = 42
AL_AP        = 43

# Extended alien fields (alx_* prefix on SNES, separate block)
# We use 0x100+ to distinguish from regular fields in the bytecode
ALX_SWPX1       = 0
ALX_SWPY1       = 2
ALX_SWPZ1       = 4
ALX_DEPTHOFFSET = 21

# ============================================================
# Map Opcode Constants
# ============================================================
MAP_OP_MAPOBJ    = 0
MAP_OP_END       = 2
MAP_OP_LOOP      = 4
MAP_OP_NOP       = 8
MAP_OP_MOTHER    = 10
MAP_OP_REMOVE    = 12
MAP_OP_WAIT      = 18
MAP_OP_SETBGM    = 20
MAP_OP_SETSTAGE  = 14
MAP_OP_SETBG     = 16
MAP_OP_OBJZROT   = 38
MAP_OP_IF        = 44
MAP_OP_JSR       = 40
MAP_OP_RTS       = 42
MAP_OP_GOTO      = 46
MAP_OP_SETXROT   = 48
MAP_OP_SETYROT   = 50
MAP_OP_SETZROT   = 52
MAP_OP_SETALVARB = 54
MAP_OP_SETALVARW = 56
MAP_OP_SETALVARL = 58
MAP_OP_SETALXVARB = 60
MAP_OP_SETALXVARW = 62
MAP_OP_FADEUP    = 66
MAP_OP_FADEDOWN  = 68
MAP_OP_SETALVARPB = 70
MAP_OP_SETALVARPW = 72
MAP_OP_SETVAROBJ = 74
MAP_OP_WAITFADE  = 76
MAP_OP_QFADEUP   = 78
MAP_OP_QFADEDOWN = 80
MAP_OP_SPECIAL   = 90
MAP_OP_SETVARB   = 92
MAP_OP_SETVARW   = 94
MAP_OP_SETVARL   = 96
MAP_OP_WAITSETBG = 100
MAP_OP_SETBGINFO = 102
MAP_OP_ADDALVARPB = 104
MAP_OP_ADDALVARPW = 106
MAP_OP_FADETOSEA = 108
MAP_OP_FADETOGROUND = 110
MAP_OP_CODE65816 = 120
MAP_OP_CODEJSL  = 122
MAP_OP_JMPVARLESS = 124
MAP_OP_JMPVARMORE = 126
MAP_OP_JMPVAREQ = 128
MAP_OP_SENDMSG   = 130
MAP_OP_CSPECIAL  = 132
MAP_OP_NORMOBJ   = 134
MAP_OP_WAIT2     = 138
MAP_OP_SETPATH   = 140

# ============================================================
# World native callback IDs (must match src/game/world.h)
# Flat-memory runtime IDs for mapif/mapcode_jsl literals.
# ============================================================
MAP_CB_CHKSTAGEDONE         = 0x010001
MAP_CB_CHKSTRATDONE1        = 0x010002
MAP_CB_CHKSTRATDONE2        = 0x010003
MAP_CB_CHKBOSSDEAD          = 0x010004
MAP_CB_THEENDDEAD           = 0x010005
MAP_CB_INITBLACK_L          = 0x010101
MAP_CB_SETCHARMAPFROMMAP_L  = 0x010102
MAP_CB_INITFADEWHITE2NORM_L = 0x010103
MAP_CB_KILL_ROBOT_L         = 0x010104
MAP_CB_CLEARMAP_L           = 0x010105
MAP_CB_CLEARREALOBJMAP_L    = 0x010106
MAP_CB_SETRESTART_L         = 0x010107
MAP_CB_MARKBOSS_L           = 0x010108

MAP_CB_FROG_ALIVE           = 0x010201
MAP_CB_BUNNY_ALIVE          = 0x010202
MAP_CB_COCK_ALIVE           = 0x010203

MAP_CB_CLFRIENDMSG_FROG     = 0x010211
MAP_CB_CLFRIENDMSG_BUNNY    = 0x010212
MAP_CB_CLFRIENDMSG_COCK     = 0x010213

MAP_CB_SET_PLAYER_EXITBASE_L = 0x010301
MAP_CB_SET_PLAYER_ONPLANET_L = 0x010302
MAP_CB_SET_PLAYER_CLEARDEMO_L = 0x010303

MAP_CB_IS_PLAYER_DEAD       = 0x010401
MAP_CB_PLAYER_OUTVIEW_L     = 0x010402
MAP_CB_LEVELFINISHED_ZERO   = 0x010403

# ============================================================
# Game Constants
# ============================================================
MEDPSPEED = 6
PEXITBASE_SPEED = 50
DEG0   = 0
DEG22  = 16
DEG45  = 32
DEG90  = 64
DEG180 = 128
MYBASE_SCALE = 4  # shift amount for base coordinates


class MapBuilder:
    """Builds a map script bytecode array with label support."""

    def __init__(self):
        self.data = bytearray()
        self.labels = {}
        self.fixups = []  # (byte_offset, label_name)

    def pos(self):
        return len(self.data)

    def label(self, name):
        self.labels[name] = self.pos()

    def _emit8(self, v):
        self.data.append(v & 0xFF)

    def _emit16(self, v):
        self.data.extend(struct.pack('<H', v & 0xFFFF))

    def _emit16s(self, v):
        self.data.extend(struct.pack('<h', max(-32768, min(32767, v))))

    # ----------------------------------------------------------
    # Standard object spawn
    # Data: opcode(1) + frame(2) + x(2) + y(2) + z(2) + shape(1) + strat(1) = 11 bytes
    # ----------------------------------------------------------
    def mapobj(self, frame, x, y, z, shape, strat):
        self._emit8(MAP_OP_MAPOBJ)
        self._emit16(frame)
        self._emit16s(x)
        self._emit16s(y)
        self._emit16s(z)
        self._emit8(shape)
        self._emit8(strat)

    def mapnobj(self, frame, x, y, z, shape16, strat24):
        self._emit8(MAP_OP_NORMOBJ)
        self._emit16(frame)
        self._emit16s(x)
        self._emit16s(y)
        self._emit16s(z)
        self._emit16(shape16)
        self._emit16(strat24 & 0xFFFF)
        self._emit8((strat24 >> 16) & 0xFF)

    def mapmother(self, frame, x, y, z, shape16, strat24, map_ref):
        self._emit8(MAP_OP_MOTHER)
        self._emit16(frame)
        self._emit16s(x)
        self._emit16s(y)
        self._emit16s(z)
        self._emit16(shape16)
        self._emit16(strat24 & 0xFFFF)
        self._emit8((strat24 >> 16) & 0xFF)
        if isinstance(map_ref, str):
            self.fixups.append((self.pos(), map_ref))
            self._emit16(0)
        else:
            self._emit16(map_ref)

    def maprem(self, frame, shape16):
        self._emit8(MAP_OP_REMOVE)
        self._emit16(frame)
        self._emit16(shape16)

    def mapobjzrot(self, frame, x, y, z, shape, strat, zrot):
        self._emit8(MAP_OP_OBJZROT)
        self._emit16(frame)
        self._emit16s(x)
        self._emit16s(y)
        self._emit16s(z)
        self._emit8(shape)
        self._emit8(strat)
        self._emit8(zrot)

    # ----------------------------------------------------------
    # Wait by distance
    # ----------------------------------------------------------
    def mapwait(self, dist):
        self._emit8(MAP_OP_WAIT)
        self._emit16(dist)

    def mapwait2(self, dist_raw):
        self._emit8(MAP_OP_WAIT2)
        self._emit8(dist_raw)

    # ----------------------------------------------------------
    # Loop: jump to label, count times
    # ----------------------------------------------------------
    def maploop(self, label_name, count):
        self._emit8(MAP_OP_LOOP)
        self.fixups.append((self.pos(), label_name))
        self._emit16(0)  # fixup slot
        self._emit16(count)

    # ----------------------------------------------------------
    # Subroutine call / return
    # ----------------------------------------------------------
    def mapjsr(self, label_name):
        self._emit8(MAP_OP_JSR)
        self.fixups.append((self.pos(), label_name))
        self._emit16(0)  # fixup slot
        self._emit8(0)   # bank byte (ignored in HD)

    def maprts(self):
        self._emit8(MAP_OP_RTS)

    # ----------------------------------------------------------
    # End of level
    # ----------------------------------------------------------
    def mapend(self):
        self._emit8(MAP_OP_END)

    # ----------------------------------------------------------
    # Goto
    # ----------------------------------------------------------
    def mapgoto(self, label_name):
        self._emit8(MAP_OP_GOTO)
        self.fixups.append((self.pos(), label_name))
        self._emit16(0)  # fixup slot
        self._emit8(0)   # bank

    # ----------------------------------------------------------
    # Conditional branch callback
    # mapif callback_addr24, else_label
    # ----------------------------------------------------------
    def mapif_builtin(self, callback_addr24, else_label):
        self._emit8(MAP_OP_IF)
        self._emit16(callback_addr24)
        self._emit8((callback_addr24 >> 16) & 0xFF)
        self.fixups.append((self.pos(), else_label))
        self._emit16(0)  # else-address fixup slot

    # ----------------------------------------------------------
    # Set rotation on last spawned object
    # ----------------------------------------------------------
    def setxrot(self, rot):
        self._emit8(MAP_OP_SETXROT)
        self._emit8(rot)

    def setyrot(self, rot):
        self._emit8(MAP_OP_SETYROT)
        self._emit8(rot)

    def setzrot(self, rot):
        self._emit8(MAP_OP_SETZROT)
        self._emit8(rot)

    # ----------------------------------------------------------
    # Set alien variable (byte/word) using SNES field offset
    # ----------------------------------------------------------
    def setalvarb(self, field_offset, value):
        self._emit8(MAP_OP_SETALVARB)
        self._emit16(field_offset)
        self._emit8(value & 0xFF)

    def setalvarw(self, field_offset, value):
        self._emit8(MAP_OP_SETALVARW)
        self._emit16(field_offset)
        self._emit16s(value)

    def setalvarl(self, field_offset, value24):
        self._emit8(MAP_OP_SETALVARL)
        self._emit16(field_offset)
        self._emit16(value24 & 0xFFFF)
        self._emit8((value24 >> 16) & 0xFF)

    def setalxvarb(self, field_offset, value):
        self._emit8(MAP_OP_SETALXVARB)
        self._emit16(field_offset)
        self._emit8(value & 0xFF)

    def setalxvarw(self, field_offset, value):
        self._emit8(MAP_OP_SETALXVARW)
        self._emit16(field_offset)
        self._emit16s(value)

    def setalxvarl(self, field_offset, value24):
        self._emit8(MAP_OP_SETALXVARL)
        self._emit16(field_offset)
        self._emit16(value24 & 0xFFFF)
        self._emit8((value24 >> 16) & 0xFF)

    def setalvarptrb(self, field_offset, ptr16):
        self._emit8(MAP_OP_SETALVARPB)
        self._emit16(field_offset)
        self._emit16(ptr16)
        self._emit8(0)  # flat-memory bank ignored

    def setalvarptrw(self, field_offset, ptr16):
        self._emit8(MAP_OP_SETALVARPW)
        self._emit16(field_offset)
        self._emit16(ptr16)
        self._emit8(0)  # flat-memory bank ignored

    def addalvarptrb(self, field_offset, ptr16):
        self._emit8(MAP_OP_ADDALVARPB)
        self._emit16(field_offset)
        self._emit16(ptr16)
        self._emit8(0)  # flat-memory bank ignored

    def addalvarptrw(self, field_offset, ptr16):
        self._emit8(MAP_OP_ADDALVARPW)
        self._emit16(field_offset)
        self._emit16(ptr16)
        self._emit8(0)  # flat-memory bank ignored

    # ----------------------------------------------------------
    # Special / CSpecial markers
    # ----------------------------------------------------------
    def mapspecial(self):
        self._emit8(MAP_OP_SPECIAL)

    def mapcspecial(self):
        self._emit8(MAP_OP_CSPECIAL)

    # ----------------------------------------------------------
    # Set path on last object
    # path_id is a path data index (we'll define path data separately)
    # ----------------------------------------------------------
    def mapsetpath(self, path_id):
        self._emit8(MAP_OP_SETPATH)
        self._emit16(path_id)

    # ----------------------------------------------------------
    # Music
    # ----------------------------------------------------------
    def setbgm(self, music_id):
        self._emit8(MAP_OP_SETBGM)
        self._emit8(music_id)

    def setstage(self):
        self._emit8(MAP_OP_SETSTAGE)

    def setbg(self, bg_id):
        self._emit8(MAP_OP_SETBG)
        self._emit16(bg_id)

    def waitsetbg(self):
        self._emit8(MAP_OP_WAITSETBG)

    def setbginfo(self):
        self._emit8(MAP_OP_SETBGINFO)

    # ----------------------------------------------------------
    # Fade
    # ----------------------------------------------------------
    def fadeup(self):
        self._emit8(MAP_OP_FADEUP)

    def fadedown(self):
        self._emit8(MAP_OP_FADEDOWN)

    def qfadeup(self):
        self._emit8(MAP_OP_QFADEUP)

    def qfadedown(self):
        self._emit8(MAP_OP_QFADEDOWN)

    def waitfade(self):
        self._emit8(MAP_OP_WAITFADE)

    def sendmsg(self, msg_id):
        self._emit8(MAP_OP_SENDMSG)
        self._emit8(msg_id)

    def fadetosea(self):
        self._emit8(MAP_OP_FADETOSEA)

    def fadetoground(self):
        self._emit8(MAP_OP_FADETOGROUND)

    def setvarb(self, ptr16, value8):
        self._emit8(MAP_OP_SETVARB)
        self._emit8(value8 & 0xFF)
        self._emit16(ptr16)
        self._emit8(0)  # flat-memory bank ignored at runtime

    def setvarw(self, ptr16, value16):
        self._emit8(MAP_OP_SETVARW)
        self._emit16(value16 & 0xFFFF)
        self._emit16(ptr16)
        self._emit8(0)  # flat-memory bank ignored at runtime

    def setvarl(self, ptr16, value24):
        self._emit8(MAP_OP_SETVARL)
        self._emit16(ptr16)
        self._emit8(0)  # flat-memory bank ignored at runtime
        self._emit16(value24 & 0xFFFF)
        self._emit8((value24 >> 16) & 0xFF)

    def setvarobj(self, ptr16):
        self._emit8(MAP_OP_SETVAROBJ)
        self._emit16(ptr16)
        self._emit8(0)  # flat-memory bank ignored at runtime

    def mapjmpvarless(self, ptr16, value8, label_name):
        self._emit8(MAP_OP_JMPVARLESS)
        self._emit16(ptr16)
        self._emit8(0)  # flat-memory bank ignored
        self._emit8(value8 & 0xFF)
        self.fixups.append((self.pos(), label_name))
        self._emit16(0)

    def mapjmpvarmore(self, ptr16, value8, label_name):
        self._emit8(MAP_OP_JMPVARMORE)
        self._emit16(ptr16)
        self._emit8(0)  # flat-memory bank ignored
        self._emit8(value8 & 0xFF)
        self.fixups.append((self.pos(), label_name))
        self._emit16(0)

    def mapjmpvareq(self, ptr16, value8, label_name):
        self._emit8(MAP_OP_JMPVAREQ)
        self._emit16(ptr16)
        self._emit8(0)  # flat-memory bank ignored
        self._emit8(value8 & 0xFF)
        self.fixups.append((self.pos(), label_name))
        self._emit16(0)

    # ----------------------------------------------------------
    # JSL callback
    # mapcode_jsl callback_addr24
    # SNES macro encodes (func - 1) low word.
    # ----------------------------------------------------------
    def mapcodejsl_builtin(self, callback_addr24):
        self._emit8(MAP_OP_CODEJSL)
        encoded = (callback_addr24 - 1) & 0xFFFF
        self._emit16(encoded)
        self._emit8((callback_addr24 >> 16) & 0xFF)

    # ----------------------------------------------------------
    # Compound macros (matching MAPMACS.INC)
    # ----------------------------------------------------------

    def special(self, wait, x, y, z, shape, strat):
        """mapobj + mapspecial + mapwait"""
        self.mapobj(0, x, y, z, shape, strat)
        self.mapspecial()
        self.mapwait(wait)

    def cspecial(self, wait, x, y, z, shape, strat):
        """mapobj + mapcspecial + mapwait"""
        self.mapobj(0, x, y, z, shape, strat)
        self.mapcspecial()
        self.mapwait(wait)

    def pathobj(self, wait, x, y, z, shape, path_id, hp, ap):
        """Spawn a path-following object."""
        if hp == 10 and ap == 10:
            self.mapobj(0, x, y, z, shape, IS_PATH)
            self.setalvarb(AL_HP, hp)
            self.setalvarb(AL_AP, ap)
        else:
            self.mapobj(0, x, y, z, shape, IS_PATH)
            self.setalvarb(AL_HP, hp)
            self.setalvarb(AL_AP, ap)
        self.mapsetpath(path_id)
        self.mapwait(wait)

    def pathcspecial(self, wait, x, y, z, shape, path_id, hp, ap):
        """Spawn a path-following cspecial object."""
        if hp == 10 and ap == 10:
            self.mapobj(0, x, y, z, shape, IS_PATH)
            self.setalvarb(AL_HP, hp)
            self.setalvarb(AL_AP, ap)
        else:
            self.mapobj(0, x, y, z, shape, IS_PATH)
            self.setalvarb(AL_HP, hp)
            self.setalvarb(AL_AP, ap)
        self.mapsetpath(path_id)
        self.mapcspecial()
        self.mapwait(wait)

    # ----------------------------------------------------------
    # Resolve label references
    # ----------------------------------------------------------
    def resolve(self):
        for byte_offset, label_name in self.fixups:
            if label_name not in self.labels:
                raise ValueError(f"Undefined label: {label_name}")
            addr = self.labels[label_name]
            struct.pack_into('<H', self.data, byte_offset, addr & 0xFFFF)

    # ----------------------------------------------------------
    # Output as C byte array
    # ----------------------------------------------------------
    def to_c_array(self, name):
        self.resolve()
        lines = [f"static const uint8 {name}[] = {{"]
        for i in range(0, len(self.data), 16):
            chunk = self.data[i:i+16]
            hex_vals = ", ".join(f"0x{b:02X}" for b in chunk)
            lines.append(f"    {hex_vals},")
        lines.append("};")
        return "\n".join(lines)


# ============================================================
# Path data IDs (path scripts are defined separately)
# These correspond to path bytecode scripts for wingmen/enemies.
# ============================================================
PATH_MATEMSG     = 0
PATH_FALCO_LV1   = 1
PATH_FROG_LV1    = 2
PATH_FROG1_1     = 3
PATH_E_GATE      = 4
PATH_TOW_0       = 5
PATH_PONPON      = 6
PATH_ROBOT       = 7
PATH_ROBOTWITHLOG = 8
PATH_ROBOTSWITHLOG = 9
PATH_KORORI      = 10
PATH_PATROL      = 11
PATH_CHASE8_1    = 12
PATH_CHASE8_2    = 13
PATH_CHASE8_3    = 14
PATH_CHASE6_1    = 15
PATH_CHASE6_2    = 16
PATH_E_UFO       = 17
PATH_CHASE7_1    = 18
PATH_CHASE7_2    = 19
PATH_FALCON3_1   = 20


class LiteralMapCompileError(Exception):
    pass


class LiteralAsmParser:
    """Literal parser for reference MAP*.ASM files into flat map bytecode."""

    _TOKEN_RE = re.compile(
        r"(?<![A-Za-z0-9_$\.])(?:[A-Za-z_.][A-Za-z0-9_.]*|[0-9][A-Za-z_][A-Za-z0-9_.]*)(?![A-Za-z0-9_.])"
    )

    _ASSIGN_RE = re.compile(r"^\s*([A-Za-z_.][A-Za-z0-9_.]*)\s*=\s*(.+)$")
    _EQU_RE = re.compile(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s+equ\s+(.+)$", re.IGNORECASE)

    _DIRECTIVES = {"ifeq", "ifne", "ifd", "ifnd", "elseif", "else", "endc"}

    _QUIET_NOOP_MACROS = {
        "initlevel",
        "mapplayercantdie",
        "mapmsufade",
        "mapmsuplay",
        "waitfadefin",
        # Mother-script helpers are interpreted by mother strategies, not map VM.
        "motherobj",
        "motherloop",
        "mothergoto",
        "motherend",
        "motherrnd",
        "motherwait",
        "mothercnt",
        "motherjump",
        "motherset",
        # Other script-side helpers currently modeled as no-op in map VM.
        "mapblocksnd",
        "mapnozremove",
        "textpath",
        "mapdef",
        "coursedef",
        "incpublics",
        "incfile",
        "endm",
        "?maps",
    }

    def __init__(self, source_path, strict=False):
        self.source_path = os.path.abspath(source_path)
        self.strict = strict
        self.m = MapBuilder()
        self.diagnostics = []
        self._file_seq = 0
        self._file_prefix = {}  # abs_path -> prefix for local labels
        self._parsed_files = set()
        self._macro_seq = 0
        self._ctx_stack = []

        self.repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
        self.map_dir = os.path.dirname(self.source_path)
        self.label_file_index = self._index_global_map_labels(self.map_dir)

        self.symbols = self._build_base_symbols()
        self.shape_ids = {}
        self.strat_ids = {}
        self.path_ids = self._build_path_ids()
        self.next_auto_path_id = (max(self.path_ids.values()) + 1) if self.path_ids else 0
        self.al_offsets = self._build_al_offsets()
        self.alx_offsets = self._build_alx_offsets()
        self.callback_ids = self._build_callback_ids()

        self._load_reference_istrats()
        self._load_reference_sound_equ()
        self._load_reference_stratequ_symbols()
        self._load_reference_vars_symbols()
        self._load_reference_alloc_symbols()
        self._load_reference_bg_symbols()
        self._load_reference_path_names()
        self._load_reference_ext_symbols()

        self.next_auto_shape_id = (max(self.shape_ids.values()) + 1) if self.shape_ids else 0
        self.next_auto_strat_id = (max(self.strat_ids.values()) + 1) if self.strat_ids else 0
        self.next_auto_raw_shape = 0x2000
        self.next_auto_raw_strat = 0x020000

        # Macro-time state for MAPMACS composite helpers.
        self._spacebar_style = "solid"
        self._spacebar_pos = 0
        self._spacebar_swait = 0
        self._spacebar_dowait_z = 1
        self._truck_tx = 0
        self._truck_tz = 0
        self._truck_ta = 0

    # ----------------------------------------------------------
    # Public API
    # ----------------------------------------------------------
    def compile(self):
        self._parse_file(self.source_path)
        self._auto_include_unresolved_label_files()
        self._materialize_unresolved_stubs()
        if self.strict and self.diagnostics:
            raise LiteralMapCompileError(
                f"Literal parse emitted {len(self.diagnostics)} diagnostic(s) in strict mode"
            )
        return self.m

    # ----------------------------------------------------------
    # Setup helpers
    # ----------------------------------------------------------
    def _build_base_symbols(self):
        syms = {
            "wm": 0xFFFF,
            "noscrambleseq": 1,
            "msu1": 0,
            "hidehudonbossdeath": 0,
            "medpspeed": MEDPSPEED,
            "pexitbasespeed": PEXITBASE_SPEED,
            "mybase_scale": MYBASE_SCALE,
            "boss7_scale": 3,
            "bossa_scale": 2,
            "deg0": DEG0,
            "deg11": 8,
            "deg22": DEG22,
            "deg45": DEG45,
            "deg90": DEG90,
            "deg180": DEG180,
            "deg360": 256,
            # Flat-memory external var slots used by map setvar* macros.
            "clb2": 0x0300,
            "stageclear": 0x0302,
            "scramble": 0x0303,
            "skillfly": 0x0304,
            "levelfinished": 0x0305,
            "m_meters": 0x0306,
            "mapvar1": 0x0320,
            "mapvar2": 0x0322,
            "mapvar3": 0x0324,
            "mapvar4": 0x0326,
            "mapvar5": 0x0328,
            "mapvar6": 0x032A,
            "mapvar7": 0x032C,
            "mapvar8": 0x032E,
            # Common environment symbols from STRATEQU/MAPMACS.
            "space_viewcy": -60,
            "mtunnel_viewcy": -60,
            "mtexit_viewcy": -60,
            "mtexit_minx": -50,
            "mtexit_maxx": 50,
            "mtexit_mminx": -50,
            "mtexit_mmaxx": 50,
            "mtexit_miny": -95,
            "mtexit_maxy": -25,
            "mtexit_mmaxy": -25,
            "space_minx": -240,
            "space_maxx": 240,
            "sxspacebarlen": 250,
            "syspacebarlen": 250,
            "szspacebarlen": 250,
            "xlen": 125,
            "ylen": 125,
            "zlen": 125,
            "clen": 125,
            "dist": 3000,
            "swait": 0,
            "dowaitz": 1,
            "speed": 0,
            "pipescale": 1,
            "pdist": 0,
            "bglists": 0,
            "rumble": 0,
            "german": 0,
            "pal": 0,
            "exitcredits": 0,
            "infog": 0,
            "bg_1_4b_1": 0,
            "prntrouln_on": 0,
            "totalmsgs": 0,
            "messagetest": 0,
            "mapbase": 0,
            "airlock_pos": 0,
            # Direction aliases used by MAPMACS truck helpers.
            "dirnorth": DEG0,
            "direast": DEG90,
            "dirsouth": DEG180,
            "dirwest": (DEG180 + DEG90),
            # Shape/pointer aliases used by literal maps.
            "tower_2a": SH_TOWER_2,
            "boss_7_0": SH_BOSS_7_1,
            "boss_7_3": SH_BOSS_7_1,
            # Strategy alias used by skillfly_bonus in route-1 maps.
            "gate3_istrat": IS_GATE,
            "frog": 1,
            "bunny": 1,
            "cock": 1,
        }

        # Reuse existing numeric constants from this compiler.
        for k, v in globals().items():
            if not isinstance(v, int):
                continue
            if k.startswith(("MAP_CB_", "MAP_OP_")):
                syms[k.lower()] = v
                continue
            if k.startswith("SH_"):
                syms[k[3:].lower()] = v
                syms[k.lower()] = v
                continue
            if k.startswith("IS_"):
                syms[k[3:].lower()] = v
                syms[k.lower()] = v
                continue
            if k.startswith("AL_"):
                syms[k[3:].lower()] = v
                syms[k.lower()] = v
                continue
            if k.startswith("ALX_"):
                syms[k[4:].lower()] = v
                syms[k.lower()] = v
                continue
            if k.startswith("PATH_"):
                syms[k[5:].lower()] = v
                syms[k.lower()] = v

        # Compatibility alias used by some maps.
        syms["mybase_0"] = SH_MYBASE_0
        return syms

    def _build_path_ids(self):
        ids = {}
        for k, v in globals().items():
            if isinstance(v, int) and k.startswith("PATH_"):
                ids[k[5:].lower()] = v
        return ids

    def _build_al_offsets(self):
        offsets = {}
        for k, v in globals().items():
            if isinstance(v, int) and k.startswith("AL_") and not k.startswith("ALX_"):
                offsets[k[3:].lower()] = v
        return offsets

    def _build_alx_offsets(self):
        offsets = {}
        for k, v in globals().items():
            if isinstance(v, int) and k.startswith("ALX_"):
                offsets[k[4:].lower()] = v
        # Needed by map3_1 literal scripts (from STRUCTS.INC + GILESALX.INC).
        offsets.setdefault("pword1", 52)
        return offsets

    def _build_callback_ids(self):
        return {
            "chkstagedone": MAP_CB_CHKSTAGEDONE,
            "chkstratdone1": MAP_CB_CHKSTRATDONE1,
            "chkstratdone2": MAP_CB_CHKSTRATDONE2,
            "chkbossdead": MAP_CB_CHKBOSSDEAD,
            "theenddead": MAP_CB_THEENDDEAD,
            "initblack_l": MAP_CB_INITBLACK_L,
            "setcharmapfrommap_l": MAP_CB_SETCHARMAPFROMMAP_L,
            "initfadewhite2norm_l": MAP_CB_INITFADEWHITE2NORM_L,
            "kill_robot_l": MAP_CB_KILL_ROBOT_L,
            "clearmap_l": MAP_CB_CLEARMAP_L,
            "clearrealobjmap_l": MAP_CB_CLEARREALOBJMAP_L,
            "set_restart_position_l": MAP_CB_SETRESTART_L,
            "setboss_l": MAP_CB_MARKBOSS_L,
            "set_playerexitbase_l": MAP_CB_SET_PLAYER_EXITBASE_L,
            "set_playeronplanet_l": MAP_CB_SET_PLAYER_ONPLANET_L,
            "set_playercleardemo_l": MAP_CB_SET_PLAYER_CLEARDEMO_L,
            "is_player_dead_l": MAP_CB_IS_PLAYER_DEAD,
            "set_playeroutview_l": MAP_CB_PLAYER_OUTVIEW_L,
            "is_levelfinished_zero_l": MAP_CB_LEVELFINISHED_ZERO,
        }

    def _load_reference_istrats(self):
        path = os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "STRAT", "ISTRATS.ASM")
        if not os.path.exists(path):
            return

        cs = 0
        ci = 0
        with open(path, "r", encoding="latin1", errors="ignore") as f:
            for raw in f:
                line = raw.split(";", 1)[0].strip().replace("\x1a", "")
                if not line:
                    continue
                m_shape = re.match(r"(?i)^def_shape\s+([A-Za-z0-9_]+)", line)
                if m_shape:
                    name = m_shape.group(1).lower()
                    self.shape_ids[name] = cs
                    cs += 1
                    continue
                m_strat = re.match(r"(?i)^def_istrat\s+([A-Za-z0-9_]+)", line)
                if m_strat:
                    name = m_strat.group(1).lower() + "_istrat"
                    self.strat_ids[name] = ci
                    ci += 1

        # Preserve existing hardcoded IDs where they are already known in this compiler.
        for k, v in globals().items():
            if isinstance(v, int) and k.startswith("SH_"):
                self.shape_ids.setdefault(k[3:].lower(), v)
            if isinstance(v, int) and k.startswith("IS_"):
                self.strat_ids.setdefault(k[3:].lower() + "_istrat", v)
                self.strat_ids.setdefault(k[3:].lower(), v)

    def _load_reference_sound_equ(self):
        path = os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "INC", "SOUNDEQU.INC")
        self._load_reference_equ_assign_file(path)

    def _load_reference_stratequ_symbols(self):
        path = os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "INC", "STRATEQU.INC")
        self._load_reference_equ_assign_file(path)

    def _load_reference_vars_symbols(self):
        path = os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "INC", "VARS.INC")
        self._load_reference_equ_assign_file(path)

    def _load_reference_alloc_symbols(self):
        inc_dir = os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "INC")
        if not os.path.isdir(inc_dir):
            return

        # Flat-memory space for ext vars referenced by setvar/mapjmpvar opcodes.
        next_addr = max([v for v in self.symbols.values() if isinstance(v, int)], default=0x0400)
        next_addr = max(next_addr + 0x20, 0x0400)

        alloc_files = (
            "ALCS.INC",
            "DALCS.INC",
            "EALCS.INC",
            "GILESALC.INC",
            "KALCS.INC",
            "MOUSEALC.INC",
        )
        for rel in alloc_files:
            path = os.path.join(inc_dir, rel)
            if not os.path.exists(path):
                continue
            self._load_reference_equ_assign_file(path)
            with open(path, "r", encoding="latin1", errors="ignore") as f:
                for lineno, raw in enumerate(f, start=1):
                    line = raw.split(";", 1)[0].strip().replace("\x1a", "")
                    if not line:
                        continue
                    m = re.match(r"(?i)^alc\s+([A-Za-z_][A-Za-z0-9_]*)\s*,\s*(.+)$", line)
                    if not m:
                        continue
                    name = m.group(1).lower()
                    size = self._eval_expr(m.group(2), path, lineno, quiet=True)
                    if size <= 0:
                        size = 1
                    if name not in self.symbols:
                        self.symbols[name] = next_addr
                    next_addr = max(next_addr, int(self.symbols[name])) + size

    def _load_reference_ext_symbols(self):
        ext_dir = os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "EXT")
        if not os.path.isdir(ext_dir):
            return

        next_shape = max(self.shape_ids.values(), default=0) + 1
        next_strat = max(self.strat_ids.values(), default=0) + 1
        next_sym = 0x2000

        entries = sorted([n for n in os.listdir(ext_dir) if n.lower().endswith(".ext")])
        for name in entries:
            path = os.path.join(ext_dir, name)
            stem = os.path.splitext(name)[0].lower()
            with open(path, "r", encoding="latin1", errors="ignore") as f:
                for raw in f:
                    line = raw.split(";", 1)[0].strip().replace("\x1a", "")
                    if not line:
                        continue
                    m = re.match(r"(?i)^extern\s+([A-Za-z_][A-Za-z0-9_]*)", line)
                    if not m:
                        continue
                    sym = m.group(1).lower()
                    if stem == "shapes":
                        if sym not in self.shape_ids:
                            self.shape_ids[sym] = next_shape
                            next_shape += 1
                        self.symbols.setdefault(sym, self.shape_ids[sym])
                        continue

                    # Strategy object files expose \1_istrat routine symbols.
                    if "strat" in stem or stem in ("mother", "paths", "world", "game", "continue", "endseq"):
                        if sym.endswith("_istrat") and sym not in self.strat_ids:
                            self.strat_ids[sym] = next_strat
                            next_strat += 1
                        self.symbols.setdefault(sym, self.strat_ids.get(sym, next_sym))
                        if sym not in self.strat_ids:
                            next_sym += 1
                        continue

                    self.symbols.setdefault(sym, next_sym)
                    next_sym += 1

    def _load_reference_equ_assign_file(self, path):
        if not os.path.exists(path):
            return
        with open(path, "r", encoding="latin1", errors="ignore") as f:
            for lineno, raw in enumerate(f, start=1):
                line = raw.split(";", 1)[0].strip().replace("\x1a", "")
                if not line:
                    continue
                m = self._EQU_RE.match(line)
                if m:
                    name = m.group(1).lower()
                    expr = m.group(2).strip()
                    value = self._eval_expr(expr, path, lineno, quiet=True)
                    self.symbols[name] = value
                    continue
                m2 = self._ASSIGN_RE.match(line)
                if m2:
                    name = m2.group(1).lower()
                    expr = m2.group(2).strip()
                    value = self._eval_expr(expr, path, lineno, quiet=True)
                    self.symbols[name] = value

    def _load_reference_path_names(self):
        # Build a broad path-name map from START_PATH order in source path files.
        # This avoids synthetic IDs in strict literal mode for late-game maps.
        path_dir = os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "PATH")
        if not os.path.isdir(path_dir):
            return

        names = []
        for rel in ("PATHDATA.ASM", "DPATHDAT.ASM", "KPATHDAT.ASM"):
            p = os.path.join(path_dir, rel)
            if not os.path.exists(p):
                continue
            with open(p, "r", encoding="latin1", errors="ignore") as f:
                for raw in f:
                    line = raw.split(";", 1)[0].strip().replace("\x1a", "")
                    if not line:
                        continue
                    m = re.match(r"(?i)^start_path\s+([A-Za-z0-9_]+)", line)
                    if not m:
                        continue
                    names.append(m.group(1).lower())

        if not names:
            return

        for idx, name in enumerate(names):
            self.path_ids.setdefault(name, idx)
            self.symbols.setdefault(f"path_{name}", idx)
            self.symbols.setdefault(name, idx)
        self.next_auto_path_id = max(self.next_auto_path_id, len(names))

    def _load_reference_bg_symbols(self):
        # MAPMACS setbg emits: dw bg_<name> - bglists.
        # In BGS.ASM, bglists starts with dw+db (3 bytes), then each defbg entry is 6 bytes.
        path = os.path.join(self.repo_root, "reference", "ultrastarfox", "SF", "ASM", "BGS.ASM")
        if not os.path.exists(path):
            return
        offset = 3
        with open(path, "r", encoding="latin1", errors="ignore") as f:
            for raw in f:
                line = raw.split(";", 1)[0].strip().replace("\x1a", "")
                if not line:
                    continue
                m = re.match(r"(?i)^defbg\s+(.+)$", line)
                if not m:
                    continue
                names = [n.strip().lower() for n in m.group(1).split(",") if n.strip()]
                for name in names:
                    self.symbols.setdefault(name, offset)
                    self.symbols.setdefault(f"bg_{name}", offset)
                offset += 6

    def _index_global_map_labels(self, map_dir):
        out = {}
        if not os.path.isdir(map_dir):
            return out
        for name in os.listdir(map_dir):
            if not name.lower().endswith(".asm"):
                continue
            path = os.path.join(map_dir, name)
            labels = self._scan_global_labels(path)
            for label in labels:
                if label not in out:
                    out[label] = path
        return out

    def _scan_global_labels(self, path):
        labels = []
        try:
            with open(path, "r", encoding="latin1", errors="ignore") as f:
                for raw in f:
                    line = raw.split(";", 1)[0].rstrip().replace("\x1a", "")
                    if not line.strip():
                        continue
                    if line.lstrip().startswith("#"):
                        continue
                    stripped = line.lstrip()
                    first = stripped.split(None, 1)[0].lower()
                    if first in self._DIRECTIVES:
                        continue
                    if self._ASSIGN_RE.match(stripped):
                        continue
                    if self._EQU_RE.match(stripped):
                        continue
                    label_name, _stmt = self._split_label_and_stmt(line)
                    if label_name is None:
                        continue
                    if label_name.startswith("."):
                        continue
                    labels.append(label_name.lower())
        except OSError:
            return labels
        return labels

    # ----------------------------------------------------------
    # Diagnostics
    # ----------------------------------------------------------
    def _diag(self, file_path, lineno, message):
        self.diagnostics.append((file_path, lineno, message))

    def _resolve_include_path(self, cur_dir, inc_token):
        inc = inc_token.strip().strip("<>").strip().replace("\\", "/")
        if not inc:
            return os.path.abspath(os.path.join(cur_dir, inc))
        if not inc.lower().endswith(".asm"):
            inc += ".ASM"

        direct = os.path.abspath(os.path.join(cur_dir, inc))
        if os.path.exists(direct):
            return direct

        parts = [p for p in inc.split("/") if p and p != "."]
        probe = os.path.abspath(cur_dir)
        for part in parts:
            try:
                entries = os.listdir(probe)
            except OSError:
                raise FileNotFoundError(direct)
            hit = None
            for name in entries:
                if name.lower() == part.lower():
                    hit = name
                    break
            if hit is None:
                probe = os.path.join(probe, part)
            else:
                probe = os.path.join(probe, hit)
        if os.path.exists(probe):
            return probe
        raise FileNotFoundError(probe)

    # ----------------------------------------------------------
    # Parsing
    # ----------------------------------------------------------
    def _parse_file(self, path):
        path = os.path.abspath(path)
        if path in self._parsed_files:
            return
        if not os.path.exists(path):
            self._diag(path, 0, "File not found")
            return

        self._parsed_files.add(path)
        self._file_prefix[path] = f"f{self._file_seq}_"
        self._file_seq += 1

        cond_stack = []
        cpu_block_depth = 0

        with open(path, "r", encoding="latin1", errors="ignore") as f:
            lines = list(enumerate(f, start=1))

        def process_lines(items):
            nonlocal cpu_block_depth
            i = 0
            while i < len(items):
                lineno, raw = items[i]
                line = raw.replace("\x1a", "").split(";", 1)[0].rstrip()
                if not line.strip():
                    i += 1
                    continue

                stripped = line.lstrip()
                if stripped.startswith("#"):
                    i += 1
                    continue
                first = stripped.split(None, 1)[0].lower()

                if first in self._DIRECTIVES:
                    self._handle_directive(path, lineno, stripped, cond_stack)
                    i += 1
                    continue

                active = cond_stack[-1]["active"] if cond_stack else True

                if first == "rept":
                    tail = stripped.split(None, 1)[1].strip() if " " in stripped else "0"
                    count = self._eval_expr(tail or "0", path, lineno)
                    if count < 0:
                        self._diag(path, lineno, f"Negative REPT count '{count}' clamped to 0")
                        count = 0
                    depth = 1
                    body = []
                    j = i + 1
                    while j < len(items):
                        l2, raw2 = items[j]
                        line2 = raw2.replace("\x1a", "").split(";", 1)[0].rstrip()
                        stripped2 = line2.lstrip()
                        tok2 = stripped2.split(None, 1)[0].lower() if stripped2 else ""
                        if tok2 == "rept":
                            depth += 1
                        elif tok2 == "endr":
                            depth -= 1
                            if depth == 0:
                                break
                        body.append((l2, raw2))
                        j += 1
                    if depth != 0:
                        self._diag(path, lineno, "REPT without matching ENDR")
                        return
                    if active:
                        for _ in range(count):
                            process_lines(body)
                    i = j + 1
                    continue

                if first == "endr":
                    i += 1
                    continue

                if not active:
                    i += 1
                    continue

                m_assign = self._ASSIGN_RE.match(stripped)
                if m_assign:
                    name = m_assign.group(1).lower()
                    value = self._eval_expr(m_assign.group(2), path, lineno)
                    self.symbols[name] = value
                    i += 1
                    continue

                m_equ = self._EQU_RE.match(stripped)
                if m_equ:
                    name = m_equ.group(1).lower()
                    value = self._eval_expr(m_equ.group(2), path, lineno)
                    self.symbols[name] = value
                    i += 1
                    continue

                label_name, stmt = self._split_label_and_stmt(line)
                if label_name is not None:
                    stmt_stripped = stmt.strip() if stmt else ""
                    m_label_equ = re.match(r"(?i)^equ\s+(.+)$", stmt_stripped)
                    if m_label_equ:
                        self.symbols[label_name.lower()] = self._eval_expr(
                            m_label_equ.group(1), path, lineno
                        )
                        i += 1
                        continue
                    canon = self._canon_label(label_name, self._file_prefix[path])
                    if canon not in self.m.labels:
                        self.m.label(canon)
                    if not stmt:
                        i += 1
                        continue
                    stripped = stmt.strip()
                    if not stripped:
                        i += 1
                        continue
                    m_equ = self._EQU_RE.match(stripped)
                    if m_equ:
                        name = label_name.lower()
                        value = self._eval_expr(m_equ.group(2), path, lineno)
                        self.symbols[name] = value
                        i += 1
                        continue

                op_token, op_args = self._split_op_and_args(stripped)
                op_name, op_suffix = self._split_suffix(op_token)

                if cpu_block_depth > 0:
                    if op_name == "start_65816":
                        cpu_block_depth += 1
                    elif op_name == "end_65816":
                        cpu_block_depth -= 1
                    i += 1
                    continue
                if op_name == "start_65816":
                    cpu_block_depth = 1
                    i += 1
                    continue
                if op_name == "end_65816":
                    i += 1
                    continue

                self._emit_statement(
                    path,
                    lineno,
                    op_name,
                    op_suffix,
                    self._split_args(op_args),
                    self._file_prefix[path],
                    os.path.dirname(path),
                )
                i += 1

        process_lines(lines)

    def _handle_directive(self, path, lineno, stripped, cond_stack):
        parts = stripped.split(None, 1)
        directive = parts[0].lower()
        tail = parts[1].strip() if len(parts) > 1 else ""

        parent_active = cond_stack[-1]["active"] if cond_stack else True

        if directive in ("ifeq", "ifne"):
            cond = self._eval_expr(tail or "0", path, lineno)
            is_true = (cond == 0) if directive == "ifeq" else (cond != 0)
            active = parent_active and is_true
            cond_stack.append({"parent": parent_active, "active": active, "taken": active})
            return

        if directive in ("ifd", "ifnd"):
            name = (tail or "").strip().lower()
            defined = name in self.symbols
            is_true = defined if directive == "ifd" else (not defined)
            active = parent_active and is_true
            cond_stack.append({"parent": parent_active, "active": active, "taken": active})
            return

        if directive == "elseif":
            if not cond_stack:
                self._diag(path, lineno, "ELSEIF without matching IF")
                return
            node = cond_stack[-1]
            if node["taken"] or not node["parent"]:
                node["active"] = False
                return
            cond = self._eval_expr(tail or "0", path, lineno)
            is_true = (cond != 0)
            node["active"] = node["parent"] and is_true
            if node["active"]:
                node["taken"] = True
            return

        if directive == "else":
            if not cond_stack:
                self._diag(path, lineno, "ELSE without matching IF")
                return
            node = cond_stack[-1]
            node["active"] = node["parent"] and (not node["taken"])
            node["taken"] = True
            return

        if directive == "endc":
            if not cond_stack:
                self._diag(path, lineno, "ENDC without matching IF")
                return
            cond_stack.pop()

    def _split_label_and_stmt(self, line):
        stripped = line.lstrip()
        leading_ws = len(line) - len(stripped)
        if not stripped:
            return None, ""
        parts = stripped.split(None, 1)
        first = parts[0]
        rest = parts[1] if len(parts) > 1 else ""
        token = first.rstrip(":")
        has_colon = first.endswith(":")
        rest = rest.strip()

        if has_colon:
            return token, rest

        if token.startswith("."):
            if not rest:
                return token, ""
            nxt = rest.split(None, 1)[0]
            if self._looks_like_statement(nxt):
                return token, rest

        if leading_ws == 0 and not self._looks_like_statement(token):
            return token, rest

        return None, stripped

    def _split_op_and_args(self, stmt):
        parts = stmt.strip().split(None, 1)
        if not parts:
            return "", ""
        if len(parts) == 1:
            return parts[0], ""
        return parts[0], parts[1].strip()

    def _split_suffix(self, op_token):
        lower = op_token.lower()
        if "." not in lower:
            return lower, None
        base, suffix = lower.rsplit(".", 1)
        if suffix in ("b", "w", "l", "n"):
            return base, suffix
        return lower, None

    def _split_args(self, raw):
        if not raw:
            return []
        args = []
        cur = []
        depth = 0
        for ch in raw:
            if ch == "," and depth == 0:
                args.append("".join(cur).strip())
                cur = []
                continue
            if ch in "([":
                depth += 1
            elif ch in ")]" and depth > 0:
                depth -= 1
            cur.append(ch)
        args.append("".join(cur).strip())
        return [a for a in args if a != ""]

    def _emit_statement(self, path, lineno, op, suffix, args, file_prefix, cur_dir):
        try:
            if op == "incmap":
                if not args:
                    self._diag(path, lineno, "incmap requires one argument")
                    return
                try:
                    inc_path = self._resolve_include_path(cur_dir, args[0])
                except FileNotFoundError:
                    missing = args[0].strip().lower().rstrip()
                    if missing in ("macro", "macro.asm", "macros", "macros.asm"):
                        return
                    raise
                self._parse_file(inc_path)
                return

            if op in self._QUIET_NOOP_MACROS:
                return

            if op == "meters_off":
                ptr = self._eval_expr("m_meters", path, lineno)
                self.m.setvarb(ptr, 0)
                if args and args[0].strip().lower() == "trans":
                    self.m.mapcodejsl_builtin(MAP_CB_SETCHARMAPFROMMAP_L)
                return

            if op == "meters_on":
                ptr = self._eval_expr("m_meters", path, lineno)
                self.m.setvarb(ptr, 1)
                if args and args[0].strip().lower() == "trans":
                    self.m.mapcodejsl_builtin(MAP_CB_SETCHARMAPFROMMAP_L)
                return

            if op == "setrestart":
                self.m.mapcodejsl_builtin(MAP_CB_SETRESTART_L)
                return

            if op == "markboss":
                self.m.mapcodejsl_builtin(MAP_CB_MARKBOSS_L)
                return

            if op in ("mapplayermode", "mapclplayermode"):
                if not args:
                    self._diag(path, lineno, f"{op} expects a mode argument")
                    return
                self.m.mapcodejsl_builtin(self._resolve_player_mode_callback(args[0], path, lineno))
                return

            if op == "mapgotoifplayerdead":
                if not args:
                    self._diag(path, lineno, "mapgotoifplayerdead expects label")
                    return
                self.m.mapif_builtin(MAP_CB_IS_PLAYER_DEAD, self._canon_label(args[0], file_prefix))
                return

            if op == "mapplayeroutview":
                self._emit_mapplayeroutview(path, lineno)
                return

            if op == "maptexitwait":
                base = 1000
                if args:
                    base += self._eval_expr(args[0], path, lineno)
                self._emit_wait_macro(base, path, lineno)
                return

            if op == "mapend__not":
                self._emit_mapend_not(path, lineno, args, file_prefix)
                return

            if op == "mapmother":
                if len(args) < 7:
                    self._diag(path, lineno, "mapmother expects frame,x,y,z,shape,strategy,map")
                    return
                frame = self._eval_expr(args[0], path, lineno)
                x = self._eval_expr(args[1], path, lineno)
                y = self._eval_expr(args[2], path, lineno)
                z = self._eval_expr(args[3], path, lineno)
                shape = self._resolve_shape_raw(args[4], path, lineno)
                strat = self._resolve_strat_raw(args[5], path, lineno)
                map_ref = self._resolve_map_ref(args[6], path, lineno, file_prefix)
                self.m.mapmother(frame, x, y, z, shape, strat, map_ref)
                return

            if op == "maprem":
                if not args:
                    self._diag(path, lineno, "maprem expects shape or frame,shape")
                    return
                if len(args) == 1:
                    frame = 0
                    shape = self._resolve_shape_raw(args[0], path, lineno)
                else:
                    frame = self._eval_expr(args[0], path, lineno)
                    shape = self._resolve_shape_raw(args[1], path, lineno)
                self.m.maprem(frame, shape)
                return

            if op == "mapobjzrot":
                if len(args) < 7:
                    self._diag(path, lineno, "mapobjzrot expects frame,x,y,z,shape,strat,zrot")
                    return
                frame = self._eval_expr(args[0], path, lineno)
                x = self._eval_expr(args[1], path, lineno)
                y = self._eval_expr(args[2], path, lineno)
                z = self._eval_expr(args[3], path, lineno)
                shape = self._resolve_shape(args[4], path, lineno)
                strat = self._resolve_strat(args[5], path, lineno)
                zrot = self._eval_expr(args[6], path, lineno)
                self.m.mapobjzrot(frame, x, y, z, shape, strat, zrot)
                return

            if op in ("mapobj", "mapobjnomem"):
                if len(args) < 6:
                    self._diag(path, lineno, f"{op} expects at least 6 arguments")
                    return
                frame = self._eval_expr(args[0], path, lineno)
                x = self._eval_expr(args[1], path, lineno)
                y = self._eval_expr(args[2], path, lineno)
                z = self._eval_expr(args[3], path, lineno)
                self._emit_mapobj_literal(frame, x, y, z, args[4], args[5], path, lineno)
                return

            if op == "mapwait":
                if not args:
                    self._diag(path, lineno, "mapwait requires a distance argument")
                    return
                dist = self._eval_expr(args[0], path, lineno)
                self._emit_wait_macro(dist, path, lineno)
                return

            if op == "mapwait2":
                if not args:
                    self._diag(path, lineno, "mapwait2 requires one argument")
                    return
                self.m.mapwait2(self._eval_expr(args[0], path, lineno))
                return

            if op == "maploop":
                if len(args) < 2:
                    self._diag(path, lineno, "maploop expects label,count")
                    return
                label = self._canon_label(args[0], file_prefix)
                count = self._eval_expr(args[1], path, lineno)
                self.m.maploop(label, count)
                return

            if op == "mapjsr":
                if not args:
                    self._diag(path, lineno, "mapjsr expects a label")
                    return
                self.m.mapjsr(self._canon_label(args[0], file_prefix))
                return

            if op == "maprts":
                self.m.maprts()
                return

            if op == "mapgoto":
                if not args:
                    self._diag(path, lineno, "mapgoto expects a label")
                    return
                self.m.mapgoto(self._canon_label(args[0], file_prefix))
                return

            if op in ("mapjmpvarless", "mapjmpvarmore", "mapjmpvareq"):
                if len(args) < 3:
                    self._diag(path, lineno, f"{op} expects var,value,label")
                    return
                ptr = self._eval_expr(args[0], path, lineno)
                value = self._eval_expr(args[1], path, lineno)
                label = self._canon_label(args[2], file_prefix)
                if op == "mapjmpvarless":
                    self.m.mapjmpvarless(ptr, value, label)
                elif op == "mapjmpvarmore":
                    self.m.mapjmpvarmore(ptr, value, label)
                else:
                    self.m.mapjmpvareq(ptr, value, label)
                return

            if op == "mapif":
                if len(args) < 2:
                    self._diag(path, lineno, "mapif expects callback,label")
                    return
                cb = self._resolve_callback(args[0], path, lineno)
                label = self._canon_label(args[1], file_prefix)
                self.m.mapif_builtin(cb, label)
                return

            if op == "mapcode_jsl":
                if not args:
                    self._diag(path, lineno, "mapcode_jsl expects a callback symbol")
                    return
                self.m.mapcodejsl_builtin(self._resolve_callback(args[0], path, lineno))
                return

            if op == "mapsetpath":
                if not args:
                    self._diag(path, lineno, "mapsetpath expects a path symbol")
                    return
                self.m.mapsetpath(self._resolve_path(args[0], path, lineno))
                return

            if op in ("special", "cspecial"):
                if len(args) < 6:
                    self._diag(path, lineno, f"{op} expects 6 arguments")
                    return
                wait = self._eval_expr(args[0], path, lineno)
                x = self._eval_expr(args[1], path, lineno)
                y = self._eval_expr(args[2], path, lineno)
                z = self._eval_expr(args[3], path, lineno)
                self._emit_mapobj_literal(0, x, y, z, args[4], args[5], path, lineno)
                if op == "special":
                    self.m.mapspecial()
                else:
                    self.m.mapcspecial()
                self._emit_wait_macro(wait, path, lineno)
                return

            if op in ("mapspecial",):
                self.m.mapspecial()
                return

            if op in ("mapcspecial",):
                self.m.mapcspecial()
                return

            if op in ("pathobj", "pathspecial", "pathcspecial"):
                if len(args) < 8:
                    self._diag(path, lineno, f"{op} expects 8 arguments")
                    return
                wait = self._eval_expr(args[0], path, lineno)
                x = self._eval_expr(args[1], path, lineno)
                y = self._eval_expr(args[2], path, lineno)
                z = self._eval_expr(args[3], path, lineno)
                path_id = self._resolve_path(args[5], path, lineno)
                hp = self._eval_expr(args[6], path, lineno)
                ap = self._eval_expr(args[7], path, lineno)
                strat = "pathdha_istrat" if (hp == 10 and ap == 10) else "path_istrat"

                self._emit_mapobj_literal(0, x, y, z, args[4], strat, path, lineno)
                if hp != 10 or ap != 10:
                    self.m.setalvarb(AL_HP, hp)
                    self.m.setalvarb(AL_AP, ap)
                self.m.mapsetpath(path_id)
                if op == "pathspecial":
                    self.m.mapspecial()
                elif op == "pathcspecial":
                    self.m.mapcspecial()
                self._emit_wait_macro(wait, path, lineno)
                return

            if op == "setxrot":
                if args:
                    self.m.setxrot(self._eval_expr(args[0], path, lineno))
                return

            if op == "setyrot":
                if args:
                    self.m.setyrot(self._eval_expr(args[0], path, lineno))
                return

            if op == "setzrot":
                if args:
                    self.m.setzrot(self._eval_expr(args[0], path, lineno))
                return

            if op == "setalvar":
                if len(args) < 2:
                    self._diag(path, lineno, "setalvar expects field,value")
                    return
                field = self._resolve_al_field(args[0], path, lineno, is_alx=False)
                value = self._eval_expr(args[1], path, lineno)
                mode = suffix or "b"
                if mode in ("b", "n"):
                    self.m.setalvarb(field, value)
                elif mode == "w":
                    self.m.setalvarw(field, value)
                elif mode == "l":
                    self.m.setalvarl(field, value)
                else:
                    self._diag(path, lineno, f"Unsupported setalvar mode '.{mode}'")
                return

            if op == "setalxvar":
                if len(args) < 2:
                    self._diag(path, lineno, "setalxvar expects field,value")
                    return
                field = self._resolve_al_field(args[0], path, lineno, is_alx=True)
                value = self._eval_expr(args[1], path, lineno)
                mode = suffix or "b"
                if mode in ("b", "n"):
                    self.m.setalxvarb(field, value)
                elif mode == "w":
                    self.m.setalxvarw(field, value)
                elif mode == "l":
                    self.m.setalxvarl(field, value)
                else:
                    self._diag(path, lineno, f"Unsupported setalxvar mode '.{mode}'")
                return

            if op == "setalvarptr":
                if len(args) < 2:
                    self._diag(path, lineno, "setalvarptr expects field,ptr")
                    return
                field = self._resolve_al_field(args[0], path, lineno, is_alx=False)
                ptr = self._eval_expr(args[1], path, lineno)
                mode = suffix or "b"
                if mode in ("b", "n"):
                    self.m.setalvarptrb(field, ptr)
                elif mode == "w":
                    self.m.setalvarptrw(field, ptr)
                else:
                    self._diag(path, lineno, f"Unsupported setalvarptr mode '.{mode}'")
                return

            if op == "addalvarptr":
                if len(args) < 2:
                    self._diag(path, lineno, "addalvarptr expects field,ptr")
                    return
                field = self._resolve_al_field(args[0], path, lineno, is_alx=False)
                ptr = self._eval_expr(args[1], path, lineno)
                mode = suffix or "b"
                if mode in ("b", "n"):
                    self.m.addalvarptrb(field, ptr)
                elif mode == "w":
                    self.m.addalvarptrw(field, ptr)
                else:
                    self._diag(path, lineno, f"Unsupported addalvarptr mode '.{mode}'")
                return

            if op == "setvar":
                if len(args) < 2:
                    self._diag(path, lineno, "setvar expects var,value")
                    return
                ptr = self._eval_expr(args[0], path, lineno)
                value = self._eval_expr(args[1], path, lineno)
                mode = suffix or "b"
                if mode in ("b", "n"):
                    self.m.setvarb(ptr, value)
                elif mode == "w":
                    self.m.setvarw(ptr, value)
                elif mode == "l":
                    self.m.setvarl(ptr, value)
                else:
                    self._diag(path, lineno, f"Unsupported setvar mode '.{mode}'")
                return

            if op == "setvarobj":
                if not args:
                    self._diag(path, lineno, "setvarobj expects ptr")
                    return
                self.m.setvarobj(self._eval_expr(args[0], path, lineno))
                return

            if op == "setbgm":
                if not args:
                    self._diag(path, lineno, "setbgm expects one argument")
                    return
                self.m.setbgm(self._eval_expr(args[0], path, lineno))
                return

            if op == "setbg":
                if not args:
                    self._diag(path, lineno, "setbg expects one argument")
                    return
                self.m.setbg(self._eval_expr(args[0], path, lineno))
                return

            if op == "initbg":
                self.m.waitsetbg()
                self.m.setbginfo()
                return

            if op == "setstage":
                self.m.setstage()
                return

            if op == "setfadeup":
                if args:
                    self.m.qfadeup()
                else:
                    self.m.fadeup()
                return

            if op == "setfadedown":
                if args:
                    self.m.qfadedown()
                else:
                    self.m.fadedown()
                return

            if op == "mapwaitfade":
                self.m.waitfade()
                return

            if op == "mapfadetosea":
                self.m.fadetosea()
                return

            if op == "mapfadetoground":
                self.m.fadetoground()
                return

            if op == "mapmsg":
                if not args:
                    self._diag(path, lineno, "mapmsg expects one argument")
                    return
                self.m.sendmsg(self._eval_expr(args[0], path, lineno))
                return

            if op == "printlevelfin":
                ptr = self._eval_expr("levelfinished", path, lineno)
                self.m.setvarb(ptr, 3)
                return

            if op == "mapjmpfrienddead":
                if len(args) < 2:
                    self._diag(path, lineno, "mapjmpfrienddead expects friend,label")
                    return
                cb = self._resolve_friend_alive_callback(args[0], path, lineno)
                dead_label = self._canon_label(args[1], file_prefix)
                ok_label = f"__mapjmpfrienddead_ok_{self._macro_seq}"
                self._macro_seq += 1
                self.m.mapif_builtin(cb, ok_label)
                if len(args) < 3:
                    self._emit_wait_macro(MEDPSPEED * 30, path, lineno)
                self.m.mapgoto(dead_label)
                self.m.label(ok_label)
                return

            if op == "clfriendmsg":
                if not args:
                    self._diag(path, lineno, "clfriendmsg expects friend")
                    return
                self.m.mapcodejsl_builtin(self._resolve_clfriendmsg_callback(args[0], path, lineno))
                return

            if op == "fadeoutbgm":
                self.m.setbgm(0xF1)
                self._emit_wait_macro(MEDPSPEED * 30, path, lineno)
                return

            if op == "mapwaitboss":
                self._emit_mapwaitboss(path, lineno, args)
                return

            if op == "wipein":
                # Keep this literal but minimal: callback + timed wait.
                self.m.mapcodejsl_builtin(MAP_CB_INITBLACK_L)
                self._emit_wait_macro(300, path, lineno)
                return

            if op == "mapexploderobot":
                self.m.mapcodejsl_builtin(MAP_CB_KILL_ROBOT_L)
                return

            if op == "skillfly_init":
                # setvar.b skillfly,0 - keep explicit opcode path.
                ptr = self._eval_expr("skillfly", path, lineno)
                self.m.setvarb(ptr, 0)
                return

            if op == "skillfly_set":
                if len(args) < 3:
                    self._diag(path, lineno, "skillfly_set expects at least x,y,z")
                    return
                x = self._eval_expr(args[0], path, lineno)
                y = self._eval_expr(args[1], path, lineno)
                z = self._eval_expr(args[2], path, lineno)
                strat = self._resolve_strat("skillfly_istrat", path, lineno)
                self.m.mapobj(0, x, y, z, self._resolve_shape("nullshape", path, lineno), strat)
                if len(args) >= 4:
                    self.m.setalvarw(AL_SWORD1, self._eval_expr(args[3], path, lineno))
                return

            if op == "skillfly_bonus":
                if len(args) < 6:
                    self._diag(path, lineno, "skillfly_bonus expects wait,x,y,z,shape,istrat")
                    return
                wait = self._eval_expr(args[0], path, lineno)
                x = self._eval_expr(args[1], path, lineno)
                y = self._eval_expr(args[2], path, lineno)
                z = self._eval_expr(args[3], path, lineno)
                shape = self._resolve_shape(args[4], path, lineno)
                strat = self._resolve_strat(args[5], path, lineno)
                self.m.mapobj(wait, x, y, z, shape, strat)
                return

            if op == "maphardrot":
                if len(args) < 8:
                    self._diag(path, lineno, "maphardrot expects wait,x,y,z,shape,rx,ry,rz")
                    return
                wait = self._eval_expr(args[0], path, lineno)
                x = self._eval_expr(args[1], path, lineno)
                y = self._eval_expr(args[2], path, lineno)
                z = self._eval_expr(args[3], path, lineno)
                shape = self._resolve_shape(args[4], path, lineno)
                strat = self._resolve_strat("hardrot_istrat", path, lineno)
                self.m.mapobj(0, x, y, z, shape, strat)
                self.m.setalvarb(AL_SBYTE1, self._eval_expr(args[5], path, lineno))
                self.m.setalvarb(AL_SBYTE2, self._eval_expr(args[6], path, lineno))
                self.m.setalvarb(AL_SBYTE3, self._eval_expr(args[7], path, lineno))
                self._emit_wait_macro(wait, path, lineno)
                return

            if op == "mapend":
                self.m.mapend()
                return

            if op == "mapendwipe":
                if len(args) < 2:
                    self._diag(path, lineno, "mapendwipe expects circle,wait")
                    return
                self.m.setbgm(0xF1)
                self._emit_wait_macro(self._eval_expr(args[1], path, lineno), path, lineno)
                return

            if self._emit_composite_macro(op, args, path, lineno, file_prefix):
                return

            self._diag(path, lineno, f"Unsupported statement '{op_token_display(op, suffix)}'")
        except Exception as exc:
            self._diag(path, lineno, f"Failed to parse statement '{op_token_display(op, suffix)}': {exc}")

    # ----------------------------------------------------------
    # Emit helpers
    # ----------------------------------------------------------
    def _emit_wait_macro(self, dist, path, lineno):
        if dist == 0:
            return
        if dist < 0:
            return
        if (dist >> 4) < 256:
            self.m.mapwait2((dist >> 4) & 0xFF)
        else:
            self.m.mapwait(dist)

    def _emit_mapwaitboss(self, path, lineno, args):
        self._emit_wait_macro(100, path, lineno)
        loop = f"__mwb_loop_{self._macro_seq}"
        done = f"__mwb_done_{self._macro_seq}"
        self._macro_seq += 1
        self.m.label(loop)
        self.m.mapif_builtin(MAP_CB_CHKBOSSDEAD, done)
        self.m.mapgoto(loop)
        self.m.label(done)

        # Macro variant: mapwaitboss nosound -> no bgm change.
        if args:
            arg0 = args[0].strip().lower()
            if arg0 == "nosound":
                return
            self.m.setbgm(self._eval_expr(args[0], path, lineno))
            return
        self.m.setbgm(0xF1)

    def _emit_mapplayeroutview(self, path, lineno):
        loop = f"__mpov_wait_{self._macro_seq}"
        self._macro_seq += 1
        self.m.label(loop)
        self._emit_wait_macro(5, path, lineno)
        self.m.mapif_builtin(MAP_CB_IS_PLAYER_DEAD, loop)
        self.m.mapcodejsl_builtin(MAP_CB_PLAYER_OUTVIEW_L)
        self._emit_wait_macro(2000, path, lineno)

    def _emit_mapend_not(self, path, lineno, args, file_prefix):
        if not args:
            self._diag(path, lineno, "mapend__not expects a target map label")
            return
        ptr = self._eval_expr("levelfinished", path, lineno)
        self.m.setvarb(ptr, 7)
        loop = f"__mapend_not_loop_{self._macro_seq}"
        self._macro_seq += 1
        self.m.label(loop)
        self.m.mapif_builtin(MAP_CB_LEVELFINISHED_ZERO, self._canon_label(args[0], file_prefix))
        self._emit_wait_macro(1, path, lineno)
        self.m.mapgoto(loop)

    def _sb_sym(self, name):
        return int(self.symbols.get(name, 0))

    def _sb_shape(self, prefix):
        return f"{prefix}{self._spacebar_style}spacebar"

    def _sb_reset(self, path, lineno, args):
        mode = args[0].strip().lower() if args else "solid"
        if mode not in ("solid", "wire"):
            mode = "solid"
        self._spacebar_style = mode
        self._spacebar_pos = 0
        self._spacebar_swait = 0
        self._spacebar_dowait_z = 1
        if len(args) >= 2 and args[1].strip().lower() == "autowait":
            self._spacebar_dowait_z = 0

        sx = self._eval_expr("sxspacebarlen", path, lineno, quiet=True) or 250
        sy = self._eval_expr("syspacebarlen", path, lineno, quiet=True) or 250
        sz = self._eval_expr("szspacebarlen", path, lineno, quiet=True) or 250
        self.symbols["xlen"] = sx // 2
        self.symbols["ylen"] = sy // 2
        self.symbols["zlen"] = sz // 2
        self.symbols["clen"] = self.symbols["xlen"]
        self.symbols["dist"] = 3000
        self.symbols["swait"] = 0
        self.symbols["dowaitz"] = self._spacebar_dowait_z

    def _sb_wait(self, wait_steps, path, lineno):
        zlen = self._sb_sym("zlen") or 125
        self._emit_wait_macro(wait_steps * zlen, path, lineno)

    def _sb_calcwait(self, z_step, path, lineno):
        # Literal MAPMACS behavior: only emits waits when dowaitZ == 0.
        if self._spacebar_dowait_z != 0:
            return
        if z_step > self._spacebar_swait:
            zlen = self._sb_sym("zlen") or 125
            self._emit_wait_macro((z_step - self._spacebar_swait) * zlen, path, lineno)
            self._spacebar_swait = z_step
            self.symbols["swait"] = self._spacebar_swait

    def _emit_spacebar_axis(self, prefix, x, y, z, path, lineno, strat_name="spacebar_istrat"):
        xlen = self._sb_sym("xlen") or 125
        ylen = self._sb_sym("ylen") or 125
        zlen = self._sb_sym("zlen") or 125
        dist = self._sb_sym("dist") or 3000
        z_part = (z * zlen) * self._spacebar_dowait_z
        shape = self._resolve_shape(self._sb_shape(prefix), path, lineno)
        strat = self._resolve_strat(strat_name, path, lineno)
        self.m.mapobj(0, x * xlen, self._eval_expr("space_viewcy", path, lineno, quiet=True) + (y * ylen),
                      dist + z_part, shape, strat)
        self._sb_calcwait(z, path, lineno)

    def _emit_ttruck(self, x, z, a, path, lineno):
        tsize = self._eval_expr("tsize", path, lineno, quiet=True) or 200
        self.m.mapobj(
            0,
            -500 + (tsize * x),
            0,
            4096 + (z * tsize),
            self._resolve_shape("truck", path, lineno),
            self._resolve_strat("truck_istrat", path, lineno),
        )
        self.m.setalvarb(AL_ROTY, a)

    def _emit_thoriz(self, x, z, path, lineno):
        tsize = self._eval_expr("tsize", path, lineno, quiet=True) or 200
        self.m.mapobj(
            0,
            -500 + (tsize * x),
            0,
            4096 + (z * tsize),
            self._resolve_shape("rail_0", path, lineno),
            self._resolve_strat("nocoll_istrat", path, lineno),
        )

    def _emit_tvert(self, x, z, path, lineno):
        self._emit_thoriz(x, z, path, lineno)
        self.m.setalvarb(AL_ROTY, self._eval_expr("deg90", path, lineno, quiet=True))

    def _emit_tcorner(self, x, z, a, turn_flag, path, lineno):
        tsize = self._eval_expr("tsize", path, lineno, quiet=True) or 200
        self.m.mapobj(
            0,
            -500 + (tsize * x),
            0,
            4096 + (z * tsize),
            self._resolve_shape("rail_4", path, lineno),
            self._resolve_strat("trackcorner_istrat", path, lineno),
        )
        self.m.setalvarb(AL_ROTY, a)
        self.m.setalvarb(AL_SBYTE1, turn_flag)

    def _emit_mapobj_literal(self, frame, x, y, z, shape_token, strat_token, path, lineno):
        if self._can_compact_shape(shape_token, path, lineno) and self._can_compact_strat(strat_token, path, lineno):
            shape = self._resolve_shape(str(shape_token), path, lineno)
            strat = self._resolve_strat(str(strat_token), path, lineno)
            self.m.mapobj(frame, x, y, z, shape, strat)
            return

        shape = self._resolve_shape_raw(str(shape_token), path, lineno)
        strat = self._resolve_strat_raw(str(strat_token), path, lineno)
        self.m.mapnobj(frame, x, y, z, shape, strat)

    def _emit_composite_macro(self, op, args, path, lineno, file_prefix):
        op = op.lower()

        def e(arg):
            return self._eval_expr(arg, path, lineno)

        def emit(name, vals):
            return self._emit_composite_macro(name, [str(v) for v in vals], path, lineno, file_prefix)

        if op == "map_setbarshape":
            self._sb_reset(path, lineno, args)
            return True

        if op == "map_spacebarwait":
            if not args:
                self._diag(path, lineno, "map_spacebarwait expects wait")
                return True
            self._sb_wait(e(args[0]), path, lineno)
            return True

        if op in ("map_xspacebar", "map_yspacebar", "map_zspacebar", "map_sxspacebar", "map_syspacebar", "map_szspacebar"):
            if len(args) < 3:
                self._diag(path, lineno, f"{op} expects x,y,z")
                return True
            prefix = {
                "map_xspacebar": "x",
                "map_yspacebar": "y",
                "map_zspacebar": "z",
                "map_sxspacebar": "sx",
                "map_syspacebar": "sy",
                "map_szspacebar": "sz",
            }[op]
            self._emit_spacebar_axis(prefix, e(args[0]), e(args[1]), e(args[2]), path, lineno)
            return True

        if op in ("map_ryspacebar", "map_sryspacebar"):
            if len(args) < 4:
                self._diag(path, lineno, f"{op} expects x,y,z,rot")
                return True
            base = "map_xspacebar" if op == "map_ryspacebar" else "map_sxspacebar"
            emit(base, [e(args[0]), e(args[1]), e(args[2])])
            self.m.setalvarb(AL_ROTY, e(args[3]) * self._eval_expr("deg45", path, lineno, quiet=True))
            self._sb_calcwait(e(args[2]), path, lineno)
            return True

        if op in ("map_spacebarc", "map_spacebaric", "map_spacebarsc"):
            if len(args) < 5:
                self._diag(path, lineno, f"{op} expects x,y,z,init,speed")
                return True
            x = e(args[0])
            y = e(args[1])
            z = e(args[2])
            init = e(args[3])
            speed = e(args[4])
            clen = self._sb_sym("clen") or 125
            dist = self._sb_sym("dist") or 3000
            shape_prefix = "xp" if op != "map_spacebarsc" else "sxp"
            shape_name = self._sb_shape(shape_prefix)
            z_world = dist + (z * clen)
            if op == "map_spacebarsc":
                z_world = dist + z
            self.m.mapobj(
                0,
                x * clen,
                self._eval_expr("space_viewcy", path, lineno, quiet=True) + (y * clen),
                z_world,
                self._resolve_shape(shape_name, path, lineno),
                self._resolve_strat("spacebar1_istrat", path, lineno),
            )
            self.m.setalvarb(AL_ROTZ, init * self._eval_expr("deg45", path, lineno, quiet=True))
            self.m.setalvarb(AL_SBYTE1, speed)
            if op == "map_spacebaric":
                self.m.setalvarb(AL_SBYTE2, 1)
            self.m.setvarobj(self._eval_expr("mapvar1", path, lineno))
            self._spacebar_pos = z * clen
            return True

        if op in ("map_spacebarx", "map_spacebarz", "map_spacebarsx", "map_spacebarsz"):
            if len(args) < 4:
                self._diag(path, lineno, f"{op} expects x,y,z,init")
                return True
            x = e(args[0])
            y = e(args[1])
            z = e(args[2])
            init = e(args[3])
            clen = self._sb_sym("clen") or 125
            dist = self._sb_sym("dist") or 3000
            shape_prefix = {
                "map_spacebarx": "xp",
                "map_spacebarz": "z",
                "map_spacebarsx": "sxp",
                "map_spacebarsz": "sz",
            }[op]
            self.m.mapobj(
                0,
                x * clen,
                self._eval_expr("space_viewcy", path, lineno, quiet=True) + (y * clen),
                dist + (z * clen) + self._spacebar_pos,
                self._resolve_shape(self._sb_shape(shape_prefix), path, lineno),
                self._resolve_strat("spacebar3_istrat", path, lineno),
            )
            self.m.setalvarb(AL_ROTZ, init * self._eval_expr("deg45", path, lineno, quiet=True))
            self.m.setalvarptrw(AL_PTR, self._eval_expr("mapvar1", path, lineno))
            return True

        if op in ("map_xpspacebar", "map_sxpspacebar"):
            if len(args) < 6:
                self._diag(path, lineno, f"{op} expects wait,x,y,z,init,speed")
                return True
            wait = e(args[0])
            x = e(args[1])
            y = e(args[2])
            z = e(args[3])
            init = e(args[4])
            speed = e(args[5])
            xlen = self._sb_sym("xlen") or 125
            ylen = self._sb_sym("ylen") or 125
            zlen = self._sb_sym("zlen") or 125
            dist = self._sb_sym("dist") or 3000
            shape_prefix = "xp" if op == "map_xpspacebar" else "sxp"
            self.m.mapobj(
                0,
                x * xlen,
                self._eval_expr("space_viewcy", path, lineno, quiet=True) + (y * ylen),
                dist + (z * zlen),
                self._resolve_shape(self._sb_shape(shape_prefix), path, lineno),
                self._resolve_strat("spinspacebar_istrat", path, lineno),
            )
            self.m.setalvarb(AL_ROTZ, init * self._eval_expr("deg45", path, lineno, quiet=True))
            self.m.setalvarb(AL_SBYTE1, speed)
            self._sb_wait(wait, path, lineno)
            return True

        if op == "map_sbtypeobj":
            if len(args) < 6:
                self._diag(path, lineno, "map_sbtypeobj expects wait,x,y,z,shape,strat")
                return True
            xlen = self._sb_sym("xlen") or 125
            ylen = self._sb_sym("ylen") or 125
            zlen = self._sb_sym("zlen") or 125
            dist = self._sb_sym("dist") or 3000
            self.m.mapobj(
                e(args[0]) * zlen,
                e(args[1]) * xlen,
                self._eval_expr("space_viewcy", path, lineno, quiet=True) + (e(args[2]) * ylen),
                dist + (e(args[3]) * zlen),
                self._resolve_shape(args[4], path, lineno),
                self._resolve_strat(args[5], path, lineno),
            )
            return True

        if op == "map_sbtype0":
            return emit("map_yspacebar", [e(args[1]), e(args[2]), e(args[3])]) and emit("map_spacebarwait", [e(args[0])])
        if op == "map_sbtype1":
            return emit("map_xspacebar", [e(args[1]), e(args[2]), e(args[3])]) and emit("map_spacebarwait", [e(args[0])])
        if op == "map_sbtype2":
            emit("map_xspacebar", [e(args[1]), e(args[2]), e(args[3])])
            emit("map_yspacebar", [e(args[1]), e(args[2]), e(args[3])])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype3":
            emit("map_spacebarc", [e(args[1]), e(args[2]), e(args[3]), 0, -6])
            emit("map_spacebarx", [e(args[1]), e(args[2]), 0, 2])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype4":
            emit("map_xspacebar", [e(args[1]), e(args[2]), e(args[3])])
            emit("map_yspacebar", [e(args[1]), e(args[2]) + 2, e(args[3])])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype5":
            emit("map_zspacebar", [e(args[1]), e(args[2]), e(args[3])])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype6":
            emit("map_xpspacebar", [e(args[0]), e(args[1]), e(args[2]), e(args[3]), 2, -4])
            return True
        if op == "map_sbtype7":
            emit("map_spacebarc", [e(args[1]), e(args[2]), e(args[3]), 0, 4])
            emit("map_spacebarx", [e(args[1]) + 2, e(args[2]), 0, 2])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype8":
            emit("map_szspacebar", [e(args[1]), e(args[2]), e(args[3])])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtypea":
            emit("map_sxpspacebar", [e(args[0]), e(args[1]), e(args[2]), e(args[3]), 2, -4])
            return True
        if op == "map_sbtypeb":
            emit("map_syspacebar", [e(args[1]), e(args[2]), e(args[3])])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtypec":
            emit("map_sxspacebar", [e(args[1]), e(args[2]), e(args[3])])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtyped":
            emit("map_xspacebar", [e(args[1]) + 2, e(args[2]) - 2, e(args[3])])
            emit("map_yspacebar", [e(args[1]) + 3, e(args[2]), e(args[3])])
            emit("map_sxspacebar", [e(args[1]) + 2, e(args[2]), e(args[3])])
            emit("map_zspacebar", [e(args[1]) + 3, e(args[2]) + 2, e(args[3]) + 2])
            emit("map_xspacebar", [e(args[1]) + 5, e(args[2]) + 2, e(args[3]) + 4])
            emit("map_yspacebar", [e(args[1]) + 3, e(args[2]) + 1, e(args[3]) + 4])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtypee":
            emit("map_xspacebar", [e(args[1]) - 1, e(args[2]), e(args[3])])
            emit("map_xspacebar", [e(args[1]) - 5, e(args[2]), e(args[3])])
            emit("map_yspacebar", [e(args[1]) - 7, e(args[2]) + 1, e(args[3])])
            emit("map_xspacebar", [e(args[1]) - 9, e(args[2]) - 1, e(args[3])])
            emit("map_xspacebar", [e(args[1]) - 9, e(args[2]) + 3, e(args[3])])
            emit("map_zspacebar", [e(args[1]) - 7, e(args[2]), e(args[3]) + 2])
            emit("map_syspacebar", [e(args[1]) - 7, e(args[2]), e(args[3]) + 4])
            emit("map_sxspacebar", [e(args[1]) - 8, e(args[2]) - 1, e(args[3])])
            emit("map_sxspacebar", [e(args[1]) - 8, e(args[2]) + 1, e(args[3])])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtypef":
            emit("map_xspacebar", [e(args[1]), e(args[2]), e(args[3])])
            emit("map_ryspacebar", [e(args[1]) + 2, e(args[2]), e(args[3]), 3])
            emit("map_ryspacebar", [e(args[1]) - 1, e(args[2]), e(args[3]) + 3, 3])
            emit("map_xspacebar", [e(args[1]) - 2, e(args[2]), e(args[3]) + 6])
            emit("map_xspacebar", [e(args[1]) + 2, e(args[2]), e(args[3]) + 6])
            emit("map_ryspacebar", [e(args[1]) + 4, e(args[2]), e(args[3]) + 6, 3])
            emit("map_ryspacebar", [e(args[1]) + 1, e(args[2]), e(args[3]) + 9, 3])
            emit("map_xspacebar", [e(args[1]), e(args[2]), e(args[3]) + 12])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype10":
            emit("map_spacebarc", [e(args[1]), e(args[2]), e(args[3]), 0, 6])
            emit("map_spacebarx", [e(args[1]), e(args[2]), 0, 2])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype11":
            emit("map_yspacebar", [e(args[1]), e(args[2]), e(args[3])])
            emit("map_xspacebar", [e(args[1]) + 2, e(args[2]) - 2, e(args[3])])
            emit("map_xspacebar", [e(args[1]) + 2, e(args[2]) + 2, e(args[3])])
            emit("map_yspacebar", [e(args[1]) + 4, e(args[2]) + 2, e(args[3])])
            emit("map_zspacebar", [e(args[1]), e(args[2]) - 2, e(args[3]) - 2])
            emit("map_zspacebar", [e(args[1]), e(args[2]) + 2, e(args[3]) + 2])
            emit("map_syspacebar", [e(args[1]), e(args[2]) + 1, e(args[3]) + 4])
            emit("map_zspacebar", [e(args[1]) + 4, e(args[2]) - 2, e(args[3]) + 2])
            emit("map_sxspacebar", [e(args[1]) + 3, e(args[2]) - 2, e(args[3]) + 4])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype12":
            emit("map_yspacebar", [e(args[1]), e(args[2]), e(args[3])])
            emit("map_yspacebar", [e(args[1]) + 4, e(args[2]), e(args[3])])
            emit("map_xspacebar", [e(args[1]) + 2, e(args[2]) - 2, e(args[3])])
            emit("map_xspacebar", [e(args[1]) + 2, e(args[2]) + 2, e(args[3])])
            emit("map_syspacebar", [e(args[1]), e(args[2]) - 3, e(args[3])])
            emit("map_szspacebar", [e(args[1]), e(args[2]) - 2, e(args[3]) + 1])
            emit("map_zspacebar", [e(args[1]), e(args[2]) + 2, e(args[3]) + 2])
            emit("map_syspacebar", [e(args[1]), e(args[2]) + 2, e(args[3]) + 4])
            emit("map_szspacebar", [e(args[1]) + 1, e(args[2]) + 2, e(args[3]) + 4])
            emit("map_ryspacebar", [e(args[1]) + 4, e(args[2]) - 2, e(args[3]), 3])
            emit("map_szspacebar", [e(args[1]) + 4, e(args[2]) - 2, e(args[3]) - 1])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype13":
            emit("map_xpspacebar", [e(args[0]), e(args[1]), e(args[2]), e(args[3]), 2, 4])
            return True
        if op == "map_sbtype14":
            emit("map_spacebarc", [e(args[1]), e(args[2]), e(args[3]), 0, -3])
            emit("map_spacebarsx", [e(args[1]) - 2, e(args[2]) - 1, 0, 2])
            emit("map_spacebarz", [e(args[1]) + 2, e(args[2]), 2, 0])
            emit("map_spacebarx", [e(args[1]) + 2, e(args[2]) + 2, 0, 2])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype15":
            if len(args) < 6:
                self._diag(path, lineno, "map_sbtype15 expects wait,x,y,z,init,speed")
                return True
            emit("map_spacebaric", [e(args[1]), e(args[2]), e(args[3]), e(args[4]), e(args[5])])
            emit("map_spacebarx", [e(args[1]), e(args[2]) - 2, 0, e(args[4])])
            emit("map_spacebarx", [e(args[1]), e(args[2]) + 1, 0, e(args[4])])
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype16":
            if len(args) < 6:
                self._diag(path, lineno, "map_sbtype16 expects wait,x,y,z,xvel,spin")
                return True
            xlen = self._sb_sym("xlen") or 125
            ylen = self._sb_sym("ylen") or 125
            zlen = self._sb_sym("zlen") or 125
            dist = self._sb_sym("dist") or 3000
            spin = e(args[5])
            shape_prefix = "x" if spin == 0 else "xp"
            self.m.mapobj(
                0,
                e(args[1]) * xlen,
                self._eval_expr("space_viewcy", path, lineno, quiet=True) + (e(args[2]) * ylen),
                dist + (e(args[3]) * zlen),
                self._resolve_shape(self._sb_shape(shape_prefix), path, lineno),
                self._resolve_strat("spacebarshoot_istrat", path, lineno),
            )
            self.m.setalvarw(AL_SWORD1, e(args[4]))
            self.m.setalvarb(AL_SBYTE1, spin)
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype17":
            if len(args) < 6:
                self._diag(path, lineno, "map_sbtype17 expects wait,x,y,z,yvel,spin")
                return True
            xlen = self._sb_sym("xlen") or 125
            ylen = self._sb_sym("ylen") or 125
            zlen = self._sb_sym("zlen") or 125
            dist = self._sb_sym("dist") or 3000
            spin = e(args[5])
            shape_prefix = "y" if spin == 0 else "xp"
            self.m.mapobj(
                0,
                e(args[1]) * xlen,
                self._eval_expr("space_viewcy", path, lineno, quiet=True) + (e(args[2]) * ylen),
                dist + (e(args[3]) * zlen),
                self._resolve_shape(self._sb_shape(shape_prefix), path, lineno),
                self._resolve_strat("spacebarshoot_istrat", path, lineno),
            )
            if spin != 0:
                self.m.setalvarb(AL_ROTZ, self._eval_expr("deg90", path, lineno, quiet=True))
            self.m.setalvarw(AL_SWORD2, e(args[4]))
            self.m.setalvarb(AL_SBYTE1, spin)
            emit("map_spacebarwait", [e(args[0])])
            return True
        if op == "map_sbtype18":
            emit("map_xpspacebar", [e(args[0]), e(args[1]), e(args[2]), e(args[3]), e(args[4]), e(args[5])])
            return True
        if op == "map_sbtype19":
            emit("map_sxpspacebar", [e(args[0]), e(args[1]), e(args[2]), e(args[3]), e(args[4]), e(args[5])])
            return True

        if op == "szaco2_mapobj":
            if len(args) < 5:
                self._diag(path, lineno, "szaco2_mapobj expects x,y,tox,toy,wait")
                return True
            x = e(args[0])
            y = e(args[1])
            self.m.mapobj(0, x, y, 2000, self._resolve_shape("zaco_8", path, lineno),
                          self._resolve_strat("szaco2_istrat", path, lineno))
            self.m.setalxvarw(ALX_SWPX1, e(args[2]))
            self.m.setalxvarw(ALX_SWPY1, e(args[3]))
            self.m.setalvarb(AL_ROTX, -self._eval_expr("deg90", path, lineno, quiet=True))
            if x == 0:
                self.m.setalvarb(AL_ROTY, self._eval_expr("deg180", path, lineno, quiet=True))
            elif x < 0:
                self.m.setalvarb(AL_ROTY, -self._eval_expr("deg90", path, lineno, quiet=True))
            else:
                self.m.setalvarb(AL_ROTY, self._eval_expr("deg90", path, lineno, quiet=True))
            if e(args[4]) != 0:
                self._emit_wait_macro(e(args[4]), path, lineno)
            return True

        if op == "roottree":
            if len(args) < 5:
                self._diag(path, lineno, "roottree expects wait,x,y,z,rotz")
                return True
            self.m.mapobj(0, e(args[1]), e(args[2]), e(args[3]),
                          self._resolve_shape("stalk", path, lineno),
                          self._resolve_strat("tree3_istrat", path, lineno))
            self.m.setalvarb(AL_SBYTE2, e(args[4]))
            self._emit_wait_macro(e(args[0]), path, lineno)
            return True

        if op == "nessie":
            if len(args) < 6:
                self._diag(path, lineno, "nessie expects wait,x,y,z,roty,tail_delay")
                return True
            self.m.mapobj(0, e(args[1]), e(args[2]), e(args[3]),
                          self._resolve_shape("nullshape", path, lineno),
                          self._resolve_strat("lochnessmonster_istrat", path, lineno))
            self.m.setalvarb(AL_ROTY, e(args[4]))
            self.m.setalvarw(AL_SWORD1 + 1, e(args[5]))
            self._emit_wait_macro(e(args[0]), path, lineno)
            return True

        if op in ("map_farships0", "map_farships1", "map_farships2"):
            if len(args) < 6:
                self._diag(path, lineno, f"{op} expects x,y,z,xspd,yspd,depth")
                return True
            shape = {"map_farships0": "ship_s_0", "map_farships1": "ship_s_1", "map_farships2": "ships"}[op]
            self.m.mapobj(0x300, e(args[0]), self._eval_expr("space_viewcy", path, lineno, quiet=True) + e(args[1]),
                          e(args[2]), self._resolve_shape(shape, path, lineno),
                          self._resolve_strat("ships_istrat", path, lineno))
            self.m.setalvarw(AL_SWORD1, e(args[3]))
            self.m.setalvarw(AL_SWORD2, e(args[4]))
            self.m.setalxvarw(ALX_DEPTHOFFSET, e(args[5]))
            if op != "map_farships2":
                self.m.setalvarb(AL_ROTY, self._eval_expr("deg180", path, lineno, quiet=True))
            return True

        if op == "map_sfish":
            if len(args) < 5:
                self._diag(path, lineno, "map_sfish expects wait,x,y,z,count")
                return True
            n = max(1, e(args[4]))
            self.m.mapobj(0, e(args[1]), e(args[2]), e(args[3]),
                          self._resolve_shape("s_fish", path, lineno),
                          self._resolve_strat("sfish_istrat", path, lineno))
            self.m.setvarobj(self._eval_expr("mapvar1", path, lineno))
            for _ in range(n - 1):
                self.m.mapobj(0, 0, 0, 4000,
                              self._resolve_shape("s_fish", path, lineno),
                              self._resolve_strat("sfish_istrat", path, lineno))
                self.m.setalvarptrw(AL_PTR, self._eval_expr("mapvar1", path, lineno))
            self._emit_wait_macro(e(args[0]), path, lineno)
            return True

        if op == "mapplayeroutdist":
            sptr = self._eval_expr("splayerflymode", path, lineno, quiet=True)
            spfm_tonorm = self._eval_expr("spfm_tonorm", path, lineno, quiet=True) or 4
            vptr = self._eval_expr("viewdist", path, lineno, quiet=True)
            outdist = self._eval_expr("outviewdist", path, lineno, quiet=True) or 120
            self.m.setvarb(sptr, spfm_tonorm)
            self.m.setvarw(vptr, outdist)
            return True

        if op == "maplrdoor":
            if len(args) < 2:
                self._diag(path, lineno, "maplrdoor expects wait,z")
                return True
            self._emit_mapobj_literal(0, -45, -60, e(args[1]), "open_l", "openlr_istrat", path, lineno)
            self._emit_mapobj_literal(0, 45, -60, e(args[1]), "open_l", "openlr_istrat", path, lineno)
            self.m.setalvarb(AL_ROTZ, self._eval_expr("deg180", path, lineno, quiet=True))
            self._emit_wait_macro(e(args[0]), path, lineno)
            return True

        if op == "mapupdndoor":
            if len(args) < 2:
                self._diag(path, lineno, "mapupdndoor expects wait,z")
                return True
            self._emit_mapobj_literal(e(args[0]), 0, -60, e(args[1]), "up_door", "updoor_istrat", path, lineno)
            return True

        if op == "mapdnupdoor":
            if len(args) < 2:
                self._diag(path, lineno, "mapdnupdoor expects wait,z")
                return True
            self._emit_mapobj_literal(0, 0, -60, e(args[1]), "up_door", "updoor_istrat", path, lineno)
            self.m.setalvarb(AL_ROTZ, self._eval_expr("deg180", path, lineno, quiet=True))
            self._emit_wait_macro(e(args[0]), path, lineno)
            return True

        if op == "mappipe":
            if len(args) < 5:
                self._diag(path, lineno, "mappipe expects y,z,rotx_step,rotz_step,pipe_idx[,nognd]")
                return True
            pipescale = self._eval_expr("pipescale", path, lineno, quiet=True) or 1
            pdist = self._eval_expr("pdist", path, lineno, quiet=True)
            y = -60 + (e(args[0]) * pipescale)
            z = pdist + (e(args[1]) * pipescale)
            shape = f"pipe_{args[4].strip()}"
            strat = "nocoll_istrat" if len(args) >= 6 else "gnd_istrat"
            self._emit_mapobj_literal(0, 0, y, z, shape, strat, path, lineno)
            deg360 = self._eval_expr("deg360", path, lineno, quiet=True) or 256
            deg180 = self._eval_expr("deg180", path, lineno, quiet=True) or 128
            self.m.setalvarb(AL_ROTX, (deg360 // 12) * e(args[2]))
            self.m.setalvarb(AL_ROTZ, deg180 * e(args[3]))
            return True

        if op == "mappipewait":
            pipescale = self._eval_expr("pipescale", path, lineno, quiet=True) or 1
            step = 40 * pipescale
            self._emit_wait_macro(step, path, lineno)
            pdist = self._eval_expr("pdist", path, lineno, quiet=True)
            self.symbols["pdist"] = pdist - step
            return True

        if op in ("maphalfdl", "maphalfdr"):
            if len(args) < 2:
                self._diag(path, lineno, f"{op} expects wait,z")
                return True
            if op == "maphalfdr":
                self._emit_mapobj_literal(e(args[0]), 60, -60, e(args[1]), "half_d", "halfd_istrat", path, lineno)
            else:
                self._emit_mapobj_literal(0, -60, -60, e(args[1]), "half_d", "halfd_istrat", path, lineno)
                self.m.setalvarb(AL_ROTZ, self._eval_expr("deg180", path, lineno, quiet=True))
                self._emit_wait_macro(e(args[0]), path, lineno)
            return True

        if op in ("mapdpilarr", "mapdpilarl"):
            if len(args) < 3:
                self._diag(path, lineno, f"{op} expects wait,y,z")
                return True
            if op == "mapdpilarr":
                self._emit_mapobj_literal(e(args[0]), 60, e(args[1]), e(args[2]), "d_pilar", "dpilar_istrat", path, lineno)
            else:
                self._emit_mapobj_literal(0, -60, e(args[1]), e(args[2]), "d_pilar", "dpilar_istrat", path, lineno)
                self.m.setalvarb(AL_ROTZ, self._eval_expr("deg180", path, lineno, quiet=True))
                self._emit_wait_macro(e(args[0]), path, lineno)
            return True

        if op in ("mapgotoiflevel", "mapgotoifnotlevel"):
            if len(args) < 2:
                self._diag(path, lineno, f"{op} expects level,label")
                return True
            level = e(args[0])
            cur = self._eval_expr("currentlevel", path, lineno, quiet=True)
            cmp_value = level - 1
            target = self._canon_label(args[1], file_prefix)
            if op == "mapgotoiflevel":
                self.m.mapjmpvareq(cur, cmp_value, target)
            else:
                skip = f"__ifnotlevel_skip_{self._macro_seq}"
                self._macro_seq += 1
                self.m.mapjmpvareq(cur, cmp_value, skip)
                self.m.mapgoto(target)
                self.m.label(skip)
            return True

        if op == "tstart":
            if len(args) < 3:
                self._diag(path, lineno, "tstart expects x,z,dir")
                return True
            self._truck_tx = e(args[0])
            self._truck_tz = e(args[1])
            self._truck_ta = self._eval_expr(f"dir{args[2].strip().lower()}", path, lineno, quiet=True)
            self._emit_ttruck(self._truck_tx, self._truck_tz, self._truck_ta, path, lineno)
            return True

        if op == "tanothertruck":
            self._emit_ttruck(self._truck_tx, self._truck_tz, self._truck_ta, path, lineno)
            return True

        if op == "tsouth":
            dir_west = self._eval_expr("dirwest", path, lineno, quiet=True)
            dir_east = self._eval_expr("direast", path, lineno, quiet=True)
            if self._truck_ta == dir_west:
                self._truck_ta += self._eval_expr("deg90", path, lineno, quiet=True)
                self._emit_tcorner(self._truck_tx, self._truck_tz, self._truck_ta, 0, path, lineno)
                self._truck_tz -= 1
            if self._truck_ta == dir_east:
                self._truck_ta -= self._eval_expr("deg90", path, lineno, quiet=True)
                self._emit_tcorner(self._truck_tx, self._truck_tz, self._truck_ta, 1, path, lineno)
                self._truck_tz -= 1
            self._emit_tvert(self._truck_tx, self._truck_tz, path, lineno)
            self._truck_tz -= 1
            self._truck_ta = self._eval_expr("dirsouth", path, lineno, quiet=True)
            return True

        if op == "teast":
            dir_north = self._eval_expr("dirnorth", path, lineno, quiet=True)
            dir_south = self._eval_expr("dirsouth", path, lineno, quiet=True)
            if self._truck_ta == dir_north:
                self._truck_ta -= self._eval_expr("deg90", path, lineno, quiet=True)
                self._emit_tcorner(self._truck_tx, self._truck_tz, self._truck_ta, 1, path, lineno)
                self._truck_tx += 1
            if self._truck_ta == dir_south:
                self._truck_ta += self._eval_expr("deg90", path, lineno, quiet=True)
                self._emit_tcorner(self._truck_tx, self._truck_tz, self._truck_ta, 0, path, lineno)
                self._truck_tx += 1
            self._emit_thoriz(self._truck_tx, self._truck_tz, path, lineno)
            self._truck_tx += 1
            self._truck_ta = self._eval_expr("direast", path, lineno, quiet=True)
            return True

        if op == "eguchi2fly_goto":
            # Macro wrapper around mapgoto in MAP2_3A.ASM.
            if not args:
                self._diag(path, lineno, "eguchi2fly_goto expects label")
                return True
            self.m.mapgoto(self._canon_label(args[0], file_prefix))
            return True

        return False

    # ----------------------------------------------------------
    # Symbol resolvers
    # ----------------------------------------------------------
    def _is_zero_token(self, token):
        t = (token or "").strip().lower()
        return t in ("0", "0000", "0x0", "$0")

    def _looks_like_identifier(self, token):
        return re.fullmatch(r"[A-Za-z_.][A-Za-z0-9_.]*", (token or "").strip()) is not None

    def _can_compact_shape(self, token, path, lineno):
        t = str(token).strip()
        key = t.lower()
        if key in self.shape_ids:
            return 0 <= int(self.shape_ids[key]) <= 0xFF
        if key in self.symbols and isinstance(self.symbols[key], int):
            v = int(self.symbols[key])
            return 0 <= v <= 0xFF
        v = self._eval_expr(t, path, lineno, quiet=True)
        return (self._is_zero_token(t) or v != 0) and (0 <= v <= 0xFF)

    def _can_compact_strat(self, token, path, lineno):
        t = str(token).strip()
        key = t.lower()
        if key in self.strat_ids:
            return 0 <= int(self.strat_ids[key]) <= 0xFF
        if not key.endswith("_istrat"):
            k2 = key + "_istrat"
            if k2 in self.strat_ids:
                return 0 <= int(self.strat_ids[k2]) <= 0xFF
        if key in self.symbols and isinstance(self.symbols[key], int):
            v = int(self.symbols[key])
            return 0 <= v <= 0xFF
        v = self._eval_expr(t, path, lineno, quiet=True)
        return (self._is_zero_token(t) or v != 0) and (0 <= v <= 0xFF)

    def _resolve_shape(self, token, path, lineno):
        key = token.strip().lower()
        if key in self.shape_ids:
            return int(self.shape_ids[key]) & 0xFF
        if key in self.symbols and isinstance(self.symbols[key], int):
            v = int(self.symbols[key])
            if 0 <= v <= 0xFF:
                return v
        # Last chance: expression (numeric literal etc.)
        value = self._eval_expr(token, path, lineno, quiet=True)
        if (value != 0 or self._is_zero_token(token)) and 0 <= value <= 0xFF:
            return value & 0xFF
        self._diag(path, lineno, f"Unknown shape '{token}', using 0")
        return 0

    def _resolve_strat(self, token, path, lineno):
        key = token.strip().lower()
        if key in self.strat_ids:
            return int(self.strat_ids[key]) & 0xFF
        if key.endswith("_istrat") and key in self.symbols and isinstance(self.symbols[key], int):
            v = int(self.symbols[key])
            if 0 <= v <= 0xFF:
                return v
        if not key.endswith("_istrat"):
            k2 = key + "_istrat"
            if k2 in self.strat_ids:
                return int(self.strat_ids[k2]) & 0xFF
            if k2 in self.symbols and isinstance(self.symbols[k2], int):
                v = int(self.symbols[k2])
                if 0 <= v <= 0xFF:
                    return v
        if key in self.symbols and isinstance(self.symbols[key], int):
            v = int(self.symbols[key])
            if 0 <= v <= 0xFF:
                return v
        value = self._eval_expr(token, path, lineno, quiet=True)
        if (value != 0 or self._is_zero_token(token)) and 0 <= value <= 0xFF:
            return value & 0xFF
        self._diag(path, lineno, f"Unknown strategy '{token}', using 0")
        return 0

    def _resolve_shape_raw(self, token, path, lineno):
        key = token.strip().lower()
        if key in self.shape_ids:
            return int(self.shape_ids[key]) & 0xFFFF
        if key in self.symbols and isinstance(self.symbols[key], int):
            return int(self.symbols[key]) & 0xFFFF
        value = self._eval_expr(token, path, lineno, quiet=True)
        if value != 0 or self._is_zero_token(token):
            return int(value) & 0xFFFF
        if self._looks_like_identifier(token):
            if key not in self.symbols:
                self.symbols[key] = self.next_auto_raw_shape
                self.next_auto_raw_shape += 1
            return int(self.symbols[key]) & 0xFFFF
        self._diag(path, lineno, f"Unknown shape '{token}', using 0")
        return 0

    def _resolve_strat_raw(self, token, path, lineno):
        key = token.strip().lower()
        if key in self.strat_ids:
            return int(self.strat_ids[key]) & 0xFFFFFF
        if not key.endswith("_istrat"):
            k2 = key + "_istrat"
            if k2 in self.strat_ids:
                return int(self.strat_ids[k2]) & 0xFFFFFF
        if key in self.symbols and isinstance(self.symbols[key], int):
            return int(self.symbols[key]) & 0xFFFFFF
        value = self._eval_expr(token, path, lineno, quiet=True)
        if value != 0 or self._is_zero_token(token):
            return int(value) & 0xFFFFFF
        if self._looks_like_identifier(token):
            if key not in self.symbols:
                self.symbols[key] = self.next_auto_raw_strat
                self.next_auto_raw_strat += 1
            return int(self.symbols[key]) & 0xFFFFFF
        self._diag(path, lineno, f"Unknown strategy '{token}', using 0")
        return 0

    def _resolve_map_ref(self, token, path, lineno, file_prefix):
        t = token.strip()
        if self._looks_like_identifier(t):
            return self._canon_label(t, file_prefix)
        value = self._eval_expr(t, path, lineno, quiet=True)
        if value != 0 or self._is_zero_token(t):
            return value
        self._diag(path, lineno, f"Unknown map label/expression '{token}', using 0")
        return 0

    def _resolve_al_field(self, token, path, lineno, is_alx):
        base = self.alx_offsets if is_alx else self.al_offsets
        # Allow expressions like sword1+1.
        local_syms = dict(self.symbols)
        for k, v in base.items():
            local_syms[k] = v
        return self._eval_expr(token, path, lineno, symbol_table=local_syms)

    def _resolve_path(self, token, path, lineno):
        key = token.strip().lower()
        if key in self.path_ids:
            return self.path_ids[key]
        value = self._eval_expr(token, path, lineno, quiet=True)
        if value != 0 or token.strip() in ("0", "0000"):
            return value
        assigned = self.next_auto_path_id
        self.next_auto_path_id += 1
        self.path_ids[key] = assigned
        self._diag(path, lineno, f"Unknown path '{token}', assigning synthetic path id {assigned}")
        return assigned

    def _resolve_callback(self, token, path, lineno):
        key = token.strip().lower()
        if key in self.callback_ids:
            return self.callback_ids[key]
        return self._eval_expr(token, path, lineno)

    def _resolve_friend_alive_callback(self, token, path, lineno):
        key = token.strip().lower()
        table = {
            "frog": MAP_CB_FROG_ALIVE,
            "bunny": MAP_CB_BUNNY_ALIVE,
            "cock": MAP_CB_COCK_ALIVE,
        }
        if key in table:
            return table[key]
        self._diag(path, lineno, f"Unknown friend '{token}' for mapjmpfrienddead, assuming alive")
        return MAP_CB_FROG_ALIVE

    def _resolve_clfriendmsg_callback(self, token, path, lineno):
        key = token.strip().lower()
        table = {
            "frog": MAP_CB_CLFRIENDMSG_FROG,
            "bunny": MAP_CB_CLFRIENDMSG_BUNNY,
            "cock": MAP_CB_CLFRIENDMSG_COCK,
        }
        if key in table:
            return table[key]
        self._diag(path, lineno, f"Unknown friend '{token}' for clfriendmsg, using frog table")
        return MAP_CB_CLFRIENDMSG_FROG

    def _resolve_player_mode_callback(self, token, path, lineno):
        key = token.strip().lower()
        table = {
            "exitbase": MAP_CB_SET_PLAYER_EXITBASE_L,
            "onplanet": MAP_CB_SET_PLAYER_ONPLANET_L,
            "cleardemo": MAP_CB_SET_PLAYER_CLEARDEMO_L,
            # Clear-sequence aliases used in CL_*.ASM scripts.
            "clearship2": MAP_CB_SET_PLAYER_CLEARDEMO_L,
            "clearunder": MAP_CB_SET_PLAYER_CLEARDEMO_L,
            "clearearth": MAP_CB_SET_PLAYER_CLEARDEMO_L,
            "clearbridge": MAP_CB_SET_PLAYER_CLEARDEMO_L,
            "clearchase": MAP_CB_SET_PLAYER_CLEARDEMO_L,
            "clearturn": MAP_CB_SET_PLAYER_CLEARDEMO_L,
            # Space/out-view transitions.
            "outoflb2a": MAP_CB_PLAYER_OUTVIEW_L,
            "inspace": MAP_CB_PLAYER_OUTVIEW_L,
        }
        if key in table:
            return table[key]
        if key.startswith("clear"):
            return MAP_CB_SET_PLAYER_CLEARDEMO_L
        return MAP_CB_SET_PLAYER_ONPLANET_L

    # ----------------------------------------------------------
    # Expressions
    # ----------------------------------------------------------
    def _eval_expr(self, expr, path, lineno, quiet=False, symbol_table=None):
        symbol_table = symbol_table or self.symbols
        text = (expr or "").strip()
        if not text:
            return 0
        # Some source lines use tab-separated duplicate literals (e.g. "4360\t4160");
        # the original assembler effectively consumed the first token for 1-arg macros.
        if re.fullmatch(r"[+-]?[0-9]+\s+[+-]?[0-9]+", text):
            text = text.split()[0]
        if text.startswith("{") and text.endswith("}"):
            text = text[1:-1].strip()

        text = re.sub(r"\$([0-9A-Fa-f]+)", r"0x\1", text)
        text = re.sub(r"%([01]+)", r"0b\1", text)
        text = re.sub(r"(?<![A-Za-z0-9_])\*(?![A-Za-z0-9_])", "0", text)
        # 65816 sources frequently use decimal literals with leading zeroes
        # (e.g. 01000). Normalize these before Python AST parsing.
        text = re.sub(
            r"(?<![A-Za-z0-9_])0([0-9]+)(?![A-Za-z0-9_])",
            lambda m: str(int(m.group(0), 10)),
            text,
        )

        def replace(m):
            tok = m.group(0)
            ltok = tok.lower()
            if ltok.startswith(("0x", "0b")):
                return tok
            if re.fullmatch(r"[0-9]+", tok):
                return tok
            if ltok in symbol_table:
                return str(symbol_table[ltok])
            if not quiet:
                self._diag(path, lineno, f"Unknown symbol '{tok}' in expression '{expr}', using 0")
            return "0"

        py_expr = self._TOKEN_RE.sub(replace, text)
        try:
            node = ast.parse(py_expr, mode="eval").body
            return self._eval_ast(node)
        except Exception as exc:
            if not quiet:
                self._diag(path, lineno, f"Bad expression '{expr}' -> '{py_expr}': {exc}")
            return 0

    def _eval_ast(self, node):
        if isinstance(node, ast.Constant) and isinstance(node.value, int):
            return int(node.value)
        if isinstance(node, ast.UnaryOp):
            v = self._eval_ast(node.operand)
            if isinstance(node.op, ast.UAdd):
                return +v
            if isinstance(node.op, ast.USub):
                return -v
            if isinstance(node.op, ast.Invert):
                return ~v
            raise ValueError("unsupported unary op")
        if isinstance(node, ast.BinOp):
            a = self._eval_ast(node.left)
            b = self._eval_ast(node.right)
            if isinstance(node.op, ast.Add):
                return a + b
            if isinstance(node.op, ast.Sub):
                return a - b
            if isinstance(node.op, ast.Mult):
                return a * b
            if isinstance(node.op, ast.Div):
                return int(a / b)
            if isinstance(node.op, ast.FloorDiv):
                return int(a // b)
            if isinstance(node.op, ast.LShift):
                return a << b
            if isinstance(node.op, ast.RShift):
                return a >> b
            if isinstance(node.op, ast.BitAnd):
                return a & b
            if isinstance(node.op, ast.BitOr):
                return a | b
            if isinstance(node.op, ast.BitXor):
                return a ^ b
            raise ValueError("unsupported binary op")
        raise ValueError("unsupported expression node")

    # ----------------------------------------------------------
    # Label handling
    # ----------------------------------------------------------
    def _canon_label(self, raw_label, file_prefix):
        lbl = raw_label.strip().rstrip(":")
        if lbl.startswith("."):
            return f"{file_prefix}{lbl.lower()}"
        return lbl.lower()

    def _auto_include_unresolved_label_files(self):
        while True:
            unresolved = {
                label for _, label in self.m.fixups if label not in self.m.labels
            }
            added = False
            for label in sorted(unresolved):
                file_path = self.label_file_index.get(label)
                if file_path and os.path.abspath(file_path) not in self._parsed_files:
                    self._parse_file(file_path)
                    added = True
            if not added:
                return

    def _materialize_unresolved_stubs(self):
        unresolved = sorted({label for _, label in self.m.fixups if label not in self.m.labels})
        for label in unresolved:
            self._diag(self.source_path, 0, f"Unresolved label '{label}', emitting RTS stub")
            self.m.label(label)
            self.m.maprts()

    # ----------------------------------------------------------
    # Lexical helpers
    # ----------------------------------------------------------
    def _looks_like_statement(self, token):
        t = token.lower().rstrip(":")
        if t in self._DIRECTIVES:
            return True
        base, _ = self._split_suffix(t)
        if base in self._QUIET_NOOP_MACROS:
            return True
        if base in {
            "incmap",
            "mapobj",
            "mapobjnomem",
            "mapmother",
            "maprem",
            "mapobjzrot",
            "mapwait",
            "mapwait2",
            "maploop",
            "mapjsr",
            "maprts",
            "mapgoto",
            "mapif",
            "mapjmpvarless",
            "mapjmpvarmore",
            "mapjmpvareq",
            "mapcode_jsl",
            "mapsetpath",
            "special",
            "cspecial",
            "mapspecial",
            "mapcspecial",
            "pathobj",
            "pathspecial",
            "pathcspecial",
            "setxrot",
            "setyrot",
            "setzrot",
            "setalvar",
            "setalxvar",
            "setvar",
            "setbgm",
            "setbg",
            "initbg",
            "setstage",
            "setfadeup",
            "setfadedown",
            "mapwaitfade",
            "mapmsg",
            "fadeoutbgm",
            "meters_on",
            "meters_off",
            "mapplayermode",
            "mapclplayermode",
            "mapgotoifplayerdead",
            "mapgotoiflevel",
            "mapgotoifnotlevel",
            "mapplayeroutview",
            "mapplayeroutdist",
            "maptexitwait",
            "setrestart",
            "markboss",
            "mapjmpfrienddead",
            "clfriendmsg",
            "printlevelfin",
            "mapwaitboss",
            "wipein",
            "mapexploderobot",
            "skillfly_init",
            "skillfly_set",
            "skillfly_bonus",
            "maphardrot",
            "map_setbarshape",
            "map_spacebarwait",
            "map_spacebarc",
            "map_spacebaric",
            "map_spacebarsc",
            "map_xspacebar",
            "map_yspacebar",
            "map_zspacebar",
            "map_sxspacebar",
            "map_syspacebar",
            "map_szspacebar",
            "map_ryspacebar",
            "map_sryspacebar",
            "map_spacebarx",
            "map_spacebarz",
            "map_spacebarsx",
            "map_spacebarsz",
            "map_xpspacebar",
            "map_sxpspacebar",
            "map_sbtypeobj",
            "map_sbtype0",
            "map_sbtype1",
            "map_sbtype2",
            "map_sbtype3",
            "map_sbtype4",
            "map_sbtype5",
            "map_sbtype6",
            "map_sbtype7",
            "map_sbtype8",
            "map_sbtypea",
            "map_sbtypeb",
            "map_sbtypec",
            "map_sbtyped",
            "map_sbtypee",
            "map_sbtypef",
            "map_sbtype10",
            "map_sbtype11",
            "map_sbtype12",
            "map_sbtype13",
            "map_sbtype14",
            "map_sbtype15",
            "map_sbtype16",
            "map_sbtype17",
            "map_sbtype18",
            "map_sbtype19",
            "szaco2_mapobj",
            "roottree",
            "nessie",
            "map_farships0",
            "map_farships1",
            "map_farships2",
            "map_sfish",
            "mappipe",
            "mappipewait",
            "maplrdoor",
            "mapupdndoor",
            "mapdnupdoor",
            "maphalfdr",
            "maphalfdl",
            "mapdpilarr",
            "mapdpilarl",
            "tstart",
            "tanothertruck",
            "tsouth",
            "teast",
            "eguchi2fly_goto",
            "mapend__not",
            "mapend",
            "mapendwipe",
            "start_65816",
            "end_65816",
        }:
            return True
        return False


def op_token_display(op, suffix):
    if suffix:
        return f"{op}.{suffix}"
    return op


def infer_array_name_from_source(source_asm):
    stem = os.path.splitext(os.path.basename(source_asm))[0].lower()
    safe = re.sub(r"[^a-z0-9_]+", "_", stem)
    return f"{safe}_data"


def emit_literal_map_header(source_asm, array_name, builder):
    guard = f"STARFOX_MAP_{array_name.upper()}_H"
    print("// Auto-generated by tools/map_compiler.py --source-asm")
    print(f"// Source: {source_asm}")
    print()
    print(f"#ifndef {guard}")
    print(f"#define {guard}")
    print()
    print('#include "../types.h"')
    print()
    print(builder.to_c_array(array_name))
    print(f"\n#define {array_name.upper()}_SIZE {len(builder.data)}")
    print()
    print(f"#endif // {guard}")


def build_level1_1():
    """Build Corneria Level 1 map script data."""
    m = MapBuilder()

    # === LEVEL1_1 main script ===
    # Skip the scramble intro sequence (map1_1a) — go straight to gameplay

    # mapcode_jsl initblack_l
    m.mapcodejsl_builtin(MAP_CB_INITBLACK_L)

    # mapwait medpspeed*2
    m.mapwait(MEDPSPEED * 2)

    # Spawn the player's base (decorative)
    # mapobj 0000,0000,0000,0000,mybase_1,nocoll_Istrat
    m.mapobj(0, 0, 0, 0, SH_MYBASE_1, IS_NOCOLL)
    # Note: mybase_0 shape doesn't exist in our table, skip it
    # mapobj 0000,0000,0000,0000,mybase_0,nocoll_Istrat

    # Wingman exit sequences
    # mapobj 0000,-27<<mybase_scale,-39<<mybase_scale,-200,myship_4,friendexitbase_Istrat
    m.mapobj(0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200,
             SH_MYSHIP_4, IS_FRIENDEXITBASE)
    # setalvar sbyte1,17
    m.setalvarb(AL_SBYTE1, 17)

    # mapobj 0000,-27<<mybase_scale,-39<<mybase_scale,-200,myship_4,friendexitbase_Istrat
    m.mapobj(0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200,
             SH_MYSHIP_4, IS_FRIENDEXITBASE)
    # setalvar sbyte1,17+(1000/pexitbasespeed)
    m.setalvarb(AL_SBYTE1, 17 + (1000 // PEXITBASE_SPEED))

    # Wingman path objects (Falco, Frog)
    m.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_MATEMSG, 10, 10)
    m.pathobj(0, 100, -90, 1400, SH_FRIENDSHIP_4, PATH_FALCO_LV1, 10, 10)
    m.pathobj(0, -80, -140, 1200, SH_FRIENDSHIP_4, PATH_FROG_LV1, 10, 10)

    # First buildings
    # mapobj 0000,-600,0000,2000,BU_1,HARD180YR_ISTRAT
    m.mapobj(0, -600, 0, 2000, SH_BU_1, IS_HARD180YR)
    m.mapobj(0, 600, 0, 2000, SH_BU_1, IS_HARD180YR)

    # More buildings
    m.mapobj(0, -800, 0, 3500, SH_BU_1, IS_HARD180YR)
    m.mapobj(0, 800, 0, 3500, SH_BU_1, IS_HARD180YR)

    # Building loop
    m.label("buloop")
    m.mapobj(0, -1000, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(1500, 1000, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.maploop("buloop", 3)

    # First enemy — zaco1L
    m.cspecial(0, -500, -300, 0, SH_ZACO_5, IS_ZACO1L)
    m.mapobj(0, -1100, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(1500, 1100, 0, 5000, SH_BU_1, IS_HARD180YR)

    m.mapobj(0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(0, 1200, 0, 5000, SH_BU_1, IS_HARD180YR)

    # Frog path
    m.pathobj(0, 0, -400, -100, SH_FRIENDSHIP_4, PATH_FROG1_1, 10, 10)

    # === map1_1b boss section (inline instead of JSR) ===
    m.label("boss_section")

    # fadeoutbgm → just set music
    m.setbgm(0xF1)  # fade-out cue
    m.mapwait(2000)
    m.setbgm(5)  # bgm_boss1

    # Boss spawn
    # mapobj 0000,0000,-70<<boss7_scale,-200,boss_7_1,boss7_Istrat
    BOSS7_SCALE = 3
    m.mapobj(0, 0, -70 << BOSS7_SCALE, -200, SH_BOSS_7_1, IS_BOSS7)

    # mapwaitboss (literal loop core): wait until chkbossdead callback succeeds.
    m.mapwait(100)
    m.label("boss_wait_loop")
    m.mapif_builtin(MAP_CB_CHKBOSSDEAD, "boss_wait_done")
    m.mapgoto("boss_wait_loop")
    m.label("boss_wait_done")

    # === cl_ground clear sequence (simplified) ===
    m.label("clear_section")

    m.setbgm(0xF1)  # fade
    m.mapwait(2000)
    # fanfare
    m.mapwait(3000)

    # Friend messages and ships (simplified)
    m.sendmsg(1)
    m.mapwait(MEDPSPEED * 30)

    m.mapwait(3800)

    # End level
    m.mapend()

    # === Include the 1-1 sub-map data (mid-level terrain) ===
    # This is the meat of Corneria — buildings, enemies, robots, etc.
    # For the first build, we include a simplified version.

    return m


def build_level1_1_full():
    """Build full Corneria Level 1 with 1-1.ASM content inlined."""
    m = MapBuilder()

    # === Opening sequence ===
    # mapcode_jsl initblack_l
    m.mapcodejsl_builtin(MAP_CB_INITBLACK_L)
    m.mapwait(MEDPSPEED * 2)

    # Decorative base
    m.mapobj(0, 0, 0, 0, SH_MYBASE_1, IS_NOCOLL)

    # Wingman exits
    m.mapobj(0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200,
             SH_MYSHIP_4, IS_FRIENDEXITBASE)
    m.setalvarb(AL_SBYTE1, 17)
    m.mapobj(0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200,
             SH_MYSHIP_4, IS_FRIENDEXITBASE)
    m.setalvarb(AL_SBYTE1, 37)  # 17 + 1000/50

    # Wingman paths
    m.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_MATEMSG, 10, 10)
    m.pathobj(0, 100, -90, 1400, SH_FRIENDSHIP_4, PATH_FALCO_LV1, 10, 10)
    m.pathobj(0, -80, -140, 1200, SH_FRIENDSHIP_4, PATH_FROG_LV1, 10, 10)

    # First buildings
    m.mapobj(0, -600, 0, 2000, SH_BU_1, IS_HARD180YR)
    m.mapobj(0, 600, 0, 2000, SH_BU_1, IS_HARD180YR)
    m.mapobj(0, -800, 0, 3500, SH_BU_1, IS_HARD180YR)
    m.mapobj(0, 800, 0, 3500, SH_BU_1, IS_HARD180YR)

    # Building loop (3 iterations)
    m.label("buloop")
    m.mapobj(0, -1000, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(1500, 1000, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.maploop("buloop", 3)

    # First zaco
    m.cspecial(0, -500, -300, 0, SH_ZACO_5, IS_ZACO1L)
    m.mapobj(0, -1100, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(1500, 1100, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(0, 1200, 0, 5000, SH_BU_1, IS_HARD180YR)

    # Frog path
    m.pathobj(0, 0, -400, -100, SH_FRIENDSHIP_4, PATH_FROG1_1, 10, 10)

    # === Begin 1-1.ASM content (the main level terrain) ===

    # zaco1L + buildings
    m.cspecial(0, -700, -500, 0, SH_ZACO_5, IS_ZACO1L)
    m.mapobj(0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(1500, 1200, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(1500, 1200, 0, 5000, SH_BU_1, IS_HARD180YR)

    # Arch + buildings
    m.mapobj(0, 0, 0, 4000, SH_ARCH_0, IS_HARD)
    m.mapobj(0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(1500, 1200, 0, 5000, SH_BU_1, IS_HARD180YR)

    m.mapobj(0, 200, 0, 4000, SH_ARCH_0, IS_HARD)
    m.mapobj(0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR)
    m.mapobj(1500, 1200, 0, 5000, SH_BU_1, IS_HARD180YR)

    # zaco1R + radar
    m.special(200, 400, -400, 0, SH_ZACO_A, IS_ZACO1R)
    m.mapobj(0, 350, -120, 4000, SH_RADER_0, IS_RADER0)
    m.mapobj(1000, 350, 0, 4000, SH_RADER_1, IS_RADER1)

    m.cspecial(1500, 400, -400, -250, SH_ZACO_5, IS_ZACO1R)
    m.mapobj(500, 0, 0, 4000, SH_ARCH_0, IS_HARD)

    # Buildings — avenue section
    m.mapobj(0, -600, 0, 5000, SH_BU_0, IS_HARD180YR)
    m.mapobj(1500, 600, 0, 5000, SH_BU_0, IS_HARD180YR)
    m.mapobj(2000, 0, 0, 4500, SH_BIG_GATE, IS_HARD)

    m.mapobj(0, -600, 0, 5000, SH_BU_0, IS_HARD180YR)
    m.mapobj(3000, 600, 0, 5000, SH_BU_0, IS_HARD180YR)

    # Tower section
    m.pathobj(0, -500, 0, 4000, SH_TOW_0, PATH_TOW_0, 10, 10)
    m.pathobj(1500, 500, 0, 4000, SH_TOW_0, PATH_TOW_0, 10, 10)

    # Flying enemies section
    m.special(0, 0, -1000, 2300, SH_ZACO_A, IS_ZACOS)
    m.cspecial(0, -200, -1300, 2300, SH_ZACO_6, IS_ZACOS)
    m.cspecial(3500, 200, -1300, 2300, SH_ZACO_6, IS_ZACOS)

    # Towers
    m.mapobj(0, 800, 0, 4000, SH_TOWER_2, IS_TOWER0)
    m.mapobj(3000, -800, 0, 4000, SH_TOWER_2, IS_TOWER0)
    m.cspecial(0, -800, -300, 3000, SH_KAMIKAZE, IS_ZACO4)

    m.mapobj(0, 1200, 0, 4000, SH_TOWER_2, IS_TOWER0)
    m.mapobj(600, -1200, 0, 4000, SH_TOWER_2, IS_TOWER0)
    m.cspecial(0, 800, -250, 3000, SH_KAMIKAZE, IS_ZACO4)

    # Pillars + robots with log
    m.mapobj(0, 400, 0, 4000, SH_PILLAR3, IS_PILLAR3)
    m.mapobj(3500, -400, 0, 4000, SH_PILLAR3, IS_PILLAR3)

    m.mapwait(800)

    # More buildings
    m.mapobj(0, 200, 0, 5000, SH_BU_8, IS_HARD180YR)
    m.mapobj(2400, -200, 0, 5000, SH_BU_8, IS_HARD180YR)

    m.mapobj(0, 800, 0, 5000, SH_BU_6, IS_HARD180YR)

    # Residential buildings with r_bu_1
    m.mapobj(0, 200, -50, 5000, SH_R_BU_1, IS_HARD180YR)
    m.mapobj(0, -200, -50, 5000, SH_R_BU_1, IS_HARD180YR)
    m.mapobj(0, 200, -50, 5000, SH_R_BU_1, IS_HARD180YR)
    m.mapobj(0, -200, -50, 5000, SH_R_BU_1, IS_HARD180YR)
    m.mapobj(2000, -800, 0, 5000, SH_BU_2, IS_HARD180YR)
    m.mapobj(0, 200, -50, 5000, SH_R_BU_1, IS_HARD180YR)
    m.mapobj(0, -200, -50, 5000, SH_R_BU_1, IS_HARD180YR)
    m.mapobj(0, 200, -50, 5000, SH_R_BU_1, IS_HARD180YR)

    # Town section
    m.mapobj(1000, -200, -50, 5000, SH_R_BU_1, IS_HARD180YR)
    m.mapobj(0, 800, 0, 5000, SH_BU_5, IS_HARD)
    m.setalvarb(AL_ROTY, 64)
    m.mapwait(2000)
    # mapexploderobot macro -> mapcode_jsl kill_robot_l
    m.mapcodejsl_builtin(MAP_CB_KILL_ROBOT_L)

    # More buildings
    m.mapobj(0, 820, 0, 4500, SH_BU_1, IS_HARD180YR)
    m.mapobj(1400, -1200, 0, 4000, SH_BU_2, IS_HARD180YR)
    m.cspecial(0, 300, -30, 4000, SH_BOM_WING, IS_BOMWING)
    m.mapobj(0, -820, 0, 4500, SH_BU_1, IS_HARD180YR)
    m.mapobj(2000, 820, 0, 4500, SH_BU_1, IS_HARD180YR)
    m.mapobj(0, -900, 0, 5000, SH_BU_0, IS_HARD180YR)
    m.mapobj(2800, 900, 0, 5000, SH_BU_0, IS_HARD180YR)

    # Town
    m.mapobj(0, -1000, 0, 4500, SH_BU_1, IS_HARD180YR)
    m.mapobj(0, 1000, 0, 4500, SH_BU_6, IS_HARD180YR)
    m.mapobj(0, -800, 0, 5000, SH_BU_2, IS_HARD180YR)
    m.mapobj(0, 500, 0, 5000, SH_BU_5, IS_HARD)
    m.setalvarb(AL_ROTY, 64)
    m.mapobj(2000, -350, 0, 5000, SH_BU_4, IS_HARD180YR)
    m.mapobj(0, -400, 0, 4000, SH_BU_4, IS_HARD180YR)
    m.mapobj(0, 400, 0, 4000, SH_BU_4, IS_HARD)
    m.setalvarb(AL_ROTY, 64)
    m.mapwait(1400)

    m.mapobj(1200, 0, 0, 4000, SH_BU_7, IS_HARD180YR)
    m.mapobj(0, -1000, 0, 4000, SH_BU_6, IS_HARD180YR)
    m.mapobj(600, 1000, 0, 4000, SH_BU_6, IS_HARD180YR)
    m.mapobj(0, -450, 0, 4000, SH_BU_6, IS_HARD180YR)
    m.mapobj(700, 450, 0, 4000, SH_BU_6, IS_HARD180YR)

    # Patrol enemies
    m.pathcspecial(1000, -1800, -600, 2000, SH_ZACO_5, PATH_PATROL, 10, 10)

    m.mapobj(1200, 100, 0, 4000, SH_BU_7, IS_HARD180YR)
    m.mapobj(0, -1000, 0, 4000, SH_BU_0, IS_HARD180YR)
    m.mapobj(600, 1000, 0, 4000, SH_BU_0, IS_HARD180YR)
    m.mapobj(0, -400, 0, 4000, SH_BU_4, IS_HARD180YR)
    m.mapobj(600, 450, 0, 4000, SH_BU_6, IS_HARD180YR)
    m.mapobj(600, -400, 0, 4000, SH_BU_4, IS_HARD180YR)
    m.mapobj(0, 450, 0, 4000, SH_BU_6, IS_HARD180YR)
    m.mapobj(1400, -400, 0, 4000, SH_BU_4, IS_HARD180YR)

    # More buildings + item
    m.mapobj(1000, 0, 0, 4000, SH_BU_7, IS_HARD180YR)
    m.mapobj(0, -900, 0, 4000, SH_BU_0, IS_HARD180YR)
    m.mapobj(600, 900, 0, 4000, SH_BU_0, IS_HARD180YR)
    m.mapobj(0, -400, 0, 4000, SH_BU_4, IS_HARD180YR)
    m.mapobj(1000, 400, 0, 4000, SH_BU_5, IS_HARD90YR)

    # Item
    m.mapobj(0, 440, -230, 4050, SH_ITEM_5, IS_ITEM5)
    m.setalvarb(AL_SBYTE1, 1)

    m.mapobj(0, 400, 0, 4000, SH_BU_5, IS_HARD90YR)
    m.mapobj(800, -400, 0, 4000, SH_BU_4, IS_HARD180YR)

    # More patrols
    m.pathcspecial(400, -1500, -700, 2000, SH_ZACO_5, PATH_PATROL, 10, 10)

    m.mapobj(0, 1000, 0, 4000, SH_BU_0, IS_HARD180YR)
    m.mapobj(500, -1000, 0, 4000, SH_BU_0, IS_HARD180YR)
    m.mapobj(0, 350, 0, 4000, SH_BU_4, IS_HARD)
    m.setalvarb(AL_ROTY, 64)
    m.mapobj(800, -350, 0, 4000, SH_BU_4, IS_HARD180YR)
    m.mapobj(0, 350, 0, 4000, SH_BU_4, IS_HARD)
    m.setalvarb(AL_ROTY, 64)
    m.mapobj(0, -350, 0, 4000, SH_BU_4, IS_HARD180YR)
    m.pathcspecial(500, 2000, -500, 2000, SH_ZACO_5, PATH_PATROL, 10, 10)
    m.mapwait(800)

    m.mapobj(0, 450, 0, 4000, SH_BU_6, IS_HARD180YR)
    m.mapobj(800, -400, 0, 4000, SH_BU_5, IS_HARD180YR)
    m.mapobj(0, 450, 0, 4000, SH_BU_6, IS_HARD180YR)
    m.mapobj(1100, -400, 0, 4000, SH_BU_5, IS_HARD180YR)

    # Gate + columns section
    m.mapobj(1000, 0, -100, 4000, SH_GATE_0, IS_GATE)
    m.mapobj(0, -900, 0, 5000, SH_BU_0, IS_HARD180YR)
    m.mapobj(1000, 900, 0, 5000, SH_BU_0, IS_HARD180YR)
    m.mapobj(800, 350, 0, 4000, SH_PILLAR3, IS_PILLAR3)
    m.mapobj(800, -350, 0, 4000, SH_PILLAR3, IS_PILLAR3)
    m.mapobj(0, -900, 0, 5000, SH_BU_0, IS_HARD180YR)
    m.mapobj(0, 900, 0, 5000, SH_BU_0, IS_HARD180YR)
    m.mapobj(800, 300, 0, 4000, SH_PILLAR3, IS_PILLAR3)
    m.mapobj(800, -250, 0, 4000, SH_PILLAR3, IS_PILLAR3)
    m.mapobj(0, -900, 0, 5000, SH_BU_0, IS_HARD180YR)
    m.mapobj(0, 900, 0, 5000, SH_BU_0, IS_HARD180YR)
    m.mapobj(800, 250, 0, 4000, SH_PILLAR3, IS_PILLAR3)
    m.mapobj(100, -200, 0, 4000, SH_PILLAR3, IS_PILLAR3)
    m.mapobj(1500, 200, 0, 4000, SH_PILLAR3, IS_PILLAR3)

    # Zaco fighters
    m.cspecial(100, 400, -600, -200, SH_ZACO_5, IS_ZACO1R)
    m.cspecial(800, -400, -800, -200, SH_ZACO_5, IS_ZACO1L)

    # Friend + buildings before boss
    m.mapobj(0, -1000, 0, 6000, SH_BU_5, IS_HARD180YR)
    m.mapobj(0, 1000, 0, 6000, SH_BU_4, IS_HARD)
    m.setalvarb(AL_ROTY, 64)
    m.mapwait(1000)

    m.mapobj(1000, -1000, 0, 6000, SH_BU_5, IS_HARD180YR)
    m.mapobj(0, 1300, 0, 6000, SH_BU_5, IS_HARD)
    m.setalvarb(AL_ROTY, 64)
    m.mapwait(2000)

    m.mapobj(1000, -1000, 0, 6000, SH_BU_4, IS_HARD180YR)
    m.mapobj(0, 1300, 0, 6000, SH_BU_4, IS_HARD)
    m.setalvarb(AL_ROTY, 64)

    # Gate before carrier
    m.mapobj(0, 0, -150, 4000, SH_GATE_0, IS_GATE)
    m.mapobj(0, 1300, 0, 6000, SH_BU_2, IS_HARD180YR)
    m.mapobj(2000, -1300, 0, 6000, SH_BU_2, IS_HARD180YR)

    m.special(400, -400, -200, -200, SH_ZACO_A, IS_ZACO1L)
    m.mapobj(0, -1300, 0, 7000, SH_BU_5, IS_HARD180YR)
    m.mapobj(0, 1300, 0, 7000, SH_BU_4, IS_HARD)
    m.setalvarb(AL_ROTY, 120)
    m.mapwait(3000)

    m.mapobj(0, 1300, 0, 7000, SH_BU_6, IS_HARD180YR)
    m.mapobj(3000, -1300, 0, 7000, SH_BU_6, IS_HARD180YR)
    m.mapobj(0, 1300, 0, 6000, SH_BU_4, IS_HARD)
    m.setalvarb(AL_ROTY, 120)
    m.mapobj(4000, -1300, 0, 6000, SH_BU_5, IS_HARD180YR)

    m.mapwait(4000)

    # === Boss section (map1_1b) ===
    m.label("boss")

    # Boss music
    m.fadedown()
    m.mapwait(2000)
    m.setbgm(5)  # bgm_boss1

    # Boss 7 spawn
    BOSS7_SCALE = 3
    m.mapobj(0, 0, -70 << BOSS7_SCALE, -200, SH_BOSS_7_1, IS_BOSS7)
    # mapwaitboss (literal loop core): wait until chkbossdead callback succeeds.
    m.mapwait(100)
    m.label("boss_wait_loop")
    m.mapif_builtin(MAP_CB_CHKBOSSDEAD, "boss_wait_done")
    m.mapgoto("boss_wait_loop")
    m.label("boss_wait_done")

    # === Clear sequence (cl_ground simplified) ===
    m.label("clear")

    m.setbgm(0xF1)  # silence
    m.mapwait(2000)
    # fanfare would play here
    m.mapwait(3000)

    m.sendmsg(1)
    m.mapwait(MEDPSPEED * 30)
    m.mapwait(3800)

    m.mapend()

    return m


def emit_legacy_level1_header():
    m = build_level1_1_full()

    print("// Auto-generated by tools/map_compiler.py")
    print("// Corneria Level 1 (LEVEL1_1.ASM + 1-1.ASM + MAP1_1B.ASM)")
    print("// Do not edit manually — regenerate with: python3 tools/map_compiler.py")
    print()
    print("#ifndef STARFOX_LEVELS_LEVEL1_1_DATA_H")
    print("#define STARFOX_LEVELS_LEVEL1_1_DATA_H")
    print()
    print('#include "../types.h"')
    print()

    # Output shape ID enum
    print("// Shape IDs (from ISTRATS.ASM def_shape order)")
    print("enum {")
    for name, val in sorted(
        [(k, v) for k, v in globals().items() if k.startswith("SH_")],
        key=lambda x: x[1]
    ):
        print(f"    {name} = {val},")
    print("};")
    print()

    # Output strategy ID enum
    print("// Strategy IDs (from ISTRATS.ASM def_istrat order)")
    print("enum {")
    for name, val in sorted(
        [(k, v) for k, v in globals().items() if k.startswith("IS_")],
        key=lambda x: x[1]
    ):
        print(f"    {name} = {val},")
    print("};")
    print()

    # Output SNES field offsets
    print("// SNES Alien struct field offsets (for setalvar bytecode)")
    for name, val in sorted(
        [(k, v) for k, v in globals().items() if k.startswith("AL_") and not k.startswith("ALX_")],
        key=lambda x: x[1]
    ):
        print(f"#define {name:<20} {val}")
    print()

    # Output the level data
    print(m.to_c_array("level1_1_data"))
    print(f"\n#define LEVEL1_1_DATA_SIZE {len(m.data)}")
    print()
    print("#endif // STARFOX_LEVELS_LEVEL1_1_DATA_H")


def main():
    parser = argparse.ArgumentParser(
        description="Compile Star Fox map scripts into C bytecode headers."
    )
    parser.add_argument(
        "--source-asm",
        help="Path to source MAP*.ASM file for literal compilation",
    )
    parser.add_argument(
        "--array-name",
        help="Output C array name (default: inferred from source filename)",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Fail if literal parser emits any diagnostics",
    )
    args = parser.parse_args()

    if args.array_name and not args.source_asm:
        parser.error("--array-name requires --source-asm")

    if args.source_asm:
        source = os.path.abspath(args.source_asm)
        array_name = args.array_name or infer_array_name_from_source(source)

        literal = LiteralAsmParser(source, strict=False)
        builder = literal.compile()

        for file_path, lineno, message in literal.diagnostics:
            if lineno:
                loc = f"{file_path}:{lineno}"
            else:
                loc = file_path
            print(f"[literal-map] {loc}: {message}", file=sys.stderr)

        if args.strict and literal.diagnostics:
            print(
                f"[literal-map] strict mode failed: {len(literal.diagnostics)} diagnostic(s)",
                file=sys.stderr,
            )
            return 1

        emit_literal_map_header(source, array_name, builder)
        return 0

    emit_legacy_level1_header()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
