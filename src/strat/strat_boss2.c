// GBSTRATS.ASM boss2 — "SPINNING TOP" (Macbeth spider-top boss).
// Used by MAP1_4 (route 1) and reused as the Venom1 boss by MAP3_5 (route 3).
//
// Literal port of the boss2 block from
// reference/ultrastarfox/SF/STRAT/GBSTRATS.ASM (boss2_Istrat .. boss2spark),
// following the Alien-based porting pattern established by Strat_Boss1 in
// strat_enemy.c.
//
// ASM label -> C function map:
//   boss2_Istrat          -> Strat_Boss2_Init
//   boss2_strat           -> boss2_strat            (6-state machine)
//   boss2exp_Istrat       -> boss2exp_init/_strat   (+ bossbigoutexplodeoff)
//   boss2top_Istrat       -> boss2top_init
//   boss2top_strat        -> boss2top_strat
//   boss2topcol_Istrat    -> boss2topcol_coll
//   boss2topexp_Istrat    -> boss2topexp
//   boss2turret_Istrat    -> boss2turret_init
//   boss2turret_strat     -> boss2turret_strat
//   boss2turretexp_Istrat -> boss2turretexp
//   boss2petal_Istrat     -> boss2petal_init
//   boss2petal_strat      -> boss2petal_strat
//   boss2petalexp_Istrat  -> boss2petalexp_init
//   boss2petalexp_strat   -> boss2petalexp_strat
//   boss2plasma_Istrat    -> boss2plasma_init
//   boss2plasma_strat     -> boss2plasma_strat
//   addyrot2z_srou        -> boss2_addyrot2z
//   s_falldown_Yvec       -> boss2_falldown_yvec
//   makesmoke_srou_l      -> boss2_make_smoke
//   s_boss_dying          -> boss2_boss_dying
// boss2spark_Istrat / boss2spark_srou are dead code in the original
// (boss2spark_srou starts with a plain `rts`) and are not ported.
// boss2rots_srou / doboss2rot_srou are not referenced by any boss2 strategy
// and are not ported.
//
// Registration is called from Strat_RegisterAll after the istrat tables are
// cleared each (re)load.

#include "strat_boss2.h"
#include "strat_common.h"
#include "strat_enemy.h"
#include "../game/game_vars.h"
#include "../game/obj.h"
#include "../game/sound.h"
#include "../game/world.h"
#include "../sf_rtl.h"
#include "../variables.h"
#include <math.h>
#include <stdlib.h>

// ============================================================
// Constants
// ============================================================

// ISTRATS.ASM:532 `def_Istrat boss2,boss_2_2` — literal def_Istrat row count
// (calibrated against IS_BOSS1=69/IS_BOSS7=99/IS_BOSSF=116 in strat_table.c;
// matches the IS_BOSS2 define used by src/map/levels.c lines 9244/9727).
#define IS_BOSS2 108

// boss2_scale equ 3 (GBSTRATS.ASM); world units are (v << 3).
#define BOSS2_SCALE 3
#define B2U(v) ((int16)((v) << BOSS2_SCALE))

// STRATEQU.INC HP/AP constants.
#define BOSS2_AP          10u   // s_set_aldata x,#hardHP,#10
#define BOSS2TOP_HP       64u   // boss2topHP equ 64
#define BOSS2TOP_AP        1u   // boss2topAP equ 1
#define BOSS2TURRET_HP    16u   // boss2turretHP equ 16
#define BOSS2TURRET_AP    16u   // boss2turretAP equ 16
#define BOSS2PETAL_AP      1u   // boss2petalAP equ 1
#define BOSS2PLASMA_AP     8u   // boss2plasma_Istrat s_set_aldata x,#hardHP,#8

// Weapon parameters from GSTRATS.ASM fire_* routines:
//   fire_relfastElaser:      speed 90,  lifecnt 40,  AP enemylaserAP(2)
//   fire_relslowElaserHome:  rel speed, lifecnt 40,  AP enemylaserAP(2)
//   fire_Hmissile2 (BOSSHMISSILE2): speed 60, lifecnt 100, AP hmissile1AP(8)
//   fire_Hplasma:            speed 60,  lifecnt 50,  AP HplasmaAP(10)
#define RELFASTELASER_SPEED     90u
#define RELFASTELASER_LIFE      40u
#define ENEMYLASER_AP            2u
#define BOSSHMISSILE2_SPEED     60u
#define BOSSHMISSILE2_LIFE     100u
#define BOSSHMISSILE2_AP         8u
#define BOSS2_HPLASMA_SPEED     60u
#define BOSS2_HPLASMA_LIFE      50u
#define BOSS2_HPLASMA_AP        10u

// Child object numbers (s_make_childobj in boss2_Istrat).
#define BOSS2_CHILD_TOP      1u
#define BOSS2_CHILD_PETAL0   2u
#define BOSS2_CHILD_PETAL1   3u
#define BOSS2_CHILD_PETAL2   4u
#define BOSS2_CHILD_PETAL3   5u
#define BOSS2_CHILD_TURRET0  7u
#define BOSS2_CHILD_TURRET1  9u
#define BOSS2_CHILD_TURRET2 11u
#define BOSS2_CHILD_TURRET3 13u

// Strategy flags sflag1..sflag4 (STRATEQU.INC make_sflag order).
// Mapped onto sflags2 bits alongside the shared explosion flags
// (ASF2_RELEXPLODE=0x04, ASF2_NOEXPSND=0x08, ASF2_SFLAG1=0x10).
#define BOSS2_SFLAG1 ASF2_SFLAG1  // 0x10 — mother: petals-open + spin-snd gate
#define BOSS2_SFLAG2 0x20u        // mother: top critical -> petals die
                                  // petal:  killed marker (read by plasma)
#define BOSS2_SFLAG3 0x40u        // mother: strafe phase started (state 4)
#define BOSS2_SFLAG4 0x80u        // mother: top missile fire enable

// Shape ids. Only boss_2_2 (the base/top body, compiled shape 70) exists in
// shape_data.h. The part shapes below are not in the compiled set, so they
// proxy through nullshape (same convention as SH_BOSS_2_2_PROXY in levels.c).
// Collision still works: coldet falls back to default extents for shapes
// without vertex data.
#define SH_B2_NULLSHAPE       0u
#define SH_BOSS_2_0_PROXY     259u  // SHAPE_EXT_BOSS_2_0 turret pod
#define SH_BOSS_2_1_PROXY     260u  // SHAPE_EXT_BOSS_2_1 spinning top core
#define SH_BOSS_2_3_PROXY     261u  // SHAPE_EXT_BOSS_2_3 petal (fully open)
#define SH_BOSS_2_4_PROXY     262u  // SHAPE_EXT_BOSS_2_4 petal (half open)
#define SH_BOSS_2_5_PROXY     263u  // SHAPE_EXT_BOSS_2_5 petal (closed)
#define SH_ROCKBEAM_PROXY     272u  // SHAPE_EXT_ROCKBEAM plasma orb
#define SH_L2SMOKE_PROXY      273u  // SHAPE_EXT_L2SMOKE smoke

// boss2petal_tab (GBSTRATS.ASM): dw boss_2_5, boss_2_4, boss_2_3
static const uint16 boss2petal_tab[3] = {
    SH_BOSS_2_5_PROXY, SH_BOSS_2_4_PROXY, SH_BOSS_2_3_PROXY,
};

// Sized explosion markers shared with strat_enemy.c make_*_exp_obj.
#define B2_EXPSHAPE_MEDIUM  2u
#define B2_EXPSHAPE_LARGE   3u
#define B2_EXPSHAPE_FOLARGE 4u

// Sound ids (trigse arguments in the ASM).
#define B2_SE_SPAWN     0x95u
#define B2_SE_SPIN      0x71u
#define B2_SE_JUMP      0x85u
#define B2_SE_LAND      0x8Eu
#define B2_SE_BOSSDYING 0x1Eu  // s_boss_dying (matches strat_enemy.c)
#define B2_BGM_BOSSDYING 0xF1u

// ============================================================
// Forward declarations
// ============================================================
static void boss2_strat(Alien *self);
static void boss2exp_init(Alien *self);
static void boss2exp_strat(Alien *self);
static void boss2top_init(Alien *self);
static void boss2top_strat(Alien *self);
static void boss2topcol_coll(Alien *self);
static void boss2topexp(Alien *self);
static void boss2turret_init(Alien *self);
static void boss2turret_strat(Alien *self);
static void boss2turretexp(Alien *self);
static void boss2petal_init(Alien *self);
static void boss2petal_strat(Alien *self);
static void boss2petalexp_init(Alien *self);
static void boss2petalexp_strat(Alien *self);
static void boss2plasma_init(Alien *plasma, Alien *petal);
static void boss2plasma_strat(Alien *self);
static void boss2_delayexplode_strat(Alien *self);
static void boss2_delayremove_strat(Alien *self);
static void boss2_particle_strat(Alien *self);
static void boss2_smoke_strat(Alien *self);
static void boss2_homelaser_strat(Alien *self);
static void boss2_hmissile2_strat(Alien *self);
static void boss2_hplasma_strat(Alien *self);

// ============================================================
// Small shared helpers (local copies of statics in strat_enemy.c —
// those symbols are file-local there and cannot be linked against).
// ============================================================

static float b2_sin(uint8 angle) {
    return sinf((float)angle * (2.0f * 3.14159265f / 256.0f));
}
static float b2_cos(uint8 angle) {
    return cosf((float)angle * (2.0f * 3.14159265f / 256.0f));
}

// sr_addplayerZ (STRATROU.ASM:3092) — scroll with the world.
static void b2_add_player_z(Alien *al) {
    al->worldz = (int16)(al->worldz + g_pviewvelz);
}

// sr_keeprelto_player (STRATROU.ASM:2928):
//   worldz += playervelZ - pviewvelz
static void b2_keeprelto_player(Alien *al) {
    al->worldz = (int16)(al->worldz + (g_playervelZ - g_pviewvelz));
}

static void b2_copy_pos(Alien *dst, const Alien *src) {
    dst->worldx = src->worldx;
    dst->worldy = src->worldy;
    dst->worldz = src->worldz;
}

static void b2_copy_rots(Alien *dst, const Alien *src) {
    dst->rotx = src->rotx;
    dst->roty = src->roty;
    dst->rotz = src->rotz;
}

// [-range, +range] random (weapon scatter / explosion offsets).
static int16 b2_random_signed(int16 range) {
    uint16 span;
    if (range <= 0) {
        return 0;
    }
    span = (uint16)((range * 2) + 1);
    return (int16)(SfRtl_Random() % span) - range;
}

// addrnd2posy_srou (EXPSTRAT.ASM:342-357) — random signed byte to X and Y.
static void b2_addrnd2pos_xy(Alien *al) {
    int8 rx = (int8)(SfRtl_Random() & 0xFF);
    int8 ry = (int8)(SfRtl_Random() & 0xFF);
    al->worldx = (int16)(al->worldx + rx);
    al->worldy = (int16)(al->worldy + ry);
}

// Achase (proportional chase): step = (current - target) >> shift.
static void b2_achase16(int16 *current, int16 target, int shift) {
    int16 diff;
    int16 step;
    if (!current || *current == target) {
        return;
    }
    diff = (int16)(*current - target);
    step = (int16)(diff >> shift);
    if (step == 0) {
        step = (diff > 0) ? 1 : -1;
    }
    *current = (int16)(*current - step);
}

// Yaw angle from src toward dst in the XZ plane (0-255).
static uint8 b2_angle_xz(const Alien *src, const Alien *dst) {
    return Strat_AngleXZ(src, dst);
}

// Pitch angle from src toward dst (same formulation as strat_enemy.c
// strat_pitch_toward).
static uint8 b2_pitch_toward(const Alien *src, const Alien *dst) {
    float dy, dx, dz, dist, pitch;
    if (!src || !dst) {
        return 0u;
    }
    dy = (float)(dst->worldy - src->worldy);
    dx = (float)(dst->worldx - src->worldx);
    dz = (float)(dst->worldz - src->worldz);
    dist = sqrtf(dx * dx + dz * dz);
    if (dist <= 1.0f) {
        return src->rotx;
    }
    pitch = atan2f(dy, dist);
    return (uint8)(int)(pitch * (256.0f / (2.0f * 3.14159265f)));
}

// Position `self` at mother + local offset rotated around Y by mother roty
// (s_add_roffs2pos with rot flags 0,1,0). Does not touch rotations.
static void b2_yaw_offset_pos(Alien *self, const Alien *base, uint8 roty,
                              int16 offx, int16 offy, int16 offz) {
    float s = b2_sin(roty);
    float c = b2_cos(roty);
    int16 rx = (int16)lroundf(((float)offx * c) + ((float)offz * s));
    int16 rz = (int16)lroundf(((float)offz * c) - ((float)offx * s));
    self->worldx = (int16)(base->worldx + rx);
    self->worldy = (int16)(base->worldy + offy);
    self->worldz = (int16)(base->worldz + rz);
}

// addyrot2z_srou (GBSTRATS.ASM:59-77):
// fold al_roty into a signed value, +64, >>4 (arithmetic), add to worldz.
static void boss2_addyrot2z(Alien *self) {
    int16 a = (int16)self->roty;
    if (a < 128) {
        a = (int16)(-1 - a);      // eor #255 then sign-extend
    } else {
        a = (int16)(a - 256);     // already negative as signed byte
    }
    a = (int16)((a + 64) >> 4);
    self->worldz = (int16)(self->worldz + a);
}

// s_falldown_Yvec (STRATMAC.INC:1813):
//   vy += gravity; if still above ground (worldy < ground) do nothing more.
//   Otherwise clamp to ground and bounce: vy = (-vy) >> bounce_shift,
//   snapping small bounces ([-5,-1]) to zero. Returns true when the bounce
//   has fully decayed (the ASM's optional label branch).
static bool boss2_falldown_yvec(Alien *self, int bounce_shift, int16 gravity,
                                int16 ground) {
    int16 v;
    self->vy = (int16)(self->vy + gravity);
    if (self->worldy < ground) {   // s_jmp_higher — still airborne
        return false;
    }
    self->worldy = ground;
    v = (int16)(-self->vy);
    v = (int16)(v >> bounce_shift);
    if (v >= -5 && v <= 0) {       // cmp #-5 (unsigned) — tiny bounce -> stop
        v = 0;
    }
    self->vy = v;
    return (v == 0);
}

// s_add_vecs2pos
static void b2_add_vecs2pos(Alien *self) {
    self->worldx = (int16)(self->worldx + self->vx);
    self->worldy = (int16)(self->worldy + self->vy);
    self->worldz = (int16)(self->worldz + self->vz);
}

// s_boss_dying (STRATMAC.INC:7758-7767) — same behavior as the boss1 port.
static void boss2_boss_dying(void) {
    if ((g_bossflags & BF_DYING) == 0u) {
        Sound_PlaySE(B2_SE_BOSSDYING);
        Sound_PlayMusic(B2_BGM_BOSSDYING);
        g_bossflags |= BF_DYING;
        g_pstratflags |= PSTF_NOTDIE;
        g_stratflags |= SF_NOFIRING;
    }
}

// ============================================================
// Object-index links (same encoding as the boss1/boss7 ports:
// child->ptr = mother index + 1, chain through child->sword1,
// child->sbyte1 = child number).
// ============================================================

static uint16 b2_obj_index_or_null(const Alien *al) {
    if (!al) return 0u;
    if (al < g_aliens || al >= (g_aliens + NUMBER_AL)) return 0u;
    return (uint16)((al - g_aliens) + 1);
}

static Alien *b2_obj_from_index(uint16 index) {
    if (index == 0u) return NULL;
    index--;
    if (index >= NUMBER_AL) return NULL;
    return &g_aliens[index];
}

static void b2_clear_child_link(Alien *child) {
    if (!child) return;
    child->sflags3 &= (uint8)~ASF3_CHILDOBJ;
    child->ptr = 0u;
    child->sword1 = 0;
}

static void b2_prune_family_links(Alien *mother) {
    uint16 mother_idx;
    uint16 prev_idx;
    uint16 idx;
    int guard;

    if (!mother || (mother->sflags3 & ASF3_MOTHEROBJ) == 0u) {
        return;
    }

    mother_idx = b2_obj_index_or_null(mother);
    prev_idx = 0u;
    idx = (uint16)mother->sword1;
    guard = NUMBER_AL + 1;
    while (idx != 0u && guard-- > 0) {
        Alien *raw = b2_obj_from_index(idx);
        uint16 next_idx;
        bool valid;

        if (!raw) {
            break;
        }

        next_idx = (uint16)raw->sword1;
        valid = raw->active &&
                ((raw->sflags3 & ASF3_CHILDOBJ) != 0u) &&
                ((uint16)raw->ptr == mother_idx);

        if (!valid) {
            if (prev_idx == 0u) {
                mother->sword1 = (int16)next_idx;
            } else {
                Alien *prev = b2_obj_from_index(prev_idx);
                if (prev) {
                    prev->sword1 = (int16)next_idx;
                }
            }
            if ((uint16)raw->ptr == mother_idx) {
                b2_clear_child_link(raw);
            }
            idx = next_idx;
            continue;
        }

        prev_idx = idx;
        idx = next_idx;
    }

    if ((uint16)mother->sword1 == 0u) {
        mother->sflags3 &= (uint8)~ASF3_MOTHEROBJ;
    }
}

// s_set_objtobemother
static Alien *b2_get_mother_obj(Alien *self) {
    Alien *mother;
    if (!self || (self->sflags3 & ASF3_CHILDOBJ) == 0u) {
        return NULL;
    }
    mother = b2_obj_from_index((uint16)self->ptr);
    if (!mother || !mother->active) {
        b2_clear_child_link(self);
        return NULL;
    }
    return mother;
}

// s_set_objtobechild
static Alien *b2_find_child_obj(Alien *mother, uint8 child_num) {
    uint16 idx;
    int guard;

    if (!mother) {
        return NULL;
    }

    b2_prune_family_links(mother);
    idx = (uint16)mother->sword1;
    guard = NUMBER_AL + 1;
    while (idx != 0u && guard-- > 0) {
        Alien *child = b2_obj_from_index(idx);
        if (!child) {
            break;
        }
        if (child->active &&
            (child->sflags3 & ASF3_CHILDOBJ) != 0u &&
            (uint16)child->ptr == b2_obj_index_or_null(mother) &&
            child->sbyte1 == child_num) {
            return child;
        }
        idx = (uint16)child->sword1;
    }
    return NULL;
}

// s_count_childs
static uint8 b2_count_children(Alien *mother) {
    uint8 count;
    uint16 idx;
    int guard;

    if (!mother) {
        return 0u;
    }

    b2_prune_family_links(mother);
    count = 0u;
    idx = (uint16)mother->sword1;
    guard = NUMBER_AL + 1;
    while (idx != 0u && guard-- > 0) {
        Alien *child = b2_obj_from_index(idx);
        if (!child) {
            break;
        }
        count++;
        idx = (uint16)child->sword1;
    }
    return count;
}

static bool b2_attach_child_to_mother(Alien *mother, Alien *child,
                                      uint8 child_num) {
    uint16 child_idx;
    uint16 idx;
    int guard;

    if (!mother || !child || !mother->active || !child->active ||
        mother == child) {
        return false;
    }

    mother->sflags3 |= ASF3_MOTHEROBJ;
    child->sflags3 |= ASF3_CHILDOBJ;
    child->sbyte1 = child_num;
    child->ptr = b2_obj_index_or_null(mother);
    child->sword1 = 0;

    child_idx = b2_obj_index_or_null(child);
    if (child_idx == 0u) {
        b2_clear_child_link(child);
        return false;
    }

    if ((uint16)mother->sword1 == 0u) {
        mother->sword1 = (int16)child_idx;
        return true;
    }

    idx = (uint16)mother->sword1;
    guard = NUMBER_AL + 1;
    while (guard-- > 0) {
        Alien *link = b2_obj_from_index(idx);
        if (!link) {
            return false;
        }
        if ((uint16)link->sword1 == 0u) {
            link->sword1 = (int16)child_idx;
            return true;
        }
        idx = (uint16)link->sword1;
    }
    return false;
}

// s_make_childobj shape,child_num,Istrat,enemy1 (STRATMAC.INC:7137)
static Alien *boss2_spawn_child(Alien *mother, uint16 shape, uint8 child_num,
                                void (*init_fn)(Alien *)) {
    Alien *child;

    if (!mother || !init_fn) {
        return NULL;
    }

    child = Obj_Alloc();
    if (!child) {
        return NULL;
    }

    Strat_InitObjVars(child);
    child->shape = shape;
    b2_copy_pos(child, mother);
    if (!b2_attach_child_to_mother(mother, child, child_num)) {
        Obj_Free(child);
        return NULL;
    }

    // colltype argument of s_make_childobj: enemy1.
    child->collflags |= COLLTYPE_ENEMY1;
    init_fn(child);
    return child;
}

// ============================================================
// Explosion object factories (local copies of the EXPSTRAT.ASM ports
// in strat_enemy.c: makeexpobj/makeLexpobj/makeMEDexpobj/makeFOLexpobj).
// ============================================================

// delayexplode_strat (EXPSTRAT.ASM:259-268)
static void boss2_delayexplode_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->sflags |= ASF_HITFLASH;
    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        g_aldead = 1u;
        if (self->expstratptr) {
            self->expstratptr(self);
        }
        return;
    }
    if (self->sflags2 & ASF2_RELEXPLODE) {
        b2_add_player_z(self);
    }
}

// makeexpobj_srou (EXPSTRAT.ASM:327-338)
static Alien *b2_make_exp_obj(const Alien *parent) {
    Alien *child;

    if (!parent) {
        return NULL;
    }

    child = Strat_MakeObj(0u);
    if (!child) {
        return NULL;
    }

    child->sflags3 &= (uint8)~ASF3_REALOBJ;
    child->sflags |= ASF_COLLDISABLE;
    child->sflags2 |= (ASF2_NOEXPSND | ASF2_RELEXPLODE);
    child->HP = HARD_HP;
    child->AP = HARD_AP;
    child->stratptr = boss2_delayexplode_strat;
    child->collstratptr = NULL;
    child->expstratptr = Strat_Explode;
    b2_copy_pos(child, parent);
    return child;
}

static Alien *b2_make_large_exp_obj(const Alien *parent) {
    Alien *child = b2_make_exp_obj(parent);
    if (child) {
        child->shape = B2_EXPSHAPE_LARGE;
    }
    return child;
}

static Alien *b2_make_medium_exp_obj(const Alien *parent) {
    Alien *child = b2_make_exp_obj(parent);
    if (child) {
        child->shape = B2_EXPSHAPE_MEDIUM;
    }
    return child;
}

static Alien *b2_make_fol_exp_obj(const Alien *parent) {
    Alien *child = b2_make_exp_obj(parent);
    if (child) {
        child->shape = B2_EXPSHAPE_FOLARGE;
    }
    return child;
}

// delayremove_strat (GSTRATS.ASM:1188-1193), relexplode variant.
static void boss2_delayremove_strat(Alien *self) {
    if (!self) {
        return;
    }
    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        // Final removal of the boss shell: release the boss HUD bar and the
        // dying state (same convention as the boss1/boss7 ports).
        g_bossflags &= (uint8)~BF_DYING;
        g_bossmaxhp = 0u;
        g_aldead = 1u;
        return;
    }
    if (self->sflags2 & ASF2_RELEXPLODE) {
        b2_add_player_z(self);
    }
}

// makesmoke_srou_l (GSTRATS.ASM:1265-1275) + smokeP_Istrat.
// The L2smoke sprite shape is not in the compiled set; the object is spawned
// with a nullshape proxy so the timing/structure is preserved.
static Alien *boss2_make_smoke(const Alien *parent) {
    Alien *smoke = Strat_MakeObj(SH_L2SMOKE_PROXY);
    if (!smoke) {
        return NULL;
    }
    smoke->sflags3 &= (uint8)~ASF3_REALOBJ;
    smoke->sflags |= ASF_COLLDISABLE;
    smoke->type |= ATZREMOVE;
    smoke->stratptr = boss2_smoke_strat;
    smoke->collstratptr = NULL;
    smoke->expstratptr = NULL;
    smoke->count = 8u;   // smokeP colanim runs 8 steps
    b2_copy_pos(smoke, parent);
    return smoke;
}

// smokeP_strat (GSTRATS.ASM): drift x by -1 per frame, die after anim.
static void boss2_smoke_strat(Alien *self) {
    if (!self) {
        return;
    }
    self->worldx = (int16)(self->worldx - 1);
    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        g_aldead = 1u;
    }
}

// particlefiredown_Istrat placeholder — the jump-thruster particle emitter
// object created in boss2_strat state 1. The particle system shape/emitter is
// not ported; the object exists so the al_ptr bookkeeping stays literal
// (position is copied from the boss each frame in state 2).
static void boss2_particle_strat(Alien *self) {
    (void)self;
}

// ============================================================
// Weapons
// ============================================================

static Alien *b2_target_obj(const Alien *self) {
    Alien *target;
    if (!self || self->fireobjptr == 0u) {
        return NULL;
    }
    target = b2_obj_from_index(self->fireobjptr);
    if (!target || !target->active) {
        return NULL;
    }
    return target;
}

// Common enemy projectile spawn: offset is rotated around Y by self->roty.
static Alien *b2_spawn_shot(Alien *self, int16 offx, int16 offy, int16 offz,
                            uint8 pitch, uint8 yaw,
                            uint8 speed, uint8 life, uint8 ap) {
    Alien *shot = Strat_SpawnProjectile(self, 0, 0, 0, pitch, yaw,
                                        speed, life, ap, ACF_COLLTYPE4);
    if (!shot) {
        return NULL;
    }
    b2_yaw_offset_pos(shot, self, self->roty, offx, offy, offz);
    shot->rotx = pitch;
    shot->roty = yaw;
    shot->rotz = self->rotz;
    Strat_GenVecs3D(shot);
    return shot;
}

// fire_relfastElaser (GSTRATS.ASM:2578) — straight laser, speed 90.
// s_weapon_rndrots2obj y,7,7 scatter is applied by the caller.
static void boss2_fire_relfastelaser(Alien *self, int16 offy, Alien *target,
                                     int16 rnd_pitch, int16 rnd_yaw) {
    uint8 pitch = self->rotx;
    uint8 yaw = self->roty;

    if (target && target->active) {
        yaw = (uint8)(b2_angle_xz(self, target) + b2_random_signed(rnd_yaw));
        pitch = (uint8)(b2_pitch_toward(self, target) +
                        b2_random_signed(rnd_pitch));
    }
    (void)b2_spawn_shot(self, 0, offy, 0, pitch, yaw,
                        RELFASTELASER_SPEED, RELFASTELASER_LIFE,
                        ENEMYLASER_AP);
}

// relelaserhome (fire_relslowElaserHome) tick — chase target angles then fly.
static void boss2_homelaser_strat(Alien *self) {
    Alien *target;

    if (!self) {
        return;
    }

    target = b2_target_obj(self);
    if (target) {
        uint8 want_yaw = b2_angle_xz(self, target);
        uint8 want_pitch = b2_pitch_toward(self, target);
        int8 dyaw = (int8)(self->roty - want_yaw);
        int8 dpitch = (int8)(self->rotx - want_pitch);
        self->roty = (uint8)(self->roty - (dyaw >> 3));
        self->rotx = (uint8)(self->rotx - (dpitch >> 3));
        Strat_GenVecs3D(self);
    }

    b2_add_player_z(self);
    Strat_ApplyVelocity(self);

    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        g_aldead = 1u;
    }
}

// fire_relslowElaserHome (GSTRATS.ASM:2563) — homing enemy laser.
static void boss2_fire_relslowelaserhome(Alien *self, int16 offy,
                                         Alien *target) {
    Alien *shot;
    uint8 speed = (g_currentlevel == 1u) ? 48u : 60u;  // doelaserspeed
    uint8 pitch = self->rotx;
    uint8 yaw = self->roty;

    if (target && target->active) {
        yaw = b2_angle_xz(self, target);
        pitch = b2_pitch_toward(self, target);
    }
    shot = b2_spawn_shot(self, 0, offy, 0, pitch, yaw,
                         speed, RELFASTELASER_LIFE, ENEMYLASER_AP);
    if (shot && target) {
        shot->stratptr = boss2_homelaser_strat;
        shot->fireobjptr = b2_obj_index_or_null(target);
    }
}

// hmissile2_strat (GSTRATS.ASM:1500) — "move straight for short time then
// quick turn relative to player".
static void boss2_hmissile2_strat(Alien *self) {
    Alien *target;

    if (!self) {
        return;
    }

    self->rotz = (uint8)(self->rotz + 10u);  // missile spin

    if (self->count1 > 0u) {
        // Straight launch phase.
        self->count1--;
    } else {
        target = b2_target_obj(self);
        if (target) {
            uint8 want_yaw = b2_angle_xz(self, target);
            uint8 want_pitch = b2_pitch_toward(self, target);
            int8 dyaw = (int8)(self->roty - want_yaw);
            int8 dpitch = (int8)(self->rotx - want_pitch);
            self->roty = (uint8)(self->roty - (dyaw >> 1));  // quick turn
            self->rotx = (uint8)(self->rotx - (dpitch >> 1));
        }
        Strat_GenVecs3D(self);
    }

    b2_add_player_z(self);
    Strat_ApplyVelocity(self);

    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        g_aldead = 1u;
    }
}

// fire_bossHmissile2 (GSTRATS.ASM:2675 -> fire_Hmissile2).
// The ASM sets weapon rot (0, +/-deg22) with the firer's roty forced to 0,
// then targets the player via al_ptr = playpt.
static void boss2_fire_bosshmissile2(Alien *self, int16 offy, uint8 yaw,
                                     Alien *target) {
    Alien *shot = Strat_SpawnProjectile(self, 0, offy, 0, 0u, yaw,
                                        BOSSHMISSILE2_SPEED,
                                        BOSSHMISSILE2_LIFE,
                                        BOSSHMISSILE2_AP,
                                        ACF_COLLTYPE4);
    if (!shot) {
        return;
    }
    shot->stratptr = boss2_hmissile2_strat;
    shot->type = (uint8)(ATMISSILE | ATZREMOVE);
    shot->sflags &= (uint8)~ASF_INVISIBLE;
    shot->sflags |= ASF_SHADOW;
    shot->HP = 2u;                       // hmissile1HP
    shot->count1 = 16u;                  // straight-flight frames
    shot->fireobjptr = b2_obj_index_or_null(target);
    Strat_GenVecs3D(shot);
}

// homingflat_Istrat tick used by fire_Hplasma projectiles.
static void boss2_hplasma_strat(Alien *self) {
    Alien *target;

    if (!self) {
        return;
    }

    target = b2_target_obj(self);
    if (target && abs(self->worldz - target->worldz) >= 500) {
        uint8 want_yaw = b2_angle_xz(self, target);
        uint8 want_pitch = b2_pitch_toward(self, target);
        int8 dyaw = (int8)(self->roty - want_yaw);
        int8 dpitch = (int8)(self->rotx - want_pitch);
        self->roty = (uint8)(self->roty - (dyaw >> 4));
        self->rotx = (uint8)(self->rotx - (dpitch >> 4));
        Strat_GenVecs3D(self);
    }

    b2_add_player_z(self);
    Strat_ApplyVelocity(self);

    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        g_aldead = 1u;
    }
}

// fire_Hplasma (GSTRATS.ASM:2517) — slow homing plasma ball.
static void boss2_fire_hplasma(Alien *self, int16 offy, int16 offz,
                               Alien *target) {
    Alien *shot = b2_spawn_shot(self, 0, offy, offz,
                                self->rotx, self->roty,
                                BOSS2_HPLASMA_SPEED, BOSS2_HPLASMA_LIFE,
                                BOSS2_HPLASMA_AP);
    if (!shot) {
        return;
    }
    shot->stratptr = boss2_hplasma_strat;
    // s_set_alvar W,y,al_ptr,playpt — home on the player.
    shot->fireobjptr = b2_obj_index_or_null(target);
}

// ============================================================
// boss2exp_Istrat -> bossbigoutexplodeoff_Istrat (EXPSTRAT.ASM:155)
// Final boss detonation. Sets GF_BOSSDEAD immediately (which releases
// world_cb_chkbossdead / mapwaitboss), spawns the big explosion cluster
// around pos + (vx,vy,vz), then lingers 11 frames as delayremove(rel).
// ============================================================
static void boss2exp_init(Alien *self) {
    int i;
    int16 offx;
    int16 offy;
    int16 offz;

    if (!self) {
        return;
    }

    // boss2exp_Istrat: s_set_vecs x,#0,#15<<boss2_scale,#0
    self->vx = 0;
    self->vy = B2U(15);
    self->vz = 0;

    offx = self->vx;
    offy = self->vy;
    offz = self->vz;

    // bossbigoutexplode_Icont: s_or_var B,gameflags,#gf_bossdead
    g_gameflags |= GF_BOSSDEAD;
    Sound_PlaySE(0x1Du);   // makebosscircexp boom (circdelayexplode port)

    // 10 iterations of paired L/FOL explosions flying outward with random
    // lifetimes (.genexp loop + setoutexp_srou, simplified to position/time
    // scatter — the outward xy-vector visual lives in the renderer).
    for (i = 0; i < 10; i++) {
        Alien *exp;

        exp = b2_make_large_exp_obj(self);
        if (exp) {
            exp->sflags4 |= ASF4_NOPOLYEXP;
            exp->count = (uint8)((SfRtl_Random() % 15u) + 1u);
            b2_addrnd2pos_xy(exp);
            exp->worldx = (int16)(exp->worldx + offx + b2_random_signed(64));
            exp->worldy = (int16)(exp->worldy + offy + b2_random_signed(64));
            exp->worldz = (int16)(exp->worldz + offz);
        }

        exp = b2_make_fol_exp_obj(self);
        if (exp) {
            exp->sflags4 |= ASF4_NOPOLYEXP;
            exp->count = (uint8)((SfRtl_Random() % 15u) + 8u);
            b2_addrnd2pos_xy(exp);
            exp->worldx = (int16)(exp->worldx + offx + b2_random_signed(64));
            exp->worldy = (int16)(exp->worldy + offy + b2_random_signed(64));
            exp->worldz = (int16)(exp->worldz + offz);
        }
    }

    // s_set_lifecnt x,#11 ; s_jmp delayremoverel_Istrat
    self->stratptr = boss2exp_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    self->sflags |= ASF_COLLDISABLE;
    self->sflags2 |= ASF2_RELEXPLODE;
    self->flags |= AFEXP;
    self->count = 11u;
}

static void boss2exp_strat(Alien *self) {
    boss2_delayremove_strat(self);
}

// ============================================================
// boss2top_Istrat / boss2top_strat — the spinning-top core (child 1).
// ============================================================
static void boss2top_init(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_alptrs x,boss2top_strat,boss2topcol_Istrat,boss2topexp_Istrat
    self->stratptr = boss2top_strat;
    self->collstratptr = boss2topcol_coll;
    self->expstratptr = boss2topexp;
    // s_set_aldata x,#boss2topHP,#boss2topAP
    self->HP = BOSS2TOP_HP;
    self->AP = BOSS2TOP_AP;
    // s_add_bossmaxHP #boss2topHP
    g_bossmaxhp = (uint16)(g_bossmaxhp + BOSS2TOP_HP);
    // s_set_colltype x,enemyweap
    self->collflags |= COLLTYPE_ENEMYWEAP;
    // s_set_alsflag x,colldisable
    self->sflags |= ASF_COLLDISABLE;
}

static void boss2top_strat(Alien *self) {
    Alien *mother;
    Alien *player;

    if (!self) {
        return;
    }

    // s_set_objtobemother y,x
    mother = b2_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    // s_copy_pos x,y ; s_copy_rots x,y
    b2_copy_pos(self, mother);
    b2_copy_rots(self, mother);

    // s_jmpNOT_alsflag y,sflag3,.nocol / s_clr_alsflag x,colldisable
    if ((mother->sflags2 & BOSS2_SFLAG3) != 0u) {
        self->sflags &= (uint8)~ASF_COLLDISABLE;
    }

    // s_jmp_alvarmore B,x,al_HP,#16,.npfly — top critical below 16 HP.
    if (self->HP <= 16u) {
        // s_set_alsflag y,sflag2 — order the petals to die.
        mother->sflags2 |= BOSS2_SFLAG2;
        // s_weapon_pos #0,#((-30-20)*(1<<boss2_scale))>>weapon_scale,#0
        // s_jmp_notdelay 3 — every 8 frames, homing laser at the player.
        if ((g_gameframe & 7u) == 0u) {
            player = Obj_GetPlayer();
            if (player && player->active) {
                boss2_fire_relslowelaserhome(self, B2U(-30 - 20), player);
            }
        }
    }

    // .npfly: s_jmpNOT_alsflag y,sflag4,.nfire / s_jmp_NOTdelay 5,.nfire
    if ((mother->sflags2 & BOSS2_SFLAG4) != 0u && (g_gameframe & 31u) == 0u) {
        // s_weapon_rot #0,#-deg22 (or +deg22 on s_jmp_random)
        uint8 yaw = (uint8)-DEG22;
        if ((SfRtl_Random() & 1u) != 0u) {
            yaw = DEG22;
        }
        // s_push_alvar B,x,al_roty / s_set_alvar B,x,al_roty,#0
        // s_weapon_pos #0,#((-30)*(1<<boss2_scale))>>weapon_scale,#0
        // s_fire_weapon x,BOSSHMISSILE2 / al_ptr = playpt
        player = Obj_GetPlayer();
        {
            uint8 saved_roty = self->roty;
            self->roty = 0u;
            boss2_fire_bosshmissile2(self, B2U(-30), yaw, player);
            self->roty = saved_roty;
        }
    }

    // s_add_bossHP x,al_hp — HUD boss bar accumulation is implicit in the
    // flat runtime (nmi.c currently mirrors g_bossmaxhp).
}

// boss2topcol_Istrat — every hit spawns a medium explosion puff, then the
// standard hitflash damage path.
static void boss2topcol_coll(Alien *self) {
    Alien *exp;

    if (!self) {
        return;
    }

    // s_jsl makeMEDexpobj_srou
    exp = b2_make_medium_exp_obj(self);
    if (exp) {
        // s_add_alvar W,y,al_worldy,#(60)<<boss2_scale
        exp->worldy = (int16)(exp->worldy + B2U(60));
        // s_set_alvar W,y,al_vy,#-20 (delayexplode child keeps vy for visual)
        exp->vy = -20;
        exp->count = 1u;
    }

    // s_jmp hitflash_Istrat
    Strat_HitFlash(self);
}

// boss2topexp_Istrat
static void boss2topexp(Alien *self) {
    if (!self) {
        return;
    }
    // s_add_alvar W,x,al_worldy,#(60)<<boss2_scale
    self->worldy = (int16)(self->worldy + B2U(60));
    // s_jmp explode_Istrat
    Strat_Explode(self);
}

// ============================================================
// boss2turret_Istrat / boss2turret_strat — orbiting gun pods (children
// 7/9/11/13, phase 1 targets).
// ============================================================
static void boss2turret_init(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_alptrs x,boss2turret_strat,hitflash_Istrat,boss2turretexp_Istrat
    self->stratptr = boss2turret_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = boss2turretexp;
    // s_set_aldata x,#boss2turretHP,#boss2turretAP
    self->HP = BOSS2TURRET_HP;
    self->AP = BOSS2TURRET_AP;
    // s_add_bossmaxHP #boss2turretHP
    g_bossmaxhp = (uint16)(g_bossmaxhp + BOSS2TURRET_HP);
    // s_set_colltype x,enemyweap
    self->collflags |= COLLTYPE_ENEMYWEAP;
    // s_set_alsflag x,shadow
    self->sflags |= ASF_SHADOW;
}

static void boss2turret_strat(Alien *self) {
    Alien *mother;

    if (!self) {
        return;
    }

    mother = b2_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    // s_copy_pos x,y ; s_copy_rots x,y
    b2_copy_pos(self, mother);
    b2_copy_rots(self, mother);
    // s_add_alvars B,x,al_roty,x,al_sbyte2 — per-pod angle around the top.
    self->roty = (uint8)(self->roty + self->sbyte2);
    // s_jsr addyrot2z_srou
    boss2_addyrot2z(self);

    // s_jmp_alvarne B,x,al_roty,#deg180,.nfire — fire when facing the player.
    if (self->roty == DEG180) {
        // s_weapon_pos #0,#(-26*8)>>weapon_scale,#(36*8)>>weapon_scale
        // s_weapon_rot #0,#0 / s_fire_weapon x,HPLASMA / al_ptr = playpt
        Alien *player = Obj_GetPlayer();
        boss2_fire_hplasma(self, (int16)(-26 * 8), (int16)(36 * 8), player);
    }

    // s_add_bossHP x,al_hp — implicit (see boss2top_strat).
}

// boss2turretexp_Istrat — large explosion at the pod position, then remove.
static void boss2turretexp(Alien *self) {
    Alien *exp;

    if (!self) {
        return;
    }

    // s_jsl makeLexpobj_srou
    exp = b2_make_large_exp_obj(self);
    if (exp) {
        // s_add_roffs2pos B,y,x,x,#0,#5-30,#35,0,1,0,boss2_scale x3
        b2_yaw_offset_pos(exp, self, self->roty,
                          0, B2U(5 - 30), B2U(35));
        exp->count = 1u;
    }

    // s_jmp remove_Istrat
    g_aldead = 1u;
}

// ============================================================
// boss2petal_Istrat / boss2petal_strat — four armor petals (children 2-5).
// Indestructible shields that open/close on mother sflag1 and launch the
// orbiting rock-beam plasma in the strafe phase.
// ============================================================
static void boss2petal_init(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_alptrs x,boss2petal_strat,0,boss2petalexp_Istrat
    self->stratptr = boss2petal_strat;
    self->collstratptr = NULL;
    self->expstratptr = boss2petalexp_init;
    // s_set_aldata x,#hardHP,#boss2petalAP
    self->HP = HARD_HP;
    self->AP = BOSS2PETAL_AP;
    // s_set_state x,#0
    self->stratstate = 0u;
    // s_set_alvar B,x,al_sbyte3,#0
    self->sbyte3 = 0u;
    // s_or_alvar B,x,al_collflags,#colltype_enemyweap / s_set_colltype
    self->collflags |= COLLTYPE_ENEMYWEAP;
    // s_set_alsflag x,shadow
    self->sflags |= ASF_SHADOW;
}

static void boss2petal_strat(Alien *self) {
    Alien *mother;
    uint8 target_open;

    if (!self) {
        return;
    }

    mother = b2_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    // s_jmpNOT_alsflag y,sflag2,.ntopd — top destroyed: petals die.
    if ((mother->sflags2 & BOSS2_SFLAG2) != 0u) {
        // s_jmp_NOTdelay 0,... — delay mask 0, always taken.
        // s_kill_obj x / s_set_alsflag x,sflag2
        self->sflags2 |= BOSS2_SFLAG2;
        self->HP = 0u;   // dead -> engine runs expstratptr (petalexp)
        return;          // .endpetal
    }

    // s_copy_alvar2var.w B,y,svar_byte1,al_sbyte3 — target openness.
    target_open = mother->sbyte3;

    if ((mother->sflags2 & BOSS2_SFLAG1) != 0u) {
        // .boss2petal_open — every 4 frames step sbyte3 toward the target.
        if ((g_gameframe & 3u) == 0u && self->sbyte3 != target_open) {
            self->sbyte3 = (uint8)(self->sbyte3 + 2u);
            // .boss2petal_chg: shape = boss2petal_tab[sbyte3/2]
            if ((self->sbyte3 >> 1) < 3u) {
                self->shape = boss2petal_tab[self->sbyte3 >> 1];
            }
        }
    } else {
        // .boss2petal_close
        if ((g_gameframe & 3u) == 0u && self->sbyte3 != 0u) {
            self->sbyte3 = (uint8)(self->sbyte3 - 2u);
            if ((self->sbyte3 >> 1) < 3u) {
                self->shape = boss2petal_tab[self->sbyte3 >> 1];
            }
        }
    }

    // s_copy_pos x,y ; s_copy_rots x,y ; roty += sbyte2 ; addyrot2z
    b2_copy_pos(self, mother);
    b2_copy_rots(self, mother);
    self->roty = (uint8)(self->roty + self->sbyte2);
    boss2_addyrot2z(self);

    // s_jmp_NOTalsflag y,sflag3,.nballg / s_jmp_alsflag x,sflag1,.nballg
    // Launch one orbiting plasma per petal when the strafe phase starts.
    if ((mother->sflags2 & BOSS2_SFLAG3) != 0u &&
        (self->sflags2 & BOSS2_SFLAG1) == 0u) {
        Alien *plasma;
        // s_set_alsflag x,colldisable / s_set_alsflag x,sflag1
        self->sflags |= ASF_COLLDISABLE;
        self->sflags2 |= BOSS2_SFLAG1;
        // s_make_obj #rockbeam,.nballg / strat = boss2plasma_Istrat
        plasma = Strat_MakeObj(SH_ROCKBEAM_PROXY);
        if (plasma) {
            boss2plasma_init(plasma, self);
        }
    }
}

// boss2petalexp_Istrat — first frame of petal death: become spinning debris.
static void boss2petalexp_init(Alien *self) {
    if (!self) {
        return;
    }
    // s_set_expstrat x,boss2petalexp_strat (runs while HP == 0)
    self->expstratptr = boss2petalexp_strat;
    // s_set_lifecnt x,#50
    self->count = 50u;
    self->sflags |= ASF_COLLDISABLE;
}

// boss2petalexp_strat — spin away, then vanish.
static void boss2petalexp_strat(Alien *self) {
    if (!self) {
        return;
    }
    // s_add_alvar B,x,al_rotx,#-4
    self->rotx = (uint8)(self->rotx - 4u);
    // s_dec_lifecnt x
    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        g_aldead = 1u;
    }
}

// ============================================================
// boss2plasma_Istrat / boss2plasma_strat — the orbiting rock-beam plasma
// spawned by each petal in the strafe phase. Spirals outward around its
// petal while tracking the player's height.
// ============================================================
static void boss2plasma_init(Alien *plasma, Alien *petal) {
    if (!plasma || !petal) {
        return;
    }

    // s_set_strat x,boss2plasma_strat
    plasma->stratptr = boss2plasma_strat;
    plasma->collstratptr = NULL;
    plasma->expstratptr = NULL;
    // s_set_aldata x,#hardHP,#8
    plasma->HP = HARD_HP;
    plasma->AP = BOSS2PLASMA_AP;
    // s_set_colltype x,enemy1 / s_set_colltype x,enemyweap
    plasma->collflags |= (COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP);
    // s_set_objtobealvar y,x,al_sword1 (inverted here: store petal in sword1)
    plasma->sword1 = (int16)b2_obj_index_or_null(petal);
    // s_copy_alvar2alvar B,x,al_sbyte1,y,al_roty
    plasma->sbyte1 = petal->roty;
    plasma->sbyte2 = 0u;   // orbit distance
    // s_set_alvar B,x,al_roty,#deg180
    plasma->roty = DEG180;
    // s_sprite_obj x,#0 — sprite shape (rockbeam) proxied via nullshape.
    // s_copy_alvar2alvar W,x,al_sword2,y,al_worldy
    plasma->sword2 = petal->worldy;
    // s_setnoremove_behind x
    plasma->type &= (uint8)~ATZREMOVE;
    b2_copy_pos(plasma, petal);
}

static void boss2plasma_strat(Alien *self) {
    Alien *petal;
    uint8 dist;
    uint8 spin;
    int16 hover;

    if (!self) {
        return;
    }

    // s_set_objtobealvar y,x,al_sword1 / s_jmp_alsflag y,sflag2,remove
    petal = b2_obj_from_index((uint16)self->sword1);
    if (!petal || !petal->active ||
        (petal->sflags2 & BOSS2_SFLAG2) != 0u) {
        g_aldead = 1u;
        return;
    }

    dist = self->sbyte2;

    // s_copy_alvar2alvar B,x,al_roty,x,al_sbyte1
    // s_add_Roffs2pos B,x,y,x,#0,#0,svar_byte2,0,1,0,2,0,4
    // (offset (0,0,dist<<4) rotated around Y by al_sbyte1)
    b2_yaw_offset_pos(self, petal, self->sbyte1,
                      0, 0, (int16)((int16)dist << 4));

    // s_set_alvar B,x,al_roty,#deg180
    self->roty = DEG180;

    // Rotation speed falls off as the spiral widens: +4/+3/+2 then +1.
    spin = 1u;
    if (dist <= 30u) {
        spin = (uint8)(spin + 4u);
    }
    if (dist <= 60u) {
        spin = (uint8)(spin + 3u);
    }
    if (dist <= 90u) {
        spin = (uint8)(spin + 2u);
    }
    self->sbyte1 = (uint8)(self->sbyte1 + spin);

    // s_add_alvar B,x,al_sbyte2,#1 / wrap at 120.
    self->sbyte2 = (uint8)(self->sbyte2 + 1u);
    if (self->sbyte2 >= 120u) {
        self->sbyte2 = 0u;
    }

    // s_copy_alvar2alvar W,x,al_worldy,x,al_sword2
    self->worldy = self->sword2;
    // s_achase_alvar W,x,al_sword2,player_posy,2
    hover = self->sword2;
    b2_achase16(&hover, g_player_posy, 2);
    self->sword2 = hover;
}

// ============================================================
// boss2_strat — the mother state machine.
//   state 0: turret phase (spin up as pods die; smoke while far)
//   state 1: leap into the air
//   state 2: flip and slam down over the player
//   state 3: back away to firing distance
//   state 4: strafe circle + laser volleys; petals open, plasma orbs launch
//   state 5: top destroyed — topple, boss_dying, final kill
// ============================================================
static void boss2_strat(Alien *self) {
    Alien *player;
    uint8 nchildren;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();

    // s_count_childs x,svar_byte5
    nchildren = b2_count_children(self);

    // ---------------------------------------------------------------
    if (self->stratstate == 0u) {
        if (nchildren <= 5u + 2u) {   // s_jmp_varMORE B,svar_byte5,#5+2,.nso
            self->sflags2 |= BOSS2_SFLAG4;
            self->roty = (uint8)(self->roty + 2u);
            if ((self->sflags2 & BOSS2_SFLAG1) == 0u) {
                Sound_PlaySE(B2_SE_SPIN);   // trigse $71 (gated by sflag1)
            }
            self->sflags2 |= BOSS2_SFLAG1;
            self->sbyte3 = 2u;              // petals: half open
            if (nchildren == 5u + 1u) {     // s_jmp_varNE ...,#5+1,.nso
                self->roty = (uint8)(self->roty + 2u);
            }
        }

        if (nchildren == 5u) {
            // s_jmp_varEQ B,svar_byte5,#5,nextstate — all pods destroyed.
            self->stratstate++;
        } else {
            // s_set_objtobeplayer y / s_jmp_Zdistmore x,y,#1100,.smoke
            if (player && player->active &&
                abs(self->worldz - player->worldz) <= 1100) {
                // s_keeprelto_player x / s_add_playerZ x / s_brl .end
                b2_keeprelto_player(self);
                b2_add_player_z(self);
            } else {
                // .smoke — two ground-churn smoke puffs per frame.
                Alien *smoke;

                smoke = boss2_make_smoke(self);   // jsl makesmoke_srou_l
                if (smoke) {
                    b2_addrnd2pos_xy(smoke);      // s_jsl addrnd2posy_srou
                    smoke->worldz = (int16)(smoke->worldz - 100);
                    smoke->worldy = (int16)(smoke->worldy + B2U(-35));
                }

                smoke = boss2_make_smoke(self);
                if (smoke) {
                    b2_addrnd2pos_xy(smoke);
                    smoke->worldz = (int16)(smoke->worldz - 100);
                    smoke->worldy = (int16)(smoke->worldy + B2U(-20));
                }
            }
            // .end
            self->roty = (uint8)(self->roty + 2u);
            return;
        }
    }

    // ---------------------------------------------------------------
    if (self->stratstate == 1u) {
        Alien *particle;

        Sound_PlaySE(B2_SE_JUMP);              // trigse $85
        self->sflags2 &= (uint8)~BOSS2_SFLAG4; // s_clr_alsflag x,sflag4
        // s_set_vecs x,#0,#-80,#0 — leap upward.
        self->vx = 0;
        self->vy = -80;
        self->vz = 0;
        self->sword2 = 0;                      // landing height (ground)
        self->stratstate++;                    // s_next_state x

        // s_set_alvar W,x,al_ptr,#0 / s_make_obj #nullshape
        // / s_set_strat y,particlefiredown_Istrat / s_copy_pos y,x
        self->ptr = 0u;
        particle = Strat_MakeObj(0u);
        if (particle) {
            particle->stratptr = boss2_particle_strat;
            particle->sflags |= ASF_COLLDISABLE;
            particle->sflags3 &= (uint8)~ASF3_REALOBJ;
            b2_copy_pos(particle, self);
            // s_set_alvartobeobj x,al_ptr,y
            self->ptr = b2_obj_index_or_null(particle);
        }
    }

    // ---------------------------------------------------------------
    if (self->stratstate == 2u) {
        Alien *particle;
        int16 ground;

        // s_set_objtobealvar y,x,al_ptr / s_copy_pos y,x
        particle = b2_obj_from_index(self->ptr);
        if (particle && particle->active) {
            b2_copy_pos(particle, self);
        }

        // s_jmp_lower x,#-1000,.low — while high up, flip and track player.
        if (self->worldy < -1000) {
            int16 chase;

            self->rotz = DEG180;                    // upside down
            self->sword2 = B2U(-60);                // hover-slam height
            // s_achase_alvar W,x,al_worldx,player_posX,4
            chase = self->worldx;
            b2_achase16(&chase, g_player_posx, 4);
            self->worldx = chase;
            // svar_word1 = player_posz + 200 ; achase worldz shift 5
            chase = self->worldz;
            b2_achase16(&chase, (int16)(g_player_posz + 200), 5);
            self->worldz = chase;
        }

        // .low
        b2_add_player_z(self);       // s_add_playerZ x
        b2_add_vecs2pos(self);       // s_add_vecs2pos x

        // s_copy_alvar2var W,x,svar_word1,al_sword2
        ground = self->sword2;

        // s_falldown_Yvec x,2,#2,svar_word1,.fin
        if (boss2_falldown_yvec(self, 2, 2, ground)) {
            // .fin — landed: remove the thruster particle, next state.
            if (particle && particle->active) {
                Obj_Free(particle);
            }
            self->ptr = 0u;
            self->stratstate++;
        } else {
            // s_jmp_higher x,svar_word1,.nsnd2 / trigse $8e — bounce thud.
            // Flow then falls through to .ns2 and on to the shared .end
            // (state checks below fail while stratstate is still 2).
            if (self->worldy >= ground) {
                Sound_PlaySE(B2_SE_LAND);
            }
        }
    }

    // ---------------------------------------------------------------
    if (self->stratstate == 3u) {
        int16 chase;

        b2_add_player_z(self);            // s_add_playerZ x
        // s_achase_alvar W,x,al_worldx,#0,3
        chase = self->worldx;
        b2_achase16(&chase, 0, 3);
        self->worldx = chase;
        // s_add_alvar W,x,al_worldz,#30 — back away.
        self->worldz = (int16)(self->worldz + 30);
        // s_set_alvar B,x,al_sbyte4,#25
        self->sbyte4 = 25u;
        // s_jmp_Zdistmore x,y,#1100,nextstate
        if (player && player->active &&
            abs(self->worldz - player->worldz) > 1100) {
            self->stratstate++;
        }
    }

    // ---------------------------------------------------------------
    if (self->stratstate == 4u) {
        Alien *top;

        b2_add_player_z(self);   // s_add_playerZ x

        // s_decbne_alvar B,x,al_sbyte4,.nres / reset to 100.
        self->sbyte4--;
        if (self->sbyte4 == 0u) {
            self->sbyte4 = 100u;
        }

        if (self->sbyte4 <= 25u) {
            // Firing window: s_jmp_notdelay 1 — every other frame.
            if ((g_gameframe & 1u) == 0u && player && player->active) {
                // s_weapon_pos #0,#(-60<<boss2_scale)>>weapon_scale,#0
                // s_weapon_rndrots2obj y,7,7 / s_fire_weapon x,RELFASTELASER
                boss2_fire_relfastelaser(self, B2U(-60), player, 7, 7);
            }
        } else {
            // .nfire — strafe movement.
            b2_add_vecs2pos(self);   // s_add_vecs2pos x
            // s_jmp_Zdistmore x,y,#500,.nminz — hold minimum distance.
            if (player && player->active &&
                abs(self->worldz - player->worldz) <= 500) {
                self->worldz = (int16)(self->worldz - self->vz);
            }
        }

        // .stop
        self->roty = (uint8)(self->roty + 2u);
        self->sflags2 |= BOSS2_SFLAG1;   // petals: open
        self->sbyte3 = 4u;               // petals: fully open
        self->roty = (uint8)(self->roty + 2u);

        // s_jmp_alsflag x,sflag3,.nsnd3 / trigse $71
        if ((self->sflags2 & BOSS2_SFLAG3) == 0u) {
            Sound_PlaySE(B2_SE_SPIN);
        }
        self->sflags2 |= BOSS2_SFLAG3;
        // s_clr_alsflag x,colldisable (30/9/92)
        self->sflags &= (uint8)~ASF_COLLDISABLE;

        // Circle-strafe velocity from sin/cos tables (SGDATA.ASM sintab is a
        // signed-byte table, amplitude 127):
        //   s_set_alvar2alvartab ... al_vx,sintab,-3 / al_vz,costab,-1
        self->vx = (int16)((int16)(b2_sin(self->sbyte2) * 127.0f) >> 3);
        self->vz = (int16)((int16)(b2_cos(self->sbyte2) * 127.0f) >> 1);
        // s_add_alvar B,x,al_sbyte2,#4
        self->sbyte2 = (uint8)(self->sbyte2 + 4u);

        // s_set_objtobechild y,x,#1 / s_jmp_objptrok y,.ns4 — top alive?
        top = b2_find_child_obj(self, BOSS2_CHILD_TOP);
        if (!top) {
            // s_next_state x / s_set_vecs x,#0,#10,#30
            self->stratstate++;
            self->vx = 0;
            self->vy = 10;
            self->vz = 30;
        }
    }

    // ---------------------------------------------------------------
    if (self->stratstate == 5u) {
        // s_jmp_ifplayeralive .dodie
        if ((g_pshipflags2 & PSF2_PLAYERHP0) == 0u) {
            Alien *exp;

            // .dodie
            boss2_boss_dying();   // s_boss_dying

            // s_falldown_Yvec x,1,#1,#-30<<boss2_scale,kill_Istrat
            if (boss2_falldown_yvec(self, 1, 1, B2U(-30))) {
                // kill_Istrat: s_kill_obj — detonate via boss2exp_Istrat.
                boss2exp_init(self);
                return;
            }

            // s_jsl makeLexpobj_srou — continuous large explosions while
            // toppling.
            exp = b2_make_large_exp_obj(self);
            if (exp) {
                exp->sflags4 |= ASF4_NOPOLYEXP;   // s_set_alsflag y,nopolyexp
                b2_addrnd2pos_xy(exp);            // s_jsl addrnd2posy_srou
                exp->worldy = (int16)(exp->worldy + B2U(15));
                exp->vy = -20;                    // s_set_alvar W,y,al_vy,#-20
                exp->count = 1u;                  // s_set_lifecnt y,#1
            }

            b2_add_player_z(self);   // s_add_playerZ x
            b2_add_vecs2pos(self);   // s_add_vecs2pos x

            // s_jmp_NOTdelay 1,.nf / s_set_alsflag x,hitflash
            if ((g_gameframe & 1u) == 0u) {
                self->sflags |= ASF_HITFLASH;
            }
        } else {
            // Player is dead — just keep falling, don't finish the boss.
            (void)boss2_falldown_yvec(self, 1, 1, B2U(-30));
            b2_add_player_z(self);
        }
    }

    // .end
    self->roty = (uint8)(self->roty + 2u);   // s_add_alvar B,x,al_roty,#2
}

// ============================================================
// boss2_Istrat — entry point (called by the map executor).
// ============================================================
void Strat_Boss2_Init(Alien *self) {
    Alien *child;

    if (!self) {
        return;
    }

    // Reset boss-progress state on (re)spawn so mapwaitboss can gate on it
    // (same convention as the Strat_Boss1_Init / Strat_Boss7_Init ports).
    g_gameflags &= (uint8)~GF_BOSSDEAD;
    g_bossflags &= (uint8)~BF_DYING;

    // s_set_bossmaxHP #0 — children below accumulate the HUD bar total.
    g_bossmaxhp = 0u;
    g_meters = 1u;

    // s_make_childobj #boss_2_1,#1,boss2top_Istrat,enemy1
    (void)boss2_spawn_child(self, SH_BOSS_2_1_PROXY, BOSS2_CHILD_TOP,
                            boss2top_init);

    // Four petals at 90-degree spacing.
    child = boss2_spawn_child(self, SH_BOSS_2_5_PROXY, BOSS2_CHILD_PETAL0,
                              boss2petal_init);
    if (child) child->sbyte2 = 0u;
    child = boss2_spawn_child(self, SH_BOSS_2_5_PROXY, BOSS2_CHILD_PETAL1,
                              boss2petal_init);
    if (child) child->sbyte2 = DEG90;
    child = boss2_spawn_child(self, SH_BOSS_2_5_PROXY, BOSS2_CHILD_PETAL2,
                              boss2petal_init);
    if (child) child->sbyte2 = DEG180;
    child = boss2_spawn_child(self, SH_BOSS_2_5_PROXY, BOSS2_CHILD_PETAL3,
                              boss2petal_init);
    if (child) child->sbyte2 = (uint8)(DEG180 + DEG90);

    // Four gun pods at 45-degree offsets (`ifeq 0` block — assembled in the
    // original: IFEQ expr is true when expr == 0).
    child = boss2_spawn_child(self, SH_BOSS_2_0_PROXY, BOSS2_CHILD_TURRET0,
                              boss2turret_init);
    if (child) child->sbyte2 = DEG45;
    child = boss2_spawn_child(self, SH_BOSS_2_0_PROXY, BOSS2_CHILD_TURRET1,
                              boss2turret_init);
    if (child) child->sbyte2 = (uint8)(DEG45 + DEG90);
    child = boss2_spawn_child(self, SH_BOSS_2_0_PROXY, BOSS2_CHILD_TURRET2,
                              boss2turret_init);
    if (child) child->sbyte2 = (uint8)(DEG180 + DEG45);
    child = boss2_spawn_child(self, SH_BOSS_2_0_PROXY, BOSS2_CHILD_TURRET3,
                              boss2turret_init);
    if (child) child->sbyte2 = (uint8)(DEG180 + DEG90 + DEG45);

    // s_set_alptrs x,boss2_strat,0,boss2exp_Istrat
    self->stratptr = boss2_strat;
    self->collstratptr = NULL;
    self->expstratptr = boss2exp_init;
    // s_set_aldata x,#hardHP,#10
    self->HP = HARD_HP;
    self->AP = BOSS2_AP;
    // s_set_colltype x,enemy1 / s_set_colltype x,enemyweap
    self->collflags |= (COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP);
    // s_set_lifecnt x,#50
    self->count = 50u;
    // s_set_alsflag x,colldisable (30/9/92) / s_set_alsflag x,shadow
    self->sflags |= (ASF_COLLDISABLE | ASF_SHADOW);
    self->stratstate = 0u;

    // trigse $95
    Sound_PlaySE(B2_SE_SPAWN);
}

// ============================================================
// Registration
// ============================================================
void StratBoss2_Register(void) {
    // ISTRATS.ASM def_Istrat row 108: boss2 (mb_mapobj IS_BOSS2 in levels.c
    // MAP1_4 line ~9244 and MAP3_5 line ~9727). No synthetic STRAT_ADDR_*
    // is used for boss2, so no World_RegisterStrategyAddress call is needed.
    g_istrats[IS_BOSS2] = (StrategyFunc)Strat_Boss2_Init;
}
