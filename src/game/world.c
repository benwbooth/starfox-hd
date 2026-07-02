// WORLD.ASM → C conversion
// Map script executor — distance-driven object spawning system.
//
// Decompiled from WORLD.ASM by Krister Wombell, Giles Goddard, Dylan Cuthbert.
//
// Core functions:
//   update_objects_l  → World_UpdateObjects()
//   newobjex          → map_exec()  [bytecode dispatcher]
//   mapobjdo          → op_mapobj() [object spawning]
//   mapwait/mapwait2  → op_mapwait()/op_mapwait2()
//   maploopdo         → op_maploop()
//   mapjsrdo/maprtsdo → op_mapjsr()/op_maprts()
//   mapgotodo         → op_mapgoto()
//
// The map executor is a simple bytecode VM:
//   1. Player moves forward → lastzchange = player.worldz - lastplayz
//   2. mapcnt -= lastzchange; if mapcnt < 0 → call newobjex
//   3. newobjex reads opcode byte, dispatches via jump table
//   4. Spawn opcodes allocate aliens and set position/shape/strategy
//   5. mapwait pauses execution by setting mapcnt to a new distance
//   6. mapend stops execution

#include "world.h"
#include "game_vars.h"
#include "obj.h"
#include "alien_compat.h"
#include "sound.h"
#include "windows.h"
#include "bgs.h"
#include "strings.h"
#include "../renderer/shapes.h"
#include "../path/paths.h"
#include "../strat/istrat_shapes.h"
#include "../strat/strat_common.h"
#include "../strat/strat_player.h"
#include "../strat/strat_enemy.h"
#include "../sf_rtl.h"
#include "../variables.h"
#include <math.h>
#include <string.h>
#include <stdio.h>

// ============================================================
// Map executor internal state
// ============================================================

// The current map script being executed
static const uint8 *s_map_data = NULL;
static uint32 s_map_length = 0;

// Player Z tracking (from WORLD.ASM update_objects_l)
int16 g_lastplayz = 0;
int16 g_lastzchange = 0;

// Last spawned object (pointer stored as Alien*, was index on SNES)
// Used by setalvar/setxrot/etc. to modify the most recently created object.
static Alien *s_lastmapobj = NULL;
uint16 g_lastmapobj = 0;  // Exposed for external reference (0 = invalid)

// Special/boss tracking
uint8 g_specialobjtotal = 0;
uint8 g_levelfinished = 0;

// JSR call stack (mapjsrdo/maprtsdo)
static uint16 s_jsr_stack[MAP_JSR_STACK_SIZE];
static int    s_jsr_top = 0;
static int    s_num_jsr = 0;

// Loop tracking (maploopdo)
// Up to MAP_MAX_LOOPS simultaneous loops, each tracked by address + remaining count.
static uint16 s_loop_addrs[MAP_MAX_LOOPS];
static uint16 s_loop_counts[MAP_MAX_LOOPS];
static int    s_num_loops = 0;

// Strategy lookup table — populated during init
// Index → StrategyFunc
#define MAX_STRATS ISTRAT_CAPACITY
StrategyFunc g_istrats[MAX_STRATS];
uint16       g_istrat_shapes[MAX_STRATS];

// Shape lookup table — populated during init
#define MAX_SHAPES 256
uint16 g_shapes_table[MAX_SHAPES];

// Native callback table for mapif/mapcodejsl.
#define MAX_NATIVE_CALLBACKS 64
typedef struct {
    uint32 addr24;
    WorldNativeFunc fn;
} NativeCallback;
static NativeCallback s_native_callbacks[MAX_NATIVE_CALLBACKS];

#define MAX_STRAT_ADDR_MAPS 768
typedef struct {
    uint32 addr24;
    StrategyFunc fn;
} StrategyAddrMap;
static StrategyAddrMap s_strat_addr_maps[MAX_STRAT_ADDR_MAPS];
static uint8 s_missing_strat_warned[MAX_STRATS];
static uint32 s_missing_strat_addr_warned[64];
static uint8 s_missing_strat_addr_warned_count = 0;

#define MAX_INLINE_MAP_FUNCS 64
typedef struct {
    uint16 script_ptr;
    WorldInlineMapFunc fn;
} InlineMapFuncEntry;
static InlineMapFuncEntry s_inline_map_funcs[MAX_INLINE_MAP_FUNCS];

// ============================================================
// Helper: read bytes from map data at a given offset
// ============================================================

static uint8 map_read8(uint16 ptr) {
    if (ptr < s_map_length)
        return s_map_data[ptr];
    return 0;
}

static uint16 map_read16(uint16 ptr) {
    if ((uint32)(ptr + 1) < s_map_length)
        return (uint16)s_map_data[ptr] | ((uint16)s_map_data[ptr + 1] << 8);
    return 0;
}

static int16 map_read16s(uint16 ptr) {
    return (int16)map_read16(ptr);
}

static uint32 ext_addr_wrap(uint32 addr) {
    return addr % WRAM_SIZE;
}

static uint8 world_read_ext8(uint16 addr) {
    return g_ram[ext_addr_wrap(addr)];
}

static uint16 world_read_ext16(uint16 addr) {
    uint32 a0 = ext_addr_wrap(addr);
    uint32 a1 = ext_addr_wrap((uint32)addr + 1);
    return (uint16)g_ram[a0] | ((uint16)g_ram[a1] << 8);
}

static void world_write_ext8(uint16 addr, uint8 value) {
    g_ram[ext_addr_wrap(addr)] = value;
}

static void world_write_ext16(uint16 addr, uint16 value) {
    uint32 a0 = ext_addr_wrap(addr);
    uint32 a1 = ext_addr_wrap((uint32)addr + 1);
    g_ram[a0] = (uint8)(value & 0xFF);
    g_ram[a1] = (uint8)(value >> 8);
}

static void world_warn_missing_strat(uint8 strat_idx) {
    if (s_missing_strat_warned[strat_idx]) {
        return;
    }
    s_missing_strat_warned[strat_idx] = 1u;
    printf("World: strategy id %u is not ported yet; object will stay inert\n",
           (unsigned)strat_idx);
}

static void world_warn_missing_strat_addr(uint32 addr24) {
    for (uint8 i = 0; i < s_missing_strat_addr_warned_count; i++) {
        if (s_missing_strat_addr_warned[i] == addr24) {
            return;
        }
    }
    if (s_missing_strat_addr_warned_count < ARRAY_SIZE(s_missing_strat_addr_warned)) {
        s_missing_strat_addr_warned[s_missing_strat_addr_warned_count++] = addr24;
    }
    printf("World: strategy addr 0x%06X is not ported yet; object will stay inert\n",
           (unsigned)addr24);
}

static void world_set_spacebar_hardvars(Alien *self) {
    if (!self) {
        return;
    }
    self->collflags |= COLLTYPE_ENEMY1;
    self->HP = HARD_HP;
    self->AP = HARD_AP;
}

static void world_spacebar_scroll(Alien *self) {
    if (!self) {
        return;
    }
    self->worldz += g_pviewvelz;
}

static bool world_achase_angle(uint8 *current, uint8 target, int shift) {
    if (!current || *current == target) {
        return true;
    }

    int8 diff = (int8)(*current - target);
    int8 step = diff >> shift;
    if (step == 0) {
        step = (diff > 0) ? 1 : -1;
    }
    *current -= (uint8)step;
    return *current == target;
}

static Alien *world_obj_from_ptr(uint16 ptr) {
    if (ptr == 0u) {
        return NULL;
    }
    return Obj_GetByIndex((int)ptr - 1);
}

static void world_rotate_spacebar_offset(int16 x, int16 y, uint8 rotz,
                                         int16 *out_x, int16 *out_y) {
    float radians = (float)rotz * (2.0f * 3.14159265f / 256.0f);
    float cos_v = cosf(radians);
    float sin_v = sinf(radians);
    float xf = (float)x;
    float yf = (float)y;

    if (out_x) {
        *out_x = (int16)(xf * cos_v - yf * sin_v);
    }
    if (out_y) {
        *out_y = (int16)(xf * sin_v + yf * cos_v);
    }
}

static void world_spacebar_strat(Alien *self) {
    world_spacebar_scroll(self);
}

static void world_spinspacebar_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->rotz = (uint8)(self->rotz + self->sbyte1);
    (void)world_achase_angle(&self->roty, 0u, 3);
    world_spacebar_scroll(self);
}

static void world_spacebar1_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->rotz = (uint8)(self->rotz + self->sbyte1);
    world_spacebar_scroll(self);
}

static void world_spacebar3_strat(Alien *self) {
    Alien *parent;
    int16 rotated_x;
    int16 rotated_y;

    if (!self) {
        return;
    }

    parent = world_obj_from_ptr(self->ptr);
    if (!parent || !parent->active) {
        world_spacebar_scroll(self);
        return;
    }

    world_rotate_spacebar_offset(self->sword1, self->sword2,
                                 (uint8)(0u - parent->rotz),
                                 &rotated_x, &rotated_y);
    self->worldx = (int16)(parent->worldx + rotated_x);
    self->worldy = (int16)(parent->worldy + rotated_y);
    self->worldz = (int16)(parent->worldz + (int16)self->immuneptr);
    self->rotz = (uint8)(self->rotz + parent->sbyte1);
}

static void world_istrat_spacebar_init(Alien *self) {
    world_set_spacebar_hardvars(self);
    if (self) {
        self->stratptr = world_spacebar_strat;
    }
}

static void world_istrat_spinspacebar_init(Alien *self) {
    world_set_spacebar_hardvars(self);
    if (self) {
        self->stratptr = world_spinspacebar_strat;
    }
}

static void world_istrat_spacebar1_init(Alien *self) {
    world_set_spacebar_hardvars(self);
    if (!self) {
        return;
    }

    if (self->sbyte2 != 0u) {
        self->sflags |= ASF_COLLDISABLE;
    }
    self->stratptr = world_spacebar1_strat;
}

static void world_istrat_spacebar3_init(Alien *self) {
    Alien *parent;

    world_set_spacebar_hardvars(self);
    if (!self) {
        return;
    }

    parent = world_obj_from_ptr(self->ptr);
    if (parent && parent->active) {
        self->sword1 = (int16)(self->worldx - parent->worldx);
        self->sword2 = (int16)(self->worldy - parent->worldy);
        self->immuneptr = (uint16)(self->worldz - parent->worldz);
    }
    self->stratptr = world_spacebar3_strat;
}

static void world_register_builtin_istrats(void) {
    g_istrats[MAP_ISTRAT_SPACEBAR] = world_istrat_spacebar_init;
    g_istrats[MAP_ISTRAT_SPINSPACEBAR] = world_istrat_spinspacebar_init;
    g_istrats[MAP_ISTRAT_SPACEBAR1] = world_istrat_spacebar1_init;
    g_istrats[MAP_ISTRAT_SPACEBAR3] = world_istrat_spacebar3_init;
}

// ============================================================
// Forward declarations for opcode handlers
// ============================================================
static void map_exec(void);
static WorldNativeFunc world_find_native_callback(uint32 addr24);
static WorldInlineMapFunc world_find_inline_map_func(uint16 script_ptr);
static void world_register_builtin_callbacks(void);
static void world_register_builtin_istrats(void);
static void world_warn_missing_strat(uint8 strat_idx);
static void world_warn_missing_strat_addr(uint32 addr24);

void World_RegisterNativeCallback(uint32 addr24, WorldNativeFunc fn) {
    for (int i = 0; i < MAX_NATIVE_CALLBACKS; i++) {
        if (s_native_callbacks[i].fn && s_native_callbacks[i].addr24 == addr24) {
            s_native_callbacks[i].fn = fn;
            return;
        }
    }
    for (int i = 0; i < MAX_NATIVE_CALLBACKS; i++) {
        if (!s_native_callbacks[i].fn) {
            s_native_callbacks[i].addr24 = addr24;
            s_native_callbacks[i].fn = fn;
            return;
        }
    }
}

static WorldNativeFunc world_find_native_callback(uint32 addr24) {
    for (int i = 0; i < MAX_NATIVE_CALLBACKS; i++) {
        if (s_native_callbacks[i].fn && s_native_callbacks[i].addr24 == addr24) {
            return s_native_callbacks[i].fn;
        }
    }
    return NULL;
}

void World_RegisterInlineMapCode(uint16 script_ptr, WorldInlineMapFunc fn) {
    for (int i = 0; i < MAX_INLINE_MAP_FUNCS; i++) {
        if (s_inline_map_funcs[i].fn && s_inline_map_funcs[i].script_ptr == script_ptr) {
            s_inline_map_funcs[i].fn = fn;
            return;
        }
    }
    for (int i = 0; i < MAX_INLINE_MAP_FUNCS; i++) {
        if (!s_inline_map_funcs[i].fn) {
            s_inline_map_funcs[i].script_ptr = script_ptr;
            s_inline_map_funcs[i].fn = fn;
            return;
        }
    }
}

static WorldInlineMapFunc world_find_inline_map_func(uint16 script_ptr) {
    for (int i = 0; i < MAX_INLINE_MAP_FUNCS; i++) {
        if (s_inline_map_funcs[i].fn && s_inline_map_funcs[i].script_ptr == script_ptr) {
            return s_inline_map_funcs[i].fn;
        }
    }
    return NULL;
}

void World_RegisterStrategyAddress(uint32 addr24, StrategyFunc fn) {
    for (int i = 0; i < MAX_STRAT_ADDR_MAPS; i++) {
        if (s_strat_addr_maps[i].fn && s_strat_addr_maps[i].addr24 == addr24) {
            s_strat_addr_maps[i].fn = fn;
            return;
        }
    }
    for (int i = 0; i < MAX_STRAT_ADDR_MAPS; i++) {
        if (!s_strat_addr_maps[i].fn) {
            s_strat_addr_maps[i].addr24 = addr24;
            s_strat_addr_maps[i].fn = fn;
            return;
        }
    }
}

StrategyFunc World_FindStrategyAddress(uint32 addr24) {
    for (int i = 0; i < MAX_STRAT_ADDR_MAPS; i++) {
        if (s_strat_addr_maps[i].fn && s_strat_addr_maps[i].addr24 == addr24) {
            return s_strat_addr_maps[i].fn;
        }
    }
    return NULL;
}

static void world_clear_map_objects(bool only_realobj) {
    Alien *al = g_active_list;
    while (al) {
        Alien *next = al->next;
        if ((al->sflags4 & ASF4_PLAYEROBJ) == 0) {
            if (!only_realobj || (al->sflags3 & ASF3_REALOBJ)) {
                Obj_Free(al);
            }
        }
        al = next;
    }
}

// ISTRATS def_shape index for ro_0 (used by DSTRATS kill_robot_l).
// MACRO-counted numbering (matches renderer/shape_data.h catalog ids).
#define WORLD_SHAPE_RO_0 170u

// mapif/mapcodejsl builtins ported from GSTRATS/KSTRATS/WINDOWS/STRATROU.
static bool world_cb_chkstagedone(Alien *last_obj) {
    (void)last_obj;
    if ((g_gameflags & GF_STAGEDONE) == 0) return false;
    g_gameflags &= (uint8)~GF_STAGEDONE;
    return true;
}

static bool world_cb_chkstratdone1(Alien *last_obj) {
    (void)last_obj;
    if ((g_gameflags & GF_STRATDONE1) == 0) return false;
    g_gameflags &= (uint8)~GF_STRATDONE1;
    return true;
}

static bool world_cb_chkstratdone2(Alien *last_obj) {
    (void)last_obj;
    return (g_gameflags & GF_STRATDONE2) != 0;
}

static bool world_cb_chkbossdead(Alien *last_obj) {
    (void)last_obj;
    if ((g_gameflags & GF_BOSSDEAD) == 0) return false;
    g_gameflags &= (uint8)~GF_BOSSDEAD;
    return true;
}

static bool world_cb_theenddead(Alien *last_obj) {
    (void)last_obj;
    return g_numendok == 0xFF;
}

static bool world_cb_initblack_l(Alien *last_obj) {
    (void)last_obj;
    initblack_l();
    return true;
}

static bool world_cb_setcharmapfrommap_l(Alien *last_obj) {
    // MAIN.ASM setcharmapfrommap_l is SNES-charmap transfer work.
    // Flat-memory renderer path has no direct equivalent yet.
    (void)last_obj;
    return true;
}

static bool world_cb_initfadewhite2norm_l(Alien *last_obj) {
    (void)last_obj;
    initfadewhite2norm_l();
    return true;
}

static bool world_cb_kill_robot_l(Alien *last_obj) {
    // DSTRATS.ASM kill_robot_l: find first robot_0 and set sflag8 on it.
    (void)last_obj;
    Alien *al = g_active_list;
    while (al) {
        if (al->shape == WORLD_SHAPE_RO_0) {
            al->sflags4 |= ASF4_SFLAG8;
            break;
        }
        al = al->next;
    }
    return true;
}

static bool world_cb_clearmap_l(Alien *last_obj) {
    (void)last_obj;
    world_clear_map_objects(false);
    return true;
}

static bool world_cb_clearrealobjmap_l(Alien *last_obj) {
    (void)last_obj;
    world_clear_map_objects(true);
    return true;
}

static bool world_cb_setrestart_l(Alien *last_obj) {
    // MAPMACS setrestart stores the current map position as restart state.
    // HD runtime keeps this minimal and marks restart intent on bgflags.
    (void)last_obj;
    g_bgflags |= BGF_RESTART;
    return true;
}

static bool world_cb_markboss_l(Alien *last_obj) {
    // markboss wires end-sequence metadata on SNES. Keep as a no-op marker.
    (void)last_obj;
    return true;
}

static bool world_cb_blocksnd_l(Alien *last_obj) {
    // mapblocksnd macro: trigse $49 — plays the block-impact sound effect.
    (void)last_obj;
    Strat_TrigSE(0x49);
    return true;
}

static bool world_cb_friend_frog_alive(Alien *last_obj) {
    (void)last_obj;
    return g_frog_hp != 0;
}

static bool world_cb_friend_bunny_alive(Alien *last_obj) {
    (void)last_obj;
    return g_bunny_hp != 0;
}

static bool world_cb_friend_cock_alive(Alien *last_obj) {
    (void)last_obj;
    return g_falcon_hp != 0;
}

static uint8 world_friend_meter_from_hp(uint8 hp) {
    // CLfriendmsg on SNES used 10/20/30/40 friend-state buckets.
    if (hp == 0) return 0;
    if (hp >= 40) return 40;
    if (hp >= 30) return 30;
    if (hp >= 20) return 20;
    if (hp >= 10) return 10;
    // HD runtime friend HP is compact (typically 1..3). Map that to buckets.
    if (hp >= 3) return 40;
    if (hp == 2) return 30;
    return 20;
}

static bool world_cb_clfriendmsg_frog(Alien *last_obj) {
    (void)last_obj;
    uint8 meter = world_friend_meter_from_hp(g_frog_hp);
    switch (meter) {
    case 10: Strings_SendMessage(87); break;
    case 20: Strings_SendMessage(84); break;
    case 30: Strings_SendMessage(81); break;
    case 40: Strings_SendMessage(77); break;
    default: break;  // dead or unsupported bucket
    }
    return true;
}

static bool world_cb_clfriendmsg_bunny(Alien *last_obj) {
    (void)last_obj;
    uint8 meter = world_friend_meter_from_hp(g_bunny_hp);
    switch (meter) {
    case 10: Strings_SendMessage(86); break;
    case 20: Strings_SendMessage(83); break;
    case 30: Strings_SendMessage(80); break;
    case 40: Strings_SendMessage(76); break;
    default: break;
    }
    return true;
}

static bool world_cb_clfriendmsg_cock(Alien *last_obj) {
    (void)last_obj;
    uint8 meter = world_friend_meter_from_hp(g_falcon_hp);
    switch (meter) {
    case 10: Strings_SendMessage(85); break;
    case 20: Strings_SendMessage(82); break;
    case 30: Strings_SendMessage(79); break;
    case 40: Strings_SendMessage(75); break;
    default: break;
    }
    return true;
}

static bool world_cb_set_player_exitbase_l(Alien *last_obj) {
    // set_playerExitBase_l (PISTRATS.ASM:621-655) — full port lives in
    // strat_player.c (playerExitBasewait/Go/Follow hangar launch chain).
    (void)last_obj;
    g_game_mode = WATER_MODE;

    // s_set_var W,lastplayZ,#0 — re-zero the map progression counter so the
    // wrapper content after the scramble opening spawns from a fresh origin.
    g_lastplayz = 0;
    // jsl clearmap_l — remove leftover scramble objects.
    world_clear_map_objects(false);
    Strat_PlayerExitBase(Obj_GetPlayer());
    return true;
}

static bool world_cb_set_player_onplanet_l(Alien *last_obj) {
    // Return to normal in-level control.
    (void)last_obj;
    g_pshipflags &= (uint8)~(PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags &= (uint8)~(PSTF_INSEQ | PSTF_NOTDIE);
    g_game_mode = WATER_MODE;
    g_playerflymode |= PFM_SHADOWS;
    return true;
}

static bool world_cb_set_player_cleardemo_l(Alien *last_obj) {
    // Stage-clear flyby: scripted control, no fire, no wobble.
    (void)last_obj;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags |= (PSTF_INSEQ | PSTF_NOTDIE);
    g_playerflymode &= (uint8)~PFM_WOBBLE;
    return true;
}

static bool world_cb_set_player_warp_l(Alien *last_obj) {
    // Bounded literal bridge for PCSTRATS.ASM:set_playerWarp_l.
    // The full player warp strategy remains unported; this callback only
    // applies the setup-side state writes that CL_WARP relies on.
    (void)last_obj;
    g_pstratflags |= (PSTF_NOVDISTC | PSTF_INSEQ);
    g_psvar_word2 = 0;
    g_gameflags |= GF_NOZREMOVE;
    g_gameflags &= (uint8)~GF_VIEWROT;
    g_psvar_word1 = 200;
    g_playerflymode |= PFM_WOBBLE;
    g_pshipflags3 |= PSF3_NOCOLLISIONS;
    return true;
}

static bool world_cb_set_player_clear_earth_l(Alien *last_obj) {
    // Bounded literal bridge for PCSTRATS.ASM:set_playerClearEarth_l.
    // The dedicated player strategy family is still unported; keep the setup
    // side-effects so CL_EARTH can stay literal in map C.
    (void)last_obj;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags |= (PSTF_INSEQ | PSTF_NOTDIE);
    g_playerflymode &= (uint8)~PFM_WOBBLE;
    g_game_mode = SPACE_MODE;
    if (g_splayerflymode == SPFM_INSIDE) {
        g_splayerflymode = SPFM_TONORM;
    }
    g_viewdist = OUTVIEWDIST;
    return true;
}

static bool world_cb_set_player_clear_chase_l(Alien *last_obj) {
    // Bounded literal bridge for PCSTRATS.ASM:set_playerClearCHASE_l.
    // Preserve the setup-time control/psvar writes even before the dedicated
    // chase player strategies are ported.
    (void)last_obj;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags |= (PSTF_NOVDISTC | PSTF_INSEQ | PSTF_NOTDIE);
    g_playerflymode &= (uint8)~PFM_WOBBLE;
    g_gameflags |= GF_NOZREMOVE;
    g_gameflags &= (uint8)~GF_VIEWROT;
    g_psvar_word1 = 300;
    g_psvar_word2 = 0;
    g_psvar_word3 = 218;
    g_psvar_word4 = 5;
    g_minpmoveY = -10000;
    g_game_mode = SPACE_MODE;
    if (g_splayerflymode == SPFM_INSIDE) {
        g_splayerflymode = SPFM_TONORM;
    }
    g_viewdist = OUTVIEWDIST;
    return true;
}

static bool world_cb_set_player_clear_ship2_l(Alien *last_obj) {
    // Bounded literal bridge for PCSTRATS.ASM:set_playerClearShip2_l.
    (void)last_obj;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags |= PSTF_INSEQ;
    g_gameflags |= GF_NOZREMOVE;
    g_game_mode = SPACE_MODE;
    if (g_splayerflymode == SPFM_INSIDE) {
        g_splayerflymode = SPFM_TONORM;
    }
    return true;
}

static bool world_cb_set_player_clear_under_l(Alien *last_obj) {
    // Bounded literal bridge for PCSTRATS.ASM:set_playerClearUNDER_l.
    (void)last_obj;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags |= (PSTF_NOVDISTC | PSTF_INSEQ);
    g_gameflags &= (uint8)~GF_VIEWROT;
    g_minpmoveY = -10000;
    g_game_mode = WATER_MODE;
    return true;
}

static bool world_cb_set_player_dive_l(Alien *last_obj) {
    // Bounded literal bridge for PCSTRATS.ASM:set_playerDIVE_l.
    (void)last_obj;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags |= PSTF_NOVDISTC;
    g_playerflymode |= PFM_WOBBLE;
    g_game_mode = SPACE_MODE;
    if (g_splayerflymode == SPFM_INSIDE) {
        g_splayerflymode = SPFM_TONORM;
    }
    return true;
}

static bool world_cb_set_player_clear_bridge_l(Alien *last_obj) {
    // Bounded literal bridge for PCSTRATS.ASM:set_playerClearbridge_l.
    (void)last_obj;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags |= (PSTF_NOVDISTC | PSTF_INSEQ);
    g_minpmoveY = -10000;
    g_game_mode = SPACE_MODE;
    if (g_splayerflymode == SPFM_INSIDE) {
        g_splayerflymode = SPFM_TONORM;
    }
    return true;
}

static bool world_cb_set_player_clear_turn_l(Alien *last_obj) {
    // Bounded literal bridge for PCSTRATS.ASM:set_playerClearTurn_l.
    (void)last_obj;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags |= (PSTF_NOVDISTC | PSTF_INSEQ);
    g_gameflags &= (uint8)~GF_VIEWROT;
    g_playerflymode |= PFM_WOBBLE;
    g_game_mode = SPACE_MODE;
    if (g_splayerflymode == SPFM_INSIDE) {
        g_splayerflymode = SPFM_TONORM;
    }
    return true;
}

static bool world_cb_set_player_warpout_l(Alien *last_obj) {
    // Bounded literal bridge for PISTRATS.ASM:set_playerWarpOut_l.
    (void)last_obj;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags |= (PSTF_NOVDISTC | PSTF_INSEQ);
    g_game_mode = SPACE_MODE;
    if (g_splayerflymode == SPFM_INSIDE) {
        g_splayerflymode = SPFM_TONORM;
    }
    return true;
}

static bool world_cb_is_player_dead(Alien *last_obj) {
    (void)last_obj;
    return (g_pshipflags2 & PSF2_PLAYERHP0) != 0;
}

static bool world_cb_set_playeroutview_l(Alien *last_obj) {
    (void)last_obj;
    g_pstratflags |= PSTF_INSEQ;
    g_game_mode = SPACE_MODE;
    if (g_splayerflymode == SPFM_INSIDE) {
        g_splayerflymode = SPFM_TONORM;
    }
    g_viewdist = OUTVIEWDIST;
    return true;
}

static bool world_cb_is_levelfinished_zero(Alien *last_obj) {
    (void)last_obj;
    return g_levelfinished == 0;
}

static void world_register_builtin_callbacks(void) {
    World_RegisterNativeCallback(MAP_CB_CHKSTAGEDONE, world_cb_chkstagedone);
    World_RegisterNativeCallback(MAP_CB_CHKSTRATDONE1, world_cb_chkstratdone1);
    World_RegisterNativeCallback(MAP_CB_CHKSTRATDONE2, world_cb_chkstratdone2);
    World_RegisterNativeCallback(MAP_CB_CHKBOSSDEAD, world_cb_chkbossdead);
    World_RegisterNativeCallback(MAP_CB_THEENDDEAD, world_cb_theenddead);

    World_RegisterNativeCallback(MAP_CB_INITBLACK_L, world_cb_initblack_l);
    World_RegisterNativeCallback(MAP_CB_SETCHARMAPFROMMAP_L, world_cb_setcharmapfrommap_l);
    World_RegisterNativeCallback(MAP_CB_INITFADEWHITE2NORM_L, world_cb_initfadewhite2norm_l);
    World_RegisterNativeCallback(MAP_CB_KILL_ROBOT_L, world_cb_kill_robot_l);
    World_RegisterNativeCallback(MAP_CB_CLEARMAP_L, world_cb_clearmap_l);
    World_RegisterNativeCallback(MAP_CB_CLEARREALOBJMAP_L, world_cb_clearrealobjmap_l);
    World_RegisterNativeCallback(MAP_CB_SETRESTART_L, world_cb_setrestart_l);
    World_RegisterNativeCallback(MAP_CB_MARKBOSS_L, world_cb_markboss_l);
    World_RegisterNativeCallback(MAP_CB_BLOCKSND_L, world_cb_blocksnd_l);

    World_RegisterNativeCallback(MAP_CB_FROG_ALIVE, world_cb_friend_frog_alive);
    World_RegisterNativeCallback(MAP_CB_BUNNY_ALIVE, world_cb_friend_bunny_alive);
    World_RegisterNativeCallback(MAP_CB_COCK_ALIVE, world_cb_friend_cock_alive);

    World_RegisterNativeCallback(MAP_CB_CLFRIENDMSG_FROG, world_cb_clfriendmsg_frog);
    World_RegisterNativeCallback(MAP_CB_CLFRIENDMSG_BUNNY, world_cb_clfriendmsg_bunny);
    World_RegisterNativeCallback(MAP_CB_CLFRIENDMSG_COCK, world_cb_clfriendmsg_cock);

    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_EXITBASE_L, world_cb_set_player_exitbase_l);
    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_ONPLANET_L, world_cb_set_player_onplanet_l);
    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_CLEARDEMO_L, world_cb_set_player_cleardemo_l);
    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_WARP_L, world_cb_set_player_warp_l);
    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_CLEAR_EARTH_L,
                                 world_cb_set_player_clear_earth_l);
    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_CLEAR_CHASE_L,
                                 world_cb_set_player_clear_chase_l);
    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_CLEAR_SHIP2_L,
                                 world_cb_set_player_clear_ship2_l);
    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_CLEAR_UNDER_L,
                                 world_cb_set_player_clear_under_l);
    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_DIVE_L,
                                 world_cb_set_player_dive_l);
    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_CLEAR_BRIDGE_L,
                                 world_cb_set_player_clear_bridge_l);
    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_CLEAR_TURN_L,
                                 world_cb_set_player_clear_turn_l);
    World_RegisterNativeCallback(MAP_CB_SET_PLAYER_WARPOUT_L,
                                 world_cb_set_player_warpout_l);
    World_RegisterNativeCallback(MAP_CB_IS_PLAYER_DEAD, world_cb_is_player_dead);
    World_RegisterNativeCallback(MAP_CB_PLAYER_OUTVIEW_L, world_cb_set_playeroutview_l);
    World_RegisterNativeCallback(MAP_CB_LEVELFINISHED_ZERO, world_cb_is_levelfinished_zero);
}

// ============================================================
// World_Init — clear all map executor state
// ============================================================
void World_Init(void) {
    s_map_data = NULL;
    s_map_length = 0;
    g_lastplayz = 0;
    g_lastzchange = 0;
    s_lastmapobj = NULL;
    g_lastmapobj = 0;
    g_specialobjtotal = 0;
    g_levelfinished = 0;
    s_jsr_top = 0;
    s_num_jsr = 0;
    s_num_loops = 0;

    memset(s_loop_addrs, 0, sizeof(s_loop_addrs));
    memset(s_loop_counts, 0, sizeof(s_loop_counts));
    memset(s_jsr_stack, 0, sizeof(s_jsr_stack));
    memset(s_native_callbacks, 0, sizeof(s_native_callbacks));
    memset(s_strat_addr_maps, 0, sizeof(s_strat_addr_maps));
    memset(s_inline_map_funcs, 0, sizeof(s_inline_map_funcs));
    memset(s_missing_strat_warned, 0, sizeof(s_missing_strat_warned));
    memset(s_missing_strat_addr_warned, 0, sizeof(s_missing_strat_addr_warned));
    s_missing_strat_addr_warned_count = 0;
    world_register_builtin_callbacks();

    // Initialize strategy table with NULLs
    memset(g_istrats, 0, sizeof(g_istrats));
    memset(g_istrat_shapes, 0, sizeof(g_istrat_shapes));
    for (int i = 0; i < MAX_SHAPES; i++) {
        g_shapes_table[i] = Shapes_ResolveShapeWord((uint16)i);
    }
    world_register_builtin_istrats();
}

// ============================================================
// World_LoadLevel — set current map script
// ============================================================
void World_LoadLevel(const uint8 *map_data, uint32 map_length) {
    s_map_data = map_data;
    s_map_length = map_length;

    // Reset execution state
    g_mapptr = 0;
    g_mapcnt = 0;
    g_lastplayz = 0;
    g_lastzchange = 0;
    s_lastmapobj = NULL;
    g_lastmapobj = 0;
    g_levelfinished = 0;
    s_jsr_top = 0;
    s_num_jsr = 0;
    s_num_loops = 0;
    world_register_builtin_istrats();
}

// ============================================================
// World_UpdateObjects — decompiled from update_objects_l
// (WORLD.ASM:52-75)
//
// Called once per frame from Obj_RunStrategies().
// Tracks player Z movement and triggers map script execution.
// ============================================================
void World_UpdateObjects(void) {
    // No map loaded — nothing to do
    if (!s_map_data || g_levelfinished) return;

    // Get player's current Z position
    Alien *player = Obj_GetPlayer();
    if (player && player->active) {
        int16 player_z = player->worldz;

        // Calculate Z delta (how far player moved forward this frame)
        // Decompiled: lda al_worldz,x; sec; sbc lastplayz; sta lastzchange
        g_lastzchange = player_z - g_lastplayz;

        // Update last known Z
        // Decompiled: lda al_worldz,x; sta lastplayz
        g_lastplayz = player_z;
    } else {
        // No player (title screen, demos, etc.) — use a fixed scroll speed
        // so mapwait countdowns still progress. MEDPSPEED=65 per frame.
        g_lastzchange = 65;
    }

    // Decrement mapcnt by Z delta
    // Decompiled: lda mapcnt; sec; sbc lastzchange; bmi newobjs_l
    int32 cnt = (int32)(int16)g_mapcnt - (int32)g_lastzchange;
    if (cnt < 0) {
        // mapcnt went negative → trigger map script execution
        map_exec();
    } else {
        // Not time yet — save updated countdown
        g_mapcnt = (uint16)(int16)cnt;
    }
}

// ============================================================
// Spawning helper — common to mapobj, mapqobj, mapdobj, etc.
// Allocates a new alien, inits vars, sets position relative to player Z.
// Returns the alien, or NULL if pool exhausted.
// ============================================================
static Alien *spawn_object(int16 x, int16 y, int16 z_offset) {
    Alien *al = Obj_Alloc();
    if (!al) {
        s_lastmapobj = NULL;
        g_lastmapobj = 0;
        return NULL;
    }

    Strat_InitObjVars(al);

    // Set world position. Z is relative to player position.
    al->worldx = x;
    al->worldy = y;

    Alien *player = Obj_GetPlayer();
    if (player && player->active) {
        al->worldz = player->worldz + z_offset;
    } else {
        al->worldz = z_offset;
    }

    s_lastmapobj = al;
    g_lastmapobj = (uint16)((al - g_aliens) + 1);  // 0 = invalid, else encoded index
    return al;
}

// ============================================================
// Assign strategy to alien by table index
// ============================================================
static void assign_strategy(Alien *al, uint8 strat_idx) {
    if (g_istrats[strat_idx]) {
        al->stratptr = g_istrats[strat_idx];
    } else {
        world_warn_missing_strat(strat_idx);
    }
}

static void assign_strategy_addr(Alien *al, uint16 strat_addr, uint8 strat_bank) {
    uint32 key = ((uint32)strat_bank << 16) | strat_addr;
    StrategyFunc fn = World_FindStrategyAddress(key);
    if (fn) {
        al->stratptr = fn;
    } else {
        world_warn_missing_strat_addr(key);
    }
}

// ============================================================
// Assign shape to alien by table index
// ============================================================
static void assign_shape(Alien *al, uint8 shape_idx) {
    al->shape = Shapes_ResolveShapeWord(g_shapes_table[shape_idx]);
}

// ============================================================
// map_exec — decompiled from newobjex (WORLD.ASM:76-158)
//
// Bytecode dispatcher. Reads opcode from mapdata[mapptr],
// dispatches to handler. Handlers either:
//   - call map_exec() again to continue (for instant opcodes)
//   - return (for wait/pause opcodes: mapwait, mapend)
// ============================================================
static void map_exec(void) {
    // Safety: don't run if no map loaded
    if (!s_map_data || g_mapptr >= s_map_length) return;

    // Prevent infinite loops in a single frame (safety valve)
    int max_ops = 500;

    while (max_ops-- > 0) {
        if (g_mapptr >= s_map_length) return;

        // Read opcode byte (WORLD.ASM:82: lda mapbase&WM,x; and #$ff)
        uint8 opcode = map_read8(g_mapptr);

        // The opcode values are even numbers (0,2,4,...) used as jump table
        // indices on SNES. We dispatch directly on the opcode value.
        switch (opcode) {

        // ============================================================
        // 0: mapobj — Spawn object (standard format)
        // Data: frame(2), x(2), y(2), z(2), shape(1), strat(1)
        // Total: 1 + 10 = 11 bytes
        // ============================================================
        case MAP_OP_MAPOBJ: {
            uint16 frame = map_read16(g_mapptr + 1);
            int16  x     = map_read16s(g_mapptr + 3);
            int16  y     = map_read16s(g_mapptr + 5);
            int16  z     = map_read16s(g_mapptr + 7);
            uint8  shape = map_read8(g_mapptr + 9);
            uint8  strat = map_read8(g_mapptr + 10);

            // Decompiled: lda map_count,x; sta mapcnt
            // The 'frame' field doubles as mapcnt for the next wait.
            g_mapcnt = frame;

            Alien *al = spawn_object(x, y, z);
            if (al) {
                assign_shape(al, shape);
                assign_strategy(al, strat);
            }

            g_mapptr += 11;

            // Decompiled: lda mapcnt; bne .listfull; jmp newobjex
            if (g_mapcnt != 0) return;  // Pause — wait for distance
            continue;  // mapcnt==0 → execute next opcode immediately
        }

        // ============================================================
        // 2: mapend — End of level script
        // ============================================================
        case MAP_OP_END: {
            g_levelfinished = 1;
            return;
        }

        // ============================================================
        // 4: maploop — Loop control
        // Data: addr(2), count(2)
        // Total: 1 + 4 = 5 bytes
        // ============================================================
        case MAP_OP_LOOP: {
            uint16 loop_addr = g_mapptr;  // Current opcode address
            uint16 target    = map_read16(g_mapptr + 1);
            uint16 count     = map_read16(g_mapptr + 3);

            // Search for existing loop at this address
            int found = -1;
            for (int i = 0; i < s_num_loops; i++) {
                if (s_loop_addrs[i] == loop_addr) {
                    found = i;
                    break;
                }
            }

            if (found < 0) {
                // New loop — register it
                if (s_num_loops < MAP_MAX_LOOPS) {
                    found = s_num_loops++;
                    s_loop_addrs[found] = loop_addr;
                    s_loop_counts[found] = count;
                }
                // Jump to loop body
                g_mapptr = target;
                continue;
            } else {
                // Existing loop — decrement count
                s_loop_counts[found]--;
                if (s_loop_counts[found] == 0) {
                    // Loop done — clean up and skip past
                    s_loop_addrs[found] = 0;
                    s_loop_counts[found] = 0;
                    // Compact the loop array
                    for (int i = found; i < s_num_loops - 1; i++) {
                        s_loop_addrs[i] = s_loop_addrs[i + 1];
                        s_loop_counts[i] = s_loop_counts[i + 1];
                    }
                    s_num_loops--;
                    g_mapptr += 5;  // Skip loop opcode
                    continue;
                } else {
                    // Continue loop — jump to target
                    g_mapptr = target;
                    continue;
                }
            }
        }

        // ============================================================
        // 6: debug breakpoint — NOP in HD build
        // 8: mapnop — No operation
        // ============================================================
        case MAP_OP_DEBUG:
            g_mapptr += 1;
            continue;

        case MAP_OP_NOP:
            g_mapptr += 1;
            continue;

        // ============================================================
        // 10: mapmother — Spawn mother ship (generates other aliens)
        // Data: frame(2), x(2), y(2), z(2), shape(2), strat(3), map(2)
        // Total: 1 + 15 = 16 bytes
        // ============================================================
        case MAP_OP_MOTHER: {
            uint16 frame = map_read16(g_mapptr + 1);
            int16  x     = map_read16s(g_mapptr + 3);
            int16  y     = map_read16s(g_mapptr + 5);
            int16  z     = map_read16s(g_mapptr + 7);
            uint16 shape = map_read16(g_mapptr + 9);
            uint16 strat_addr = map_read16(g_mapptr + 11);
            uint8  strat_bank = map_read8(g_mapptr + 13);
            uint16 map_ref = map_read16(g_mapptr + 14);

            g_mapcnt = frame;

            Alien *al = spawn_object(x, y, z);
            if (al) {
                al->shape = Shapes_ResolveShapeWord(shape);
                assign_strategy_addr(al, strat_addr, strat_bank);
                al->ptr = map_ref;       // Store sub-map pointer
                al->type = ATZREMOVE;    // Remove when behind camera
            }

            g_mapptr += 16;
            if (g_mapcnt != 0) return;
            continue;
        }

        // ============================================================
        // 12: mapremove — Remove all aliens matching shape
        // Data: frame(2), shape(2)
        // Total: 1 + 4 = 5 bytes
        // ============================================================
        case MAP_OP_REMOVE: {
            uint16 target_shape = Shapes_ResolveShapeWord(map_read16(g_mapptr + 3));

            // Walk active list, remove matching shapes
            Alien *al = g_active_list;
            while (al) {
                Alien *next = al->next;
                if (al->shape == target_shape) {
                    Obj_Free(al);
                }
                al = next;
            }

            g_mapptr += 5;
            continue;
        }

        // ============================================================
        // 14: setstage — Set stage timer to 50
        // ============================================================
        case MAP_OP_SETSTAGE:
            g_stagecnt = 50;
            g_mapptr += 1;
            continue;

        // ============================================================
        // 16: setbg — Set background
        // Data: bg_index(2)
        // Total: 1 + 2 = 3 bytes
        // ============================================================
        case MAP_OP_SETBG: {
            uint16 bg_index = map_read16(g_mapptr + 1);
            g_currentbg = bg_index;
            g_bgflags |= BGF_BG;
            g_mapptr += 3;
            continue;
        }

        // ============================================================
        // 18: mapwait — Wait for player to move distance forward
        // Data: distance(2)
        // Total: 1 + 2 = 3 bytes
        // Decompiled from WORLD.ASM:974-982 (mapwaitdo)
        // ============================================================
        case MAP_OP_WAIT: {
            uint16 dist = map_read16(g_mapptr + 1);
            if (dist == 0) {
                // Zero wait — skip immediately
                g_mapptr += 3;
                continue;
            }
            g_mapcnt = dist;
            g_mapptr += 3;
            return;  // Pause execution
        }

        // ============================================================
        // 20: setbgm — Set background music
        // Data: music_id(1)
        // Total: 1 + 1 = 2 bytes
        // ============================================================
        case MAP_OP_SETBGM: {
            uint8 music_id = map_read8(g_mapptr + 1);
            Sound_PlayMusic(music_id);
            g_mapptr += 2;
            continue;
        }

        // ============================================================
        // 22-26: Particle control — single byte opcodes
        // ============================================================
        case MAP_OP_NODOTS:
            g_dotsflag = 0;
            g_mapptr += 1;
            continue;

        case MAP_OP_GNDDOTS:
            g_dotsflag = 1;
            g_mapptr += 1;
            continue;

        case MAP_OP_SPACEDUST:
            g_dotsflag = -1;
            g_mapptr += 1;
            continue;

        // ============================================================
        // 28: setothmus — Set other music track
        // Data: music(1)
        // ============================================================
        case MAP_OP_SETOTHMUS: {
            g_othmusic = map_read8(g_mapptr + 1);
            g_mapptr += 2;
            continue;
        }

        // ============================================================
        // 30-36: Offset controls — single byte opcodes
        // ============================================================
        case MAP_OP_VOFSON:
        case MAP_OP_VOFSOFF:
        case MAP_OP_HOFSON:
        case MAP_OP_HOFSOFF:
            g_mapptr += 1;
            continue;

        // ============================================================
        // 38: mapobjzrot — Object with Z rotation
        // Data: frame(2), x(2), y(2), z(2), shape(1), strat(1), zrot(1)
        // Total: 1 + 11 = 12 bytes
        // ============================================================
        case MAP_OP_OBJZROT: {
            uint16 frame = map_read16(g_mapptr + 1);
            int16  x     = map_read16s(g_mapptr + 3);
            int16  y     = map_read16s(g_mapptr + 5);
            int16  z     = map_read16s(g_mapptr + 7);
            uint8  shape = map_read8(g_mapptr + 9);
            uint8  strat = map_read8(g_mapptr + 10);
            uint8  zrot  = map_read8(g_mapptr + 11);

            g_mapcnt = frame;

            Alien *al = spawn_object(x, y, z);
            if (al) {
                assign_shape(al, shape);
                assign_strategy(al, strat);
                al->rotz = zrot;
            }

            g_mapptr += 12;
            if (g_mapcnt != 0) return;
            continue;
        }

        // ============================================================
        // 40: mapjsr — Call subroutine
        // Data: addr(2), bank(1)
        // Total: 1 + 3 = 4 bytes
        // Decompiled from WORLD.ASM:1104-1124
        // ============================================================
        case MAP_OP_JSR: {
            uint16 target = map_read16(g_mapptr + 1);
            // bank byte ignored in HD build (single address space)

            // Push return address onto JSR stack
            if (s_jsr_top < MAP_JSR_STACK_SIZE) {
                s_jsr_stack[s_jsr_top++] = g_mapptr;  // Save current position
                s_num_jsr++;
            }

            g_mapptr = target;
            continue;
        }

        // ============================================================
        // 42: maprts — Return from subroutine
        // Total: 1 byte
        // Decompiled from WORLD.ASM:1128-1145
        // ============================================================
        case MAP_OP_RTS: {
            if (s_jsr_top > 0) {
                uint16 ret_addr = s_jsr_stack[--s_jsr_top];
                s_num_jsr--;
                g_mapptr = ret_addr + 4;  // Skip past the JSR opcode (1+3 bytes)
            } else {
                g_mapptr += 1;  // Shouldn't happen, but don't get stuck
            }
            continue;
        }

        // ============================================================
        // 44: mapif — Conditional branch
        // Data: func_addr(2), func_bank(1), else_addr(2)
        // Total: 1 + 5 = 6 bytes
        // ============================================================
        case MAP_OP_IF: {
            uint16 func_addr = map_read16(g_mapptr + 1);
            uint8 func_bank = map_read8(g_mapptr + 3);
            uint16 else_addr = map_read16(g_mapptr + 4);
            uint32 key = ((uint32)func_bank << 16) | func_addr;
            WorldNativeFunc fn = world_find_native_callback(key);
            bool carry = true;

            if (fn) {
                carry = fn(s_lastmapobj);
            }

            if (!carry) {
                g_mapptr += 6;
                g_mapcnt = 1;
                return;
            }

            g_mapptr = else_addr;
            continue;
        }

        // ============================================================
        // 46: mapgoto — Unconditional jump
        // Data: addr(2), bank(1)
        // Total: 1 + 3 = 4 bytes
        // Decompiled from WORLD.ASM:1025-1033
        // ============================================================
        case MAP_OP_GOTO: {
            uint16 target = map_read16(g_mapptr + 1);
            g_mapptr = target;
            continue;
        }

        // ============================================================
        // 48-52: Set rotation on last spawned object
        // Data: rot(1)
        // Total: 1 + 1 = 2 bytes
        // ============================================================
        case MAP_OP_SETXROT: {
            if (s_lastmapobj) {
                s_lastmapobj->rotx = map_read8(g_mapptr + 1);
            }
            g_mapptr += 2;
            continue;
        }

        case MAP_OP_SETYROT: {
            if (s_lastmapobj) {
                s_lastmapobj->roty = map_read8(g_mapptr + 1);
            }
            g_mapptr += 2;
            continue;
        }

        case MAP_OP_SETZROT: {
            if (s_lastmapobj) {
                s_lastmapobj->rotz = map_read8(g_mapptr + 1);
            }
            g_mapptr += 2;
            continue;
        }

        // ============================================================
        // 54: setalvarb — Set byte in alien struct
        // Data: offset(2), value(1)
        // Total: 1 + 3 = 4 bytes
        // Decompiled from WORLD.ASM:848-862
        //
        // On SNES: address = alien_base + offset, then store byte.
        // In HD: we use struct field offsets.
        // ============================================================
        case MAP_OP_SETALVARB: {
            if (s_lastmapobj) {
                uint16 offset = map_read16(g_mapptr + 1);
                uint8  value  = map_read8(g_mapptr + 3);
                (void)AlienCompat_Write8(s_lastmapobj, offset, false, value);
            }
            g_mapptr += 4;
            continue;
        }

        // ============================================================
        // 56: setalvarw — Set word in alien struct
        // Data: offset(2), value(2)
        // Total: 1 + 4 = 5 bytes
        // ============================================================
        case MAP_OP_SETALVARW: {
            if (s_lastmapobj) {
                uint16 offset = map_read16(g_mapptr + 1);
                uint16 value  = map_read16(g_mapptr + 3);
                (void)AlienCompat_Write16(s_lastmapobj, offset, false, value);
            }
            g_mapptr += 5;
            continue;
        }

        // ============================================================
        // 58: setalvarl — Set 3-byte value in alien struct
        // Data: offset(2), value_lo(2), value_hi(1)
        // Total: 1 + 5 = 6 bytes
        // ============================================================
        case MAP_OP_SETALVARL: {
            if (s_lastmapobj) {
                uint16 offset = map_read16(g_mapptr + 1);
                uint16 lo     = map_read16(g_mapptr + 3);
                uint8  hi     = map_read8(g_mapptr + 5);
                (void)AlienCompat_Write16(s_lastmapobj, offset, false, lo);
                (void)AlienCompat_Write8(s_lastmapobj, (uint16)(offset + 2), false, hi);
            }
            g_mapptr += 6;
            continue;
        }

        // ============================================================
        // 60-64: setalxvar — Set extended alien var (byte/word/long)
        // Same as setalvar but offset is into extended data area
        // In HD build, the Alien struct is flat, so these work the same.
        // ============================================================
        case MAP_OP_SETALXVARB: {
            if (s_lastmapobj) {
                uint16 offset = map_read16(g_mapptr + 1);
                uint8  value  = map_read8(g_mapptr + 3);
                (void)AlienCompat_Write8(s_lastmapobj, offset, true, value);
            }
            g_mapptr += 4;
            continue;
        }

        case MAP_OP_SETALXVARW: {
            if (s_lastmapobj) {
                uint16 offset = map_read16(g_mapptr + 1);
                uint16 value  = map_read16(g_mapptr + 3);
                (void)AlienCompat_Write16(s_lastmapobj, offset, true, value);
            }
            g_mapptr += 5;
            continue;
        }

        case MAP_OP_SETALXVARL: {
            if (s_lastmapobj) {
                uint16 offset = map_read16(g_mapptr + 1);
                uint16 lo     = map_read16(g_mapptr + 3);
                uint8  hi     = map_read8(g_mapptr + 5);
                (void)AlienCompat_Write16(s_lastmapobj, offset, true, lo);
                (void)AlienCompat_Write8(s_lastmapobj, (uint16)(offset + 2), true, hi);
            }
            g_mapptr += 6;
            continue;
        }

        // ============================================================
        // 66-68: Fade up / Fade down
        // Single byte opcodes
        // ============================================================
        case MAP_OP_FADEUP:
            Windows_FadeFromBlack(1);
            g_mapptr += 1;
            continue;

        case MAP_OP_FADEDOWN:
            Windows_FadeToBlack(1);
            g_mapptr += 1;
            continue;

        // ============================================================
        // 70-72: setalvarptrb/w — Set alien var via external pointer
        // Data: offset(2), ptr(2), bank(1)
        // Total: 1 + 5 = 6 bytes
        // Flat-memory port: external pointer is read from g_ram WRAM mirror.
        // ============================================================
        case MAP_OP_SETALVARPB:
        case MAP_OP_SETALVARPW: {
            if (s_lastmapobj) {
                uint16 offset = map_read16(g_mapptr + 1);
                uint16 extptr = map_read16(g_mapptr + 3);
                if (opcode == MAP_OP_SETALVARPB) {
                    (void)AlienCompat_Write8(s_lastmapobj, offset, false, world_read_ext8(extptr));
                } else {
                    (void)AlienCompat_Write16(s_lastmapobj, offset, false, world_read_ext16(extptr));
                }
            }
            g_mapptr += 6;
            continue;
        }

        // ============================================================
        // 74: setvarobj — Store last object pointer to external var
        // Data: ptr(2), bank(1)
        // Total: 1 + 3 = 4 bytes
        // ============================================================
        case MAP_OP_SETVAROBJ: {
            uint16 extptr = map_read16(g_mapptr + 1);
            world_write_ext16(extptr, g_lastmapobj);
            g_mapptr += 4;
            continue;
        }

        // ============================================================
        // 76: mapwaitfade — Wait for fade to complete
        // Total: 1 byte
        // Decompiled from WORLD.ASM:726-740
        // ============================================================
        case MAP_OP_WAITFADE: {
            if (Windows_IsMapFadeActive()) {
                g_mapcnt = 1;
                return;
            }
            g_mapptr += 1;
            continue;
        }

        // ============================================================
        // 78-80: Quick fade up/down
        // ============================================================
        case MAP_OP_QFADEUP:
            Windows_FadeFromBlack(2);
            g_mapptr += 1;
            continue;

        case MAP_OP_QFADEDOWN:
            Windows_FadeToBlack(2);
            g_mapptr += 1;
            continue;

        // ============================================================
        // 82-84: Screen on/off
        // ============================================================
        case MAP_OP_SCREENOFF:
        case MAP_OP_SCREENON:
            g_mapptr += 1;
            continue;

        // ============================================================
        // 86-88: Z rotation enable/disable
        // ============================================================
        case MAP_OP_ZROTOFF:
        case MAP_OP_ZROTON:
            g_mapptr += 1;
            continue;

        // ============================================================
        // 90: mapspecial — Mark last object as special/boss
        // Decompiled from WORLD.ASM:654-663
        // ============================================================
        case MAP_OP_SPECIAL:
            if (s_lastmapobj) {
                s_lastmapobj->sflags4 |= ASF4_SPECIAL;
                g_specialobjtotal++;
            }
            g_mapptr += 1;
            continue;

        // ============================================================
        // 92: setvarb — Set external variable byte
        // Data: value(1), ptr(2), bank(1)
        // Total: 1 + 4 = 5 bytes
        // ============================================================
        case MAP_OP_SETVARB: {
            uint8 value = map_read8(g_mapptr + 1);
            uint16 extptr = map_read16(g_mapptr + 2);
            world_write_ext8(extptr, value);
            g_mapptr += 5;
            continue;
        }

        // ============================================================
        // 94: setvarw — Set external variable word
        // Data: value(2), ptr(2), bank(1)
        // Total: 1 + 5 = 6 bytes
        // ============================================================
        case MAP_OP_SETVARW: {
            uint16 value = map_read16(g_mapptr + 1);
            uint16 extptr = map_read16(g_mapptr + 3);
            world_write_ext16(extptr, value);
            g_mapptr += 6;
            continue;
        }

        // ============================================================
        // 96: setvarl — Set external variable long
        // Data: value(2), high(1), ptr(2), bank(1)
        // Total: 1 + 6 = 7 bytes
        // ============================================================
        case MAP_OP_SETVARL: {
            uint16 extptr = map_read16(g_mapptr + 1);
            uint16 lo = map_read16(g_mapptr + 4);
            uint8 hi = map_read8(g_mapptr + 6);
            world_write_ext16(extptr, lo);
            world_write_ext8((uint16)(extptr + 2), hi);
            g_mapptr += 7;
            continue;
        }

        // ============================================================
        // 98: setbgslow — Set background with transition speed
        // Data: speed(1), bg_index(2)
        // Total: 1 + 3 = 4 bytes
        // ============================================================
        case MAP_OP_SETBGSLOW: {
            g_bgtransspeed = (uint16)map_read8(g_mapptr + 1);
            g_bg_dmalist = map_read16(g_mapptr + 2);
            g_currentbg = g_bg_dmalist;
            g_mapptr += 4;
            continue;
        }

        // ============================================================
        // 100: waitsetbg — Wait for background set to complete
        // Total: 1 byte
        // ============================================================
        case MAP_OP_WAITSETBG:
            if (g_bg_dmalist != 0) {
                return;
            }
            g_mapptr += 1;
            continue;

        // ============================================================
        // 102: setbginfo — Set background info flag
        // Total: 1 byte
        // ============================================================
        case MAP_OP_SETBGINFO:
            g_bgflags |= BGF_INFO;
            g_mapptr += 1;
            continue;

        // ============================================================
        // 104-106: addalvarptrb/w — Add to alien var via pointer
        // Data: offset(2), ptr(2), bank(1)
        // Total: 1 + 5 = 6 bytes
        // ============================================================
        case MAP_OP_ADDALVARPB:
        case MAP_OP_ADDALVARPW: {
            if (s_lastmapobj) {
                uint16 offset = map_read16(g_mapptr + 1);
                uint16 extptr = map_read16(g_mapptr + 3);
                if (opcode == MAP_OP_ADDALVARPB) {
                    (void)AlienCompat_Add8(s_lastmapobj, offset, false, (int8)world_read_ext8(extptr));
                } else {
                    (void)AlienCompat_Add16(s_lastmapobj, offset, false, (int16)world_read_ext16(extptr));
                }
            }
            g_mapptr += 6;
            continue;
        }

        // ============================================================
        // 108-110: Palette fades
        // Total: 1 byte each
        // ============================================================
        case MAP_OP_FADETOSEA:
        case MAP_OP_FADETOGROUND:
            g_mapptr += 1;
            continue;

        // ============================================================
        // 112: mapqobj — Quick object spawn (compact format)
        // Data: frame(1)<<4, x(1)<<2, y(1)<<2, z(1)<<4, shape(1), strat(1)
        // Total: 1 + 6 = 7 bytes (or 6 for qobj2)
        // Decompiled from WORLD.ASM:1343-1465 (mapqobjdo)
        // ============================================================
        case MAP_OP_QOBJ: {
            uint8 frame_raw = map_read8(g_mapptr + 1);
            int8  x_raw     = (int8)map_read8(g_mapptr + 2);
            int8  y_raw     = (int8)map_read8(g_mapptr + 3);
            uint8 z_raw     = map_read8(g_mapptr + 4);
            uint8 shape_idx = map_read8(g_mapptr + 5);
            uint8 strat_idx = map_read8(g_mapptr + 6);

            // Decompress coordinates (from MAPMACS.INC mapqobj format)
            g_mapcnt = (uint16)frame_raw << 4;
            int16 x = (int16)x_raw << 2;
            int16 y = (int16)y_raw << 2;
            int16 z = (int16)((uint16)z_raw << 4);

            Alien *al = spawn_object(x, y, z);
            if (al) {
                assign_shape(al, shape_idx);
                assign_strategy(al, strat_idx);
            }

            g_mapptr += 7;
            if (g_mapcnt != 0) return;
            continue;
        }

        // ============================================================
        // 114: mapobj8 — Object type 8 (compact coordinates)
        // ============================================================
        case MAP_OP_OBJ8: {
            uint8 frame_raw = map_read8(g_mapptr + 1);
            int8  x_raw     = (int8)map_read8(g_mapptr + 2);
            int8  y_raw     = (int8)map_read8(g_mapptr + 3);
            int8  z_raw     = (int8)map_read8(g_mapptr + 4);
            uint16 shape    = map_read16(g_mapptr + 5);
            // strat is 3 bytes (addr+bank) in obj8 format
            uint16 strat_addr = map_read16(g_mapptr + 7);
            uint8  strat_bank = map_read8(g_mapptr + 9);

            g_mapcnt = (uint16)frame_raw << 2;
            int16 x = (int16)x_raw << 2;
            int16 y = (int16)y_raw << 2;
            int16 z = (int16)((int16)z_raw << 4);

            Alien *al = spawn_object(x, y, z);
            if (al) {
                al->shape = Shapes_ResolveShapeWord(shape);
                assign_strategy_addr(al, strat_addr, strat_bank);
            }

            g_mapptr += 10;
            if (g_mapcnt != 0) return;
            continue;
        }

        // ============================================================
        // 116: mapdobj — Double object (shape derived from strategy)
        // Data: frame(2), x(2), y(2), z(2), strat(1)
        // Total: 1 + 9 = 10 bytes
        // ============================================================
        case MAP_OP_DOBJ: {
            uint16 frame = map_read16(g_mapptr + 1);
            int16  x     = map_read16s(g_mapptr + 3);
            int16  y     = map_read16s(g_mapptr + 5);
            int16  z     = map_read16s(g_mapptr + 7);
            uint8  strat = map_read8(g_mapptr + 9);

            g_mapcnt = frame;

            Alien *al = spawn_object(x, y, z);
            if (al) {
                assign_strategy(al, strat);
                // Shape comes from strategy table's associated shape
                al->shape = g_istrat_shapes[strat];
            }

            g_mapptr += 10;
            if (g_mapcnt != 0) return;
            continue;
        }

        // ============================================================
        // 118: mapqobj2 — Quick object variant (shape from strat table)
        // Same compact format as qobj but one byte shorter (no shape byte)
        // Data: frame(1), x(1), y(1), z(1), strat(1)
        // Total: 1 + 5 = 6 bytes
        // ============================================================
        case MAP_OP_QOBJ2: {
            uint8 frame_raw = map_read8(g_mapptr + 1);
            int8  x_raw     = (int8)map_read8(g_mapptr + 2);
            int8  y_raw     = (int8)map_read8(g_mapptr + 3);
            uint8 z_raw     = map_read8(g_mapptr + 4);
            uint8 strat_idx = map_read8(g_mapptr + 5);

            g_mapcnt = (uint16)frame_raw << 4;
            int16 x = (int16)x_raw << 2;
            int16 y = (int16)y_raw << 2;
            int16 z = (int16)((uint16)z_raw << 4);

            Alien *al = spawn_object(x, y, z);
            if (al) {
                assign_strategy(al, strat_idx);
                // Shape derived from strategy's associated shape (istrats+3)
                al->shape = g_istrat_shapes[strat_idx];
            }

            g_mapptr += 6;
            if (g_mapcnt != 0) return;
            continue;
        }

        // ============================================================
        // 120: ctrl65816 — Execute native 65816 code
        // start_65816/end_65816 inline callback.
        // Literal behavior: continue dispatch after callback returns.
        // ============================================================
        case MAP_OP_CODE65816:
        {
            uint16 script_ptr = (uint16)(g_mapptr + 1);
            WorldInlineMapFunc fn = world_find_inline_map_func(script_ptr);
            if (fn) {
                uint16 next_ptr = g_mapptr;
                fn(&next_ptr, s_lastmapobj);
                g_mapptr = next_ptr;
                continue;
            } else {
                printf("World: WARNING — ctrl65816 at mapptr=%u has no inline callback\n", g_mapptr);
            }
            return;
        }

        // ============================================================
        // 122: mapcodejsl — JSL to native code function
        // Data: addr(2), bank(1)
        // Total: 1 + 3 = 4 bytes
        // ============================================================
        case MAP_OP_CODEJSL:
        {
            // mapcode_jsl macro encodes (func-1) so RTL enters at func.
            uint16 func_addr = map_read16(g_mapptr + 1);
            uint8 func_bank = map_read8(g_mapptr + 3);
            uint32 key = ((uint32)func_bank << 16) | (uint16)(func_addr + 1u);
            WorldNativeFunc fn = world_find_native_callback(key);
            if (!fn) {
                // Fallback for HD-generated scripts that may store raw func addr.
                uint32 raw_key = ((uint32)func_bank << 16) | func_addr;
                fn = world_find_native_callback(raw_key);
            }
            if (fn) {
                (void)fn(s_lastmapobj);
            }
            g_mapptr += 4;
            continue;
        }

        // ============================================================
        // 124-128: Conditional jumps based on external variable
        // Data: ptr(2), bank(1), value(1), target(2)
        // Total: 1 + 6 = 7 bytes
        // Decompiled from WORLD.ASM:287-364
        // ============================================================
        case MAP_OP_JMPVARLESS: {
            uint16 extptr = map_read16(g_mapptr + 1);
            uint8 cmpval = map_read8(g_mapptr + 4);
            uint16 target = map_read16(g_mapptr + 5);
            int8 diff = (int8)((uint8)(world_read_ext8(extptr) - cmpval));
            if (diff < 0) {
                g_mapptr = target;
                continue;
            }
            g_mapptr += 7;
            continue;
        }

        case MAP_OP_JMPVARMORE: {
            uint16 extptr = map_read16(g_mapptr + 1);
            uint8 cmpval = map_read8(g_mapptr + 4);
            uint16 target = map_read16(g_mapptr + 5);
            int8 diff = (int8)((uint8)(world_read_ext8(extptr) - cmpval));
            if (diff > 0) {
                g_mapptr = target;
                continue;
            }
            g_mapptr += 7;
            continue;
        }

        case MAP_OP_JMPVAREQ: {
            uint16 extptr = map_read16(g_mapptr + 1);
            uint8 cmpval = map_read8(g_mapptr + 4);
            uint16 target = map_read16(g_mapptr + 5);
            if (world_read_ext8(extptr) == cmpval) {
                g_mapptr = target;
                continue;
            }
            g_mapptr += 7;
            continue;
        }

        // ============================================================
        // 130: sendmessage — Display text message
        // Data: msg_id(1)
        // Total: 1 + 1 = 2 bytes
        // ============================================================
        case MAP_OP_SENDMSG: {
            uint8 msg_id = map_read8(g_mapptr + 1);
            Strings_SendMessage(msg_id);
            g_mapptr += 2;
            continue;
        }

        // ============================================================
        // 132: mapCspecial — Mark object as C-special
        // Decompiled from WORLD.ASM:668-677
        // ============================================================
        case MAP_OP_CSPECIAL:
            if (s_lastmapobj) {
                s_lastmapobj->sflags4 |= ASF4_CSPECIAL;
                g_specialobjtotal++;
            }
            g_mapptr += 1;
            continue;

        // ============================================================
        // 134: mapnormobj — Normal object spawn (full pointers)
        // Data: frame(2), x(2), y(2), z(2), shape(2), strat(3)
        // Total: 1 + 13 = 14 bytes
        // Decompiled from WORLD.ASM:1835-1887
        // ============================================================
        case MAP_OP_NORMOBJ: {
            uint16 frame = map_read16(g_mapptr + 1);
            int16  x     = map_read16s(g_mapptr + 3);
            int16  y     = map_read16s(g_mapptr + 5);
            int16  z     = map_read16s(g_mapptr + 7);
            uint16 shape = map_read16(g_mapptr + 9);
            uint16 strat_addr = map_read16(g_mapptr + 11);
            uint8  strat_bank = map_read8(g_mapptr + 13);

            g_mapcnt = frame;

            Alien *al = spawn_object(x, y, z);
            if (al) {
                al->shape = Shapes_ResolveShapeWord(shape);
                assign_strategy_addr(al, strat_addr, strat_bank);
            }

            g_mapptr += 14;
            if (g_mapcnt != 0) return;
            continue;
        }

        // ============================================================
        // 136: reserved — Not implemented
        // ============================================================
        case MAP_OP_RESERVED:
            g_mapptr += 1;
            continue;

        // ============================================================
        // 138: mapwait2 — Quick wait (byte value << 4)
        // Data: distance_raw(1)
        // Total: 1 + 1 = 2 bytes
        // Decompiled from WORLD.ASM:175-187 (mapwait2do)
        // ============================================================
        case MAP_OP_WAIT2: {
            uint8 raw = map_read8(g_mapptr + 1);
            g_mapcnt = (uint16)raw << 4;
            g_mapptr += 2;
            if (g_mapcnt == 0) continue;
            return;  // Pause execution
        }

        // ============================================================
        // 140: mapsetpath — Set path for last spawned object
        // Data: path_ptr(2)
        // Total: 1 + 2 = 3 bytes
        // Decompiled from WORLD.ASM:162-170
        // ============================================================
        case MAP_OP_SETPATH: {
            if (s_lastmapobj) {
                uint16 path_ptr = map_read16(g_mapptr + 1);
                s_lastmapobj->sword2 = (int16)Paths_ResolveStart(path_ptr);
            }
            g_mapptr += 3;
            continue;
        }

        // ============================================================
        // Unknown/invalid opcode — stop execution
        // ============================================================
        default:
            printf("World: Unknown opcode %u at mapptr=%u\n", opcode, g_mapptr);
            g_mapptr += 1;
            return;
        }
    }

    // Max ops reached — resume next frame
    if (max_ops <= 0) {
        printf("World: Max ops reached at mapptr=%u\n", g_mapptr);
    }
}
