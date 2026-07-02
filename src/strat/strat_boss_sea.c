// Sea Zone (MAP2_3) bosses: bossseamon (sea monster) + bossg.
// Registration is called from Strat_RegisterAll after the istrat tables are
// cleared each (re)load.
//
// Literal ports of:
//   GA2STRAT.ASM:3056-3196  bossseamon_Istrat / bossseamon_strat /
//                           bossseamonexp_Istrat
//   GASTRATS.ASM:2046-2140  seamon_Istrat / seamon_strat (small sea monster,
//                           spawned by MAP3_3B via istrat index 81)
//   D2STRATS.ASM:54-495     bossg_istrat / bossgexplode_istrat / bossgs_istrat
//   D3STRATS.ASM:1098-1155  flyingfish_istrat (launched by bossg)
//
// MAP2_3 gating (MAP2_3B.ASM / MAP2_3C.ASM, mirrored by levels.c):
//   - bossg spawns in the 2_3C boss room (STRAT_ADDR_BOSSG = 0x030006).
//   - bossg `.disappear` sets maptrigger bit 0 -> the 2_3B `.waitabit` inline
//     script consumes it and spawns 5 bossseamon (STRAT_ADDR_BOSSSEAMON =
//     0x030005) plus gsvar_byte1 = 5.
//   - each bossseamon death runs bossseamonexp_Istrat which decrements
//     gsvar_byte1; the `.seatest` inline script releases when it hits 0.
//   - bossg `.waitsometime` waits until no sea_0/sea_0_0/sea_0_1 objects
//     remain, then reappears with the HUD boss bar (m_bossmaxHP = bossgHP).
//   - killing bossg runs bossgexplode_istrat which sets maptrigger bit 1
//     (routing 2_3B to `.carryon`) and tail-jumps bossexplode_Istrat, whose
//     38-frame bossdelayexplode sets GF_BOSSDEAD so `mapwaitboss`
//     (world_cb_chkbossdead) releases and `markboss boss23` runs.

#include "strat_boss_sea.h"
#include "strat_common.h"
#include "strat_enemy.h"  // Strat_HitFlash / Strat_Explode / Strat_BossExplode_Init
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

// Synthetic strategy addresses used by src/map/levels.c for the MAP2_3
// spawn rows (mapobj ...,bossSeamon_Istrat / mapobj ...,bossg_istrat).
#define STRAT_ADDR_BOSSSEAMON 0x030005u
#define STRAT_ADDR_BOSSG      0x030006u

// def_istrat indices (ISTRATS.ASM enumeration order).
// Calibrated against IS_BOSSF=116 (ISTRATS.ASM:540) / IS_TORPEDO=80
// (ISTRATS.ASM:503): `def_istrat seamon,sea_0_0` is row 81 (ISTRATS.ASM:504)
// and `def_istrat bossg,boss_g_0` is row 144 (ISTRATS.ASM:570), matching
// g_istrat_shape_defaults[81]=32 (sea_0_0) and [144]=121 (boss_g_0).
#define IS_SEAMON  81u
#define IS_BOSSG  144u

// Flat runtime shape ids (shape_data.h numbering: def_shape MACRO = id 0).
#define SH_SEA_0_0        32u   // def_shape sea_0_0 (compiled mesh)
// `sea_0` (airborne pose) and `sea_0_1` (splash pose) are raw SHAPES symbols
// with no compiled mesh yet; proxy through the surfaced sea_0_0 mesh so the
// monster stays visible.  bossg's find-by-shape wait checks all three ids, so
// the proxy keeps that release condition intact.
#define SH_SEA_0_PROXY    257u  // SHAPE_EXT_SEA_0 (airborne pose)
#define SH_SEA_0_1_PROXY  258u  // SHAPE_EXT_SEA_0_1 (splash pose)
#define SH_BOSS_G_0      121u   // def_shape boss_g_0 (compiled mesh)
// `boss_g_s` (bossg shadow clone) is one of the uncompiled wireframe/Face2
// shapes.  Reserve a stable flat id (>245, past the boss1/boss7/bossA raw-id
// block in strat_enemy.c) so find/remove-by-shape semantics stay literal even
// though nothing renders for it yet.
#define SH_BOSS_G_S      270u  // SHAPE_EXT_BOSS_G_S shadow clone
// `f_fish` has no compiled mesh; levels.c proxies it through `s_fish` (212).
#define SH_F_FISH_PROXY  271u  // SHAPE_EXT_F_FISH
#define SH_NULLSHAPE_WORD  1u   // def_shape nullshape (renders nothing)

// D2STRATS.ASM:27-28
#define BOSSG_HP 120u
#define BOSSG_AP   8u
// GA2STRAT.ASM:3059 s_set_aldata x,#2,#4
#define BOSSSEAMON_HP 2u
#define BOSSSEAMON_AP 4u
// GASTRATS.ASM:2049 s_set_aldata x,#4,#8
#define SEAMON_HP 4u
#define SEAMON_AP 8u
// D3STRATS.ASM:40-41
#define FISH_HP 4u
#define FISH_AP 8u

// STRATEQU.INC:628-629 — bossseamon clamps to bridge_minX*2..bridge_maxX*2.
#define SEA_BRIDGE_MINX2 (-400)
#define SEA_BRIDGE_MAXX2 400

// RELSLOWELASER projectile parameters (match strat_enemy.c's relslowelaser).
#define SEA_ELASER_LIFE 40u
#define SEA_ELASER_AP    2u

// Map RAM mirror written by `setvar gsvar_byte1,5` (levels.c WM_GSVAR_BYTE1);
// the 2_3B `.seatest` inline script reads RAM8(WM_GSVAR_BYTE1).
#define SEA_WM_GSVAR_BYTE1 0x0310u

// Local strategy-flag bits (al_sflags2 free bits for these aliens):
#define SEA_SFLAG1 0x20u   // seamon: splashed-down latch (STRATEQU sflag1)
#define SEA_SFLAG2 0x40u   // seamon: swim-anim toggle / fish: landed latch
#define SEA_SFLAG3 0x80u   // fish: launch to the +X side (STRATEQU sflag3)
// bossg sflag8 lives in al_sflags4 like the other literal ports.
#define SEA_SFLAG8 ASF4_SFLAG8

// Positional sea sounds (SOUND.ASM enemyupsea_l:861 / enemydownsea_l:874).
// The flat runtime plays the center-channel variant.
#define SND_UPSEA   0x69u
#define SND_DOWNSEA 0x75u

// bossg mode-table indices (D2STRATS.ASM:66-109 s_mode_entry order).
enum {
    BOSSG_MODE_WAITHITPLAYER = 0,   // .waituntilalmosthitplayer
    BOSSG_MODE_SCROLLMSG     = 1,   // .scrollmsg
    BOSSG_MODE_SF9E_A        = 2,   // .sf9e
    BOSSG_MODE_LOOPBACK_PT   = 3,   // .runaway  (label bossg_loopback)
    BOSSG_MODE_DISAPPEAR     = 4,   // .disappear
    BOSSG_MODE_WAITSOMETIME  = 5,   // .waitsometime
    BOSSG_MODE_APPEAR        = 6,   // .appear
    BOSSG_MODE_MOVETO600H    = 7,   // .moveto600h
    // 8..13, 14..19, 20..25: opentrunk/launchleftfish/launchrightfish/
    // waitabit2/closetrunk/waitabit2 (three full trunk cycles)
    // 26..30: opentrunk/launchleftfish/launchrightfish/waitabit2/closetrunk
    //         (fourth cycle; trailing waitabit2 is commented out in the ASM)
    BOSSG_MODE_GENSHADOWS    = 31,  // .generateshadows
    BOSSG_MODE_WAITABIT      = 32,  // .waitabit
    BOSSG_MODE_SF9E_B        = 33,  // .sf9e
    BOSSG_MODE_RUNAWAY_B     = 34,  // .runaway
    BOSSG_MODE_LOOPBACK      = 35,  // .loopback
};

// Explosion size marker shared with strat_enemy.c's flat explosion objects
// (EXPSHAPE_SMALL).  bossgexplode's children use shape #explosion5 in the
// ASM; that mesh is not compiled, so the marker id is kept instead.
#define SEA_EXPSHAPE_SMALL 1u

static void bossseamon_strat(Alien *self);
static void bossseamonexp_init(Alien *self);
static void seamon_strat(Alien *self);
static void bossg_strat(Alien *self);
static void bossgexplode_init(Alien *self);
static void bossgexplode_strat(Alien *self);
static void bossgs_init(Alien *self);
static void bossgs_strat(Alien *self);
static void flyingfish_init(Alien *self);
static void flyingfish_strat(Alien *self);
static void flyingfish_flying_strat(Alien *self);

// ============================================================
// Small shared helpers (same idioms as strat_enemy.c)
// ============================================================

// s_add_playerZ macro → sr_addplayerZx (STRATROU.ASM:3092)
static void sea_add_player_z(Alien *al) {
    al->worldz += g_pviewvelz;
}

// al1pt phase staggering used by s_jmp_notdelay N,label,al1pt.
static uint8 sea_phase_offset(const Alien *self) {
    if (!self || self < g_aliens || self >= (g_aliens + NUMBER_AL)) {
        return 0u;
    }
    return (uint8)(self - g_aliens);
}

// s_jmp_notdelay period,label[,al1pt]: true when this frame is NOT the tick.
static bool sea_not_delay(const Alien *self, uint8 period) {
    if (period <= 1u) {
        return false;
    }
    return ((uint8)((uint8)g_gameframe + sea_phase_offset(self)) % period) != 0u;
}

static Alien *sea_player(void) {
    Alien *player = Obj_GetPlayer();
    if (!player || !player->active) {
        return NULL;
    }
    return player;
}

// dzdistless_l (DSTRATS.ASM:151-163): abs(objz - playerz) < dist.
static bool sea_dz_less(const Alien *self, int32 dist) {
    Alien *player = sea_player();
    if (!player) {
        return false;
    }
    return labs((long)self->worldz - (long)player->worldz) < (long)dist;
}

// s_gen_vecs obj,anglevar,al_vel → alvelvecs from an arbitrary angle byte.
static void sea_gen_vecs_angle(Alien *self, uint8 angle) {
    float rad = (float)angle * (2.0f * 3.14159265f / 256.0f);
    float speed = (float)(int16)self->vel;
    self->vx = (int16)(speed * sinf(rad));
    self->vy = 0;
    self->vz = (int16)(speed * cosf(rad));
}

// strat_pitch_toward (strat_enemy.c convention for weapon pitch).
static uint8 sea_pitch_toward(const Alien *src, const Alien *dst) {
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

// relslowelaser speed matches strat_relslowelaser_speed (strat_enemy.c).
static uint8 sea_relslowelaser_speed(void) {
    return (g_currentlevel == 1u) ? 48u : 60u;
}

// s_weapon_pos #0,#0,#0 + s_weapon_rots2obj player + s_fire_weapon
// x,RELSLOWELASER — straight enemy laser aimed at the player.
static void sea_fire_relslowelaser(Alien *self, Alien *target) {
    Alien *shot;
    uint8 yaw;
    uint8 pitch;

    if (!self || !target || !target->active) {
        return;
    }

    yaw = Strat_AngleXZ(self, target);
    pitch = sea_pitch_toward(self, target);

    shot = Strat_SpawnProjectile(self,
                                 0, 0, 0,
                                 pitch, yaw,
                                 sea_relslowelaser_speed(),
                                 SEA_ELASER_LIFE,
                                 SEA_ELASER_AP,
                                 ACF_COLLTYPE4);
    if (shot) {
        shot->rotz = self->rotz;
    }
}

// makesplash_srou_l / s_make_splash (GSTRATS.ASM:1112-1152).
// The splash/Ssplash sprite objects are software sprites on SNES; the flat
// runtime has no splash sprite shapes yet, so this is a visual no-op kept as
// a hook for when the splash renderer lands.
static void sea_make_splash(Alien *al) {
    (void)al;
}

// enemyupsea_l / enemydownsea_l (SOUND.ASM:861/874) — positional splash SFX.
static void sea_enemy_up_sea(void) {
    Sound_PlaySE(SND_UPSEA);
}

static void sea_enemy_down_sea(void) {
    Sound_PlaySE(SND_DOWNSEA);
}

// find_y_l equivalent: first active alien with the given shape word.
static Alien *sea_find_shape(uint16 shape) {
    Alien *al;
    for (al = g_active_list; al; al = al->next) {
        if (al->active && al->shape == shape) {
            return al;
        }
    }
    return NULL;
}

// s_dec_var B,gsvar_byte1 — keep both the map-RAM mirror (read by the 2_3B
// `.seatest` inline script) and the C global in sync.
static void sea_dec_gsvar_byte1(void) {
    if (RAM8(SEA_WM_GSVAR_BYTE1) > 0u) {
        RAM8(SEA_WM_GSVAR_BYTE1)--;
    }
    g_gsvar_byte1 = RAM8(SEA_WM_GSVAR_BYTE1);
}

// s_init_anim stores the frame with bit 7 set so the renderer keeps it
// (same convention as szaco2 in strat_enemy.c).
static uint8 sea_anim_get(const Alien *al) {
    return (uint8)(al->animframe & 0x7Fu);
}

static void sea_anim_set(Alien *al, uint8 frame) {
    al->animframe = (uint8)(0x80u | (frame & 0x7Fu));
}

// s_add_rnd2pos y,15,15,15
static void sea_add_rnd2pos(Alien *al, int16 range) {
    int16 span = (int16)(range * 2 + 1);
    al->worldx = (int16)(al->worldx + (int16)(SfRtl_Random() % span) - range);
    al->worldy = (int16)(al->worldy + (int16)(SfRtl_Random() % span) - range);
    al->worldz = (int16)(al->worldz + (int16)(SfRtl_Random() % span) - range);
}

// delayexplode child used by bossgexplode (makeSMLexpobj_srou,
// EXPSTRAT.ASM:313-338).  noexpsnd is honored: the child just times out.
static void sea_expchild_strat(Alien *self) {
    if (!self) {
        return;
    }
    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        g_aldead = 1u;
        return;
    }
    if (self->sflags2 & ASF2_RELEXPLODE) {
        sea_add_player_z(self);
    }
}

static Alien *sea_make_small_expobj(const Alien *parent) {
    Alien *child = Strat_MakeObj(SEA_EXPSHAPE_SMALL);
    if (!child) {
        return NULL;
    }
    child->sflags3 &= (uint8)~ASF3_REALOBJ;
    child->sflags |= ASF_COLLDISABLE;
    child->sflags2 |= (ASF2_NOEXPSND | ASF2_RELEXPLODE);
    child->HP = HARD_HP;
    child->AP = HARD_AP;
    child->stratptr = sea_expchild_strat;
    child->collstratptr = NULL;
    child->expstratptr = NULL;
    child->count = 2u;
    child->worldx = parent->worldx;
    child->worldy = parent->worldy;
    child->worldz = parent->worldz;
    return child;
}

// ============================================================
// bossseamon_Istrat (GA2STRAT.ASM:3056-3065)
// Sea Zone mid-boss school: five spawn at once (MAP2_3B.ASM:26-30),
// each worth one gsvar_byte1 tick on death.
// ============================================================
void Strat_BossSeamon_Init(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_alptrs x,bossseamon_strat,hitflash_Istrat,bossseamonexp_Istrat
    self->stratptr = bossseamon_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = bossseamonexp_init;
    // s_set_aldata x,#2,#4
    self->HP = BOSSSEAMON_HP;
    self->AP = BOSSSEAMON_AP;
    // s_set_alvar2rnd x,al_sbyte2
    self->sbyte2 = (uint8)SfRtl_Random();
    // s_set_alvar B,x,al_roty,#deg180
    self->roty = DEG180;
    // s_set_colltype x,enemyweap
    self->collflags |= COLLTYPE_ENEMYWEAP;
    // s_setnoremove_behind x
    self->type &= (uint8)~ATZREMOVE;
    // s_set_alvar B,x,al_sbyte3,#60
    self->sbyte3 = 60u;
    // s_set_alvar B,x,al_sbyte4,#3
    self->sbyte4 = 3u;

    self->stratstate = 0u;
    // levels.c passes the MACRO-counted shape id (31); the flat mesh id for
    // sea_0_0 is 32, so pin it here (the strat rewrites al_shape constantly
    // anyway).
    self->shape = SH_SEA_0_0;
}

// bossseamon_strat (GA2STRAT.ASM:3066-3191)
// stratstate machine; blocks are sequential (a state change inside one block
// lets later blocks run the same tick, exactly like the ASM fallthrough).
static void bossseamon_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = sea_player();

restart:  // jmptostrat target for setstate2/setstate5 (STRATROU.ASM:2984/2990)

    // --- state 0: move around in the distance (GA2STRAT.ASM:3070-3083) ---
    if (self->stratstate == 0u) {
        self->sflags |= ASF_COLLDISABLE;
        // s_beqdec_alvar B,x,al_sbyte3,setstate2
        if (self->sbyte3 == 0u) {
            self->stratstate = 2u;
            goto restart;
        }
        self->sbyte3--;
        (void)Strat_SpeedTo(self, 20u, 1u);
        // s_gen_vecs x,al_sbyte2,al_vel
        sea_gen_vecs_angle(self, self->sbyte2);
        // s_set_alvar W,x,al_vz,#-40
        self->vz = -40;
        // s_add_alvar B,x,al_sbyte2,#4
        self->sbyte2 = (uint8)(self->sbyte2 + 4u);
        // s_jmp_notdelay 5,.ns0,al1pt
        if (!sea_not_delay(self, 5u)) {
            sea_enemy_down_sea();
            sea_make_splash(self);
            self->sbyte1 = 10u;
            self->shape = SH_SEA_0_1_PROXY;  // #sea_0_1
            self->stratstate++;              // s_next_state
        }
    }

    // --- state 1: make small splash (GA2STRAT.ASM:3085-3092) ---
    if (self->stratstate == 1u) {
        // s_set_vecs x,#0,#0,#0
        self->vx = 0;
        self->vy = 0;
        self->vz = 0;
        // s_decbne_alvar B,x,al_Sbyte1,.ns1
        self->sbyte1--;
        if (self->sbyte1 == 0u) {
            self->stratstate = 0u;
            self->shape = SH_SEA_0_0;
            sea_enemy_down_sea();
            sea_make_splash(self);
        }
    }

    // --- state 2: jump out of water and fire laser (GA2STRAT.ASM:3094-3105) ---
    if (self->stratstate == 2u) {
        self->sflags &= (uint8)~ASF_COLLDISABLE;
        self->vz = 0;
        // s_neg_alvar W,x,al_vx
        self->vx = (int16)-self->vx;
        self->vy = -15;
        self->shape = SH_SEA_0_PROXY;  // #sea_0 (airborne pose, mesh missing)
        sea_enemy_down_sea();
        sea_make_splash(self);
        Strat_ApplyVelocity(self);  // s_add_vecs2pos
        // s_beqdec_alvar B,x,al_sbyte4,setstate5
        if (self->sbyte4 == 0u) {
            self->stratstate = 5u;
            goto restart;
        }
        self->sbyte4--;
        self->stratstate++;  // s_next_state → 3
    }

    // --- state 3: airborne arc + fire (GA2STRAT.ASM:3107-3125) ---
    if (self->stratstate == 3u) {
        // fire while falling only (s_jmp_alvarMI W,x,al_vy,.nfire)
        if (self->vy >= 0 &&
            !sea_not_delay(self, 3u) &&
            player) {
            // s_weapon_pos #0,#0,#0 / s_weapon_rots2obj player / RELSLOWELASER
            sea_fire_relslowelaser(self, player);
        }

        self->vy = (int16)(self->vy + 1);
        // s_jmp_higher x,#0,.ns3 — still above the surface (worldy < 0)
        if (self->worldy >= 0) {
            self->vy = 0;
            sea_enemy_down_sea();
            sea_make_splash(self);
            self->shape = SH_SEA_0_0;
            self->vel = 0u;  // s_set_speed x,#0
            self->sbyte3 = 30u;
            self->stratstate++;  // → 4
        }
    }

    // --- state 4: rest under water (GA2STRAT.ASM:3127-3130) ---
    if (self->stratstate == 4u) {
        self->sflags |= ASF_COLLDISABLE;
        // s_beqdec_alvar B,x,al_sbyte3,setstate2
        if (self->sbyte3 == 0u) {
            self->stratstate = 2u;
            goto restart;
        }
        self->sbyte3--;
    }

    // --- state 5: dive-jump toward the player (GA2STRAT.ASM:3133-3139) ---
    if (self->stratstate == 5u) {
        sea_enemy_down_sea();
        sea_make_splash(self);
        self->vy = -20;
        self->shape = SH_SEA_0_PROXY;  // #sea_0
        Strat_ApplyVelocity(self);     // s_add_vecs2pos
        self->stratstate++;            // → 6
    }

    // --- state 6: fall back to the surface (GA2STRAT.ASM:3143-3151) ---
    if (self->stratstate == 6u) {
        self->vy = (int16)(self->vy + 2);
        // s_jmp_higher x,#0,.ns6
        if (self->worldy >= 0) {
            self->vy = 0;
            sea_enemy_down_sea();
            sea_make_splash(self);
            self->shape = SH_SEA_0_1_PROXY;  // #sea_0_1
            self->stratstate++;              // → 7
        }
    }

    // --- state 7: pick chase or retreat (GA2STRAT.ASM:3154-3167) ---
    if (self->stratstate == 7u && player) {
        // s_jmp_Zdistmore x,y,#200,.ok / s_jmp_objinfront x,y,.ok
        if (labs((long)self->worldz - (long)player->worldz) >= 200 ||
            self->worldz >= player->worldz) {
            // .ok — line up another dive at the player
            self->sbyte2 = Strat_AngleXZ(self, player);  // s_obj2obj_angle,0
            self->vel = 20u;
            sea_gen_vecs_angle(self, self->sbyte2);
            self->stratstate = 5u;
        } else {
            // behind & too close — swim away downfield
            self->stratstate = 8u;
            self->vx = 0;
            self->vy = 0;
            self->vz = 50;  // s_set_vecs x,#0,#0,#50
            self->worldy = -150;
            self->shape = SH_SEA_0_PROXY;  // #sea_0
        }
    }

    // --- state 8: re-enter the water ahead (GA2STRAT.ASM:3171-3184) ---
    if (self->stratstate == 8u) {
        self->vy = (int16)(self->vy + 2);
        // s_jmp_higher x,#0,.ns8
        if (self->worldy >= 0) {
            self->sbyte4 = 2u;
            self->stratstate = 2u;
            self->vy = 0;
            // s_set_alvar2rnd x,al_vx,#7 / s_add_alvar W,x,al_vx,#5
            self->vx = (int16)((SfRtl_Random() & 7u) + 5u);
            // s_jmp_random .nneg / s_neg_alvar W,x,al_vx
            if (SfRtl_Random() & 1u) {
                self->vx = (int16)-self->vx;
            }
            self->shape = SH_SEA_0_0;
        }
    }

    // --- common tail (GA2STRAT.ASM:3187-3191) ---
    // s_limit_alvar W,x,al_worldx,bridge_minX*2,bridge_maxX*2
    if (self->worldx < SEA_BRIDGE_MINX2) {
        self->worldx = SEA_BRIDGE_MINX2;
    } else if (self->worldx > SEA_BRIDGE_MAXX2) {
        self->worldx = SEA_BRIDGE_MAXX2;
    }
    Strat_ApplyVelocity(self);  // s_Add_vecs2pos
    sea_add_player_z(self);     // s_add_playerZ
}

// bossseamonexp_Istrat (GA2STRAT.ASM:3193-3196)
// s_dec_var B,gsvar_byte1 / s_jmp explode_Istrat
static void bossseamonexp_init(Alien *self) {
    sea_dec_gsvar_byte1();
    Strat_Explode(self);
}

// ============================================================
// seamon_Istrat (GASTRATS.ASM:2046-2059)
// Small sea monster (MAP3_3B waves; istrat index 81).
// ============================================================
void Strat_Seamon_Init(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_alptrs x,seamon_strat,hitflash_Istrat,explode_Istrat
    self->stratptr = seamon_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    // s_set_aldata x,#4,#8
    self->HP = SEAMON_HP;
    self->AP = SEAMON_AP;
    self->vz = 30;
    self->roty = DEG180;
    self->sbyte1 = 10u;
    self->sbyte2 = (uint8)SfRtl_Random();
    self->sbyte4 = 1u;
    self->sword2 = -10;
    self->sbyte3 = 40u;
    self->stratstate = 2u;  // s_set_state x,#2
    // s_set_colltype x,Zenemy
    self->collflags |= COLLTYPE_ZENEMY;
    // MACRO-counted map id 31 → flat mesh id 32 (see Strat_BossSeamon_Init).
    self->shape = SH_SEA_0_0;
}

// seamon_strat (GASTRATS.ASM:2060-2140)
static void seamon_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = sea_player();

    Strat_ApplyVelocity(self);  // s_add_vecs2pos

    // --- surface swim wiggle/animation (.dmc/.dlrm) ---
    // s_jmp_IFstate x,3,.dmc / s_jmp_alvarNOTZERO W,x,al_worldy,.nsc
    if (self->stratstate == 3u || self->worldy == 0) {
        // s_beqdec_alvar B,x,al_sbyte3,.dlrm
        if (self->sbyte3 == 0u) {
            // .dlrm: vx = sintab[sbyte2] >> 4
            self->vx = (int16)((int16)(sinf((float)self->sbyte2 *
                                             (2.0f * 3.14159265f / 256.0f)) *
                                       127.0f) >> 4);
            self->sbyte2 = (uint8)(self->sbyte2 + 4u);
            // s_decbne_alvar B,x,al_sbyte1,.nsc
            self->sbyte1--;
            if (self->sbyte1 == 0u) {
                self->sbyte1 = 10u;
                self->shape = SH_SEA_0_1_PROXY;  // #sea_0_1
                // s_not_alsflag x,sflag2 / s_beq .nsc
                self->sflags2 ^= SEA_SFLAG2;
                if (self->sflags2 & SEA_SFLAG2) {
                    self->shape = SH_SEA_0_0;
                }
            }
        } else {
            self->sbyte3--;
        }
    }

    // --- airborne / surface handling ---
    // s_jmp_lower x,#-1,.nds
    if (self->worldy >= -1) {
        // .nds: clamp back onto the surface when falling
        if (self->vy >= 0) {  // s_jmp_alvarmi W,x,al_vy,.nzv
            self->worldy = 0;
            self->vy = 0;
        }
    } else {
        self->vy = (int16)(self->vy + 2);
        // s_jmp_lower x,#-30,.hs
        if (self->worldy >= -30) {
            // .hs: near the surface
            if (self->vy >= 0) {  // falling
                self->shape = SH_SEA_0_0;
                // s_jmp_alsflag x,sflag1,.nds (splash-down latch)
                if (!(self->sflags2 & SEA_SFLAG1)) {
                    self->sflags2 |= SEA_SFLAG1;
                    sea_enemy_down_sea();
                    sea_make_splash(self);  // splash spawns at worldy 0
                    self->sbyte3 = 10u;
                    self->sflags |= ASF_COLLDISABLE;
                }
            }
        } else {
            // high in the air: show jump pose, fire at the player
            self->shape = SH_SEA_0_PROXY;  // #sea_0
            // s_jmp_NOTdelay 4,.nzv,al1pt
            if (!sea_not_delay(self, 4u) && player) {
                long zd = labs((long)self->worldz - (long)player->worldz);
                long xd = labs((long)self->worldx - (long)player->worldx);
                // s_jmp_outZdistrng x,y,#500,#1000,.nzv
                // s_jmp_Xdistmore x,y,#200,.nzv
                if (zd >= 500 && zd < 1000 && xd < 200) {
                    sea_fire_relslowelaser(self, player);
                }
            }
        }
    }

    // --- .nzv: jump countdown (GASTRATS.ASM:2110-2138) ---
    // s_decbne_alvar B,x,al_sbyte4,.njump1
    self->sbyte4--;
    if (self->sbyte4 != 0u) {
        return;  // .njump1 / s_end_strat
    }

    // s_next_state x,#3 (wraps 1→2→3→1…)
    self->stratstate++;
    if (self->stratstate > 3u) {
        self->stratstate = 1u;
    }

    if (self->stratstate == 1u || self->stratstate == 2u) {
        self->sbyte4 = 40u;
        self->vy = -15;
    } else if (self->stratstate == 3u) {
        self->sbyte4 = 60u;
        self->vy = -25;
    }

    sea_enemy_up_sea();
    self->sflags &= (uint8)~ASF_COLLDISABLE;
    self->vx = 0;
    self->sflags2 &= (uint8)~SEA_SFLAG1;
    sea_make_splash(self);
}

// ============================================================
// flyingfish_istrat (D3STRATS.ASM:1098-1155)
// Fish launched from bossg's trunk.
// ============================================================
static void flyingfish_init(Alien *self) {
    if (!self) {
        return;
    }
    // s_set_colltype x,ENEMY1
    self->collflags |= COLLTYPE_ENEMY1;
    // s_set_alptrs x,.strat,hitflash_istrat,explode_istrat
    self->stratptr = flyingfish_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    // s_add_alvar B,x,al_roty,#deg180
    self->roty = (uint8)(self->roty + DEG180);
    // s_set_aldata x,#fishHP,#fishAP
    self->HP = FISH_HP;
    self->AP = FISH_AP;
    // s_init_anim x,#0
    sea_anim_set(self, 0u);
    self->type |= ATZREMOVE;
}

static void flyingfish_strat(Alien *self) {
    Alien *player;
    int16 newx;

    if (!self) {
        return;
    }

    // s_jmp_alsflag x,sflag2,.end — landed for good
    if (self->sflags2 & SEA_SFLAG2) {
        return;
    }

    sea_make_splash(self);  // s_make_splash x,s (every frame)

    // gravity while above the surface (worldy < 0)
    if (self->worldy < 0) {
        self->vy = (int16)(self->vy + 2);
        self->worldy = (int16)(self->worldy + self->vy);
        if (self->worldy >= 0) {
            self->worldy = 0;
        }
    }

    // .atsealevel
    sea_add_player_z(self);

    if (!(self->sflags2 & SEA_SFLAG3)) {
        // drift toward x = -200 (s_achase_alvar W,x,al_worldx,#-200,3)
        newx = Strat_ChaseProportional(self->worldx, -200, 3);
        self->worldx = newx;
        if (newx != -200 && self->worldx >= -150) {
            return;  // .end
        }
    } else {
        // .left: drift toward x = +200
        newx = Strat_ChaseProportional(self->worldx, 200, 3);
        self->worldx = newx;
        if (newx != 200 && self->worldx < 150) {
            return;  // .end
        }
    }

    // .jumping (D3STRATS.ASM:1131-1143)
    player = sea_player();
    if (player) {
        self->roty = Strat_AngleXZ(self, player);  // s_obj2obj_angle,0
    }
    sea_anim_set(self, 0u);
    self->stratptr = flyingfish_flying_strat;
    self->vel = 70u;
    sea_gen_vecs_angle(self, self->roty);  // s_gen_vecs x,al_roty,al_vel
    self->vy = -15;
    sea_make_splash(self);
    sea_enemy_up_sea();  // jsl enemyupsea_l

    flyingfish_flying_strat(self);  // fallthrough into .flying
}

static void flyingfish_flying_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->vy = (int16)(self->vy + 2);
    Strat_ApplyVelocity(self);  // s_add_vecs2pos
    sea_add_player_z(self);

    // s_cmp_alvar W,x,al_worldy,#0 / s_bmi .end
    if (self->worldy >= 0) {
        sea_make_splash(self);
        self->sflags2 |= SEA_SFLAG2;
        // zremove substitute: the flat runtime has no behind-camera sweep
        // yet, so reclaim the sunken fish once it drops well below the sea.
        if (self->worldy > 300) {
            g_aldead = 1u;
        }
    }
}

// ============================================================
// bossgs_istrat (D2STRATS.ASM:474-495)
// bossg shadow clones generated by .generateshadows.
// ============================================================
static void bossgs_init(Alien *self) {
    if (!self) {
        return;
    }
    // s_set_colltype x,ENEMY1
    self->collflags |= COLLTYPE_ENEMY1;
    // s_set_alptrs x,.strat,hitflash_istrat,explode_istrat
    self->stratptr = bossgs_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    // s_set_aldata x,#hardHP,#hardAP
    self->HP = HARD_HP;
    self->AP = HARD_AP;
    // s_set_alvar B,x,al_sbyte1,#40
    self->sbyte1 = 40u;
    self->type |= ATZREMOVE;
}

static void bossgs_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    // ASM flickers al_coltab between BLACK_C and normal every other frame;
    // the flat coltab set (id_0..id_5) has no BLACK_C entry yet, and the
    // boss_g_s wireframe mesh is not compiled, so the flicker is a no-op.

    // s_fchase_alvar2alvar W,x,al_worldx,x,al_sword1,5
    self->worldx = Strat_Chase(self->worldx, self->sword1, 5);

    // s_beqdec_alvar B,x,al_sbyte1,.dash
    if (self->sbyte1 == 0u) {
        // .dash
        self->worldz = (int16)(self->worldz - 50);
        // zremove substitute (no behind-camera sweep in the flat runtime).
        player = sea_player();
        if (player && (int32)player->worldz - (int32)self->worldz > 2000) {
            g_aldead = 1u;
        }
        return;
    }
    self->sbyte1--;
    sea_add_player_z(self);
}

// ============================================================
// bossg_istrat (D2STRATS.ASM:54-63)
// ============================================================
void Strat_BossG_Init(Alien *self) {
    if (!self) {
        return;
    }

    // stz maptrigger
    g_maptrigger = 0u;
    // s_set_alptrs x,.strat,hitflash_istrat,bossgexplode_istrat
    self->stratptr = bossg_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = bossgexplode_init;
    // s_set_aldata x,#hardHP,#bossgAP — invulnerable until .appear
    self->HP = HARD_HP;
    self->AP = BOSSG_AP;
    // s_init_anim x,#0
    sea_anim_set(self, 0u);
    // s_set_alsflag x,shadow
    self->sflags |= ASF_SHADOW;
    // s_set_colltype x,ENEMY1
    self->collflags |= COLLTYPE_ENEMY1;
    // s_mode_change x,#0 — mode index lives in al_stratmem
    self->stratmem = 0u;
    // trigse $9d
    Sound_PlaySE(0x9Du);
    // levels.c passes MACRO-counted shape 120 (kichi_0 under flat ids);
    // pin the flat boss_g_0 mesh (121) here.
    self->shape = SH_BOSS_G_0;
}

// .genspark helper (D2STRATS.ASM:343-352) → sgenspark_srou_l.
// The spark particle sprite routine has no flat-runtime port yet.
static void bossg_genspark(Alien *self) {
    (void)self;
}

// .launchfish (D2STRATS.ASM:355-363)
static Alien *bossg_launch_fish(Alien *self) {
    Alien *fish = Strat_MakeObj(SH_F_FISH_PROXY);  // s_make_obj #f_fish
    if (!fish) {
        return NULL;  // .dekinai
    }
    // d2copypos_yx / al_worldy += 30 / d2copyrots_yx
    fish->worldx = self->worldx;
    fish->worldy = (int16)(self->worldy + 30);
    fish->worldz = self->worldz;
    fish->rotx = self->rotx;
    fish->roty = self->roty;
    fish->rotz = self->rotz;
    // s_set_strat y,flyingfish_istrat
    flyingfish_init(fish);
    return fish;
}

// .generateshadows (D2STRATS.ASM:160-190)
static void bossg_generate_shadows(Alien *self) {
    static const int16 offsets[3] = { -100, 0, 100 };
    int i;

    Sound_PlaySE(0x2Du);  // trigse $2d
    for (i = 0; i < 3; i++) {
        Alien *shadow = Strat_MakeObj(SH_BOSS_G_S);  // s_make_obj #boss_g_s
        if (!shadow) {
            return;  // branch to .nxtmode on alloc failure
        }
        shadow->worldx = self->worldx;
        shadow->worldy = self->worldy;
        shadow->worldz = (int16)(self->worldz - 50);
        shadow->rotx = self->rotx;
        shadow->roty = self->roty;
        shadow->rotz = self->rotz;
        shadow->sword1 = offsets[i];  // al_sword1 = -100 / (unset) / +100
        bossgs_init(shadow);          // s_set_strat y,bossgs_istrat
    }
}

// .move2 (D2STRATS.ASM:367-381): scroll with player + splash trail.
// s_add_bosshp x,al_hp feeds the per-frame boss HP meter; the HD HUD
// currently mirrors g_bossmaxhp as a full bar (see nmi.c), so the
// accumulation has no flat-runtime sink yet.
static void bossg_move2(Alien *self) {
    sea_add_player_z(self);
    if (!sea_not_delay(self, 1u)) {
        sea_make_splash(self);  // splash at worldz+30
    }
}

// bossg .strat mode table (D2STRATS.ASM:65-387).
// Handlers either advance the mode and re-dispatch within the same tick
// (`.nxtmode: s_mode_change x,+1 / s_jmp .strat`) or finish the frame via
// .end/.move/.move2 — mirrored here with continue/return.
static void bossg_strat(Alien *self) {
    uint8 anim;

    if (!self) {
        return;
    }

    for (;;) {
        switch (self->stratmem) {

        // ********** .waituntilalmosthitplayer (D2STRATS.ASM:296-303)
        case BOSSG_MODE_WAITHITPLAYER:
            self->worldz = (int16)(self->worldz - 40);
            if (sea_dz_less(self, 150)) {
                self->stratmem++;  // .nxtmode
                continue;
            }
            return;  // .end

        // ********** .scrollmsg (D2STRATS.ASM:307-318)
        case BOSSG_MODE_SCROLLMSG:
            if (sea_dz_less(self, 140)) {
                self->worldz = (int16)(self->worldz + 40);
            }
            self->tx = (uint8)(self->tx + 4u);
            if ((self->tx & 127u) == 0u) {
                self->stratmem++;
                continue;
            }
            sea_add_player_z(self);  // .move
            return;

        // ********** .sf9e (D2STRATS.ASM:117-119)
        case BOSSG_MODE_SF9E_A:
        case BOSSG_MODE_SF9E_B:
            Sound_PlaySE(0x9Eu);
            self->stratmem++;
            continue;

        // ********** .runaway (D2STRATS.ASM:277-293)
        case BOSSG_MODE_LOOPBACK_PT:  // bossg_loopback entry
        case BOSSG_MODE_RUNAWAY_B:
            if (sea_dz_less(self, 1000)) {
                bossg_genspark(self);  // .genspark (sprite spark not ported)
            }
            self->worldz = (int16)(self->worldz + 70);
            if (!sea_dz_less(self, 4000)) {
                self->stratmem++;  // far enough — .nxtmode
                continue;
            }
            if (g_bossmaxhp == 0u) {
                sea_add_player_z(self);  // .move
            } else {
                bossg_move2(self);       // .move2
            }
            return;

        // ********** .disappear (D2STRATS.ASM:258-261)
        case BOSSG_MODE_DISAPPEAR:
            self->shape = SH_NULLSHAPE_WORD;  // #nullshape
            g_maptrigger |= 1u;               // s_or_var B,maptrigger,#1
            self->stratmem++;
            continue;

        // ********** .waitsometime (D2STRATS.ASM:223-256)
        case BOSSG_MODE_WAITSOMETIME:
            // regenerate HP toward bossgHP every 2 frames once the boss
            // bar has been armed
            if (g_bossmaxhp != 0u &&
                !sea_not_delay(self, 2u) &&
                self->HP != (uint8)BOSSG_HP) {
                self->HP++;
            }
            // wait until the map consumed maptrigger bit 0 and every
            // sea monster shape is gone
            if ((g_maptrigger & 1u) == 0u &&
                !sea_find_shape(SH_SEA_0_PROXY) &&
                !sea_find_shape(SH_SEA_0_0) &&
                !sea_find_shape(SH_SEA_0_1_PROXY)) {
                self->sbyte1 = 0u;  // .znxtmode
                self->stratmem++;
                continue;
            }
            // .wait
            if (g_bossmaxhp == 0u) {
                sea_add_player_z(self);  // .move
            } else {
                bossg_move2(self);       // .move2
            }
            return;

        // ********** .appear (D2STRATS.ASM:263-270)
        case BOSSG_MODE_APPEAR:
            self->shape = SH_BOSS_G_0;  // #boss_g_0
            if (g_bossmaxhp == 0u) {
                // s_set_aldata x,#bossgHP,#bossgAP / s_set_bossmaxHp x,al_hp
                self->HP = BOSSG_HP;
                self->AP = BOSSG_AP;
                g_bossmaxhp = BOSSG_HP;
            }
            self->stratmem++;
            continue;

        // ********** .moveto600h (D2STRATS.ASM:321-340)
        case BOSSG_MODE_MOVETO600H:
            if (!(self->sflags4 & SEA_SFLAG8) && sea_dz_less(self, 1500)) {
                Sound_PlaySE(0x9Du);  // trigse $9d
                self->sflags4 |= SEA_SFLAG8;
            }
            if (sea_dz_less(self, 600)) {
                self->sflags4 &= (uint8)~SEA_SFLAG8;  // .z8nxtmode
                self->stratmem++;
                continue;
            }
            self->worldz = (int16)(self->worldz - 40);
            bossg_move2(self);
            return;

        // ********** .opentrunk (D2STRATS.ASM:125-131)
        case 8: case 14: case 20: case 26:
            anim = sea_anim_get(self);
            if (anim == 0u) {
                Sound_PlaySE(0x5Au);
            }
            // s_add_anim x,#1,#10,.nxtmode
            if (anim >= 9u) {
                self->stratmem++;
                continue;
            }
            sea_anim_set(self, (uint8)(anim + 1u));
            bossg_move2(self);
            return;

        // ********** .launchleftfish (D2STRATS.ASM:207-208)
        case 9: case 15: case 21: case 27:
            (void)bossg_launch_fish(self);
            self->stratmem++;
            continue;

        // ********** .launchrightfish (D2STRATS.ASM:211-214)
        case 10: case 16: case 22: case 28: {
            Alien *fish = bossg_launch_fish(self);
            if (fish) {
                fish->sflags2 |= SEA_SFLAG3;  // s_set_alsflag y,sflag3
            }
            self->stratmem++;
            continue;
        }

        // ********** .waitabit2 (D2STRATS.ASM:153-157)
        case 11: case 13: case 17: case 19: case 23: case 25: case 29:
            self->sbyte1++;
            if (self->sbyte1 == 10u) {
                self->sbyte1 = 0u;  // .znxtmode
                self->stratmem++;
                continue;
            }
            bossg_move2(self);
            return;

        // ********** .closetrunk (D2STRATS.ASM:134-142)
        case 12: case 18: case 24: case 30:
            anim = sea_anim_get(self);
            if (anim == 9u) {
                Sound_PlaySE(0x59u);
            }
            if (anim == 0u) {
                self->stratmem++;
                continue;
            }
            sea_anim_set(self, (uint8)(anim - 1u));
            bossg_move2(self);
            return;

        // ********** .generateshadows (D2STRATS.ASM:160-190)
        case BOSSG_MODE_GENSHADOWS:
            bossg_generate_shadows(self);
            self->stratmem++;
            continue;

        // ********** .waitabit (D2STRATS.ASM:146-150)
        case BOSSG_MODE_WAITABIT:
            self->sbyte1++;
            if (self->sbyte1 == 70u) {
                self->sbyte1 = 0u;  // .znxtmode
                self->stratmem++;
                continue;
            }
            bossg_move2(self);
            return;

        // ********** .loopback (D2STRATS.ASM:121-123)
        case BOSSG_MODE_LOOPBACK:
        default:
            self->stratmem = BOSSG_MODE_LOOPBACK_PT;  // s_mode_change
            continue;
        }
    }
}

// ============================================================
// bossgexplode_istrat (D2STRATS.ASM:393-437)
// Runs as expstrat while HP == 0 (do_strat dead path).
// ============================================================
static void bossgexplode_init(Alien *self) {
    Alien *shadow;

    if (!self) {
        return;
    }

    // .chkagain: remove every boss_g_s shadow clone
    while ((shadow = sea_find_shape(SH_BOSS_G_S)) != NULL) {
        Obj_Free(shadow);
    }

    // s_set_expstrat x,.strat — subsequent dead-path frames run .strat
    self->expstratptr = bossgexplode_strat;
    bossgexplode_strat(self);  // ASM falls straight through into .strat
}

static void bossgexplode_strat(Alien *self) {
    uint8 anim;
    int i;

    if (!self) {
        return;
    }

    // lda #3 / .make3: three small explosions per frame
    for (i = 0; i < 3; i++) {
        Alien *child = sea_make_small_expobj(self);
        if (child) {
            child->sflags4 |= ASF4_NOPOLYEXP;  // s_set_alsflag y,nopolyexp
            child->worldy = (int16)(child->worldy - 60);
            sea_add_rnd2pos(child, 15);        // s_add_rnd2pos y,15,15,15
            // ASM: s_set_alvar W,y,al_shape,#explosion5 — mesh not compiled;
            // the small-explosion marker shape is kept instead.
        }
    }

    // s_add_anim x,#1,#10,.nomore
    anim = sea_anim_get(self);
    if (anim < 9u) {
        sea_anim_set(self, (uint8)(anim + 1u));
    }

    if (sea_dz_less(self, 350)) {
        // .toppleoff (D2STRATS.ASM:435-437)
        g_maptrigger |= 2u;  // s_or_var B,maptrigger,#2 → 2_3B .carryon
        // jml bossexplode_istrat — staged boss explosion; its
        // bossdelayexplode tail sets GF_BOSSDEAD (+ hard HP so the normal
        // strat path runs), which releases mapwaitboss via chkbossdead.
        // (The `.strat2` topple-animation block after the jml is dead code
        // in the original and is not ported.)
        Strat_BossExplode_Init(self);
        return;
    }

    // .end: s_make_splash x,s
    sea_make_splash(self);
}

// ============================================================
// Registration
// ============================================================
void StratBossSea_Register(void) {
    // def_istrat rows (ISTRATS.ASM:504 seamon / :570 bossg).
    g_istrats[IS_SEAMON] = (StrategyFunc)Strat_Seamon_Init;
    g_istrats[IS_BOSSG]  = (StrategyFunc)Strat_BossG_Init;

    // Synthetic addresses referenced by the MAP2_3 literal map data
    // (src/map/levels.c).  Without these the bosses spawn inert and the
    // 2_3B mapwaitboss never releases.
    World_RegisterStrategyAddress(STRAT_ADDR_BOSSSEAMON,
                                  (StrategyFunc)Strat_BossSeamon_Init);
    World_RegisterStrategyAddress(STRAT_ADDR_BOSSG,
                                  (StrategyFunc)Strat_BossG_Init);
}
