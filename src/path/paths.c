// PATHS.ASM → C conversion
// PATH bytecode interpreter — scripted movement sequences.
//
// Decompiled from PATHS.ASM: path_istrat, .strat, mode_table, .move
// and PATHMACS.ASM: opcode definitions and getparam mechanism.
//
// The path interpreter is a bytecode VM embedded within the strategy system.
// Each path-following alien has al_sword2 as its instruction pointer into
// the global "paths" bytecode array. Each frame, the interpreter reads one
// or more opcodes, applies their effects, then runs the physics "move" phase.
//
// Key architectural details:
// - al_sword2: instruction pointer (16-bit offset into paths[])
// - al_sbyte2: loop counter
// - al_sbyte3: wait counter
// - al_sbyte4: friend ID (0 = not a friend)
// - al_count / al_count1: acceleration parameters
// - sflags2 bits: control per-frame behavior (Z-relative, auto-genvecs, etc.)

#include "paths.h"
#include "../game/obj.h"
#include "../game/alien_compat.h"
#include "../game/game_vars.h"
#include "../game/world.h"
#include "../game/sound.h"
#include "../game/strings.h"
#include "../strat/strat_common.h"
#include "../strat/strat_enemy.h"
#include "../sf_rtl.h"
#include "../variables.h"
#include <math.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

// ============================================================
// Path data (loaded from ROM extraction)
// ============================================================
const uint8 *g_path_data = NULL;
uint32 g_path_length = 0;
const uint16 *g_path_offsets = NULL;
uint16 g_path_count = 0;
static uint8 s_missing_path_warned[512];

#define PATH_VM_STACK_DEPTH 64
#define PATH_VM_TRIGGER_CAPACITY 16
#define PATH_INLINE_CALLBACK_CAPACITY 16
#define ALIEN_ABS_ALX_START 0x100u
#define PATH_NULL_OBJ 0u
#define PATH_MED_PSPEED 65

enum {
    PATH_TRIGGER_ALWAYS = 0,
    PATH_TRIGGER_2 = 1,
    PATH_TRIGGER_4 = 2,
    PATH_TRIGGER_8 = 3,
    PATH_TRIGGER_16 = 4,
    PATH_TRIGGER_32 = 5,
    PATH_TRIGGER_64 = 6,
    PATH_TRIGGER_128 = 7,
    PATH_TRIGGER_WHENHIT = 8,
    PATH_TRIGGER_WHENHITBYPLAYER = 9,
    PATH_TRIGGER_WHENFLAGGED = 10,
    PATH_TRIGGER_WHENSHAPEDEAD = 11,
    PATH_TRIGGER_WHENDEAD = 12,
};

typedef struct {
    uint16 addr;
    uint8 trigger;
} PathTriggerEntry;

typedef struct {
    Alien *owner;
    uint16 data[PATH_VM_STACK_DEPTH];
    uint8 sp;
    PathTriggerEntry triggers[PATH_VM_TRIGGER_CAPACITY];
    uint8 trigger_count;
    bool in_trigger;
    bool trigger_force_pending;
    uint16 trigger_forced_ip;
    bool ifnot_pending;
} PathVmState;

static PathVmState g_path_vm[NUMBER_AL];
static uint16 g_path_link_obj = PATH_NULL_OBJ;
static uint16 g_path_become_obj = PATH_NULL_OBJ;
static uint16 g_path_become_last_obj = PATH_NULL_OBJ;
static uint16 g_path_become_path = 0;

typedef struct {
    uint16 script_ptr;
    PathInlineFunc fn;
} PathInlineCallback;

static PathInlineCallback s_path_inline_callbacks[PATH_INLINE_CALLBACK_CAPACITY];

// ============================================================
// Bytecode read helpers
// ============================================================
static uint8 path_read8(uint16 ip, int offset) {
    uint32 addr = (uint32)ip + offset;
    if (addr < g_path_length) return g_path_data[addr];
    return 0;
}

static int8 path_read8s(uint16 ip, int offset) {
    return (int8)path_read8(ip, offset);
}

static uint16 path_read16(uint16 ip, int offset) {
    uint32 addr = (uint32)ip + offset;
    if (addr + 1 < g_path_length) {
        return (uint16)g_path_data[addr] | ((uint16)g_path_data[addr + 1] << 8);
    }
    return 0;
}

static int16 path_read16s(uint16 ip, int offset) {
    return (int16)path_read16(ip, offset);
}

static PathInlineFunc path_find_inline_code(uint16 script_ptr) {
    for (int i = 0; i < PATH_INLINE_CALLBACK_CAPACITY; i++) {
        if (s_path_inline_callbacks[i].fn &&
            s_path_inline_callbacks[i].script_ptr == script_ptr) {
            return s_path_inline_callbacks[i].fn;
        }
    }
    return NULL;
}

void Paths_RegisterInlineCode(uint16 script_ptr, PathInlineFunc fn) {
    for (int i = 0; i < PATH_INLINE_CALLBACK_CAPACITY; i++) {
        if (s_path_inline_callbacks[i].fn &&
            s_path_inline_callbacks[i].script_ptr == script_ptr) {
            s_path_inline_callbacks[i].fn = fn;
            return;
        }
    }
    for (int i = 0; i < PATH_INLINE_CALLBACK_CAPACITY; i++) {
        if (!s_path_inline_callbacks[i].fn) {
            s_path_inline_callbacks[i].script_ptr = script_ptr;
            s_path_inline_callbacks[i].fn = fn;
            return;
        }
    }
}

static bool path_decode_start65816_return(uint16 ip, uint16 *out_target) {
    if (!out_target) return false;
    uint32 start = (uint32)ip + 1u;
    if (start >= g_path_length) return false;

    // Inline 65816 snippets in PATH data are small macro expansions.
    // Scan forward for an RTL (0x6B) and decode common return patterns:
    // - P_END65816:   A9 lo hi 6B
    // - P_SWITCHOUT:  A9 hi EB A9 lo 6B
    const uint32 max_scan = 384u;
    uint32 end = start + max_scan;
    if (end > g_path_length) end = g_path_length;

    for (uint32 p = start; p < end; p++) {
        if (g_path_data[p] != 0x6Bu) continue;

        // a16; lda #imm16; rtl  => A9 lo hi 6B
        if (p >= start + 3u && g_path_data[p - 3u] == 0xA9u) {
            uint8 lo = g_path_data[p - 2u];
            uint8 hi = g_path_data[p - 1u];
            *out_target = (uint16)lo | ((uint16)hi << 8);
            return true;
        }

        // lda #hi ; xba ; lda #lo ; rtl => A9 hi EB A9 lo 6B
        if (p >= start + 5u &&
            g_path_data[p - 5u] == 0xA9u &&
            g_path_data[p - 3u] == 0xEBu &&
            g_path_data[p - 2u] == 0xA9u) {
            uint8 hi = g_path_data[p - 4u];
            uint8 lo = g_path_data[p - 1u];
            *out_target = (uint16)lo | ((uint16)hi << 8);
            return true;
        }
    }

    return false;
}

static uint32 path_ext_addr_wrap(uint32 addr) {
    return addr % WRAM_SIZE;
}

#define PATH_EXT_EROLL1 0x2302u
#define PATH_EXT_EBYTE3 0x2303u

static uint8 path_read_ext8(uint16 addr) {
    if (addr == PATH_EXT_EROLL1) {
        return g_eroll1;
    }
    if (addr == PATH_EXT_EBYTE3) {
        return g_ebyte3;
    }
    return g_ram[path_ext_addr_wrap(addr)];
}

static uint16 path_read_ext16(uint16 addr) {
    uint32 a0 = path_ext_addr_wrap(addr);
    uint32 a1 = path_ext_addr_wrap((uint32)addr + 1);
    return (uint16)g_ram[a0] | ((uint16)g_ram[a1] << 8);
}

static void path_write_ext8(uint16 addr, uint8 value) {
    if (addr == PATH_EXT_EROLL1) {
        g_eroll1 = value;
        return;
    }
    if (addr == PATH_EXT_EBYTE3) {
        g_ebyte3 = value;
        return;
    }
    g_ram[path_ext_addr_wrap(addr)] = value;
}

static void path_write_ext16(uint16 addr, uint16 value) {
    uint32 a0 = path_ext_addr_wrap(addr);
    uint32 a1 = path_ext_addr_wrap((uint32)addr + 1);
    g_ram[a0] = (uint8)(value & 0xFFu);
    g_ram[a1] = (uint8)((value >> 8) & 0xFFu);
}

static bool path_abs_read8(const Alien *al, uint16 abs_offset, uint8 *out) {
    if (abs_offset < ALIEN_ABS_ALX_START) {
        return AlienCompat_Read8(al, abs_offset, false, out);
    }
    return AlienCompat_Read8(al, (uint16)(abs_offset - ALIEN_ABS_ALX_START), true, out);
}

static bool path_abs_read16(const Alien *al, uint16 abs_offset, uint16 *out) {
    if (abs_offset < ALIEN_ABS_ALX_START) {
        return AlienCompat_Read16(al, abs_offset, false, out);
    }
    return AlienCompat_Read16(al, (uint16)(abs_offset - ALIEN_ABS_ALX_START), true, out);
}

static bool path_abs_write8(Alien *al, uint16 abs_offset, uint8 value) {
    if (abs_offset < ALIEN_ABS_ALX_START) {
        return AlienCompat_Write8(al, abs_offset, false, value);
    }
    return AlienCompat_Write8(al, (uint16)(abs_offset - ALIEN_ABS_ALX_START), true, value);
}

static bool path_abs_write16(Alien *al, uint16 abs_offset, uint16 value) {
    if (abs_offset < ALIEN_ABS_ALX_START) {
        return AlienCompat_Write16(al, abs_offset, false, value);
    }
    return AlienCompat_Write16(al, (uint16)(abs_offset - ALIEN_ABS_ALX_START), true, value);
}

static PathVmState *path_vm_for(Alien *al) {
    if (!al) return NULL;
    if (al < g_aliens || al >= (g_aliens + NUMBER_AL)) return NULL;

    int idx = (int)(al - g_aliens);
    PathVmState *vm = &g_path_vm[idx];
    if (vm->owner != al) {
        memset(vm, 0, sizeof(*vm));
        vm->owner = al;
    }
    return vm;
}

static bool path_vm_push(PathVmState *vm, uint16 value) {
    if (!vm || vm->sp >= PATH_VM_STACK_DEPTH) return false;
    vm->data[vm->sp++] = value;
    return true;
}

static bool path_vm_pop(PathVmState *vm, uint16 *out) {
    if (!vm || !out || vm->sp == 0) return false;
    *out = vm->data[--vm->sp];
    return true;
}

// ============================================================
// Sin/Cos (matching strat_common 256-entry angle system)
// ============================================================
// Sin/cos helpers — currently unused, will be needed for P_GOTOPOS etc.
// static float path_sin(uint8 angle) {
//     return sinf((float)angle * (2.0f * 3.14159265f / 256.0f));
// }
// static float path_cos(uint8 angle) {
//     return cosf((float)angle * (2.0f * 3.14159265f / 256.0f));
// }

// ============================================================
// Generate velocity vectors (shared helper)
// ============================================================
static void path_genvecs(Alien *al) {
    if (al->sflags2 & PSFLAG3_HELI) {
        // Helicopter/3D mode: generate 3D vectors
        Strat_GenVecs3D(al);
    } else {
        // Normal 2D mode: generate XZ vectors from roty + vel
        Strat_GenVecs2D(al);
    }
}

static bool path_get_obj_by_ptr(uint16 ptr_index, Alien **out) {
    if (!out) return false;
    if (ptr_index == PATH_NULL_OBJ) return false;
    if (ptr_index >= NUMBER_AL) return false;
    Alien *al = &g_aliens[ptr_index];
    if (!al->active) return false;
    *out = al;
    return true;
}

static uint8 path_pitch_toward(const Alien *src, const Alien *dst) {
    float dy = (float)(dst->worldy - src->worldy);
    float dx = (float)(dst->worldx - src->worldx);
    float dz = (float)(dst->worldz - src->worldz);
    float dist = sqrtf(dx * dx + dz * dz);
    if (dist <= 1.0f) return src->rotx;
    float pitch = atan2f(dy, dist);
    return (uint8)(int)(pitch * (256.0f / (2.0f * 3.14159265f)));
}

static Alien *path_find_near_shape(const Alien *self, uint16 shape_id,
                                   Alien *exclude, int16 max_z, int16 max_xy) {
    if (!self) return NULL;

    uint16 mapped_shape = shape_id;
    if (shape_id < 256) {
        mapped_shape = g_shapes_table[(uint8)shape_id];
    }

    Alien *best = NULL;
    int32 best_metric = 0x7FFFFFFF;
    for (Alien *it = g_active_list; it; it = it->next) {
        if (!it->active || it == self || it == exclude) continue;
        if (!(it->shape == shape_id || it->shape == mapped_shape)) continue;

        int16 dz = (int16)abs(it->worldz - self->worldz);
        if (dz > max_z) continue;
        int16 dx = (int16)abs(it->worldx - self->worldx);
        int16 dy = (int16)abs(it->worldy - self->worldy);
        if (dx > max_xy || dy > max_xy) continue;

        int32 metric = (int32)dx + (int32)dy + (int32)dz;
        if (metric < best_metric) {
            best_metric = metric;
            best = it;
        }
    }
    return best;
}

static Alien *path_fire_weapon(Alien *self, bool can_hit_owner) {
    uint8 speed = self->vel ? self->vel : 48;
    uint8 ap = 2;
    uint8 lifetime = 50;

    switch (self->weapontype) {
    case 0:
        break;
    case 1:
        speed = (uint8)(speed + 8);
        break;
    case 2:
        ap = 4;
        break;
    default:
        speed = (uint8)(speed + 4);
        break;
    }

    Alien *shot = Strat_SpawnProjectile(self,
                                        0, 0, 0,
                                        self->rotx, self->roty,
                                        speed, lifetime, ap,
                                        ACF_COLLTYPE4);
    if (shot && can_hit_owner) {
        shot->immuneptr = 0;
    }
    return shot;
}

static Alien *path_get_player(void) {
    Alien *player = Obj_GetPlayer();
    if (!player || !player->active) return NULL;
    return player;
}

static StrategyFunc path_resolve_strategy(uint16 addr16, uint8 bank) {
    uint32 addr24 = ((uint32)bank << 16) | (uint32)addr16;
    return World_FindStrategyAddress(addr24);
}

static uint8 *path_friend_hp_ptr(uint8 friend_id) {
    switch (friend_id) {
    case FRIEND_RABBIT: return &g_bunny_hp;
    case FRIEND_FALCON: return &g_falcon_hp;
    case FRIEND_FROG: return &g_frog_hp;
    default: return NULL;
    }
}

static void path_fire_at_target(Alien *self, bool can_hit_owner, Alien *target) {
    Alien *shot = path_fire_weapon(self, can_hit_owner);
    if (!shot || !target) return;
    if (target >= g_aliens && target < (g_aliens + NUMBER_AL)) {
        shot->ptr = (uint16)(target - g_aliens);
    }
}

static uint16 path_obj_index_or_null(const Alien *al) {
    if (!al) return PATH_NULL_OBJ;
    if (al < g_aliens || al >= (g_aliens + NUMBER_AL)) return PATH_NULL_OBJ;
    return (uint16)(al - g_aliens);
}

static Alien *path_obj_from_index_raw(uint16 index) {
    if (index >= NUMBER_AL) return NULL;
    return &g_aliens[index];
}

static Alien *path_obj_from_index(uint16 index) {
    Alien *al = path_obj_from_index_raw(index);
    if (!al || !al->active) return NULL;
    return al;
}

static void path_clear_child_link(Alien *child) {
    if (!child) return;
    child->sflags3 &= (uint8)~ASF3_CHILDOBJ;
    child->ptr = PATH_NULL_OBJ;
    child->sword1 = 0;
}

static Alien *path_get_mother_obj(Alien *self) {
    if (!self) return NULL;
    if ((self->sflags3 & ASF3_MOTHEROBJ) != 0) {
        return self;
    }
    if ((self->sflags3 & ASF3_CHILDOBJ) == 0) {
        return NULL;
    }
    Alien *mother = path_obj_from_index(self->ptr);
    if (!mother) {
        path_clear_child_link(self);
        return NULL;
    }
    return mother;
}

static Alien *path_find_child_obj(Alien *mother, uint8 child_num) {
    if (!mother) return NULL;
    uint16 idx = (uint16)mother->sword1;
    int guard = NUMBER_AL + 1;
    while (idx != PATH_NULL_OBJ && guard-- > 0) {
        Alien *child = path_obj_from_index(idx);
        if (!child) break;
        if ((child->sflags3 & ASF3_CHILDOBJ) != 0 &&
            child->ptr == path_obj_index_or_null(mother) &&
            child->sbyte1 == child_num) {
            return child;
        }
        idx = (uint16)child->sword1;
    }
    return NULL;
}

static void path_prune_family_links(Alien *self) {
    if (!self) return;

    if ((self->sflags3 & ASF3_CHILDOBJ) != 0) {
        Alien *mother = path_obj_from_index(self->ptr);
        if (!mother || (mother->sflags3 & ASF3_MOTHEROBJ) == 0) {
            path_clear_child_link(self);
        }
    }

    if ((self->sflags3 & ASF3_MOTHEROBJ) == 0) {
        return;
    }

    uint16 mother_idx = path_obj_index_or_null(self);
    uint16 prev_idx = PATH_NULL_OBJ;
    uint16 idx = (uint16)self->sword1;
    int guard = NUMBER_AL + 1;
    while (idx != PATH_NULL_OBJ && guard-- > 0) {
        Alien *raw = path_obj_from_index_raw(idx);
        if (!raw) break;
        uint16 next_idx = (uint16)raw->sword1;
        bool valid = raw->active &&
                     ((raw->sflags3 & ASF3_CHILDOBJ) != 0) &&
                     (raw->ptr == mother_idx);

        if (!valid) {
            if (prev_idx == PATH_NULL_OBJ) {
                self->sword1 = (int16)next_idx;
            } else {
                Alien *prev = path_obj_from_index_raw(prev_idx);
                if (prev) {
                    prev->sword1 = (int16)next_idx;
                }
            }
            if (raw->ptr == mother_idx) {
                path_clear_child_link(raw);
            }
            idx = next_idx;
            continue;
        }

        prev_idx = idx;
        idx = next_idx;
    }

    if ((uint16)self->sword1 == PATH_NULL_OBJ) {
        self->sflags3 &= (uint8)~ASF3_MOTHEROBJ;
    }
}

static void path_detach_child_from_mother(Alien *child) {
    if (!child || (child->sflags3 & ASF3_CHILDOBJ) == 0) {
        return;
    }
    Alien *mother = path_obj_from_index(child->ptr);
    if (!mother) {
        path_clear_child_link(child);
        return;
    }

    uint16 child_idx = path_obj_index_or_null(child);
    uint16 prev_idx = PATH_NULL_OBJ;
    uint16 idx = (uint16)mother->sword1;
    int guard = NUMBER_AL + 1;
    while (idx != PATH_NULL_OBJ && guard-- > 0) {
        Alien *it = path_obj_from_index_raw(idx);
        if (!it) break;
        uint16 next_idx = (uint16)it->sword1;
        if (idx == child_idx) {
            if (prev_idx == PATH_NULL_OBJ) {
                mother->sword1 = (int16)next_idx;
            } else {
                Alien *prev = path_obj_from_index_raw(prev_idx);
                if (prev) {
                    prev->sword1 = (int16)next_idx;
                }
            }
            break;
        }
        prev_idx = idx;
        idx = next_idx;
    }

    path_clear_child_link(child);
    if ((uint16)mother->sword1 == PATH_NULL_OBJ) {
        mother->sflags3 &= (uint8)~ASF3_MOTHEROBJ;
    }
}

static bool path_attach_child_to_mother(Alien *mother, Alien *child, uint8 child_num) {
    if (!mother || !child || !mother->active || !child->active || mother == child) {
        return false;
    }

    path_detach_child_from_mother(child);
    mother->sflags3 |= ASF3_MOTHEROBJ;
    child->sflags3 |= ASF3_CHILDOBJ;
    child->sbyte1 = child_num;
    child->ptr = path_obj_index_or_null(mother);
    child->sword1 = 0;

    uint16 child_idx = path_obj_index_or_null(child);
    if (child_idx == PATH_NULL_OBJ) {
        path_clear_child_link(child);
        return false;
    }

    uint16 first = (uint16)mother->sword1;
    if (first == PATH_NULL_OBJ) {
        mother->sword1 = (int16)child_idx;
        return true;
    }

    uint16 idx = first;
    int guard = NUMBER_AL + 1;
    while (idx != PATH_NULL_OBJ && guard-- > 0) {
        Alien *it = path_obj_from_index_raw(idx);
        if (!it) break;
        uint16 next_idx = (uint16)it->sword1;
        if (next_idx == PATH_NULL_OBJ) {
            it->sword1 = (int16)child_idx;
            return true;
        }
        idx = next_idx;
    }

    mother->sword1 = (int16)child_idx;
    return true;
}

static void path_remove_all_children(Alien *mother) {
    if (!mother) return;

    uint16 idx = (uint16)mother->sword1;
    mother->sword1 = 0;
    mother->sflags3 &= (uint8)~ASF3_MOTHEROBJ;

    int guard = NUMBER_AL + 1;
    while (idx != PATH_NULL_OBJ && guard-- > 0) {
        Alien *child = path_obj_from_index_raw(idx);
        if (!child) break;
        uint16 next_idx = (uint16)child->sword1;
        path_clear_child_link(child);
        if (child->active) {
            Obj_Free(child);
        }
        idx = next_idx;
    }
}

static bool path_switch_context_to(Alien **self, PathVmState **vm, Alien *target) {
    if (!self || !vm || !target || !target->active) {
        return false;
    }
    PathVmState *next_vm = path_vm_for(target);
    if (!next_vm) return false;
    *self = target;
    *vm = next_vm;
    return true;
}

static bool path_switch_context_by_ptr(Alien **self, PathVmState **vm, uint16 ptr) {
    Alien *target = path_obj_from_index(ptr);
    if (!target) return false;
    return path_switch_context_to(self, vm, target);
}

static void path_add_rotated_offset(Alien *dst, const Alien *src, int8 ox, int8 oy, int8 oz) {
    if (!dst || !src) return;

    float x = (float)ox;
    float y = (float)oy;
    float z = (float)oz;

    float rx = (float)src->rotx * (2.0f * 3.14159265f / 256.0f);
    float ry = (float)src->roty * (2.0f * 3.14159265f / 256.0f);
    float rz = (float)src->rotz * (2.0f * 3.14159265f / 256.0f);

    float cx = cosf(rx), sx = sinf(rx);
    float cy = cosf(ry), sy = sinf(ry);
    float cz = cosf(rz), sz = sinf(rz);

    // X rotation
    float y1 = (y * cx) - (z * sx);
    float z1 = (y * sx) + (z * cx);
    float x1 = x;

    // Y rotation
    float x2 = (x1 * cy) + (z1 * sy);
    float z2 = (-x1 * sy) + (z1 * cy);
    float y2 = y1;

    // Z rotation
    float x3 = (x2 * cz) - (y2 * sz);
    float y3 = (x2 * sz) + (y2 * cz);
    float z3 = z2;

    dst->worldx = (int16)(dst->worldx + (int16)lroundf(x3));
    dst->worldy = (int16)(dst->worldy + (int16)lroundf(y3));
    dst->worldz = (int16)(dst->worldz + (int16)lroundf(z3));
}

static bool path_trigger_condition(Alien *self, uint8 trigger) {
    if (!self) return false;

    switch (trigger) {
    case PATH_TRIGGER_ALWAYS:
        return true;
    case PATH_TRIGGER_2:
    case PATH_TRIGGER_4:
    case PATH_TRIGGER_8:
    case PATH_TRIGGER_16:
    case PATH_TRIGGER_32:
    case PATH_TRIGGER_64:
    case PATH_TRIGGER_128: {
        uint8 shift = trigger;
        uint16 mask = (uint16)((1u << shift) - 1u);
        return (g_gameframe & mask) == 0;
    }
    case PATH_TRIGGER_WHENHIT:
        return (self->sflags3 & ASF3_SFLAG7) != 0;
    case PATH_TRIGGER_WHENHITBYPLAYER:
        return (self->sflags3 & ASF3_SFLAG5) != 0;
    case PATH_TRIGGER_WHENFLAGGED:
        if ((self->sflags4 & ASF4_SFLAG8) != 0) {
            self->sflags4 &= (uint8)~ASF4_SFLAG8;
            return true;
        }
        return false;
    case PATH_TRIGGER_WHENSHAPEDEAD: {
        Alien *obj = NULL;
        return !path_get_obj_by_ptr(self->ptr, &obj);
    }
    case PATH_TRIGGER_WHENDEAD:
        return self->HP == 0;
    default:
        return false;
    }
}

static void path_register_trigger(PathVmState *vm, uint16 addr, uint8 trigger) {
    if (!vm) return;
    if (vm->trigger_count >= PATH_VM_TRIGGER_CAPACITY) return;
    vm->triggers[vm->trigger_count].addr = addr;
    vm->triggers[vm->trigger_count].trigger = trigger;
    vm->trigger_count++;
}

static void path_unregister_trigger(PathVmState *vm, uint16 addr) {
    if (!vm || vm->trigger_count == 0) return;
    for (uint8 i = 0; i < vm->trigger_count; i++) {
        if (vm->triggers[i].addr == addr) {
            for (uint8 j = i + 1; j < vm->trigger_count; j++) {
                vm->triggers[j - 1] = vm->triggers[j];
            }
            vm->trigger_count--;
            return;
        }
    }
}

static void path_run_triggers(Alien *self, PathVmState *vm) {
    if (!self || !vm || vm->in_trigger || vm->trigger_count == 0) {
        return;
    }

    uint16 resume_ip = (uint16)self->sword2;
    uint8 count = vm->trigger_count;
    for (uint8 i = 0; i < count; i++) {
        if (i >= vm->trigger_count) break;
        PathTriggerEntry entry = vm->triggers[i];
        if (!path_trigger_condition(self, entry.trigger)) {
            continue;
        }

        (void)path_vm_push(vm, 0xFFFFu);
        vm->in_trigger = true;
        vm->trigger_force_pending = false;
        self->sword2 = (int16)entry.addr;
        Strat_Path_Tick(self);
        vm->in_trigger = false;

        if (g_aldead) {
            return;
        }

        if (vm->trigger_force_pending) {
            resume_ip = vm->trigger_forced_ip;
        }
        if (vm->sp > 0 && vm->data[vm->sp - 1] == 0xFFFFu) {
            vm->sp--;
        }
        self->sword2 = (int16)resume_ip;
    }
}

static void path_on_collision(Alien *self) {
    if (!self) return;

    self->sflags3 |= ASF3_SFLAG7;
    Alien *other = path_obj_from_index(self->collobjptr);
    if (other &&
        (other->collflags & (ACF_COLLTYPE1 | ACF_COLLTYPE5)) ==
            (ACF_COLLTYPE1 | ACF_COLLTYPE5)) {
        self->sflags3 |= ASF3_SFLAG5;
    }

    Strat_HitFlash(self);
}

static void path_particleexplode_strat(Alien *self) {
    if (!self) return;

    // particleexplode_strat:
    // s_particle_data x,0,0,0
    self->sbyte1 = 0;
    self->sbyte2 = 0;
    self->sbyte3 = 0;

    // s_inc_alvar B,x,al_count
    self->count++;

    // s_jmp_alvarEQ B,x,al_count,#40,remove_Istrat
    if (self->count == 40) {
        g_aldead = 1;
        return;
    }

    // s_add_alvar W,x,al_worldz,#medpspeed-30
    self->worldz = (int16)(self->worldz + (PATH_MED_PSPEED - 30));
}

static void path_particleexplode_istrat(Alien *self) {
    if (!self) return;

    // particleexplode_Istrat
    // s_set_expstrat x,particleexplode_strat
    self->expstratptr = path_particleexplode_strat;
    // s_set_alsflag x,colldisable
    self->sflags |= ASF_COLLDISABLE;
    // s_set_alflag x,exp
    self->flags |= AFEXP;
    // Particle payload for renderer: s_particle_data x,6,60,30
    self->sflags |= ASF_PARTOBJ;
    self->sbyte1 = 6;
    self->sbyte2 = 60;
    self->sbyte3 = 30;
}

static void path_trail_tick(Alien *self) {
    if (!self) return;

    // trail_istrat: fade depth colour every other frame except stop colours.
    if ((self->sbyte1 & 1u) == 0u) {
        if (self->depthoffset != 1 && self->depthoffset != 5 && self->depthoffset != 9) {
            self->depthoffset--;
        }
    }

    // Keep in player-relative Z space.
    self->worldz += g_pviewvelz;

    // Lifetime countdown (starts at 5 on spawn).
    if (self->sbyte1 > 0) {
        self->sbyte1--;
    }
    if (self->sbyte1 == 0) {
        g_aldead = 1;
    }
}

static void path_apply_setv(Alien *self, uint8 op, uint8 dst_off, uint8 src_off) {
    uint8 src_b = 0;
    uint16 src_w = 0;
    switch (op) {
    case P_SETVBB:
        if (AlienCompat_PathRead8(self, src_off, &src_b)) {
            (void)AlienCompat_PathWrite8(self, dst_off, src_b);
        }
        break;
    case P_SETVWW:
        if (AlienCompat_PathRead16(self, src_off, &src_w)) {
            (void)AlienCompat_PathWrite16(self, dst_off, src_w);
        }
        break;
    case P_SETVBW:
        if (AlienCompat_PathRead16(self, src_off, &src_w)) {
            (void)AlienCompat_PathWrite8(self, dst_off, (uint8)(src_w & 0xFFu));
        }
        break;
    case P_SETVWB:
        if (AlienCompat_PathRead8(self, src_off, &src_b)) {
            (void)AlienCompat_PathWrite16(self, dst_off, (uint16)(int16)(int8)src_b);
        }
        break;
    default:
        break;
    }
}

static void path_apply_addv(Alien *self, uint8 op, uint8 dst_off, uint8 src_off) {
    uint8 src_b = 0;
    uint16 src_w = 0;
    switch (op) {
    case P_ADDVBB:
        if (AlienCompat_PathRead8(self, src_off, &src_b)) {
            (void)AlienCompat_PathAdd8(self, dst_off, (int8)src_b);
        }
        break;
    case P_ADDVWW:
        if (AlienCompat_PathRead16(self, src_off, &src_w)) {
            (void)AlienCompat_PathAdd16(self, dst_off, (int16)src_w);
        }
        break;
    case P_ADDVBW:
        if (AlienCompat_PathRead16(self, src_off, &src_w)) {
            (void)AlienCompat_PathAdd8(self, dst_off, (int8)(src_w & 0xFFu));
        }
        break;
    case P_ADDVWB:
        if (AlienCompat_PathRead8(self, src_off, &src_b)) {
            (void)AlienCompat_PathAdd16(self, dst_off, (int16)(int8)src_b);
        }
        break;
    default:
        break;
    }
}

static void path_spawn_text_trail(Alien *self) {
    if (!self) return;
    if ((self->sflags3 & ASF3_TEXTOBJ) == 0) return;
    if (self->ty == 0) return;

    Alien *trail = Obj_Alloc();
    if (!trail) return;

    Strat_InitObjVars(trail);
    trail->HP = 10;
    trail->AP = 8;  // hardAP
    trail->stratptr = path_trail_tick;
    trail->collstratptr = NULL;
    trail->expstratptr = NULL;
    trail->sflags3 |= ASF3_TEXTOBJ;
    trail->sflags |= ASF_COLLDISABLE;
    trail->sbyte1 = 5;

    trail->rotx = self->rotx;
    trail->roty = self->roty;
    trail->rotz = self->rotz;
    trail->worldx = self->worldx;
    trail->worldy = self->worldy;
    trail->worldz = self->worldz;
    trail->coltab = self->coltab;
    trail->tx = self->tx;
    trail->depthoffset = self->ty;
}

static void path_init_common(Alien *self) {
    if (!self) return;

    // s_set_alptrs x,.strat,pathhit_istrat,explode_istrat
    self->stratptr = Strat_Path_Tick;
    self->collstratptr = path_on_collision;
    self->expstratptr = Strat_Explode;

    // s_set_colltype x,ENEMY1
    self->collflags |= ACF_COLLTYPE2;

    // s_set_alsflag x,shadow
    self->sflags |= ASF_SHADOW;

    // s_set_alvar B,x,al_sbyte4,#0 — friend ID = 0
    self->sbyte4 = 0;
}

// ============================================================
// Path init strategies
// Decompiled from PATHS.ASM:54-70
// ============================================================
void Strat_Path_Init(Alien *self) {
    // path_istrat
    path_init_common(self);
}

void Strat_PathDha_Init(Alien *self) {
    if (!self) return;
    // pathdha_istrat: s_set_aldata x,#10,#10 ; bra path_istrat
    self->HP = 10;
    self->AP = 10;
    path_init_common(self);
}

void Strat_PathText_Init(Alien *self) {
    if (!self) return;
    // patht_istrat: colldisable + textobj + HP/AP, then path_istrat.
    self->sflags |= ASF_COLLDISABLE;
    self->sflags3 |= ASF3_TEXTOBJ;
    self->HP = 10;
    self->AP = 8;  // hardAP
    path_init_common(self);
}

// ============================================================
// Path strategy per-frame (PATHS.ASM .strat + .move)
// ============================================================
void Strat_Path_Tick(Alien *self) {
    if (!self || !self->active) return;
    if (!g_path_data || g_path_length == 0) return;

    path_prune_family_links(self);

    PathVmState *vm = path_vm_for(self);
    if (!vm) return;

    uint16 ip = (uint16)self->sword2;
    int max_ops = 50;  // Safety: max opcodes per frame
    bool invert_next_cond = vm->ifnot_pending;

    // --- Opcode dispatch loop ---
    while (max_ops-- > 0) {
        if (ip >= g_path_length) {
            // Past end of data — remove
            g_aldead = 1;
            return;
        }

        uint8 opcode = g_path_data[ip];
        int advance = 1;  // Default: advance 1 byte

        switch (opcode) {

        // ============================================================
        // Flag opcodes (0-4)
        // ============================================================
        case P_RELTOPLAYERON:
            self->sflags2 |= PSFLAG1_RELZ;
            advance = 1;
            break;

        case P_RELTOPLAYEROFF:
            self->sflags2 &= ~PSFLAG1_RELZ;
            advance = 1;
            break;

        case P_ALWAYSGENVECSON:
            self->sflags2 |= PSFLAG2_GENVECS;
            advance = 1;
            break;

        case P_ALWAYSGENVECSOFF:
            self->sflags2 &= ~PSFLAG2_GENVECS;
            advance = 1;
            break;

        // ============================================================
        // Wait (opcode 2): pause N frames
        // ============================================================
        case P_WAIT: {
            uint8 target = path_read8(ip, 1);
            if (self->sbyte3 != target) {
                self->sbyte3++;
                self->sword2 = (int16)ip;
                goto move_phase;  // Don't advance IP, go to move
            }
            self->sbyte3 = 0;
            advance = 2;
            break;
        }

        case P_WAIT1:
            self->sword2 = (int16)(ip + 1);
            goto move_phase;

        // ============================================================
        // Set velocity (opcode 5)
        // ============================================================
        case P_SETVEL: {
            uint8 vel = path_read8(ip, 1);
            self->vel = vel;
            if (!(self->sflags2 & PSFLAG2_GENVECS)) {
                path_genvecs(self);
            }
            advance = 2;
            break;
        }

        // ============================================================
        // Loop (opcode 6): repeat N times
        // ============================================================
        case P_LOOP: {
            uint8 count = path_read8(ip, 1);
            if (self->sbyte2 == count) {
                self->sbyte2 = 0;
                advance = 4;
                break;
            }

            self->sbyte2++;
            ip = path_read16(ip, 2);
            self->sword2 = (int16)ip;
            goto move_phase;

        }

        // ============================================================
        // Stack loops
        // ============================================================
        case P_DO: {
            uint16 loop_count = path_read16(ip, 1);
            (void)path_vm_push(vm, (uint16)(ip + 3));
            (void)path_vm_push(vm, loop_count);
            advance = 3;
            break;
        }

        case P_DOQ: {
            uint16 loop_count = path_read8(ip, 1);
            (void)path_vm_push(vm, (uint16)(ip + 2));
            (void)path_vm_push(vm, loop_count);
            advance = 2;
            break;
        }

        case P_DOALVARB: {
            uint8 offset = path_read8(ip, 1);
            uint8 loop_count = 0;
            (void)AlienCompat_PathRead8(self, offset, &loop_count);
            (void)path_vm_push(vm, (uint16)(ip + 2));
            (void)path_vm_push(vm, loop_count);
            advance = 2;
            break;
        }

        case P_DOALVARW: {
            uint8 offset = path_read8(ip, 1);
            uint16 loop_count = 0;
            (void)AlienCompat_PathRead16(self, offset, &loop_count);
            (void)path_vm_push(vm, (uint16)(ip + 2));
            (void)path_vm_push(vm, loop_count);
            advance = 2;
            break;
        }

        case P_NEXT:
        case P_INEXT: {
            uint16 loop_count = 0;
            uint16 loop_ip = 0;
            if (!path_vm_pop(vm, &loop_count)) {
                advance = 1;
                break;
            }

            loop_count = (uint16)(loop_count - 1u);
            if (loop_count != 0) {
                if (!path_vm_pop(vm, &loop_ip)) {
                    advance = 1;
                    break;
                }

                (void)path_vm_push(vm, loop_ip);
                (void)path_vm_push(vm, loop_count);

                ip = loop_ip;
                self->sword2 = (int16)ip;
                if (opcode == P_NEXT) {
                    goto move_phase;
                }
                continue;
            }

            (void)path_vm_pop(vm, &loop_ip);  // discard stored loop target
            advance = 1;
            break;
        }

        case P_BREAK: {
            uint16 throwaway = 0;
            uint16 target = path_read16(ip, 1);
            (void)path_vm_pop(vm, &throwaway);
            (void)path_vm_pop(vm, &throwaway);
            ip = target;
            self->sword2 = (int16)ip;
            continue;
        }

        case P_BREAKC: {
            uint16 throwaway = 0;
            (void)path_vm_pop(vm, &throwaway);
            (void)path_vm_pop(vm, &throwaway);
            advance = 1;
            break;
        }

        // ============================================================
        // Add byte to alien var (opcode 7)
        // ============================================================
        case P_ADDB: {
            uint8 offset = path_read8(ip, 1);
            int8 value = path_read8s(ip, 2);
            (void)AlienCompat_PathAdd8(self, offset, value);
            advance = 3;
            break;
        }

        // ============================================================
        // Add word to alien var (opcode 8)
        // ============================================================
        case P_ADDW: {
            uint8 offset = path_read8(ip, 1);
            int16 value = path_read16s(ip, 2);
            (void)AlienCompat_PathAdd16(self, offset, value);
            advance = 4;
            break;
        }

        // ============================================================
        // Face player (opcode 9)
        // ============================================================
        case P_FACEPLAYER: {
            Alien *player = Obj_GetPlayer();
            if (player) {
                uint8 target_yaw = Strat_AngleXZ(self, player);
                uint8 target_pitch = path_pitch_toward(self, player);
                self->roty = Strat_Chase8(self->roty, target_yaw, 2);
                self->rotx = Strat_Chase8(self->rotx, target_pitch, 2);
            }
            advance = 1;
            break;
        }

        // ============================================================
        // End / remove (opcodes 15, 21)
        // ============================================================
        case P_END:
            self->type |= ATZREMOVE;
            self->sword2 = (int16)ip;
            goto move_phase;

        case P_REMOVE:
            if ((self->sflags3 & ASF3_CHILDOBJ) != 0) {
                path_detach_child_from_mother(self);
            }
            if ((self->sflags3 & ASF3_MOTHEROBJ) != 0) {
                path_remove_all_children(self);
            }
            g_aldead = 1;
            return;

        case P_REMOVECHILD: {
            uint8 child_num = path_read8(ip, 1);
            Alien *mother = path_get_mother_obj(self);
            if (mother) {
                Alien *child = path_find_child_obj(mother, child_num);
                if (child && child->active) {
                    path_clear_child_link(child);
                    Obj_Free(child);
                }
            }
            advance = 2;
            break;
        }

        case P_EXPLODE: {
            uint8 *hp_ptr = path_friend_hp_ptr(self->sbyte4);
            if (hp_ptr) {
                *hp_ptr = 0;
            }
            if ((self->sflags3 & ASF3_CHILDOBJ) != 0) {
                path_detach_child_from_mother(self);
            }
            if ((self->sflags3 & ASF3_MOTHEROBJ) != 0) {
                path_remove_all_children(self);
            }
            Strat_Explode(self);
            return;
        }

        // ============================================================
        // Set byte/word (opcodes 16, 17)
        // ============================================================
        case P_SETB: {
            uint8 value = path_read8(ip, 1);
            uint8 offset = path_read8(ip, 2);
            (void)AlienCompat_PathWrite8(self, offset, value);
            advance = 3;
            break;
        }

        case P_SETW: {
            int16 value = path_read16s(ip, 1);
            uint8 offset = path_read8(ip, 3);
            (void)AlienCompat_PathWrite16(self, offset, (uint16)value);
            advance = 4;
            break;
        }

        case P_FINDSHAPE: {
            uint16 shape = path_read16(ip, 1);
            Alien *found = path_find_near_shape(self, shape, NULL, 7000, 7000);
            self->ptr = found ? (uint16)(found - g_aliens) : PATH_NULL_OBJ;
            advance = 3;
            break;
        }

        case P_FINDNEXTSHAPE: {
            uint16 shape = path_read16(ip, 1);
            Alien *cur = NULL;
            (void)path_get_obj_by_ptr(self->ptr, &cur);
            Alien *found = path_find_near_shape(self, shape, cur, 3000, 3000);
            self->ptr = found ? (uint16)(found - g_aliens) : PATH_NULL_OBJ;
            advance = 3;
            break;
        }

        case P_FACESHAPE: {
            Alien *obj = NULL;
            if (!path_get_obj_by_ptr(self->ptr, &obj)) {
                advance = 1;
                break;
            }
            uint8 target_yaw = Strat_AngleXZ(self, obj);
            uint8 target_pitch = path_pitch_toward(self, obj);
            self->roty = Strat_Chase8(self->roty, target_yaw, 3);
            self->rotx = Strat_Chase8(self->rotx, target_pitch, 3);
            if (self->roty != target_yaw || self->rotx != target_pitch) {
                self->sword2 = (int16)ip;
                goto move_phase;
            }
            advance = 1;
            break;
        }

        case P_GOTOPOS:
        case P_GOTOSHAPEPOS: {
            int16 off_x = path_read16s(ip, 1);
            int16 off_y = path_read16s(ip, 3);
            int16 off_z = path_read16s(ip, 5);
            uint8 speed = path_read8(ip, 7);

            int16 target_x = off_x;
            int16 target_y = off_y;
            int16 target_z = off_z;

            if (opcode == P_GOTOSHAPEPOS) {
                Alien *obj = NULL;
                if (!path_get_obj_by_ptr(self->ptr, &obj)) {
                    advance = 8;
                    break;
                }
                target_x = (int16)(obj->worldx + off_x);
                target_y = (int16)(obj->worldy + off_y);
                target_z = (int16)(obj->worldz + off_z);
            } else {
                Alien *player = Obj_GetPlayer();
                if (player) {
                    target_x = (int16)(player->worldx + off_x);
                    target_y = (int16)(player->worldy + off_y);
                    target_z = (int16)(player->worldz + off_z);
                }
            }

            if (self->sflags2 & PSFLAG1_RELZ) {
                self->worldz += g_pviewvelz;
            }

            int16 dx = (int16)(target_x - self->worldx);
            int16 dy = (int16)(target_y - self->worldy);
            int16 dz = (int16)(target_z - self->worldz);
            if (abs(dx) <= 4 && abs(dy) <= 4 && abs(dz) <= 4) {
                advance = 8;
                break;
            }

            float fdx = (float)dx;
            float fdy = (float)dy;
            float fdz = (float)dz;
            float yaw_rad = atan2f(fdx, fdz);
            if (yaw_rad < 0.0f) yaw_rad += 2.0f * 3.14159265f;
            uint8 target_yaw = (uint8)(yaw_rad * (256.0f / (2.0f * 3.14159265f)));
            float flat = sqrtf(fdx * fdx + fdz * fdz);
            uint8 target_pitch = self->rotx;
            if (flat > 1.0f) {
                float pitch = atan2f(fdy, flat);
                target_pitch = (uint8)(pitch * (256.0f / (2.0f * 3.14159265f)));
            }

            self->roty = Strat_Chase8(self->roty, target_yaw, 2);
            self->rotx = Strat_Chase8(self->rotx, target_pitch, 2);
            self->vel = speed;
            path_genvecs(self);
            Strat_ApplyVelocity(self);
            self->sword2 = (int16)ip;
            return;
        }

        case P_IMMUNE: {
            Alien *obj = NULL;
            if (path_get_obj_by_ptr(self->ptr, &obj)) {
                if (self >= g_aliens && self < (g_aliens + NUMBER_AL)) {
                    obj->immuneptr = (uint16)(self - g_aliens);
                }
            }
            advance = 1;
            break;
        }

        case P_SHAPEDEAD: {
            uint16 target = path_read16(ip, 1);
            Alien *obj = NULL;
            bool take = !path_get_obj_by_ptr(self->ptr, &obj);
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 3;
            break;
        }

        case P_LINK: {
            uint16 self_idx = path_obj_index_or_null(self);
            if (g_path_link_obj == PATH_NULL_OBJ) {
                g_path_link_obj = self_idx;
            } else {
                Alien *other = path_obj_from_index(g_path_link_obj);
                if (other && other != self) {
                    self->ptr = g_path_link_obj;
                    other->ptr = self_idx;
                }
                g_path_link_obj = PATH_NULL_OBJ;
            }
            advance = 1;
            break;
        }

        case P_INITANIM:
            self->animframe = path_read8(ip, 1);
            advance = 2;
            break;

        case P_ADDANIM:
            self->animframe = (uint8)(self->animframe + path_read8(ip, 2));
            advance = 3;
            break;

        case P_DEBRIS:
            self->debrisshape = path_read16(ip, 1);
            advance = 3;
            break;

        case P_FRIEND: {
            uint8 friend_id = path_read8(ip, 1);
            if (friend_id == FRIEND_ANYONE) {
                uint8 candidates[3];
                int n = 0;
                if (g_falcon_hp > 0) candidates[n++] = FRIEND_FALCON;
                if (g_bunny_hp > 0) candidates[n++] = FRIEND_RABBIT;
                if (g_frog_hp > 0) candidates[n++] = FRIEND_FROG;
                if (n > 0) {
                    self->sbyte4 = candidates[SfRtl_Random() % (uint8)n];
                }
            } else {
                uint8 *hp_ptr = path_friend_hp_ptr(friend_id);
                if (hp_ptr && *hp_ptr > 0) {
                    self->sbyte4 = friend_id;
                }
            }
            advance = 2;
            break;
        }

        case P_SPAWN:
        case P_SPAWNLINK: {
            uint16 shape = path_read16(ip, 1);
            uint16 path_start = path_read16(ip, 3);
            int8 rotx_off = path_read8s(ip, 5);
            int8 roty_off = path_read8s(ip, 6);
            int8 rotz_off = path_read8s(ip, 7);
            uint8 hp = path_read8(ip, 8);
            uint8 ap = path_read8(ip, 9);
            int8 off_x = path_read8s(ip, 10);
            int8 off_y = path_read8s(ip, 11);
            int8 off_z = path_read8s(ip, 12);

            Alien *al = Obj_Alloc();
            if (al) {
                Strat_InitObjVars(al);
                if (shape < 256) {
                    al->shape = g_shapes_table[(uint8)shape];
                } else {
                    al->shape = shape;
                }
                Strat_Path_Init(al);
                al->sword2 = (int16)Paths_ResolveStart(path_start);
                al->worldx = self->worldx;
                al->worldy = self->worldy;
                al->worldz = self->worldz;
                path_add_rotated_offset(al, self, off_x, off_y, off_z);
                al->rotx = (uint8)(self->rotx + rotx_off);
                al->roty = (uint8)(self->roty + roty_off);
                al->rotz = (uint8)(self->rotz + rotz_off);
                al->HP = hp;
                al->AP = ap;
                g_path_become_last_obj = path_obj_index_or_null(al);

                if (opcode == P_SPAWNLINK) {
                    uint16 new_idx = path_obj_index_or_null(al);
                    uint16 self_idx = path_obj_index_or_null(self);
                    self->ptr = new_idx;
                    if (new_idx != PATH_NULL_OBJ) {
                        al->ptr = self_idx;
                    }
                }
            }
            advance = 13;
            break;
        }

        case P_SPAWNCHILD: {
            uint16 shape = path_read16(ip, 1);
            uint16 path_start = path_read16(ip, 3);
            uint8 rotx = path_read8(ip, 5);
            uint8 roty = path_read8(ip, 6);
            uint8 rotz = path_read8(ip, 7);
            uint8 hp = path_read8(ip, 8);
            uint8 ap = path_read8(ip, 9);
            int8 off_x = path_read8s(ip, 10);
            int8 off_y = path_read8s(ip, 11);
            int8 off_z = path_read8s(ip, 12);
            uint8 child_num = path_read8(ip, 13);

            Alien *al = Obj_Alloc();
            if (al) {
                Strat_InitObjVars(al);
                if (shape < 256) {
                    al->shape = g_shapes_table[(uint8)shape];
                } else {
                    al->shape = shape;
                }
                Strat_Path_Init(al);
                al->sword2 = (int16)Paths_ResolveStart(path_start);
                Alien *mother = path_get_mother_obj(self);
                if (!mother) {
                    mother = self;
                }
                (void)path_attach_child_to_mother(mother, al, child_num);
                al->childx = (uint8)off_x;
                al->childy = (uint8)off_y;
                al->childz = (uint8)off_z;
                al->childrotx = rotx;
                al->childroty = roty;
                al->childrotz = rotz;

                al->worldx = mother->worldx;
                al->worldy = mother->worldy;
                al->worldz = mother->worldz;
                path_add_rotated_offset(al, mother, off_x, off_y, off_z);
                al->rotx = (uint8)(mother->rotx + rotx);
                al->roty = (uint8)(mother->roty + roty);
                al->rotz = (uint8)(mother->rotz + rotz);
                al->HP = hp;
                al->AP = ap;
                g_path_become_last_obj = path_obj_index_or_null(al);
            }
            advance = 14;
            break;
        }

        case P_CHILDDEAD: {
            uint8 child_num = path_read8(ip, 1);
            uint16 target = path_read16(ip, 2);
            Alien *mother = path_get_mother_obj(self);
            bool take = true;
            if (mother) {
                take = path_find_child_obj(mother, child_num) == NULL;
            }
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 4;
            break;
        }

        case P_LINKCHILD: {
            uint8 child_num = path_read8(ip, 1);
            Alien *mother = path_get_mother_obj(self);
            Alien *child = mother ? path_find_child_obj(mother, child_num) : NULL;
            self->ptr = child ? path_obj_index_or_null(child) : PATH_NULL_OBJ;
            advance = 2;
            break;
        }

        case P_FLAGCHILD: {
            uint8 child_num = path_read8(ip, 1);
            Alien *mother = path_get_mother_obj(self);
            Alien *child = mother ? path_find_child_obj(mother, child_num) : NULL;
            if (child) {
                child->sflags4 |= ASF4_SFLAG8;
            }
            advance = 2;
            break;
        }

        case P_UNLINKCHILD: {
            uint8 child_num = path_read8(ip, 1);
            Alien *mother = path_get_mother_obj(self);
            Alien *child = mother ? path_find_child_obj(mother, child_num) : NULL;
            if (child) {
                path_detach_child_from_mother(child);
            }
            advance = 2;
            break;
        }

        case P_FLAGMOTHER: {
            Alien *mother = path_get_mother_obj(self);
            if (mother) {
                mother->sflags4 |= ASF4_SFLAG8;
            }
            advance = 1;
            break;
        }

        case P_FLAGSHAPE: {
            Alien *obj = NULL;
            if (path_get_obj_by_ptr(self->ptr, &obj)) {
                obj->sflags4 |= ASF4_SFLAG8;
            }
            advance = 1;
            break;
        }

        case P_UNLINKSELF:
            if ((self->sflags3 & ASF3_CHILDOBJ) != 0) {
                path_detach_child_from_mother(self);
            }
            advance = 1;
            break;

        case P_BECOMECHILD: {
            uint16 caller_ip = ip;
            uint8 child_num = path_read8(ip, 1);
            g_path_become_obj = path_obj_index_or_null(self);
            g_path_become_path = (uint16)self->sword2;

            Alien *mother = path_get_mother_obj(self);
            Alien *child = mother ? path_find_child_obj(mother, child_num) : NULL;
            if (!vm->in_trigger && child && path_switch_context_to(&self, &vm, child)) {
                ip = caller_ip;
                self->sword2 = (int16)ip;
            }
            advance = 2;
            break;
        }

        case P_BECOMESHAPE: {
            uint16 caller_ip = ip;
            g_path_become_obj = path_obj_index_or_null(self);
            g_path_become_path = (uint16)self->sword2;
            if (!vm->in_trigger && path_switch_context_by_ptr(&self, &vm, self->ptr)) {
                ip = caller_ip;
                self->sword2 = (int16)ip;
            }
            advance = 1;
            break;
        }

        case P_BECOMEMOTHER: {
            uint16 caller_ip = ip;
            g_path_become_obj = path_obj_index_or_null(self);
            g_path_become_path = (uint16)self->sword2;
            Alien *mother = path_get_mother_obj(self);
            if (!vm->in_trigger && mother && path_switch_context_to(&self, &vm, mother)) {
                ip = caller_ip;
                self->sword2 = (int16)ip;
            }
            advance = 1;
            break;
        }

        case P_UNBECOME: {
            uint16 caller_ip = ip;
            g_path_become_path = (uint16)self->sword2;
            if (!vm->in_trigger && path_switch_context_by_ptr(&self, &vm, g_path_become_obj)) {
                ip = caller_ip;
                self->sword2 = (int16)ip;
            }
            advance = 1;
            break;
        }

        case P_BECOME: {
            uint16 caller_ip = ip;
            g_path_become_obj = path_obj_index_or_null(self);
            g_path_become_path = (uint16)self->sword2;
            if (!vm->in_trigger && path_switch_context_by_ptr(&self, &vm, g_path_become_last_obj)) {
                ip = caller_ip;
                self->sword2 = (int16)ip;
            }
            advance = 1;
            break;
        }

        case P_TEXT:
            self->sflags3 |= ASF3_TEXTOBJ;
            self->sflags |= ASF_COLLDISABLE;
            self->coltab = path_read16(ip, 1);
            self->depthoffset = (int16)path_read8s(ip, 3);
            self->tx = path_read8(ip, 4);
            advance = 5;
            break;

        case P_TRAIL:
            self->ty = path_read8(ip, 1);
            advance = 2;
            break;

        case P_SCORE:
            g_playerscore = (uint16)(g_playerscore + path_read16(ip, 1));
            advance = 3;
            break;

        case P_SET0B: {
            uint8 offset = path_read8(ip, 1);
            (void)AlienCompat_PathWrite8(self, offset, 0);
            advance = 2;
            break;
        }

        case P_SET0W: {
            uint8 offset = path_read8(ip, 1);
            (void)AlienCompat_PathWrite16(self, offset, 0);
            advance = 2;
            break;
        }

        case P_INCB: {
            uint8 offset = path_read8(ip, 1);
            (void)AlienCompat_PathAdd8(self, offset, 1);
            advance = 2;
            break;
        }

        case P_INCW: {
            uint8 offset = path_read8(ip, 1);
            (void)AlienCompat_PathAdd16(self, offset, 1);
            advance = 2;
            break;
        }

        case P_DECB: {
            uint8 offset = path_read8(ip, 1);
            (void)AlienCompat_PathAdd8(self, offset, -1);
            advance = 2;
            break;
        }

        case P_DECW: {
            uint8 offset = path_read8(ip, 1);
            (void)AlienCompat_PathAdd16(self, offset, -1);
            advance = 2;
            break;
        }

        case P_SETVBB:
        case P_SETVBW:
        case P_SETVWW:
        case P_SETVWB: {
            uint8 dst_off = path_read8(ip, 1);
            uint8 src_off = path_read8(ip, 2);
            path_apply_setv(self, opcode, dst_off, src_off);
            advance = 3;
            break;
        }

        case P_ADDVBB:
        case P_ADDVBW:
        case P_ADDVWW:
        case P_ADDVWB: {
            uint8 dst_off = path_read8(ip, 1);
            uint8 src_off = path_read8(ip, 2);
            path_apply_addv(self, opcode, dst_off, src_off);
            advance = 3;
            break;
        }

        case P_NEGB:
        case P_NEGW: {
            uint16 abs_off = path_read16(ip, 1);
            if (opcode == P_NEGW) {
                uint16 cur_u = 0;
                if (path_abs_read16(self, abs_off, &cur_u)) {
                    int16 cur = (int16)cur_u;
                    cur = (int16)(-cur);
                    (void)path_abs_write16(self, abs_off, (uint16)cur);
                }
            } else {
                uint8 cur_u = 0;
                if (path_abs_read8(self, abs_off, &cur_u)) {
                    int8 cur = (int8)cur_u;
                    cur = (int8)(-cur);
                    (void)path_abs_write8(self, abs_off, (uint8)cur);
                }
            }
            advance = 3;
            break;
        }

        case P_SETRANDOMB:
        case P_SETRANDOMW: {
            uint16 abs_off = path_read16(ip, 1);
            if (opcode == P_SETRANDOMW) {
                uint16 mask = path_read16(ip, 3);
                uint16 rnd = (uint16)((SfRtl_Random() << 8) | SfRtl_Random());
                (void)path_abs_write16(self, abs_off, (uint16)(rnd & mask));
                advance = 5;
            } else {
                uint8 mask = path_read8(ip, 3);
                uint8 rnd = SfRtl_Random();
                (void)path_abs_write8(self, abs_off, (uint8)(rnd & mask));
                advance = 4;
            }
            break;
        }

        case P_IFHITFLAG: {
            uint16 target = path_read16(ip, 1);
            uint8 mask = path_read8(ip, 3);
            bool take = (self->hitflags & mask) != 0;
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                self->hitflags &= (uint8)~mask;
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 4;
            break;
        }

        case P_ADDROTX:
            self->rotx = (uint8)(self->rotx + path_read8s(ip, 1));
            advance = 2;
            break;

        case P_ADDROTY:
            self->roty = (uint8)(self->roty + path_read8s(ip, 1));
            advance = 2;
            break;

        case P_ADDROTZ:
            self->rotz = (uint8)(self->rotz + path_read8s(ip, 1));
            advance = 2;
            break;

        case P_ADDWORLDX:
            self->worldx = (int16)(self->worldx + path_read8s(ip, 1));
            advance = 2;
            break;

        case P_ADDWORLDY:
            self->worldy = (int16)(self->worldy + path_read8s(ip, 1));
            advance = 2;
            break;

        case P_ADDWORLDZ:
            self->worldz = (int16)(self->worldz + path_read8s(ip, 1));
            advance = 2;
            break;

        case P_ADDWS: {
            uint8 offset = path_read8(ip, 1);
            int8 value = path_read8s(ip, 2);
            (void)AlienCompat_PathAdd16(self, offset, (int16)value);
            advance = 3;
            break;
        }

        // ============================================================
        // Chase byte toward target (opcode 11)
        // ============================================================
        case P_ACHASEB: {
            uint8 target = path_read8(ip, 1);
            uint8 offset = path_read8(ip, 2);
            uint8 cur = 0;
            if (AlienCompat_PathRead8(self, offset, &cur)) {
                cur = Strat_Chase8(cur, target, 1);
                (void)AlienCompat_PathWrite8(self, offset, cur);
            }
            advance = 3;
            break;
        }

        // ============================================================
        // Distance check jump (opcode 30)
        // ============================================================
        case P_DISTLESS: {
            int16 dist = path_read16s(ip, 1);
            uint16 target = path_read16(ip, 3);
            Alien *player = path_get_player();
            bool take = false;
            if (player) {
                int16 zdist = abs(self->worldz - player->worldz);
                take = (zdist < dist);
            }
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;  // Dispatch next opcode
            }
            advance = 5;
            break;
        }

        case P_SHAPEDISTLESS: {
            int16 dist = path_read16s(ip, 1);
            uint16 target = path_read16(ip, 3);
            Alien *obj = NULL;
            bool take = false;
            if (path_get_obj_by_ptr(self->ptr, &obj)) {
                int16 zdist = abs(self->worldz - obj->worldz);
                take = (zdist < dist);
            }
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 5;
            break;
        }

        // ============================================================
        // Goto (opcode 32): unconditional jump
        // ============================================================
        case P_GOTO: {
            uint16 target = path_read16(ip, 1);
            ip = target;
            self->sword2 = (int16)ip;
            goto move_phase;
        }

        case P_IGOTO: {
            uint16 target = path_read16(ip, 1);
            ip = target;
            self->sword2 = (int16)ip;
            continue;
        }

        // ============================================================
        // Set acceleration (opcode 34)
        // ============================================================
        case P_SETACCEL: {
            uint8 target_vel = path_read8(ip, 1);
            uint8 accel_rate = path_read8(ip, 2);
            self->count = target_vel;   // Target velocity
            self->count1 = accel_rate;  // Acceleration rate
            advance = 3;
            break;
        }

        case P_HITGROUND: {
            int16 ground_h = path_read16s(ip, 1);
            uint16 target = path_read16(ip, 3);
            int16 sum = (int16)(self->worldy + ground_h);
            bool take = (sum >= 0);
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 5;
            break;
        }

        case P_HITWALL: {
            uint16 target = path_read16(ip, 1);
            bool out_x = (self->worldx < g_minpmoveX) || (self->worldx >= g_maxpmoveX);
            bool out_y = (self->worldy < g_minpmoveY) || (self->worldy >= g_maxpmoveY);
            bool take = out_x || out_y;
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 3;
            break;
        }

        case P_IFLEVEL: {
            uint8 level_1based = path_read8(ip, 1);
            uint16 target = path_read16(ip, 2);
            uint8 level_0based = (uint8)(level_1based - 1u);
            bool take = (g_currentlevel == level_0based);
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 4;
            break;
        }

        case P_LEFTOFPLAYER:
        case P_RIGHTOFPLAYER:
        case P_ABOVEPLAYER:
        case P_BELOWPLAYER:
        case P_BEHINDPLAYER: {
            uint16 target = path_read16(ip, 1);
            Alien *player = path_get_player();
            bool take = false;
            if (player) {
                switch (opcode) {
                case P_LEFTOFPLAYER: take = (player->worldx >= self->worldx); break;
                case P_RIGHTOFPLAYER: take = (player->worldx < self->worldx); break;
                case P_ABOVEPLAYER: take = (player->worldy >= self->worldy); break;
                case P_BELOWPLAYER: take = (player->worldy < self->worldy); break;
                case P_BEHINDPLAYER: take = (player->worldz >= self->worldz); break;
                default: break;
                }
            }
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 3;
            break;
        }

        // ============================================================
        // waitfaceplayer — pause until roughly facing player
        // ============================================================
        case P_WAITFACEPLAYER: {
            Alien *player = Obj_GetPlayer();
            if (player) {
                uint8 target_yaw = Strat_AngleXZ(self, player);
                uint8 target_pitch = path_pitch_toward(self, player);
                self->roty = Strat_Chase8(self->roty, target_yaw, 3);
                self->rotx = Strat_Chase8(self->rotx, target_pitch, 3);
                if (self->roty != target_yaw || self->rotx != target_pitch) {
                    self->sword2 = (int16)ip;
                    goto move_phase;
                }
            }
            advance = 1;
            break;
        }

        // ============================================================
        // 16-bit chase helpers
        // ============================================================
        case P_ACHASEW: {
            int16 target = path_read16s(ip, 1);
            uint8 offset = path_read8(ip, 3);
            uint16 cur_u = 0;
            if (AlienCompat_PathRead16(self, offset, &cur_u)) {
                int16 cur = (int16)cur_u;
                int16 next = Strat_Chase(cur, target, 1);
                (void)AlienCompat_PathWrite16(self, offset, (uint16)next);
            }
            advance = 4;
            break;
        }

        case P_WAITACHASEB: {
            uint8 target = path_read8(ip, 1);
            uint8 offset = path_read8(ip, 2);
            uint8 cur = 0;
            if (AlienCompat_PathRead8(self, offset, &cur)) {
                uint8 next = Strat_Chase8(cur, target, 1);
                (void)AlienCompat_PathWrite8(self, offset, next);
                if (next != target) {
                    self->sword2 = (int16)ip;
                    goto move_phase;
                }
            }
            advance = 3;
            break;
        }

        case P_WAITACHASEW: {
            int16 target = path_read16s(ip, 1);
            uint8 offset = path_read8(ip, 3);
            uint16 cur_u = 0;
            if (AlienCompat_PathRead16(self, offset, &cur_u)) {
                int16 cur = (int16)cur_u;
                int16 next = Strat_Chase(cur, target, 1);
                (void)AlienCompat_PathWrite16(self, offset, (uint16)next);
                if (next != target) {
                    self->sword2 = (int16)ip;
                    goto move_phase;
                }
            }
            advance = 4;
            break;
        }

        case P_SPACESHIPON:
            self->sflags2 |= PSFLAG4_SPACE;
            advance = 1;
            break;

        case P_SPACESHIPOFF:
            self->sflags2 &= (uint8)~PSFLAG4_SPACE;
            advance = 1;
            break;

        case P_HELION:
            self->sflags2 |= PSFLAG3_HELI;
            advance = 1;
            break;

        case P_HELIOFF:
            self->sflags2 &= (uint8)~PSFLAG3_HELI;
            advance = 1;
            break;

        case P_INVISIBLEON:
            self->sflags |= ASF_INVISIBLE;
            self->sflags |= ASF_COLLDISABLE;
            advance = 1;
            break;

        case P_INVISIBLEOFF:
            self->sflags &= (uint8)~ASF_INVISIBLE;
            self->sflags &= (uint8)~ASF_COLLDISABLE;
            advance = 1;
            break;

        case P_SHADOWON:
            self->sflags |= ASF_SHADOW;
            advance = 1;
            break;

        case P_SHADOWOFF:
            self->sflags &= (uint8)~ASF_SHADOW;
            advance = 1;
            break;

        case P_ALWAYS:
            path_register_trigger(vm, path_read16(ip, 1), path_read8(ip, 3));
            advance = 4;
            break;

        case P_ALWAYSOFF:
            path_unregister_trigger(vm, path_read16(ip, 1));
            advance = 3;
            break;

        case P_FORCE:
            if (vm->in_trigger) {
                vm->trigger_force_pending = true;
                vm->trigger_forced_ip = path_read16(ip, 1);
            }
            advance = 3;
            break;

        case P_SPRITE:
            self->sflags3 |= ASF3_SSPRITE;
            self->depthoffset = (int16)path_read8s(ip, 1);
            self->tx = path_read8(ip, 2);
            advance = 3;
            break;

        case P_SETSTRAT:
        {
            uint16 strat_addr = path_read16(ip, 1);
            uint8 strat_bank = path_read8(ip, 3);
            StrategyFunc fn = path_resolve_strategy(strat_addr, strat_bank);
            if (fn) {
                self->stratptr = fn;
                self->sword2 = 0;
                goto move_phase;
            }
            advance = 4;
            break;
        }

        case P_DEBUG:
            advance = 1;
            break;

        case P_START65816:
        {
            uint16 target = 0;
            PathInlineFunc fn = path_find_inline_code(ip);
            if (fn) {
                fn(self);
                if (g_aldead) {
                    return;
                }
            }
            if (path_decode_start65816_return(ip, &target)) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            printf("Path: unsupported start65816 blob at ip=%u\n", (unsigned)ip);
            g_aldead = 1;
            return;
        }

        case P_PARTICLE: {
            Alien *p = Obj_Alloc();
            if (p) {
                Strat_InitObjVars(p);
                p->shape = 0;
                p->stratptr = path_particleexplode_istrat;
                p->collstratptr = NULL;
                p->expstratptr = NULL;
                p->worldx = self->worldx;
                p->worldy = self->worldy;
                p->worldz = self->worldz;
            }
            advance = 1;
            break;
        }

        case P_COLLISIONSON:
            self->sflags &= (uint8)~ASF_COLLDISABLE;
            advance = 1;
            break;

        case P_COLLISIONSOFF:
            self->sflags |= ASF_COLLDISABLE;
            advance = 1;
            break;

        case P_SOUND:
            Strat_TrigSE(path_read8(ip, 1));
            advance = 2;
            break;

        case P_SOUND2:
            self->snd2 = path_read8(ip, 1);
            advance = 2;
            break;

        case P_INVINCIBLEON:
            self->sflags |= ASF_NOHITAFFECT;
            advance = 1;
            break;

        case P_INVINCIBLEOFF:
            self->sflags &= (uint8)~ASF_NOHITAFFECT;
            advance = 1;
            break;

        case P_PLAYERDEAD: {
            uint16 target = path_read16(ip, 1);
            bool take = ((g_pshipflags2 & PSF2_PLAYERHP0) != 0) ||
                        ((g_gameflags & GF_PLAYERDYING) != 0);
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 3;
            break;
        }

        case P_ZREMOVEON:
            self->type |= ATZREMOVE;
            advance = 1;
            break;

        case P_ZREMOVEOFF:
            self->type &= (uint8)~ATZREMOVE;
            advance = 1;
            break;

        case P_SMOKEON:
            self->sflags2 |= PSFLAG6_SMOKE;
            advance = 1;
            break;

        case P_SMOKEOFF:
            self->sflags2 &= (uint8)~PSFLAG6_SMOKE;
            advance = 1;
            break;

        case P_WEAPON: {
            uint8 weapon = path_read8(ip, 1);
            self->weapontype = weapon;
            advance = 2;
            break;
        }

        case P_NOTFRIEND: {
            uint8 friend_id = path_read8(ip, 1);
            uint16 target = path_read16(ip, 2);
            bool take = false;
            if (friend_id == FRIEND_ANYONE) {
                take = (self->sbyte4 == 0);
            } else {
                take = (self->sbyte4 != friend_id);
            }
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 4;
            break;
        }

        case P_IFFLAG: {
            uint16 target = path_read16(ip, 1);
            bool take = (self->sflags4 & ASF4_SFLAG8) != 0;
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                self->sflags4 &= (uint8)~ASF4_SFLAG8;
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 3;
            break;
        }

        case P_IFNOT:
            invert_next_cond = true;
            advance = 1;
            break;

        case P_IFSAMEB: {
            uint8 offset = path_read8(ip, 1);
            uint8 value = path_read8(ip, 2);
            uint16 target = path_read16(ip, 3);
            uint8 cur = 0;
            bool take = AlienCompat_PathRead8(self, offset, &cur) && (cur == value);
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 5;
            break;
        }

        case P_IFSAMEW: {
            uint8 offset = path_read8(ip, 1);
            uint16 value = path_read16(ip, 2);
            uint16 target = path_read16(ip, 4);
            uint16 cur = 0;
            bool take = AlienCompat_PathRead16(self, offset, &cur) && (cur == value);
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 6;
            break;
        }

        case P_IFBETWEENB: {
            uint8 offset = path_read8(ip, 1);
            int8 lo = path_read8s(ip, 2);
            int8 hi = path_read8s(ip, 3);
            uint16 target = path_read16(ip, 4);
            uint8 cur_u = 0;
            int8 cur = 0;
            bool take = AlienCompat_PathRead8(self, offset, &cur_u);
            if (take) {
                cur = (int8)cur_u;
                take = (cur >= lo && cur <= hi);
            }
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 6;
            break;
        }

        case P_IFBETWEENW: {
            uint8 offset = path_read8(ip, 1);
            int16 lo = path_read16s(ip, 2);
            int16 hi = path_read16s(ip, 4);
            uint16 target = path_read16(ip, 6);
            uint16 cur_u = 0;
            int16 cur = 0;
            bool take = AlienCompat_PathRead16(self, offset, &cur_u);
            if (take) {
                cur = (int16)cur_u;
                take = (cur >= lo && cur <= hi);
            }
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 8;
            break;
        }

        case P_IFZEROB:
        case P_IFNOTZEROB: {
            uint8 offset = path_read8(ip, 1);
            uint16 target = path_read16(ip, 2);
            uint8 cur = 0;
            bool take = AlienCompat_PathRead8(self, offset, &cur) &&
                        ((opcode == P_IFZEROB) ? (cur == 0) : (cur != 0));
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 4;
            break;
        }

        case P_IFZEROW:
        case P_IFNOTZEROW: {
            uint8 offset = path_read8(ip, 1);
            uint16 target = path_read16(ip, 2);
            uint16 cur = 0;
            bool take = AlienCompat_PathRead16(self, offset, &cur) &&
                        ((opcode == P_IFZEROW) ? (cur == 0) : (cur != 0));
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 4;
            break;
        }

        case P_MSG: {
            uint8 msg_id = path_read8(ip, 1);
            Strings_SendMessage(msg_id);
            advance = 2;
            break;
        }

        case P_MSG2:
            Strings_SendMessage(self->sbyte1);
            advance = 2;
            break;

        case P_MSGWITHMETER: {
            uint8 msg_id = path_read8(ip, 1);
            g_friends_meter = 0xFFu;
            Strings_SendMessage(msg_id);
            advance = 2;
            break;
        }

        case P_ALMOSTDEAD: {
            uint16 target = path_read16(ip, 1);
            bool take = false;
            uint8 *hp_ptr = path_friend_hp_ptr(self->sbyte4);
            if (hp_ptr && *hp_ptr <= 10u) {
                take = true;
            }
            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 3;
            break;
        }

        case P_DAMAGE: {
            uint8 *hp_ptr = path_friend_hp_ptr(self->sbyte4);
            if (hp_ptr) {
                uint8 hp = *hp_ptr;
                hp = (hp > 10u) ? (uint8)(hp - 10u) : 0u;
                *hp_ptr = hp;
            }
            advance = 1;
            break;
        }

        case P_RANDOMGOTO: {
            uint16 target = path_read16(ip, 1);
            if ((SfRtl_Random() & 1u) != 0) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 3;
            break;
        }

        case P_QSPAWN: {
            uint16 shape = path_read16(ip, 1);
            uint16 path_start = path_read16(ip, 3);
            uint8 hp = path_read8(ip, 5);
            uint8 ap = path_read8(ip, 6);

            Alien *al = Obj_Alloc();
            if (al) {
                Strat_InitObjVars(al);
                if (shape < 256) {
                    al->shape = g_shapes_table[(uint8)shape];
                } else {
                    al->shape = shape;
                }
                Strat_Path_Init(al);
                al->sword2 = (int16)Paths_ResolveStart(path_start);
                al->worldx = self->worldx;
                al->worldy = self->worldy;
                al->worldz = self->worldz;
                al->rotx = self->rotx;
                al->roty = self->roty;
                al->rotz = self->rotz;
                al->HP = hp;
                al->AP = ap;
                g_path_become_last_obj = path_obj_index_or_null(al);
            }
            advance = 7;
            break;
        }

        case P_IMPORTB:
        case P_IMPORTW: {
            uint8 offset = path_read8(ip, 1);
            uint16 addr = path_read16(ip, 2);
            if (opcode == P_IMPORTW) {
                uint16 value = path_read_ext16(addr);
                (void)AlienCompat_PathWrite16(self, offset, value);
            } else {
                uint8 value = path_read_ext8(addr);
                (void)AlienCompat_PathWrite8(self, offset, value);
            }
            advance = 4;
            break;
        }

        case P_EXPORTB:
        case P_EXPORTW: {
            uint8 offset = path_read8(ip, 1);
            uint16 addr = path_read16(ip, 2);
            if (opcode == P_EXPORTW) {
                uint16 value = 0;
                if (AlienCompat_PathRead16(self, offset, &value)) {
                    path_write_ext16(addr, value);
                }
            } else {
                uint8 value = 0;
                if (AlienCompat_PathRead8(self, offset, &value)) {
                    path_write_ext8(addr, value);
                }
            }
            advance = 4;
            break;
        }

        case P_PUSHB:
        case P_PUSHW: {
            uint8 offset = path_read8(ip, 1);
            if (opcode == P_PUSHW) {
                uint16 value = 0;
                if (AlienCompat_PathRead16(self, offset, &value)) {
                    (void)path_vm_push(vm, value);
                }
            } else {
                uint8 value = 0;
                if (AlienCompat_PathRead8(self, offset, &value)) {
                    (void)path_vm_push(vm, value);
                }
            }
            advance = 2;
            break;
        }

        case P_PULLB:
        case P_PULLW: {
            uint8 offset = path_read8(ip, 1);
            uint16 value = 0;
            (void)path_vm_pop(vm, &value);
            if (opcode == P_PULLW) {
                (void)AlienCompat_PathWrite16(self, offset, value);
            } else {
                (void)AlienCompat_PathWrite8(self, offset, (uint8)(value & 0xFFu));
            }
            advance = 2;
            break;
        }

        case P_DIV2B:
        case P_DIV2W: {
            uint16 abs_offset = path_read16(ip, 1);
            if (opcode == P_DIV2W) {
                uint16 value_u = 0;
                if (path_abs_read16(self, abs_offset, &value_u)) {
                    int16 value = (int16)value_u;
                    value = (int16)(value / 2);
                    (void)path_abs_write16(self, abs_offset, (uint16)value);
                }
            } else {
                uint8 value_u = 0;
                if (path_abs_read8(self, abs_offset, &value_u)) {
                    int8 value = (int8)value_u;
                    value = (int8)(value / 2);
                    (void)path_abs_write8(self, abs_offset, (uint8)value);
                }
            }
            advance = 3;
            break;
        }

        case P_INDEXB:
        case P_INDEXW: {
            uint16 table_addr = path_read16(ip, 1);
            (void)path_read8(ip, 3);  // bank byte is ignored in flat-memory runtime
            uint16 index_abs = path_read16(ip, 4);
            uint16 dest_abs = path_read16(ip, 6);

            uint16 index_src = 0;
            if (path_abs_read16(self, index_abs, &index_src)) {
                uint16 idx = (uint16)(index_src & 0x00FFu);
                if (opcode == P_INDEXW) {
                    uint16 value = path_read_ext16((uint16)(table_addr + (idx << 1)));
                    (void)path_abs_write16(self, dest_abs, value);
                } else {
                    uint8 value = path_read_ext8((uint16)(table_addr + idx));
                    (void)path_abs_write8(self, dest_abs, value);
                }
            }
            advance = 8;
            break;
        }

        case P_WITHINRANGE:
        case P_SHAPEINRANGE: {
            int16 radius = path_read16s(ip, 1);
            uint16 target = path_read16(ip, 3);
            Alien *obj = NULL;
            if (opcode == P_SHAPEINRANGE) {
                (void)path_get_obj_by_ptr(self->ptr, &obj);
            } else {
                obj = Obj_GetPlayer();
            }

            bool take = false;
            if (obj) {
                int16 dz = (int16)abs(self->worldz - obj->worldz);
                if (dz <= 200) {
                    int16 dx = (int16)abs(self->worldx - obj->worldx);
                    int16 dy = (int16)abs(self->worldy - obj->worldy);
                    take = (dx <= radius) && (dy <= radius);
                }
            }

            if (invert_next_cond) {
                take = !take;
                invert_next_cond = false;
            }
            if (take) {
                ip = target;
                self->sword2 = (int16)ip;
                continue;
            }
            advance = 5;
            break;
        }

        // ============================================================
        // Fire weapon (opcodes 69, 70)
        // ============================================================
        case P_FIRE:
            path_fire_weapon(self, false);
            advance = 1;
            break;

        case P_FIRECANHIT:
            path_fire_weapon(self, true);
            advance = 1;
            break;

        case P_FIREATPLAYER:
        case P_FIREATPLAYERCANHIT: {
            bool can_hit_owner = (opcode == P_FIREATPLAYERCANHIT);
            path_fire_at_target(self, can_hit_owner, path_get_player());
            advance = 1;
            break;
        }

        case P_FIREATSHAPE:
        case P_FIREATSHAPECANHIT: {
            bool can_hit_owner = (opcode == P_FIREATSHAPECANHIT);
            Alien *target = NULL;
            (void)path_get_obj_by_ptr(self->ptr, &target);
            path_fire_at_target(self, can_hit_owner, target);
            advance = 1;
            break;
        }

        // ============================================================
        // Gosub / Return
        // ============================================================
        case P_GOSUBALVAR: {
            uint8 offset = path_read8(ip, 1);
            uint16 target = 0;
            (void)AlienCompat_PathRead16(self, offset, &target);

            // gosubalvar pushes (ip - 1) because return handler adds 3.
            (void)path_vm_push(vm, (uint16)(ip - 1));
            ip = target;
            self->sword2 = (int16)ip;
            continue;
        }

        case P_GOSUB: {
            (void)path_vm_push(vm, ip);
            uint16 target = path_read16(ip, 1);
            ip = target;
            self->sword2 = (int16)ip;
            continue;
        }

        case P_RETURN: {
            uint16 return_ip = 0xFFFFu;
            if (!path_vm_pop(vm, &return_ip)) {
                return_ip = 0xFFFFu;
            }
            if (return_ip == 0xFFFFu) {
                if (vm->in_trigger) {
                    vm->ifnot_pending = invert_next_cond;
                    return;
                }
                g_aldead = 1;
                return;
            }
            ip = return_ip;
            self->sword2 = (int16)ip;
            advance = 3;
            break;
        }

        default:
            if (opcode >= P_NUM_OPCODES) {
                printf("Path: invalid opcode %u at ip=%u\n",
                       (unsigned)opcode, (unsigned)ip);
                g_aldead = 1;
                return;
            }
            printf("Path: unhandled opcode %u at ip=%u\n",
                   (unsigned)opcode, (unsigned)ip);
            g_aldead = 1;
            return;
        }

        // Advance instruction pointer
        ip += advance;
        self->sword2 = (int16)ip;
    }

move_phase:
    vm->ifnot_pending = invert_next_cond;
    if (vm->in_trigger) {
        return;
    }

    // ============================================================
    // Move phase (PATHS.ASM .move, lines 2816-2898)
    // Applied every frame after opcode dispatch.
    // ============================================================

    // 1. Acceleration: chase vel toward count (target) at count1 (rate)
    if (self->count1 != 0) {
        if (self->vel < self->count) {
            self->vel += self->count1;
            if (self->vel > self->count) {
                self->vel = self->count;
                self->count1 = 0;
            }
        } else if (self->vel > self->count) {
            if (self->vel - self->count < self->count1)
                self->vel = self->count;
            else
                self->vel -= self->count1;
            if (self->vel == self->count)
                self->count1 = 0;
        }
        if (!(self->sflags2 & PSFLAG2_GENVECS)) {
            path_genvecs(self);
        }
    }

    // 2. Space flight coupling (sflag4): roty += rotz / 4
    if (self->sflags2 & PSFLAG4_SPACE) {
        int8 rotz = (int8)self->rotz;
        self->roty += (uint8)(rotz >> 2);
    }

    // 3. Helicopter mode (sflag3): vel = rotx
    if (self->sflags2 & PSFLAG3_HELI) {
        self->vel = self->rotx;
        if (!(self->sflags2 & PSFLAG2_GENVECS)) {
            path_genvecs(self);
        }
    }

    // 4. Z-relative to player (sflag1): add player view velocity
    if (self->sflags2 & PSFLAG1_RELZ) {
        self->worldz += g_pviewvelz;
    }

    // 5. Always regenerate vectors (sflag2)
    if (self->sflags2 & PSFLAG2_GENVECS) {
        path_genvecs(self);
    }

    // 6. Apply velocity to position
    Strat_ApplyVelocity(self);

    // Text objects can emit short-lived trail sprites.
    path_spawn_text_trail(self);

    // Trigger routines run after movement and may execute immediate path code.
    path_run_triggers(self, vm);
    if (g_aldead) {
        return;
    }

    // Mother objects drive child transforms from child-relative offsets.
    if ((self->sflags3 & ASF3_MOTHEROBJ) != 0) {
        path_prune_family_links(self);
        uint16 idx = (uint16)self->sword1;
        int guard = NUMBER_AL + 1;
        while (idx != PATH_NULL_OBJ && guard-- > 0) {
            Alien *child = path_obj_from_index_raw(idx);
            if (!child) break;
            uint16 next_idx = (uint16)child->sword1;

            if (child->active &&
                (child->sflags3 & ASF3_CHILDOBJ) != 0 &&
                child->ptr == path_obj_index_or_null(self)) {
                child->worldx = self->worldx;
                child->worldy = self->worldy;
                child->worldz = self->worldz;
                path_add_rotated_offset(child, self,
                                        (int8)child->childx,
                                        (int8)child->childy,
                                        (int8)child->childz);
                child->rotx = (uint8)(self->rotx + child->childrotx);
                child->roty = (uint8)(self->roty + child->childroty);
                child->rotz = (uint8)(self->rotz + child->childrotz);
            }

            idx = next_idx;
        }
    }

    // Path collision trigger flags are one-frame latches.
    self->sflags3 &= (uint8)~(ASF3_SFLAG5 | ASF3_SFLAG7);
}

// ============================================================
// System init
// ============================================================
void Paths_Init(void) {
    g_path_data = NULL;
    g_path_length = 0;
    g_path_offsets = NULL;
    g_path_count = 0;
    g_path_link_obj = PATH_NULL_OBJ;
    g_path_become_obj = PATH_NULL_OBJ;
    g_path_become_last_obj = PATH_NULL_OBJ;
    g_path_become_path = 0;
    memset(g_path_vm, 0, sizeof(g_path_vm));
    memset(s_path_inline_callbacks, 0, sizeof(s_path_inline_callbacks));
    memset(s_missing_path_warned, 0, sizeof(s_missing_path_warned));
}

void Paths_LoadData(const uint8 *data, uint32 length,
                    const uint16 *offsets, uint16 path_count) {
    g_path_data = data;
    g_path_length = length;
    g_path_offsets = offsets;
    g_path_count = path_count;
}

uint16 Paths_ResolveStart(uint16 path_id) {
    if (g_path_offsets && path_id < g_path_count) {
        uint16 offset = g_path_offsets[path_id];
        if (offset != 0xFFFFu) {
            return offset;
        }
        if (path_id < ARRAY_SIZE(s_missing_path_warned) && !s_missing_path_warned[path_id]) {
            s_missing_path_warned[path_id] = 1u;
            printf("Paths: path id %u is not ported yet; using remove-stub\n",
                   (unsigned)path_id);
        }
        return 0u;
    }
    if (path_id < ARRAY_SIZE(s_missing_path_warned) && !s_missing_path_warned[path_id]) {
        s_missing_path_warned[path_id] = 1u;
        printf("Paths: path id %u is out of range; using remove-stub\n",
               (unsigned)path_id);
    }
    return 0u;
}
