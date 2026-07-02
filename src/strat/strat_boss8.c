// Boss8 — the MAP3_4 "washing machine" wash boss (Sector Z boss).
//
// Literal port of:
//   GB3STRAT.ASM:42-513   boss8_Istrat / boss8wait / boss8a / boss8b /
//                         boss8die_Istrat / boss8cov_Istrat /
//                         nucleusbeamL_Istrat / nucleusbeamcol_Istrat /
//                         nucleusbeam_Istrat / boss8shrap_Istrat
//   GASTRATS.ASM:39-220   nucleuslauncher_Istrat / nucleuspillar_Istrat /
//                         nucleuswallrot_srou_l / nucleuswallrot2_srou_l
//   EXPSTRAT.ASM:451-478  bigexplode_Istrat (+ delayexplode_strat 259-268)
//   GB2STRAT.ASM:633-670  shrapfall_Istrat
//   PISTRATS.ASM:526-532  straight_Istrat
//
// Spawn context (WASHMAP.ASM via src/map/levels.c build_level3_4_slice):
//   mapobj ... boss_8_0,boss8_Istrat            -> STRAT_ADDR 0x060014
//   mapobj ... hou_4,nucleuslauncher_Istrat x4  -> STRAT_ADDR 0x060015
//   mapobj ... boss_8_4,nucleuspillar_Istrat x8 -> STRAT_ADDR 0x060016
//
// Registration is called from Strat_RegisterAll after the istrat tables are
// cleared each (re)load (and before Strat_RegisterAddressMap, so the index
// entries below also pick up flat/synthetic address registration).

#include "strat_boss8.h"

#include <math.h>
#include <stdlib.h>

#include "strat_common.h"
#include "strat_enemy.h"
#include "../game/game_vars.h"
#include "../game/obj.h"
#include "../game/sound.h"
#include "../game/world.h"
#include "../sf_rtl.h"
#include "../variables.h"

// ============================================================
// Constants (STRATEQU.INC)
// ============================================================
#define BOSS8_SCALE        3                            // boss8_scale
#define BOSS8_HP           32u                          // boss8HP (STRATEQU.INC:275)
#define NUCLEUS_LAUNCH_AP  8u                           // nucleusLaunchAP
#define NUCLEUS_BEAML_AP   8u                           // nucleusBeamLAP
#define NUCLEUS_HEIGHT     ((100 / 2) << BOSS8_SCALE)   // nucleusheight = 400
#define NUCLEUS_VIEWCY     (-60)                        // nucleus_viewCY
#define BOSS8_CIRC         (210 << BOSS8_SCALE)         // boss8_circ = 1680

// Shape ids ("MACRO-counted" numbering, see src/renderer/shape_data.h).
#define SH_BOSS_8_5   44u   // nucleus beam emitter
#define SH_HOU_4      45u   // nucleus launcher
#define SH_BOSS_8_4   46u   // nucleus pillar
#define SH_BOSS_8_0   47u   // wash boss shell
// Missing from the compiled shape set — proxied through nullshape (0).
// Do not invent geometry; noted as gaps:
#define SH_BOSS_8_1   264u  // SHAPE_EXT_BOSS_8_1 cover, open variant
#define SH_BOSS_8_1C  265u  // SHAPE_EXT_BOSS_8_1C cover, closed variant
#define SH_SPARKLAS   266u  // SHAPE_EXT_SPARKLAS rotating beam bolt
#define SH_SHRAP1     267u  // SHAPE_EXT_SHRAP1 falling shrapnel
#define SH_SHYPER     268u  // SHAPE_EXT_SHYPER hyper ring
#define SH_ZACO_9     269u  // SHAPE_EXT_ZACO_9 kami homing missile

// def_Istrat indices from ISTRATS.ASM (calibrated against IS_BOSS1=69,
// IS_BOSSA=85, IS_BOSS7=99, IS_BOSSF=116 in strat_table.c):
#define IS_NUCLEUSBEAML     82
#define IS_BOSS8SHRAP       83
#define IS_BOSS8            84
#define IS_NUCLEUSLAUNCHER  86
#define IS_NUCLEUSPILLAR    87

// Synthetic strategy addresses used by src/map/levels.c (washmap slice).
#define B8_STRAT_ADDR_BOSS8            0x060014u
#define B8_STRAT_ADDR_NUCLEUSLAUNCHER  0x060015u
#define B8_STRAT_ADDR_NUCLEUSPILLAR    0x060016u

// Strategy flags (STRATEQU sflag1/sflag2/sflag4/sflag5) mapped onto sflags2.
// sflag1 keeps the codebase-wide ASF2_SFLAG1 bit; the others take free bits.
#define B8_SFLAG1  ASF2_SFLAG1  // 0x10: shell=rotation dir / beam=switched off
#define B8_SFLAG2  0x20u        // shell: "switch wait" (vestigial, see ASM)
#define B8_SFLAG4  0x40u        // shell: cover commanded open (read by cover)
#define B8_SFLAG5  0x80u        // shell: cover fully open (set by cover)

// Projectile "stop chasing when close" marker (like HMISSILE1_NOCHASE_FLAG).
#define B8_SHOT_NOCHASE  0x01u

// Explosion child size markers (match strat_enemy.c EXPSHAPE_*).
#define B8_EXPSHAPE_MEDIUM   2u
#define B8_EXPSHAPE_LARGE    3u
#define B8_EXPSHAPE_FOLARGE  4u

// Sound/BGM ids (match strat_enemy.c boss_dying()).
#define B8_SE_BOSS_DYING   0x1Eu
#define B8_BGM_BOSS_DYING  0xF1u

// Boss8 child object numbers (s_make_childobj arg 2).
#define B8_CHILD_COVER  1u
#define B8_CHILD_BEAM1  2u
#define B8_CHILD_BEAM2  3u
#define B8_CHILD_BEAM3  4u

// ============================================================
// Forward declarations
// ============================================================
static void boss8wait_init(Alien *self);
static void boss8wait_strat(Alien *self);
static void boss8_cont(Alien *self);
static void boss8a_init(Alien *self);
static void boss8a_strat(Alien *self);
static void boss8b_init(Alien *self);
static void boss8b_strat(Alien *self);
static void boss8die_istrat(Alien *self);
static void boss8die_strat(Alien *self);
static void boss8cov_istrat(Alien *self);
static void boss8cov_strat(Alien *self);
static void nucleusbeaml_istrat(Alien *self);
static void nucleusbeaml_strat(Alien *self);
static void nucleusbeamcol_istrat(Alien *self);
static void nucleusbeam_istrat(Alien *self);
static void nucleusbeam_strat(Alien *self);
static void boss8shrap_istrat(Alien *self);
static void boss8shrap_strat(Alien *self);
static void nucleuslauncher_istrat(Alien *self);
static void nucleuslauncher_init(Alien *self);
static void nucleuslauncher_strat(Alien *self);
static void nucleuslauncherfire_strat(Alien *self);
static void nucleuslauncherclose_strat(Alien *self);
static void nuclaunch_cont(Alien *self);
static void nucleuspillar_istrat(Alien *self);
static void nucleuspillar_strat(Alien *self);
static void boss8_bigexplode(Alien *self);
static void boss8_delayexplode_strat(Alien *self);
static void boss8_shrapfall_istrat(Alien *self);
static void boss8_shrapfall_strat(Alien *self);
static void boss8_straight_istrat(Alien *self);
static void boss8_straight_strat(Alien *self);
static void boss8_homing_shot_strat(Alien *self);
static void boss8_kamimissile_strat(Alien *self);

// ============================================================
// Small generic helpers (local copies of strat_enemy.c statics; those are
// file-local there and this file may not edit other translation units).
// ============================================================

// s_add_playerZ macro -> sr_addplayerZx (STRATROU.ASM:3092)
static void b8_add_player_z(Alien *al) {
    al->worldz = (int16)(al->worldz + g_pviewvelz);
}

// s_copy_pos macro
static void b8_copy_pos(Alien *dst, const Alien *src) {
    dst->worldx = src->worldx;
    dst->worldy = src->worldy;
    dst->worldz = src->worldz;
}

// achase (proportional angle chase), same as strat_enemy.c achase_angle
static void b8_achase_angle(uint8 *current, uint8 target, int shift) {
    int8 diff;
    int8 step;

    if (*current == target) {
        return;
    }
    diff = (int8)(*current - target);
    step = (int8)(diff >> shift);
    if (step == 0) {
        step = (diff > 0) ? 1 : -1;
    }
    *current = (uint8)(*current - (uint8)step);
}

static uint16 b8_obj_index_or_null(const Alien *al) {
    if (!al || al < g_aliens || al >= (g_aliens + NUMBER_AL)) {
        return 0u;
    }
    return (uint16)((al - g_aliens) + 1);
}

static Alien *b8_obj_from_index(uint16 index) {
    if (index == 0u) {
        return NULL;
    }
    index--;
    if (index >= NUMBER_AL) {
        return NULL;
    }
    return &g_aliens[index];
}

// pitch angle toward target (matches strat_enemy.c strat_pitch_toward)
static uint8 b8_pitch_toward(const Alien *src, const Alien *dst) {
    float dy;
    float dx;
    float dz;
    float dist;
    float pitch;

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

// s_obj2obj_3dangle equivalent (matches strat_enemy.c strat_aim_3d)
static void b8_aim_3d(Alien *self, const Alien *target, int shift) {
    if (!self || !target) {
        return;
    }
    b8_achase_angle(&self->roty, Strat_AngleXZ(self, target), shift);
    b8_achase_angle(&self->rotx, b8_pitch_toward(self, target), shift);
}

// ============================================================
// Child/mother linkage (same field conventions as strat_enemy.c:
// mother->sword1 = first child index, child->sword1 = next sibling,
// child->ptr = mother index, child->sbyte1 = child number).
// ============================================================
static bool b8_attach_child(Alien *mother, Alien *child, uint8 child_num) {
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
    child->ptr = b8_obj_index_or_null(mother);
    child->sword1 = 0;

    child_idx = b8_obj_index_or_null(child);
    if (child_idx == 0u) {
        child->sflags3 &= (uint8)~ASF3_CHILDOBJ;
        child->ptr = 0u;
        return false;
    }

    if ((uint16)mother->sword1 == 0u) {
        mother->sword1 = (int16)child_idx;
        return true;
    }

    idx = (uint16)mother->sword1;
    guard = NUMBER_AL + 1;
    while (idx != 0u && guard-- > 0) {
        Alien *it = b8_obj_from_index(idx);
        if (!it) {
            break;
        }
        if ((uint16)it->sword1 == 0u) {
            it->sword1 = (int16)child_idx;
            return true;
        }
        idx = (uint16)it->sword1;
    }

    mother->sword1 = (int16)child_idx;
    return true;
}

// s_set_objtobemother
static Alien *b8_get_mother(Alien *self) {
    Alien *mother;

    if (!self || (self->sflags3 & ASF3_CHILDOBJ) == 0u) {
        return NULL;
    }
    mother = b8_obj_from_index((uint16)self->ptr);
    if (!mother || !mother->active) {
        return NULL;
    }
    return mother;
}

// s_set_objtobechild
static Alien *b8_find_child(Alien *mother, uint8 child_num) {
    uint16 idx;
    int guard;
    uint16 mother_idx;

    if (!mother) {
        return NULL;
    }

    mother_idx = b8_obj_index_or_null(mother);
    idx = (uint16)mother->sword1;
    guard = NUMBER_AL + 1;
    while (idx != 0u && guard-- > 0) {
        Alien *child = b8_obj_from_index(idx);
        if (!child) {
            break;
        }
        if (child->active &&
            (child->sflags3 & ASF3_CHILDOBJ) != 0u &&
            (uint16)child->ptr == mother_idx &&
            child->sbyte1 == child_num) {
            return child;
        }
        idx = (uint16)child->sword1;
    }
    return NULL;
}

// s_make_childobj shape,num,strat,colltype
static Alien *b8_spawn_child(Alien *mother, uint16 shape, uint8 child_num,
                             void (*init_fn)(Alien *), uint8 colltype) {
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
    if (!b8_attach_child(mother, child, child_num)) {
        Obj_Free(child);
        return NULL;
    }
    child->collflags |= colltype;
    init_fn(child);
    return child;
}

// ============================================================
// Explosion-object factories (local copies of makeexpobj_srou +
// makeMED/L/FOLexpobj_srou, EXPSTRAT.ASM:327-338).
// ============================================================
static Alien *b8_make_exp_obj(const Alien *parent, uint16 size_shape) {
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
    child->stratptr = boss8_delayexplode_strat;
    child->collstratptr = NULL;
    child->expstratptr = Strat_Explode;
    b8_copy_pos(child, parent);
    child->shape = size_shape;
    return child;
}

// addrnd2posy_srou — add random offset to x/y
static void b8_add_rnd_xy(Alien *al) {
    int8 rx = (int8)(SfRtl_Random() & 0xFFu);
    int8 ry = (int8)(SfRtl_Random() & 0xFFu);
    al->worldx = (int16)(al->worldx + (int16)rx);
    al->worldy = (int16)(al->worldy + (int16)ry);
}

// addrnd2posxyz2_srou — add random offset to x/y/z
static void b8_add_rnd_xyz(Alien *al) {
    int8 rz = (int8)(SfRtl_Random() & 0xFFu);
    b8_add_rnd_xy(al);
    al->worldz = (int16)(al->worldz + (int16)rz);
}

// delayexplode_strat (EXPSTRAT.ASM:259-268)
static void boss8_delayexplode_strat(Alien *self) {
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
        b8_add_player_z(self);
    }
}

// s_boss_dying macro (STRATMAC.INC:7758-7767)
static void b8_boss_dying(void) {
    if ((g_bossflags & BF_DYING) == 0u) {
        Sound_PlaySE(B8_SE_BOSS_DYING);
        Sound_PlayMusic(B8_BGM_BOSS_DYING);
        g_bossflags |= BF_DYING;
        g_pstratflags |= PSTF_NOTDIE;
        g_stratflags |= SF_NOFIRING;
    }
}

// ============================================================
// nucleuswallrot_srou_l / nucleuswallrot2_srou_l (GASTRATS.ASM:157-220)
// al_sword2 = distance, al_sbyte2 = angle.
// rotate (0,dist) by angle; worldx = x2, worldz = z2*2 + zbase + zref;
// then advance the angle by gsvar_byte1 (the shared wall rotation speed).
// ============================================================
static void b8_wallrot_common(Alien *al, int16 zbase, int16 zref) {
    float a = (float)al->sbyte2 * (2.0f * 3.14159265f / 256.0f);
    float dist = (float)al->sword2;
    int16 x2 = (int16)lroundf(dist * sinf(a));
    int16 z2 = (int16)lroundf(dist * cosf(a));

    al->worldx = x2;
    al->worldz = (int16)((z2 * 2) + zbase + zref);
    al->sbyte2 = (uint8)(al->sbyte2 + g_gsvar_byte1);
}

// nucleuswallrot_srou_l: base 160<<boss8_scale, relative to player_posz
static void b8_wallrot(Alien *al) {
    b8_wallrot_common(al, (int16)(160 << BOSS8_SCALE), g_player_posz);
}

// nucleuswallrot2_srou_l: base 210<<boss8_scale, relative to pviewposz
static void b8_wallrot2(Alien *al) {
    b8_wallrot_common(al, (int16)(210 << BOSS8_SCALE), g_pviewposz);
}

// ============================================================
// Projectiles
// ============================================================

// s_fire_weapon x,HPLASMA with al_ptr = playpt (homing plasma).
// Modeled on the boss7/boss1 HPLASMA port in strat_enemy.c.
static Alien *b8_fire_hplasma(Alien *self, Alien *target) {
    Alien *shot;

    if (!self || !target || !target->active) {
        return NULL;
    }
    shot = Strat_MakeObj(0u);
    if (!shot) {
        return NULL;
    }
    b8_copy_pos(shot, self);
    shot->stratptr = boss8_homing_shot_strat;
    shot->collstratptr = Strat_ProjectileOnCollide;
    shot->expstratptr = Strat_ProjectileOnCollide;
    shot->HP = 1u;
    shot->AP = 10u;    // HPLASMA_AP
    shot->vel = 60u;   // HPLASMA_SPEED
    shot->count = 50u; // HPLASMA_LIFE
    shot->snd2 = 6u;
    shot->type = (uint8)(ATLASER | ATZREMOVE);
    shot->sflags |= ASF_INVISIBLE;
    shot->collflags = (uint8)(ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4);
    shot->immuneptr = b8_obj_index_or_null(self);
    shot->fireobjptr = b8_obj_index_or_null(target); // al_ptr = playpt
    shot->sbyte1 = Strat_AngleXZ(shot, target);
    shot->sbyte2 = b8_pitch_toward(shot, target);
    return shot;
}

// homingflat-style tick for the shell's HPLASMA shots
static void boss8_homing_shot_strat(Alien *self) {
    Alien *target;
    Alien *player;
    int16 dz;

    if (!self) {
        return;
    }

    target = (self->fireobjptr != 0u)
                 ? Obj_GetByIndex((int)(self->fireobjptr - 1u))
                 : NULL;
    if (target && target->active &&
        abs(self->worldz - target->worldz) >= 500) {
        b8_achase_angle(&self->sbyte1, Strat_AngleXZ(self, target), 4);
        b8_achase_angle(&self->sbyte2, b8_pitch_toward(self, target), 4);
    }
    self->roty = self->sbyte1;
    self->rotx = self->sbyte2;
    Strat_GenVecs3D(self);
    b8_add_player_z(self);
    Strat_ApplyVelocity(self);

    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        g_aldead = 1u;
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }
    dz = (int16)(self->worldz - player->worldz);
    if (dz < -12000 || dz > 12000) {
        g_aldead = 1u;
        return;
    }
    if ((g_gameflags & GF_BOSSDEAD) != 0u || (g_bossflags & BF_DYING) != 0u) {
        self->sflags |= ASF_COLLDISABLE;
        self->HP = 0u;
        g_aldead = 1u;
    }
}

// s_fire_weapon x,KAMIHMISSILE1 (nucleus launcher's homing missile,
// zaco_9 body). Modeled on the HMISSILE1 port in strat_enemy.c.
static Alien *b8_fire_kamimissile(Alien *self, Alien *target) {
    Alien *shot;

    if (!self || !target || !target->active) {
        return NULL;
    }
    shot = Strat_MakeObj(SH_ZACO_9);
    if (!shot) {
        return NULL;
    }
    b8_copy_pos(shot, self);
    // ASM pushes al_roty, forces #deg180 for the launch, then restores.
    shot->roty = DEG180;
    shot->rotx = 0u;
    shot->stratptr = boss8_kamimissile_strat;
    shot->collstratptr = Strat_ProjectileOnCollide;
    shot->expstratptr = Strat_ProjectileOnCollide;
    shot->HP = 2u;
    shot->AP = 8u;      // HMISSILE1_AP
    shot->vel = 60u;    // HMISSILE1_SPEED
    shot->count = 100u; // HMISSILE1_LIFE
    shot->snd2 = 2u;
    shot->type = (uint8)(ATMISSILE | ATZREMOVE);
    shot->sflags |= ASF_SHADOW;
    shot->collflags = (uint8)(ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4);
    shot->immuneptr = b8_obj_index_or_null(self);
    shot->fireobjptr = b8_obj_index_or_null(target); // al_ptr = playpt
    shot->sflags2 |= B8_SFLAG1;                      // s_set_alsflag y,sflag1
    return shot;
}

static void boss8_kamimissile_strat(Alien *self) {
    Alien *target;
    Alien *player;
    int16 dz;

    if (!self) {
        return;
    }

    self->rotz = (uint8)(self->rotz + 10u);

    if ((self->sflags2 & B8_SHOT_NOCHASE) == 0u) {
        target = (self->fireobjptr != 0u)
                     ? Obj_GetByIndex((int)(self->fireobjptr - 1u))
                     : NULL;
        if (target && target->active) {
            if ((abs(self->worldx - target->worldx) +
                 abs(self->worldy - target->worldy) +
                 abs(self->worldz - target->worldz)) < 300) {
                self->sflags2 |= B8_SHOT_NOCHASE;
            } else {
                b8_aim_3d(self, target, 3);
            }
        }
        Strat_GenVecs3D(self);
    }

    b8_add_player_z(self);
    Strat_ApplyVelocity(self);

    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        g_aldead = 1u;
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }
    dz = (int16)(self->worldz - player->worldz);
    if (dz < -12000 || dz > 12000) {
        g_aldead = 1u;
        return;
    }
    if ((g_gameflags & GF_BOSSDEAD) != 0u || (g_bossflags & BF_DYING) != 0u) {
        self->sflags |= ASF_COLLDISABLE;
        self->HP = 0u;
        g_aldead = 1u;
    }
}

// zaco_9 live-count gate (GASTRATS.ASM find_shape_l/count_shapes_l calls):
// the launcher only fires while fewer than 1 (easy) / 2 (hard) of its
// missiles are alive. HD counts by strategy instead of by shape id.
static uint8 b8_count_kamimissiles(void) {
    uint8 n = 0u;
    int i;

    for (i = 0; i < NUMBER_AL; i++) {
        if (g_aliens[i].active &&
            g_aliens[i].stratptr == boss8_kamimissile_strat) {
            n++;
        }
    }
    return n;
}

// ============================================================
// straight_Istrat / straight_strat (PISTRATS.ASM:526-532)
// Used by the death-sequence Shyper rings.
// ============================================================
static void boss8_straight_istrat(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = boss8_straight_strat;
    Strat_GenVecs3D(self); // s_gen_3dvecs x,al_roty,al_rotx,al_vel,2
    boss8_straight_strat(self);
}

static void boss8_straight_strat(Alien *self) {
    if (!self) {
        return;
    }
    Strat_ApplyVelocity(self); // s_add_vecs2pos
    b8_add_player_z(self);     // s_add_playerZ
}

// ============================================================
// WASHING MACHINE MAP BOSS (GB3STRAT.ASM:42-81)
// ============================================================

// boss8_Istrat
void Strat_Boss8_Init(Alien *self) {
    uint8 hp;
    Alien *c;

    if (!self) {
        return;
    }

    // s_set_aldata x,#boss8HP,#hardAP / s_set_bossmaxHP #boss8HP
    // s_jmp_iflevel 1,.easy — level 1 keeps base HP, others double it.
    hp = (uint8)BOSS8_HP;
    if (g_currentlevel != 1u) {
        hp = (uint8)(BOSS8_HP * 2u);
    }
    self->HP = hp;
    self->AP = HARD_AP;
    g_gameflags &= (uint8)~GF_BOSSDEAD;
    g_bossflags &= (uint8)~(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3);
    g_bossmaxhp = hp;
    g_meters = 1u;

    // levels.c spawns the shell with a nullshape proxy and an approximated
    // nucleusheight; normalize to the WASHMAP.ASM values here.
    self->shape = SH_BOSS_8_0;
    self->worldy = (int16)((-50 << BOSS8_SCALE) + NUCLEUS_HEIGHT); // = 0

    // s_set_alptrs x,boss8wait_strat,0,boss8die_Istrat
    self->stratptr = boss8wait_strat;
    self->collstratptr = NULL;
    self->expstratptr = boss8die_istrat;

    // s_make_childobj #boss_8_1c,#1,boss8cov_Istrat,enemy2
    (void)b8_spawn_child(self, SH_BOSS_8_1C, B8_CHILD_COVER,
                         boss8cov_istrat, COLLTYPE_ENEMY2);

    // s_make_childobj #boss_8_5,#2..#4,nucleusbeamL_Istrat,enemy2
    c = b8_spawn_child(self, SH_BOSS_8_5, B8_CHILD_BEAM1,
                       nucleusbeaml_istrat, COLLTYPE_ENEMY2);
    if (c) {
        c->worldy = (int16)((-75 << BOSS8_SCALE) + NUCLEUS_HEIGHT);
        c->worldz = (int16)BOSS8_CIRC;
        c->sbyte2 = (uint8)(DEG45 + DEG22);
    }
    c = b8_spawn_child(self, SH_BOSS_8_5, B8_CHILD_BEAM2,
                       nucleusbeaml_istrat, COLLTYPE_ENEMY2);
    if (c) {
        c->worldy = (int16)((-35 << BOSS8_SCALE) + NUCLEUS_HEIGHT);
        c->worldz = (int16)BOSS8_CIRC;
        c->sbyte2 = (uint8)(DEG180 + DEG22);
    }
    c = b8_spawn_child(self, SH_BOSS_8_5, B8_CHILD_BEAM3,
                       nucleusbeaml_istrat, COLLTYPE_ENEMY2);
    if (c) {
        c->worldy = (int16)((-75 << BOSS8_SCALE) + NUCLEUS_HEIGHT);
        c->worldz = (int16)BOSS8_CIRC;
        c->sbyte2 = (uint8)(0u - (DEG45 + DEG22));
    }

    // s_set_colltype x,enemy2 / s_set_colltype x,enemyweap
    self->collflags |= (uint8)(COLLTYPE_ENEMY2 | COLLTYPE_ENEMYWEAP);

    self->sbyte4 = 150u;                              // rotation flip timer
    self->sflags2 &= (uint8)~(B8_SFLAG1 | B8_SFLAG2); // speed up / wait on
    g_gsvar_byte1 = 0u;                               // wall rotation speed
    self->animframe = 0u;                             // s_init_anim x,#0

    // HD substitute for the cover colbox: while the cover is closed the
    // shell cannot be hit (the SNES relied on boss_8_1's collision box
    // physically blocking shots; the cover child still rams the player).
    self->sflags |= ASF_COLLDISABLE;

    boss8_cont(self); // s_brl boss8_cont
}

// boss8wait_init (GB3STRAT.ASM:85)
static void boss8wait_init(Alien *self) {
    self->stratptr = boss8wait_strat;
    boss8wait_strat(self);
}

// boss8wait_strat (GB3STRAT.ASM:87-100)
// Wait until every surviving beam emitter has been switched off (sflag1
// set) — then open up (boss8a_init).
static void boss8wait_strat(Alien *self) {
    Alien *c;

    if (!self) {
        return;
    }

    c = b8_find_child(self, B8_CHILD_BEAM1);
    if (c && (c->sflags2 & B8_SFLAG1) == 0u) { // s_jmpNOT_alsflag y,sflag1,.cont
        boss8_cont(self);
        return;
    }
    c = b8_find_child(self, B8_CHILD_BEAM2);
    if (c && (c->sflags2 & B8_SFLAG1) == 0u) {
        boss8_cont(self);
        return;
    }
    c = b8_find_child(self, B8_CHILD_BEAM3);
    if (!c || (c->sflags2 & B8_SFLAG1) != 0u) { // objptrbad / sflag1 -> open
        boss8a_init(self);
        return;
    }
    boss8_cont(self);
}

// boss8_cont (GB3STRAT.ASM:108-132) — shared per-frame tail.
static void boss8_cont(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_alvar W,x,al_worldz,#210<<boss8_scale / s_add_alvar player_posz
    self->worldz = (int16)((210 << BOSS8_SCALE) + g_player_posz);

    // s_decbne_alvar B,x,al_sbyte4,.nchg — flip rotation dir every 150 frames
    self->sbyte4--;
    if (self->sbyte4 == 0u) {
        self->sbyte4 = 150u;
        self->sflags2 ^= B8_SFLAG1; // s_not_alsflag x,sflag1
    }

    // s_jmp_notdelay 3,.donespeed — ease gsvar_byte1 toward +/-5 every 8 frames
    if ((g_gameframe & 7u) == 0u) {
        if ((self->sflags2 & B8_SFLAG1) != 0u) {
            if ((int8)g_gsvar_byte1 != -5) { // s_jmp_varEQ B,gsvar_byte1,#-5
                g_gsvar_byte1--;
            }
        } else {
            if ((int8)g_gsvar_byte1 != 5) { // s_jmp_varEQ B,gsvar_byte1,#5
                g_gsvar_byte1++;
            }
        }
    }

    // s_add_bossHP x,al_hp — HUD boss bar accumulation. The HD HUD reads
    // g_bossmaxhp directly (src/game/nmi.c), so there is nothing to do.
}

// boss8a_init (GB3STRAT.ASM:134-139) — open the shell.
static void boss8a_init(Alien *self) {
    self->stratptr = boss8a_strat;
    self->collstratptr = Strat_HitFlash;     // s_set_collstrat x,hitflash_Istrat
    self->sflags &= (uint8)~ASF_COLLDISABLE; // HD: shell shootable while open
    self->sbyte2 = 100u;
    self->animframe = 0u; // s_init_anim x,#0
    Sound_PlaySE(0x73u);  // trigse $73
    boss8a_strat(self);
}

// boss8a_strat (GB3STRAT.ASM:140-171)
static void boss8a_strat(Alien *self) {
    Alien *c;
    uint8 frame;

    if (!self) {
        return;
    }

    self->sflags2 |= B8_SFLAG4; // cover: open

    // s_jmpNOT_alsflag x,sflag5,.nanim / s_add_anim x,#1,#15
    if ((self->sflags2 & B8_SFLAG5) != 0u) {
        self->animframe = (uint8)((self->animframe + 1u) % 15u);
    }

    // s_jmp_onframe 25,31 / s_jmpNOT_onframe 30,31 — homing plasma volleys
    frame = (uint8)(g_gameframe & 31u);
    if (frame == 25u || frame == 30u) {
        Alien *player = Obj_GetPlayer();
        if (player && player->active) {
            // s_weapon_rot #0,#deg180 / s_fire_weapon x,HPLASMA /
            // s_set_alvar W,y,al_ptr,playpt
            (void)b8_fire_hplasma(self, player);
        }
    }

    // s_jmp_iflevel 1,boss8_cont — easy route stays open forever
    if (g_currentlevel == 1u) {
        boss8_cont(self);
        return;
    }

    // s_beqdec_alvar B,x,al_sbyte2,boss8b_init
    if (self->sbyte2 == 0u) {
        boss8b_init(self);
        return;
    }
    self->sbyte2--;

    // Close early if any surviving beam has been switched back on
    // (sflag1 cleared): children #2..#4.
    c = b8_find_child(self, B8_CHILD_BEAM1);
    if (c && (c->sflags2 & B8_SFLAG1) == 0u) {
        boss8b_init(self);
        return;
    }
    c = b8_find_child(self, B8_CHILD_BEAM2);
    if (c && (c->sflags2 & B8_SFLAG1) == 0u) {
        boss8b_init(self);
        return;
    }
    c = b8_find_child(self, B8_CHILD_BEAM3);
    if (c && (c->sflags2 & B8_SFLAG1) == 0u) {
        boss8b_init(self);
        return;
    }

    boss8_cont(self);
}

// boss8b_init (GB3STRAT.ASM:174-191) — close the shell.
static void boss8b_init(Alien *self) {
    Alien *c;
    uint8 n;

    self->collstratptr = NULL;       // s_set_collstrat x,0
    self->sflags |= ASF_COLLDISABLE; // HD cover-shield substitute
    self->stratptr = boss8b_strat;
    self->sbyte2 = 15u;

    // Beams switch back on: clear sflag1 on children #2..#4.
    for (n = B8_CHILD_BEAM1; n <= B8_CHILD_BEAM3; n++) {
        c = b8_find_child(self, n);
        if (c) {
            c->sflags2 &= (uint8)~B8_SFLAG1;
        }
    }

    Sound_PlaySE(0x72u); // trigse $72
    boss8b_strat(self);
}

// boss8b_strat (GB3STRAT.ASM:193-204)
static void boss8b_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->sflags2 &= (uint8)~B8_SFLAG4; // cover: close

    // s_jmp_alsflag x,sflag5,.nanim / s_init_anim x,#0
    if ((self->sflags2 & B8_SFLAG5) == 0u) {
        self->animframe = 0u;
    }

    // s_beqdec_alvar B,x,al_sbyte2,boss8wait_init
    if (self->sbyte2 == 0u) {
        boss8wait_init(self);
        return;
    }
    self->sbyte2--;

    boss8_cont(self);
}

// ============================================================
// boss8die_Istrat / boss8die_strat (GB3STRAT.ASM:208-264)
// Runs as expstratptr once HP reaches 0 — the death path MUST raise
// GF_BOSSDEAD (and GF_STAGEDONE) so the washmap script can proceed.
// ============================================================
static void boss8die_istrat(Alien *self) {
    if (!self) {
        return;
    }

    self->expstratptr = boss8die_strat; // s_set_expstrat x,boss8die_strat
    g_gameflags |= GF_BOSSDEAD; // s_or_var B,gameflags,#gf_bossdead
    self->sbyte2 = 30u;
    g_pshipflags |= (uint8)(PSF_NOCTRL | PSF_NOFIRE); // s_playerctrl off
    g_gameflags |= GF_STAGEDONE; // s_or_var B,gameflags,#gf_stagedone
    b8_boss_dying();             // s_boss_dying
    g_bossmaxhp = 0u; // hide the HUD boss bar during the death run
    boss8die_strat(self);
}

static void boss8die_strat(Alien *self) {
    Alien *player;
    Alien *e;

    if (!self) {
        return;
    }

    // Pull the player camera toward the nucleus center.
    player = Obj_GetPlayer();
    if (player && player->active) {
        // s_achase_alvar.w W,y,al_worldy,#nucleus_viewCY+20,3
        player->worldy = Strat_ChaseProportional(
            player->worldy, (int16)(NUCLEUS_VIEWCY + 20), 3);
        // s_achase_alvar.w W,y,al_worldx,#0,3
        player->worldx = Strat_ChaseProportional(player->worldx, 0, 3);
    }

    // s_jmp_notdelay 1,.nexp — explosion barrage every other frame
    if ((g_gameframe & 1u) == 0u) {
        self->sflags |= ASF_HITFLASH;

        e = b8_make_exp_obj(self, B8_EXPSHAPE_MEDIUM); // makeMEDexpobj_srou
        if (e) {
            b8_add_rnd_xy(e);               // addrnd2posy_srou
            e->sflags4 |= ASF4_NOPOLYEXP;   // s_set_alsflag y,nopolyexp
            if ((g_gameframe & 3u) == 0u) { // s_jmp_notdelay 2,.nsnd
                e->sflags2 &= (uint8)~ASF2_NOEXPSND;
            }
        }

        e = b8_make_exp_obj(self, B8_EXPSHAPE_LARGE); // makeLexpobj_srou
        if (e) {
            b8_add_rnd_xy(e);
            e->sflags4 |= ASF4_NOPOLYEXP;
            if ((g_gameframe & 3u) == 0u) { // s_jmp_notdelay 2,.nsnd1
                e->sflags2 &= (uint8)~ASF2_NOEXPSND;
            }
        }

        // s_make_obj #Shyper — hyper ring racing at the camera.
        e = Strat_MakeObj(SH_SHYPER);
        if (e) {
            e->sflags |= ASF_COLLDISABLE; // s_set_alsflag y,colldisable
            e->stratptr = boss8_straight_istrat; // straight_Istrat
            e->vel = 40u;                        // s_set_speed y,#40
            b8_copy_pos(e, self);                // s_copy_pos y,x
            e->worldy = g_pviewposy;             // al_worldy = viewposy
            e->roty = DEG180;
            e->rotz = (uint8)SfRtl_Random(); // s_set_alvar2rnd y,al_rotz
        }
    }

    // s_decbne_alvar B,x,al_sbyte2,boss8_cont
    self->sbyte2--;
    if (self->sbyte2 != 0u) {
        boss8_cont(self);
        return;
    }

    // s_make_obj #nullshape / s_set_strat y,boss8shrap_Istrat
    e = Strat_MakeObj(0u);
    if (e) {
        boss8shrap_istrat(e);
    }

    boss8_bigexplode(self); // s_jmp bigexplode_Istrat
}

// bigexplode_Istrat (EXPSTRAT.ASM:451-478)
static void boss8_bigexplode(Alien *self) {
    Alien *e;
    uint8 i;

    if (!self) {
        return;
    }

    for (i = 0u; i < 5u; i++) {
        e = b8_make_exp_obj(self, B8_EXPSHAPE_LARGE); // makeLexpobj_srou x5
        if (e) {
            e->sflags4 |= ASF4_NOPOLYEXP;
            b8_add_rnd_xy(e);           // addrnd2posy_srou
            e->count = (uint8)(i + 1u); // s_set_lifecnt y,#1..#5
            if (i == 1u || i == 3u) {   // s_clr_alsflag y,noexpsnd (2nd, 4th)
                e->sflags2 &= (uint8)~ASF2_NOEXPSND;
            }
        }
    }

    // s_set_lifecnt x,#4 / s_set_alsflag x,relexplode / s_jmp delayexplode
    self->count = 4u;
    self->sflags2 |= ASF2_RELEXPLODE;
    self->expstratptr = boss8_delayexplode_strat;
    self->stratptr = boss8_delayexplode_strat;
    self->collstratptr = NULL;
}

// ============================================================
// boss8cov_Istrat / boss8cov_strat (GB3STRAT.ASM:269-309)
// The rotating cover shield in front of the shell.
// ============================================================
static void boss8cov_istrat(Alien *self) {
    Alien *mother;

    if (!self) {
        return;
    }

    // s_set_alptrs x,boss8cov_strat,0,explode_Istrat
    self->stratptr = boss8cov_strat;
    self->collstratptr = NULL;
    self->expstratptr = Strat_Explode;
    // s_set_aldata x,#hardHP,#10
    self->HP = HARD_HP;
    self->AP = 10u;
    // s_set_objtobemother / s_copy_pos x,y
    mother = b8_get_mother(self);
    if (mother) {
        b8_copy_pos(self, mother);
    }
    self->worldy = (int16)NUCLEUS_HEIGHT; // al_worldy = #nucleusheight
    self->collflags |= COLLTYPE_ENEMYWEAP; // s_set_colltype x,enemyweap
    self->animframe = 0u;                  // s_init_anim x,#0
}

static void boss8cov_strat(Alien *self) {
    Alien *mother;

    if (!self) {
        return;
    }

    mother = b8_get_mother(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    if ((mother->sflags2 & B8_SFLAG4) != 0u) {
        // .open — swap to the colboxed shape and slide the cover open.
        self->shape = SH_BOSS_8_1; // s_set_alvar W,x,al_shape,#BOSS_8_1
        if (self->animframe != 17u) {
            self->animframe = (uint8)((self->animframe + 1u) % 18u);
        } else {
            mother->sflags2 |= B8_SFLAG5; // fully open
        }
    } else {
        // close thing.
        if (self->animframe != 0u) {
            self->animframe--; // s_add_anim x,#-1,#18
        } else {
            self->shape = SH_BOSS_8_1C; // s_set_alvar W,x,al_shape,#BOSS_8_1c
            mother->sflags2 &= (uint8)~B8_SFLAG5; // closed
        }
    }

    // s_add_alvar B,x,al_roty,gsvar_byte1
    self->roty = (uint8)(self->roty + g_gsvar_byte1);
    // s_set_alvar W,x,al_worldz,#210<<boss8_scale + player_posz
    self->worldz = (int16)((210 << BOSS8_SCALE) + g_player_posz);
}

// ============================================================
// nucleusbeamL_Istrat / nucleusbeamL_strat / nucleusbeamcol_Istrat
// (GB3STRAT.ASM:358-424) — the three beam-switch emitters.
// ============================================================
static void nucleusbeaml_istrat(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_alptrs x,nucleusbeamL_strat,nucleusbeamcol_Istrat,explode_Istrat
    self->stratptr = nucleusbeaml_strat;
    self->collstratptr = nucleusbeamcol_istrat;
    self->expstratptr = Strat_Explode;
    // s_set_aldata x,#hardHP,#nucleusBeamLAP
    self->HP = HARD_HP;
    self->AP = NUCLEUS_BEAML_AP;
    self->sword2 = (int16)BOSS8_CIRC;      // wall radius
    self->type &= (uint8)~ATZREMOVE;       // s_setnoremove_behind
    self->collflags |= COLLTYPE_ENEMYWEAP; // s_set_colltype x,enemyweap
    self->sbyte3 = 50u;                    // delay
    self->sflags |= ASF_NOHITAFFECT;       // s_set_alsflag x,nohitaffect
}

static void nucleusbeaml_strat(Alien *self) {
    if (!self) {
        return;
    }

    // s_beqdec_alvar B,x,al_sbyte4,.col — invulnerability window after a
    // switch toggle (sbyte4 set to 10 by the collision strat).
    if (self->sbyte4 == 0u) {
        self->sflags &= (uint8)~ASF_COLLDISABLE; // .col
    } else {
        self->sbyte4--;
        self->sflags |= ASF_COLLDISABLE;
    }

    // s_jmp_alvarNE B,x,al_sbyte3,#2,.nsnd — beam warm-up sound
    if (self->sbyte3 == 2u) {
        Sound_PlaySE(0x71u); // trigse $71
    }

    // s_decbne_alvar B,x,al_sbyte3,.nfire
    self->sbyte3--;
    if (self->sbyte3 != 0u) {
        self->colframe = 4u; // .nfire: s_init_colanim x,#4
    } else {
        self->sbyte3 = 1u;
        self->sflags &= (uint8)~ASF_NOHITAFFECT; // s_clr_alsflag x,nohitaffect

        // s_jmp_varAND B,gameflags,#gf_bossdead,explode_Istrat
        if ((g_gameflags & GF_BOSSDEAD) != 0u) {
            Strat_Explode(self);
            return;
        }

        if ((self->sflags2 & B8_SFLAG1) != 0u) {
            self->colframe = 4u; // switched off -> .nfire
        } else {
            // s_add_colanim x,#1,#4
            self->colframe = (uint8)((self->colframe + 1u) & 3u);

            // s_jmp_notdelay 2,.fire — emit a beam bolt every 4 frames
            if ((g_gameframe & 3u) == 0u) {
                Alien *e = Strat_MakeObj(SH_SPARKLAS); // s_make_obj #sparklas
                if (e) {
                    e->sbyte2 = self->sbyte2; // copy angle (al_sbyte2)
                    e->sword2 = self->sword2; // copy radius (al_sword2)
                    nucleusbeam_istrat(e); // s_set_strat y,nucleusbeam_Istrat
                    b8_copy_pos(e, self);  // s_copy_pos y,x
                    e->roty = self->roty;  // copy al_roty
                }
            }
        }
    }

    // .fire: track the rotating wall.
    b8_wallrot(self); // s_jsl nucleuswallrot_srou_l
    self->roty = (uint8)(self->sbyte2 + DEG180); // roty = angle + deg180
}

// nucleusbeamcol_Istrat (GB3STRAT.ASM:410-424) — collision toggles the
// beam switch on/off.
static void nucleusbeamcol_istrat(Alien *self) {
    if (!self) {
        return;
    }

    self->sflags &= (uint8)~ASF_COLLIDE; // s_clr_alsflag x,collide
    self->sflags |= ASF_HITFLASH;        // s_set_alsflag x,hitflash
    self->sflags2 ^= B8_SFLAG1;          // s_not_alsflag x,sflag1
    if ((self->sflags2 & B8_SFLAG1) != 0u) {
        Sound_PlaySE(0x70u); // .on trigse $70
    } else {
        Sound_PlaySE(0x71u); // .off trigse $71
    }
    self->sbyte4 = 10u;

    // s_jmp_iflevel 1,explode_Istrat — easy route: one hit destroys it.
    if (g_currentlevel == 1u) {
        Strat_Explode(self);
        return;
    }
    nucleusbeaml_strat(self); // s_jmpto_strat
}

// ============================================================
// nucleusbeam_Istrat / nucleusbeam_strat (GB3STRAT.ASM:428-445)
// The actual sweeping beam bolts spawned by the emitters.
// ============================================================
static void nucleusbeam_istrat(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_alptrs x,nucleusbeam_strat,0,0
    self->stratptr = nucleusbeam_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    // s_set_aldata x,#hardHP,#hardAP
    self->HP = HARD_HP;
    self->AP = HARD_AP;
    // s_set_colltype laser + enemyweap + enemy2: it damages the player but
    // ignores player lasers. HD uses the enemy-projectile bit set.
    self->type |= ATLASER;
    self->type &= (uint8)~ATZREMOVE; // s_setnoremove_behind
    self->collflags = (uint8)(ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4);
}

static void nucleusbeam_strat(Alien *self) {
    if (!self) {
        return;
    }

    b8_wallrot2(self); // s_jsl nucleuswallrot2_srou_l
    self->roty = (uint8)(self->sbyte2 + DEG180);

    // s_sub_alvar W,x,al_sword2,#50 — sweep inward
    self->sword2 = (int16)(self->sword2 - 50);
    // s_jmp_alvarless W,x,al_sword2,#100<<boss8_scale,remove_Istrat
    if (self->sword2 < (int16)(100 << BOSS8_SCALE)) {
        g_aldead = 1u;
    }
}

// ============================================================
// boss8shrap_Istrat / boss8shrap_strat (GB3STRAT.ASM:450-506)
// Death-sequence driver: shrapnel rain, follow explosions, BG shake.
// ============================================================
static void boss8shrap_istrat(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_alptrs x,boss8shrap_strat,0,0
    self->stratptr = boss8shrap_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    // s_set_aldata x,#hardHP,#hardAP
    self->HP = HARD_HP;
    self->AP = HARD_AP;
    self->sflags |= ASF_COLLDISABLE; // s_set_alsflag x,colldisable
    self->type &= (uint8)~ATZREMOVE; // s_setnoremove_behind
    self->sbyte1 = 50u;              // s_set_alvar B,x,al_sbyte1,#50
}

static void boss8shrap_strat(Alien *self) {
    Alien *e;

    if (!self) {
        return;
    }

    // Pin to the player's view position (+1000 ahead).
    self->worldx = g_pviewposx;
    self->worldy = g_pviewposy;
    self->worldz = (int16)(g_pviewposz + 1000);

    // s_beqdec_alvar B,x,al_sbyte1,.folexp
    if (self->sbyte1 == 0u) {
        // .folexp: follow-object large explosion at the original position
        e = b8_make_exp_obj(self, B8_EXPSHAPE_FOLARGE); // makeFOLexpobj_srou
        if (e) {
            // s_sub_alvar W,y,al_worldz,#1000
            e->worldz = (int16)(e->worldz - 1000);
            e->sflags4 |= ASF4_NOPOLYEXP;
            b8_add_rnd_xy(e); // s_add_rnd2pos y,127,127,0
        }
    } else {
        self->sbyte1--;
    }

    // s_jmp_notdelay 3,.nshrap — drop shrapnel every 8 frames
    if ((g_gameframe & 7u) == 0u) {
        e = Strat_MakeObj(SH_SHRAP1); // s_make_obj #shrap1
        if (e) {
            boss8_shrapfall_istrat(e); // s_set_strat y,shrapfall_Istrat
        }
    }

    // s_jmp_notdelay 1,.nexp — large explosion every other frame
    if ((g_gameframe & 1u) == 0u) {
        e = b8_make_exp_obj(self, B8_EXPSHAPE_LARGE); // makeLexpobj_srou
        if (e) {
            b8_add_rnd_xyz(e); // addrnd2posxyz2_srou
        }
    }

    // BG shake: bg2Xscroll = rnd&7, bg2Yscroll = (rnd&3)+248
    g_bg2Xscroll = (int16)(SfRtl_Random() & 7u);
    g_bg2Yscroll = (int16)((SfRtl_Random() & 3u) + 248u);
}

// ============================================================
// shrapfall_Istrat / shrapfall_strat (GB2STRAT.ASM:633-670)
// ============================================================
static void boss8_shrapfall_istrat(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = boss8_shrapfall_strat;
    // al_worldy = pviewposy - 500
    self->worldy = (int16)(g_pviewposy - 500);
    self->sflags |= ASF_COLLDISABLE;
    self->count = 26u; // s_set_lifecnt x,#26
    // al_worldx = (rnd8 - 128) + pviewposx
    self->worldx =
        (int16)((int16)(SfRtl_Random() & 0xFFu) - 128 + g_pviewposx);
    // al_sword1 = rnd8 * 2 (per-piece z offset)
    self->sword1 = (int16)((SfRtl_Random() & 0xFFu) << 1);
    self->roty = (uint8)SfRtl_Random(); // s_set_alvar2rnd x,al_roty
}

static void boss8_shrapfall_strat(Alien *self) {
    if (!self) {
        return;
    }

    // al_worldz = pviewposz + 300 + al_sword1
    self->worldz = (int16)(g_pviewposz + 300 + self->sword1);
    // s_add_alvar W,x,al_worldy,#35 — fall
    self->worldy = (int16)(self->worldy + 35);
    // s_dec_lifecnt x — remove when the lifetime expires
    if (Strat_CountDown(self)) {
        g_aldead = 1u;
    }
}

// ============================================================
// nucleuslauncher_Istrat (GASTRATS.ASM:39-129)
// Wall-mounted missile launchers around the wash room.
// ============================================================
static void nucleuslauncher_istrat(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_aldata x,#hardHP,#nucleusLaunchAP
    self->HP = HARD_HP;
    self->AP = NUCLEUS_LAUNCH_AP;
    self->sword2 = (int16)BOSS8_CIRC; // s_set_alvar W,x,al_sword2,#boss8_circ
    self->type &= (uint8)~ATZREMOVE;  // s_setnoremove_behind
    // s_set_alvar2rnd x,al_sbyte3,#5 — randomized fire delay (avoid 0 so the
    // decrement-based countdown below behaves).
    self->sbyte3 = (uint8)((SfRtl_Random() % 5u) + 1u);
    self->collflags |= COLLTYPE_ENEMYWEAP; // s_set_colltype x,enemyweap

    // WASHMAP.ASM spawn height is (-50<<boss8_scale)+nucleusheight = 0;
    // levels.c uses an approximated nucleusheight, so normalize here.
    self->worldy = (int16)((-50 << BOSS8_SCALE) + NUCLEUS_HEIGHT);
    if (self->shape == 0u) {
        self->shape = SH_HOU_4;
    }

    nucleuslauncher_init(self);
}

// nucleuslauncher_init (GASTRATS.ASM:48-50)
static void nucleuslauncher_init(Alien *self) {
    self->animframe = 0u; // s_init_anim x,#0
    // s_set_alptrs x,nucleuslauncher_strat,hitflash_Istrat,explode_Istrat
    self->stratptr = nucleuslauncher_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
}

// nucleuslauncher_strat (GASTRATS.ASM:51-88)
static void nucleuslauncher_strat(Alien *self) {
    Alien *player;
    uint8 limit;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();

    // Fire gating: the ASM checks s_jmp_objinfront y,x (skipped in HD — the
    // launcher rides the wall around the player, the X-band test dominates),
    // |worldx| in [700,900] and player within 200 on X.
    if (player && player->active &&
        abs(self->worldx) >= 700 && // s_jmp_ABSalvarLESS W,x,al_worldx,#700
        abs(self->worldx) <= 900 && // s_jmp_ABSalvarMORE W,x,al_worldx,#900
        abs(self->worldx - player->worldx) <= 200) { // s_jmp_Xdistmore #200
        // zaco_9 shape-count gate: easy allows 1 live missile, hard 2.
        limit = (g_currentlevel == 1u) ? 1u : 2u;
        if (b8_count_kamimissiles() < limit) {
            // nucleuslauncherfire_init: s_decbne_alvar B,x,al_sbyte3,cont
            self->sbyte3--;
            if (self->sbyte3 == 0u) {
                self->stratptr = nucleuslauncherfire_strat;
                nucleuslauncherfire_strat(self);
                return;
            }
        }
    }

    nuclaunch_cont(self);
}

// nucleuslauncherfire_strat (GASTRATS.ASM:90-111)
static void nucleuslauncherfire_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    // .anim s_add_anim x,#1,#4 / .nanim s_cmp_anim x,#3
    self->animframe = (uint8)((self->animframe + 1u) % 4u);
    if (self->animframe == 3u) {
        // Fire KAMIHMISSILE1 straight inward (roty forced deg180), homing
        // on the player (al_ptr = playpt), missile gets sflag1.
        player = Obj_GetPlayer();
        if (player && player->active) {
            (void)b8_fire_kamimissile(self, player);
        }
        // s_brl nucleuslauncherclose_init
        self->stratptr = nucleuslauncherclose_strat;
        nucleuslauncherclose_strat(self);
        return;
    }

    nuclaunch_cont(self);
}

// nucleuslauncherclose_init/strat (GASTRATS.ASM:113-124)
static void nucleuslauncherclose_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->sbyte3 = 5u; // s_set_alvar B,x,al_sbyte3,#5

    if (self->animframe != 0u) {
        self->animframe--; // s_add_anim x,#-1,#4
    }
    if (self->animframe != 0u) {
        nuclaunch_cont(self);
        return;
    }

    nucleuslauncher_init(self); // s_brl nucleuslauncher_init
    nucleuslauncher_strat(self);
}

// nuclaunch_cont (GASTRATS.ASM:105-110)
static void nuclaunch_cont(Alien *self) {
    b8_wallrot(self); // s_jsl nucleuswallrot_srou_l
    // s_copy_alvar2alvar B,x,al_roty,x,al_sbyte2 / s_add_alvar #deg180
    self->roty = (uint8)(self->sbyte2 + DEG180);
    // s_jmp_varAND B,gameflags,#gf_bossdead,explode_Istrat
    if ((g_gameflags & GF_BOSSDEAD) != 0u) {
        Strat_Explode(self);
    }
}

// ============================================================
// nucleuspillar_Istrat / nucleuspillar_strat (GASTRATS.ASM:132-150)
// Decorative rotating wall pillars.
// ============================================================
static void nucleuspillar_istrat(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_alptrs x,nucleuspillar_strat,0,0
    self->stratptr = nucleuspillar_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    // s_set_aldata x,#hardHP,#hardAP
    self->HP = HARD_HP;
    self->AP = HARD_AP;
    self->sword2 = (int16)BOSS8_CIRC; // s_set_alvar W,x,al_sword2,#boss8_circ
    self->type &= (uint8)~ATZREMOVE;  // s_setnoremove_behind
    self->sflags |= ASF_COLLDISABLE;  // s_set_alsflag x,colldisable
    self->collflags |= COLLTYPE_ENEMYWEAP; // s_set_colltype x,enemyweap
    if (self->shape == 0u) {
        self->shape = SH_BOSS_8_4;
    }
}

static void nucleuspillar_strat(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_alvar W,x,al_worldy,#nucleusheight
    self->worldy = (int16)NUCLEUS_HEIGHT;
    b8_wallrot(self); // s_jsl nucleuswallrot_srou_l
    // s_copy_alvar2alvar B,x,al_roty,x,al_sbyte2
    self->roty = self->sbyte2;
}

// ============================================================
// Registration
// ============================================================
void StratBoss8_Register(void) {
    // ISTRATS.ASM def_Istrat rows (flat/synthetic address forms for these
    // indices are derived by Strat_RegisterAddressMap, which runs right
    // after this):
    g_istrats[IS_NUCLEUSBEAML]    = (StrategyFunc)nucleusbeaml_istrat;
    g_istrats[IS_BOSS8SHRAP]      = (StrategyFunc)boss8shrap_istrat;
    g_istrats[IS_BOSS8]           = (StrategyFunc)Strat_Boss8_Init;
    g_istrats[IS_NUCLEUSLAUNCHER] = (StrategyFunc)nucleuslauncher_istrat;
    g_istrats[IS_NUCLEUSPILLAR]   = (StrategyFunc)nucleuspillar_istrat;

    // Synthetic addresses referenced by the washmap slice in levels.c.
    World_RegisterStrategyAddress(B8_STRAT_ADDR_BOSS8,
                                  (StrategyFunc)Strat_Boss8_Init);
    World_RegisterStrategyAddress(B8_STRAT_ADDR_NUCLEUSLAUNCHER,
                                  (StrategyFunc)nucleuslauncher_istrat);
    World_RegisterStrategyAddress(B8_STRAT_ADDR_NUCLEUSPILLAR,
                                  (StrategyFunc)nucleuspillar_istrat);
}
