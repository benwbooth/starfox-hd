#include "path_literals.h"
#include "paths.h"
#include "../sf_rtl.h"
#include "../game/game_vars.h"
#include "../game/sound.h"
#include "../game/world.h"
#include "../strat/strat_common.h"
#include "../strat/strat_table.h"
#include "../variables.h"
#include <math.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

void routechange1(void);

#define PATH_DATA_CAPACITY      65536u
#define PATH_LABEL_CAPACITY     2048u
#define PATH_FIXUP_CAPACITY     2048u
#define PATH_MISSING_OFFSET     0xFFFFu
#define PATH_EXT_SINTAB         0x2200u
#define PATH_EXT_GWORD1         0x2300u
#define PATH_EXT_EROLL1         0x2302u
#define PATH_EXT_EBYTE2         0x2303u
#define PATH_EXT_EFLAG1         0x2304u
#define PATH_EXT_CTYPE          0x2305u
#define PATH_TWO_PI             6.28318530717958647692f
#define PATH_I8(value)          ((int8)(uint8)(value))

#define ON  1
#define OFF 0

enum {
    PAL_ABS_PBYTE1 = 0x100u + 50u,
    PAL_ABS_PBYTE2 = 0x100u + 51u,
    PAL_ABS_PWORD1 = 0x100u + 52u,
    PAL_SHAPE  = 4,
    PAL_WORLDX = 12,
    PAL_WORLDY = 14,
    PAL_WORLDZ = 16,
    PAL_ROTX   = 18,
    PAL_ROTY   = 19,
    PAL_ROTZ   = 20,
    PAL_VEL    = 21,
    PAL_HP     = 42,
    PAL_AP     = 43,
    PAL_COLTAB = 0x80u | 32u,
    PAL_CHILDX = 0x80u | 34u,
    PAL_CHILDY = 0x80u | 35u,
    PAL_CHILDZ = 0x80u | 36u,
    PAL_CHILDROTX = 0x80u | 37u,
    PAL_CHILDROTY = 0x80u | 38u,
    PAL_PBYTE1 = 0x80u | 50u,
    PAL_PBYTE2 = 0x80u | 51u,
    PAL_PWORD1 = 0x80u | 52u,
    PAL_DEPTHOFFSET = 0x80u | 21u,
};

enum {
    SH_NULLSHAPE = 0,
    SH_PILLAR3   = 27,
    SH_BOM_WING  = 48,
    SH_R_BU_7    = 102,
    SH_ROBOT_0   = 169,
    SH_B_HOU_0   = 164,
    SH_S_HOU_0   = SH_B_HOU_0,
    SH_WALKER_2  = 164,
    SH_GATE_2    = 210,
    SH_ZACO_A    = 217,
    SH_ZACO_B    = 224,
    SH_FRIENDSHIP_4 = 218,
    // `flower` still lacks a recovered flat runtime shape id. Keep the path
    // flow literal and use the null proxy until the canonical shape lands.
    SH_FLOWER = SH_NULLSHAPE,
    SH_BOSS_7_0 = 240,
    SH_BOSS_7_1 = 241,
    SH_BOSS_7_1O = 242,
    SH_BOSS_7_2 = 243,
    SH_BOSS_7_3 = 244,
    SH_BOSS_7_4 = 245,
    SH_ARCH_0 = 228,
    SH_TOW_0  = 247,
    // `pillar3_ns` and `mediumshape` still lack canonical flat ids in the
    // active runtime. Keep those two on safe temporary ids until their slices
    // are ported.
    SH_PILLAR3_NS = SH_PILLAR3,
    SH_MEDIUMSHAPE = SH_NULLSHAPE,
    // `asteroid1` is still missing a recovered flat-runtime shape id. The
    // `e_aste`/`pyonta` path logic is ported literally; only the decorative
    // child shape stays on a null proxy until the raw symbol is mapped.
    SH_ASTEROID1 = SH_NULLSHAPE,
    // `egg`, `boss_d_8`, `boss_d_9`, and `big_bird` still lack recovered
    // flat runtime shape ids in the active renderer.
    SH_EGG = SH_NULLSHAPE,
    SH_BOSS_D_8 = SH_NULLSHAPE,
    SH_BOSS_D_9 = SH_NULLSHAPE,
    SH_BIG_BIRD = SH_NULLSHAPE,
};

enum {
    // The renderer currently ignores coltab. Keep a stable symbolic value so
    // path/strategy logic can still compare against it until the flat value is
    // recovered.
    COLTAB_ID_1_C = 1,
};

enum {
    PATH_TRIGGER_ALWAYS_VALUE = 0,
    PATH_TRIGGER_32_VALUE = 5,
    PATH_TRIGGER_WHENHIT_VALUE = 8,
    PATH_TRIGGER_WHENHITBYPLAYER_VALUE = 9,
    PATH_TRIGGER_WHENSHAPEDEAD_VALUE = 11,
    PATH_TRIGGER_WHENDEAD_VALUE = 12,
};

enum {
    WEAPON_REBELASER    = 2,
    WEAPON_FRIENDELASER  = 6,
    WEAPON_HPLASMA       = 38,
    WEAPON_RINGLASER     = 24,
    WEAPON_RELOVALBEAM   = 50,
    WEAPON_RELSLOWELASER = 12,
    WEAPON_RELBEAMBALL   = 56,
};

enum {
    STRAT_ID_BREAK_METEOR = 235,
    STRAT_ID_GATE2 = 207,
};

typedef struct {
    const char *name;
    uint16 offset;
} PathLiteralLabel;

typedef struct {
    const char *label;
    uint16 offset;
} PathLiteralFixup;

typedef struct {
    uint8 *data;
    uint16 *offsets;
    uint32 capacity;
    uint32 length;
    PathLiteralLabel labels[PATH_LABEL_CAPACITY];
    uint16 label_count;
    PathLiteralFixup fixups[PATH_FIXUP_CAPACITY];
    uint16 fixup_count;
    bool failed;
} PathLiteralBuilder;

static uint8 s_path_data[PATH_DATA_CAPACITY];
static uint16 s_path_offsets[PATH_DATA_COUNT_LITERAL];
static PathLiteralCatalog s_catalog = {
    s_path_data,
    0,
    s_path_offsets,
    PATH_DATA_COUNT_LITERAL,
};
static bool s_catalog_ready;
static uint16 s_tow_0_set_expstrat_ip = PATH_MISSING_OFFSET;
static uint16 s_robexplode_nopolyexp_ip = PATH_MISSING_OFFSET;
static uint16 s_dsmoke_init_colanim_ip = PATH_MISSING_OFFSET;
static uint16 s_dsmoke_add_colanim_ip = PATH_MISSING_OFFSET;
static uint16 s_pbooston_makeengine_ip = PATH_MISSING_OFFSET;
static uint16 s_pboostcode_updateengine_ip = PATH_MISSING_OFFSET;
static uint16 s_makepollen_ip = PATH_MISSING_OFFSET;
static uint16 s_e_big_bird_touch_ip = PATH_MISSING_OFFSET;
static uint16 s_checkifend1_ip = PATH_MISSING_OFFSET;
static uint16 s_checkifend2_ip = PATH_MISSING_OFFSET;
static uint16 s_checkifend3_ip = PATH_MISSING_OFFSET;
static uint16 s_checkifend4_ip = PATH_MISSING_OFFSET;
static uint16 s_checkifend5_ip = PATH_MISSING_OFFSET;
static uint16 s_checkifend6_ip = PATH_MISSING_OFFSET;
static uint16 s_checkifend7_ip = PATH_MISSING_OFFSET;

static void pb_fail(PathLiteralBuilder *b, const char *reason) {
    if (!b || b->failed) {
        return;
    }
    b->failed = true;
    fprintf(stderr, "Path literals: %s\n", reason);
}

static void pb_emit8(PathLiteralBuilder *b, uint8 value) {
    if (!b || b->failed) {
        return;
    }
    if (b->length >= b->capacity) {
        pb_fail(b, "bytecode buffer overflow");
        return;
    }
    b->data[b->length++] = value;
}

static void pb_emit16(PathLiteralBuilder *b, uint16 value) {
    pb_emit8(b, (uint8)(value & 0xFFu));
    pb_emit8(b, (uint8)(value >> 8));
}

static void pb_emit16s(PathLiteralBuilder *b, int16 value) {
    pb_emit16(b, (uint16)value);
}

static void pb_label(PathLiteralBuilder *b, const char *name) {
    if (!b || b->failed || !name) {
        return;
    }
    if (b->label_count >= PATH_LABEL_CAPACITY) {
        pb_fail(b, "label table overflow");
        return;
    }
    b->labels[b->label_count].name = name;
    b->labels[b->label_count].offset = (uint16)b->length;
    b->label_count++;
}

static void pb_fixup16(PathLiteralBuilder *b, const char *label) {
    if (!b || b->failed || !label) {
        return;
    }
    if (b->fixup_count >= PATH_FIXUP_CAPACITY) {
        pb_fail(b, "fixup table overflow");
        return;
    }
    b->fixups[b->fixup_count].label = label;
    b->fixups[b->fixup_count].offset = (uint16)b->length;
    b->fixup_count++;
    pb_emit16(b, 0);
}

static void pb_start_path(PathLiteralBuilder *b, uint16 path_id, const char *label) {
    if (!b || b->failed || path_id >= PATH_DATA_COUNT_LITERAL) {
        pb_fail(b, "invalid path id");
        return;
    }
    b->offsets[path_id] = (uint16)b->length;
    pb_label(b, label);
}

static bool pb_lookup_label(const PathLiteralBuilder *b, const char *label, uint16 *out) {
    if (!b || !label || !out) {
        return false;
    }
    for (uint16 i = 0; i < b->label_count; i++) {
        if (strcmp(b->labels[i].name, label) == 0) {
            *out = b->labels[i].offset;
            return true;
        }
    }
    return false;
}

static void pb_resolve(PathLiteralBuilder *b) {
    if (!b || b->failed) {
        return;
    }
    for (uint16 i = 0; i < b->fixup_count; i++) {
        uint16 target = 0;
        if (!pb_lookup_label(b, b->fixups[i].label, &target)) {
            fprintf(stderr, "Path literals: unresolved label '%s'\n", b->fixups[i].label);
            target = 0;
        }
        b->data[b->fixups[i].offset] = (uint8)(target & 0xFFu);
        b->data[b->fixups[i].offset + 1u] = (uint8)(target >> 8);
    }
}

static void pb_emit_add(PathLiteralBuilder *b, uint8 offset, int16 value) {
    if (offset == PAL_ROTX) {
        pb_emit8(b, P_ADDROTX);
        pb_emit8(b, (uint8)((int8)value));
        return;
    }
    if (offset == PAL_ROTY) {
        pb_emit8(b, P_ADDROTY);
        pb_emit8(b, (uint8)((int8)value));
        return;
    }
    if (offset == PAL_ROTZ) {
        pb_emit8(b, P_ADDROTZ);
        pb_emit8(b, (uint8)((int8)value));
        return;
    }
    if (offset == PAL_WORLDX && value >= -128 && value <= 127) {
        pb_emit8(b, P_ADDWORLDX);
        pb_emit8(b, (uint8)((int8)value));
        return;
    }
    if (offset == PAL_WORLDY && value >= -128 && value <= 127) {
        pb_emit8(b, P_ADDWORLDY);
        pb_emit8(b, (uint8)((int8)value));
        return;
    }
    if (offset == PAL_WORLDZ && value >= -128 && value <= 127) {
        pb_emit8(b, P_ADDWORLDZ);
        pb_emit8(b, (uint8)((int8)value));
        return;
    }
    if (offset == PAL_WORLDX || offset == PAL_WORLDY || offset == PAL_WORLDZ) {
        pb_emit8(b, P_ADDWS);
        pb_emit8(b, offset);
        pb_emit8(b, (uint8)((int8)value));
        return;
    }

    pb_emit8(b, P_ADDB);
    pb_emit8(b, offset);
    pb_emit8(b, (uint8)((int8)value));
}

static void pb_emit_addw(PathLiteralBuilder *b, uint8 offset, int16 value) {
    pb_emit8(b, P_ADDW);
    pb_emit8(b, offset);
    pb_emit16s(b, value);
}

static bool pb_is_byte_offset(uint8 offset) {
    switch (offset) {
    case PAL_ROTX:
    case PAL_ROTY:
    case PAL_ROTZ:
    case PAL_VEL:
    case PAL_HP:
    case PAL_AP:
    case PAL_CHILDX:
    case PAL_CHILDY:
    case PAL_CHILDZ:
    case PAL_CHILDROTX:
    case PAL_CHILDROTY:
    case PAL_PBYTE1:
    case PAL_PBYTE2:
        return true;
    default:
        return false;
    }
}

static void pb_emit_set(PathLiteralBuilder *b, uint8 offset, int16 value) {
    if (pb_is_byte_offset(offset)) {
        pb_emit8(b, P_SETB);
        pb_emit8(b, (uint8)value);
        pb_emit8(b, offset);
        return;
    }

    pb_emit8(b, P_SETW);
    pb_emit16s(b, value);
    pb_emit8(b, offset);
}

static void pb_emit_setb(PathLiteralBuilder *b, uint8 offset, int8 value) {
    pb_emit8(b, P_SETB);
    pb_emit8(b, (uint8)value);
    pb_emit8(b, offset);
}

static void pb_emit_ifsameb(PathLiteralBuilder *b, uint8 offset, uint8 value, const char *label) {
    pb_emit8(b, P_IFSAMEB);
    pb_emit8(b, offset);
    pb_emit8(b, value);
    pb_fixup16(b, label);
}

static void pb_emit_ifsamew(PathLiteralBuilder *b, uint8 offset, uint16 value, const char *label) {
    pb_emit8(b, P_IFSAMEW);
    pb_emit8(b, offset);
    pb_emit16(b, value);
    pb_fixup16(b, label);
}

static void pb_emit_iflevel(PathLiteralBuilder *b, uint8 level_1based, const char *label) {
    pb_emit8(b, P_IFLEVEL);
    pb_emit8(b, level_1based);
    pb_fixup16(b, label);
}

static void pb_emit_zero(PathLiteralBuilder *b, uint8 offset) {
    pb_emit8(b, pb_is_byte_offset(offset) ? P_SET0B : P_SET0W);
    pb_emit8(b, offset);
}

static void pb_emit_goto(PathLiteralBuilder *b, uint8 opcode, const char *label) {
    pb_emit8(b, opcode);
    pb_fixup16(b, label);
}

static void pb_emit_wait(PathLiteralBuilder *b, uint8 frames) {
    if (frames == 1u) {
        pb_emit8(b, P_WAIT1);
        return;
    }
    pb_emit8(b, P_WAIT);
    pb_emit8(b, frames);
}

static void pb_emit_loop(PathLiteralBuilder *b, uint8 count, const char *label) {
    pb_emit8(b, P_LOOP);
    pb_emit8(b, count);
    pb_fixup16(b, label);
}

static void pb_emit_chaseb(PathLiteralBuilder *b, uint8 offset, uint8 value) {
    pb_emit8(b, P_ACHASEB);
    pb_emit8(b, value);
    pb_emit8(b, offset);
}

static void pb_emit_chasew(PathLiteralBuilder *b, uint8 offset, int16 value) {
    pb_emit8(b, P_ACHASEW);
    pb_emit16s(b, value);
    pb_emit8(b, offset);
}

static void pb_emit_waitchaseb(PathLiteralBuilder *b, uint8 offset, uint8 value) {
    pb_emit8(b, P_WAITACHASEB);
    pb_emit8(b, value);
    pb_emit8(b, offset);
}

static void pb_emit_waitchasew(PathLiteralBuilder *b, uint8 offset, int16 value) {
    pb_emit8(b, P_WAITACHASEW);
    pb_emit16s(b, value);
    pb_emit8(b, offset);
}

static void pb_emit_friend(PathLiteralBuilder *b, uint8 friend_id) {
    pb_emit8(b, P_FRIEND);
    pb_emit8(b, friend_id);
}

static void pb_emit_notfriend(PathLiteralBuilder *b, uint8 friend_id, const char *label) {
    pb_emit8(b, P_NOTFRIEND);
    pb_emit8(b, friend_id);
    pb_fixup16(b, label);
}

static void pb_emit_message(PathLiteralBuilder *b, uint8 msg_id) {
    pb_emit8(b, P_MSG);
    pb_emit8(b, msg_id);
}

static void pb_emit_message2(PathLiteralBuilder *b) {
    pb_emit8(b, P_MSG2);
    pb_emit8(b, 0);
}

static void pb_emit_message_meter(PathLiteralBuilder *b, uint8 msg_id) {
    pb_emit8(b, P_MSGWITHMETER);
    pb_emit8(b, msg_id);
}

static void pb_emit_findshape(PathLiteralBuilder *b, uint16 shape) {
    pb_emit8(b, P_FINDSHAPE);
    pb_emit16(b, shape);
}

static void pb_emit_setvel(PathLiteralBuilder *b, uint8 velocity) {
    pb_emit8(b, P_SETVEL);
    pb_emit8(b, velocity);
}

static void pb_emit_accel(PathLiteralBuilder *b, uint8 speed, uint8 rate) {
    pb_emit8(b, P_SETACCEL);
    pb_emit8(b, speed);
    pb_emit8(b, rate);
}

static void pb_emit_break(PathLiteralBuilder *b, const char *label) {
    pb_emit8(b, P_BREAK);
    pb_fixup16(b, label);
}

static void pb_emit_trigger(PathLiteralBuilder *b, const char *label, int trigger) {
    if (trigger < 0) {
        pb_emit8(b, P_ALWAYSOFF);
        pb_fixup16(b, label);
        return;
    }
    pb_emit8(b, P_ALWAYS);
    pb_fixup16(b, label);
    pb_emit8(b, (uint8)trigger);
}

static void pb_emit_hitground(PathLiteralBuilder *b, int16 ground_height, const char *label) {
    pb_emit8(b, P_HITGROUND);
    pb_emit16s(b, ground_height);
    pb_fixup16(b, label);
}

static void pb_emit_distless(PathLiteralBuilder *b, uint16 distance, const char *label) {
    pb_emit8(b, P_DISTLESS);
    pb_emit16(b, distance);
    pb_fixup16(b, label);
}

static void pb_emit_distmore(PathLiteralBuilder *b, uint16 distance, const char *label) {
    pb_emit8(b, P_IFNOT);
    pb_emit_distless(b, distance, label);
}

static void pb_emit_ifbetweenb(PathLiteralBuilder *b, uint8 offset, int8 lo, int8 hi, const char *label) {
    pb_emit8(b, P_IFBETWEENB);
    pb_emit8(b, offset);
    pb_emit8(b, (uint8)lo);
    pb_emit8(b, (uint8)hi);
    pb_fixup16(b, label);
}

static void pb_emit_ifbetweenw(PathLiteralBuilder *b, uint8 offset, int16 lo, int16 hi,
                               const char *label) {
    pb_emit8(b, P_IFBETWEENW);
    pb_emit8(b, offset);
    pb_emit16s(b, lo);
    pb_emit16s(b, hi);
    pb_fixup16(b, label);
}

static void pb_emit_setrandomb(PathLiteralBuilder *b, uint16 abs_offset, uint8 mask) {
    pb_emit8(b, P_SETRANDOMB);
    pb_emit16(b, abs_offset);
    pb_emit8(b, mask);
}

static void pb_emit_setrandomw(PathLiteralBuilder *b, uint16 abs_offset, uint16 mask) {
    pb_emit8(b, P_SETRANDOMW);
    pb_emit16(b, abs_offset);
    pb_emit16(b, mask);
}

static void pb_emit_negb(PathLiteralBuilder *b, uint16 abs_offset) {
    pb_emit8(b, P_NEGB);
    pb_emit16(b, abs_offset);
}

static void pb_emit_negw(PathLiteralBuilder *b, uint16 abs_offset) {
    pb_emit8(b, P_NEGW);
    pb_emit16(b, abs_offset);
}

static void pb_emit_div2b(PathLiteralBuilder *b, uint16 abs_offset) {
    pb_emit8(b, P_DIV2B);
    pb_emit16(b, abs_offset);
}

static void pb_emit_varop(PathLiteralBuilder *b, bool is_add, bool dst_word, uint8 dst_offset,
                          bool src_word, uint8 src_offset) {
    uint8 opcode = 0;
    if (is_add) {
        if (dst_word) {
            opcode = src_word ? P_ADDVWW : P_ADDVWB;
        } else {
            opcode = src_word ? P_ADDVBW : P_ADDVBB;
        }
    } else {
        if (dst_word) {
            opcode = src_word ? P_SETVWW : P_SETVWB;
        } else {
            opcode = src_word ? P_SETVBW : P_SETVBB;
        }
    }
    pb_emit8(b, opcode);
    pb_emit8(b, dst_offset);
    pb_emit8(b, src_offset);
}

static void pb_emit_setv(PathLiteralBuilder *b, bool dst_word, uint8 dst_offset,
                         bool src_word, uint8 src_offset) {
    pb_emit_varop(b, false, dst_word, dst_offset, src_word, src_offset);
}

static void pb_emit_addv(PathLiteralBuilder *b, bool dst_word, uint8 dst_offset,
                         bool src_word, uint8 src_offset) {
    pb_emit_varop(b, true, dst_word, dst_offset, src_word, src_offset);
}

static void pb_emit_addvwb(PathLiteralBuilder *b, uint8 dst_offset, uint8 src_offset) {
    pb_emit_addv(b, true, dst_offset, false, src_offset);
}

static void pb_emit_spawn(PathLiteralBuilder *b, uint8 opcode,
                          int8 x, int8 y, int8 z,
                          int8 rotx, int8 roty, int8 rotz,
                          uint16 shape, uint16 path_id,
                          uint8 hp, uint8 ap,
                          uint8 child_num) {
    pb_emit8(b, opcode);
    pb_emit16(b, shape);
    pb_emit16(b, path_id);
    pb_emit8(b, (uint8)rotx);
    pb_emit8(b, (uint8)roty);
    pb_emit8(b, (uint8)rotz);
    pb_emit8(b, hp);
    pb_emit8(b, ap);
    pb_emit8(b, (uint8)x);
    pb_emit8(b, (uint8)y);
    pb_emit8(b, (uint8)z);
    if (opcode == P_SPAWNCHILD) {
        pb_emit8(b, child_num);
    }
}

static void pb_emit_spawn_link(PathLiteralBuilder *b, int8 x, int8 y, int8 z,
                               int8 rotx, int8 roty, int8 rotz,
                               uint16 shape, uint16 path_id,
                               uint8 hp, uint8 ap) {
    pb_emit_spawn(b, P_SPAWNLINK, x, y, z, rotx, roty, rotz, shape, path_id, hp, ap, 0);
}

static void pb_emit_spawn_child(PathLiteralBuilder *b, int8 x, int8 y, int8 z,
                                int8 rotx, int8 roty, int8 rotz,
                                uint16 shape, uint16 path_id,
                                uint8 hp, uint8 ap, uint8 child_num) {
    pb_emit_spawn(b, P_SPAWNCHILD, x, y, z, rotx, roty, rotz, shape, path_id, hp, ap, child_num);
}

static void pb_emit_qspawn(PathLiteralBuilder *b, uint16 shape, uint16 path_id, uint8 hp, uint8 ap) {
    pb_emit8(b, P_QSPAWN);
    pb_emit16(b, shape);
    pb_emit16(b, path_id);
    pb_emit8(b, hp);
    pb_emit8(b, ap);
}

static void pb_emit_gotopos(PathLiteralBuilder *b, int16 x, int16 y, int16 z, uint8 speed) {
    pb_emit8(b, P_GOTOPOS);
    pb_emit16s(b, x);
    pb_emit16s(b, y);
    pb_emit16s(b, z);
    pb_emit8(b, speed);
}

static void pb_emit_sprite(PathLiteralBuilder *b, uint8 colour, uint8 size) {
    pb_emit8(b, P_SPRITE);
    pb_emit8(b, colour);
    pb_emit8(b, size);
}

static void pb_emit_soundeffect(PathLiteralBuilder *b, uint8 sound_id) {
    pb_emit8(b, P_SOUND);
    pb_emit8(b, sound_id);
}

static void pb_emit_sound2(PathLiteralBuilder *b, uint8 sound_id) {
    pb_emit8(b, P_SOUND2);
    pb_emit8(b, sound_id);
}

static void pb_emit_exportw(PathLiteralBuilder *b, uint8 offset, uint16 addr) {
    pb_emit8(b, P_EXPORTW);
    pb_emit8(b, offset);
    pb_emit16(b, addr);
}

static void pb_emit_exportb(PathLiteralBuilder *b, uint8 offset, uint16 addr) {
    pb_emit8(b, P_EXPORTB);
    pb_emit8(b, offset);
    pb_emit16(b, addr);
}

static void pb_emit_importw(PathLiteralBuilder *b, uint8 offset, uint16 addr) {
    pb_emit8(b, P_IMPORTW);
    pb_emit8(b, offset);
    pb_emit16(b, addr);
}

static void pb_emit_importb(PathLiteralBuilder *b, uint8 offset, uint16 addr) {
    pb_emit8(b, P_IMPORTB);
    pb_emit8(b, offset);
    pb_emit16(b, addr);
}

static void pb_emit_pushb(PathLiteralBuilder *b, uint8 offset) {
    pb_emit8(b, P_PUSHB);
    pb_emit8(b, offset);
}

static void pb_emit_pullb(PathLiteralBuilder *b, uint8 offset) {
    pb_emit8(b, P_PULLB);
    pb_emit8(b, offset);
}

static void pb_emit_ifzerob(PathLiteralBuilder *b, uint8 offset, const char *label) {
    pb_emit8(b, P_IFZEROB);
    pb_emit8(b, offset);
    pb_fixup16(b, label);
}

static void pb_emit_do(PathLiteralBuilder *b, uint16 count) {
    pb_emit8(b, P_DO);
    pb_emit16(b, count);
}

static void pb_emit_setstrat_flat(PathLiteralBuilder *b, uint16 strat_id) {
    pb_emit8(b, P_SETSTRAT);
    pb_emit16(b, strat_id);
    pb_emit8(b, 0);
}

static void pb_emit_indexb(PathLiteralBuilder *b, uint16 table_addr, uint16 index_abs, uint16 dest_abs) {
    pb_emit8(b, P_INDEXB);
    pb_emit16(b, table_addr);
    pb_emit8(b, 0);
    pb_emit16(b, index_abs);
    pb_emit16(b, dest_abs);
}

static void pb_emit_start65816(PathLiteralBuilder *b, uint16 *out_ip, const char *resume_label) {
    if (out_ip) {
        *out_ip = (uint16)b->length;
    }
    pb_emit8(b, P_START65816);
    pb_emit8(b, 0xA9u);
    pb_fixup16(b, resume_label);
    pb_emit8(b, 0x6Bu);
}

static void path_literal_tow_0_set_expstrat(Alien *self) {
    if (!self) {
        return;
    }
    self->expstratptr = World_FindStrategyAddress(STRAT_ADDR_TOW0EXPLODE);
    if (!self->expstratptr) {
        printf("Path literals: missing tow0explode strategy binding\n");
    }
}

static void path_literal_dsmoke_init_colanim(Alien *self) {
    if (!self) {
        return;
    }
    self->colframe = 0;
}

static void path_literal_robexplode_set_nopolyexp(Alien *self) {
    if (!self) {
        return;
    }
    self->sflags4 |= ASF4_NOPOLYEXP;
}

static void path_literal_dsmoke_add_colanim(Alien *self) {
    if (!self) {
        return;
    }
    if (self->colframe < 15u) {
        self->colframe++;
    }
}

static void path_literal_pbooston_makeengine(Alien *self) {
    (void)self;
    // PATHDATA/DPATHDAT boost helpers use makeengine_srou_l here. The engine
    // child effect is still a visual gap in the flat runtime, so keep the
    // literal control-flow hook and make the callback explicit.
}

static void path_literal_pboostcode_updateengine(Alien *self) {
    (void)self;
    // Literal pboostcode callback placeholder until make/updateengine_srou_l
    // is ported as a real runtime visual effect.
}

static void path_literal_particlepollen_strat(Alien *self) {
    if (!self) {
        return;
    }
    self->sbyte1 = 0;
    self->sbyte2 = 0;
    self->sbyte3 = 0;
    self->count++;
    if (self->count == 250u) {
        g_aldead = 1;
    }
}

static void path_literal_makepollen(Alien *self) {
    if (!self) {
        return;
    }
    Alien *p = Obj_Alloc();
    if (!p) {
        return;
    }
    Strat_InitObjVars(p);
    p->shape = SH_NULLSHAPE;
    p->expstratptr = path_literal_particlepollen_strat;
    p->worldx = self->worldx;
    p->worldy = (int16)(self->worldy - 120);
    p->worldz = self->worldz;
    p->sflags |= (ASF_COLLDISABLE | ASF_PARTOBJ);
    p->flags |= AFEXP;
    p->sbyte1 = 6;
    p->sbyte2 = 60;
    p->sbyte3 = 250;
}

static void path_literal_e_big_bird_touch(Alien *self) {
    if (!self) {
        return;
    }
    g_levelfinished = self->pbyte1;
    g_nosetport3 = 1;
    g_pshipflags3 &= (uint8)~PSF3_ENGINESND;
    g_pshipflags3 |= PSF3_NOCOLLISIONS;
    Sound_PlayMusic(2);
    routechange1();
    // PATHDATA.ASM also queues rumble and clears m_clrbitmaps here. The HD
    // runtime has no rumble or Mario-chip bitmap-clear hook yet, so keep both
    // side effects explicit as bounded no-ops for now.
}

// KPATHDAT.ASM checkifend macro: if g_stage == expected, set c_type = 201.
// The c_type countdown is stored in g_ram[PATH_EXT_CTYPE].
static void path_literal_checkifend1(Alien *self) {
    (void)self;
    if (g_stage == 1) { g_ram[PATH_EXT_CTYPE % WRAM_SIZE] = 201; }
}
static void path_literal_checkifend2(Alien *self) {
    (void)self;
    if (g_stage == 2) { g_ram[PATH_EXT_CTYPE % WRAM_SIZE] = 201; }
}
static void path_literal_checkifend3(Alien *self) {
    (void)self;
    if (g_stage == 3) { g_ram[PATH_EXT_CTYPE % WRAM_SIZE] = 201; }
}
static void path_literal_checkifend4(Alien *self) {
    (void)self;
    if (g_stage == 4) { g_ram[PATH_EXT_CTYPE % WRAM_SIZE] = 201; }
}
static void path_literal_checkifend5(Alien *self) {
    (void)self;
    if (g_stage == 5) { g_ram[PATH_EXT_CTYPE % WRAM_SIZE] = 201; }
}
static void path_literal_checkifend6(Alien *self) {
    (void)self;
    if (g_stage == 6) { g_ram[PATH_EXT_CTYPE % WRAM_SIZE] = 201; }
}
static void path_literal_checkifend7(Alien *self) {
    (void)self;
    if (g_stage == 7) { g_ram[PATH_EXT_CTYPE % WRAM_SIZE] = 201; }
}

static void register_inline_callbacks(void) {
    if (s_tow_0_set_expstrat_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_tow_0_set_expstrat_ip, path_literal_tow_0_set_expstrat);
    }
    if (s_robexplode_nopolyexp_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_robexplode_nopolyexp_ip,
                                 path_literal_robexplode_set_nopolyexp);
    }
    if (s_dsmoke_init_colanim_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_dsmoke_init_colanim_ip, path_literal_dsmoke_init_colanim);
    }
    if (s_dsmoke_add_colanim_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_dsmoke_add_colanim_ip, path_literal_dsmoke_add_colanim);
    }
    if (s_pbooston_makeengine_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_pbooston_makeengine_ip, path_literal_pbooston_makeengine);
    }
    if (s_pboostcode_updateengine_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_pboostcode_updateengine_ip,
                                 path_literal_pboostcode_updateengine);
    }
    if (s_makepollen_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_makepollen_ip, path_literal_makepollen);
    }
    if (s_e_big_bird_touch_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_e_big_bird_touch_ip, path_literal_e_big_bird_touch);
    }
    if (s_checkifend1_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_checkifend1_ip, path_literal_checkifend1);
    }
    if (s_checkifend2_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_checkifend2_ip, path_literal_checkifend2);
    }
    if (s_checkifend3_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_checkifend3_ip, path_literal_checkifend3);
    }
    if (s_checkifend4_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_checkifend4_ip, path_literal_checkifend4);
    }
    if (s_checkifend5_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_checkifend5_ip, path_literal_checkifend5);
    }
    if (s_checkifend6_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_checkifend6_ip, path_literal_checkifend6);
    }
    if (s_checkifend7_ip != PATH_MISSING_OFFSET) {
        Paths_RegisterInlineCode(s_checkifend7_ip, path_literal_checkifend7);
    }
}

static void seed_runtime_tables(void) {
    for (uint16 i = 0; i < 256u; i++) {
        float angle = ((float)i * PATH_TWO_PI) / 256.0f;
        float s = sinf(angle) * 127.0f;
        int value = (s >= 0.0f) ? (int)(s + 0.5f) : (int)(s - 0.5f);
        g_ram[(PATH_EXT_SINTAB + i) % WRAM_SIZE] = (uint8)(int8)value;
    }
    g_ram[PATH_EXT_GWORD1 % WRAM_SIZE] = 0;
    g_ram[(PATH_EXT_GWORD1 + 1u) % WRAM_SIZE] = 0;
}

static void build_path_catalog(void) {
    PathLiteralBuilder b;

    memset(&b, 0, sizeof(b));
    memset(s_path_offsets, 0xFF, sizeof(s_path_offsets));

    b.data = s_path_data;
    b.offsets = s_path_offsets;
    b.capacity = PATH_DATA_CAPACITY;
    s_tow_0_set_expstrat_ip = PATH_MISSING_OFFSET;
    s_robexplode_nopolyexp_ip = PATH_MISSING_OFFSET;
    s_dsmoke_init_colanim_ip = PATH_MISSING_OFFSET;
    s_dsmoke_add_colanim_ip = PATH_MISSING_OFFSET;
    s_pbooston_makeengine_ip = PATH_MISSING_OFFSET;
    s_pboostcode_updateengine_ip = PATH_MISSING_OFFSET;
    s_makepollen_ip = PATH_MISSING_OFFSET;
    s_e_big_bird_touch_ip = PATH_MISSING_OFFSET;
    s_checkifend1_ip = PATH_MISSING_OFFSET;
    s_checkifend2_ip = PATH_MISSING_OFFSET;
    s_checkifend3_ip = PATH_MISSING_OFFSET;
    s_checkifend4_ip = PATH_MISSING_OFFSET;
    s_checkifend5_ip = PATH_MISSING_OFFSET;
    s_checkifend6_ip = PATH_MISSING_OFFSET;
    s_checkifend7_ip = PATH_MISSING_OFFSET;

    // Missing/unported path scripts resolve here and immediately self-remove.
    pb_emit8(&b, P_REMOVE);

    // PATHDATA.ASM:7
    pb_start_path(&b, PATH_ID_E_GATE, "e_gate");
    pb_emit8(&b, P_INVISIBLEON);
    pb_emit8(&b, P_WAIT1);
    pb_emit8(&b, P_REMOVE);

    // PATHDATA.ASM:17
    pb_start_path(&b, PATH_ID_E_FLOWER, "e_flower");
    pb_emit8(&b, P_INVISIBLEON);
    pb_label(&b, "e_flower.flow_w");
    pb_emit_distless(&b, 1500, "e_flower.flow_open");
    pb_emit_goto(&b, P_GOTO, "e_flower.flow_w");
    pb_label(&b, "e_flower.flow_open");
    pb_emit_qspawn(&b, SH_FLOWER, PATH_ID_E_FLOPEN, 10, 8);
    pb_emit8(&b, P_REMOVE);

    // PATHDATA.ASM:28
    pb_start_path(&b, PATH_ID_E_FLOPEN, "e_flopen");
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 0);
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit_zero(&b, PAL_PBYTE1);
    pb_emit_do(&b, 12);
    pb_emit8(&b, P_ADDANIM);
    pb_emit8(&b, 1);
    pb_emit8(&b, 13);
    pb_emit_add(&b, PAL_PBYTE1, 1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_PBYTE1, 8, "e_flopen.e_flpo");
    pb_emit_start65816(&b, &s_makepollen_ip, "e_flopen.after_pollen");
    pb_label(&b, "e_flopen.after_pollen");
    pb_label(&b, "e_flopen.e_flpo");
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:229
    pb_start_path(&b, PATH_ID_BIRD_METEOR, "bird_meteor");
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit_trigger(&b, "bird_meteor.big_met_c", PATH_TRIGGER_WHENHITBYPLAYER_VALUE);
    pb_emit_setb(&b, PAL_PBYTE1, 0);
    pb_emit_exportb(&b, PAL_PBYTE1, PATH_EXT_EFLAG1);
    pb_emit_sprite(&b, 0, 0);

    pb_label(&b, "bird_meteor.big_met_w");
    pb_emit_distless(&b, 1000, "bird_meteor.big_met_end");
    pb_emit_goto(&b, P_GOTO, "bird_meteor.big_met_w");

    pb_label(&b, "bird_meteor.big_met_end");
    pb_emit_trigger(&b, "bird_meteor.big_met_c", -1);
    pb_emit8(&b, P_END);

    pb_label(&b, "bird_meteor.big_met_c");
    pb_emit_add(&b, PAL_PBYTE1, 1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_PBYTE1, 10, "bird_meteor.big_met_ret");
    pb_emit_goto(&b, P_FORCE, "bird_meteor.big_met_next");

    pb_label(&b, "bird_meteor.big_met_ret");
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "bird_meteor.big_met_next");
    pb_emit_trigger(&b, "bird_meteor.big_met_c", -1);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit_do(&b, 8);
    pb_emit_qspawn(&b, SH_NULLSHAPE, PATH_ID_DAMY_EXP, 10, 10);
    pb_emit8(&b, P_NEXT);
    pb_emit_qspawn(&b, SH_EGG, PATH_ID_E_EGG, 10, 10);
    pb_emit8(&b, P_EXPLODE);

    pb_start_path(&b, PATH_ID_DAMY_EXP, "damy_exp");
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE1, 127);
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE2, 127);
    pb_emit_addv(&b, false, PAL_PBYTE1, false, PAL_PBYTE1);
    pb_emit_addv(&b, false, PAL_PBYTE1, false, PAL_PBYTE1);
    pb_emit_addv(&b, false, PAL_PBYTE2, false, PAL_PBYTE2);
    pb_emit_addv(&b, false, PAL_PBYTE2, false, PAL_PBYTE2);
    pb_emit_addvwb(&b, PAL_WORLDX, PAL_PBYTE1);
    pb_emit_addvwb(&b, PAL_WORLDY, PAL_PBYTE2);
    pb_emit_add(&b, PAL_WORLDZ, -50);
    pb_emit8(&b, P_EXPLODE);

    pb_start_path(&b, PATH_ID_DAMY_EXP2, "damy_exp2");
    pb_emit8(&b, P_EXPLODE);

    pb_start_path(&b, PATH_ID_E_EGG, "e_egg");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit_sprite(&b, 0, 0);
    pb_label(&b, "e_egg.egg_0");
    pb_emit_distmore(&b, 6000, "e_egg.egg_1");
    pb_emit_chasew(&b, PAL_WORLDX, 0);
    pb_emit_chasew(&b, PAL_WORLDY, 140);
    pb_emit_add(&b, PAL_WORLDZ, 100);
    pb_emit_goto(&b, P_GOTO, "e_egg.egg_0");
    pb_label(&b, "e_egg.egg_1");
    pb_emit8(&b, P_INVISIBLEON);
    pb_emit_wait(&b, 5);
    pb_emit_soundeffect(&b, 0x3A);
    pb_emit_qspawn(&b, SH_BIG_BIRD, PATH_ID_E_BIG_BIRD, 10, 10);
    pb_emit8(&b, P_REMOVE);

    pb_start_path(&b, PATH_ID_E_BIG_BIRD, "e_big_bird");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit_setb(&b, PAL_PBYTE1, -20);
    pb_emit_zero(&b, PAL_PBYTE2);
    pb_emit_trigger(&b, "e_big_bird.bird_updown", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit_trigger(&b, "e_big_bird.bird_pcheck", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_emit_spawn_child(&b, 0, 0, 0, 0, 0, 0,
                        SH_BOSS_D_8, PATH_ID_CHIBIR_1, 10, 10, 1);
    pb_emit_spawn_child(&b, 0, 0, 0, 0, 0, 0,
                        SH_BOSS_D_9, PATH_ID_CHIBIR_2, 10, 10, 2);
    pb_emit_setvel(&b, 40);
    pb_emit_wait(&b, 9);
    pb_emit_soundeffect(&b, 0x90);
    pb_emit_wait(&b, 1);
    pb_emit_waitchaseb(&b, PAL_ROTY, 128 - 32);
    pb_emit_wait(&b, 1);
    pb_emit_waitchaseb(&b, PAL_ROTY, 128);
    pb_emit_wait(&b, 1);
    pb_emit_waitchaseb(&b, PAL_ROTY, 128 + 32);
    pb_emit_wait(&b, 23);
    pb_emit_waitchaseb(&b, PAL_ROTY, 128);
    pb_emit_wait(&b, 1);
    pb_emit_waitchaseb(&b, PAL_ROTY, 128 - 32);
    pb_emit_wait(&b, 1);
    pb_emit_waitchaseb(&b, PAL_ROTY, 128);
    pb_emit_wait(&b, 100);
    pb_emit8(&b, P_REMOVE);

    pb_label(&b, "e_big_bird.bird_updown");
    pb_emit8(&b, P_PLAYERDEAD);
    pb_fixup16(&b, "e_big_bird.bird_end");
    pb_emit_ifsameb(&b, PAL_PBYTE2, 1, "e_big_bird.bird_bil");
    pb_emit_addvwb(&b, PAL_WORLDY, PAL_PBYTE1);
    pb_emit_add(&b, PAL_PBYTE1, 1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_PBYTE1, 20, "e_big_bird.abo_norm");
    pb_emit_setb(&b, PAL_PBYTE2, 1);
    pb_label(&b, "e_big_bird.abo_norm");
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "e_big_bird.bird_end");
    pb_emit_goto(&b, P_FORCE, "e_big_bird.bird_end2");
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "e_big_bird.bird_end2");
    pb_emit_trigger(&b, "e_big_bird.bird_pcheck", -1);
    pb_emit8(&b, P_END);

    pb_label(&b, "e_big_bird.bird_bil");
    pb_emit_addvwb(&b, PAL_WORLDY, PAL_PBYTE1);
    pb_emit_add(&b, PAL_PBYTE1, -1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_PBYTE1, (uint8)-20, "e_big_bird.bil_norm");
    pb_emit_zero(&b, PAL_PBYTE2);
    pb_label(&b, "e_big_bird.bil_norm");
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "e_big_bird.bird_pcheck");
    pb_emit8(&b, P_PLAYERDEAD);
    pb_fixup16(&b, "e_big_bird.bird_end");
    pb_emit8(&b, P_WITHINRANGE);
    pb_emit16s(&b, 40);
    pb_fixup16(&b, "e_big_bird.bird_pc_0");
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "e_big_bird.bird_pc_0");
    pb_emit_goto(&b, P_FORCE, "e_big_bird.bird_touch");
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "e_big_bird.bird_touch");
    pb_emit8(&b, P_PLAYERDEAD);
    pb_fixup16(&b, "e_big_bird.bird_endee");
    pb_emit_setb(&b, PAL_PBYTE1, 1);
    pb_emit_exportb(&b, PAL_PBYTE1, PATH_EXT_EFLAG1);
    pb_emit_setb(&b, PAL_PBYTE1, LE_ENTERSPEC);
    // `levelfinished`/`nosetport3` are globals in the flat runtime rather than
    // WRAM-exported path vars, so keep the literal transition in the inline
    // callback instead of reviving the old export indirection.
    pb_emit_start65816(&b, &s_e_big_bird_touch_ip, "e_big_bird.bird_endee");
    pb_label(&b, "e_big_bird.bird_endee");
    pb_emit8(&b, P_END);

    pb_start_path(&b, PATH_ID_CHIBIR_2, "chibir_2");
    pb_emit_add(&b, PAL_CHILDX, -6);
    pb_emit_goto(&b, P_IGOTO, "pchibir_1.chibir_0");

    pb_start_path(&b, PATH_ID_CHIBIR_1, "chibir_1");
    pb_emit_add(&b, PAL_CHILDX, 6);
    pb_label(&b, "pchibir_1.chibir_0");
    pb_emit_add(&b, PAL_CHILDY, 19);
    pb_emit_add(&b, PAL_CHILDZ, 8);
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 4);
    pb_emit_setb(&b, PAL_PBYTE1, 4);
    pb_emit_setb(&b, PAL_CHILDROTY, 128);
    pb_emit_trigger(&b, "chibir_1.bird_anim", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit8(&b, P_END);

    pb_label(&b, "chibir_1.bird_anim");
    pb_emit_add(&b, PAL_PBYTE1, 1);
    pb_emit8(&b, P_ADDANIM);
    pb_emit8(&b, 1);
    pb_emit8(&b, 13);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_PBYTE1, 13, "chibir_1.bird_ani1");
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 4);
    pb_emit_setb(&b, PAL_PBYTE1, 4);
    pb_label(&b, "chibir_1.bird_ani1");
    pb_emit8(&b, P_RETURN);

    pb_start_path(&b, PATH_ID_PINITA_B, "pinita_b");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "ppinita_a.pinita_init");
    pb_emit8(&b, P_SHADOWOFF);
    pb_emit_goto(&b, P_IGOTO, "ppinita_a.pinita_0");

    pb_start_path(&b, PATH_ID_PINITA_A, "pinita_a");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "ppinita_a.pinita_init");
    pb_emit_add(&b, PAL_WORLDY, -100);

    pb_label(&b, "ppinita_a.pinita_0");
    pb_emit_trigger(&b, "pinita_a.pinita", PATH_TRIGGER_WHENHITBYPLAYER_VALUE);
    pb_label(&b, "pinita_a.pinita_s");
    pb_emit_wait(&b, 1);
    pb_emit_goto(&b, P_GOTO, "pinita_a.pinita_s");
    pb_emit8(&b, P_END);

    pb_label(&b, "pinita_a.pinita");
    pb_emit_goto(&b, P_FORCE, "pinita_a.pinita_1");
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "pinita_a.pinita_1");
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "pinita_a.pinita_2");
    pb_emit_do(&b, 15);
    pb_emit_add(&b, PAL_ROTX, -32);
    pb_emit8(&b, P_NEXT);
    pb_emit_goto(&b, P_IGOTO, "pinita_a.pinita_3");

    pb_label(&b, "pinita_a.pinita_2");
    pb_emit_do(&b, 15);
    pb_emit_add(&b, PAL_ROTX, 32);
    pb_emit8(&b, P_NEXT);

    pb_label(&b, "pinita_a.pinita_3");
    pb_emit8(&b, P_COLLISIONSON);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_emit_setb(&b, PAL_ROTX, 64);
    pb_emit_goto(&b, P_GOTO, "pinita_a.pinita_s");

    pb_label(&b, "ppinita_a.pinita_init");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit_add(&b, PAL_WORLDX, 130);
    pb_emit_qspawn(&b, SH_R_BU_7, PATH_ID_E_PILL, 10, 8);
    pb_emit_add(&b, PAL_WORLDX, -260);
    pb_emit_qspawn(&b, SH_R_BU_7, PATH_ID_E_PILL, 10, 8);
    pb_emit_add(&b, PAL_WORLDX, 130);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_emit_setb(&b, PAL_ROTX, 64);
    pb_emit8(&b, P_RETURN);

    pb_start_path(&b, PATH_ID_E_PILL, "e_pill");
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit8(&b, P_SHADOWOFF);
    pb_emit_setb(&b, PAL_ROTZ, 128);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:495
    pb_start_path(&b, PATH_ID_ITACHI_B, "itachi_b");
    pb_emit_set(&b, PAL_COLTAB, COLTAB_ID_1_C);
    pb_emit_setb(&b, PAL_PBYTE1, 1);
    pb_emit_set(&b, PAL_PWORD1, 0);
    pb_emit_goto(&b, P_RANDOMGOTO, "pitachi_a.itachi_init");
    pb_emit_set(&b, PAL_PWORD1, 1);
    pb_emit_goto(&b, P_IGOTO, "pitachi_a.itachi_init");

    // PATHDATA.ASM:507
    pb_start_path(&b, PATH_ID_ITACHI_A, "itachi_a");
    pb_emit_setb(&b, PAL_PBYTE1, 0);
    pb_label(&b, "pitachi_a.itachi_init");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_HPLASMA);
    pb_emit_setb(&b, PAL_PBYTE2, 0);
    pb_emit_setb(&b, PAL_HP, 4);
    pb_emit_setrandomb(&b, PAL_ROTY, 127);
    pb_emit_trigger(&b, "itachi_a.itachi_fire", PATH_TRIGGER_WHENHITBYPLAYER_VALUE);
    pb_emit_trigger(&b, "itachi_a.itachi_kait", PATH_TRIGGER_ALWAYS_VALUE);

    pb_label(&b, "itachi_a.itachi_0");
    pb_emit_distless(&b, 700, "itachi_a.itachi_f");
    pb_emit_goto(&b, P_GOTO, "itachi_a.itachi_0");

    pb_label(&b, "itachi_a.itachi_f");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 0, "itachi_a.itachi_ashot");
    pb_emit_ifsamew(&b, PAL_PWORD1, 0, "itachi_a.itachi_g");
    pb_emit_setb(&b, PAL_ROTX, 0);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_emit8(&b, P_FIRE);
    pb_emit_setb(&b, PAL_ROTX, 10);
    pb_emit_setb(&b, PAL_ROTY, 128 + 10);
    pb_emit8(&b, P_FIRE);
    pb_emit_setb(&b, PAL_ROTY, 128 - 10);
    pb_emit8(&b, P_FIRE);
    pb_emit_setb(&b, PAL_ROTX, (uint8)-10);
    pb_emit_setb(&b, PAL_ROTY, 128 + 10);
    pb_emit8(&b, P_FIRE);
    pb_emit_setb(&b, PAL_ROTY, 128 - 10);
    pb_emit8(&b, P_FIRE);
    pb_emit_set(&b, PAL_PWORD1, 0);
    pb_emit_goto(&b, P_IGOTO, "itachi_a.itachi_back");

    pb_label(&b, "itachi_a.itachi_g");
    pb_emit_setb(&b, PAL_ROTX, 0);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_emit8(&b, P_FIRE);
    pb_emit_setb(&b, PAL_ROTY, 128 + 10);
    pb_emit8(&b, P_FIRE);
    pb_emit_setb(&b, PAL_ROTY, 128 - 10);
    pb_emit8(&b, P_FIRE);
    pb_emit_setb(&b, PAL_ROTX, (uint8)-10);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_emit8(&b, P_FIRE);
    pb_emit_setb(&b, PAL_ROTX, 10);
    pb_emit8(&b, P_FIRE);
    pb_emit_set(&b, PAL_PWORD1, 1);
    pb_emit_goto(&b, P_IGOTO, "itachi_a.itachi_back");

    pb_label(&b, "itachi_a.itachi_ashot");
    pb_emit_setb(&b, PAL_ROTX, 0);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "facefire");
    pb_emit_ifsameb(&b, PAL_PBYTE2, 0, "itachi_a.itachi_back");
    pb_emit8(&b, P_EXPLODE);

    pb_label(&b, "itachi_a.itachi_back");
    pb_emit_distmore(&b, 700, "itachi_a.itachi_0");
    pb_emit8(&b, P_END);

    pb_label(&b, "itachi_a.itachi_fire");
    pb_emit_goto(&b, P_FORCE, "itachi_a.itachi_f");
    pb_emit_setb(&b, PAL_PBYTE2, 1);
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "itachi_a.itachi_kait");
    pb_emit_add(&b, PAL_ROTX, 8);
    pb_emit_add(&b, PAL_WORLDZ, 30);
    pb_emit8(&b, P_RETURN);

    // PATHDATA.ASM:1773
    pb_start_path(&b, PATH_ID_E_UFO, "e_ufo");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit_trigger(&b, "e_ufo.ufo_rot", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit_soundeffect(&b, 0x0E);
    pb_emit_setb(&b, PAL_HP, 10);
    pb_emit_setb(&b, PAL_AP, 8);
    pb_label(&b, "e_ufo.ufo_0");
    pb_emit_add(&b, PAL_WORLDZ, 35);
    pb_emit_chasew(&b, PAL_WORLDY, -600);
    pb_emit_wait(&b, 1);
    pb_emit_distless(&b, 3050, "e_ufo.ufo_0");
    pb_emit_spawn(&b, P_SPAWN, 0, 10, 0, 0, 0, 0,
                  SH_BOM_WING, PATH_ID_PONPON, 10, 10, 0);
    pb_label(&b, "e_ufo.ufo_1");
    pb_emit_chaseb(&b, PAL_ROTZ, 8);
    pb_emit_add(&b, PAL_WORLDX, 50);
    pb_emit_loop(&b, 10, "e_ufo.ufo_1");
    pb_emit_wait(&b, 10);
    pb_emit_spawn(&b, P_SPAWN, -5, 10, 0, 0, 0, 0,
                  SH_WALKER_2, PATH_ID_KORORI, 10, 10, 0);
    pb_label(&b, "e_ufo.ufo_2");
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_add(&b, PAL_WORLDX, -50);
    pb_emit_loop(&b, 10, "e_ufo.ufo_2");
    pb_emit_wait(&b, 10);
    pb_emit_spawn(&b, P_SPAWN, 5, 10, 0, 0, 0, 0,
                  SH_WALKER_2, PATH_ID_KORORI, 10, 10, 0);
    pb_label(&b, "e_ufo.ufo_3");
    pb_emit_chaseb(&b, PAL_ROTZ, (uint8)-4);
    pb_emit_add(&b, PAL_WORLDX, 50);
    pb_emit_loop(&b, 10, "e_ufo.ufo_3");
    pb_emit_wait(&b, 10);
    pb_emit_spawn(&b, P_SPAWN, 0, 10, 0, 0, 0, 0,
                  SH_BOM_WING, PATH_ID_PONPON, 10, 10, 0);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_label(&b, "e_ufo.ufo_6");
    pb_emit_add(&b, PAL_ROTY, 4);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_wait(&b, 1);
    pb_emit_loop(&b, 40, "e_ufo.ufo_6");
    pb_emit8(&b, P_END);
    pb_label(&b, "e_ufo.ufo_rot");
    pb_emit_add(&b, PAL_ROTY, 16);
    pb_emit8(&b, P_RETURN);

    // PATHDATA.ASM:1829
    pb_start_path(&b, PATH_ID_PONPON, "ponpon");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_SHADOWOFF);
    pb_emit_trigger(&b, "ponpon.pon_exp", PATH_TRIGGER_WHENHITBYPLAYER_VALUE);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELBEAMBALL);
    pb_emit_setb(&b, PAL_HP, 6);
    pb_emit_setb(&b, PAL_AP, 6);
    pb_emit_set(&b, PAL_ROTY, 128);
    pb_emit_set(&b, PAL_ROTZ, 0);
    pb_label(&b, "ponpon.pon_0");
    pb_emit_hitground(&b, 30, "ponpon.pon_1");
    pb_emit_add(&b, PAL_WORLDY, 20);
    pb_emit_goto(&b, P_GOTO, "ponpon.pon_0");
    pb_label(&b, "ponpon.pon_1");
    pb_emit_distless(&b, 3000, "ponpon.pon_2");
    pb_emit_add(&b, PAL_WORLDZ, -15);
    pb_emit8(&b, P_WAIT1);
    pb_emit_goto(&b, P_GOTO, "ponpon.pon_1");
    pb_label(&b, "ponpon.pon_2");
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_LOOP);
    pb_emit8(&b, 15);
    pb_fixup16(&b, "ponpon.pon_2");
    pb_emit_distless(&b, 1000, "ponpon.pon_4");
    pb_emit8(&b, P_FIRE);
    pb_emit_goto(&b, P_RANDOMGOTO, "ponpon.pon_2l");
    pb_label(&b, "ponpon.pon_2r");
    pb_emit_add(&b, PAL_WORLDX, 8);
    pb_emit8(&b, P_LOOP);
    pb_emit8(&b, 10);
    pb_fixup16(&b, "ponpon.pon_2r");
    pb_emit_goto(&b, P_GOTO, "ponpon.pon_3");
    pb_label(&b, "ponpon.pon_2l");
    pb_emit_add(&b, PAL_WORLDX, -8);
    pb_emit8(&b, P_LOOP);
    pb_emit8(&b, 10);
    pb_fixup16(&b, "ponpon.pon_2l");
    pb_label(&b, "ponpon.pon_3");
    pb_emit_distless(&b, 1000, "ponpon.pon_4");
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_LOOP);
    pb_emit8(&b, 5);
    pb_fixup16(&b, "ponpon.pon_3");
    pb_emit_goto(&b, P_RANDOMGOTO, "ponpon.pon_31");
    pb_emit_goto(&b, P_RANDOMGOTO, "ponpon.pon_32");
    pb_emit8(&b, P_FIRE);
    pb_emit_goto(&b, P_IGOTO, "ponpon.pon_32");
    pb_label(&b, "ponpon.pon_31");
    pb_emit_goto(&b, P_RANDOMGOTO, "ponpon.pon_32");
    pb_emit_add(&b, PAL_ROTX, -6);
    pb_emit8(&b, P_FIRE);
    pb_emit_add(&b, PAL_ROTX, 6);
    pb_label(&b, "ponpon.pon_32");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit_goto(&b, P_RANDOMGOTO, "ponpon.pon_3l");
    pb_label(&b, "ponpon.pon_3r");
    pb_emit_add(&b, PAL_WORLDX, 8);
    pb_emit8(&b, P_LOOP);
    pb_emit8(&b, 15);
    pb_fixup16(&b, "ponpon.pon_3r");
    pb_emit_goto(&b, P_GOTO, "ponpon.pon_2");
    pb_label(&b, "ponpon.pon_3l");
    pb_emit_add(&b, PAL_WORLDX, -8);
    pb_emit8(&b, P_LOOP);
    pb_emit8(&b, 15);
    pb_fixup16(&b, "ponpon.pon_3l");
    pb_emit_goto(&b, P_GOTO, "ponpon.pon_2");
    pb_label(&b, "ponpon.pon_4");
    pb_emit8(&b, P_END);
    pb_label(&b, "ponpon.pon_exp");
    pb_emit_goto(&b, P_FORCE, "ponpon.pon_ex1");
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE2, 31);
    pb_emit8(&b, P_SHADOWON);
    pb_emit_goto(&b, P_RANDOMGOTO, "ponpon.pon_ex3");
    pb_emit_negb(&b, PAL_ABS_PBYTE2);
    pb_label(&b, "ponpon.pon_ex3");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "ponpon.pon_ex1");
    pb_emit_trigger(&b, "ponpon.pon_exp", -1);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_setb(&b, PAL_PBYTE1, -45);
    pb_label(&b, "ponpon.pon_ex12");
    pb_emit_addvwb(&b, PAL_WORLDY, PAL_PBYTE1);
    pb_emit_addvwb(&b, PAL_WORLDX, PAL_PBYTE2);
    pb_emit_add(&b, PAL_ROTZ, 32);
    pb_emit_add(&b, PAL_ROTX, 32);
    pb_emit_add(&b, PAL_PBYTE1, 5);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifbetweenb(&b, PAL_PBYTE1, -50, 20, "ponpon.pon_ex13");
    pb_emit_goto(&b, P_GOTO, "ponpon.pon_ex12");
    pb_label(&b, "ponpon.pon_ex13");
    pb_emit8(&b, P_EXPLODE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:4875
    pb_start_path(&b, PATH_ID_MATEMSG, "matemsg");
    pb_emit_friend(&b, FRIEND_ANYONE);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_INVISIBLEON);
    pb_emit8(&b, P_WAIT);
    pb_emit8(&b, 54);
    pb_emit_message(&b, 1);
    pb_emit_wait(&b, 25);
    pb_emit_message(&b, 5);
    pb_emit_wait(&b, 25);
    pb_emit_message(&b, 45);
    pb_emit_wait(&b, 25);
    pb_emit_message(&b, 25);
    pb_emit8(&b, P_REMOVE);

    // PATHDATA.ASM:4894
    pb_start_path(&b, PATH_ID_ASTEMSG, "astemsg");
    pb_emit_friend(&b, FRIEND_ANYONE);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_INVISIBLEON);
    pb_emit_notfriend(&b, FRIEND_FALCON, "astemsg.astms_0");
    pb_emit_message(&b, 16);
    pb_emit_goto(&b, P_GOTO, "astemsg.astms_2");
    pb_label(&b, "astemsg.astms_0");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "astemsg.astms_1");
    pb_emit_message(&b, 36);
    pb_emit_goto(&b, P_GOTO, "astemsg.astms_2");
    pb_label(&b, "astemsg.astms_1");
    pb_emit_notfriend(&b, FRIEND_FROG, "astemsg.astms_2");
    pb_emit_message(&b, 56);
    pb_label(&b, "astemsg.astms_2");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:5157
    pb_start_path(&b, PATH_ID_MES_MESSAGE, "mes_message");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_INVISIBLEON);
    pb_emit_message2(&b);
    pb_emit_wait(&b, 60);
    pb_emit8(&b, P_REMOVE);

    // Shared chase init helper (PATHDATA.ASM:8948).
    pb_label(&b, "chase_init");
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELSLOWELASER);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit_setb(&b, PAL_HP, 6);
    pb_emit_iflevel(&b, 2, "chase_init.chase_hp1");
    pb_emit_iflevel(&b, 3, "chase_init.chase_hp2");
    pb_emit_setb(&b, PAL_HP, 4);
    pb_emit_goto(&b, P_GOTO, "chase_init.chase_hp1");
    pb_label(&b, "chase_init.chase_hp2");
    pb_emit_setb(&b, PAL_HP, 8);
    pb_label(&b, "chase_init.chase_hp1");
    pb_emit8(&b, P_RETURN);

    // Shared friend init helper (PATHDATA.ASM:8966).
    pb_label(&b, "friend_init");
    pb_emit_trigger(&b, "save", PATH_TRIGGER_WHENSHAPEDEAD_VALUE);
    pb_emit_trigger(&b, "yamete", PATH_TRIGGER_WHENHITBYPLAYER_VALUE);
    pb_emit_trigger(&b, "recover", PATH_TRIGGER_WHENHIT_VALUE);
    pb_emit_trigger(&b, "clpby2", PATH_TRIGGER_32_VALUE);
    pb_emit_setb(&b, PAL_PBYTE1, 0);
    pb_emit_setb(&b, PAL_PBYTE2, 0);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_FRIENDELASER);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit_setb(&b, PAL_HP, 100);
    pb_emit_setb(&b, PAL_AP, 1);
    pb_emit8(&b, P_RETURN);

    // Shared face-and-fire helper (PATHDATA.ASM:8996).
    pb_label(&b, "facefire");
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_RETURN);

    // Helper used by wingman scripts (PATHDATA.ASM:9012).
    pb_label(&b, "recover");
    pb_emit_set(&b, PAL_HP, 100);
    pb_emit8(&b, P_DISTLESS);
    pb_emit16(&b, 100);
    pb_fixup16(&b, "recover.fri_nocol");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "recover.fri_nocol");
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit8(&b, P_RETURN);

    // Helper used by friend_inib trigger setup (PATHDATA.ASM:9023).
    pb_label(&b, "clpby2");
    pb_emit_setb(&b, PAL_PBYTE2, 0);
    pb_emit8(&b, P_RETURN);

    // Helper used by friend_inib trigger setup (PATHDATA.ASM:9027).
    pb_label(&b, "yamete");
    pb_emit_ifsameb(&b, PAL_PBYTE2, 0, "yamete.yame_ck");
    pb_emit_setb(&b, PAL_HP, 100);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "yamete.yame_ck");
    pb_emit_setb(&b, PAL_PBYTE2, 1);
    pb_emit_notfriend(&b, FRIEND_FALCON, "yamete.yame_a");
    pb_emit_message(&b, 14);
    pb_emit_setb(&b, PAL_HP, 110);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "yamete.yame_a");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "yamete.yame_b");
    pb_emit_message(&b, 34);
    pb_emit_setb(&b, PAL_HP, 110);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "yamete.yame_b");
    pb_emit_message(&b, 54);
    pb_emit_setb(&b, PAL_HP, 110);
    pb_emit8(&b, P_RETURN);

    // Shared friend init helper (PATHDATA.ASM:8983).
    pb_label(&b, "friend_inib");
    pb_emit8(&b, P_ALWAYS);
    pb_fixup16(&b, "yamete");
    pb_emit8(&b, PATH_TRIGGER_WHENHITBYPLAYER_VALUE);
    pb_emit8(&b, P_ALWAYS);
    pb_fixup16(&b, "recover");
    pb_emit8(&b, PATH_TRIGGER_WHENHIT_VALUE);
    pb_emit8(&b, P_ALWAYS);
    pb_fixup16(&b, "clpby2");
    pb_emit8(&b, PATH_TRIGGER_32_VALUE);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_FRIENDELASER);
    pb_emit_setb(&b, PAL_HP, 100);
    pb_emit_setb(&b, PAL_AP, 1);
    pb_emit8(&b, P_RETURN);

    // Shared boost helpers (DPATHDAT.ASM:2085).
    pb_label(&b, "pbooston");
    pb_emit_start65816(&b, &s_pbooston_makeengine_ip, "pbooston.after_makeengine");
    pb_label(&b, "pbooston.after_makeengine");
    pb_emit_trigger(&b, "pboostcode", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pboostcode");
    pb_emit_start65816(&b, &s_pboostcode_updateengine_ip, "pboostcode.after_updateengine");
    pb_label(&b, "pboostcode.after_updateengine");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pboostoff");
    pb_emit8(&b, P_RETURN);

    // Shared wingman-save helper and rescued-wingman paths (PATHDATA.ASM:9051).
    pb_label(&b, "save");
    pb_emit_goto(&b, P_FORCE, "save.fit_0");
    pb_emit_notfriend(&b, FRIEND_FALCON, "save.save_0");
    pb_emit_message_meter(&b, 8);
    pb_emit_goto(&b, P_IGOTO, "save.save_2");
    pb_label(&b, "save.save_0");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "save.save_1");
    pb_emit_message_meter(&b, 28);
    pb_emit_goto(&b, P_IGOTO, "save.save_2");
    pb_label(&b, "save.save_1");
    pb_emit_message_meter(&b, 48);
    pb_label(&b, "save.save_2");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "save.fit_0");
    pb_emit_trigger(&b, "save", -1);
    pb_emit_notfriend(&b, FRIEND_FALCON, "save.fit_sr");
    pb_emit_qspawn(&b, SH_FRIENDSHIP_4, PATH_ID_E_FALCON, 100, 1);
    pb_emit8(&b, P_REMOVE);
    pb_label(&b, "save.fit_sr");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "save.fit_sf");
    pb_emit_qspawn(&b, SH_FRIENDSHIP_4, PATH_ID_E_RABBIT, 100, 1);
    pb_emit8(&b, P_REMOVE);
    pb_label(&b, "save.fit_sf");
    pb_emit_qspawn(&b, SH_FRIENDSHIP_4, PATH_ID_E_FROG, 100, 1);
    pb_emit8(&b, P_REMOVE);

    pb_start_path(&b, PATH_ID_E_RABBIT, "e_rabbit");
    pb_emit_friend(&b, FRIEND_RABBIT);
    pb_emit_goto(&b, P_IGOTO, "pe_falcon.fit_start_0");

    pb_start_path(&b, PATH_ID_E_FROG, "e_frog");
    pb_emit_friend(&b, FRIEND_FROG);
    pb_emit_goto(&b, P_IGOTO, "pe_falcon.fit_start_0");

    pb_start_path(&b, PATH_ID_E_FALCON, "e_falcon");
    pb_emit_friend(&b, FRIEND_FALCON);
    pb_label(&b, "pe_falcon.fit_start_0");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "friend_inib");
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_emit_soundeffect(&b, 1);
    pb_label(&b, "pe_falcon.fit_start");
    pb_emit_setvel(&b, 30);
    pb_emit_distless(&b, 2000, "pe_falcon.fit");
    pb_emit_setvel(&b, 10);
    pb_label(&b, "pe_falcon.fit");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTY, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_ifsameb(&b, PAL_ROTX, 0, "pe_falcon.fit_1");
    pb_emit_goto(&b, P_GOTO, "pe_falcon.fit");
    pb_label(&b, "pe_falcon.fit_1");
    pb_emit_ifsameb(&b, PAL_ROTY, 0, "pe_falcon.fit_2");
    pb_emit_goto(&b, P_GOTO, "pe_falcon.fit");
    pb_label(&b, "pe_falcon.fit_2");
    pb_emit_ifsameb(&b, PAL_ROTZ, 0, "pe_falcon.go");
    pb_emit_goto(&b, P_GOTO, "pe_falcon.fit");
    pb_label(&b, "pe_falcon.go");
    pb_emit_setvel(&b, (uint8)-30);
    pb_label(&b, "pe_falcon.go_0");
    pb_emit_distless(&b, 200, "pe_falcon.go_1");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -15, 15, "pe_falcon.go_d");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, 1000, 7000, "pe_falcon.go_bl");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, 0, 1000, "pe_falcon.go_l");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -1000, 0, "pe_falcon.go_r");
    pb_emit_add(&b, PAL_WORLDX, 100);
    pb_emit_goto(&b, P_IGOTO, "pe_falcon.go_d");
    pb_label(&b, "pe_falcon.go_r");
    pb_emit_add(&b, PAL_WORLDX, 10);
    pb_emit_goto(&b, P_IGOTO, "pe_falcon.go_d");
    pb_label(&b, "pe_falcon.go_bl");
    pb_emit_add(&b, PAL_WORLDX, -100);
    pb_emit_goto(&b, P_IGOTO, "pe_falcon.go_d");
    pb_label(&b, "pe_falcon.go_l");
    pb_emit_add(&b, PAL_WORLDX, -10);
    pb_label(&b, "pe_falcon.go_d");
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -115, -85, "pe_falcon.go_e");
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -100, 5000, "pe_falcon.go_u");
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -7000, -1000, "pe_falcon.go_bd");
    pb_emit_add(&b, PAL_WORLDY, 10);
    pb_emit_goto(&b, P_IGOTO, "pe_falcon.go_e");
    pb_label(&b, "pe_falcon.go_bd");
    pb_emit_add(&b, PAL_WORLDY, 100);
    pb_emit_goto(&b, P_IGOTO, "pe_falcon.go_e");
    pb_label(&b, "pe_falcon.go_u");
    pb_emit_add(&b, PAL_WORLDY, -10);
    pb_label(&b, "pe_falcon.go_e");
    pb_emit_goto(&b, P_GOTO, "pe_falcon.go_0");
    pb_label(&b, "pe_falcon.go_1");
    pb_emit_setvel(&b, 0);
    pb_emit8(&b, P_FIRECANHIT);
    pb_emit_wait(&b, 15);
    pb_emit8(&b, P_FIRECANHIT);
    pb_emit_wait(&b, 15);
    pb_emit8(&b, P_FIRECANHIT);
    pb_emit_wait(&b, 15);
    pb_emit8(&b, P_FIRECANHIT);
    pb_emit_wait(&b, 15);
    pb_emit8(&b, P_FIRECANHIT);
    pb_emit_wait(&b, 15);
    pb_emit8(&b, P_FIRECANHIT);
    pb_emit_wait(&b, 15);

    // Shared Falco follow-through label from PATHDATA.ASM:9180.
    pb_label(&b, "pe_falcon.gfal_ff");
    pb_emit8(&b, P_WAITACHASEB);
    pb_emit8(&b, 224);
    pb_emit8(&b, PAL_ROTZ);
    pb_label(&b, "pe_falcon.go_2");
    pb_emit_add(&b, PAL_ROTX, -2);
    pb_emit_add(&b, PAL_ROTY, -2);
    pb_emit_add(&b, PAL_WORLDX, 10);
    pb_emit_add(&b, PAL_WORLDY, -10);
    pb_emit8(&b, P_LOOP);
    pb_emit8(&b, 6);
    pb_fixup16(&b, "pe_falcon.go_2");
    pb_label(&b, "pe_falcon.go_3");
    pb_emit_add(&b, PAL_WORLDX, 10);
    pb_emit_add(&b, PAL_WORLDY, -10);
    pb_emit8(&b, P_LOOP);
    pb_emit8(&b, 20);
    pb_fixup16(&b, "pe_falcon.go_3");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:8834
    pb_start_path(&b, PATH_ID_FALCON3_1, "falcon3_1");
    pb_emit_friend(&b, FRIEND_FALCON);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "friend_inib");
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_sound2(&b, 1);
    pb_emit_setvel(&b, 30);
    pb_emit_accel(&b, 0, 1);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_wait(&b, 15);
    pb_emit_message(&b, 117);
    pb_emit_wait(&b, 30);
    pb_emit8(&b, P_COLLISIONSON);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pbooston");
    pb_emit_setvel(&b, 100);
    pb_emit_wait(&b, 15);
    pb_emit_do(&b, 5);
    pb_emit_add(&b, PAL_WORLDX, 5);
    pb_emit8(&b, P_NEXT);
    pb_emit_wait(&b, 5);
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:8870
    pb_start_path(&b, PATH_ID_FROG1_1, "frog1_1");
    pb_emit_friend(&b, FRIEND_FROG);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "friend_inib");
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_wait(&b, 38);
    pb_emit_set(&b, PAL_ROTX, 32);
    pb_emit8(&b, P_SOUND2);
    pb_emit8(&b, 1);
    pb_emit_setvel(&b, 30);
    pb_emit_wait(&b, 10);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_emit8(&b, P_COLLISIONSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, (uint8)-20);
    pb_emit_wait(&b, 20);
    pb_emit_setvel(&b, 0);
    pb_emit_message(&b, 78);
    pb_label(&b, "frog1_1.frog1_1w");
    pb_emit_findshape(&b, SH_ARCH_0);
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "frog1_1.frog1_1w0");
    pb_emit8(&b, P_IGOTO);
    pb_fixup16(&b, "frog1_1.frog1_10");
    pb_label(&b, "frog1_1.frog1_1w0");
    pb_emit8(&b, P_GOTO);
    pb_fixup16(&b, "frog1_1.frog1_1w");
    pb_label(&b, "frog1_1.frog1_10");
    pb_emit8(&b, P_SHAPEDISTLESS);
    pb_emit16(&b, 60);
    pb_fixup16(&b, "frog1_1.frog1_11");
    pb_emit8(&b, P_GOTO);
    pb_fixup16(&b, "frog1_1.frog1_10");
    pb_label(&b, "frog1_1.frog1_11");
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 10);
    pb_emit_add(&b, PAL_WORLDX, 10);
    pb_emit_add(&b, PAL_ROTZ, -3);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_WAIT1);
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 10);
    pb_emit_add(&b, PAL_WORLDX, 10);
    pb_emit_add(&b, PAL_ROTZ, 3);
    pb_emit8(&b, P_NEXT);
    pb_emit_findshape(&b, SH_ARCH_0);
    pb_label(&b, "frog1_1.frog1_12");
    pb_emit8(&b, P_SHAPEDISTLESS);
    pb_emit16(&b, 60);
    pb_fixup16(&b, "frog1_1.frog1_13");
    pb_emit8(&b, P_GOTO);
    pb_fixup16(&b, "frog1_1.frog1_12");
    pb_label(&b, "frog1_1.frog1_13");
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 14);
    pb_emit_add(&b, PAL_WORLDX, -15);
    pb_emit_add(&b, PAL_ROTZ, 3);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_WAIT1);
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 14);
    pb_emit_add(&b, PAL_WORLDX, -15);
    pb_emit_add(&b, PAL_ROTZ, -3);
    pb_emit8(&b, P_NEXT);
    pb_emit_message(&b, 64);
    pb_emit_wait(&b, 25);
    pb_emit8(&b, P_IGOTO);
    pb_fixup16(&b, "pe_falcon.gfal_ff");

    // PATHDATA.ASM:9199
    pb_start_path(&b, PATH_ID_FALCO_LV1, "falco_lv1");
    pb_emit_friend(&b, FRIEND_FALCON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_SOUND2);
    pb_emit8(&b, 1);
    pb_emit8(&b, P_ALWAYS);
    pb_fixup16(&b, "recover");
    pb_emit8(&b, PATH_TRIGGER_WHENHIT_VALUE);
    pb_emit_wait(&b, 80);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit_wait(&b, 50);
    pb_emit8(&b, P_IGOTO);
    pb_fixup16(&b, "pe_falcon.gfal_ff");

    // PATHDATA.ASM:9214
    pb_start_path(&b, PATH_ID_FROG_LV1, "frog_lv1");
    pb_emit_friend(&b, FRIEND_FROG);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_SOUND2);
    pb_emit8(&b, 1);
    pb_emit8(&b, P_ALWAYS);
    pb_fixup16(&b, "recover");
    pb_emit8(&b, PATH_TRIGGER_WHENHIT_VALUE);
    pb_emit_wait(&b, 80);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit_wait(&b, 50);
    pb_emit_wait(&b, 15);
    pb_label(&b, "frog_lv1.gfro_ff");
    pb_emit_waitchaseb(&b, PAL_ROTZ, 32);
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 7);
    pb_emit_add(&b, PAL_ROTX, -2);
    pb_emit_add(&b, PAL_ROTY, 2);
    pb_emit_add(&b, PAL_WORLDX, -10);
    pb_emit_add(&b, PAL_WORLDY, -10);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 21);
    pb_emit_add(&b, PAL_WORLDX, -10);
    pb_emit_add(&b, PAL_WORLDY, -10);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit8(&b, P_END);

    // DPATHDAT.ASM:238
    pb_start_path(&b, PATH_ID_TOW_0, "tow_0");
    pb_emit_setb(&b, PAL_HP, 4);
    pb_emit_setb(&b, PAL_AP, 8);
    pb_emit8(&b, P_SHADOWOFF);
    pb_emit_add(&b, PAL_WORLDY, -160);
    pb_emit_start65816(&b, &s_tow_0_set_expstrat_ip, "tow_0.after_expstrat");
    pb_label(&b, "tow_0.after_expstrat");
    // DPATHDAT uses `tow_1` for the child shape here, but raw path-only shape
    // symbols still lack canonical flat ids. Keep the existing shape proxy and
    // fix the literal child offsets.
    pb_emit_spawn_link(&b, 0, PATH_I8(-200), 5, 0, 0, 0, SH_TOW_0, PATH_ID_TOW_1, 10, 10);
    pb_emit_trigger(&b, "tow_0.explode", PATH_TRIGGER_WHENDEAD_VALUE);
    pb_emit8(&b, P_END);
    pb_label(&b, "tow_0.explode");
    pb_emit8(&b, P_FLAGSHAPE);
    pb_emit8(&b, P_RETURN);

    // DPATHDAT.ASM:260
    pb_start_path(&b, PATH_ID_TOW_1, "tow_1");
    pb_emit8(&b, P_INVINCIBLEON);
    pb_label(&b, "tow_1.waitforflag");
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "tow_1.falldown");
    pb_emit8(&b, P_GOTO);
    pb_fixup16(&b, "tow_1.waitforflag");
    pb_label(&b, "tow_1.falldown");
    pb_emit8(&b, P_WAIT);
    pb_emit8(&b, 7);
    pb_emit_setb(&b, PAL_PBYTE1, 0);
    pb_label(&b, "tow_1.fall");
    pb_emit8(&b, P_WAIT1);
    pb_emit_add(&b, PAL_PBYTE1, 4);
    pb_emit_addvwb(&b, PAL_WORLDY, PAL_PBYTE1);
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -1100, -25, "tow_1.fall");
    pb_emit_set(&b, PAL_WORLDY, -25);
    pb_emit_soundeffect(&b, 0x49);
    pb_emit_qspawn(&b, 0, PATH_ID_DSMOKE, 10, 10);
    pb_emit_qspawn(&b, 0, PATH_ID_DSMOKE2, 10, 10);
    pb_emit_qspawn(&b, 0, PATH_ID_DSMOKE3, 10, 10);
    pb_emit8(&b, P_END);

    // DPATHDAT.ASM:286
    pb_start_path(&b, PATH_ID_DSMOKE2, "dsmoke2");
    pb_emit_add(&b, PAL_WORLDX, -40);
    pb_start_path(&b, PATH_ID_DSMOKE3, "dsmoke3");
    pb_emit_add(&b, PAL_WORLDX, 20);
    pb_start_path(&b, PATH_ID_DSMOKE, "dsmoke");
    pb_emit_sprite(&b, 0, 0);
    pb_emit_add(&b, PAL_WORLDZ, -10);
    pb_emit_start65816(&b, &s_dsmoke_init_colanim_ip, "dsmoke.after_init_colanim");
    pb_label(&b, "dsmoke.after_init_colanim");
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 8);
    pb_emit_start65816(&b, &s_dsmoke_add_colanim_ip, "dsmoke.after_add_colanim");
    pb_label(&b, "dsmoke.after_add_colanim");
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_REMOVE);

    // Shared DPATHDAT.ASM label used by carrier/tumbler robots.
    pb_label(&b, "premove");
    pb_emit8(&b, P_REMOVE);

    // DPATHDAT.ASM:1401
    pb_start_path(&b, PATH_ID_ROBOT, "robot");
    pb_emit8(&b, P_SHADOWOFF);
    pb_emit_sound2(&b, 0x0D);
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 0);
    pb_emit_trigger(&b, "robot.robdead", PATH_TRIGGER_WHENDEAD_VALUE);
    pb_label(&b, "robot.waittowalk");
    pb_emit8(&b, P_WAIT1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_distless(&b, 2500, "robot.waittowalk");
    pb_emit_trigger(&b, "robot.robanim", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit8(&b, P_SETVEL);
    pb_emit8(&b, 20);
    pb_label(&b, "robot.forever");
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "robot.robforce");
    pb_emit_goto(&b, P_GOTO, "robot.forever");
    pb_label(&b, "robot.robforce");
    pb_emit_trigger(&b, "robot.robdead", -1);
    pb_emit_trigger(&b, "robot.robanim", -1);
    pb_emit8(&b, P_SETVEL);
    pb_emit8(&b, 0);
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 0);
    pb_emit_trigger(&b, "robot.dying", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit_setb(&b, PAL_PBYTE1, 0);
    pb_emit8(&b, P_WAIT);
    pb_emit8(&b, 10);
    pb_emit_trigger(&b, "robot.dying", -1);
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_label(&b, "robot.falloverdead");
    pb_emit8(&b, P_WAIT1);
    pb_emit_add(&b, PAL_PBYTE1, 1);
    pb_emit_addv(&b, false, PAL_ROTX, false, PAL_PBYTE1);
    pb_emit_ifbetweenb(&b, PAL_ROTX, 0, DEG90, "robot.falloverdead");
    pb_emit_set(&b, PAL_ROTX, DEG90);
    pb_emit_negb(&b, PAL_ABS_PBYTE1);
    pb_emit_div2b(&b, PAL_ABS_PBYTE1);
    pb_emit_div2b(&b, PAL_ABS_PBYTE1);
    pb_emit_soundeffect(&b, 0x49);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_PBYTE1, 0, "robot.falloverdead");
    pb_emit8(&b, P_EXPLODE);
    pb_emit8(&b, P_END);
    pb_label(&b, "robot.dying");
    pb_emit_qspawn(&b, SH_MEDIUMSHAPE, PATH_ID_ROBEXPLODE, 10, 10);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "robot.robdead");
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit_setb(&b, PAL_HP, 10);
    pb_emit_goto(&b, P_FORCE, "robot.robforce");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "robot.robanim");
    pb_emit8(&b, P_ADDANIM);
    pb_emit8(&b, 1);
    pb_emit8(&b, 12);
    pb_emit8(&b, P_RETURN);

    // DPATHDAT.ASM:1467
    pb_start_path(&b, PATH_ID_ROBEXPLODE, "robexplode");
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit_start65816(&b, &s_robexplode_nopolyexp_ip, "robexplode.after_nopolyexp");
    pb_label(&b, "robexplode.after_nopolyexp");
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE1, 127);
    pb_emit_add(&b, PAL_PBYTE1, -64);
    pb_emit_addv(&b, true, PAL_WORLDX, false, PAL_PBYTE1);
    pb_emit_setrandomw(&b, PAL_ABS_PWORD1, 511);
    pb_emit_negw(&b, PAL_ABS_PWORD1);
    pb_emit_addv(&b, true, PAL_WORLDY, true, PAL_PWORD1);
    pb_emit_add(&b, PAL_WORLDZ, -20);
    pb_emit8(&b, P_EXPLODE);
    pb_emit8(&b, P_END);

    // DPATHDAT.ASM:1296
    pb_start_path(&b, PATH_ID_ROBOTWITHLOG, "robotwithlog");
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_exportw(&b, PAL_PWORD1, PATH_EXT_GWORD1);
    pb_emit_spawn_child(&b, 0, 0, 0, 0, 0, 0,
                        SH_ROBOT_0, PATH_ID_ROBOTWITHLOG2, 4, 10, 2);
    pb_emit_spawn_child(&b, 0, 0, 0, 0, 0, 0,
                        SH_NULLSHAPE, PATH_ID_DUMMY, 4, 10, 3);
    pb_emit_goto(&b, P_IGOTO, "robotswithlog.in");

    pb_start_path(&b, PATH_ID_DUMMY, "dummy");
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit8(&b, P_INVISIBLEON);
    pb_emit8(&b, P_SHADOWOFF);
    pb_emit8(&b, P_END);

    // DPATHDAT.ASM:1325
    pb_start_path(&b, PATH_ID_ROBOTWITHLOG2, "robotwithlog2");
    pb_emit8(&b, P_SHADOWOFF);
    pb_emit_sound2(&b, 0x0D);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 0);
    pb_emit_trigger(&b, "robot.robanim", PATH_TRIGGER_ALWAYS_VALUE);
    pb_label(&b, "robotwithlog2.chkflag");
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "robotwithlog2.exit");
    pb_emit_goto(&b, P_GOTO, "robotwithlog2.chkflag");
    pb_label(&b, "robotwithlog2.exit");
    pb_emit_trigger(&b, "robot.robanim", -1);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_goto(&b, P_IGOTO, "robot.robforce");

    // DPATHDAT.ASM:1309
    pb_start_path(&b, PATH_ID_ROBOTSWITHLOG, "robotswithlog");
    pb_emit_spawn_child(&b, 0, 0, -90, 0, 0, 0,
                        SH_ROBOT_0, PATH_ID_ROBOTWITHLOG2, 10, 10, 2);
    pb_emit_spawn_child(&b, 0, 0, 90, 0, 0, 0,
                        SH_ROBOT_0, PATH_ID_ROBOTWITHLOG2, 10, 10, 3);
    pb_label(&b, "robotswithlog.in");
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit_ifbetweenb(&b, PAL_ROTY, 0, 127, "robotswithlog.logoneside");
    pb_emit_spawn_child(&b, -20, -110, -100, DEG90, 0, 0,
                        SH_NULLSHAPE, PATH_ID_CARRIEDLOG, 10, 10, 1);
    pb_emit_goto(&b, P_IGOTO, "robotswithlog.logcreated");
    pb_label(&b, "robotswithlog.logoneside");
    pb_emit_spawn_child(&b, 20, -110, -100, DEG90, 0, 0,
                        SH_NULLSHAPE, PATH_ID_CARRIEDLOG, 10, 10, 1);
    pb_label(&b, "robotswithlog.logcreated");
    pb_emit8(&b, P_SETVEL);
    pb_emit8(&b, 30);
    pb_label(&b, "robotswithlog.waitabit");
    pb_emit8(&b, P_BEHINDPLAYER);
    pb_fixup16(&b, "premove");
    pb_emit_goto(&b, P_GOTO, "robotswithlog.waitabit");

    // DPATHDAT.ASM:1340
    pb_start_path(&b, PATH_ID_CARRIEDLOG, "carriedlog");
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 0);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit_importw(&b, PAL_SHAPE, PATH_EXT_GWORD1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsamew(&b, PAL_SHAPE, 0, "carriedlog.notapillar");
    pb_emit_set(&b, PAL_SHAPE, SH_PILLAR3_NS);
    pb_label(&b, "carriedlog.notapillar");
    pb_emit_ifsamew(&b, PAL_SHAPE, SH_BOSS_7_0, "carriedlog.rotit");
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsamew(&b, PAL_SHAPE, SH_BOSS_7_3, "carriedlog.norot");
    pb_label(&b, "carriedlog.rotit");
    pb_emit_add(&b, PAL_CHILDROTX, DEG90);
    pb_emit_add(&b, PAL_ROTX, DEG90);
    pb_label(&b, "carriedlog.norot");
    pb_label(&b, "carriedlog.chkagain");
    pb_emit_indexb(&b, PATH_EXT_SINTAB, PAL_ABS_PBYTE2, PAL_ABS_PBYTE1);
    pb_emit_div2b(&b, PAL_ABS_PBYTE1);
    pb_emit_div2b(&b, PAL_ABS_PBYTE1);
    pb_emit_div2b(&b, PAL_ABS_PBYTE1);
    pb_emit_div2b(&b, PAL_ABS_PBYTE1);
    pb_emit_div2b(&b, PAL_ABS_PBYTE1);
    pb_emit_div2b(&b, PAL_ABS_PBYTE1);
    pb_emit_add(&b, PAL_PBYTE1, -28);
    pb_emit_setv(&b, false, PAL_CHILDY, false, PAL_PBYTE1);
    pb_emit_add(&b, PAL_PBYTE2, 40);
    pb_emit8(&b, P_CHILDDEAD);
    pb_emit8(&b, 2);
    pb_fixup16(&b, "carriedlog.man1dead");
    pb_emit8(&b, P_CHILDDEAD);
    pb_emit8(&b, 3);
    pb_fixup16(&b, "carriedlog.man2dead");
    pb_emit_goto(&b, P_GOTO, "carriedlog.chkagain");
    pb_label(&b, "carriedlog.man1dead");
    pb_emit8(&b, P_FLAGCHILD);
    pb_emit8(&b, 3);
    pb_emit8(&b, P_UNLINKCHILD);
    pb_emit8(&b, 3);
    pb_emit_goto(&b, P_IGOTO, "carriedlog.mandead");
    pb_label(&b, "carriedlog.man2dead");
    pb_emit8(&b, P_FLAGCHILD);
    pb_emit8(&b, 2);
    pb_emit8(&b, P_UNLINKCHILD);
    pb_emit8(&b, 2);
    pb_label(&b, "carriedlog.mandead");
    pb_emit8(&b, P_UNLINKCHILD);
    pb_emit8(&b, 1);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setb(&b, PAL_PBYTE1, -50);
    pb_label(&b, "carriedlog.droplog");
    pb_emit8(&b, P_WAIT1);
    pb_emit_add(&b, PAL_PBYTE1, 8);
    pb_emit_addv(&b, true, PAL_WORLDY, false, PAL_PBYTE1);
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -1100, -5, "carriedlog.droplog");
    pb_emit_set(&b, PAL_WORLDY, 0);
    pb_emit_negb(&b, PAL_ABS_PBYTE1);
    pb_emit_div2b(&b, PAL_ABS_PBYTE1);
    pb_emit_div2b(&b, PAL_ABS_PBYTE1);
    pb_emit_soundeffect(&b, 0x49);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifbetweenb(&b, PAL_PBYTE1, -3, 0, "carriedlog.droplog");
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:593
    pb_start_path(&b, PATH_ID_E_ASTE, "e_aste");
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit_sprite(&b, 0, 0);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:601
    pb_start_path(&b, PATH_ID_E_ASTE_B, "e_aste_b");
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit_sprite(&b, 0, 0);
    pb_emit8(&b, P_ZREMOVEON);
    pb_label(&b, "e_aste_b.ast_bw");
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "e_aste_b.ast_bb");
    pb_emit_goto(&b, P_GOTO, "e_aste_b.ast_bw");
    pb_label(&b, "e_aste_b.ast_bb");
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE1, 31);
    pb_emit_goto(&b, P_RANDOMGOTO, "e_aste_b.ast_ne1");
    pb_emit_negb(&b, PAL_ABS_PBYTE1);
    pb_label(&b, "e_aste_b.ast_ne1");
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE2, 31);
    pb_emit_goto(&b, P_RANDOMGOTO, "e_aste_b.ast_ne2");
    pb_emit_negb(&b, PAL_ABS_PBYTE2);
    pb_label(&b, "e_aste_b.ast_ne2");
    pb_emit_addvwb(&b, PAL_WORLDX, PAL_PBYTE1);
    pb_emit_addvwb(&b, PAL_WORLDY, PAL_PBYTE2);
    pb_emit_goto(&b, P_GOTO, "e_aste_b.ast_ne2");

    // PATHDATA.ASM:629
    pb_start_path(&b, PATH_ID_E_BREASTE, "e_breaste");
    pb_emit_setstrat_flat(&b, STRAT_ID_BREAK_METEOR);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:637
    pb_start_path(&b, PATH_ID_INSEKIKUN, "insekikun");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit8(&b, P_INVISIBLEON);
    pb_emit_spawn_child(&b, 65, 0, 0, 0, 0, 0,
                        SH_ASTEROID1, PATH_ID_E_ASTE_B, 10, 8, 1);
    pb_emit_spawn_child(&b, PATH_I8(130), 0, 0, 0, 0, 0,
                        SH_ASTEROID1, PATH_ID_E_ASTE_B, 10, 8, 2);
    pb_emit_spawn_child(&b, -65, 0, 0, 0, 0, 0,
                        SH_ASTEROID1, PATH_ID_E_ASTE_B, 10, 8, 3);
    pb_emit_spawn_child(&b, PATH_I8(-130), 0, 0, 0, 0, 0,
                        SH_ASTEROID1, PATH_ID_E_ASTE_B, 10, 8, 4);
    pb_emit_spawn_child(&b, 0, 0, 0, 0, 0, 0,
                        SH_ASTEROID1, PATH_ID_E_BREASTE, 10, 10, 5);
    pb_label(&b, "insekikun.inse_0");
    pb_emit_add(&b, PAL_ROTZ, 4);
    pb_emit8(&b, P_CHILDDEAD);
    pb_emit8(&b, 5);
    pb_fixup16(&b, "insekikun.unlinast");
    pb_emit8(&b, P_BEHINDPLAYER);
    pb_fixup16(&b, "insekikun.inse_1");
    pb_emit_goto(&b, P_GOTO, "insekikun.inse_0");
    pb_label(&b, "insekikun.inse_1");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "insekikun.unlin_0");
    pb_emit_wait(&b, 20);
    pb_emit8(&b, P_END);
    pb_label(&b, "insekikun.inse_2");
    pb_emit8(&b, P_EXPLODE);
    pb_emit8(&b, P_END);
    pb_label(&b, "insekikun.unlinast");
    pb_emit8(&b, P_FLAGCHILD);
    pb_emit8(&b, 1);
    pb_emit8(&b, P_FLAGCHILD);
    pb_emit8(&b, 2);
    pb_emit8(&b, P_FLAGCHILD);
    pb_emit8(&b, 3);
    pb_emit8(&b, P_FLAGCHILD);
    pb_emit8(&b, 4);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "insekikun.unlin_0");
    pb_emit_goto(&b, P_IGOTO, "insekikun.inse_2");
    pb_label(&b, "insekikun.unlin_0");
    pb_emit8(&b, P_UNLINKCHILD);
    pb_emit8(&b, 1);
    pb_emit8(&b, P_UNLINKCHILD);
    pb_emit8(&b, 2);
    pb_emit8(&b, P_UNLINKCHILD);
    pb_emit8(&b, 3);
    pb_emit8(&b, P_UNLINKCHILD);
    pb_emit8(&b, 4);
    pb_emit8(&b, P_UNLINKCHILD);
    pb_emit8(&b, 5);
    pb_emit8(&b, P_RETURN);

    // PATHDATA.ASM:676
    pb_start_path(&b, PATH_ID_PYONTA, "pyonta");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELSLOWELASER);
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit_trigger(&b, "pyonta.pyonta_j", PATH_TRIGGER_WHENHITBYPLAYER_VALUE);
    pb_emit_trigger(&b, "pyonta.pyonta_e", PATH_TRIGGER_WHENDEAD_VALUE);
    pb_emit_trigger(&b, "pyonta.pyonta_h", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit_setb(&b, PAL_PBYTE2, 20);
    pb_emit_zero(&b, PAL_PWORD1);
    pb_emit_setb(&b, PAL_HP, 4);
    pb_emit_iflevel(&b, 2, "pyonta.pyonta_s");
    pb_emit_spawn(&b, P_SPAWN, 0, 12, 0, 0, 0, 0,
                  SH_ASTEROID1, PATH_ID_E_ASTE, 10, 8, 0);
    pb_emit_spawn(&b, P_SPAWN, -100, 12, 0, 0, 0, 0,
                  SH_ASTEROID1, PATH_ID_E_ASTE, 10, 8, 0);
    pb_label(&b, "pyonta.pyonta_s");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pyonta.pyonlr");
    pb_emit_setb(&b, PAL_PBYTE1, -47);
    pb_emit_do(&b, 5);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pyonta.pyon_up");
    pb_emit_add(&b, PAL_PBYTE1, 4);
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pyonta.pyon_up");
    pb_emit_add(&b, PAL_PBYTE1, 6);
    pb_emit8(&b, P_NEXT);
    pb_emit_distless(&b, 1000, "pyonta.pyon_pass");
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_emit_add(&b, PAL_WORLDY, -50);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "facefire");
    pb_emit_zero(&b, PAL_ROTY);
    pb_emit_zero(&b, PAL_ROTX);
    pb_emit_add(&b, PAL_WORLDY, 50);
    pb_label(&b, "pyonta.pyon_pass");
    pb_emit_setb(&b, PAL_PBYTE1, 3);
    pb_emit_do(&b, 5);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pyonta.pyon_up");
    pb_emit_add(&b, PAL_PBYTE1, 4);
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pyonta.pyon_up");
    pb_emit_add(&b, PAL_PBYTE1, 6);
    pb_emit8(&b, P_NEXT);
    pb_emit_wait(&b, 10);
    pb_emit_goto(&b, P_IGOTO, "pyonta.pyonta_s");
    pb_label(&b, "pyonta.pyon_rem");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);
    pb_label(&b, "pyonta.pyon_up");
    pb_emit_addvwb(&b, PAL_WORLDY, PAL_PBYTE1);
    pb_emit_addvwb(&b, PAL_WORLDX, PAL_PBYTE2);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsamew(&b, PAL_PWORD1, 0, "pyonta.pyon_ru");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pyonta.pyon_ru");
    pb_emit_add(&b, PAL_ROTY, 32);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pyonta.pyonlr");
    pb_emit_ifsameb(&b, PAL_PBYTE2, 20, "pyonta.pyon_l");
    pb_emit_setb(&b, PAL_PBYTE2, 20);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pyonta.pyon_l");
    pb_emit_setb(&b, PAL_PBYTE2, -20);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pyonta.pyonta_j");
    pb_emit_goto(&b, P_FORCE, "pyonta.pyonta_bye");
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE1, 31);
    pb_emit_goto(&b, P_RANDOMGOTO, "pyonta.pyon_ne1");
    pb_emit_negb(&b, PAL_ABS_PBYTE1);
    pb_label(&b, "pyonta.pyon_ne1");
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE2, 31);
    pb_emit_goto(&b, P_RANDOMGOTO, "pyonta.pyon_ne2");
    pb_emit_negb(&b, PAL_ABS_PBYTE2);
    pb_label(&b, "pyonta.pyon_ne2");
    pb_emit_set(&b, PAL_PWORD1, 1);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pyonta.pyonta_bye");
    pb_emit_addw(&b, PAL_PWORD1, 1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsamew(&b, PAL_PWORD1, 100, "pyonta.bye_tru");
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "pyonta.bye_tru");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pyonta.pyon_up");
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -1000, 1000, "pyonta.pyon_byey");
    pb_emit_goto(&b, P_GOTO, "pyonta.pyonta_bye");
    pb_label(&b, "pyonta.pyon_byey");
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -1000, 1000, "pyonta.pyon_rem");
    pb_emit_goto(&b, P_GOTO, "pyonta.pyonta_bye");
    pb_label(&b, "pyonta.pyonta_e");
    pb_emit_goto(&b, P_FORCE, "pyonta.pyon_ex");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pyonta.pyon_ex");
    pb_emit_add(&b, PAL_WORLDY, -50);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pyonta.pyonta_h");
    pb_emit_distless(&b, 2500, "pyonta.pyonta_hit");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pyonta.pyonta_hit");
    pb_emit8(&b, P_COLLISIONSON);
    pb_emit8(&b, P_RETURN);

    // PATHDATA.ASM:5469
    pb_start_path(&b, PATH_ID_SCREW, "screw");
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELOVALBEAM);
    pb_emit_trigger(&b, "pdamyscr.scr_rot", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit_setb(&b, PAL_HP, 2);
    pb_emit_setb(&b, PAL_AP, 6);
    pb_label(&b, "screw.scr_0");
    pb_emit_distless(&b, 1500, "screw.scr_1");
    pb_emit_goto(&b, P_GOTO, "screw.scr_0");
    pb_label(&b, "screw.scr_1");
    pb_emit_trigger(&b, "pdamyscr.scr_rot", -1);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit_goto(&b, P_RANDOMGOTO, "screw.scr_3");
    pb_emit_setb(&b, PAL_ROTY, 128 + 32);
    pb_emit_setb(&b, PAL_ROTX, 192 + 32);
    pb_emit_do(&b, 10);
    pb_emit8(&b, P_FIRE);
    pb_emit_add(&b, PAL_ROTX, 6);
    pb_emit_add(&b, PAL_ROTY, -6);
    pb_emit8(&b, P_NEXT);
    pb_emit_goto(&b, P_IGOTO, "screw.scr_2");
    pb_label(&b, "screw.scr_3");
    pb_emit_setb(&b, PAL_ROTY, 128 - 32);
    pb_emit_setb(&b, PAL_ROTX, 192 + 32);
    pb_emit_do(&b, 10);
    pb_emit8(&b, P_FIRE);
    pb_emit_add(&b, PAL_ROTX, 6);
    pb_emit_add(&b, PAL_ROTY, 6);
    pb_emit8(&b, P_NEXT);
    pb_label(&b, "screw.scr_2");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit_trigger(&b, "pdamyscr.scr_rot", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:5515
    pb_start_path(&b, PATH_ID_DAMYSCR, "damyscr");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_emit_trigger(&b, "pdamyscr.scr_rot", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit_setb(&b, PAL_HP, 2);
    pb_emit_setb(&b, PAL_AP, 6);
    pb_emit8(&b, P_END);
    pb_label(&b, "pdamyscr.scr_rot");
    pb_emit_add(&b, PAL_ROTX, 8);
    pb_emit_add(&b, PAL_ROTY, -8);
    pb_emit8(&b, P_RETURN);

    // PATHDATA.ASM:5543
    pb_start_path(&b, PATH_ID_CHASE1_1, "chase1_1");
    pb_emit_friend(&b, FRIEND_ANYONE);
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "friend_init");
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_notfriend(&b, FRIEND_ANYONE, "chase1_1.c1_1_e");
    pb_label(&b, "chase1_1.c1_1_0");
    pb_emit_setb(&b, PAL_ROTY, DEG90);
    pb_emit_setvel(&b, 30);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase1_1.c1_1_m2");
    pb_emit_message_meter(&b, 7);
    pb_emit_goto(&b, P_GOTO, "chase1_1.c1_1_4");
    pb_label(&b, "chase1_1.c1_1_m2");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase1_1.c1_1_m3");
    pb_emit_message_meter(&b, 27);
    pb_emit_goto(&b, P_GOTO, "chase1_1.c1_1_4");
    pb_label(&b, "chase1_1.c1_1_m3");
    pb_emit_message_meter(&b, 47);
    pb_label(&b, "chase1_1.c1_1_4");
    pb_emit_wait(&b, 35);
    pb_label(&b, "chase1_1.c1_1_1");
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_add(&b, PAL_ROTX, -4);
    pb_emit_add(&b, PAL_ROTZ, -1);
    pb_emit_ifsameb(&b, PAL_ROTZ, (uint8)-64, "chase1_1.c1_1_2");
    pb_emit_goto(&b, P_GOTO, "chase1_1.c1_1_1");
    pb_label(&b, "chase1_1.c1_1_2");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 20, "chase1_1.c1_1_2");
    pb_label(&b, "chase1_1.c1_1_21");
    pb_emit_wait(&b, 30);
    pb_emit_trigger(&b, "save", -1);
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase1_1.c1_1_c");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase1_1.c1_1_2a");
    pb_emit_message_meter(&b, 15);
    pb_emit_goto(&b, P_IGOTO, "chase1_1.c1_a_3");
    pb_label(&b, "chase1_1.c1_1_2a");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase1_1.c1_1_2b");
    pb_emit_message_meter(&b, 35);
    pb_emit_goto(&b, P_IGOTO, "chase1_1.c1_a_3");
    pb_label(&b, "chase1_1.c1_1_2b");
    pb_emit_message_meter(&b, 55);
    pb_label(&b, "chase1_1.c1_a_3");
    pb_emit8(&b, P_DAMAGE);
    pb_label(&b, "chase1_1.c1_a_30");
    pb_emit_add(&b, PAL_ROTZ, 4);
    pb_emit_loop(&b, 7, "chase1_1.c1_a_30");
    pb_label(&b, "chase1_1.c1_a_31");
    pb_emit_wait(&b, 21);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_accel(&b, 60, 5);
    pb_emit_waitchaseb(&b, PAL_ROTX, 234);
    pb_label(&b, "chase1_1.c1_a_32");
    pb_emit_wait(&b, 7);
    pb_emit_goto(&b, P_GOTO, "chase1_1.c1_1_e");
    pb_label(&b, "chase1_1.c1_1_c");
    pb_emit8(&b, P_FLAGSHAPE);
    pb_emit_trigger(&b, "yamete", -1);
    pb_emit_notfriend(&b, FRIEND_FROG, "chase1_1.c1_1_h");
    pb_emit_wait(&b, 1);
    pb_label(&b, "chase1_1.c1_1_h");
    pb_emit8(&b, P_SMOKEON);
    pb_emit_wait(&b, 4);
    pb_emit8(&b, P_SMOKEOFF);
    pb_emit_accel(&b, 10, 1);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase1_1.c1_1_m6");
    pb_emit_message_meter(&b, 9);
    pb_emit_goto(&b, P_GOTO, "chase1_1.c1_1_d");
    pb_label(&b, "chase1_1.c1_1_m6");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase1_1.c1_1_m7");
    pb_emit_message_meter(&b, 29);
    pb_emit_goto(&b, P_GOTO, "chase1_1.c1_1_d");
    pb_label(&b, "chase1_1.c1_1_m7");
    pb_emit_message_meter(&b, 49);
    pb_label(&b, "chase1_1.c1_1_d");
    pb_emit8(&b, P_DAMAGE);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_emit_waitchaseb(&b, PAL_ROTY, 0);
    pb_label(&b, "chase1_1.c1_1_f");
    pb_emit_chaseb(&b, PAL_VEL, (uint8)-20);
    pb_emit_add(&b, PAL_WORLDY, -3);
    pb_emit_add(&b, PAL_ROTZ, 10);
    pb_emit_loop(&b, 50, "chase1_1.c1_1_f");
    pb_emit8(&b, P_SMOKEON);
    pb_label(&b, "chase1_1.c1_1_g");
    pb_emit_add(&b, PAL_ROTZ, 10);
    pb_emit_loop(&b, 30, "chase1_1.c1_1_g");
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "chase1_1.c1_1_e");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:5674
    pb_start_path(&b, PATH_ID_CHASE1_2, "chase1_2");
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "chase_init");
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_label(&b, "chase1_2.c1_2_0");
    pb_emit_wait(&b, 12);
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase1_2.c1_2_z");
    pb_emit_setb(&b, PAL_ROTY, DEG90);
    pb_emit_setvel(&b, 30);
    pb_emit_wait(&b, 20);
    pb_emit8(&b, P_COLLISIONSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 15);
    pb_label(&b, "chase1_2.c1_2_1");
    pb_emit_add(&b, PAL_ROTX, -4);
    pb_emit_add(&b, PAL_ROTZ, -1);
    pb_emit_loop(&b, 63, "chase1_2.c1_2_1");
    pb_label(&b, "chase1_2.c1_2_2");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 20, "chase1_2.c1_2_2");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 8);
    pb_emit8(&b, P_FIRE);
    pb_emit_set(&b, PAL_HP, 100);
    pb_emit_wait(&b, 8);
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 14);
    pb_emit_set(&b, PAL_HP, 2);
    pb_label(&b, "chase1_2.c1_2_4");
    pb_emit_add(&b, PAL_ROTZ, 4);
    pb_emit_loop(&b, 7, "chase1_2.c1_2_4");
    pb_emit_wait(&b, 20);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_accel(&b, 60, 5);
    pb_emit_waitchaseb(&b, PAL_ROTX, 234);
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "chase1_2.c1_2_y");
    pb_emit_wait(&b, 7);
    pb_emit_setvel(&b, 0);
    pb_emit_goto(&b, P_GOTO, "chase1_2.c1_2_z");
    pb_label(&b, "chase1_2.c1_2_y");
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_emit_waitchaseb(&b, PAL_ROTY, 128);
    pb_emit_set(&b, PAL_HP, 10);
    pb_emit_gotopos(&b, 0, 0, 600, 40);
    pb_emit8(&b, P_WAITFACEPLAYER);
    pb_emit_add(&b, PAL_ROTZ, 16);
    pb_emit_wait(&b, 5);
    pb_emit_add(&b, PAL_ROTZ, -32);
    pb_emit_wait(&b, 5);
    pb_emit_add(&b, PAL_ROTZ, 16);
    pb_emit_wait(&b, 5);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_wait(&b, 5);
    pb_emit8(&b, P_SMOKEOFF);
    pb_emit_waitchaseb(&b, PAL_ROTY, DEG90);
    pb_emit_accel(&b, 50, 2);
    pb_emit_wait(&b, 50);
    pb_label(&b, "chase1_2.c1_2_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:1080
    pb_start_path(&b, PATH_ID_CHECK, "check");
    pb_emit8(&b, P_INVISIBLEON);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit_zero(&b, PAL_PBYTE1);
    pb_label(&b, "check.ck_st");
    pb_emit8(&b, P_LEFTOFPLAYER);
    pb_fixup16(&b, "check.ck_l");
    pb_label(&b, "check.ck_r");
    pb_emit_add(&b, PAL_WORLDX, -40);
    pb_emit8(&b, P_LEFTOFPLAYER);
    pb_fixup16(&b, "check.ck_r1");
    pb_emit_goto(&b, P_GOTO, "check.ck_r");
    pb_label(&b, "check.ck_r1");
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -350, 350, "check.ck_atck");
    pb_emit_goto(&b, P_GOTO, "check.ck_wait");
    pb_label(&b, "check.ck_l");
    pb_emit_add(&b, PAL_WORLDX, 40);
    pb_emit8(&b, P_RIGHTOFPLAYER);
    pb_fixup16(&b, "check.ck_l1");
    pb_emit_goto(&b, P_GOTO, "check.ck_l");
    pb_label(&b, "check.ck_l1");
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -350, 350, "check.ck_atck");
    pb_emit_goto(&b, P_GOTO, "check.ck_wait");
    pb_label(&b, "check.ck_atck");
    pb_emit_qspawn(&b, SH_S_HOU_0, PATH_ID_AT_HBEAM, 10, 10);
    pb_label(&b, "check.ck_wait");
    pb_emit_wait(&b, 50);
    pb_emit_add(&b, PAL_PBYTE1, 1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_PBYTE1, 6, "check.ck_st");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:1114
    pb_start_path(&b, PATH_ID_AT_HBEAM, "at_hbeam");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit_zero(&b, PAL_PBYTE1);
    pb_emit_trigger(&b, "at_hbeam.at_rot", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_HPLASMA);
    pb_emit_set(&b, PAL_ROTY, 128);
    pb_label(&b, "at_hbeam.at_down1");
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -500, 200, "at_hbeam.at_atck");
    pb_emit_goto(&b, P_GOTO, "at_hbeam.at_down1");
    pb_label(&b, "at_hbeam.at_atck");
    pb_emit_do(&b, 5);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "facefire");
    pb_emit_wait(&b, 4);
    pb_emit8(&b, P_NEXT);
    pb_label(&b, "at_hbeam.at_down2");
    pb_emit_wait(&b, 10);
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);
    pb_label(&b, "at_hbeam.at_rot");
    pb_emit_add(&b, PAL_ROTZ, 8);
    pb_emit_add(&b, PAL_WORLDY, 30);
    pb_emit8(&b, P_RETURN);

    // PATHDATA.ASM:2453
    pb_start_path(&b, PATH_ID_EGU6, "egu6");
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELSLOWELASER);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 3);
    pb_emit_soundeffect(&b, 3);
    pb_emit_setb(&b, PAL_HP, 2);
    pb_emit_setb(&b, PAL_AP, 6);
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "egu6.e6_a");
    pb_emit8(&b, P_RIGHTOFPLAYER);
    pb_fixup16(&b, "egu6.e6_br");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_bl");
    pb_label(&b, "egu6.e6_a");
    pb_emit8(&b, P_RIGHTOFPLAYER);
    pb_fixup16(&b, "egu6.e6_ar");
    pb_emit_set(&b, PAL_ROTY, (uint8)-64);
    pb_emit_set(&b, PAL_ROTX, 32);
    pb_emit_setvel(&b, 60);
    pb_label(&b, "egu6.e6_al_1");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "egu6.e6_al_1");
    pb_emit_accel(&b, 30, 1);
    pb_label(&b, "egu6.e6_al_2");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 32);
    pb_emit_chaseb(&b, PAL_ROTY, (uint8)-32);
    pb_emit_ifsameb(&b, PAL_ROTY, (uint8)-32, "egu6.e6_al_3");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_al_2");
    pb_label(&b, "egu6.e6_al_3");
    pb_emit_chaseb(&b, PAL_ROTX, (uint8)-64);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_ifsameb(&b, PAL_ROTX, (uint8)-64, "egu6.e6_al_4");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_al_3");
    pb_label(&b, "egu6.e6_al_4");
    pb_emit_add(&b, PAL_ROTY, 4);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_ifsameb(&b, PAL_ROTY, 128, "egu6.e6_0");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_al_4");
    pb_label(&b, "egu6.e6_0");
    pb_emit_accel(&b, 40, 1);
    pb_emit_do(&b, 4);
    pb_emit_do(&b, 11);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_NEXT);
    pb_label(&b, "egu6.e6_4");
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit_wait(&b, 3);
    pb_emit_loop(&b, 10, "egu6.e6_4");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_z");
    pb_label(&b, "egu6.e6_ar");
    pb_emit_set(&b, PAL_ROTY, 64);
    pb_emit_set(&b, PAL_ROTX, 32);
    pb_emit_setvel(&b, 60);
    pb_label(&b, "egu6.e6_ar_1");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "egu6.e6_ar_1");
    pb_emit_accel(&b, 30, 1);
    pb_label(&b, "egu6.e6_ar_2");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, (uint8)-32);
    pb_emit_chaseb(&b, PAL_ROTY, 32);
    pb_emit_ifsameb(&b, PAL_ROTY, 32, "egu6.e6_ar_3");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_ar_2");
    pb_label(&b, "egu6.e6_ar_3");
    pb_emit_chaseb(&b, PAL_ROTX, (uint8)-64);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_ifsameb(&b, PAL_ROTX, (uint8)-64, "egu6.e6_ar_4");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_ar_3");
    pb_label(&b, "egu6.e6_ar_4");
    pb_emit_add(&b, PAL_ROTY, -4);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_ifsameb(&b, PAL_ROTY, 128, "egu6.e6_0");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_ar_4");
    pb_label(&b, "egu6.e6_bl");
    pb_emit_set(&b, PAL_ROTY, (uint8)-64);
    pb_emit_set(&b, PAL_ROTX, (uint8)-32);
    pb_emit_setvel(&b, 60);
    pb_label(&b, "egu6.e6_bl_1");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "egu6.e6_bl_11");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_bl_1");
    pb_label(&b, "egu6.e6_bl_11");
    pb_emit_accel(&b, 30, 1);
    pb_label(&b, "egu6.e6_bl_2");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 32);
    pb_emit_chaseb(&b, PAL_ROTY, (uint8)-32);
    pb_emit_ifsameb(&b, PAL_ROTY, (uint8)-32, "egu6.e6_bl_3");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_bl_2");
    pb_label(&b, "egu6.e6_bl_3");
    pb_emit_chaseb(&b, PAL_ROTX, 40);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_ifsameb(&b, PAL_ROTX, 40, "egu6.e6_bl_4");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_bl_3");
    pb_label(&b, "egu6.e6_bl_4");
    pb_emit_add(&b, PAL_ROTY, 4);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_ifsameb(&b, PAL_ROTY, 128, "egu6.e6_0");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_bl_4");
    pb_label(&b, "egu6.e6_br");
    pb_emit_set(&b, PAL_ROTY, 64);
    pb_emit_set(&b, PAL_ROTX, (uint8)-32);
    pb_emit_setvel(&b, 60);
    pb_label(&b, "egu6.e6_br_1");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "egu6.e6_br_11");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_br_1");
    pb_label(&b, "egu6.e6_br_11");
    pb_emit_accel(&b, 30, 1);
    pb_label(&b, "egu6.e6_br_2");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, (uint8)-32);
    pb_emit_chaseb(&b, PAL_ROTY, 32);
    pb_emit_ifsameb(&b, PAL_ROTY, 32, "egu6.e6_br_3");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_br_2");
    pb_label(&b, "egu6.e6_br_3");
    pb_emit_chaseb(&b, PAL_ROTX, 40);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_ifsameb(&b, PAL_ROTX, 40, "egu6.e6_br_4");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_br_3");
    pb_label(&b, "egu6.e6_br_4");
    pb_emit_add(&b, PAL_ROTY, -4);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_ifsameb(&b, PAL_ROTY, 128, "egu6.e6_0");
    pb_emit_goto(&b, P_GOTO, "egu6.e6_br_4");
    pb_label(&b, "egu6.e6_z");
    pb_emit_wait(&b, 30);
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:8356
    pb_start_path(&b, PATH_ID_EGU6_IRAB, "egu6_irab");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "egu6_ifal.e6i_init");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "egu6_ifal.e6i_sub");
    pb_emit_add(&b, PAL_WORLDZ, -1500);
    pb_emit_spawn_link(&b, 0, PATH_I8(-300), 0, 0, 128, 0,
                       SH_FRIENDSHIP_4, PATH_ID_SEPTER_RAB, 100, 1);
    pb_emit_add(&b, PAL_WORLDZ, 1500);
    pb_emit_goto(&b, P_IGOTO, "egu6_ifal.e6i_tim");

    // PATHDATA.ASM:8367
    pb_start_path(&b, PATH_ID_EGU6_IFRO, "egu6_ifro");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "egu6_ifal.e6i_init");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "egu6_ifal.e6i_sub");
    pb_emit_add(&b, PAL_WORLDZ, -1500);
    pb_emit_spawn_link(&b, 0, PATH_I8(-300), 0, 0, 128, 0,
                       SH_FRIENDSHIP_4, PATH_ID_SEPTER_FRO, 100, 1);
    pb_emit_add(&b, PAL_WORLDZ, 1500);
    pb_emit_goto(&b, P_IGOTO, "egu6_ifal.e6i_tim");

    // PATHDATA.ASM:8378
    pb_start_path(&b, PATH_ID_EGU6_IFAL, "egu6_ifal");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "egu6_ifal.e6i_init");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "egu6_ifal.e6i_sub");
    pb_emit_add(&b, PAL_WORLDZ, -1500);
    pb_emit_spawn_link(&b, 0, PATH_I8(-300), 0, 0, 128, 0,
                       SH_FRIENDSHIP_4, PATH_ID_SEPTER_FAL, 100, 1);
    pb_emit_add(&b, PAL_WORLDZ, 1500);

    pb_label(&b, "egu6_ifal.e6i_tim");
    pb_emit_setb(&b, PAL_HP, 30);
    pb_emit_setvel(&b, 30);
    pb_emit_wait(&b, 25);
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "egu6_ifal.e6i_z");
    pb_emit8(&b, P_EXPLODE);

    pb_label(&b, "egu6_ifal.e6i_z");
    pb_emit_wait(&b, 30);
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    pb_label(&b, "egu6_ifal.e6i_init");
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELSLOWELASER);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 3);
    pb_emit_soundeffect(&b, 3);
    pb_emit_setb(&b, PAL_PBYTE1, 0);
    pb_emit_exportb(&b, PAL_PBYTE1, PATH_EXT_EBYTE2);
    pb_emit_setb(&b, PAL_HP, 2);
    pb_emit_setb(&b, PAL_AP, 6);
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "egu6_ifal.e6i_sub");
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "egu6_ifal.e6i_a");
    pb_emit8(&b, P_RIGHTOFPLAYER);
    pb_fixup16(&b, "egu6_ifal.e6i_br");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_bl");
    pb_label(&b, "egu6_ifal.e6i_a");
    pb_emit8(&b, P_RIGHTOFPLAYER);
    pb_fixup16(&b, "egu6_ifal.e6i_ar");
    pb_emit_set(&b, PAL_ROTY, (uint8)-64);
    pb_emit_set(&b, PAL_ROTX, 32);
    pb_emit_setvel(&b, 60);
    pb_label(&b, "egu6_ifal.e6i_al_1");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "egu6_ifal.e6i_al_1");
    pb_emit_accel(&b, 30, 1);
    pb_label(&b, "egu6_ifal.e6i_al_2");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 32);
    pb_emit_chaseb(&b, PAL_ROTY, (uint8)-32);
    pb_emit_ifsameb(&b, PAL_ROTY, (uint8)-32, "egu6_ifal.e6i_al_3");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_al_2");
    pb_label(&b, "egu6_ifal.e6i_al_3");
    pb_emit_chaseb(&b, PAL_ROTX, (uint8)-64);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_ifsameb(&b, PAL_ROTX, (uint8)-64, "egu6_ifal.e6i_al_4");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_al_3");
    pb_label(&b, "egu6_ifal.e6i_al_4");
    pb_emit_add(&b, PAL_ROTY, 4);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_ifsameb(&b, PAL_ROTY, 128, "egu6_ifal.e6i_0");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_al_4");
    pb_label(&b, "egu6_ifal.e6i_0");
    pb_emit_accel(&b, 40, 1);
    pb_label(&b, "egu6_ifal.e6i_01");
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit_distless(&b, 1300, "egu6_ifal.e6i_02");
    pb_emit_add(&b, PAL_PBYTE1, 1);
    pb_emit_ifsameb(&b, PAL_PBYTE1, 10, "egu6_ifal.e6i_03");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_01");
    pb_label(&b, "egu6_ifal.e6i_03");
    pb_emit_setb(&b, PAL_PBYTE1, 0);
    pb_emit8(&b, P_FIRE);
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_01");
    pb_label(&b, "egu6_ifal.e6i_02");
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "egu6_ifal.e6i_ar");
    pb_emit_set(&b, PAL_ROTY, 64);
    pb_emit_set(&b, PAL_ROTX, 32);
    pb_emit_setvel(&b, 60);
    pb_label(&b, "egu6_ifal.e6i_ar_1");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "egu6_ifal.e6i_ar_1");
    pb_emit_accel(&b, 30, 1);
    pb_label(&b, "egu6_ifal.e6i_ar_2");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, (uint8)-32);
    pb_emit_chaseb(&b, PAL_ROTY, 32);
    pb_emit_ifsameb(&b, PAL_ROTY, 32, "egu6_ifal.e6i_ar_3");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_ar_2");
    pb_label(&b, "egu6_ifal.e6i_ar_3");
    pb_emit_chaseb(&b, PAL_ROTX, (uint8)-64);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_ifsameb(&b, PAL_ROTX, (uint8)-64, "egu6_ifal.e6i_ar_4");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_ar_3");
    pb_label(&b, "egu6_ifal.e6i_ar_4");
    pb_emit_add(&b, PAL_ROTY, -4);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_ifsameb(&b, PAL_ROTY, 128, "egu6_ifal.e6i_0");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_ar_4");

    pb_label(&b, "egu6_ifal.e6i_bl");
    pb_emit_set(&b, PAL_ROTY, (uint8)-64);
    pb_emit_set(&b, PAL_ROTX, (uint8)-32);
    pb_emit_setvel(&b, 60);
    pb_label(&b, "egu6_ifal.e6i_bl_1");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "egu6_ifal.e6i_bl_11");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_bl_1");
    pb_label(&b, "egu6_ifal.e6i_bl_11");
    pb_emit_accel(&b, 30, 1);
    pb_label(&b, "egu6_ifal.e6i_bl_2");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 32);
    pb_emit_chaseb(&b, PAL_ROTY, (uint8)-32);
    pb_emit_ifsameb(&b, PAL_ROTY, (uint8)-32, "egu6_ifal.e6i_bl_3");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_bl_2");
    pb_label(&b, "egu6_ifal.e6i_bl_3");
    pb_emit_chaseb(&b, PAL_ROTX, 40);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_ifsameb(&b, PAL_ROTX, 40, "egu6_ifal.e6i_bl_4");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_bl_3");
    pb_label(&b, "egu6_ifal.e6i_bl_4");
    pb_emit_add(&b, PAL_ROTY, 4);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_ifsameb(&b, PAL_ROTY, 128, "egu6_ifal.e6i_0");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_bl_4");

    pb_label(&b, "egu6_ifal.e6i_br");
    pb_emit_set(&b, PAL_ROTY, 64);
    pb_emit_set(&b, PAL_ROTX, (uint8)-32);
    pb_emit_setvel(&b, 60);
    pb_label(&b, "egu6_ifal.e6i_br_1");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "egu6_ifal.e6i_br_11");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_br_1");
    pb_label(&b, "egu6_ifal.e6i_br_11");
    pb_emit_accel(&b, 30, 1);
    pb_label(&b, "egu6_ifal.e6i_br_2");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, (uint8)-32);
    pb_emit_chaseb(&b, PAL_ROTY, 32);
    pb_emit_ifsameb(&b, PAL_ROTY, 32, "egu6_ifal.e6i_br_3");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_br_2");
    pb_label(&b, "egu6_ifal.e6i_br_3");
    pb_emit_chaseb(&b, PAL_ROTX, 40);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_ifsameb(&b, PAL_ROTX, 40, "egu6_ifal.e6i_br_4");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_br_3");
    pb_label(&b, "egu6_ifal.e6i_br_4");
    pb_emit_add(&b, PAL_ROTY, -4);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_ifsameb(&b, PAL_ROTY, 128, "egu6_ifal.e6i_0");
    pb_emit_goto(&b, P_GOTO, "egu6_ifal.e6i_br_4");

    // PATHDATA.ASM:5770
    pb_start_path(&b, PATH_ID_CHASE2_1, "chase2_1");
    pb_emit_friend(&b, FRIEND_ANYONE);
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "friend_init");
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_notfriend(&b, FRIEND_ANYONE, "chase2_1.c2_1_z");
    pb_label(&b, "chase2_1.c2_1_0");
    pb_emit_chaseb(&b, PAL_ROTY, 200);
    pb_emit_chaseb(&b, PAL_ROTZ, 224);
    pb_emit_loop(&b, 20, "chase2_1.c2_1_0");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase2_1.c2_1_m1");
    pb_emit_message_meter(&b, 7);
    pb_emit_goto(&b, P_GOTO, "chase2_1.c2_1_1");
    pb_label(&b, "chase2_1.c2_1_m1");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase2_1.c2_1_m2");
    pb_emit_message_meter(&b, 27);
    pb_emit_goto(&b, P_GOTO, "chase2_1.c2_1_1");
    pb_label(&b, "chase2_1.c2_1_m2");
    pb_emit_message_meter(&b, 47);
    pb_label(&b, "chase2_1.c2_1_1");
    pb_emit_soundeffect(&b, 1);
    pb_emit_setvel(&b, 30);
    pb_label(&b, "chase2_1.c2_1_11");
    pb_emit_wait(&b, 40);
    pb_label(&b, "chase2_1.c2_1_2");
    pb_emit_chaseb(&b, PAL_ROTY, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 20, "chase2_1.c2_1_2");
    pb_emit8(&b, P_SPACESHIPON);
    pb_label(&b, "chase2_1.c2_1_3");
    pb_emit_add(&b, PAL_ROTZ, 1);
    pb_emit_add(&b, PAL_ROTX, -2);
    pb_emit_loop(&b, 15, "chase2_1.c2_1_3");
    pb_emit_wait(&b, 1);
    pb_label(&b, "chase2_1.c2_1_4");
    pb_emit_add(&b, PAL_ROTZ, -1);
    pb_emit_add(&b, PAL_ROTX, 2);
    pb_emit_loop(&b, 15, "chase2_1.c2_1_4");
    pb_emit_wait(&b, 1);
    pb_emit_accel(&b, 40, 1);
    pb_label(&b, "chase2_1.c2_1_5");
    pb_emit_add(&b, PAL_ROTZ, 1);
    pb_emit_add(&b, PAL_ROTX, 2);
    pb_emit_loop(&b, 24, "chase2_1.c2_1_5");
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase2_1.c2_1_ta");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase2_1.c2_1_tm1");
    pb_emit_message_meter(&b, 69);
    pb_emit_goto(&b, P_IGOTO, "chase2_1.c2_1_tm3");
    pb_label(&b, "chase2_1.c2_1_tm1");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase2_1.c2_1_tm2");
    pb_emit_message_meter(&b, 70);
    pb_emit_goto(&b, P_IGOTO, "chase2_1.c2_1_tm3");
    pb_label(&b, "chase2_1.c2_1_tm2");
    pb_emit_message_meter(&b, 71);
    pb_label(&b, "chase2_1.c2_1_tm3");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_wait(&b, 3);
    pb_emit_accel(&b, 30, 1);
    pb_label(&b, "chase2_1.c2_1_6");
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_chaseb(&b, PAL_ROTX, 245);
    pb_emit_loop(&b, 20, "chase2_1.c2_1_6");
    pb_emit_wait(&b, 4);
    pb_emit_add(&b, PAL_ROTZ, -13);
    pb_label(&b, "chase2_1.c2_1_61");
    pb_emit_wait(&b, 21);
    pb_emit_add(&b, PAL_ROTZ, 13);
    pb_emit_wait(&b, 4);
    pb_label(&b, "chase2_1.c2_1_7");
    pb_emit_add(&b, PAL_ROTX, -6);
    pb_emit_loop(&b, 18, "chase2_1.c2_1_7");
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_emit_wait(&b, 5);
    pb_label(&b, "chase2_1.c2_1_8");
    pb_emit_add(&b, PAL_ROTZ, -8);
    pb_emit_loop(&b, 15, "chase2_1.c2_1_8");
    pb_emit_trigger(&b, "save", -1);
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase2_1.c2_1_a");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase2_1.c2_1_8a");
    pb_emit_message_meter(&b, 15);
    pb_emit_goto(&b, P_GOTO, "chase2_1.c2_1_8c");
    pb_label(&b, "chase2_1.c2_1_8a");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase2_1.c2_1_8b");
    pb_emit_message_meter(&b, 35);
    pb_emit_goto(&b, P_GOTO, "chase2_1.c2_1_8c");
    pb_label(&b, "chase2_1.c2_1_8b");
    pb_emit_message_meter(&b, 55);
    pb_label(&b, "chase2_1.c2_1_8c");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_accel(&b, 50, 2);
    pb_emit_waitchaseb(&b, PAL_ROTX, 192);
    pb_emit_setvel(&b, 0);
    pb_emit_goto(&b, P_GOTO, "chase2_1.c2_1_z");
    pb_label(&b, "chase2_1.c2_1_ta");
    pb_emit_trigger(&b, "save", -1);
    pb_emit_trigger(&b, "yamete", -1);
    pb_emit8(&b, P_FLAGSHAPE);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_setvel(&b, 10);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase2_1.c2_1_tm5");
    pb_emit_message_meter(&b, 9);
    pb_emit_goto(&b, P_GOTO, "chase2_1.c2_1_tj");
    pb_label(&b, "chase2_1.c2_1_tm5");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase2_1.c2_1_tm6");
    pb_emit_message_meter(&b, 29);
    pb_emit_goto(&b, P_GOTO, "chase2_1.c2_1_tj");
    pb_label(&b, "chase2_1.c2_1_tm6");
    pb_emit_message_meter(&b, 49);
    pb_label(&b, "chase2_1.c2_1_tj");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_waitchaseb(&b, PAL_ROTY, (uint8)-64);
    pb_emit_waitchaseb(&b, PAL_ROTX, (uint8)-32);
    pb_emit_setvel(&b, 30);
    pb_emit_do(&b, 10);
    pb_emit_add(&b, PAL_ROTZ, 8);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "chase2_1.c2_1_a");
    pb_emit_trigger(&b, "yamete", -1);
    pb_emit8(&b, P_FLAGSHAPE);
    pb_emit_accel(&b, 0, 1);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_wait(&b, 4);
    pb_emit8(&b, P_SMOKEOFF);
    pb_emit_add(&b, PAL_ROTZ, 16);
    pb_emit_wait(&b, 5);
    pb_emit_add(&b, PAL_ROTZ, -32);
    pb_emit_wait(&b, 5);
    pb_emit_add(&b, PAL_ROTZ, 16);
    pb_emit_wait(&b, 5);
    pb_label(&b, "chase2_1.c2_1_g");
    pb_emit_chaseb(&b, PAL_ROTY, 128);
    pb_emit_chaseb(&b, PAL_ROTX, 128);
    pb_emit_loop(&b, 15, "chase2_1.c2_1_g");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase2_1.c2_1_m5");
    pb_emit_message_meter(&b, 9);
    pb_emit_goto(&b, P_GOTO, "chase2_1.c2_1_j");
    pb_label(&b, "chase2_1.c2_1_m5");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase2_1.c2_1_m6");
    pb_emit_message_meter(&b, 29);
    pb_emit_goto(&b, P_GOTO, "chase2_1.c2_1_j");
    pb_label(&b, "chase2_1.c2_1_m6");
    pb_emit_message_meter(&b, 49);
    pb_emit_wait(&b, 2);
    pb_label(&b, "chase2_1.c2_1_j");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_waitchaseb(&b, PAL_VEL, (uint8)-16);
    pb_label(&b, "chase2_1.c2_1_h");
    pb_emit_add(&b, PAL_ROTZ, 10);
    pb_emit_add(&b, PAL_WORLDY, 4);
    pb_emit_add(&b, PAL_WORLDX, -6);
    pb_emit_loop(&b, 37, "chase2_1.c2_1_h");
    pb_emit8(&b, P_SMOKEON);
    pb_label(&b, "chase2_1.c2_1_i");
    pb_emit_add(&b, PAL_ROTZ, 10);
    pb_emit_add(&b, PAL_WORLDY, 4);
    pb_emit_add(&b, PAL_WORLDX, -6);
    pb_emit_loop(&b, 18, "chase2_1.c2_1_i");
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "chase2_1.c2_1_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:6001
    pb_start_path(&b, PATH_ID_CHASE2_2, "chase2_2");
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "chase_init");
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_label(&b, "chase2_2.c2_2_02");
    pb_emit_wait(&b, 12);
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase2_2.c2_2_z");
    pb_label(&b, "chase2_2.c2_2_0");
    pb_emit_chaseb(&b, PAL_ROTY, 200);
    pb_emit_chaseb(&b, PAL_ROTZ, 224);
    pb_emit_loop(&b, 10, "chase2_2.c2_2_0");
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase2_2.c2_2_a");
    pb_emit_chaseb(&b, PAL_ROTY, 200);
    pb_emit_chaseb(&b, PAL_ROTZ, 224);
    pb_emit_loop(&b, 9, "chase2_2.c2_2_a");
    pb_emit_setvel(&b, 30);
    pb_emit8(&b, P_COLLISIONSON);
    pb_emit_wait(&b, 40);
    pb_label(&b, "chase2_2.c2_2_1");
    pb_emit_chaseb(&b, PAL_ROTY, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 20, "chase2_2.c2_2_1");
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase2_2.c2_2_2");
    pb_emit_add(&b, PAL_ROTZ, 1);
    pb_emit_add(&b, PAL_ROTX, -2);
    pb_emit_loop(&b, 15, "chase2_2.c2_2_2");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 1);
    pb_label(&b, "chase2_2.c2_2_3");
    pb_emit_add(&b, PAL_ROTZ, -1);
    pb_emit_add(&b, PAL_ROTX, 2);
    pb_emit_loop(&b, 15, "chase2_2.c2_2_3");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 1);
    pb_emit_accel(&b, 40, 1);
    pb_label(&b, "chase2_2.c2_2_4");
    pb_emit_add(&b, PAL_ROTZ, 1);
    pb_emit_add(&b, PAL_ROTX, 2);
    pb_emit_loop(&b, 24, "chase2_2.c2_2_4");
    pb_emit_wait(&b, 3);
    pb_emit_accel(&b, 30, 1);
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "chase2_2.c2_2_lea");
    pb_label(&b, "chase2_2.c2_2_5");
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_chaseb(&b, PAL_ROTX, 245);
    pb_emit_loop(&b, 20, "chase2_2.c2_2_5");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 4);
    pb_emit_add(&b, PAL_ROTZ, -13);
    pb_emit_wait(&b, 21);
    pb_emit_add(&b, PAL_ROTZ, 13);
    pb_emit_wait(&b, 4);
    pb_label(&b, "chase2_2.c2_2_6");
    pb_emit_add(&b, PAL_ROTX, -6);
    pb_emit_loop(&b, 18, "chase2_2.c2_2_6");
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_emit_wait(&b, 8);
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase2_2.c2_2_9");
    pb_emit_add(&b, PAL_ROTZ, -8);
    pb_emit_loop(&b, 15, "chase2_2.c2_2_9");
    pb_emit_accel(&b, 50, 2);
    pb_emit_wait(&b, 5);
    pb_emit_waitchaseb(&b, PAL_ROTX, 192);
    pb_emit_setvel(&b, 0);
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "chase2_2.c2_2_y");
    pb_emit_goto(&b, P_GOTO, "chase2_2.c2_2_z");
    pb_label(&b, "chase2_2.c2_2_y");
    pb_emit_zero(&b, PAL_ROTZ);
    pb_emit_set(&b, PAL_ROTX, 64);
    pb_emit_set(&b, PAL_ROTY, 128);
    pb_emit_zero(&b, PAL_WORLDX);
    pb_emit_set(&b, PAL_WORLDY, -1100);
    pb_emit_setvel(&b, 50);
    pb_label(&b, "chase2_2.c2_2_w");
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -450, 200, "chase2_2.c2_2_x");
    pb_emit_goto(&b, P_GOTO, "chase2_2.c2_2_w");
    pb_label(&b, "chase2_2.c2_2_x");
    pb_emit_setb(&b, PAL_HP, 10);
    pb_emit_accel(&b, 0, 1);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_emit_setvel(&b, 0);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 28);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_waitchaseb(&b, PAL_ROTX, 250);
    pb_emit_accel(&b, 70, 2);
    pb_emit8(&b, P_SMOKEOFF);
    pb_emit_wait(&b, 8);
    pb_emit_waitchaseb(&b, PAL_ROTX, 192);
    pb_emit_wait(&b, 20);
    pb_label(&b, "chase2_2.c2_2_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);
    pb_label(&b, "chase2_2.c2_2_lea");
    pb_emit_wait(&b, 30);
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit_wait(&b, 10);
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:6163
    pb_start_path(&b, PATH_ID_CHASE3_1, "chase3_1");
    pb_emit_friend(&b, FRIEND_ANYONE);
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "friend_init");
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_soundeffect(&b, 1);
    pb_emit_notfriend(&b, FRIEND_ANYONE, "chase3_1.c3_1_z");
    pb_label(&b, "chase3_1.c3_1_0");
    pb_emit_add(&b, PAL_ROTX, -32);
    pb_emit_setvel(&b, 30);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase3_1.c3_1_m1");
    pb_emit_message_meter(&b, 7);
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_1");
    pb_label(&b, "chase3_1.c3_1_m1");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase3_1.c3_1_m2");
    pb_emit_message_meter(&b, 27);
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_1");
    pb_label(&b, "chase3_1.c3_1_m2");
    pb_emit_message_meter(&b, 47);
    pb_label(&b, "chase3_1.c3_1_1");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chasew(&b, PAL_WORLDX, 0);
    pb_emit_chasew(&b, PAL_WORLDY, -60);
    pb_emit_loop(&b, 20, "chase3_1.c3_1_1");
    pb_label(&b, "chase3_1.c3_1_11");
    pb_emit_wait(&b, 20);
    pb_label(&b, "chase3_1.c3_1_s");
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "chase3_1.c3_1_a");
    pb_label(&b, "chase3_1.c3_1_b");
    pb_emit8(&b, P_RIGHTOFPLAYER);
    pb_fixup16(&b, "chase3_1.c3_1_br");
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_bl");
    pb_label(&b, "chase3_1.c3_1_a");
    pb_emit8(&b, P_RIGHTOFPLAYER);
    pb_fixup16(&b, "chase3_1.c3_1_ar");
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_al");
    pb_label(&b, "chase3_1.c3_1_ar");
    pb_emit_pushb(&b, PAL_PBYTE1);
    pb_emit_setb(&b, PAL_PBYTE1, 1);
    pb_emit_exportb(&b, PAL_PBYTE1, PATH_EXT_EBYTE2);
    pb_emit_pullb(&b, PAL_PBYTE1);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 242);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_label(&b, "chase3_1.c3_1_ar1");
    pb_emit_add(&b, PAL_ROTX, -4);
    pb_emit_loop(&b, 31, "chase3_1.c3_1_ar1");
    pb_emit_set(&b, PAL_ROTX, 128);
    pb_label(&b, "chase3_1.c3_1_ar2");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -100, 300, "chase3_1.c3_1_ar3");
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_ar2");
    pb_label(&b, "chase3_1.c3_1_ar3");
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase3_1.c3_1_td");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "chase3_1.c3_tyame");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 14);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_label(&b, "chase3_1.c3_1_ar33");
    pb_emit_wait(&b, 10);
    pb_label(&b, "chase3_1.c3_1_ar4");
    pb_emit_add(&b, PAL_ROTX, -4);
    pb_emit_loop(&b, 31, "chase3_1.c3_1_ar4");
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_e");
    pb_label(&b, "chase3_1.c3_1_al");
    pb_emit_pushb(&b, PAL_PBYTE1);
    pb_emit_setb(&b, PAL_PBYTE1, 2);
    pb_emit_exportb(&b, PAL_PBYTE1, PATH_EXT_EBYTE2);
    pb_emit_pullb(&b, PAL_PBYTE1);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 14);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_label(&b, "chase3_1.c3_1_al1");
    pb_emit_add(&b, PAL_ROTX, -4);
    pb_emit_loop(&b, 31, "chase3_1.c3_1_al1");
    pb_emit_set(&b, PAL_ROTX, 128);
    pb_label(&b, "chase3_1.c3_1_al2");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -300, 100, "chase3_1.c3_1_al3");
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_al2");
    pb_label(&b, "chase3_1.c3_1_al3");
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase3_1.c3_1_td");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "chase3_1.c3_tyame");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 242);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_label(&b, "chase3_1.c3_1_al33");
    pb_emit_wait(&b, 10);
    pb_label(&b, "chase3_1.c3_1_al4");
    pb_emit_add(&b, PAL_ROTX, -4);
    pb_emit_loop(&b, 31, "chase3_1.c3_1_al4");
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_e");
    pb_label(&b, "chase3_1.c3_1_br");
    pb_emit_pushb(&b, PAL_PBYTE1);
    pb_emit_setb(&b, PAL_PBYTE1, 3);
    pb_emit_exportb(&b, PAL_PBYTE1, PATH_EXT_EBYTE2);
    pb_emit_pullb(&b, PAL_PBYTE1);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 242);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_label(&b, "chase3_1.c3_1_br1");
    pb_emit_add(&b, PAL_ROTX, 4);
    pb_emit_chaseb(&b, PAL_ROTZ, 128);
    pb_emit_loop(&b, 31, "chase3_1.c3_1_br1");
    pb_label(&b, "chase3_1.c3_1_br2");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -100, 200, "chase3_1.c3_1_br3");
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_br2");
    pb_label(&b, "chase3_1.c3_1_br3");
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase3_1.c3_1_td");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "chase3_1.c3_tyame");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_waitchaseb(&b, PAL_ROTY, 0);
    pb_label(&b, "chase3_1.c3_1_br33");
    pb_emit_wait(&b, 9);
    pb_label(&b, "chase3_1.c3_1_br4");
    pb_emit_add(&b, PAL_ROTX, 4);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 31, "chase3_1.c3_1_br4");
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_waitchasew(&b, PAL_WORLDX, 0);
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_f");
    pb_label(&b, "chase3_1.c3_1_bl");
    pb_emit_pushb(&b, PAL_PBYTE1);
    pb_emit_setb(&b, PAL_PBYTE1, 4);
    pb_emit_exportb(&b, PAL_PBYTE1, PATH_EXT_EBYTE2);
    pb_emit_pullb(&b, PAL_PBYTE1);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 14);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_label(&b, "chase3_1.c3_1_bl1");
    pb_emit_add(&b, PAL_ROTX, 4);
    pb_emit_chaseb(&b, PAL_ROTZ, 128);
    pb_emit_loop(&b, 31, "chase3_1.c3_1_bl1");
    pb_label(&b, "chase3_1.c3_1_bl2");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -200, 100, "chase3_1.c3_1_bl3");
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_bl2");
    pb_label(&b, "chase3_1.c3_1_bl3");
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase3_1.c3_1_td");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "chase3_1.c3_tyame");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_waitchaseb(&b, PAL_ROTY, 0);
    pb_label(&b, "chase3_1.c3_1_bl33");
    pb_emit_wait(&b, 9);
    pb_label(&b, "chase3_1.c3_1_bl4");
    pb_emit_add(&b, PAL_ROTX, 4);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 31, "chase3_1.c3_1_bl4");
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_waitchasew(&b, PAL_WORLDX, 0);
    pb_emit_waitchasew(&b, PAL_WORLDY, -60);
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_f");
    pb_label(&b, "chase3_1.c3_1_e");
    pb_emit_waitchasew(&b, PAL_WORLDX, 0);
    pb_emit_waitchasew(&b, PAL_WORLDY, -60);
    pb_label(&b, "chase3_1.c3_1_ee");
    pb_emit_wait(&b, 10);
    pb_label(&b, "chase3_1.c3_1_f");
    pb_emit_wait(&b, 10);
    pb_emit_trigger(&b, "save", -1);
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase3_1.c3_1_4");
    pb_emit_wait(&b, 10);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase3_1.c3_1_2a");
    pb_emit_message_meter(&b, 15);
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_2c");
    pb_label(&b, "chase3_1.c3_1_2a");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase3_1.c3_1_2b");
    pb_emit_message_meter(&b, 35);
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_2c");
    pb_label(&b, "chase3_1.c3_1_2b");
    pb_emit_message_meter(&b, 55);
    pb_label(&b, "chase3_1.c3_1_2c");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_y");
    pb_label(&b, "chase3_1.c3_1_4");
    pb_emit8(&b, P_FLAGSHAPE);
    pb_emit_trigger(&b, "yamete", -1);
    pb_emit_wait(&b, 7);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_wait(&b, 4);
    pb_emit8(&b, P_SMOKEOFF);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase3_1.c3_1_m5");
    pb_emit_message_meter(&b, 9);
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_8");
    pb_label(&b, "chase3_1.c3_1_m5");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase3_1.c3_1_m6");
    pb_emit_message_meter(&b, 29);
    pb_emit_goto(&b, P_GOTO, "chase3_1.c3_1_8");
    pb_label(&b, "chase3_1.c3_1_m6");
    pb_emit_message_meter(&b, 49);
    pb_label(&b, "chase3_1.c3_1_8");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_add(&b, PAL_ROTZ, 10);
    pb_emit_chaseb(&b, PAL_VEL, (uint8)-14);
    pb_emit_add(&b, PAL_WORLDY, 1);
    pb_emit_loop(&b, 50, "chase3_1.c3_1_8");
    pb_emit8(&b, P_SMOKEON);
    pb_label(&b, "chase3_1.c3_1_9");
    pb_emit_add(&b, PAL_ROTZ, 10);
    pb_emit_add(&b, PAL_WORLDY, 2);
    pb_emit_loop(&b, 30, "chase3_1.c3_1_9");
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "chase3_1.c3_1_y");
    pb_emit_accel(&b, 70, 1);
    pb_emit_waitchaseb(&b, PAL_ROTX, 192);
    pb_label(&b, "chase3_1.c3_1_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);
    pb_label(&b, "chase3_1.c3_tyame");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase3_1.c3_m_t1");
    pb_emit_message_meter(&b, 69);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase3_1.c3_m_t1");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase3_1.c3_m_t2");
    pb_emit_message_meter(&b, 70);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase3_1.c3_m_t2");
    pb_emit_message_meter(&b, 71);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase3_1.c3_1_td");
    pb_emit8(&b, P_FLAGSHAPE);
    pb_emit_trigger(&b, "save", -1);
    pb_emit_trigger(&b, "yamete", -1);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_emit_setvel(&b, 0);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase3_1.c3_1_tm5");
    pb_emit_message_meter(&b, 9);
    pb_emit_goto(&b, P_IGOTO, "chase3_1.c3_1_td1");
    pb_label(&b, "chase3_1.c3_1_tm5");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase3_1.c3_1_tm6");
    pb_emit_message_meter(&b, 29);
    pb_emit_goto(&b, P_IGOTO, "chase3_1.c3_1_td1");
    pb_label(&b, "chase3_1.c3_1_tm6");
    pb_emit_message_meter(&b, 49);
    pb_label(&b, "chase3_1.c3_1_td1");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_add(&b, PAL_WORLDZ, -20);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTY, 0);
    pb_emit_add(&b, PAL_ROTZ, 8);
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "chase3_1.c3_d");
    pb_emit_add(&b, PAL_WORLDY, -10);
    pb_emit_goto(&b, P_IGOTO, "chase3_1.c3_x");
    pb_label(&b, "chase3_1.c3_d");
    pb_emit_add(&b, PAL_WORLDY, 10);
    pb_label(&b, "chase3_1.c3_x");
    pb_emit8(&b, P_LEFTOFPLAYER);
    pb_fixup16(&b, "chase3_1.c3_r");
    pb_emit_add(&b, PAL_WORLDX, -10);
    pb_emit_goto(&b, P_IGOTO, "chase3_1.c3_lo");
    pb_label(&b, "chase3_1.c3_r");
    pb_emit_add(&b, PAL_WORLDX, 10);
    pb_label(&b, "chase3_1.c3_lo");
    pb_emit_loop(&b, 60, "chase3_1.c3_1_td1");
    pb_emit8(&b, P_EXPLODE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:6479
    pb_start_path(&b, PATH_ID_CHASE3_2, "chase3_2");
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_zero(&b, PAL_PBYTE2);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "chase_init");
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_label(&b, "chase3_2.c3_2_0");
    pb_emit_add(&b, PAL_ROTX, -32);
    pb_emit_wait(&b, 12);
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase3_2.c3_2_z");
    pb_emit_setvel(&b, 30);
    pb_emit8(&b, P_COLLISIONSON);
    pb_label(&b, "chase3_2.c3_2_1");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chasew(&b, PAL_WORLDX, 0);
    pb_emit_chasew(&b, PAL_WORLDY, -60);
    pb_emit_loop(&b, 20, "chase3_2.c3_2_1");
    pb_emit_wait(&b, 20);
    pb_label(&b, "chase3_2.c3_2_s");
    pb_emit_importb(&b, PAL_PBYTE1, PATH_EXT_EBYTE2);
    pb_emit_ifsameb(&b, PAL_PBYTE1, 1, "chase3_2.c3_2_ar");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 2, "chase3_2.c3_2_al");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 3, "chase3_2.c3_2_br");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 4, "chase3_2.c3_2_bl");
    pb_emit8(&b, P_ABOVEPLAYER);
    pb_fixup16(&b, "chase3_2.c3_2_a");
    pb_label(&b, "chase3_2.c3_2_b");
    pb_emit8(&b, P_RIGHTOFPLAYER);
    pb_fixup16(&b, "chase3_2.c3_2_br");
    pb_emit_goto(&b, P_GOTO, "chase3_2.c3_2_bl");
    pb_label(&b, "chase3_2.c3_2_a");
    pb_emit8(&b, P_RIGHTOFPLAYER);
    pb_fixup16(&b, "chase3_2.c3_2_ar");
    pb_emit_goto(&b, P_GOTO, "chase3_2.c3_2_al");
    pb_label(&b, "chase3_2.c3_2_ar");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 10);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 242);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_label(&b, "chase3_2.c3_2_ar0");
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase3_2.c3_2_ar1");
    pb_emit_add(&b, PAL_ROTX, -4);
    pb_emit_add(&b, PAL_PBYTE2, 1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_PBYTE2, 20, "chase3_2.c3_2_ar2g");
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase3_2.c3_2_ar2g");
    pb_emit_loop(&b, 31, "chase3_2.c3_2_ar1");
    pb_emit_set(&b, PAL_ROTX, 128);
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "chase3_2.c3_2_td");
    pb_label(&b, "chase3_2.c3_2_ar2");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -100, 300, "chase3_2.c3_2_ar3");
    pb_emit_goto(&b, P_GOTO, "chase3_2.c3_2_ar2");
    pb_label(&b, "chase3_2.c3_2_ar3");
    pb_emit_waitchaseb(&b, PAL_ROTZ, 14);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_wait(&b, 10);
    pb_label(&b, "chase3_2.c3_2_ar4");
    pb_emit_add(&b, PAL_ROTX, -4);
    pb_emit_loop(&b, 31, "chase3_2.c3_2_ar4");
    pb_emit_goto(&b, P_GOTO, "chase3_2.c3_2_e");
    pb_label(&b, "chase3_2.c3_2_al");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 10);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 14);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_label(&b, "chase3_2.c3_2_al0");
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase3_2.c3_2_al1");
    pb_emit_add(&b, PAL_ROTX, -4);
    pb_emit_add(&b, PAL_PBYTE2, 1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_PBYTE2, 20, "chase3_2.c3_2_al2g");
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase3_2.c3_2_al2g");
    pb_emit_loop(&b, 31, "chase3_2.c3_2_al1");
    pb_emit_set(&b, PAL_ROTX, 128);
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "chase3_2.c3_2_td");
    pb_label(&b, "chase3_2.c3_2_al2");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -300, 100, "chase3_2.c3_2_al3");
    pb_emit_goto(&b, P_GOTO, "chase3_2.c3_2_al2");
    pb_label(&b, "chase3_2.c3_2_al3");
    pb_emit_waitchaseb(&b, PAL_ROTZ, 242);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_wait(&b, 10);
    pb_label(&b, "chase3_2.c3_2_al4");
    pb_emit_add(&b, PAL_ROTX, -4);
    pb_emit_loop(&b, 31, "chase3_2.c3_2_al4");
    pb_emit_goto(&b, P_GOTO, "chase3_2.c3_2_e");
    pb_label(&b, "chase3_2.c3_2_br");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 9);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 242);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_label(&b, "chase3_2.c3_2_br0");
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase3_2.c3_2_br1");
    pb_emit_add(&b, PAL_ROTX, 4);
    pb_emit_chaseb(&b, PAL_ROTZ, 128);
    pb_emit_add(&b, PAL_PBYTE2, 1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_PBYTE2, 25, "chase3_2.c3_2_br2g");
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase3_2.c3_2_br2g");
    pb_emit_loop(&b, 31, "chase3_2.c3_2_br1");
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "chase3_2.c3_2_td");
    pb_label(&b, "chase3_2.c3_2_br2");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -100, 200, "chase3_2.c3_2_br3");
    pb_emit_goto(&b, P_GOTO, "chase3_2.c3_2_br2");
    pb_label(&b, "chase3_2.c3_2_br3");
    pb_emit_waitchaseb(&b, PAL_ROTY, 0);
    pb_emit_wait(&b, 10);
    pb_label(&b, "chase3_2.c3_2_br4");
    pb_emit_add(&b, PAL_ROTX, 4);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 31, "chase3_2.c3_2_br4");
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_goto(&b, P_GOTO, "chase3_2.c3_2_e");
    pb_label(&b, "chase3_2.c3_2_bl");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 10);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 14);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_label(&b, "chase3_2.c3_2_bl0");
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_label(&b, "chase3_2.c3_2_bl1");
    pb_emit_add(&b, PAL_ROTX, 4);
    pb_emit_chaseb(&b, PAL_ROTZ, 128);
    pb_emit_add(&b, PAL_PBYTE2, 1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_PBYTE2, 25, "chase3_2.c3_2_bl2g");
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase3_2.c3_2_bl2g");
    pb_emit_loop(&b, 31, "chase3_2.c3_2_bl1");
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "chase3_2.c3_2_td");
    pb_label(&b, "chase3_2.c3_2_bl2");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -200, 100, "chase3_2.c3_2_bl3");
    pb_emit_goto(&b, P_GOTO, "chase3_2.c3_2_bl2");
    pb_label(&b, "chase3_2.c3_2_bl3");
    pb_emit_waitchaseb(&b, PAL_ROTY, 0);
    pb_emit_wait(&b, 10);
    pb_label(&b, "chase3_2.c3_2_bl4");
    pb_emit_add(&b, PAL_ROTX, 4);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 31, "chase3_2.c3_2_bl4");
    pb_emit8(&b, P_SPACESHIPON);
    pb_label(&b, "chase3_2.c3_2_e");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 25);
    pb_emit_accel(&b, 70, 1);
    pb_emit_waitchaseb(&b, PAL_ROTX, 192);
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "chase3_2.c3_2_td");
    pb_emit_wait(&b, 10);
    pb_label(&b, "chase3_2.c3_2_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);
    pb_label(&b, "chase3_2.c3_2_td");
    pb_emit_gotopos(&b, 0, 0, 600, 40);
    pb_emit8(&b, P_WAITFACEPLAYER);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_wait(&b, 4);
    pb_emit8(&b, P_SMOKEOFF);
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:3232
    pb_start_path(&b, PATH_ID_E_SHIELDR, "e_shieldr");
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELSLOWELASER);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_trigger(&b, "e_shieldr.e_sh_disck", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit_trigger(&b, "e_shieldr.e_sh_ref", PATH_TRIGGER_WHENHITBYPLAYER_VALUE);
    pb_emit_setb(&b, PAL_HP, 2);
    pb_emit_setvel(&b, -60);
    pb_emit_wait(&b, 30);
    pb_label(&b, "e_shieldr.e_sh_0");
    pb_emit_setvel(&b, 0);
    pb_emit_waitchaseb(&b, PAL_ROTY, 128);
    pb_emit8(&b, P_INVINCIBLEOFF);
    pb_emit_trigger(&b, "e_shieldr.e_sh_ref", -1);
    pb_emit_wait(&b, 5);
    pb_emit_do(&b, 10);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 5);
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit_trigger(&b, "e_shieldr.e_sh_ref", PATH_TRIGGER_WHENHITBYPLAYER_VALUE);
    pb_label(&b, "e_shieldr.e_sh_01");
    pb_emit_chaseb(&b, PAL_ROTY, 0);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_ROTY, 0, "e_shieldr.e_sh_01");
    pb_emit8(&b, P_IFNOT);
    pb_emit_ifsameb(&b, PAL_ROTX, 0, "e_shieldr.e_sh_01");
    pb_emit_setvel(&b, -40);
    pb_emit_goto(&b, P_RANDOMGOTO, "e_shieldr.e_sh_r");
    pb_emit_setb(&b, PAL_PBYTE1, -3);
    pb_emit_goto(&b, P_RANDOMGOTO, "e_shieldr.e_sh_ld");
    pb_emit_setb(&b, PAL_PBYTE2, -3);
    pb_emit_goto(&b, P_IGOTO, "e_shieldr.e_sh_mv");
    pb_label(&b, "e_shieldr.e_sh_ld");
    pb_emit_setb(&b, PAL_PBYTE2, 3);
    pb_emit_goto(&b, P_IGOTO, "e_shieldr.e_sh_mv");
    pb_label(&b, "e_shieldr.e_sh_r");
    pb_emit_setb(&b, PAL_PBYTE1, 3);
    pb_emit_goto(&b, P_RANDOMGOTO, "e_shieldr.e_sh_rd");
    pb_emit_setb(&b, PAL_PBYTE2, -3);
    pb_emit_goto(&b, P_IGOTO, "e_shieldr.e_sh_mv");
    pb_label(&b, "e_shieldr.e_sh_rd");
    pb_emit_setb(&b, PAL_PBYTE2, 3);
    pb_label(&b, "e_shieldr.e_sh_mv");
    pb_emit_do(&b, 32);
    pb_emit_addvwb(&b, PAL_WORLDX, PAL_PBYTE1);
    pb_emit_addvwb(&b, PAL_WORLDY, PAL_PBYTE2);
    pb_emit_ifsameb(&b, PAL_PBYTE1, 3, "e_shieldr.e_sh_mvr");
    pb_emit_add(&b, PAL_ROTZ, -2);
    pb_emit_goto(&b, P_IGOTO, "e_shieldr.e_sh_mvn");
    pb_label(&b, "e_shieldr.e_sh_mvr");
    pb_emit_add(&b, PAL_ROTZ, 2);
    pb_label(&b, "e_shieldr.e_sh_mvn");
    pb_emit8(&b, P_NEXT);
    pb_emit_goto(&b, P_IGOTO, "e_shieldr.e_sh_0");
    pb_label(&b, "e_shieldr.e_sh_z");
    pb_emit_setvel(&b, 0);
    pb_emit_trigger(&b, "e_shieldr.e_sh_disck", -1);
    pb_emit_do(&b, 10);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_INVINCIBLEOFF);
    pb_emit_trigger(&b, "e_shieldr.e_sh_ref", -1);
    pb_emit8(&b, P_WAITFACEPLAYER);
    pb_emit_wait(&b, 6);
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit_trigger(&b, "e_shieldr.e_sh_ref", PATH_TRIGGER_WHENHITBYPLAYER_VALUE);
    pb_label(&b, "e_shieldr.e_sh_z1");
    pb_emit_add(&b, PAL_ROTX, 16);
    pb_emit_loop(&b, 7, "e_shieldr.e_sh_z1");
    pb_label(&b, "e_shieldr.e_sh_z2");
    pb_emit_chaseb(&b, PAL_VEL, (uint8)-40);
    pb_emit_add(&b, PAL_ROTZ, 4);
    pb_emit_loop(&b, 100, "e_shieldr.e_sh_z2");
    pb_emit8(&b, P_END);
    pb_label(&b, "e_shieldr.e_sh_disck");
    pb_emit_distless(&b, 700, "e_shieldr.e_sh_disck1");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "e_shieldr.e_sh_disck1");
    pb_emit_goto(&b, P_FORCE, "e_shieldr.e_sh_z");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "e_shieldr.e_sh_ref");
    pb_emit_pushb(&b, PAL_PBYTE1);
    pb_emit_pushb(&b, PAL_PBYTE2);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_REBELASER);
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE1, 31);
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE2, 31);
    pb_emit_goto(&b, P_RANDOMGOTO, "e_shieldr.e_sh_neg2");
    pb_emit_negb(&b, PAL_ABS_PBYTE1);
    pb_label(&b, "e_shieldr.e_sh_neg2");
    pb_emit_goto(&b, P_RANDOMGOTO, "e_shieldr.e_sh_shot");
    pb_emit_negb(&b, PAL_ABS_PBYTE2);
    pb_label(&b, "e_shieldr.e_sh_shot");
    pb_emit_addv(&b, false, PAL_ROTX, false, PAL_PBYTE1);
    pb_emit_addv(&b, false, PAL_ROTY, false, PAL_PBYTE2);
    pb_emit_add(&b, PAL_ROTY, 128);
    pb_emit8(&b, P_FIRE);
    pb_emit_add(&b, PAL_ROTY, 128);
    pb_emit_negb(&b, PAL_ABS_PBYTE1);
    pb_emit_negb(&b, PAL_ABS_PBYTE2);
    pb_emit_addv(&b, false, PAL_ROTX, false, PAL_PBYTE1);
    pb_emit_addv(&b, false, PAL_ROTY, false, PAL_PBYTE2);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELSLOWELASER);
    pb_emit_pullb(&b, PAL_PBYTE2);
    pb_emit_pullb(&b, PAL_PBYTE1);
    pb_emit8(&b, P_RETURN);

    // PATHDATA.ASM:7339
    pb_start_path(&b, PATH_ID_CHASE6_1, "chase6_1");
    pb_emit_friend(&b, FRIEND_ANYONE);
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "friend_init");
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_notfriend(&b, FRIEND_ANYONE, "chase6_1.c6_1_z");
    pb_label(&b, "chase6_1.c6_1_0");
    pb_emit_waitchaseb(&b, PAL_ROTX, 35);
    pb_emit_waitchaseb(&b, PAL_ROTY, 215);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase6_1.c6_1_m1");
    pb_emit_message_meter(&b, 7);
    pb_emit_goto(&b, P_GOTO, "chase6_1.c6_1_6");
    pb_label(&b, "chase6_1.c6_1_m1");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase6_1.c6_1_m2");
    pb_emit_message_meter(&b, 27);
    pb_emit_goto(&b, P_GOTO, "chase6_1.c6_1_6");
    pb_label(&b, "chase6_1.c6_1_m2");
    pb_emit_message_meter(&b, 47);
    pb_label(&b, "chase6_1.c6_1_6");
    pb_emit_soundeffect(&b, 1);
    pb_emit_setvel(&b, 40);
    pb_emit_wait(&b, 3);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_label(&b, "chase6_1.c6_1_61");
    pb_emit_wait(&b, 14);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 20);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_label(&b, "chase6_1.c6_1_62");
    pb_emit_wait(&b, 15);
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase6_1.c6_td");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "chase6_1.c6_tyame");
    pb_emit8(&b, P_DAMAGE);
    pb_label(&b, "chase6_1.c6_1_1");
    pb_emit_add(&b, PAL_ROTX, -2);
    pb_emit_add(&b, PAL_ROTZ, 3);
    pb_emit_loop(&b, 15, "chase6_1.c6_1_1");
    pb_label(&b, "chase6_1.c6_1_2");
    pb_emit_add(&b, PAL_ROTX, 2);
    pb_emit_add(&b, PAL_ROTZ, -3);
    pb_emit_loop(&b, 15, "chase6_1.c6_1_2");
    pb_label(&b, "chase6_1.c6_1_21");
    pb_emit_wait(&b, 22);
    pb_label(&b, "chase6_1.c6_1_3");
    pb_emit_add(&b, PAL_ROTX, 2);
    pb_emit_add(&b, PAL_ROTZ, -4);
    pb_emit_loop(&b, 7, "chase6_1.c6_1_3");
    pb_emit_wait(&b, 7);
    pb_label(&b, "chase6_1.c6_1_4");
    pb_emit_add(&b, PAL_ROTX, -2);
    pb_emit_add(&b, PAL_ROTZ, 4);
    pb_emit_loop(&b, 7, "chase6_1.c6_1_4");
    pb_label(&b, "chase6_1.c6_1_41");
    pb_emit_wait(&b, 25);
    pb_emit_trigger(&b, "save", -1);
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase6_1.c6_1_8");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase6_1.c6_1_4a");
    pb_emit_message_meter(&b, 15);
    pb_emit_goto(&b, P_GOTO, "chase6_1.c6_1_4c");
    pb_label(&b, "chase6_1.c6_1_4a");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase6_1.c6_1_4b");
    pb_emit_message_meter(&b, 35);
    pb_emit_goto(&b, P_GOTO, "chase6_1.c6_1_4c");
    pb_label(&b, "chase6_1.c6_1_4b");
    pb_emit_message_meter(&b, 55);
    pb_label(&b, "chase6_1.c6_1_4c");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_wait(&b, 5);
    pb_label(&b, "chase6_1.c6_1_5");
    pb_emit_add(&b, PAL_ROTZ, 2);
    pb_emit_loop(&b, 8, "chase6_1.c6_1_5");
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_accel(&b, 70, 2);
    pb_emit_waitchaseb(&b, PAL_ROTX, (uint8)-8);
    pb_label(&b, "chase6_1.c6_1_51");
    pb_emit_wait(&b, 30);
    pb_emit_goto(&b, P_GOTO, "chase6_1.c6_1_z");
    pb_label(&b, "chase6_1.c6_1_8");
    pb_emit_trigger(&b, "yamete", -1);
    pb_emit_wait(&b, 8);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase6_1.c6_1_m3");
    pb_emit_message_meter(&b, 9);
    pb_emit_goto(&b, P_GOTO, "chase6_1.c6_1_e");
    pb_label(&b, "chase6_1.c6_1_m3");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase6_1.c6_1_m4");
    pb_emit_message_meter(&b, 29);
    pb_emit_goto(&b, P_GOTO, "chase6_1.c6_1_e");
    pb_label(&b, "chase6_1.c6_1_m4");
    pb_emit_message_meter(&b, 49);
    pb_label(&b, "chase6_1.c6_1_e");
    pb_emit_accel(&b, 0, 1);
    pb_emit8(&b, P_DAMAGE);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_wait(&b, 4);
    pb_emit8(&b, P_SMOKEOFF);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_label(&b, "chase6_1.c6_1_9");
    pb_emit_add(&b, PAL_WORLDY, 8);
    pb_emit_add(&b, PAL_ROTY, -6);
    pb_emit_add(&b, PAL_ROTZ, -10);
    pb_emit_hitground(&b, 0, "chase6_1.c6_1_a");
    pb_emit_goto(&b, P_GOTO, "chase6_1.c6_1_9");
    pb_label(&b, "chase6_1.c6_1_a");
    pb_emit8(&b, P_SMOKEON);
    pb_label(&b, "chase6_1.c6_1_b");
    pb_emit_chaseb(&b, PAL_ROTY, 0);
    pb_emit_add(&b, PAL_ROTZ, -10);
    pb_emit_add(&b, PAL_WORLDY, -4);
    pb_emit_loop(&b, 20, "chase6_1.c6_1_b");
    pb_emit_waitchaseb(&b, PAL_VEL, (uint8)-18);
    pb_label(&b, "chase6_1.c6_1_c");
    pb_emit_add(&b, PAL_ROTZ, -10);
    pb_emit_add(&b, PAL_WORLDY, 4);
    pb_emit_hitground(&b, 0, "chase6_1.c6_1_d");
    pb_emit_goto(&b, P_GOTO, "chase6_1.c6_1_c");
    pb_label(&b, "chase6_1.c6_1_d");
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "chase6_1.c6_1_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);
    pb_label(&b, "chase6_1.c6_tyame");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase6_1.c6_m_t1");
    pb_emit_message_meter(&b, 69);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase6_1.c6_m_t1");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase6_1.c6_m_t2");
    pb_emit_message_meter(&b, 70);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase6_1.c6_m_t2");
    pb_emit_message_meter(&b, 71);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase6_1.c6_td");
    pb_emit_trigger(&b, "save", -1);
    pb_emit_trigger(&b, "yamete", -1);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase6_1.c6_1_tm5");
    pb_emit_message_meter(&b, 9);
    pb_emit_goto(&b, P_IGOTO, "chase6_1.c6_1_td1");
    pb_label(&b, "chase6_1.c6_1_tm5");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase6_1.c6_1_tm6");
    pb_emit_message_meter(&b, 29);
    pb_emit_goto(&b, P_IGOTO, "chase6_1.c6_1_td1");
    pb_label(&b, "chase6_1.c6_1_tm6");
    pb_emit_message_meter(&b, 49);
    pb_label(&b, "chase6_1.c6_1_td1");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_add(&b, PAL_ROTX, -2);
    pb_emit_add(&b, PAL_ROTZ, 3);
    pb_emit_add(&b, PAL_WORLDY, 8);
    pb_emit_loop(&b, 15, "chase6_1.c6_1_td1");
    pb_label(&b, "chase6_1.c6_1_td2");
    pb_emit_add(&b, PAL_ROTX, 2);
    pb_emit_add(&b, PAL_ROTZ, -3);
    pb_emit_add(&b, PAL_WORLDY, 8);
    pb_emit_loop(&b, 15, "chase6_1.c6_1_td2");
    pb_emit8(&b, P_EXPLODE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:7554
    pb_start_path(&b, PATH_ID_CHASE6_2, "chase6_2");
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "chase_init");
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_label(&b, "chase6_2.c6_2_0");
    pb_emit_wait(&b, 3);
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase6_2.c6_2_z");
    pb_emit_waitchaseb(&b, PAL_ROTX, 35);
    pb_emit_waitchaseb(&b, PAL_ROTY, 215);
    pb_emit_wait(&b, 12);
    pb_emit_setvel(&b, 40);
    pb_emit8(&b, P_COLLISIONSON);
    pb_emit_wait(&b, 3);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 14);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 20);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 5);
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 10);
    pb_label(&b, "chase6_2.c6_2_1");
    pb_emit_add(&b, PAL_ROTX, -2);
    pb_emit_add(&b, PAL_ROTZ, 3);
    pb_emit_loop(&b, 15, "chase6_2.c6_2_1");
    pb_label(&b, "chase6_2.c6_2_2");
    pb_emit_add(&b, PAL_ROTX, 2);
    pb_emit_add(&b, PAL_ROTZ, -3);
    pb_emit_loop(&b, 15, "chase6_2.c6_2_2");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 6);
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase6_2.c6_2_td");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 16);
    pb_label(&b, "chase6_2.c6_2_3");
    pb_emit_add(&b, PAL_ROTX, 2);
    pb_emit_add(&b, PAL_ROTZ, -4);
    pb_emit_loop(&b, 7, "chase6_2.c6_2_3");
    pb_emit_wait(&b, 7);
    pb_label(&b, "chase6_2.c6_2_4");
    pb_emit_add(&b, PAL_ROTX, -2);
    pb_emit_add(&b, PAL_ROTZ, 4);
    pb_emit_loop(&b, 7, "chase6_2.c6_2_4");
    pb_emit8(&b, P_FACESHAPE);
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 25);
    pb_label(&b, "chase6_2.c6_2_5");
    pb_emit_add(&b, PAL_ROTZ, 2);
    pb_emit_add(&b, PAL_ROTX, -1);
    pb_emit_loop(&b, 8, "chase6_2.c6_2_5");
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_accel(&b, 70, 2);
    pb_emit_waitchaseb(&b, PAL_ROTX, (uint8)-32);
    pb_label(&b, "chase6_2.c6_2_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);
    pb_label(&b, "chase6_2.c6_2_td");
    pb_emit_gotopos(&b, 0, -80, 800, 40);
    pb_emit8(&b, P_WAITFACEPLAYER);
    pb_emit8(&b, P_FIRE);
    pb_emit_accel(&b, 40, 2);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_emit_wait(&b, 60);
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:7666
    pb_start_path(&b, PATH_ID_CHASE7_1, "chase7_1");
    pb_emit_friend(&b, FRIEND_ANYONE);
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "friend_init");
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_notfriend(&b, FRIEND_ANYONE, "chase7_1.c7_1_zz");
    pb_label(&b, "chase7_1.c7_1_0");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase7_1.c7_1_m1");
    pb_emit_message_meter(&b, 7);
    pb_emit_goto(&b, P_IGOTO, "chase7_1.c7_1_1");
    pb_label(&b, "chase7_1.c7_1_m1");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase7_1.c7_1_m2");
    pb_emit_message_meter(&b, 27);
    pb_emit_goto(&b, P_IGOTO, "chase7_1.c7_1_1");
    pb_label(&b, "chase7_1.c7_1_m2");
    pb_emit_message_meter(&b, 47);
    pb_label(&b, "chase7_1.c7_1_1");
    pb_emit_soundeffect(&b, 1);
    pb_emit_setvel(&b, 50);
    pb_emit_wait(&b, 4);
    pb_emit_setb(&b, PAL_ROTX, 32);
    pb_emit_wait(&b, 2);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_label(&b, "chase7_1.c7_1_11");
    pb_emit_wait(&b, 15);
    pb_emit8(&b, P_ZREMOVEON);
    pb_label(&b, "chase7_1.c7_1_2");
    pb_emit_add(&b, PAL_ROTX, -8);
    pb_emit_loop(&b, 7, "chase7_1.c7_1_2");
    pb_emit_wait(&b, 1);
    pb_emit_accel(&b, 10, 1);
    pb_emit_waitchaseb(&b, PAL_ROTY, 32);
    pb_emit_waitchaseb(&b, PAL_ROTX, 128);
    pb_emit_wait(&b, 5);
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase7_1.c7_td");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "chase7_1.c7_tyame");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_accel(&b, 50, 2);
    pb_label(&b, "chase7_1.c7_1_3");
    pb_emit_chaseb(&b, PAL_ROTX, 64);
    pb_emit_chaseb(&b, PAL_ROTY, 112);
    pb_emit_loop(&b, 15, "chase7_1.c7_1_3");
    pb_label(&b, "chase7_1.c7_1_4");
    pb_emit_wait(&b, 1);
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -420, 0, "chase7_1.c7_1_5");
    pb_emit_goto(&b, P_GOTO, "chase7_1.c7_1_4");
    pb_label(&b, "chase7_1.c7_1_5");
    pb_emit_add(&b, PAL_ROTX, -8);
    pb_emit_loop(&b, 11, "chase7_1.c7_1_5");
    pb_emit_wait(&b, 5);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_accel(&b, 40, 1);
    pb_label(&b, "chase7_1.c7_1_6");
    pb_emit_chaseb(&b, PAL_ROTZ, 32);
    pb_emit_chaseb(&b, PAL_ROTX, 5);
    pb_emit_loop(&b, 20, "chase7_1.c7_1_6");
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_label(&b, "chase7_1.c7_1_61");
    pb_emit_wait(&b, 16);
    pb_emit_trigger(&b, "save", -1);
    pb_emit8(&b, P_ALMOSTDEAD);
    pb_fixup16(&b, "chase7_1.c7_1_9");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase7_1.c7_1_6a");
    pb_emit_message_meter(&b, 15);
    pb_emit_goto(&b, P_GOTO, "chase7_1.c7_1_6c");
    pb_label(&b, "chase7_1.c7_1_6a");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase7_1.c7_1_6b");
    pb_emit_message_meter(&b, 35);
    pb_emit_goto(&b, P_GOTO, "chase7_1.c7_1_6c");
    pb_label(&b, "chase7_1.c7_1_6b");
    pb_emit_message_meter(&b, 55);
    pb_label(&b, "chase7_1.c7_1_6c");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 240);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_label(&b, "chase7_1.c7_1_6d");
    pb_emit_wait(&b, 30);
    pb_emit_goto(&b, P_GOTO, "chase7_1.c7_1_z");
    pb_label(&b, "chase7_1.c7_1_9");
    pb_emit_trigger(&b, "yamete", -1);
    pb_emit_wait(&b, 8);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase7_1.c7_1_m5");
    pb_emit_message_meter(&b, 9);
    pb_emit_goto(&b, P_GOTO, "chase7_1.c7_1_h");
    pb_label(&b, "chase7_1.c7_1_m5");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase7_1.c7_1_m6");
    pb_emit_message_meter(&b, 29);
    pb_emit_goto(&b, P_GOTO, "chase7_1.c7_1_h");
    pb_label(&b, "chase7_1.c7_1_m6");
    pb_emit_message_meter(&b, 49);
    pb_label(&b, "chase7_1.c7_1_h");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_setvel(&b, -19);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_wait(&b, 4);
    pb_emit8(&b, P_SMOKEOFF);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_label(&b, "chase7_1.c7_1_i");
    pb_emit_add(&b, PAL_WORLDY, 8);
    pb_emit_chaseb(&b, PAL_ROTY, 0);
    pb_emit_add(&b, PAL_ROTZ, -10);
    pb_emit_hitground(&b, 0, "chase7_1.c7_1_j");
    pb_emit_goto(&b, P_GOTO, "chase7_1.c7_1_i");
    pb_label(&b, "chase7_1.c7_1_j");
    pb_emit8(&b, P_SMOKEON);
    pb_label(&b, "chase7_1.c7_1_k");
    pb_emit_chaseb(&b, PAL_ROTY, 0);
    pb_emit_add(&b, PAL_ROTZ, -10);
    pb_emit_add(&b, PAL_WORLDY, -4);
    pb_emit_loop(&b, 20, "chase7_1.c7_1_k");
    pb_label(&b, "chase7_1.c7_1_l");
    pb_emit_add(&b, PAL_ROTZ, -10);
    pb_emit_add(&b, PAL_WORLDY, 4);
    pb_emit_hitground(&b, 0, "chase7_1.c7_1_m");
    pb_emit_goto(&b, P_GOTO, "chase7_1.c7_1_l");
    pb_label(&b, "chase7_1.c7_1_m");
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "chase7_1.c7_1_z");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit_wait(&b, 20);
    pb_label(&b, "chase7_1.c7_1_zz");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);
    pb_label(&b, "chase7_1.c7_tyame");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase7_1.c7_m_t1");
    pb_emit_message_meter(&b, 69);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase7_1.c7_m_t1");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase7_1.c7_m_t2");
    pb_emit_message_meter(&b, 70);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase7_1.c7_m_t2");
    pb_emit_message_meter(&b, 71);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase7_1.c7_td");
    pb_emit_trigger(&b, "save", -1);
    pb_emit_trigger(&b, "yamete", -1);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase7_1.c7_1_tm5");
    pb_emit_message_meter(&b, 9);
    pb_emit_goto(&b, P_IGOTO, "chase7_1.c7_1_td1");
    pb_label(&b, "chase7_1.c7_1_tm5");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase7_1.c7_1_tm6");
    pb_emit_message_meter(&b, 29);
    pb_emit_goto(&b, P_IGOTO, "chase7_1.c7_1_td1");
    pb_label(&b, "chase7_1.c7_1_tm6");
    pb_emit_message_meter(&b, 49);
    pb_label(&b, "chase7_1.c7_1_td1");
    pb_emit8(&b, P_DAMAGE);
    pb_emit_accel(&b, 50, 2);
    pb_label(&b, "chase7_1.c7_1_td3");
    pb_emit_chaseb(&b, PAL_ROTX, 64);
    pb_emit_chaseb(&b, PAL_ROTY, 112);
    pb_emit_loop(&b, 15, "chase7_1.c7_1_td3");
    pb_label(&b, "chase7_1.c7_1_td4");
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -420, 0, "chase7_1.c7_td5");
    pb_emit_goto(&b, P_GOTO, "chase7_1.c7_1_td4");
    pb_label(&b, "chase7_1.c7_td5");
    pb_emit_wait(&b, 4);
    pb_emit8(&b, P_EXPLODE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:7891
    pb_start_path(&b, PATH_ID_CHASE7_2, "chase7_2");
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "chase_init");
    pb_label(&b, "chase7_2.c7_2_0");
    pb_emit_wait(&b, 3);
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase7_2.c7_2_z");
    pb_emit_wait(&b, 10);
    pb_emit_setvel(&b, 50);
    pb_emit8(&b, P_COLLISIONSON);
    pb_emit_wait(&b, 4);
    pb_emit_setb(&b, PAL_ROTX, 32);
    pb_emit_wait(&b, 2);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_wait(&b, 15);
    pb_label(&b, "chase7_2.c7_2_1");
    pb_emit_add(&b, PAL_ROTX, -8);
    pb_emit_loop(&b, 7, "chase7_2.c7_2_1");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 1);
    pb_emit_accel(&b, 10, 1);
    pb_emit_waitchaseb(&b, PAL_ROTY, 32);
    pb_emit_waitchaseb(&b, PAL_ROTX, 128);
    pb_emit_wait(&b, 5);
    pb_emit_accel(&b, 50, 2);
    pb_label(&b, "chase7_2.c7_2_2");
    pb_emit_chaseb(&b, PAL_ROTX, 64);
    pb_emit_chaseb(&b, PAL_ROTY, 112);
    pb_emit_loop(&b, 15, "chase7_2.c7_2_2");
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 4);
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase7_2.c7_2_4");
    pb_emit_wait(&b, 1);
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -420, 0, "chase7_2.c7_2_3");
    pb_emit_goto(&b, P_GOTO, "chase7_2.c7_2_4");
    pb_label(&b, "chase7_2.c7_2_3");
    pb_emit_add(&b, PAL_ROTX, -8);
    pb_emit_loop(&b, 11, "chase7_2.c7_2_3");
    pb_emit_wait(&b, 5);
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase7_2.c7_2_td");
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_accel(&b, 40, 1);
    pb_label(&b, "chase7_2.c7_2_5");
    pb_emit_chaseb(&b, PAL_ROTZ, 32);
    pb_emit_chaseb(&b, PAL_ROTX, 5);
    pb_emit_loop(&b, 20, "chase7_2.c7_2_5");
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_set(&b, PAL_HP, 100);
    pb_emit8(&b, P_FIRE);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_emit8(&b, P_FIRE);
    pb_emit_wait(&b, 15);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 240);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_wait(&b, 30);
    pb_label(&b, "chase7_2.c7_2_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);
    pb_label(&b, "chase7_2.c7_2_td");
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_gotopos(&b, 0, -80, 700, 40);
    pb_emit8(&b, P_WAITFACEPLAYER);
    pb_emit8(&b, P_FIRE);
    pb_emit_accel(&b, 40, 2);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_emit_wait(&b, 60);
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:6693
    pb_start_path(&b, PATH_ID_CHASE4_1, "chase4_1");
    pb_emit_friend(&b, FRIEND_ANYONE);
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "friend_inib");
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_notfriend(&b, FRIEND_ANYONE, "chase4_1.c4_1_z");
    pb_label(&b, "chase4_1.c4_1_0");
    pb_emit_chaseb(&b, PAL_ROTY, 56);
    pb_emit_chaseb(&b, PAL_ROTZ, 224);
    pb_emit_loop(&b, 20, "chase4_1.c4_1_0");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase4_1.c4_1_a1");
    pb_emit_message(&b, 13);
    pb_emit_goto(&b, P_IGOTO, "chase4_1.c4_1_a3");
    pb_label(&b, "chase4_1.c4_1_a1");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase4_1.c4_1_a2");
    pb_emit_message(&b, 33);
    pb_emit_goto(&b, P_IGOTO, "chase4_1.c4_1_a3");
    pb_label(&b, "chase4_1.c4_1_a2");
    pb_emit_message(&b, 50);
    pb_label(&b, "chase4_1.c4_1_a3");
    pb_emit_wait(&b, 20);
    pb_emit_soundeffect(&b, 1);
    pb_emit_setvel(&b, 40);
    pb_label(&b, "chase4_1.c4_1_aa");
    pb_emit_wait(&b, 23);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase4_1.c4_1_a");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_fal_ms");
    pb_emit_goto(&b, P_GOTO, "chase4_1.c4_1_1");
    pb_label(&b, "chase4_1.c4_1_a");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase4_1.c4_1_b");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_rab_ms");
    pb_emit_goto(&b, P_GOTO, "chase4_1.c4_1_1");
    pb_label(&b, "chase4_1.c4_1_b");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_fro_ms");
    pb_label(&b, "chase4_1.c4_1_1");
    pb_emit_chaseb(&b, PAL_ROTY, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 18, "chase4_1.c4_1_1");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_SPACESHIPON);
    pb_label(&b, "chase4_1.c4_1_2");
    pb_emit_chaseb(&b, PAL_ROTZ, 229);
    pb_emit_chaseb(&b, PAL_ROTX, 32);
    pb_emit_loop(&b, 10, "chase4_1.c4_1_2");
    pb_emit8(&b, P_FIRECANHIT);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_waitchaseb(&b, PAL_ROTX, 0);
    pb_label(&b, "chase4_1.c4_1_3");
    pb_emit_add(&b, PAL_ROTX, -4);
    pb_emit_loop(&b, 32, "chase4_1.c4_1_3");
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase4_1.c4_1_6");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase4_1.c4_1_m1");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_fal_ms");
    pb_emit_goto(&b, P_IGOTO, "chase4_1.c4_1_4");
    pb_label(&b, "chase4_1.c4_1_m1");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase4_1.c4_1_m3");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_rab_ms");
    pb_emit_goto(&b, P_IGOTO, "chase4_1.c4_1_4");
    pb_label(&b, "chase4_1.c4_1_m3");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_fro_ms");
    pb_emit_goto(&b, P_IGOTO, "chase4_1.c4_1_4");
    pb_label(&b, "chase4_1.c4_1_6");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase4_1.c4_1_m5");
    pb_emit_message(&b, 22);
    pb_emit_goto(&b, P_GOTO, "chase4_1.c4_1_8");
    pb_label(&b, "chase4_1.c4_1_m5");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase4_1.c4_1_m7");
    pb_emit_message(&b, 42);
    pb_emit_goto(&b, P_GOTO, "chase4_1.c4_1_8");
    pb_label(&b, "chase4_1.c4_1_m7");
    pb_emit_message(&b, 61);
    pb_label(&b, "chase4_1.c4_1_8");
    pb_emit_wait(&b, 12);
    pb_emit_goto(&b, P_GOTO, "chase4_1.c4_1_7");
    pb_label(&b, "chase4_1.c4_1_4");
    pb_emit8(&b, P_FIRECANHIT);
    pb_emit_wait(&b, 8);
    pb_label(&b, "chase4_1.c4_1_7");
    pb_emit_waitchaseb(&b, PAL_ROTZ, 16);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_accel(&b, 30, 1);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_label(&b, "chase4_1.c4_1_5");
    pb_emit_add(&b, PAL_ROTX, -1);
    pb_emit_add(&b, PAL_ROTZ, -8);
    pb_emit_loop(&b, 31, "chase4_1.c4_1_5");
    pb_label(&b, "chase4_1.c4_1_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:6883
    pb_start_path(&b, PATH_ID_CHASE4_2, "chase4_2");
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELSLOWELASER);
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 3);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_setb(&b, PAL_HP, 6);
    pb_label(&b, "chase4_2.c4_2_0");
    pb_emit_chaseb(&b, PAL_ROTY, 56);
    pb_emit_chaseb(&b, PAL_ROTZ, 224);
    pb_emit_loop(&b, 20, "chase4_2.c4_2_0");
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase4_2.c4_2_z");
    pb_emit_setvel(&b, 40);
    pb_emit_wait(&b, 23);
    pb_label(&b, "chase4_2.c4_2_1");
    pb_emit_chaseb(&b, PAL_ROTY, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 18, "chase4_2.c4_2_1");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_wait(&b, 1);
    pb_emit_accel(&b, 0, 2);
    pb_emit_waitchaseb(&b, PAL_ROTX, 224);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 28);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_setvel(&b, 0);
    pb_label(&b, "chase4_2.c4_2_4");
    pb_emit_chaseb(&b, PAL_ROTX, 19);
    pb_emit_chaseb(&b, PAL_ROTY, 138);
    pb_emit_loop(&b, 10, "chase4_2.c4_2_4");
    pb_emit8(&b, P_FIRE);
    pb_emit_add(&b, PAL_WORLDY, 1);
    pb_emit_wait(&b, 1);
    pb_emit_add(&b, PAL_WORLDY, 1);
    pb_emit_wait(&b, 1);
    pb_emit_do(&b, 4);
    pb_emit_add(&b, PAL_WORLDY, 3);
    pb_emit8(&b, P_NEXT);
    pb_emit_wait(&b, 1);
    pb_emit_add(&b, PAL_WORLDY, 1);
    pb_emit_wait(&b, 1);
    pb_emit_waitchaseb(&b, PAL_ROTY, 140);
    pb_emit8(&b, P_FIRE);
    pb_emit_add(&b, PAL_WORLDY, 1);
    pb_emit_wait(&b, 5);
    pb_emit_do(&b, 3);
    pb_emit_add(&b, PAL_WORLDY, -1);
    pb_emit8(&b, P_NEXT);
    pb_emit_wait(&b, 1);
    pb_emit_waitchaseb(&b, PAL_ROTY, 136);
    pb_emit8(&b, P_FIRE);
    pb_emit_do(&b, 4);
    pb_emit_add(&b, PAL_WORLDY, -1);
    pb_emit8(&b, P_NEXT);
    pb_emit_wait(&b, 7);
    pb_emit_setvel(&b, 10);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_wait(&b, 4);
    pb_emit8(&b, P_SMOKEOFF);
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_label(&b, "chase4_2.c4_2_y");
    pb_emit_add(&b, PAL_ROTZ, 10);
    pb_emit_add(&b, PAL_ROTX, 10);
    pb_emit_add(&b, PAL_WORLDX, 10);
    pb_emit_add(&b, PAL_WORLDY, 5);
    pb_emit_loop(&b, 15, "chase4_2.c4_2_y");
    pb_emit8(&b, P_SMOKEON);
    pb_label(&b, "chase4_2.c4_2_w");
    pb_emit_add(&b, PAL_ROTZ, 10);
    pb_emit_add(&b, PAL_ROTX, 10);
    pb_emit_add(&b, PAL_WORLDX, 10);
    pb_emit_add(&b, PAL_WORLDY, 5);
    pb_emit_loop(&b, 15, "chase4_2.c4_2_w");
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "chase4_2.c4_2_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:6992
    pb_start_path(&b, PATH_ID_CHASE4_3, "chase4_3");
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 3);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELSLOWELASER);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_findshape(&b, SH_ZACO_B);
    pb_emit8(&b, P_IMMUNE);
    pb_emit_findshape(&b, SH_FRIENDSHIP_4);
    pb_emit_wait(&b, 3);
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase4_3.c4_3_z");
    pb_label(&b, "chase4_3.c4_3_0");
    pb_emit_chaseb(&b, PAL_ROTY, 56);
    pb_emit_chaseb(&b, PAL_ROTZ, 224);
    pb_emit_loop(&b, 20, "chase4_3.c4_3_0");
    pb_emit_setvel(&b, 40);
    pb_emit_wait(&b, 23);
    pb_label(&b, "chase4_3.c4_3_1");
    pb_emit_chaseb(&b, PAL_ROTY, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 18, "chase4_3.c4_3_1");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_SPACESHIPON);
    pb_label(&b, "chase4_3.c4_3_2");
    pb_emit_chaseb(&b, PAL_ROTZ, 229);
    pb_emit_chaseb(&b, PAL_ROTX, 32);
    pb_emit_loop(&b, 10, "chase4_3.c4_3_2");
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_wait(&b, 4);
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "chase4_3.c4_3_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // Shared chase4_1 message sublabels used by chase8_1 (PATHDATA.ASM:6805).
    pb_label(&b, "pchase4_1.c4_fal_ms");
    pb_emit_goto(&b, P_RANDOMGOTO, "pchase4_1.c4_fal_1");
    pb_emit_goto(&b, P_RANDOMGOTO, "pchase4_1.c4_fal_2");
    pb_label(&b, "pchase4_1.c4_fal_4");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 23, "pchase4_1.c4_fal_2");
    pb_emit_message(&b, 23);
    pb_emit_setb(&b, PAL_PBYTE1, 23);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pchase4_1.c4_fal_2");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 12, "pchase4_1.c4_fal_1");
    pb_emit_message(&b, 12);
    pb_emit_setb(&b, PAL_PBYTE1, 12);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pchase4_1.c4_fal_1");
    pb_emit_goto(&b, P_RANDOMGOTO, "pchase4_1.c4_fal_3");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 11, "pchase4_1.c4_fal_3");
    pb_emit_message(&b, 11);
    pb_emit_setb(&b, PAL_PBYTE1, 11);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pchase4_1.c4_fal_3");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 10, "pchase4_1.c4_fal_4");
    pb_emit_message(&b, 10);
    pb_emit_setb(&b, PAL_PBYTE1, 10);
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "pchase4_1.c4_rab_ms");
    pb_emit_goto(&b, P_RANDOMGOTO, "pchase4_1.c4_rab_1");
    pb_emit_goto(&b, P_RANDOMGOTO, "pchase4_1.c4_rab_2");
    pb_label(&b, "pchase4_1.c4_rab_4");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 43, "pchase4_1.c4_rab_2");
    pb_emit_message(&b, 43);
    pb_emit_setb(&b, PAL_PBYTE1, 43);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pchase4_1.c4_rab_2");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 32, "pchase4_1.c4_rab_1");
    pb_emit_message(&b, 32);
    pb_emit_setb(&b, PAL_PBYTE1, 32);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pchase4_1.c4_rab_1");
    pb_emit_goto(&b, P_RANDOMGOTO, "pchase4_1.c4_rab_3");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 31, "pchase4_1.c4_rab_3");
    pb_emit_message(&b, 31);
    pb_emit_setb(&b, PAL_PBYTE1, 31);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pchase4_1.c4_rab_3");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 30, "pchase4_1.c4_rab_4");
    pb_emit_message(&b, 30);
    pb_emit_setb(&b, PAL_PBYTE1, 30);
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "pchase4_1.c4_fro_ms");
    pb_emit_goto(&b, P_RANDOMGOTO, "pchase4_1.c4_fro_1");
    pb_emit_goto(&b, P_RANDOMGOTO, "pchase4_1.c4_fro_2");
    pb_label(&b, "pchase4_1.c4_fro_4");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 62, "pchase4_1.c4_fro_2");
    pb_emit_message(&b, 62);
    pb_emit_setb(&b, PAL_PBYTE1, 62);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pchase4_1.c4_fro_2");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 53, "pchase4_1.c4_fro_1");
    pb_emit_message(&b, 53);
    pb_emit_setb(&b, PAL_PBYTE1, 53);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pchase4_1.c4_fro_1");
    pb_emit_goto(&b, P_RANDOMGOTO, "pchase4_1.c4_fro_3");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 52, "pchase4_1.c4_fro_3");
    pb_emit_message(&b, 52);
    pb_emit_setb(&b, PAL_PBYTE1, 52);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pchase4_1.c4_fro_3");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 51, "pchase4_1.c4_fro_4");
    pb_emit_message(&b, 51);
    pb_emit_setb(&b, PAL_PBYTE1, 51);
    pb_emit8(&b, P_RETURN);

    // PATHDATA.ASM:789
    pb_start_path(&b, PATH_ID_KORORI, "korori");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit_trigger(&b, "korori.koro_jump", PATH_TRIGGER_WHENHITBYPLAYER_VALUE);
    pb_emit_trigger(&b, "korori.koro_exp", PATH_TRIGGER_WHENDEAD_VALUE);
    pb_emit_setb(&b, PAL_HP, 6);
    pb_emit_setb(&b, PAL_AP, 6);
    pb_emit_zero(&b, PAL_ROTZ);
    pb_emit_zero(&b, PAL_ROTY);
    pb_emit_zero(&b, PAL_ROTX);
    pb_emit_zero(&b, PAL_PBYTE2);
    pb_emit_goto(&b, P_IGOTO, "korori.koro_star");
    pb_label(&b, "korori.koro_0");
    pb_emit_wait(&b, 1);
    pb_emit_goto(&b, P_GOTO, "korori.koro_0");
    pb_label(&b, "korori.koro_jump");
    pb_emit_goto(&b, P_FORCE, "korori.koro_1");
    pb_emit_setb(&b, PAL_PBYTE2, 15);
    pb_emit_goto(&b, P_RANDOMGOTO, "korori.koro_r");
    pb_emit_setb(&b, PAL_PBYTE2, -15);
    pb_label(&b, "korori.koro_r");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "korori.koro_1");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit_setb(&b, PAL_PBYTE1, -47);
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 5);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "korori.koro_up");
    pb_emit_wait(&b, 1);
    pb_emit_add(&b, PAL_PBYTE1, 4);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "korori.koro_up");
    pb_emit_add(&b, PAL_PBYTE1, 6);
    pb_emit8(&b, P_NEXT);
    pb_emit_wait(&b, 2);
    pb_label(&b, "korori.koro_star");
    pb_emit_setb(&b, PAL_PBYTE1, 3);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "korori.koro_up");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 4);
    pb_emit_add(&b, PAL_PBYTE1, 4);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "korori.koro_up");
    pb_emit_hitground(&b, 0, "korori.koro_bre");
    pb_emit_wait(&b, 1);
    pb_emit_add(&b, PAL_PBYTE1, 6);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "korori.koro_up");
    pb_emit_hitground(&b, 0, "korori.koro_bre");
    pb_emit_goto(&b, P_IGOTO, "korori.koro_lp1");
    pb_label(&b, "korori.koro_bre");
    pb_emit_break(&b, "korori.koro_stnd");
    pb_label(&b, "korori.koro_lp1");
    pb_emit8(&b, P_NEXT);
    pb_emit_wait(&b, 1);
    pb_emit_setb(&b, PAL_PBYTE1, 47);
    pb_label(&b, "korori.koro_w");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "korori.koro_up");
    pb_emit_hitground(&b, 0, "korori.koro_stnd");
    pb_emit_goto(&b, P_GOTO, "korori.koro_w");
    pb_label(&b, "korori.koro_up");
    pb_emit_addvwb(&b, PAL_WORLDY, PAL_PBYTE1);
    pb_emit_addvwb(&b, PAL_WORLDX, PAL_PBYTE2);
    pb_emit_add(&b, PAL_WORLDZ, 5);
    pb_emit_add(&b, PAL_ROTY, 32);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "korori.koro_stnd");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit_zero(&b, PAL_ROTY);
    pb_emit_zero(&b, PAL_WORLDY);
    pb_emit_goto(&b, P_GOTO, "korori.koro_0");
    pb_label(&b, "korori.koro_exp");
    pb_emit_add(&b, PAL_WORLDY, -50);
    pb_emit8(&b, P_RETURN);

    // PATHDATA.ASM:8010
    pb_start_path(&b, PATH_ID_CHASE8_1, "chase8_1");
    pb_emit_friend(&b, FRIEND_ANYONE);
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "friend_inib");
    pb_emit_trigger(&b, "chase8_1.c8_toru", PATH_TRIGGER_WHENSHAPEDEAD_VALUE);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_zero(&b, PAL_PBYTE1);
    pb_emit_notfriend(&b, FRIEND_ANYONE, "chase8_1.c8_1_z");
    pb_label(&b, "chase8_1.c8_1_0");
    pb_emit_wait(&b, 15);
    pb_emit_waitchaseb(&b, PAL_ROTY, 42);
    pb_emit_soundeffect(&b, 1);
    pb_emit_setvel(&b, 40);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase8_1.c8_1_a");
    pb_emit_message(&b, 13);
    pb_emit_goto(&b, P_IGOTO, "chase8_1.c8_1_01");
    pb_label(&b, "chase8_1.c8_1_a");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase8_1.c8_1_b");
    pb_emit_message(&b, 33);
    pb_emit_goto(&b, P_IGOTO, "chase8_1.c8_1_01");
    pb_label(&b, "chase8_1.c8_1_b");
    pb_emit_message(&b, 50);
    pb_label(&b, "chase8_1.c8_1_01");
    pb_emit_wait(&b, 25);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 231);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 28);
    pb_emit_ifsameb(&b, PAL_PBYTE1, 1, "chase8_1.c8_1_1");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase8_1.c8_1_m1");
    pb_emit_add(&b, PAL_ROTZ, -5);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_fal_ms");
    pb_emit_goto(&b, P_IGOTO, "chase8_1.c8_1_1");
    pb_label(&b, "chase8_1.c8_1_m1");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase8_1.c8_1_m2");
    pb_emit_add(&b, PAL_ROTZ, -5);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_rab_ms");
    pb_emit_goto(&b, P_IGOTO, "chase8_1.c8_1_1");
    pb_label(&b, "chase8_1.c8_1_m2");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_fro_ms");
    pb_label(&b, "chase8_1.c8_1_1");
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_ifsameb(&b, PAL_PBYTE1, 1, "chase8_1.c8_1_11");
    pb_emit8(&b, P_FIRECANHIT);
    pb_label(&b, "chase8_1.c8_1_11");
    pb_emit_trigger(&b, "chase8_1.c8_toru", -1);
    pb_emit_zero(&b, PAL_PBYTE1);
    pb_emit_wait(&b, 12);
    pb_label(&b, "chase8_1.c8_1_2");
    pb_emit_chaseb(&b, PAL_ROTX, 240);
    pb_emit_chaseb(&b, PAL_ROTZ, 33);
    pb_emit_loop(&b, 18, "chase8_1.c8_1_2");
    pb_emit_findshape(&b, SH_ZACO_A);
    pb_emit_trigger(&b, "chase8_1.c8_toru", PATH_TRIGGER_WHENSHAPEDEAD_VALUE);
    pb_label(&b, "chase8_1.c8_1_3");
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_loop(&b, 18, "chase8_1.c8_1_3");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 1, "chase8_1.c8_1_43");
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase8_1.c8_1_m3");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_fal_ms");
    pb_emit_goto(&b, P_IGOTO, "chase8_1.c8_1_5");
    pb_label(&b, "chase8_1.c8_1_m3");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase8_1.c8_1_m4");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_rab_ms");
    pb_emit_goto(&b, P_IGOTO, "chase8_1.c8_1_5");
    pb_label(&b, "chase8_1.c8_1_m4");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "pchase4_1.c4_fro_ms");
    pb_label(&b, "chase8_1.c8_1_5");
    pb_emit8(&b, P_FIRECANHIT);
    pb_emit_setb(&b, PAL_PBYTE1, 1);
    pb_label(&b, "chase8_1.c8_1_43");
    pb_emit_accel(&b, 50, 1);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 234);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_label(&b, "chase8_1.c8_1_44");
    pb_emit_wait(&b, 10);
    pb_label(&b, "chase8_1.c8_1_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);
    pb_label(&b, "chase8_1.c8_toru");
    pb_emit_ifsameb(&b, PAL_PBYTE1, 0, "chase8_1.c8_toru1");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase8_1.c8_toru1");
    pb_emit_setb(&b, PAL_PBYTE1, 1);
    pb_emit_notfriend(&b, FRIEND_FALCON, "chase8_1.c8_toru2");
    pb_emit_message(&b, 22);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase8_1.c8_toru2");
    pb_emit_notfriend(&b, FRIEND_RABBIT, "chase8_1.c8_toru3");
    pb_emit_message(&b, 42);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "chase8_1.c8_toru3");
    pb_emit_message(&b, 61);
    pb_emit8(&b, P_RETURN);

    // PATHDATA.ASM:8138
    pb_start_path(&b, PATH_ID_CHASE8_2, "chase8_2");
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELSLOWELASER);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit_setb(&b, PAL_HP, 4);
    pb_emit_wait(&b, 3);
    pb_emit_waitchaseb(&b, PAL_ROTY, 64);
    pb_emit_waitchaseb(&b, PAL_ROTX, 32);
    pb_emit_setvel(&b, 70);
    pb_label(&b, "chase8_2.c8_2_0");
    pb_emit_ifbetweenw(&b, PAL_WORLDY, -900, -350, "chase8_2.c8_2_1");
    pb_emit_wait(&b, 1);
    pb_emit_goto(&b, P_GOTO, "chase8_2.c8_2_0");
    pb_label(&b, "chase8_2.c8_2_1");
    pb_emit_chaseb(&b, PAL_ROTZ, 32);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_chaseb(&b, PAL_ROTY, 128);
    pb_emit_loop(&b, 31, "chase8_2.c8_2_1");
    pb_emit_accel(&b, 0, 2);
    pb_label(&b, "chase8_2.c8_2_2");
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit_add(&b, PAL_WORLDY, 2);
    pb_emit_loop(&b, 37, "chase8_2.c8_2_2");
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase8_2.c8_2_3");
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit_loop(&b, 2, "chase8_2.c8_2_3");
    pb_label(&b, "chase8_2.c8_2_5");
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit_loop(&b, 5, "chase8_2.c8_2_5");
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "chase8_2.c8_2_4");
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit_loop(&b, 7, "chase8_2.c8_2_4");
    pb_emit8(&b, P_FIRE);
    pb_emit_findshape(&b, SH_FRIENDSHIP_4);
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase8_2.c8_2_z1");
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "chase8_2.c8_2_z1");
    pb_emit8(&b, P_RELTOPLAYEROFF);
    pb_emit_wait(&b, 20);
    pb_label(&b, "chase8_2.c8_2_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // PATHDATA.ASM:8202
    pb_start_path(&b, PATH_ID_CHASE8_3, "chase8_3");
    pb_emit8(&b, P_LINK);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELSLOWELASER);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_setb(&b, PAL_HP, 4);
    pb_emit_wait(&b, 3);
    pb_emit8(&b, P_SHAPEDEAD);
    pb_fixup16(&b, "chase8_3.c8_3_z");
    pb_emit_waitchaseb(&b, PAL_ROTY, 42);
    pb_emit_setvel(&b, 40);
    pb_emit_wait(&b, 25);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 231);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 28);
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_wait(&b, 15);
    pb_emit8(&b, P_SMOKEON);
    pb_emit_wait(&b, 5);
    pb_emit8(&b, P_SMOKEOFF);
    pb_emit8(&b, P_SPACESHIPOFF);
    pb_label(&b, "chase8_3.c8_3_x");
    pb_emit_add(&b, PAL_ROTZ, 20);
    pb_emit_add(&b, PAL_WORLDY, 10);
    pb_emit_hitground(&b, 0, "chase8_3.c8_3_y");
    pb_emit_goto(&b, P_GOTO, "chase8_3.c8_3_x");
    pb_label(&b, "chase8_3.c8_3_y");
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "chase8_3.c8_3_z");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // DPATHDAT.ASM:1140
    pb_label(&b, "pspiralexplode");
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit_setb(&b, PAL_HP, 10);
    pb_emit_goto(&b, P_FORCE, "pspiralexplode.spiral");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "pspiralexplode.spiral");
    pb_emit8(&b, P_SMOKEON);
    pb_emit_trigger(&b, "pspiralexplode", -1);
    pb_emit_trigger(&b, "pspiralexplode.spinit", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_emit_setvel(&b, 0);
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE1, 63);
    pb_emit_add(&b, PAL_PBYTE1, -32);
    pb_emit_setrandomb(&b, PAL_ABS_PBYTE2, 63);
    pb_emit_negb(&b, PAL_ABS_PBYTE2);
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 8);
    pb_emit_add(&b, PAL_WORLDZ, 100);
    pb_emit_addvwb(&b, PAL_WORLDX, PAL_PBYTE1);
    pb_emit_addvwb(&b, PAL_WORLDY, PAL_PBYTE2);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "pspiralexplode.spinit");
    pb_emit_add(&b, PAL_ROTZ, DEG22);
    pb_emit8(&b, P_RETURN);

    // DPATHDAT.ASM:1199
    pb_start_path(&b, PATH_ID_MY_BIRD, "my_bird");
    pb_emit_importb(&b, PAL_PBYTE1, PATH_EXT_EROLL1);
    pb_emit_ifzerob(&b, PAL_PBYTE1, "my_bird.carryon");
    pb_emit8(&b, P_REMOVE);
    pb_label(&b, "my_bird.carryon");
    pb_emit_zero(&b, PAL_PBYTE1);
    pb_emit_exportb(&b, PAL_PBYTE1, PATH_EXT_EROLL1);
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_INVINCIBLEON);
    pb_emit8(&b, P_COLLISIONSOFF);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit_trigger(&b, "my_bird.anim", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit_setvel(&b, 30);
    pb_emit_soundeffect(&b, 0x90);
    pb_emit_add(&b, PAL_ROTX, DEG22);
    pb_emit8(&b, P_LEFTOFPLAYER);
    pb_fixup16(&b, "my_bird.oneway");
    pb_emit_add(&b, PAL_ROTY, DEG45 + DEG22);
    pb_label(&b, "my_bird.oneway");
    pb_emit_add(&b, PAL_ROTY, -DEG22 - DEG11);
    pb_label(&b, "my_bird.wait");
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_IFNOT);
    pb_emit_distless(&b, 1001, "my_bird.wait");
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_trigger(&b, "my_bird.anim", -1);
    pb_emit_trigger(&b, "my_bird.special", PATH_TRIGGER_WHENHIT_VALUE);
    pb_label(&b, "my_bird.leftright");
    pb_emit_do(&b, 30);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit_ifbetweenb(&b, PAL_ROTY, 0, 127, "my_bird.bankright");
    pb_emit_add(&b, PAL_ROTZ, 4);
    pb_label(&b, "my_bird.bankright");
    pb_emit_add(&b, PAL_ROTZ, -2);
    pb_emit8(&b, P_NEXT);
    pb_emit_trigger(&b, "my_bird.special", -1);
    pb_emit_accel(&b, 60, 5);
    pb_emit_goto(&b, P_RANDOMGOTO, "my_bird.backtowardsplayer");
    pb_emit_trigger(&b, "my_bird.anim", PATH_TRIGGER_ALWAYS_VALUE);
    pb_emit_do(&b, 50);
    // The source spells the bounds in reverse order; the intended signed range
    // is -deg90..-deg45, matching the later shared DPATHDAT usage.
    pb_emit_ifbetweenb(&b, PAL_ROTX, -DEG90, -DEG45, "my_bird.nomore");
    pb_emit_add(&b, PAL_ROTX, -2);
    pb_label(&b, "my_bird.nomore");
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_REMOVE);
    pb_label(&b, "my_bird.backtowardsplayer");
    pb_emit8(&b, P_LEFTOFPLAYER);
    pb_fixup16(&b, "my_bird.oneway2");
    pb_emit_setb(&b, PAL_PBYTE1, 2);
    pb_emit_goto(&b, P_IGOTO, "my_bird.otherway2");
    pb_label(&b, "my_bird.oneway2");
    pb_emit_setb(&b, PAL_PBYTE1, -2);
    pb_label(&b, "my_bird.otherway2");
    pb_emit_do(&b, 10);
    pb_emit_addv(&b, false, PAL_ROTZ, false, PAL_PBYTE1);
    pb_emit8(&b, P_NEXT);
    pb_emit_negb(&b, PAL_ABS_PBYTE1);
    pb_emit_do(&b, 10);
    pb_emit_addv(&b, false, PAL_ROTZ, false, PAL_PBYTE1);
    pb_emit8(&b, P_NEXT);
    pb_label(&b, "my_bird.forever");
    pb_emit_chaseb(&b, PAL_ROTZ, 0);
    pb_emit8(&b, P_IFNOT);
    pb_emit_distless(&b, 701, "my_bird.nochange");
    pb_emit8(&b, P_ADDANIM);
    pb_emit8(&b, 1);
    pb_emit8(&b, 16);
    pb_emit_ifbetweenb(&b, PAL_ROTX, -DEG90, -DEG45, "my_bird.nochange2");
    pb_emit_add(&b, PAL_ROTX, -3);
    pb_emit_goto(&b, P_IGOTO, "my_bird.nochange2");
    pb_label(&b, "my_bird.nochange");
    pb_emit8(&b, P_FACEPLAYER);
    pb_label(&b, "my_bird.nochange2");
    pb_emit_goto(&b, P_GOTO, "my_bird.forever");
    pb_label(&b, "my_bird.anim");
    pb_emit8(&b, P_ADDANIM);
    pb_emit8(&b, 1);
    pb_emit8(&b, 16);
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "my_bird.special");
    pb_emit_qspawn(&b, SH_GATE_2, PATH_ID_RING, 10, 10);
    pb_emit8(&b, P_RETURN);

    // DPATHDAT.ASM:1290
    pb_start_path(&b, PATH_ID_RING, "ring");
    pb_emit_setstrat_flat(&b, STRAT_ID_GATE2);

    // PATHDATA.ASM:5306
    pb_start_path(&b, PATH_ID_PATROL, "patrol");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RELSLOWELASER);
    pb_emit_goto(&b, P_RANDOMGOTO, "patrol.patr_ran");
    pb_emit_trigger(&b, "pspiralexplode", PATH_TRIGGER_WHENDEAD_VALUE);
    pb_label(&b, "patrol.patr_ran");
    pb_emit8(&b, P_SOUND);
    pb_emit8(&b, 3);
    pb_emit_setb(&b, PAL_HP, 2);
    pb_emit_setb(&b, PAL_AP, 8);
    pb_emit8(&b, P_RIGHTOFPLAYER);
    pb_fixup16(&b, "patrol.pat_r");
    pb_emit_setb(&b, PAL_ROTY, (int8)DEG270);
    pb_emit_setvel(&b, 50);
    pb_label(&b, "patrol.pat_l_0");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -250, 250, "patrol.pat_l_1");
    pb_emit8(&b, P_RIGHTOFPLAYER);
    pb_fixup16(&b, "patrol.pat_l_1");
    pb_emit_goto(&b, P_GOTO, "patrol.pat_l_0");
    pb_label(&b, "patrol.pat_l_1");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "patrol.pat_f_sub");
    pb_emit_waitchaseb(&b, PAL_ROTY, (uint8)DEG270);
    pb_emit_goto(&b, P_IGOTO, "patrol.pat_b_end");
    pb_label(&b, "patrol.pat_r");
    pb_emit_setb(&b, PAL_ROTY, DEG90);
    pb_label(&b, "patrol.pat_r_s");
    pb_emit_setvel(&b, 50);
    pb_label(&b, "patrol.pat_r_0");
    pb_emit_ifbetweenw(&b, PAL_WORLDX, -250, 250, "patrol.pat_r_1");
    pb_emit8(&b, P_LEFTOFPLAYER);
    pb_fixup16(&b, "patrol.pat_r_1");
    pb_emit_goto(&b, P_GOTO, "patrol.pat_r_0");
    pb_label(&b, "patrol.pat_r_1");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "patrol.pat_f_sub");
    pb_emit_waitchaseb(&b, PAL_ROTY, DEG90);
    pb_label(&b, "patrol.pat_b_end");
    pb_emit_accel(&b, 50, 5);
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 50);
    pb_emit_chaseb(&b, PAL_ROTX, 0);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_REMOVE);
    pb_label(&b, "patrol.pat_f_sub");
    pb_emit_accel(&b, 0, 5);
    pb_emit_waitchaseb(&b, PAL_ROTY, DEG180);
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 3);
    pb_emit8(&b, P_DOQ);
    pb_emit8(&b, 10);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_RETURN);

    // PATHDATA.ASM:8250
    pb_start_path(&b, PATH_ID_PATRET_IRAB, "patret_irab");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "patret_ifal.patfal_init");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "patret_ifal.inter_sub");
    pb_emit_add(&b, PAL_WORLDZ, -1500);
    pb_emit_spawn_link(&b, 0, PATH_I8(-300), 0, 0, 128, 0,
                       SH_FRIENDSHIP_4, PATH_ID_SEPTER_RAB, 100, 1);
    pb_emit_add(&b, PAL_WORLDZ, 1500);
    pb_emit_goto(&b, P_IGOTO, "patret_ifal.inter_tim");

    pb_start_path(&b, PATH_ID_PATRET_IFRO, "patret_ifro");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "patret_ifal.patfal_init");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "patret_ifal.inter_sub");
    pb_emit_add(&b, PAL_WORLDZ, -1500);
    pb_emit_spawn_link(&b, 0, PATH_I8(-300), 0, 0, 128, 0,
                       SH_FRIENDSHIP_4, PATH_ID_SEPTER_FRO, 100, 1);
    pb_emit_add(&b, PAL_WORLDZ, 1500);
    pb_emit_goto(&b, P_IGOTO, "patret_ifal.inter_tim");

    pb_start_path(&b, PATH_ID_PATRET_IFAL, "patret_ifal");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "patret_ifal.patfal_init");
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "patret_ifal.inter_sub");
    pb_emit_add(&b, PAL_WORLDZ, -1500);
    pb_emit_spawn_link(&b, 0, PATH_I8(-300), 0, 0, 128, 0,
                       SH_FRIENDSHIP_4, PATH_ID_SEPTER_FAL, 100, 1);
    pb_emit_add(&b, PAL_WORLDZ, 1500);

    pb_label(&b, "patret_ifal.inter_tim");
    pb_emit_setb(&b, PAL_HP, 30);
    pb_emit_setvel(&b, 30);
    pb_emit_wait(&b, 22);
    pb_emit8(&b, P_IFFLAG);
    pb_fixup16(&b, "patret_ifal.inter_con");
    pb_emit8(&b, P_EXPLODE);
    pb_label(&b, "patret_ifal.inter_con");
    pb_emit_wait(&b, 50);
    pb_emit8(&b, P_END);

    pb_label(&b, "patret_ifal.patfal_init");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit8(&b, P_SPACESHIPON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_WEAPON);
    pb_emit8(&b, WEAPON_RINGLASER);
    pb_emit8(&b, P_INITANIM);
    pb_emit8(&b, 3);
    pb_emit_setb(&b, PAL_PBYTE1, 0);
    pb_emit_exportb(&b, PAL_PBYTE1, PATH_EXT_EBYTE2);
    pb_emit_iflevel(&b, 1, "patret_ifal.patfal_jp");
    pb_emit8(&b, P_SOUND);
    pb_emit8(&b, 3);
    pb_label(&b, "patret_ifal.patfal_jp");
    pb_emit_setb(&b, PAL_HP, 2);
    pb_emit_setb(&b, PAL_AP, 8);
    pb_emit8(&b, P_RETURN);

    pb_label(&b, "patret_ifal.inter_sub");
    pb_emit_setvel(&b, 60);
    pb_emit_wait(&b, 50);
    pb_emit_accel(&b, 20, 2);
    pb_emit8(&b, P_LEFTOFPLAYER);
    pb_fixup16(&b, "patret_ifal.inter_0");
    pb_emit_waitchaseb(&b, PAL_ROTZ, 28);
    pb_emit_goto(&b, P_IGOTO, "patret_ifal.inter_1");
    pb_label(&b, "patret_ifal.inter_0");
    pb_emit_waitchaseb(&b, PAL_ROTZ, (uint8)(256 - 28));
    pb_label(&b, "patret_ifal.inter_1");
    pb_emit_waitchaseb(&b, PAL_ROTZ, 0);
    pb_emit_setvel(&b, 50);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit8(&b, P_DO);
    pb_emit8(&b, 15);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_FIRE);
    pb_emit8(&b, P_DO);
    pb_emit8(&b, 15);
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit8(&b, P_NEXT);
    pb_emit8(&b, P_FIRE);
    pb_label(&b, "patret_ifal.inter_w");
    pb_emit8(&b, P_FACEPLAYER);
    pb_emit_distless(&b, 1300, "patret_ifal.inter_11");
    pb_emit_goto(&b, P_GOTO, "patret_ifal.inter_w");
    pb_label(&b, "patret_ifal.inter_11");
    pb_emit8(&b, P_RETURN);

    // ====================================================================
    // KPATHDAT.ASM — ending/transition camera paths (Krister's paths)
    // ====================================================================

    // endoff equ 1 (constant used in stage wait durations)
#define ENDOFF 1

    // ------------------------------------------------------------------
    // text_swoopin: shared subroutine from DPATHDAT.ASM used by total/ave
    // ------------------------------------------------------------------
    pb_label(&b, "text_swoopin");
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_BELOWPLAYER);
    pb_fixup16(&b, "text_swoopin.swoopup");
    // above player: swoop down
    pb_emit_setb(&b, PAL_ROTX, (int8)(-DEG90));  // -64 → 192
    pb_label(&b, "text_swoopin.loopround");
    pb_emit_add(&b, PAL_ROTX, DEG5);             // +4
    pb_emit_ifsameb(&b, PAL_ROTX, DEG0, "text_swoopin.leave");
    pb_emit_goto(&b, P_GOTO, "text_swoopin.loopround");
    pb_label(&b, "text_swoopin.leave");
    pb_emit8(&b, P_RETURN);
    pb_label(&b, "text_swoopin.swoopup");
    pb_emit_setb(&b, PAL_ROTX, DEG90);           // 64
    pb_label(&b, "text_swoopin.loopround2");
    pb_emit_add(&b, PAL_ROTX, -DEG5);            // -4
    pb_emit_ifsameb(&b, PAL_ROTX, DEG0, "text_swoopin.leave");
    pb_emit_goto(&b, P_GOTO, "text_swoopin.loopround2");

    // ------------------------------------------------------------------
    // kwaitchk: shared subroutine — wait until c_type == 0
    // ------------------------------------------------------------------
    pb_label(&b, "kwaitchk");
    pb_label(&b, "kwaitchk.wait");
    pb_emit_importb(&b, PAL_PBYTE1, PATH_EXT_CTYPE);
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_IFNOTZEROB);
    pb_emit8(&b, PAL_PBYTE1);
    pb_fixup16(&b, "kwaitchk.wait");
    pb_emit8(&b, P_RETURN);

    // ------------------------------------------------------------------
    // endin: shared tail for theend* paths — spin and drift back
    // ------------------------------------------------------------------
    pb_label(&b, "endin");
    pb_emit_add(&b, PAL_ROTY, 4);
    pb_emit_add(&b, PAL_WORLDZ, -19);
    pb_emit_loop(&b, 32, "endin");
    pb_label(&b, "endin.ww");
    pb_emit_wait(&b, 30);
    pb_emit_goto(&b, P_GOTO, "endin.ww");

    // ------------------------------------------------------------------
    // gameover
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_GAMEOVER, "gameover");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_emit8(&b, P_ZREMOVEOFF);
    // P_SET rotx, -192  → byte value 64 (= DEG90)
    pb_emit_setb(&b, PAL_ROTX, 64);
    // P_SET roty, -192  → byte value 64
    pb_emit_setb(&b, PAL_ROTY, 64);
    pb_label(&b, "gameover.lp");
    pb_emit_add(&b, PAL_ROTY, 4);
    pb_emit_add(&b, PAL_ROTX, 6);
    pb_emit_add(&b, PAL_WORLDZ, -69);
    pb_emit_loop(&b, 32, "gameover.lp");
    pb_label(&b, "gameover.lp2");
    pb_emit_add(&b, PAL_ROTY, 4);
    pb_emit_loop(&b, 16, "gameover.lp2");
    pb_emit_zero(&b, PAL_ROTX);
    pb_emit_zero(&b, PAL_ROTY);
    pb_label(&b, "gameover.wait");
    pb_emit_goto(&b, P_GOTO, "gameover.wait");

    // ------------------------------------------------------------------
    // theendt
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_THEENDT, "theendt");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_emit8(&b, P_ZREMOVEON);
    pb_label(&b, "theendt.lp");
    pb_emit_add(&b, PAL_WORLDX, -22);
    pb_emit_add(&b, PAL_WORLDY, 19);
    pb_emit_add(&b, PAL_ROTX, 5);
    pb_emit_add(&b, PAL_ROTY, 5);
    pb_emit_loop(&b, 50, "theendt.lp");
    pb_emit_zero(&b, PAL_ROTX);
    pb_emit_zero(&b, PAL_ROTY);
    pb_label(&b, "theendt.wait");
    pb_emit_goto(&b, P_GOTO, "endin");

    // ------------------------------------------------------------------
    // theendh
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_THEENDH, "theendh");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setb(&b, PAL_ROTX, 0);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_label(&b, "theendh.lp");
    pb_emit_add(&b, PAL_WORLDX, 20);
    pb_emit_add(&b, PAL_WORLDY, -27);
    pb_emit_add(&b, PAL_ROTX, 5);
    pb_emit_add(&b, PAL_ROTY, 5);
    pb_emit_loop(&b, 50, "theendh.lp");
    pb_emit_zero(&b, PAL_ROTX);
    pb_emit_zero(&b, PAL_ROTY);
    pb_label(&b, "theendh.wait");
    pb_emit_goto(&b, P_GOTO, "endin");

    // ------------------------------------------------------------------
    // theende
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_THEENDE, "theende");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setb(&b, PAL_ROTX, 0);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_label(&b, "theende.lp");
    pb_emit_add(&b, PAL_WORLDX, 19);
    pb_emit_add(&b, PAL_WORLDY, 30);
    pb_emit_add(&b, PAL_ROTX, 5);
    pb_emit_add(&b, PAL_ROTY, 5);
    pb_emit_loop(&b, 50, "theende.lp");
    pb_emit_zero(&b, PAL_ROTX);
    pb_emit_zero(&b, PAL_ROTY);
    pb_label(&b, "theende.wait");
    pb_emit_goto(&b, P_GOTO, "endin");

    // ------------------------------------------------------------------
    // theende2
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_THEENDE2, "theende2");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setb(&b, PAL_ROTX, 0);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_label(&b, "theende2.lp");
    pb_emit_add(&b, PAL_WORLDX, -20);
    pb_emit_add(&b, PAL_WORLDY, 26);
    pb_emit_add(&b, PAL_ROTX, 5);
    pb_emit_add(&b, PAL_ROTY, 5);
    pb_emit_loop(&b, 50, "theende2.lp");
    pb_emit_zero(&b, PAL_ROTX);
    pb_emit_zero(&b, PAL_ROTY);
    pb_label(&b, "theende2.wait");
    pb_emit_goto(&b, P_GOTO, "endin");

    // ------------------------------------------------------------------
    // theendn
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_THEENDN, "theendn");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setb(&b, PAL_ROTX, 0);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_label(&b, "theendn.lp");
    pb_emit_add(&b, PAL_WORLDX, -29);
    pb_emit_add(&b, PAL_WORLDY, -27);
    pb_emit_add(&b, PAL_ROTX, 5);
    pb_emit_add(&b, PAL_ROTY, 5);
    pb_emit_loop(&b, 50, "theendn.lp");
    pb_emit_zero(&b, PAL_ROTX);
    pb_emit_zero(&b, PAL_ROTY);
    pb_label(&b, "theendn.wait");
    pb_emit_goto(&b, P_GOTO, "endin");

    // ------------------------------------------------------------------
    // theendd
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_THEENDD, "theendd");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSOFF);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setb(&b, PAL_ROTY, 128);
    pb_label(&b, "theendd.lp");
    pb_emit_add(&b, PAL_WORLDX, 21);
    pb_emit_add(&b, PAL_WORLDY, -28);
    pb_emit_add(&b, PAL_ROTX, 5);
    pb_emit_add(&b, PAL_ROTY, 5);
    pb_emit_loop(&b, 50, "theendd.lp");
    pb_emit_zero(&b, PAL_ROTX);
    pb_emit_zero(&b, PAL_ROTY);
    pb_label(&b, "theendd.wait");
    pb_emit_goto(&b, P_GOTO, "endin");

    // ------------------------------------------------------------------
    // fadeintotal
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_FADEINTOTAL, "fadeintotal");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, 0);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);  // OFF
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_INCW);
    pb_emit8(&b, PAL_DEPTHOFFSET);
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_INCW);
    pb_emit8(&b, PAL_DEPTHOFFSET);
    pb_emit_wait(&b, 1);
    pb_emit8(&b, P_INCW);
    pb_emit8(&b, PAL_DEPTHOFFSET);
    pb_label(&b, "fadeintotal.eternity");
    pb_emit_goto(&b, P_GOTO, "fadeintotal.eternity");

    // ------------------------------------------------------------------
    // total
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_TOTAL, "total");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, (uint8)(int8)(-120));  // -120 = 0x88 = 136
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "text_swoopin");
    pb_emit_setvel(&b, 0);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);  // OFF
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "kwaitchk");
    pb_emit_wait(&b, 30);
    pb_label(&b, "total.boloxm");
    pb_emit_addw(&b, PAL_WORLDX, -200);
    pb_emit_loop(&b, 15, "total.boloxm");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // ------------------------------------------------------------------
    // totaln
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_TOTALN, "totaln");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, (uint8)(int8)(-120));
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "text_swoopin");
    pb_emit_setvel(&b, 0);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);  // OFF
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "kwaitchk");
    pb_emit_wait(&b, 40);
    pb_label(&b, "totaln.boloxm");
    pb_emit_addw(&b, PAL_WORLDX, 200);
    pb_emit_loop(&b, 15, "totaln.boloxm");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // ------------------------------------------------------------------
    // ave
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_AVE, "ave");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, (uint8)(int8)(-120));
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "text_swoopin");
    pb_emit_setvel(&b, 0);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);  // OFF
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "kwaitchk");
    pb_emit_wait(&b, 50);
    pb_label(&b, "ave.boloxm");
    pb_emit_addw(&b, PAL_WORLDX, -200);
    pb_emit_loop(&b, 15, "ave.boloxm");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // ------------------------------------------------------------------
    // aven
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_AVEN, "aven");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, (uint8)(int8)(-120));
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "text_swoopin");
    pb_emit_setvel(&b, 0);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);  // OFF
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "kwaitchk");
    pb_emit_wait(&b, 45);
    pb_label(&b, "aven.boloxm");
    pb_emit_addw(&b, PAL_WORLDX, 200);
    pb_emit_loop(&b, 15, "aven.boloxm");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // ------------------------------------------------------------------
    // stage1
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_STAGE1, "stage1");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, 0);
    pb_label(&b, "stage1.go");
    pb_emit_add(&b, PAL_WORLDZ, -100);
    pb_emit_loop(&b, 15, "stage1.go");
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);  // OFF
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "kwaitchk");
    pb_emit_wait(&b, 10 + ENDOFF);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 7);  // colour 7
    pb_emit_wait(&b, 1);
    // checkifend 1
    pb_emit_start65816(&b, &s_checkifend1_ip, "stage1.after_chk");
    pb_label(&b, "stage1.after_chk");
    pb_label(&b, "stage1.go2");
    pb_emit_addw(&b, PAL_WORLDX, 200);
    pb_emit_loop(&b, 15, "stage1.go2");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // ------------------------------------------------------------------
    // stage2
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_STAGE2, "stage2");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, 0);
    pb_label(&b, "stage2.go");
    pb_emit_add(&b, PAL_WORLDZ, -100);
    pb_emit_loop(&b, 15, "stage2.go");
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);  // OFF
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "kwaitchk");
    pb_emit_wait(&b, 15 + ENDOFF);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 7);
    pb_emit_wait(&b, 1);
    // checkifend 2
    pb_emit_start65816(&b, &s_checkifend2_ip, "stage2.after_chk");
    pb_label(&b, "stage2.after_chk");
    pb_label(&b, "stage2.go2");
    pb_emit_addw(&b, PAL_WORLDX, -200);
    pb_emit_loop(&b, 15, "stage2.go2");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // ------------------------------------------------------------------
    // stage3
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_STAGE3, "stage3");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, 0);
    pb_label(&b, "stage3.go");
    pb_emit_add(&b, PAL_WORLDZ, -100);
    pb_emit_loop(&b, 15, "stage3.go");
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "kwaitchk");
    pb_emit_wait(&b, 20 + ENDOFF);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 7);
    pb_emit_wait(&b, 1);
    // checkifend 3
    pb_emit_start65816(&b, &s_checkifend3_ip, "stage3.after_chk");
    pb_label(&b, "stage3.after_chk");
    pb_label(&b, "stage3.go2");
    pb_emit_addw(&b, PAL_WORLDX, 200);
    pb_emit_loop(&b, 15, "stage3.go2");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // ------------------------------------------------------------------
    // stage4
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_STAGE4, "stage4");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, 0);
    pb_label(&b, "stage4.go");
    pb_emit_add(&b, PAL_WORLDZ, -100);
    pb_emit_loop(&b, 15, "stage4.go");
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "kwaitchk");
    pb_emit_wait(&b, 25 + ENDOFF);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 7);
    pb_emit_wait(&b, 1);
    // checkifend 4
    pb_emit_start65816(&b, &s_checkifend4_ip, "stage4.after_chk");
    pb_label(&b, "stage4.after_chk");
    pb_label(&b, "stage4.go2");
    pb_emit_addw(&b, PAL_WORLDX, -200);
    pb_emit_loop(&b, 15, "stage4.go2");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // ------------------------------------------------------------------
    // stage5
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_STAGE5, "stage5");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, 0);
    pb_label(&b, "stage5.go");
    pb_emit_add(&b, PAL_WORLDZ, -100);
    pb_emit_loop(&b, 15, "stage5.go");
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "kwaitchk");
    pb_emit_wait(&b, 30 + ENDOFF);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 7);
    pb_emit_wait(&b, 1);
    // checkifend 5
    pb_emit_start65816(&b, &s_checkifend5_ip, "stage5.after_chk");
    pb_label(&b, "stage5.after_chk");
    pb_label(&b, "stage5.go2");
    pb_emit_addw(&b, PAL_WORLDX, 200);
    pb_emit_loop(&b, 15, "stage5.go2");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // ------------------------------------------------------------------
    // stage6
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_STAGE6, "stage6");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, 0);
    pb_label(&b, "stage6.go");
    pb_emit_add(&b, PAL_WORLDZ, -100);
    pb_emit_loop(&b, 15, "stage6.go");
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "kwaitchk");
    pb_emit_wait(&b, 35 + ENDOFF);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 7);
    pb_emit_wait(&b, 1);
    // checkifend 6
    pb_emit_start65816(&b, &s_checkifend6_ip, "stage6.after_chk");
    pb_label(&b, "stage6.after_chk");
    pb_label(&b, "stage6.go2");
    pb_emit_addw(&b, PAL_WORLDX, -200);
    pb_emit_loop(&b, 15, "stage6.go2");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // ------------------------------------------------------------------
    // stage7 — note: no trail colour 7, just trail OFF
    // ------------------------------------------------------------------
    pb_start_path(&b, PATH_ID_STAGE7, "stage7");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ALWAYSGENVECSON);
    pb_emit8(&b, P_ZREMOVEON);
    pb_emit_setvel(&b, 0);
    pb_label(&b, "stage7.go");
    pb_emit_add(&b, PAL_WORLDZ, -100);
    pb_emit_loop(&b, 15, "stage7.go");
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);
    pb_emit8(&b, P_GOSUB);
    pb_fixup16(&b, "kwaitchk");
    pb_emit_wait(&b, 40 + ENDOFF);
    pb_emit8(&b, P_TRAIL);
    pb_emit8(&b, 0);  // OFF (stage7 has no colour)
    pb_emit_wait(&b, 1);
    // checkifend 7
    pb_emit_start65816(&b, &s_checkifend7_ip, "stage7.after_chk");
    pb_label(&b, "stage7.after_chk");
    pb_label(&b, "stage7.go2");
    pb_emit_addw(&b, PAL_WORLDX, 200);
    pb_emit_loop(&b, 15, "stage7.go2");
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

#undef ENDOFF

    // MAP3_4B (Sector Z Part B) paths — stub until full path data is ported.
    pb_start_path(&b, PATH_ID_CALL_FOL, "call_fol");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_wait(&b, 200);
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    // MAP3_7A (Venom 3 Surface Part A) paths — stub until full path data is ported.
    pb_start_path(&b, PATH_ID_E_DOSUN, "e_dosun");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_wait(&b, 200);
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    pb_start_path(&b, PATH_ID_ITADOSUN, "itadosun");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_wait(&b, 200);
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    pb_start_path(&b, PATH_ID_E_KURURI, "e_kururi");
    pb_emit8(&b, P_RELTOPLAYERON);
    pb_emit8(&b, P_ZREMOVEOFF);
    pb_emit_wait(&b, 200);
    pb_emit8(&b, P_REMOVE);
    pb_emit8(&b, P_END);

    pb_resolve(&b);

    if (b.failed) {
        s_path_data[0] = P_REMOVE;
        memset(s_path_offsets, 0xFF, sizeof(s_path_offsets));
        s_catalog.length = 1;
    } else {
        s_catalog.length = b.length;
    }
}

const PathLiteralCatalog *PathLiterals_GetCatalog(void) {
    if (!s_catalog_ready) {
        build_path_catalog();
        s_catalog_ready = true;
    }
    seed_runtime_tables();
    register_inline_callbacks();
    return &s_catalog;
}
