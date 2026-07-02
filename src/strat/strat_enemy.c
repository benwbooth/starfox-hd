// Enemy strategies for Corneria (Level 1)
// Decompiled from GASTRATS.ASM (zaco1), GSTRATS.ASM (hard*, nocoll, hitflash),
// GISTRATS.ASM (friendexitbase), EXPSTRAT.ASM (explode).
//
// Strategy architecture:
// - Init functions (_Init) are called once by the map executor on spawn.
//   They set HP/AP, collision type, display flags, and assign the per-frame
//   strategy pointer (stratptr). Static objects set stratptr = NULL.
// - Per-frame functions are called by Obj_RunStrategies each tick (20 FPS).
//   They implement movement, AI state machines, weapon firing, etc.

#include "strat_enemy.h"
#include "strat_common.h"
#include "../game/game_vars.h"
#include "../game/obj.h"
#include "../game/sound.h"
#include "../game/world.h"
#include "../sf_rtl.h"
#include "../variables.h"
#include <string.h>
#include <stdio.h>
#include <math.h>

// ============================================================
// Sin/Cos table access (shared with strat_common.c)
// We use the same 256-entry table: angle 0-255 = 0-360 degrees
// ============================================================
static float strat_sin(uint8 angle) {
    return sinf((float)angle * (2.0f * 3.14159265f / 256.0f));
}
static float strat_cos(uint8 angle) {
    return cosf((float)angle * (2.0f * 3.14159265f / 256.0f));
}

// ============================================================
// Helper: Proportional angle chase (Achase)
// Chases current toward target with step = (current - target) >> shift.
// Works correctly with 8-bit wrapping angles.
// Returns true when target is reached.
// ============================================================
static bool achase_angle(uint8 *current, uint8 target, int shift) {
    if (*current == target) return true;
    int8 diff = (int8)(*current - target);
    int8 step = diff >> shift;
    if (step == 0) step = (diff > 0) ? 1 : -1;
    *current -= (uint8)step;
    return (*current == target);
}

// ============================================================
// Helper: Add player Z movement to alien
// From s_add_playerZ macro → sr_addplayerZx (STRATROU.ASM:3092)
// Keeps objects moving with the world scrolling.
// ============================================================
static void add_player_z(Alien *al) {
    al->worldz += g_pviewvelz;
}

// ============================================================
// HARD OBJECT STRATEGIES (GSTRATS.ASM)
// Static/indestructible ground objects — buildings, arches, etc.
// ============================================================

// s_hardvars macro: sets HP = hardHP (-1/0xFF), AP = hardAP (8)
static void set_hard_vars(Alien *al) {
    al->HP = HARD_HP;
    al->AP = HARD_AP;
}

#define RADER_HP      8
#define RADER_AP      4
#define WM_SKILLFLY   0x0304u
#define SKILLFLY_RADIUS_DEFAULT (20 << 2)
#define PILLAR3_HP    8
#define PILLAR3_FALL_HP 4
#define PILLAR3_AP    8
#define PILLAR3_DIST  500
#define PILLAR3_FALL_FRAMES 16
#define ZACOS_HP      2
#define ZACOS_AP      4
#define ZACO2_HP      4
#define ZACO2_AP      4
#define SZACO2_HP     2u
#define SZACO2_AP     8u
#define SZACO2_SPEED  40u
#define SZACO2_FIRE_NEAR_Z 400
#define SZACO2_FIRE_FAR_Z  1500
#define SZACO2_BANK_Z      1000
#define SZACO2_DASH_Z      600
#define SZACO2_TURN_SHIFT  3
#define SZACO2_FIN_SHIFT   2
#define SZACO2_FIRE_MASK   0x07u
#define SZACO2_ANIM_INIT   3u
#define SZACO2_WPY_OFFSET  150
#define ZACO3_HP      2
#define ZACO3_AP      8
#define ZACO0_HP      2
#define ZACO0_AP      8
#define ZACO4_HP      2
#define ZACO4_AP      8
#define HOUDAI_HP     8u
#define HOUDAI_AP     8u
#define CAMELEON_HP   2
#define CAMELEON_AP   8
#define WORM_HP       2
#define WORM_AP       4
#define WORM2_HP      4
#define WORM2_AP      2
#define PARA_HP       2
#define PARA_AP       4
#define PARA_SWINGSPD 5
#define PARA_SWINGMAX (PARA_SWINGSPD * 3)
#define CARRIER_HP    16u
#define CARRIER_AP    10u
#define CARRIER_RATE  30u
#define SH_PILLAR3    27
#define SH_PARA_0     60u
#define SH_ZACO_6     52u
#define SH_HOUDAI_0   54u
#define SH_ITEM_5     158
#define SH_PARA_1_PROXY SH_PARA_0
#define GASF_KILLTYPE1 0x01u
#define GASF_KILLTYPE2 0x02u
#define GATE3_TOUCH_ZDIST 200
#define GATE3_TOUCH_XY    (25 << 2)
#define GATE3_HEAL_AMOUNT 20
#define GATE2_TOUCH_ZDIST (30 << 1)
#define GATE2_TOUCH_XY    (30 << 1)
#define GATE2_HEAL_AMOUNT 5
#define GATE2_HEAL_SCORE  10
#define GATE2_GROUND_Y    (-30 << 1)
#define GATE2_SCROLL_Z    -50
#define GATE2_TOUCHED_FLAG 0x40u
#define GATE_HEAL_MAX     40
#define GATE_SOUND        0x0Fu
#define GATE3_SOUND       0x10u
#define GATE_NORM_COLS    4u
#define GATE_TOUCHED_COL0 5u
#define GATE_TOUCHED_COLE 20u
#define BOMWING_HP    4
#define BOMWING_AP    8
#define BOMWING_SPEED 20
#define SHORTPLASMA_SPEED 80u
#define SHORTPLASMA_LIFE  30u
#define SHORTPLASMA_AP    10u
#define HOUDAI_TRACK_MIN_Z 200
#define HOUDAI_FIRE_GATE_Z 800
#define HOUDAI_TRACK_MAX_Z 2000
#define HPLASMA_SPEED 60
#define HPLASMA_LIFE  50
#define HPLASMA_AP    10
#define ITEM5_PICKUP_Z 120
#define ITEM5_PICKUP_XY 60
#define ITEM5_MAX_SPEC 5
#define ITEM5_SCORE   100
#define UP1MAN_AP     8u
#define UP1MAN_PICKUP_X  (40 * 2)
#define UP1MAN_PICKUP_Y  (60 * 2)
#define UP1MAN_PICKUP_Z  (40 * 2)
#define UP1MAN_ACTIVE_Z 1500
#define UP1MAN_SCROLL_Z 30
#define UP1MAN_ROT_SPEED 5u
#define UP1MAN_SFLAG1  0x10u
#define SH_MYSHIP_4    2u
// `flashplayer_Istrat` wants the wireframe Arwing variants. Those canonical
// flat ids are still unresolved in the live runtime, so keep the per-damage
// shape choice explicit on the current player mesh proxy until that slice lands.
#define SH_MY_W_PROXY  SH_MYSHIP_4
#define SH_MY_R_W_PROXY SH_MYSHIP_4
#define SH_MY_L_W_PROXY SH_MYSHIP_4
#define SH_MY_B_W_PROXY SH_MYSHIP_4
#define SH_UP1_MAN_PROXY SH_MYSHIP_4
#define ITEM7_PICKUP_Z 120
#define ITEM7_PICKUP_XY 60
#define ITEM7_SCORE   100
#define HF2_MASK      0x02u
#define CLSHIP_FLAG1      0x10u
#define CLSHIP_FLAG2      0x20u
#define CLSHIP_FROGWAIT   30u
#define CLSHIP_BUNNYWAIT  60u
#define CLSHIP_COCKWAIT   90u
#define CLSHIP_GNDWAIT    110u
#define CLSHIP_WARP_BTIME 430u
// `gate3_Istrat` uses `al_sflag1`, which sits in the second flag byte.
// The current path runtime uses lower `sflags2` bits for path state, so 0x10
// remains available for literal ports that still use `al_sflag1`.
#define GATE3_TOUCHED_FLAG 0x10u
#define TADPOLE_SIDE_FLAG  0x80u
#define RELSLOWELASERHOME_LOCK_FLAG 0x20u
#define RELSLOWELASERHOME_CLOSE_Z 800
#define RELSLOWELASERHOME_OFFSCENE_Z 12000
#define RELSLOWELASERHOME_LIFE 40u
#define RELSLOWELASERHOME_AP 2u
#define BASE1_PHASE_FLAG 0x10u
#define BASE1_WAIT_FRAMES 10u
#define TADPOLE_HP       4u
#define TADPOLE_AP       10u
#define TADPOLE_SPEED    30u
#define TADPOLE_LIFE     60u
#define TADPOLE_SWIM_FRAMES 40u
#define TADPOLE_DIVE_FRAMES 20u
#define TADPOLE_FIRE_ZDIST 1500
#define TADPOLE_BANK_FRAMES ((DEG180 + DEG45) / 4)
#define TADPOLE_ESCAPE_SPEED 60u
#define BOSS1_HP             70u
#define BOSS1_AP             10u
#define BOSS1_TURRET_HP      8u
#define BOSS1_TURRET_AP      16u
#define BOSS1_COVER_AP       16u
#define BOSS1_EXP_FRAMES     38u
#define BOSS1_SPACE_VIEW_CY  (-60)
#define BOSS1_CHILD_COVER    1u
#define BOSS1_CHILD_TL0      2u
#define BOSS1_CHILD_TL1      3u
#define BOSS1_CHILD_TL2      4u
#define BOSS1_CHILD_TL3      5u
#define BOSS1_CHILD_TR0      6u
#define BOSS1_CHILD_TR1      7u
#define BOSS1_CHILD_TR2      8u
#define BOSS1_CHILD_TR3      9u
#define BOSS1_PARENT_FLAG_TURRETS_OPEN 0x40u
#define BOSS1_PARENT_FLAG_SIDE_RIGHT   0x80u
#define BOSS1_PARENT_FLAG_COVER_BLOCK  0x40u
#define BOSS1_PARENT_FLAG_COVER_GONE   0x80u
#define BOSS1_COVER_BLOCK_FRAMES     32u
#define BOSS1_COVER_CLEAR_FRAMES_EASY 50u
#define BOSS1_COVER_CLEAR_FRAMES_HARD 30u
#define BOSS1_TURRET_HOME_DELAY      4u
#define BOSS1_TURRET_FIRE_DELAY      5u
#define BOSS1_CENTER_FIRE_DELAY      15u
#define BOSS1_BACK_HPLASMA_DELAY      6u
#define BOSS1_BACK_MISSILE_DELAY     15u
#define BOSS1_CLOSE_ZDIST          300
#define BOSS1_MISSILE_ZDIST        1500
#define BOSS1_COVER_RADIUS 70
#define BOSS1_COVER_ZOFF  -300
#define BOSS1_TURRET_RADIUS 96
#define BOSS1_TURRET_SIDE_Y 50
// `boss_1_0`/`boss_1_1` are raw SHAPES2 symbols, not def_shape ids.
// Reserve stable flat ids here so the child objects exist in gameplay even
// before the renderer gets matching mesh registrations.
#define SH_BOSS_1_0       246u
#define SH_BOSS_1_1       247u
#define SH_BOSS_1_2       20u
#define BOSSA_AP            16u
#define BOSSA_TURRET_HP     12u
#define BOSSA_CUP_HP        24u
#define BOSSA_SCALE         2
#define BOSSA_EXP_FRAMES    30u
#define BOSSA_CHILD_TURRET_L 1u
#define BOSSA_CHILD_TURRET_M 2u
#define BOSSA_CHILD_TURRET_R 3u
#define BOSSA_CHILD_CUP_L    4u
#define BOSSA_CHILD_CUP_M    5u
#define BOSSA_CHILD_CUP_R    6u
#define BOSSA_PARENT_FLAG_ATTACK_DONE 0x10u
#define BOSSA_CUP_FLAG_FIRED          0x20u
#define BOSSA_TURRET_FIRE_DELAY       15u
#define BOSSA_PARENT_MISSILE_PERIOD   64u
#define BOSSA_CUP_GO_TIME             45u
#define BOSSA_CUP_RETURN_TIME         30u
#define BOSSA_CUP_STATE_COVER         0u
#define BOSSA_CUP_STATE_UP            1u
#define BOSSA_CUP_STATE_GO            2u
#define BOSSA_CUP_STATE_RETURN        3u
#define BOSSA_CUP_STATE_DOWN          4u
#define BOSSA_CUP_STATE_IROTATE       5u
#define BOSSA_CUP_STATE_ROTATE        6u
// `boss_A_*` shapes are not in the active def_shape-backed runtime yet.
// Keep stable flat ids here so the literal bossA port has deterministic shapes.
#define SH_BOSS_A_1       248u
#define SH_BOSS_A_2       249u
#define SH_BOSS_A_6       250u
#define BOSS7_HP          40
#define BOSS7_HATCH_HP    16
#define BOSS7_LAUNCHER_HP 8
#define BOSS7_SCALE       3
#define BOSS7_EXP_FRAMES  24
#define BOSS7_SPAWN_SOUND 0x5Bu
#define BOSS7_OPEN_SOUND  0x58u
#define BOSS7_HATCH_FIRE_DELAY 5u
#define BOSS7_LAUNCH_FIRE_DELAY 5u
#define SH_BOSS_7_1       56u
#define SH_BOSS_7_0       240u
#define SH_BOSS_7_1O      242u
#define SH_BOSS_7_2       243u
#define SH_BOSS_7_3       244u
#define SH_BOSS_7_4       245u
#define BOSS7_CHILD_HATCH      1u
#define BOSS7_CHILD_SHIELD     2u
#define BOSS7_CHILD_LAUNCHER_T 3u
#define BOSS7_CHILD_LAUNCHER_B 4u
#define BOSS7_SFLAG_HATCH 0x10u
#define BOSS7_SFLAG_LAUNCH 0x20u
#define BOSS7_SFLAG_SWAY   BOSS7_SFLAG_HATCH
#define HMISSILE1_SPEED   60u
#define HMISSILE1_LIFE    100u
#define HMISSILE1_AP      8u
#define HMISSILE1_CLOSE_DIST 300
#define HMISSILE1_NOCHASE_FLAG 0x01u

static bool strat_points_positive_z(const Alien *self) {
    int8 signed_yaw;

    if (!self) {
        return false;
    }

    signed_yaw = (int8)self->roty;
    return signed_yaw >= -(int8)DEG45 && signed_yaw <= (int8)DEG45;
}

static void strat_aim_yaw(Alien *self, const Alien *target, int shift);
static void strat_aim_3d(Alien *self, const Alien *target, int shift);
static void strat_move3d(Alien *self, uint8 speed, uint8 accel);
static uint8 strat_pitch_toward(const Alien *src, const Alien *dst);
static Alien *strat_obj_from_ptr(uint16 ptr);
static uint16 strat_obj_index_or_null(const Alien *al);
static void zaco2_Istrat(Alien *self);
static void zaco2_strat(Alien *self);
static void hmissile1_strat(Alien *self);
static void worm_strat(Alien *self);
static void wormexp_strat(Alien *self);
static void wormheadexp_strat(Alien *self);
static void wormsplit_init(Alien *self);
static void wormsplit_strat(Alien *self);
static void wormgo_init(Alien *self);
static void wormgo_strat(Alien *self);
static void worm2_strat(Alien *self);
static void item7_strat(Alien *self);
static void flashplayer_Istrat(Alien *self);
static void flashplayer_strat(Alien *self);
static uint8 strat_relslowelaser_speed(void);
static void boss1up_strat(Alien *self);
static void boss1normal_strat(Alien *self);
static void boss1in_strat(Alien *self);
static void boss1out_strat(Alien *self);
static void boss1inclose_strat(Alien *self);
static void boss1back_strat(Alien *self);
static void boss1cov_strat(Alien *self);
static void boss1covdie_strat(Alien *self);
static void boss1turretL_strat(Alien *self);
static void boss1turretR_strat(Alien *self);
static void boss1exp_init(Alien *self);

static int8 strat_random_centered(uint8 span) {
    uint8 rnd;

    if (span == 0u) {
        return 0;
    }

    rnd = (uint8)(SfRtl_Random() % span);
    return (int8)rnd - (int8)(span / 2u);
}

void Strat_Hard_Init(Alien *self) {
    // hard_Istrat (GSTRATS.ASM:642-646)
    // Set enemy1 collision, hard HP/AP, no per-frame strategy.
    self->collflags |= COLLTYPE_ENEMY1;
    set_hard_vars(self);
    self->stratptr = NULL;
}

void Strat_Hard180yr_Init(Alien *self) {
    // hard180YR_Istrat (GSTRATS.ASM:654-660)
    // Facing 180 degrees, enemy1 collision, hard vars, static.
    self->collflags |= COLLTYPE_ENEMY1;
    self->roty = DEG180;
    set_hard_vars(self);
    self->stratptr = NULL;
}

void Strat_Hard90yr_Init(Alien *self) {
    // KSTRATS.ASM:326-331
    // Despite the symbol name, the original routine writes deg180.
    self->collflags |= COLLTYPE_ENEMY1;
    self->roty = DEG180;
    set_hard_vars(self);
    self->stratptr = NULL;
}

void Strat_Hard180yrNZR_Init(Alien *self) {
    // hard180YRNZR_Istrat (GSTRATS.ASM:649-652)
    // Same as hard180YR but with noremove_behind set
    // (object persists even when behind the player).
    self->type &= ~ATZREMOVE;  // s_setnoremove_behind
    Strat_Hard180yr_Init(self);
}

// Per-frame strategy for rotating static objects (hardrot_Istrat)
static void hardrot_strat(Alien *self) {
    // hardrot_strat (GSTRATS.ASM:678-683)
    // Adds sbyte1/2/3 to rotx/roty/rotz each frame.
    self->rotx += self->sbyte1;
    self->roty += self->sbyte2;
    self->rotz += self->sbyte3;
}

void Strat_HardRot_Init(Alien *self) {
    // hardrot_Istrat (GSTRATS.ASM:673-677)
    self->collflags |= COLLTYPE_ENEMY1;
    set_hard_vars(self);
    self->stratptr = hardrot_strat;
}

// ============================================================
// NO-COLLISION DECORATIVE OBJECT (GSTRATS.ASM)
// ============================================================

void Strat_NoColl_Init(Alien *self) {
    // nocoll_Istrat (GSTRATS.ASM:735-739)
    // Disable collision, no per-frame strategy.
    self->sflags |= ASF_COLLDISABLE;
    self->stratptr = NULL;
}

static void rader0_strat(Alien *self) {
    if (!self) {
        return;
    }
    self->roty = (uint8)(self->roty + 8u);
}

void Strat_Rader0_Init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = rader0_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = RADER_HP;
    self->AP = RADER_AP;
    self->collflags |= (COLLTYPE_ENEMY1 | COLLTYPE_ZENEMY);
}

void Strat_Rader1_Init(Alien *self) {
    if (!self) {
        return;
    }
    self->collflags |= COLLTYPE_ENEMY1;
    Strat_Hard_Init(self);
}

static void pillar3stay_strat(Alien *self) {
    (void)self;
}

static void pillar3stay_init(Alien *self) {
    if (!self) {
        return;
    }

    self->sflags &= (uint8)~(ASF_NOHITAFFECT | ASF_SHADOW);
    self->stratptr = pillar3stay_strat;
    Sound_PlaySE(0x49);
}

static void pillar3fall_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->rotz = (uint8)(self->rotz + self->sbyte1);
    if (self->sbyte2 > 0u) {
        self->sbyte2--;
    }
    if (self->sbyte2 == 0u) {
        pillar3stay_init(self);
    }
}

static void pillar3_enter_fall(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = pillar3fall_strat;
    self->sflags |= (ASF_NOHITAFFECT | ASF_SHADOW);
    self->sbyte1 = 4u;
    if ((self->flags & AF_LEFT_PL) != 0u) {
        self->sbyte1 = (uint8)-4;
    }
    self->sbyte2 = PILLAR3_FALL_FRAMES;
}

static void pillar3_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }

    if (Strat_DistXZ(self, player) < PILLAR3_DIST ||
        self->HP < PILLAR3_FALL_HP ||
        (self->hitflags & HF2_MASK) != 0u) {
        pillar3_enter_fall(self);
    }
}

static void pillar3explode_wait(Alien *self) {
    if (!self) {
        return;
    }
    if (Strat_CountDown(self)) {
        Strat_RemoveObj();
    }
}

static void pillar3explode_strat(Alien *self) {
    if (!self) {
        return;
    }

    Sound_PlaySE(0x10);
    self->flags |= AFEXP;
    self->sflags |= ASF_COLLDISABLE;
    self->collflags = 0;
    self->stratptr = pillar3explode_wait;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    self->count = 7;
}

void Strat_Pillar3_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = pillar3_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = pillar3explode_strat;
    self->HP = PILLAR3_HP;
    self->AP = PILLAR3_AP;
    pillar3_strat(self);
}

static void skillfly_remove(void) {
    if (RAM8(WM_SKILLFLY) > 0u) {
        RAM8(WM_SKILLFLY)--;
    }
    Strat_RemoveObj();
}

static void skillfly_strat(Alien *self) {
    Alien *player;
    int16 dx;
    int16 dy;
    int16 radius;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }

    if (abs(self->worldz - player->worldz) < 200) {
        radius = self->sword1;
        if (radius < 0) {
            radius = (int16)-radius;
        }
        dx = (int16)(self->worldx - player->worldx);
        if (dx < 0) {
            dx = (int16)-dx;
        }
        dy = (int16)(self->worldy - player->worldy);
        if (dy < 0) {
            dy = (int16)-dy;
        }
        if (dx < radius && dy < radius) {
            skillfly_remove();
            return;
        }
    }

    self->worldz = (int16)(self->worldz + 1000);
    if (player->worldz >= self->worldz) {
        skillfly_remove();
        return;
    }
    self->worldz = (int16)(self->worldz - 1000);
}

void Strat_Skillfly_Init(Alien *self) {
    if (!self) {
        return;
    }
    self->sflags |= ASF_COLLDISABLE;
    if (self->shape == 0u) {
        self->sflags |= ASF_INVISIBLE;
    }
    self->stratptr = skillfly_strat;
    RAM8(WM_SKILLFLY)++;
    if (self->sword1 == 0) {
        self->sword1 = SKILLFLY_RADIUS_DEFAULT;
    }
}

static Alien *gate3_player_box(void) {
    Alien *box;

    box = Obj_GetByIndex((int)g_pcboxobj_B);
    if (!box || !box->active) {
        return NULL;
    }
    return box;
}

static bool gate_heal_player_box(uint8 sound_id, uint8 heal_amount) {
    Alien *box;
    uint16 hp;

    box = gate3_player_box();
    if (!box || box->HP == 0u) {
        return false;
    }

    hp = (uint16)box->HP + heal_amount;
    if (hp > GATE_HEAL_MAX) {
        hp = GATE_HEAL_MAX;
    }
    box->HP = (uint8)hp;
    Sound_PlaySE(sound_id);
    return true;
}

static void gate3_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    self->rotx = (uint8)(self->rotx + 8u);
    self->roty = (uint8)(self->roty + 6u);
    self->rotz = (uint8)(self->rotz + 12u);

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }

    if ((int16)abs(self->worldz - player->worldz) > GATE3_TOUCH_ZDIST) {
        return;
    }
    if ((int16)abs(self->worldx - player->worldx) > GATE3_TOUCH_XY ||
        (int16)abs(self->worldy - player->worldy) > GATE3_TOUCH_XY) {
        return;
    }

    if ((self->sflags2 & GATE3_TOUCHED_FLAG) != 0u) {
        self->colframe = 4u;
        return;
    }

    self->sflags2 |= GATE3_TOUCHED_FLAG;
    if (!gate_heal_player_box(GATE3_SOUND, GATE3_HEAL_AMOUNT)) {
        return;
    }
    self->colframe = 4u;
}

void Strat_Gate3_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = gate3_strat;
    self->sflags |= (ASF_COLLDISABLE | ASF_SHADOW);
    self->colframe = 0u;
    gate3_strat(self);
}

static void gate_spin_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->rotz = (uint8)(self->rotz + self->sbyte1);
    self->sbyte1++;

    if (self->colframe < GATE_TOUCHED_COL0 || self->colframe >= GATE_TOUCHED_COLE) {
        self->colframe = GATE_TOUCHED_COL0;
    } else {
        self->colframe++;
    }
}

static void gate_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }

    if ((int16)abs(self->worldz - player->worldz) <= GATE3_TOUCH_ZDIST &&
        (int16)abs(self->worldx - player->worldx) <= GATE3_TOUCH_XY &&
        (int16)abs(self->worldy - player->worldy) <= GATE3_TOUCH_XY &&
        gate_heal_player_box(GATE_SOUND, GATE3_HEAL_AMOUNT)) {
        self->stratptr = gate_spin_strat;
        self->colframe = 4u;
        g_maprestart = g_maprestarttemp;
        g_maprestartbank = g_maprestartbanktemp;
        g_restartbg = g_currentbg;
        g_restartpalfade = g_lastpalfade;
        g_eroll1 = 1u;
        return;
    }

    self->colframe = (uint8)((self->colframe + 1u) % GATE_NORM_COLS);
}

void Strat_Gate_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = gate_strat;
    self->HP = 1u;
    self->AP = 0u;
    self->sflags |= (ASF_COLLDISABLE | ASF_SHADOW);
    self->colframe = 0u;
    g_eroll1 = 0u;
    g_maprestarttemp = g_mapptr;
    g_maprestartbanktemp = 0u;
}

static void gate2_strat(Alien *self) {
    Alien *player;
    int16 dx;
    int16 dy;

    if (!self) {
        return;
    }

    if (self->worldx > g_maxpmoveX ||
        self->worldx < g_minpmoveX ||
        self->worldy > g_maxpmoveY ||
        self->worldy <= g_minpmoveY) {
        self->worldx = Strat_ChaseProportional(self->worldx, 0, 4);
        self->worldy = Strat_ChaseProportional(self->worldy, g_viewCY, 4);
    }

    if ((g_playerflymode & PFM_SHADOWS) != 0u && self->worldy <= GATE2_GROUND_Y) {
        self->worldy = GATE2_GROUND_Y;
    }

    self->rotx = (uint8)(self->rotx + 8u);
    self->roty = (uint8)(self->roty + 6u);
    self->rotz = (uint8)(self->rotz + 12u);

    player = Obj_GetPlayer();
    if ((self->sflags2 & GATE2_TOUCHED_FLAG) != 0u) {
        self->colframe = 0u;
    } else if (player && player->active &&
               (int16)abs(self->worldz - player->worldz) <= GATE2_TOUCH_ZDIST) {
        dx = (int16)abs(self->worldx - player->worldx);
        dy = (int16)abs(self->worldy - player->worldy);
        if (dx <= GATE2_TOUCH_XY &&
            dy <= GATE2_TOUCH_XY &&
            gate_heal_player_box(GATE3_SOUND, GATE2_HEAL_AMOUNT)) {
            g_playerscore = (uint16)(g_playerscore + GATE2_HEAL_SCORE);
            self->sflags2 |= GATE2_TOUCHED_FLAG;
            self->colframe = 0u;
        } else {
            self->colframe = 4u;
        }
    } else {
        self->colframe = 4u;
    }

    add_player_z(self);
    self->worldz = (int16)(self->worldz + GATE2_SCROLL_Z);
}

void Strat_Gate2_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = gate2_strat;
    self->sflags |= (ASF_COLLDISABLE | ASF_SHADOW);
    gate2_strat(self);
}

static Alien *boss_get_mother_obj(Alien *self);
static Alien *boss_find_child_obj(Alien *mother, uint8 child_num);
static void boss7_enter_alldead(Alien *self);
static void boss7_parent_cont(Alien *self);
static void boss7hatch_strat(Alien *self);
static void boss7hatch_coll(Alien *self);
static void boss7launcherT_strat(Alien *self);
static void boss7launcherB_strat(Alien *self);
static void boss7launcher_coll(Alien *self);
static void boss7a_strat(Alien *self);
static void boss7b_init(Alien *self);
static void boss7b_strat(Alien *self);
static void boss7c_init(Alien *self);
static void boss7c_strat(Alien *self);
static void boss7c2_init(Alien *self);
static void boss7c2_strat(Alien *self);
static void boss7d_init(Alien *self);
static void boss7d_strat(Alien *self);
static void boss7e_init(Alien *self);
static void boss7e_strat(Alien *self);
static void boss7alldead_strat(Alien *self);
static void boss7alldeada_init(Alien *self);
static void boss7alldeada_strat(Alien *self);
static void boss7alldeadc_init(Alien *self);
static void boss7alldeadc_strat(Alien *self);
static void boss7alldeadb_init(Alien *self);
static void boss7alldeadb_strat(Alien *self);
static void boss7exp_strat(Alien *self);
static void homingflat_strat(Alien *self);
static void relelaserhome_strat(Alien *self);

static uint16 boss_obj_index_or_null(const Alien *al) {
    if (!al) return 0u;
    if (al < g_aliens || al >= (g_aliens + NUMBER_AL)) return 0u;
    return (uint16)((al - g_aliens) + 1);
}

static Alien *boss_child_from_index_raw(uint16 index) {
    if (index == 0u) return NULL;
    index--;
    if (index >= NUMBER_AL) return NULL;
    return &g_aliens[index];
}

static void boss_clear_child_link(Alien *child) {
    if (!child) return;
    child->sflags3 &= (uint8)~ASF3_CHILDOBJ;
    child->ptr = 0u;
    child->sword1 = 0;
}

static void boss_prune_family_links(Alien *mother) {
    uint16 mother_idx;
    uint16 prev_idx;
    uint16 idx;
    int guard;

    if (!mother || (mother->sflags3 & ASF3_MOTHEROBJ) == 0) {
        return;
    }

    mother_idx = boss_obj_index_or_null(mother);
    prev_idx = 0u;
    idx = (uint16)mother->sword1;
    guard = NUMBER_AL + 1;
    while (idx != 0u && guard-- > 0) {
        Alien *raw = boss_child_from_index_raw(idx);
        uint16 next_idx;
        bool valid;

        if (!raw) {
            break;
        }

        next_idx = (uint16)raw->sword1;
        valid = raw->active &&
                ((raw->sflags3 & ASF3_CHILDOBJ) != 0) &&
                ((uint16)raw->ptr == mother_idx);

        if (!valid) {
            if (prev_idx == 0u) {
                mother->sword1 = (int16)next_idx;
            } else {
                Alien *prev = boss_child_from_index_raw(prev_idx);
                if (prev) {
                    prev->sword1 = (int16)next_idx;
                }
            }
            if ((uint16)raw->ptr == mother_idx) {
                boss_clear_child_link(raw);
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

static Alien *boss_get_mother_obj(Alien *self) {
    Alien *mother;

    if (!self) {
        return NULL;
    }
    if ((self->sflags3 & ASF3_CHILDOBJ) == 0) {
        return NULL;
    }

    mother = boss_child_from_index_raw((uint16)self->ptr);
    if (!mother || !mother->active) {
        boss_clear_child_link(self);
        return NULL;
    }
    return mother;
}

static Alien *boss_find_child_obj(Alien *mother, uint8 child_num) {
    uint16 idx;
    int guard;

    if (!mother) {
        return NULL;
    }

    boss_prune_family_links(mother);
    idx = (uint16)mother->sword1;
    guard = NUMBER_AL + 1;
    while (idx != 0u && guard-- > 0) {
        Alien *child = boss_child_from_index_raw(idx);
        if (!child) {
            break;
        }
        if (child->active &&
            (child->sflags3 & ASF3_CHILDOBJ) != 0 &&
            (uint16)child->ptr == boss_obj_index_or_null(mother) &&
            child->sbyte1 == child_num) {
            return child;
        }
        idx = (uint16)child->sword1;
    }
    return NULL;
}

static uint8 boss_count_children(Alien *mother) {
    uint8 count;
    uint16 idx;
    int guard;

    if (!mother) {
        return 0u;
    }

    boss_prune_family_links(mother);
    count = 0u;
    idx = (uint16)mother->sword1;
    guard = NUMBER_AL + 1;
    while (idx != 0u && guard-- > 0) {
        Alien *child = boss_child_from_index_raw(idx);
        if (!child) {
            break;
        }
        count++;
        idx = (uint16)child->sword1;
    }
    return count;
}

static bool boss_attach_child_to_mother(Alien *mother, Alien *child, uint8 child_num) {
    uint16 child_idx;
    uint16 idx;
    int guard;

    if (!mother || !child || !mother->active || !child->active || mother == child) {
        return false;
    }

    mother->sflags3 |= ASF3_MOTHEROBJ;
    child->sflags3 |= ASF3_CHILDOBJ;
    child->sbyte1 = child_num;
    child->ptr = boss_obj_index_or_null(mother);
    child->sword1 = 0;

    child_idx = boss_obj_index_or_null(child);
    if (child_idx == 0u) {
        boss_clear_child_link(child);
        return false;
    }

    if ((uint16)mother->sword1 == 0u) {
        mother->sword1 = (int16)child_idx;
        return true;
    }

    idx = (uint16)mother->sword1;
    guard = NUMBER_AL + 1;
    while (idx != 0u && guard-- > 0) {
        Alien *it = boss_child_from_index_raw(idx);
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

static void boss_keeprel_to_player(Alien *self) {
    if (!self) {
        return;
    }
    self->worldz = (int16)(self->worldz + g_playervelZ - g_pviewvelz);
}

static void boss_apply_yaw_offset(Alien *self, const Alien *mother,
                                  int16 offx, int16 offy, int16 offz) {
    float s;
    float c;
    int16 rx;
    int16 rz;

    if (!self || !mother) {
        return;
    }

    s = strat_sin(mother->roty);
    c = strat_cos(mother->roty);
    rx = (int16)lroundf(((float)offx * c) + ((float)offz * s));
    rz = (int16)lroundf(((float)offz * c) - ((float)offx * s));

    self->rotx = mother->rotx;
    self->roty = mother->roty;
    self->rotz = mother->rotz;
    self->worldx = (int16)(mother->worldx + rx);
    self->worldy = (int16)(mother->worldy + offy);
    self->worldz = (int16)(mother->worldz + rz);
}

static Alien *boss7hatchfire_srou(Alien *self) {
    Alien *child;

    if (!self) {
        return NULL;
    }

    child = Strat_MakeObj(SH_ZACO_6);
    if (!child) {
        return NULL;
    }

    boss_apply_yaw_offset(child, self, -(10 << BOSS7_SCALE), 7 << BOSS7_SCALE, 0);
    child->rotx = (uint8)(child->rotx + 3u);
    child->stratptr = zaco2_Istrat;
    return child;
}

static void hmissile1_remove(Alien *self) {
    if (!self) {
        return;
    }
    g_aldead = 1u;
}

static Alien *projectile_target_obj(const Alien *self) {
    if (!self || self->fireobjptr == 0u) {
        return NULL;
    }
    return Obj_GetByIndex((int)(self->fireobjptr - 1u));
}

static Alien *boss7launcher_fire_hmissile1(Alien *self, Alien *target) {
    Alien *shot;

    if (!self || !target || !target->active) {
        return NULL;
    }

    shot = Strat_MakeObj(0u);
    if (!shot) {
        return NULL;
    }

    boss_apply_yaw_offset(shot, self, 17 << BOSS7_SCALE, -(5 << BOSS7_SCALE), 0);
    shot->rotx = self->rotx;
    shot->roty = self->roty;
    shot->rotz = self->rotz;
    shot->stratptr = hmissile1_strat;
    shot->collstratptr = Strat_ProjectileOnCollide;
    shot->expstratptr = Strat_ProjectileOnCollide;
    shot->HP = 2u;
    shot->AP = HMISSILE1_AP;
    shot->vel = HMISSILE1_SPEED;
    shot->count = HMISSILE1_LIFE;
    shot->snd2 = 2u;
    shot->type = (uint8)(ATMISSILE | ATZREMOVE);
    shot->sflags |= ASF_SHADOW;
    shot->collflags = (uint8)(ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4);
    shot->immuneptr = strat_obj_index_or_null(self);
    shot->fireobjptr = (uint16)((target - g_aliens) + 1u);
    Strat_GenVecs3D(shot);
    return shot;
}

static Alien *boss7_fire_hplasma(Alien *self, Alien *target) {
    Alien *shot;

    if (!self || !target || !target->active) {
        return NULL;
    }

    shot = Strat_MakeObj(0u);
    if (!shot) {
        return NULL;
    }

    boss_apply_yaw_offset(shot, self, 0, 0, (40 << BOSS7_SCALE) >> 2);
    shot->rotx = self->rotx;
    shot->roty = self->roty;
    shot->rotz = self->rotz;
    shot->stratptr = homingflat_strat;
    shot->collstratptr = Strat_ProjectileOnCollide;
    shot->expstratptr = hmissile1_remove;
    shot->HP = 1u;
    shot->AP = HPLASMA_AP;
    shot->vel = HPLASMA_SPEED;
    shot->count = HPLASMA_LIFE;
    shot->snd2 = 6u;
    shot->type = (uint8)(ATLASER | ATZREMOVE);
    shot->sflags |= ASF_INVISIBLE;
    shot->collflags = (uint8)(ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4);
    shot->immuneptr = strat_obj_index_or_null(self);
    shot->fireobjptr = (uint16)((target - g_aliens) + 1u);
    shot->sbyte1 = shot->roty;
    shot->sbyte2 = shot->rotx;
    shot->rotx = 0u;
    shot->roty = DEG180;
    return shot;
}

static void hmissile1_strat(Alien *self) {
    Alien *target;
    Alien *player;
    int16 dz;

    if (!self) {
        return;
    }

    self->rotz = (uint8)(self->rotz + 10u);

    target = projectile_target_obj(self);
    if ((self->sflags2 & HMISSILE1_NOCHASE_FLAG) == 0u) {
        if (!target || !target->active) {
            target = NULL;
        } else if ((abs(self->worldx - target->worldx) +
                    abs(self->worldy - target->worldy) +
                    abs(self->worldz - target->worldz)) < HMISSILE1_CLOSE_DIST) {
            self->sflags2 |= HMISSILE1_NOCHASE_FLAG;
        } else {
            strat_aim_3d(self, target, 3);
        }

        Strat_GenVecs3D(self);
    }

    add_player_z(self);
    Strat_ApplyVelocity(self);

    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        hmissile1_remove(self);
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }

    dz = (int16)(self->worldz - player->worldz);
    if (dz < -12000 || dz > 12000) {
        hmissile1_remove(self);
        return;
    }

    if ((g_gameflags & GF_BOSSDEAD) != 0u || (g_bossflags & BF_DYING) != 0u) {
        self->sflags |= ASF_COLLDISABLE;
        self->HP = 0u;
        hmissile1_remove(self);
    }
}

static void homingflat_strat(Alien *self) {
    Alien *target;
    Alien *player;
    int16 dz;

    if (!self) {
        return;
    }

    target = projectile_target_obj(self);
    if (target && target->active && abs(self->worldz - target->worldz) >= 500) {
        achase_angle(&self->sbyte1, Strat_AngleXZ(self, target), 4);
        achase_angle(&self->sbyte2, strat_pitch_toward(self, target), 4);
    }

    self->roty = self->sbyte1;
    self->rotx = self->sbyte2;
    Strat_GenVecs3D(self);
    add_player_z(self);
    Strat_ApplyVelocity(self);

    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        hmissile1_remove(self);
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }

    dz = (int16)(self->worldz - player->worldz);
    if (dz < -12000 || dz > 12000) {
        hmissile1_remove(self);
        return;
    }

    if ((g_gameflags & GF_BOSSDEAD) != 0u || (g_bossflags & BF_DYING) != 0u) {
        self->sflags |= ASF_COLLDISABLE;
        self->HP = 0u;
        hmissile1_remove(self);
    }
}

static void relelaserhome_strat(Alien *self) {
    Alien *player;
    int16 dz;

    if (!self) {
        return;
    }

    if (self->animframe < 4u) {
        self->animframe = (uint8)(self->animframe + 2u);
        if (self->animframe > 4u) {
            self->animframe = 4u;
        }
    }

    player = Obj_GetPlayer();
    if ((self->sflags2 & RELSLOWELASERHOME_LOCK_FLAG) == 0u &&
        player && player->active) {
        if ((int16)abs(self->worldz - player->worldz) <= RELSLOWELASERHOME_CLOSE_Z) {
            self->sflags2 |= RELSLOWELASERHOME_LOCK_FLAG;
        }
        strat_aim_3d(self, player, 1);
        Strat_GenVecs3D(self);
    }

    add_player_z(self);
    Strat_ApplyVelocity(self);

    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        hmissile1_remove(self);
        return;
    }

    if (!player || !player->active) {
        return;
    }

    dz = (int16)(self->worldz - player->worldz);
    if (dz < -RELSLOWELASERHOME_OFFSCENE_Z || dz > RELSLOWELASERHOME_OFFSCENE_Z) {
        hmissile1_remove(self);
    }
}

static void boss7_part_explode(Alien *self) {
    if (!self) {
        return;
    }
    self->flags |= AFEXP;
    Sound_PlaySE(0x10u);
    g_aldead = 1u;
}

static void boss7hatch_strat(Alien *self) {
    Alien *mother;

    if (!self) {
        return;
    }

    mother = boss_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    self->sflags &= (uint8)~ASF_INVISIBLE;
    self->sflags |= ASF_NOHITAFFECT;

    if ((mother->sflags2 & BOSS7_SFLAG_HATCH) != 0u) {
        self->sflags &= (uint8)~ASF_NOHITAFFECT;
        if (self->animframe < 8u) {
            if (self->animframe == 0u) {
                Sound_PlaySE(0x5Au);
            }
            self->animframe++;
        } else {
            self->colframe = (uint8)((self->colframe + 1u) & 3u);
            if ((g_gameframe % BOSS7_HATCH_FIRE_DELAY) == 0u) {
                Alien *child = boss7hatchfire_srou(self);
                if (child) {
                    child->roty = (uint8)(child->roty - 7u);
                }
                child = boss7hatchfire_srou(self);
                if (child) {
                    child->roty = (uint8)(child->roty + 21u);
                }
            }
        }
    } else {
        self->colframe = 4u;
        if (self->animframe == 8u) {
            Sound_PlaySE(0x59u);
        }
        if (self->animframe > 0u) {
            self->animframe--;
        }
    }

    boss_apply_yaw_offset(self, mother, -(20 << BOSS7_SCALE), 0, 0);
}

static void boss7hatch_coll(Alien *self) {
    Alien *mother = boss_get_mother_obj(self);
    if (!mother || (mother->sflags2 & BOSS7_SFLAG_HATCH) == 0u) {
        self->hitflags = 0u;
        self->sflags &= (uint8)~ASF_COLLIDE;
        return;
    }
    Strat_HitFlash(self);
}

static void boss7hatch_init(Alien *self) {
    uint8 hp = BOSS7_HATCH_HP;

    if (!self) {
        return;
    }
    if (g_currentlevel == 2u) {
        hp = (uint8)(BOSS7_HATCH_HP * 2u);
    }

    self->stratptr = boss7hatch_strat;
    self->collstratptr = boss7hatch_coll;
    self->expstratptr = boss7_part_explode;
    self->HP = hp;
    self->AP = 10u;
    self->collflags |= COLLTYPE_ENEMY1;
    self->sflags |= (ASF_SHADOW | ASF_INVISIBLE);
    self->depthoffset = 1;
    self->colframe = 0u;
    self->animframe = 0u;
    g_bossmaxhp = (uint16)(g_bossmaxhp + hp);
}

static void boss7launcher_common_strat(Alien *self, int16 yoff) {
    Alien *mother;
    Alien *player;

    if (!self) {
        return;
    }

    mother = boss_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    self->sflags &= (uint8)~ASF_INVISIBLE;
    self->sflags |= ASF_NOHITAFFECT;

    if ((mother->sflags2 & BOSS7_SFLAG_LAUNCH) != 0u) {
        self->sflags &= (uint8)~ASF_NOHITAFFECT;
        self->colframe = (uint8)((self->colframe + 1u) & 3u);
        if (self->animframe < 9u) {
            if (self->animframe == 0u) {
                Sound_PlaySE(0x5Au);
            }
            self->animframe++;
        } else if ((g_gameframe % BOSS7_LAUNCH_FIRE_DELAY) == 0u) {
            player = Obj_GetPlayer();
            if (player && player->active) {
                (void)boss7launcher_fire_hmissile1(self, player);
            }
        }
    } else {
        self->colframe = 4u;
        if (self->animframe == 9u) {
            Sound_PlaySE(0x59u);
        }
        if (self->animframe > 0u) {
            self->animframe--;
        }
    }

    boss_apply_yaw_offset(self, mother, 30 << BOSS7_SCALE, yoff, 0);
}

static void boss7launcherT_strat(Alien *self) {
    boss7launcher_common_strat(self, -(15 << BOSS7_SCALE));
}

static void boss7launcherB_strat(Alien *self) {
    boss7launcher_common_strat(self, 25 << BOSS7_SCALE);
}

static void boss7launcher_coll(Alien *self) {
    Alien *mother = boss_get_mother_obj(self);
    if (!mother ||
        (mother->sflags2 & BOSS7_SFLAG_LAUNCH) == 0u ||
        (self->hitflags & HF2_MASK) == 0u) {
        self->hitflags = 0u;
        self->sflags &= (uint8)~ASF_COLLIDE;
        return;
    }
    self->hitflags = 0u;
    Strat_HitFlash(self);
}

static void boss7launcher_init_common(Alien *self, StrategyFunc strat) {
    uint8 hp = BOSS7_LAUNCHER_HP;

    if (!self) {
        return;
    }
    if (g_currentlevel == 2u) {
        hp = (uint8)(BOSS7_LAUNCHER_HP * 2u);
    }

    self->stratptr = strat;
    self->collstratptr = boss7launcher_coll;
    self->expstratptr = boss7_part_explode;
    self->HP = hp;
    self->AP = 10u;
    self->collflags |= COLLTYPE_ENEMY1;
    self->sflags |= (ASF_SHADOW | ASF_INVISIBLE);
    self->depthoffset = 1;
    self->colframe = 0u;
    self->animframe = 0u;
    g_bossmaxhp = (uint16)(g_bossmaxhp + hp);
}

static void boss7launcherT_init(Alien *self) {
    boss7launcher_init_common(self, boss7launcherT_strat);
}

static void boss7launcherB_init(Alien *self) {
    boss7launcher_init_common(self, boss7launcherB_strat);
}

static void boss7shield_strat(Alien *self) {
    Alien *mother;

    if (!self) {
        return;
    }

    mother = boss_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    self->sflags &= (uint8)~ASF_INVISIBLE;
    boss_apply_yaw_offset(self, mother, 20 << BOSS7_SCALE, 0, 0);
}

static void boss7shield_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = boss7shield_strat;
    self->collstratptr = NULL;
    self->expstratptr = boss7_part_explode;
    self->HP = HARD_HP;
    self->AP = 10u;
    self->collflags |= COLLTYPE_ENEMY1;
    self->sflags |= (ASF_SHADOW | ASF_INVISIBLE);
}

static Alien *boss7_spawn_child(Alien *mother, uint8 child_num, StrategyFunc init_fn) {
    Alien *child;
    uint16 shape = SH_BOSS_7_1;

    child = Obj_Alloc();
    if (!child) {
        return NULL;
    }
    Strat_InitObjVars(child);
    switch (child_num) {
    case BOSS7_CHILD_HATCH:      shape = SH_BOSS_7_0; break;
    case BOSS7_CHILD_SHIELD:     shape = SH_BOSS_7_2; break;
    case BOSS7_CHILD_LAUNCHER_T: shape = SH_BOSS_7_3; break;
    case BOSS7_CHILD_LAUNCHER_B: shape = SH_BOSS_7_4; break;
    default: break;
    }
    child->shape = shape;
    if (!boss_attach_child_to_mother(mother, child, child_num)) {
        Obj_Free(child);
        return NULL;
    }
    init_fn(child);
    return child;
}

static void boss7_enter_alldead(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = boss7alldead_strat;
    self->collstratptr = Strat_HitFlash;
    self->shape = SH_BOSS_7_1;
    self->sflags &= (uint8)~ASF_NOHITAFFECT;
    self->sflags2 &= (uint8)~(BOSS7_SFLAG_HATCH | BOSS7_SFLAG_LAUNCH);
    Sound_PlaySE(BOSS7_OPEN_SOUND);
}

static void boss7_parent_cont(Alien *self) {
    Alien *shield;

    if (!self) {
        return;
    }

    boss_keeprel_to_player(self);
    strat_move3d(self, self->vel, 0u);

    if (!boss_find_child_obj(self, BOSS7_CHILD_LAUNCHER_T) &&
        !boss_find_child_obj(self, BOSS7_CHILD_LAUNCHER_B)) {
        shield = boss_find_child_obj(self, BOSS7_CHILD_SHIELD);
        if (shield) {
            Obj_Free(shield);
        }
    }

    if (boss_count_children(self) == 0u) {
        boss7_enter_alldead(self);
        return;
    }

    add_player_z(self);
}

static void boss7a_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (player && player->active &&
        abs(self->worldz - player->worldz) >= (170 << BOSS7_SCALE)) {
        if ((g_gameframe & 1u) == 0u) {
            (void)Strat_SpeedTo(self, 0u, 1u);
        }
        if (self->roty != DEG180) {
            self->colframe = (uint8)((self->colframe + 1u) & 3u);
            self->roty++;
            if (self->roty == DEG90) {
                self->shape = SH_BOSS_7_1O;
            }
        }
        if (self->worldy < -(40 << BOSS7_SCALE)) {
            boss7b_init(self);
            return;
        }
        self->worldy = (int16)(self->worldy + 2);
    }

    boss7_parent_cont(self);
}

static void boss7b_init(Alien *self) {
    if (!self) {
        return;
    }
    if (!boss_find_child_obj(self, BOSS7_CHILD_HATCH)) {
        boss7c2_init(self);
        return;
    }
    self->sflags2 |= BOSS7_SFLAG_HATCH;
    self->sflags2 &= (uint8)~BOSS7_SFLAG_LAUNCH;
    self->sbyte4 = 30u;
    self->stratptr = boss7b_strat;
}

static void boss7b_strat(Alien *self) {
    if (!self) {
        return;
    }
    if (!achase_angle(&self->roty, (uint8)(DEG180 - DEG22), 4)) {
        boss7_parent_cont(self);
        return;
    }
    if (self->sbyte4 > 0u) {
        self->sbyte4--;
    }
    if (self->sbyte4 == 0u) {
        boss7d_init(self);
        return;
    }
    boss7_parent_cont(self);
}

static void boss7launch_cont(Alien *self) {
    if (!boss_find_child_obj(self, BOSS7_CHILD_HATCH) ||
        !boss_find_child_obj(self, BOSS7_CHILD_LAUNCHER_B)) {
        boss7_parent_cont(self);
        return;
    }
    if (self->worldy >= -(30 << BOSS7_SCALE)) {
        self->worldy = (int16)(self->worldy + 2);
    }
    boss7_parent_cont(self);
}

static void boss7c_init(Alien *self) {
    if (!self) {
        return;
    }
    if (!boss_find_child_obj(self, BOSS7_CHILD_SHIELD)) {
        boss7e_init(self);
        return;
    }
    self->sflags2 &= (uint8)~BOSS7_SFLAG_HATCH;
    self->sflags2 |= BOSS7_SFLAG_LAUNCH;
    self->sbyte4 = 50u;
    self->stratptr = boss7c_strat;
}

static void boss7c_strat(Alien *self) {
    if (!self) {
        return;
    }
    if (!achase_angle(&self->roty, (uint8)(DEG180 + DEG22), 4)) {
        boss7launch_cont(self);
        return;
    }
    if (self->sbyte4 > 0u) {
        self->sbyte4--;
    }
    if (self->sbyte4 == 0u) {
        boss7e_init(self);
        return;
    }
    boss7launch_cont(self);
}

static void boss7c2_init(Alien *self) {
    if (!self) {
        return;
    }
    self->sflags2 &= (uint8)~BOSS7_SFLAG_LAUNCH;
    self->sbyte4 = 15u;
    self->stratptr = boss7c2_strat;
}

static void boss7c2_strat(Alien *self) {
    if (!self) {
        return;
    }
    if (!achase_angle(&self->roty, (uint8)(DEG180 + DEG45 + DEG22), 4)) {
        boss7launch_cont(self);
        return;
    }
    if (self->sbyte4 > 0u) {
        self->sbyte4--;
    }
    if (self->sbyte4 == 0u) {
        boss7c_init(self);
        return;
    }
    boss7launch_cont(self);
}

static void boss7d_init(Alien *self) {
    if (!self) {
        return;
    }
    self->sflags2 &= (uint8)~(BOSS7_SFLAG_HATCH | BOSS7_SFLAG_LAUNCH);
    self->sbyte3 = 0u;
    self->stratptr = boss7d_strat;
}

static void boss7d_strat(Alien *self) {
    if (!self) {
        return;
    }
    self->worldy = (int16)(self->worldy - lroundf(strat_sin(self->sbyte3) * 8.0f));
    self->worldz = (int16)(self->worldz - lroundf(strat_cos(self->sbyte3) * 2.0f));
    (void)achase_angle(&self->roty, DEG180, 4);
    self->sbyte3 = (uint8)(self->sbyte3 + 4u);
    if (self->sbyte3 == 192u) {
        boss7c_init(self);
        return;
    }
    boss7_parent_cont(self);
}

static void boss7e_init(Alien *self) {
    if (!self) {
        return;
    }
    if (!boss_find_child_obj(self, BOSS7_CHILD_HATCH)) {
        boss7c2_init(self);
        return;
    }
    self->sflags2 &= (uint8)~(BOSS7_SFLAG_HATCH | BOSS7_SFLAG_LAUNCH);
    self->sbyte3 = 192u;
    self->stratptr = boss7e_strat;
}

static void boss7e_strat(Alien *self) {
    if (!self) {
        return;
    }
    self->worldy = (int16)(self->worldy - lroundf(strat_sin(self->sbyte3) * 8.0f));
    self->worldz = (int16)(self->worldz - lroundf(strat_cos(self->sbyte3) * 2.0f));
    (void)achase_angle(&self->roty, DEG180, 4);
    self->sbyte3 = (uint8)(self->sbyte3 + 4u);
    if (self->sbyte3 == 0u) {
        boss7b_init(self);
        return;
    }
    boss7_parent_cont(self);
}

static void boss7_alldead_cont(Alien *self) {
    if (!self) {
        return;
    }

    Strat_GenVecs3D(self);
    Strat_ApplyVelocity(self);
    self->colframe = (uint8)((self->colframe + 1u) & 3u);
    add_player_z(self);
}

static void boss7alldead_strat(Alien *self) {
    Alien *player;
    uint8 frame_masked;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();

    achase_angle(&self->rotz, 0u, 3);
    self->worldx = Strat_ChaseProportional(self->worldx, g_player_posx, 4);

    if (player && player->active) {
        if (abs(self->worldz - player->worldz) < 600) {
            boss7alldeada_init(self);
            boss7alldeada_strat(self);
            return;
        }
        strat_aim_yaw(self, player, 2);
    }
    (void)Strat_SpeedTo(self, 30u, 1u);
    if (self->worldy >= -(40 << BOSS7_SCALE)) {
        self->worldy = (int16)(self->worldy + 10);
    }

    frame_masked = (uint8)(g_gameframe & 31u);
    if (player && player->active && (frame_masked == 25u || frame_masked == 30u)) {
        (void)boss7_fire_hplasma(self, player);
    }

    boss7_alldead_cont(self);
}

static void boss7alldeada_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = boss7alldeada_strat;
    self->sbyte2 = (uint8)((DEG180 + DEG22) / 4);
    self->sflags2 ^= BOSS7_SFLAG_SWAY;
}

static void boss7alldeada_strat(Alien *self) {
    if (!self) {
        return;
    }

    if ((self->sflags2 & BOSS7_SFLAG_SWAY) != 0u) {
        self->rotz = (uint8)(self->rotz - 1u);
        self->roty = (uint8)(self->roty - 4u);
    } else {
        self->rotz = (uint8)(self->rotz + 1u);
        self->roty = (uint8)(self->roty + 4u);
    }

    if (self->sbyte2 == 0u) {
        boss7alldeadc_init(self);
        boss7alldeadc_strat(self);
        return;
    }
    self->sbyte2--;
    boss7_alldead_cont(self);
}

static void boss7alldeadc_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = boss7alldeadc_strat;
    self->sbyte2 = 50u;
}

static void boss7alldeadc_strat(Alien *self) {
    if (!self) {
        return;
    }

    (void)Strat_SpeedTo(self, 50u, 1u);
    (void)achase_angle(&self->rotz, 0u, 3);

    if (self->sbyte2 == 0u) {
        boss7alldeadb_init(self);
        boss7alldeadb_strat(self);
        return;
    }
    self->sbyte2--;
    boss7_alldead_cont(self);
}

static void boss7alldeadb_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = boss7alldeadb_strat;
    self->sbyte2 = (uint8)((DEG180 + DEG22) / 4);
}

static void boss7alldeadb_strat(Alien *self) {
    if (!self) {
        return;
    }

    (void)Strat_SpeedTo(self, 30u, 1u);
    if ((self->sflags2 & BOSS7_SFLAG_SWAY) != 0u) {
        self->rotz = (uint8)(self->rotz + 1u);
        self->roty = (uint8)(self->roty + 4u);
    } else {
        self->rotz = (uint8)(self->rotz - 1u);
        self->roty = (uint8)(self->roty - 4u);
    }

    if (self->sbyte2 == 0u) {
        boss7_enter_alldead(self);
        boss7alldead_strat(self);
        return;
    }
    self->sbyte2--;
    boss7_alldead_cont(self);
}

static void boss7exp_init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = boss7exp_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    self->sflags |= ASF_COLLDISABLE;
    self->flags |= AFEXP;
    self->count = BOSS7_EXP_FRAMES;
    self->vx = 0;
    self->vy = -30;
    self->vz = 20;
    self->sbyte2 = 2u;
    self->sbyte3 = 0u;
    g_bossflags |= BF_DYING;
    Sound_PlaySE(0x10u);
}

static void boss7exp_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->roty = (uint8)(self->roty + self->sbyte3);
    if (self->sbyte3 < 8u && (g_gameframe & 3u) == 0u) {
        self->sbyte3++;
    }

    self->vy = (int16)(self->vy + 1);
    Strat_ApplyVelocity(self);
    add_player_z(self);

    if (!Strat_CountDown(self)) {
        return;
    }

    g_gameflags |= GF_BOSSDEAD;
    g_bossflags &= (uint8)~BF_DYING;
    g_bossmaxhp = 0u;
    g_aldead = 1u;
}

void Strat_Boss7_Init(Alien *self) {
    uint8 hp;

    if (!self) {
        return;
    }

    hp = BOSS7_HP;
    if (g_currentlevel == 2u) {
        hp = (uint8)(BOSS7_HP * 2u);
    }

    self->stratptr = boss7a_strat;
    self->collstratptr = NULL;
    self->expstratptr = boss7exp_init;
    self->collflags |= COLLTYPE_ENEMY1;
    self->sflags |= (ASF_SHADOW | ASF_NOHITAFFECT);
    self->depthoffset = 1;
    self->HP = hp;
    self->AP = 20u;
    self->vel = 20u;
    self->sflags2 &= (uint8)~(BOSS7_SFLAG_HATCH | BOSS7_SFLAG_LAUNCH);

    g_gameflags &= (uint8)~GF_BOSSDEAD;
    g_bossflags &= (uint8)~BF_DYING;
    g_bossmaxhp = hp;
    g_meters = 1u;

    (void)boss7_spawn_child(self, BOSS7_CHILD_HATCH, boss7hatch_init);
    (void)boss7_spawn_child(self, BOSS7_CHILD_SHIELD, boss7shield_init);
    (void)boss7_spawn_child(self, BOSS7_CHILD_LAUNCHER_T, boss7launcherT_init);
    (void)boss7_spawn_child(self, BOSS7_CHILD_LAUNCHER_B, boss7launcherB_init);

    Sound_PlaySE(BOSS7_SPAWN_SOUND);
}

static void boss1normal_init(Alien *self);
static void boss1in_init(Alien *self);
static void boss1out_init(Alien *self);
static void boss1inclose_init(Alien *self);
static void boss1back_init(Alien *self);
static void boss1turretL_init(Alien *self);
static void boss1turretR_init(Alien *self);
static void boss1cov_init(Alien *self);
static void boss1exp_strat(Alien *self);
static void boss1turcol_coll(Alien *self);
static void boss1cov_coll(Alien *self);

static uint8 boss1_cover_clear_frames(void) {
    return (g_currentlevel == 1u) ? BOSS1_COVER_CLEAR_FRAMES_EASY
                                  : BOSS1_COVER_CLEAR_FRAMES_HARD;
}

static int8 boss1_random_signed(int8 range) {
    uint8 span;
    uint8 rnd;

    if (range <= 0) {
        return 0;
    }

    span = (uint8)((range * 2) + 1);
    rnd = (uint8)(SfRtl_Random() % span);
    return (int8)rnd - range;
}

static Alien *boss1_spawn_child(Alien *mother, uint8 child_num, StrategyFunc init_fn) {
    Alien *child;
    uint16 shape;

    if (!mother || !init_fn) {
        return NULL;
    }

    shape = (child_num == BOSS1_CHILD_COVER) ? SH_BOSS_1_0 : SH_BOSS_1_1;
    child = Obj_Alloc();
    if (!child) {
        return NULL;
    }

    Strat_InitObjVars(child);
    child->shape = shape;
    if (!boss_attach_child_to_mother(mother, child, child_num)) {
        Obj_Free(child);
        return NULL;
    }

    init_fn(child);
    return child;
}

static void boss1_release_children(Alien *self) {
    uint16 idx;
    int guard;

    if (!self) {
        return;
    }

    boss_prune_family_links(self);
    idx = (uint16)self->sword1;
    guard = NUMBER_AL + 1;
    while (idx != 0u && guard-- > 0) {
        Alien *child = boss_child_from_index_raw(idx);
        uint16 next_idx;

        if (!child) {
            break;
        }

        next_idx = (uint16)child->sword1;
        boss_clear_child_link(child);
        Obj_Free(child);
        idx = next_idx;
    }

    self->sword1 = 0;
    self->sflags3 &= (uint8)~ASF3_MOTHEROBJ;
}

static Alien *boss1_cover_obj(Alien *self) {
    return boss_find_child_obj(self, BOSS1_CHILD_COVER);
}

static bool boss1_child_bank_alive(Alien *self, uint8 first_child, uint8 last_child) {
    uint8 child_num;

    if (!self) {
        return false;
    }

    for (child_num = first_child; child_num <= last_child; child_num++) {
        if (boss_find_child_obj(self, child_num)) {
            return true;
        }
    }
    return false;
}

static uint8 boss1_live_turret_count(Alien *self) {
    uint8 count;
    uint8 child_num;

    if (!self) {
        return 0u;
    }

    count = 0u;
    for (child_num = BOSS1_CHILD_TL0; child_num <= BOSS1_CHILD_TR3; child_num++) {
        if (boss_find_child_obj(self, child_num)) {
            count++;
        }
    }
    return count;
}

static bool boss1_get_turret_offset(uint8 child_num, int16 *offx, int16 *offy, int16 *offz) {
    if (!offx || !offy || !offz) {
        return false;
    }

    // Raw local offsets from GBSTRATS.ASM boss1rots_srou/dobossrot_srou.
    switch (child_num) {
    case BOSS1_CHILD_TL0:
        *offx = 55;
        *offy = 0;
        *offz = 45;
        return true;
    case BOSS1_CHILD_TL1:
        *offx = 125;
        *offy = 0;
        *offz = 45;
        return true;
    case BOSS1_CHILD_TL2:
        *offx = 90;
        *offy = -25;
        *offz = 45;
        return true;
    case BOSS1_CHILD_TL3:
        *offx = 90;
        *offy = 25;
        *offz = 45;
        return true;
    case BOSS1_CHILD_TR0:
        *offx = -125;
        *offy = 0;
        *offz = 45;
        return true;
    case BOSS1_CHILD_TR1:
        *offx = -55;
        *offy = 0;
        *offz = 45;
        return true;
    case BOSS1_CHILD_TR2:
        *offx = -90;
        *offy = -25;
        *offz = 45;
        return true;
    case BOSS1_CHILD_TR3:
        *offx = -90;
        *offy = 25;
        *offz = 45;
        return true;
    default:
        return false;
    }
}

static void boss1_update_child_positions(Alien *self) {
    Alien *cover;
    uint8 child_num;

    if (!self) {
        return;
    }

    cover = boss1_cover_obj(self);
    if (cover) {
        // boss1rots_srou places the cover at parent + rotated local (sbyte4,0,0),
        // then applies a separate worldz -= 300.
        boss_apply_yaw_offset(cover, self, (int16)(int8)cover->sbyte4, 0, 0);
        cover->worldz = (int16)(cover->worldz + BOSS1_COVER_ZOFF);
        cover->rotx = self->rotx;
        cover->roty = self->roty;
        cover->rotz = self->rotz;
    } else {
        self->sflags4 &= (uint8)~BOSS1_PARENT_FLAG_COVER_BLOCK;
    }

    for (child_num = BOSS1_CHILD_TL0; child_num <= BOSS1_CHILD_TR3; child_num++) {
        Alien *child = boss_find_child_obj(self, child_num);
        int16 offx;
        int16 offy;
        int16 offz;

        if (!child || !boss1_get_turret_offset(child_num, &offx, &offy, &offz)) {
            continue;
        }

        boss_apply_yaw_offset(child, self, offx, offy, offz);
        child->rotz = self->rotz;
    }
}

static Alien *boss1_fire_hmissile1(Alien *self, Alien *target,
                                   int16 offx, int16 offy, int16 offz,
                                   uint8 pitch, uint8 yaw) {
    Alien *shot;

    if (!self || !target || !target->active) {
        return NULL;
    }

    shot = Strat_MakeObj(0u);
    if (!shot) {
        return NULL;
    }

    boss_apply_yaw_offset(shot, self, offx, offy, offz);
    shot->rotx = pitch;
    shot->roty = yaw;
    shot->rotz = self->rotz;
    shot->stratptr = hmissile1_strat;
    shot->collstratptr = Strat_ProjectileOnCollide;
    shot->expstratptr = Strat_ProjectileOnCollide;
    shot->HP = 2u;
    shot->AP = HMISSILE1_AP;
    shot->vel = HMISSILE1_SPEED;
    shot->count = HMISSILE1_LIFE;
    shot->snd2 = 2u;
    shot->type = (uint8)(ATMISSILE | ATZREMOVE);
    shot->sflags |= ASF_SHADOW;
    shot->collflags = (uint8)(ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4);
    shot->immuneptr = strat_obj_index_or_null(self);
    shot->fireobjptr = (uint16)((target - g_aliens) + 1u);
    Strat_GenVecs3D(shot);
    return shot;
}

static Alien *boss1_fire_hplasma(Alien *self, Alien *target,
                                 int16 offx, int16 offy, int16 offz,
                                 uint8 pitch, uint8 yaw) {
    Alien *shot;

    if (!self || !target || !target->active) {
        return NULL;
    }

    shot = Strat_MakeObj(0u);
    if (!shot) {
        return NULL;
    }

    boss_apply_yaw_offset(shot, self, offx, offy, offz);
    shot->rotx = pitch;
    shot->roty = yaw;
    shot->rotz = self->rotz;
    shot->stratptr = homingflat_strat;
    shot->collstratptr = Strat_ProjectileOnCollide;
    shot->expstratptr = hmissile1_remove;
    shot->HP = 1u;
    shot->AP = HPLASMA_AP;
    shot->vel = HPLASMA_SPEED;
    shot->count = HPLASMA_LIFE;
    shot->snd2 = 6u;
    shot->type = (uint8)(ATLASER | ATZREMOVE);
    shot->sflags |= ASF_INVISIBLE;
    shot->collflags = (uint8)(ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4);
    shot->immuneptr = strat_obj_index_or_null(self);
    shot->fireobjptr = (uint16)((target - g_aliens) + 1u);
    shot->sbyte1 = yaw;
    shot->sbyte2 = pitch;
    shot->rotx = 0u;
    shot->roty = DEG180;
    return shot;
}

static void boss1_fire_relslowlaser(Alien *self, Alien *target, bool homing) {
    Alien *shot;
    uint8 pitch;
    uint8 yaw;

    if (!self) {
        return;
    }

    pitch = self->rotx;
    yaw = self->roty;
    if (homing && target && target->active) {
        yaw = Strat_AngleXZ(self, target);
        pitch = strat_pitch_toward(self, target);
    }

    shot = Strat_SpawnProjectile(self,
                                 0, 0, 0,
                                 pitch, yaw,
                                 strat_relslowelaser_speed(),
                                 RELSLOWELASERHOME_LIFE,
                                 RELSLOWELASERHOME_AP,
                                 ACF_COLLTYPE4);
    if (!shot) {
        return;
    }

    boss_apply_yaw_offset(shot, self, 0, 0, 10);
    shot->rotx = pitch;
    shot->roty = yaw;
    shot->rotz = self->rotz;
    Strat_GenVecs3D(shot);
    if (homing) {
        shot->stratptr = relelaserhome_strat;
        shot->sbyte1 = pitch;
        shot->sbyte2 = yaw;
        shot->animframe = 0u;
    }
}

static void boss1_finish(Alien *self, bool allow_center_fire) {
    Alien *player;
    bool left_alive;
    bool right_alive;

    if (!self) {
        return;
    }

    left_alive = boss1_child_bank_alive(self, BOSS1_CHILD_TL0, BOSS1_CHILD_TL3);
    right_alive = boss1_child_bank_alive(self, BOSS1_CHILD_TR0, BOSS1_CHILD_TR3);

    if ((self->sflags4 & BOSS1_PARENT_FLAG_COVER_BLOCK) != 0u) {
        self->rotz = (uint8)(self->rotz + (DEG90 / 32));
    }

    if (allow_center_fire &&
        g_currentlevel != 1u &&
        (!left_alive || !right_alive) &&
        (g_gameframe % BOSS1_CENTER_FIRE_DELAY) == 0u) {
        player = Obj_GetPlayer();
        if (player && player->active) {
            uint8 yaw = Strat_AngleXZ(self, player);
            uint8 pitch = strat_pitch_toward(self, player);
            (void)boss1_fire_hmissile1(self, player, -96, 0, 0, pitch, yaw);
            (void)boss1_fire_hmissile1(self, player, 96, 0, 0, pitch, yaw);
        }
    }

    boss1_update_child_positions(self);
    if (boss1_live_turret_count(self) == 0u) {
        boss1back_init(self);
    }

    add_player_z(self);
}

static void boss1normal_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = boss1normal_strat;
    self->sbyte2 = 30u;
    self->sflags2 |= BOSS1_PARENT_FLAG_TURRETS_OPEN;
}

static void boss1in_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = boss1in_strat;
    self->sflags2 |= BOSS1_PARENT_FLAG_TURRETS_OPEN;
}

static void boss1out_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = boss1out_strat;
}

static void boss1inclose_init(Alien *self) {
    if (!self) {
        return;
    }
    self->sbyte3 = 2u;
    self->stratptr = boss1inclose_strat;
    self->sflags2 &= (uint8)~BOSS1_PARENT_FLAG_TURRETS_OPEN;
}

static void boss1back_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = boss1back_strat;
}

static void boss1up_strat(Alien *self) {
    if (!self) {
        return;
    }

    if (self->worldy <= BOSS1_SPACE_VIEW_CY) {
        boss1normal_init(self);
        boss1normal_strat(self);
        return;
    }

    self->worldy = (int16)(self->worldy - 10);
    boss1_finish(self, true);
}

static void boss1normal_strat(Alien *self) {
    if (!self) {
        return;
    }

    if (self->sbyte2 > 0u) {
        self->sbyte2--;
    }
    if (self->sbyte2 != 0u) {
        boss1_finish(self, true);
        return;
    }

    boss1in_init(self);
    boss1in_strat(self);
}

static void boss1in_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    self->worldz = (int16)(self->worldz - 15);
    player = Obj_GetPlayer();
    if (!player || !player->active ||
        (int16)abs(self->worldz - player->worldz) > 1000) {
        boss1_finish(self, true);
        return;
    }

    boss1out_init(self);
    boss1out_strat(self);
}

static void boss1out_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    self->worldz = (int16)(self->worldz + 15);
    player = Obj_GetPlayer();
    if (!player || !player->active ||
        (int16)abs(self->worldz - player->worldz) < BOSS1_MISSILE_ZDIST) {
        boss1_finish(self, true);
        return;
    }

    self->sflags2 |= BOSS1_PARENT_FLAG_TURRETS_OPEN;
    if (self->sbyte3 > 0u) {
        self->sbyte3--;
    }
    if (self->sbyte3 == 0u) {
        boss1inclose_init(self);
        boss1inclose_strat(self);
        return;
    }

    boss1normal_init(self);
    boss1normal_strat(self);
}

static void boss1inclose_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    self->worldz = (int16)(self->worldz - 25);
    player = Obj_GetPlayer();
    if (!player || !player->active ||
        (int16)abs(self->worldz - player->worldz) > BOSS1_CLOSE_ZDIST) {
        boss1_finish(self, false);
        return;
    }

    boss1out_init(self);
    boss1out_strat(self);
}

static void boss1back_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active ||
        (int16)abs(self->worldz - player->worldz) > BOSS1_MISSILE_ZDIST) {
        self->worldz = (int16)(self->worldz + 15);
        boss1_finish(self, true);
        return;
    }

    if ((self->sflags4 & BOSS1_PARENT_FLAG_COVER_GONE) == 0u) {
        Alien *cover = boss1_cover_obj(self);
        if (cover) {
            cover->stratptr = boss1covdie_strat;
            cover->collstratptr = NULL;
            boss_clear_child_link(cover);
            boss_prune_family_links(self);
            self->sflags &= (uint8)~ASF_COLLDISABLE;
            Sound_PlaySE(0x85u);
        }
        self->sflags4 |= BOSS1_PARENT_FLAG_COVER_GONE;
    }

    self->colframe = (uint8)((self->colframe + 1u) & 3u);
    self->rotz = (uint8)(self->rotz + (DEG90 / 32));

    if ((g_gameframe % BOSS1_BACK_HPLASMA_DELAY) == 0u) {
        uint8 yaw = (uint8)(Strat_AngleXZ(self, player) + boss1_random_signed(15));
        uint8 pitch = (uint8)(strat_pitch_toward(self, player) + boss1_random_signed(15));
        (void)boss1_fire_hplasma(self, player, 0, 0, 0, pitch, yaw);
    }

    if ((g_gameframe % BOSS1_BACK_MISSILE_DELAY) == 0u) {
        uint8 pitch = strat_pitch_toward(self, player);
        uint8 yaw = Strat_AngleXZ(self, player);
        (void)boss1_fire_hmissile1(self, player, 0, 0, 0,
                                   pitch, (uint8)(yaw + (DEG45 - DEG11)));
        if (g_currentlevel != 1u) {
            (void)boss1_fire_hmissile1(self, player, 0, 0, 0,
                                       pitch, (uint8)(yaw - (DEG45 - DEG11)));
        }
    }

    boss1_finish(self, false);
}

static void boss1cov_coll(Alien *self) {
    if (!self) {
        return;
    }
    self->hitflags = 0u;
    self->sflags &= (uint8)~ASF_COLLIDE;
}

static void boss1cov_init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = boss1cov_strat;
    self->collstratptr = boss1cov_coll;
    self->expstratptr = NULL;
    self->HP = HARD_HP;
    self->AP = BOSS1_COVER_AP;
    self->roty = DEG180;
    self->sbyte2 = 33u;
    self->sbyte3 = 10u;
    self->sbyte4 = (uint8)(((32 * 4) / 2) - 12);
    self->collflags |= COLLTYPE_ENEMY1;
    self->type |= ATGND;
}

static void boss1cov_strat(Alien *self) {
    Alien *mother;

    if (!self) {
        return;
    }

    mother = boss_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    g_bossflags |= BF_FLAG1;
    mother->sflags4 &= (uint8)~BOSS1_PARENT_FLAG_COVER_BLOCK;

    if (self->sbyte2 > 0u) {
        self->sbyte2--;
    }

    if (self->sbyte2 != 0u) {
        mother->sflags4 |= BOSS1_PARENT_FLAG_COVER_BLOCK;
        if ((mother->sflags2 & BOSS1_PARENT_FLAG_SIDE_RIGHT) != 0u) {
            self->sbyte4 = (uint8)(self->sbyte4 + 4u);
        } else {
            self->sbyte4 = (uint8)(self->sbyte4 - 4u);
        }
    } else {
        self->sbyte2 = 1u;
        if (self->sbyte3 > 0u) {
            self->sbyte3--;
        }
        if (self->sbyte3 == 0u) {
            Sound_PlaySE(0x2Fu);
            self->sbyte2 = BOSS1_COVER_BLOCK_FRAMES;
            self->sbyte3 = boss1_cover_clear_frames();
            mother->sflags2 ^= BOSS1_PARENT_FLAG_SIDE_RIGHT;
            mother->sflags4 |= BOSS1_PARENT_FLAG_COVER_BLOCK;
            if ((mother->sflags2 & BOSS1_PARENT_FLAG_SIDE_RIGHT) != 0u) {
                self->sbyte4 = (uint8)(self->sbyte4 + 4u);
            } else {
                self->sbyte4 = (uint8)(self->sbyte4 - 4u);
            }
        }
    }

    boss1_update_child_positions(mother);
}

static void boss1covdie_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (player && player->active &&
        self->worldz < player->worldz &&
        (int16)abs(self->worldz - player->worldz) > 1000) {
        g_aldead = 1u;
        return;
    }

    self->worldz = (int16)(self->worldz - 20);
}

static void boss1turcol_coll(Alien *self) {
    if (!self) {
        return;
    }
    if ((self->sflags & ASF_NOHITAFFECT) != 0u) {
        self->hitflags = 0u;
        self->sflags &= (uint8)~ASF_COLLIDE;
        return;
    }
    Strat_HitFlash(self);
}

static void boss1turret_init_common(Alien *self, StrategyFunc strat) {
    if (!self) {
        return;
    }

    self->stratptr = strat;
    self->collstratptr = boss1turcol_coll;
    self->expstratptr = Strat_Explode;
    self->HP = BOSS1_TURRET_HP;
    self->AP = BOSS1_TURRET_AP;
    self->roty = DEG180;
    self->collflags |= (COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP);
    self->sflags |= ASF_NOHITAFFECT;
    self->type |= ATGND;
    self->colframe = 0u;
    g_bossmaxhp = (uint16)(g_bossmaxhp + BOSS1_TURRET_HP);
}

static void boss1turretL_init(Alien *self) {
    boss1turret_init_common(self, boss1turretL_strat);
}

static void boss1turretR_init(Alien *self) {
    boss1turret_init_common(self, boss1turretR_strat);
}

static void boss1turret_common_strat(Alien *self, bool right_side) {
    Alien *mother;
    Alien *cover;
    Alien *player;
    bool side_matches;

    if (!self) {
        return;
    }

    mother = boss_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    side_matches = ((mother->sflags2 & BOSS1_PARENT_FLAG_SIDE_RIGHT) != 0u);
    if (side_matches != right_side ||
        (mother->sflags2 & BOSS1_PARENT_FLAG_TURRETS_OPEN) == 0u ||
        (mother->sflags4 & BOSS1_PARENT_FLAG_COVER_BLOCK) != 0u) {
        self->colframe = 0u;
        self->sflags |= ASF_NOHITAFFECT;
        boss1_update_child_positions(mother);
        return;
    }

    self->colframe = (uint8)((self->colframe + 1u) & 3u);
    self->sflags &= (uint8)~ASF_NOHITAFFECT;

    cover = boss1_cover_obj(mother);
    if (cover && cover->sbyte3 >= 20u) {
        boss1_update_child_positions(mother);
        return;
    }

    player = Obj_GetPlayer();
    if ((g_bossflags & BF_FLAG1) != 0u &&
        player && player->active &&
        (g_gameframe % BOSS1_TURRET_HOME_DELAY) == 0u) {
        g_bossflags &= (uint8)~BF_FLAG1;
        boss1_fire_relslowlaser(self, player, true);
    } else if ((g_gameframe % BOSS1_TURRET_FIRE_DELAY) == 0u) {
        boss1_fire_relslowlaser(self, player, false);
    }

    boss1_update_child_positions(mother);
}

static void boss1turretL_strat(Alien *self) {
    boss1turret_common_strat(self, false);
}

static void boss1turretR_strat(Alien *self) {
    boss1turret_common_strat(self, true);
}

static void boss1exp_init(Alien *self) {
    if (!self) {
        return;
    }

    boss1_release_children(self);
    self->stratptr = boss1exp_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    self->sflags |= ASF_COLLDISABLE;
    self->flags |= AFEXP;
    self->count = BOSS1_EXP_FRAMES;
    g_bossflags |= BF_DYING;
    Sound_PlaySE(0x10u);
}

static void boss1exp_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->rotz = (uint8)(self->rotz + (DEG90 / 32));
    add_player_z(self);

    if (!Strat_CountDown(self)) {
        return;
    }

    g_gameflags |= GF_BOSSDEAD;
    g_bossflags &= (uint8)~(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3);
    g_bossmaxhp = 0u;
    g_aldead = 1u;
}

void Strat_Boss1_Init(Alien *self) {
    uint8 hp;

    if (!self) {
        return;
    }

    hp = (g_currentlevel == 1u) ? (uint8)(BOSS1_HP / 2u) : BOSS1_HP;

    self->stratptr = boss1up_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = boss1exp_init;
    self->HP = hp;
    self->AP = BOSS1_AP;
    self->roty = DEG180;
    self->collflags |= COLLTYPE_ENEMY1;
    self->type |= ATGND;
    self->colframe = 0u;
    self->sflags |= (ASF_SHADOW | ASF_COLLDISABLE);
    self->sbyte3 = 1u;
    self->sflags2 &= (uint8)~(BOSS1_PARENT_FLAG_TURRETS_OPEN | BOSS1_PARENT_FLAG_SIDE_RIGHT);
    self->sflags4 &= (uint8)~(BOSS1_PARENT_FLAG_COVER_BLOCK | BOSS1_PARENT_FLAG_COVER_GONE);

    g_gameflags &= (uint8)~GF_BOSSDEAD;
    g_bossflags &= (uint8)~(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3);
    g_bossmaxhp = hp;
    g_meters = 1u;

    // Spawn cover last so its state update runs before the turrets in the
    // head-inserted active list and drives their same-frame fire gating.
    (void)boss1_spawn_child(self, BOSS1_CHILD_TL0, boss1turretL_init);
    (void)boss1_spawn_child(self, BOSS1_CHILD_TL1, boss1turretL_init);
    (void)boss1_spawn_child(self, BOSS1_CHILD_TL2, boss1turretL_init);
    (void)boss1_spawn_child(self, BOSS1_CHILD_TL3, boss1turretL_init);
    (void)boss1_spawn_child(self, BOSS1_CHILD_TR0, boss1turretR_init);
    (void)boss1_spawn_child(self, BOSS1_CHILD_TR1, boss1turretR_init);
    (void)boss1_spawn_child(self, BOSS1_CHILD_TR2, boss1turretR_init);
    (void)boss1_spawn_child(self, BOSS1_CHILD_TR3, boss1turretR_init);
    (void)boss1_spawn_child(self, BOSS1_CHILD_COVER, boss1cov_init);

    boss1_update_child_positions(self);
    Sound_PlaySE(0x82u);
}

static void bossAup_init(Alien *self);
static void bossAup_strat(Alien *self);
static void bossAcover_init(Alien *self);
static void bossAcover_strat(Alien *self);
static void bossAattack_init(Alien *self);
static void bossAattack_strat(Alien *self);
static void bossAexp_init(Alien *self);
static void bossAexp_strat(Alien *self);

static bool bossA_get_turret_offset(uint8 child_num, int16 *offx, int16 *offy) {
    if (!offx || !offy) {
        return false;
    }

    switch (child_num) {
    case BOSSA_CHILD_TURRET_L:
        *offx = -85 << BOSSA_SCALE;
        *offy = -50 << BOSSA_SCALE;
        return true;
    case BOSSA_CHILD_TURRET_M:
        *offx = 0;
        *offy = -40 << BOSSA_SCALE;
        return true;
    case BOSSA_CHILD_TURRET_R:
        *offx = 85 << BOSSA_SCALE;
        *offy = -50 << BOSSA_SCALE;
        return true;
    default:
        return false;
    }
}

static Alien *bossA_spawn_child(Alien *mother, uint8 child_num, StrategyFunc init_fn) {
    Alien *child;
    uint16 shape;

    if (!mother || !init_fn) {
        return NULL;
    }

    shape = (child_num <= BOSSA_CHILD_TURRET_R) ? SH_BOSS_A_1 : SH_BOSS_A_6;
    child = Obj_Alloc();
    if (!child) {
        return NULL;
    }

    Strat_InitObjVars(child);
    child->shape = shape;
    if (!boss_attach_child_to_mother(mother, child, child_num)) {
        Obj_Free(child);
        return NULL;
    }

    init_fn(child);
    return child;
}

static void bossA_release_children(Alien *self) {
    uint16 idx;
    int guard;

    if (!self) {
        return;
    }

    boss_prune_family_links(self);
    idx = (uint16)self->sword1;
    guard = NUMBER_AL + 1;
    while (idx != 0u && guard-- > 0) {
        Alien *child = boss_child_from_index_raw(idx);
        uint16 next_idx;

        if (!child) {
            break;
        }

        next_idx = (uint16)child->sword1;
        boss_clear_child_link(child);
        Obj_Free(child);
        idx = next_idx;
    }

    self->sword1 = 0;
    self->sflags3 &= (uint8)~ASF3_MOTHEROBJ;
}

static uint8 bossA_live_turret_count(Alien *self) {
    uint8 count;
    uint8 child_num;

    if (!self) {
        return 0u;
    }

    count = 0u;
    for (child_num = BOSSA_CHILD_TURRET_L; child_num <= BOSSA_CHILD_TURRET_R; child_num++) {
        if (boss_find_child_obj(self, child_num)) {
            count++;
        }
    }
    return count;
}

static uint8 bossA_live_cup_count(Alien *self) {
    uint8 count;
    uint8 child_num;

    if (!self) {
        return 0u;
    }

    count = 0u;
    for (child_num = BOSSA_CHILD_CUP_L; child_num <= BOSSA_CHILD_CUP_R; child_num++) {
        if (boss_find_child_obj(self, child_num)) {
            count++;
        }
    }
    return count;
}

static Alien *bossA_linked_turret(Alien *cup) {
    Alien *mother;

    if (!cup) {
        return NULL;
    }

    mother = boss_get_mother_obj(cup);
    if (!mother || cup->sbyte2 < BOSSA_CHILD_TURRET_L || cup->sbyte2 > BOSSA_CHILD_TURRET_R) {
        return NULL;
    }
    return boss_find_child_obj(mother, cup->sbyte2);
}

static void bossA_update_turret_position(Alien *self) {
    Alien *mother;
    int16 offx;
    int16 offy;

    mother = boss_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }
    if (!bossA_get_turret_offset(self->sbyte1, &offx, &offy)) {
        return;
    }

    boss_apply_yaw_offset(self, mother, offx, offy, 0);
    self->rotx = 0u;
    self->rotz = mother->rotz;
}

static void bossA_cup_set_home(Alien *self, int16 lift) {
    Alien *mother;
    Alien *turret;
    int16 offx;
    int16 offy;

    mother = boss_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    turret = bossA_linked_turret(self);
    if (turret && turret->active) {
        self->worldx = turret->worldx;
        self->worldy = (int16)(turret->worldy - lift);
        self->worldz = turret->worldz;
    } else if (bossA_get_turret_offset(self->sbyte2, &offx, &offy)) {
        boss_apply_yaw_offset(self, mother, offx, (int16)(offy - lift), 0);
    }

    self->rotx = (uint8)-DEG90;
    self->roty = DEG180;
    self->rotz = mother->roty;
}

static void bossA_cup_chase_home(Alien *self, int16 lift) {
    Alien *mother;
    Alien *turret;
    int16 offx;
    int16 offy;

    mother = boss_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    turret = bossA_linked_turret(self);
    if (turret && turret->active) {
        self->worldx = Strat_ChaseProportional(self->worldx, turret->worldx, 2);
        self->worldy = Strat_ChaseProportional(self->worldy,
                                               (int16)(turret->worldy - lift), 3);
        self->worldz = Strat_ChaseProportional(self->worldz, turret->worldz, 2);
        return;
    }

    if (bossA_get_turret_offset(self->sbyte2, &offx, &offy)) {
        boss_apply_yaw_offset(self, mother, offx, (int16)(offy - lift), 0);
    }
}

static Alien *bossA_pick_live_cup(Alien *self) {
    Alien *cups[3];
    uint8 child_num;
    uint8 count;

    if (!self) {
        return NULL;
    }

    count = 0u;
    for (child_num = BOSSA_CHILD_CUP_L; child_num <= BOSSA_CHILD_CUP_R; child_num++) {
        Alien *child = boss_find_child_obj(self, child_num);
        if (child) {
            cups[count++] = child;
        }
    }

    if (count == 0u) {
        return NULL;
    }
    return cups[SfRtl_Random() % count];
}

static void bossA_set_all_cup_state(Alien *self, uint8 state) {
    uint8 child_num;

    if (!self) {
        return;
    }

    for (child_num = BOSSA_CHILD_CUP_L; child_num <= BOSSA_CHILD_CUP_R; child_num++) {
        Alien *child = boss_find_child_obj(self, child_num);
        if (!child) {
            continue;
        }
        child->stratstate = state;
        if (state == BOSSA_CUP_STATE_GO || state == BOSSA_CUP_STATE_IROTATE) {
            child->sflags2 &= (uint8)~BOSSA_CUP_FLAG_FIRED;
            child->count = BOSSA_CUP_GO_TIME;
        } else if (state == BOSSA_CUP_STATE_RETURN) {
            child->count = BOSSA_CUP_RETURN_TIME;
        }
    }
}

static void bossA_update_turret_targets(Alien *self) {
    static const uint8 s_targets[3][3] = {
        { 0u, 0u, DEG180 },
        { 0u, DEG180, 0u },
        { DEG180, 0u, 0u },
    };
    uint8 pattern;
    uint8 child_num;

    if (!self) {
        return;
    }

    pattern = (uint8)(self->sbyte3 % 3u);
    for (child_num = BOSSA_CHILD_TURRET_L; child_num <= BOSSA_CHILD_TURRET_R; child_num++) {
        Alien *child = boss_find_child_obj(self, child_num);
        if (!child) {
            continue;
        }
        child->sbyte3 = s_targets[pattern][child_num - BOSSA_CHILD_TURRET_L];
    }
}

static void bossA_part_coll(Alien *self) {
    if (!self || (self->sflags & (ASF_INVISIBLE | ASF_NOHITAFFECT)) != 0u) {
        if (self) {
            self->hitflags = 0u;
            self->sflags &= (uint8)~ASF_COLLIDE;
        }
        return;
    }
    Strat_HitFlash(self);
}

static void bossA_turret_exp_init(Alien *self) {
    Alien *mother = boss_get_mother_obj(self);

    if (mother) {
        mother->sflags4 |= BOSSA_PARENT_FLAG_ATTACK_DONE;
    }
    Sound_PlaySE(0x10u);
    g_aldead = 1u;
}

static void bossA_cup_exp_init(Alien *self) {
    Alien *mother = boss_get_mother_obj(self);

    if (mother) {
        mother->sflags4 |= BOSSA_PARENT_FLAG_ATTACK_DONE;
    }
    Sound_PlaySE(0x10u);
    g_aldead = 1u;
}

static void bossA_turret_init_common(Alien *self, uint8 initial_target) {
    if (!self) {
        return;
    }

    self->stratptr = NULL;
    self->collstratptr = bossA_part_coll;
    self->expstratptr = bossA_turret_exp_init;
    self->HP = BOSSA_TURRET_HP;
    self->AP = BOSSA_AP;
    self->sbyte2 = 60u;
    self->sbyte3 = initial_target;
    self->collflags |= (COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP);
    self->sflags |= (ASF_SHADOW | ASF_NOHITAFFECT);
    self->type &= (uint8)~ATZREMOVE;
    bossA_update_turret_position(self);
}

static void bossA_turretL_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    bossA_update_turret_position(self);
    player = Obj_GetPlayer();
    if ((self->sflags & ASF_NOHITAFFECT) == 0u &&
        player && player->active &&
        strat_points_positive_z(self) &&
        (g_gameframe % BOSSA_TURRET_FIRE_DELAY) == 0u) {
        (void)boss1_fire_hplasma(self, player,
                                 0, -(10 << BOSSA_SCALE), 0,
                                 strat_pitch_toward(self, player),
                                 Strat_AngleXZ(self, player));
    }
    (void)achase_angle(&self->roty, self->sbyte3, 3);
    add_player_z(self);
}

static void bossA_turretM_strat(Alien *self) {
    bossA_turretL_strat(self);
}

static void bossA_turretR_strat(Alien *self) {
    bossA_turretL_strat(self);
}

static void bossA_turretL_init(Alien *self) {
    bossA_turret_init_common(self, 0u);
    self->stratptr = bossA_turretL_strat;
}

static void bossA_turretM_init(Alien *self) {
    bossA_turret_init_common(self, DEG180);
    self->stratptr = bossA_turretM_strat;
}

static void bossA_turretR_init(Alien *self) {
    bossA_turret_init_common(self, 0u);
    self->stratptr = bossA_turretR_strat;
}

static void bossA_cup_init_common(Alien *self, uint8 turret_child_num) {
    if (!self) {
        return;
    }

    self->stratptr = NULL;
    self->collstratptr = bossA_part_coll;
    self->expstratptr = bossA_cup_exp_init;
    self->HP = BOSSA_CUP_HP;
    self->AP = BOSSA_AP;
    self->sbyte2 = turret_child_num;
    self->stratstate = BOSSA_CUP_STATE_COVER;
    self->collflags |= (COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP);
    self->sflags |= (ASF_SHADOW | ASF_NOHITAFFECT);
    self->type &= (uint8)~ATZREMOVE;
    self->animframe = 0u;
    bossA_cup_set_home(self, 15 << BOSSA_SCALE);
}

static void bossA_cup_strat(Alien *self) {
    Alien *mother;
    Alien *player;

    if (!self) {
        return;
    }

    mother = boss_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    self->sflags &= (uint8)~ASF_INVISIBLE;
    player = Obj_GetPlayer();

    switch (self->stratstate) {
    case BOSSA_CUP_STATE_COVER:
        self->sflags |= ASF_NOHITAFFECT;
        if (self->animframe > 0u && (g_gameframe & 1u) == 0u) {
            self->animframe--;
        }
        bossA_cup_set_home(self, 15 << BOSSA_SCALE);
        break;
    case BOSSA_CUP_STATE_UP: {
        Alien *turret = bossA_linked_turret(self);
        self->sflags |= ASF_NOHITAFFECT;
        if (self->animframe < 7u && (g_gameframe & 1u) == 0u) {
            self->animframe++;
        }
        bossA_cup_chase_home(self, 110 << BOSSA_SCALE);
        self->rotz = (uint8)(self->rotz + 20u);
        (void)achase_angle(&self->rotx, (uint8)-DEG90, 3);
        if (turret) {
            turret->sflags &= (uint8)~ASF_NOHITAFFECT;
        }
        break;
    }
    case BOSSA_CUP_STATE_IROTATE:
        self->stratstate = BOSSA_CUP_STATE_ROTATE;
        self->sword2 = 40;
        self->sflags &= (uint8)~ASF_NOHITAFFECT;
        /* fallthrough */
    case BOSSA_CUP_STATE_ROTATE:
        bossA_cup_chase_home(self, 70 << BOSSA_SCALE);
        (void)achase_angle(&self->rotx, 0u, 3);
        self->rotz = (uint8)(self->rotz + 28u);
        if (self->sword2 > 0) {
            self->sword2--;
        }
        if (self->sword2 == 0) {
            mother->sflags4 |= BOSSA_PARENT_FLAG_ATTACK_DONE;
            self->stratstate = BOSSA_CUP_STATE_UP;
            self->sflags |= ASF_NOHITAFFECT;
        }
        break;
    case BOSSA_CUP_STATE_GO:
        if ((self->sflags2 & BOSSA_CUP_FLAG_FIRED) == 0u) {
            Sound_PlaySE(0x66u);
            self->sflags2 |= BOSSA_CUP_FLAG_FIRED;
            self->sflags &= (uint8)~ASF_NOHITAFFECT;
        }
        if (player && player->active) {
            strat_aim_3d(self, player, 3);
            if (self->worldz >= player->worldz ||
                (int16)abs(self->worldz - player->worldz) < 200) {
                self->worldy = -(100 << BOSSA_SCALE);
                self->count = BOSSA_CUP_RETURN_TIME;
                self->stratstate = BOSSA_CUP_STATE_RETURN;
                break;
            }
            if ((self->sflags2 & BOSSA_CUP_FLAG_FIRED) != 0u &&
                (g_gameframe % 12u) == 0u) {
                (void)boss1_fire_hmissile1(self, player, 0, 0, 0,
                                           strat_pitch_toward(self, player),
                                           Strat_AngleXZ(self, player));
            }
        }
        (void)Strat_SpeedTo(self, 45u, 1u);
        self->rotz = (uint8)(self->rotz + 28u);
        (void)achase_angle(&self->rotx, 0u, 2);
        Strat_GenVecs3D(self);
        Strat_ApplyVelocity(self);
        add_player_z(self);
        if (self->count > 0u) {
            self->count--;
        }
        if (self->count == 0u) {
            self->count = BOSSA_CUP_RETURN_TIME;
            self->stratstate = BOSSA_CUP_STATE_RETURN;
        }
        return;
    case BOSSA_CUP_STATE_RETURN:
        self->sflags |= ASF_NOHITAFFECT;
        bossA_cup_chase_home(self, 110 << BOSSA_SCALE);
        self->rotz = (uint8)(self->rotz + 20u);
        (void)achase_angle(&self->rotx, (uint8)-DEG90, 3);
        if (self->count > 0u) {
            self->count--;
        }
        if (self->count == 0u) {
            mother->sflags4 |= BOSSA_PARENT_FLAG_ATTACK_DONE;
            self->stratstate = BOSSA_CUP_STATE_UP;
        }
        break;
    case BOSSA_CUP_STATE_DOWN: {
        Alien *turret = bossA_linked_turret(self);
        self->sflags |= ASF_NOHITAFFECT;
        if (self->animframe > 0u && (g_gameframe & 1u) == 0u) {
            self->animframe--;
        }
        bossA_cup_chase_home(self, 15 << BOSSA_SCALE);
        (void)achase_angle(&self->rotz, 0u, 3);
        (void)achase_angle(&self->rotx, (uint8)-DEG90, 3);
        if (turret) {
            turret->sflags |= ASF_NOHITAFFECT;
            turret->HP = BOSSA_TURRET_HP;
        }
        break;
    }
    default:
        self->stratstate = BOSSA_CUP_STATE_COVER;
        bossA_cup_set_home(self, 15 << BOSSA_SCALE);
        break;
    }

    add_player_z(self);
}

static void bossA_cupL_init(Alien *self) {
    bossA_cup_init_common(self, BOSSA_CHILD_TURRET_L);
    self->stratptr = bossA_cup_strat;
}

static void bossA_cupM_init(Alien *self) {
    bossA_cup_init_common(self, BOSSA_CHILD_TURRET_M);
    self->stratptr = bossA_cup_strat;
}

static void bossA_cupR_init(Alien *self) {
    bossA_cup_init_common(self, BOSSA_CHILD_TURRET_R);
    self->stratptr = bossA_cup_strat;
}

static void bossA_parent_continue(Alien *self) {
    if (!self) {
        return;
    }

    if (bossA_live_turret_count(self) == 0u && bossA_live_cup_count(self) == 0u) {
        bossAexp_init(self);
        return;
    }

    if (self->worldx > 210 && self->vx < 0) {
        self->worldx = (int16)(self->worldx + self->vx);
        self->vx++;
        if (self->vx > 0) {
            self->vx = 0;
        }
    }

    boss_keeprel_to_player(self);
    add_player_z(self);
}

static void bossAup_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = bossAup_strat;
    self->sbyte2 = 30u;
    Sound_PlaySE(0x73u);
}

static void bossAup_strat(Alien *self) {
    if (!self) {
        return;
    }
    bossA_set_all_cup_state(self, BOSSA_CUP_STATE_UP);
    if (self->sbyte2 > 0u) {
        self->sbyte2--;
    }
    if (self->sbyte2 == 0u) {
        bossAattack_init(self);
        return;
    }
    bossA_parent_continue(self);
}

static void bossAcover_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = bossAcover_strat;
    self->sbyte2 = 50u;
    Sound_PlaySE(0x72u);
}

static void bossAcover_strat(Alien *self) {
    if (!self) {
        return;
    }
    if (self->sbyte2 > 0u) {
        self->sbyte2--;
    }
    if (self->sbyte2 > 20u) {
        bossA_set_all_cup_state(self, BOSSA_CUP_STATE_DOWN);
    } else {
        if (self->sbyte2 == 19u) {
            Sound_PlaySE(0x73u);
        }
        bossA_set_all_cup_state(self, BOSSA_CUP_STATE_UP);
    }
    if (self->sbyte2 == 0u) {
        bossAattack_init(self);
        return;
    }
    bossA_parent_continue(self);
}

static void bossAattack_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = bossAattack_strat;
    self->sbyte2 = 3u;
    self->sbyte3 = 0u;
    self->sflags4 |= BOSSA_PARENT_FLAG_ATTACK_DONE;
    bossA_set_all_cup_state(self, BOSSA_CUP_STATE_UP);
}

static void bossAattack_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    if ((self->sflags4 & BOSSA_PARENT_FLAG_ATTACK_DONE) != 0u) {
        Alien *cup;
        uint8 live_cups;

        self->sflags4 &= (uint8)~BOSSA_PARENT_FLAG_ATTACK_DONE;
        live_cups = bossA_live_cup_count(self);
        if (live_cups == 0u) {
            bossAcover_init(self);
            return;
        }
        if (self->sbyte2 == 0u) {
            bossAcover_init(self);
            return;
        }
        cup = bossA_pick_live_cup(self);
        if (cup) {
            cup->stratstate = (live_cups == 2u)
                                  ? BOSSA_CUP_STATE_IROTATE
                                  : BOSSA_CUP_STATE_GO;
            cup->sflags2 &= (uint8)~BOSSA_CUP_FLAG_FIRED;
            cup->count = BOSSA_CUP_GO_TIME;
            self->sbyte2--;
        }
    }

    if ((g_gameframe % 5u) == 0u) {
        self->sbyte3 = (uint8)((self->sbyte3 + 1u) % 3u);
    }
    bossA_update_turret_targets(self);

    player = Obj_GetPlayer();
    if (player && player->active && self->sbyte3 == 2u) {
        uint8 phase = (uint8)(g_gameframe % BOSSA_PARENT_MISSILE_PERIOD);
        if (phase == 20u) {
            (void)boss1_fire_hmissile1(self, player, 0, -(25 << BOSSA_SCALE), 0,
                                       0u, (uint8)(self->roty - DEG22));
        } else if (phase == 25u) {
            (void)boss1_fire_hmissile1(self, player, 0, -(25 << BOSSA_SCALE), 0,
                                       0u, self->roty);
        } else if (phase == 30u) {
            (void)boss1_fire_hmissile1(self, player, 0, -(25 << BOSSA_SCALE), 0,
                                       0u, (uint8)(self->roty + DEG22));
        }
    }

    bossA_parent_continue(self);
}

static void bossA_strat(Alien *self) {
    if (!self) {
        return;
    }

    if (self->roty != DEG180) {
        self->roty++;
        bossA_parent_continue(self);
        return;
    }

    bossAup_init(self);
}

static void bossAexp_init(Alien *self) {
    if (!self) {
        return;
    }

    bossA_release_children(self);
    self->stratptr = bossAexp_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    self->sflags |= ASF_COLLDISABLE;
    self->flags |= AFEXP;
    self->count = BOSSA_EXP_FRAMES;
    g_bossflags |= BF_DYING;
    Sound_PlaySE(0x10u);
}

static void bossAexp_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->rotz = (uint8)(self->rotz + (DEG90 / 24));
    add_player_z(self);

    if (!Strat_CountDown(self)) {
        return;
    }

    g_gameflags |= GF_BOSSDEAD;
    g_bossflags &= (uint8)~(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3);
    g_bossmaxhp = 0u;
    g_aldead = 1u;
}

void Strat_BossA_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->shape = SH_BOSS_A_2;
    self->stratptr = bossA_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = bossAexp_init;
    set_hard_vars(self);
    self->roty = DEG11;
    self->vx = -20;
    self->collflags |= (COLLTYPE_ENEMY2 | COLLTYPE_ENEMYWEAP);
    self->sflags |= ASF_SHADOW;
    self->sbyte2 = 0u;
    self->sbyte3 = 0u;
    self->sflags4 &= (uint8)~BOSSA_PARENT_FLAG_ATTACK_DONE;

    g_gameflags &= (uint8)~GF_BOSSDEAD;
    g_bossflags &= (uint8)~(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3);
    g_bossmaxhp = (uint16)((BOSSA_TURRET_HP * 3u) + (BOSSA_CUP_HP * 3u));
    g_meters = 1u;

    (void)bossA_spawn_child(self, BOSSA_CHILD_TURRET_L, bossA_turretL_init);
    (void)bossA_spawn_child(self, BOSSA_CHILD_TURRET_M, bossA_turretM_init);
    (void)bossA_spawn_child(self, BOSSA_CHILD_TURRET_R, bossA_turretR_init);
    (void)bossA_spawn_child(self, BOSSA_CHILD_CUP_L, bossA_cupL_init);
    (void)bossA_spawn_child(self, BOSSA_CHILD_CUP_M, bossA_cupM_init);
    (void)bossA_spawn_child(self, BOSSA_CHILD_CUP_R, bossA_cupR_init);
    bossA_update_turret_targets(self);
    Sound_PlaySE(0x83u);
}

static void tow0explode_wait(Alien *self) {
    if (!self) {
        return;
    }
    if (Strat_CountDown(self)) {
        Strat_RemoveObj();
    }
}

void Strat_Tow0Explode(Alien *self) {
    Alien *child;

    if (!self) {
        return;
    }

    // `tow_0` also flags its linked `tow_1` child through path script when it
    // dies. Mirror that here until generic path WHENDEAD handling is ported.
    child = (self->ptr != 0u) ? Obj_GetByIndex((int)self->ptr) : NULL;
    if (child && child->active) {
        child->sflags4 |= ASF4_SFLAG8;
    }

    Sound_PlaySE(0x10);

    self->flags |= AFEXP;
    self->sflags |= ASF_COLLDISABLE;
    self->collflags = 0;
    self->stratptr = tow0explode_wait;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    self->count = 7;
}

static void item5_strat(Alien *self);

static void zaco2_cont(Alien *self);
static void zaco2loop_init(Alien *self);
static void zaco2loop_strat(Alien *self);
static void zaco2dash_init(Alien *self);
static void zaco2dash_strat(Alien *self);

static void zaco2_reset_main(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = zaco2_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
}

static void zaco2_Istrat(Alien *self) {
    if (!self) {
        return;
    }

    self->HP = ZACO2_HP;
    self->AP = ZACO2_AP;
    self->vel = 50u;
    self->sbyte1 = 15u;
    self->sbyte2 = 3u;
    self->collflags |= (COLLTYPE_ENEMYWEAP | COLLTYPE_ENEMY1 | COLLTYPE_ZENEMY);
    self->type &= (uint8)~ATZREMOVE;
    self->snd2 = 0x0Fu;
    Strat_GenVecs3D(self);
    zaco2_reset_main(self);
}

static void zaco2_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    if (self->sbyte1 != 0u) {
        self->sbyte1--;
        zaco2_cont(self);
        return;
    }

    player = Obj_GetPlayer();
    if (player && player->active) {
        if (abs(self->worldz - player->worldz) < 500) {
            zaco2loop_init(self);
            return;
        }
        strat_aim_3d(self, player, 3);
    }

    zaco2_cont(self);
}

static void zaco2_cont(Alien *self) {
    if (!self) {
        return;
    }

    if ((g_bossflags & BF_DYING) != 0u) {
        Strat_RemoveObj();
        return;
    }

    Strat_GenVecs3D(self);
    Strat_ApplyVelocity(self);
    if (self->worldy <= 0) {
        self->worldy = 0;
        self->rotx = (uint8)(-self->rotx);
    }
    add_player_z(self);
}

static void zaco2loop_init(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    if (self->sbyte2 == 0u) {
        zaco2dash_init(self);
        return;
    }
    self->sbyte2--;
    self->stratptr = zaco2loop_strat;
    self->sbyte1 = (uint8)(DEG180 / 4);

    if (g_currentlevel != 1u) {
        player = Obj_GetPlayer();
        if (player && player->active) {
            (void)boss7launcher_fire_hmissile1(self, player);
        }
    }

    zaco2loop_strat(self);
}

static void zaco2loop_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    if (self->sbyte1 != 0u) {
        self->sbyte1--;
        if ((self->flags & AF_LEFT_PL) != 0u) {
            self->rotz = (uint8)(self->rotz + 10u);
            self->roty = (uint8)(self->roty + 4u);
        } else {
            self->rotz = (uint8)(self->rotz - 10u);
            self->roty = (uint8)(self->roty - 4u);
        }
        zaco2_cont(self);
        return;
    }

    player = Obj_GetPlayer();
    if (player && player->active &&
        abs(self->worldz - player->worldz) > 2000) {
        zaco2_reset_main(self);
        zaco2_strat(self);
        return;
    }

    zaco2_cont(self);
}

static void zaco2dash_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = zaco2dash_strat;
    self->count = 30u;
    zaco2dash_strat(self);
}

static void zaco2dash_strat(Alien *self) {
    if (!self) {
        return;
    }

    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        self->sflags |= ASF_COLLDISABLE;
        self->HP = 0u;
    }

    zaco2_cont(self);
}

static int16 strat_tab_scaled(uint8 angle, bool use_sin, int shift) {
    int16 value = (int16)((use_sin ? strat_sin(angle) : strat_cos(angle)) * 127.0f);

    if (shift < 0) {
        return (int16)(value >> -shift);
    }
    if (shift > 0) {
        return (int16)(value << shift);
    }
    return value;
}

static void worm_kill(Alien *self) {
    if (!self) {
        return;
    }
    Strat_RemoveObj();
}

static void worm_common_init(Alien *self, StrategyFunc expstrat) {
    if (!self) {
        return;
    }

    self->stratptr = worm_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = expstrat;
    self->roty = DEG180;
    self->HP = WORM_HP;
    self->AP = WORM_AP;
    self->collflags |= COLLTYPE_ENEMY1;
    self->vz = -10;
}

void Strat_Wormhead_Init(Alien *self) {
    if (!self) {
        return;
    }

    g_gasflags &= (uint8)~(GASF_KILLTYPE1 | GASF_KILLTYPE2);
    self->snd2 = 5u;
    worm_common_init(self, wormheadexp_strat);
}

void Strat_Worm_Init(Alien *self) {
    worm_common_init(self, wormexp_strat);
}

static void worm_strat(Alien *self) {
    Alien *link;

    if (!self) {
        return;
    }

    self->vx = strat_tab_scaled(self->sbyte2, true, -4);
    self->vy = strat_tab_scaled(self->sbyte2, false, -4);
    self->sbyte2 = (uint8)(self->sbyte2 + 4u);

    link = strat_obj_from_ptr((uint16)self->sword1);
    if (!link || !link->active || link->HP != 0u) {
        if ((g_gasflags & GASF_KILLTYPE2) != 0u) {
            wormsplit_init(self);
            return;
        }
    } else if ((g_gasflags & GASF_KILLTYPE1) != 0u) {
        worm_kill(self);
        return;
    }

    Strat_ApplyVelocity(self);
    add_player_z(self);
}

static void wormexp_strat(Alien *self) {
    if ((g_gasflags & GASF_KILLTYPE1) == 0u) {
        g_gasflags |= GASF_KILLTYPE2;
    }
    Strat_Explode(self);
}

static void wormheadexp_strat(Alien *self) {
    g_gasflags |= GASF_KILLTYPE1;
    Strat_Explode(self);
}

static void wormsplit_init(Alien *self) {
    if (!self) {
        return;
    }

    self->vx = (int16)((SfRtl_Random() & 63u) - 32);
    self->vy = (int16)((SfRtl_Random() & 63u) - 32);
    self->vz = 0;
    self->sbyte2 = 18u;
    self->stratptr = wormsplit_strat;
}

static void wormsplit_strat(Alien *self) {
    if (!self) {
        return;
    }

    if (self->sbyte2 == 0u) {
        wormgo_init(self);
        return;
    }

    self->sbyte2--;
    Strat_ApplyVelocity(self);
    add_player_z(self);
}

static void wormgo_init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = wormgo_strat;
    self->vx = 0;
    self->vy = 0;
    self->vz = -10;
}

static void wormgo_strat(Alien *self) {
    if (!self) {
        return;
    }

    if ((self->flags & AF_LEFT_PL) != 0u) {
        self->vx = (int16)(self->vx - 1);
    } else {
        self->vx = (int16)(self->vx + 1);
    }

    Strat_ApplyVelocity(self);
}

void Strat_Worm2_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = worm2_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = WORM2_HP;
    self->AP = WORM2_AP;
    self->collflags |= COLLTYPE_ENEMY1;
    self->stratstate = 0u;
    self->sbyte3 = 10u;
    self->rotz = (uint8)SfRtl_Random();
    self->count = 120u;
    self->snd2 = 5u;
    self->type &= (uint8)~ATZREMOVE;
}

static void worm2_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->rotz = (uint8)(self->rotz + 3u);

    if (self->sbyte3 > 0u) {
        self->sbyte3--;
    }
    if (self->sbyte3 == 0u) {
        self->sbyte3 = 32u;
        self->stratstate++;
        if (self->stratstate > 4u) {
            self->stratstate = 1u;
        }
    }

    switch (self->stratstate) {
    case 1u:
        self->vx = strat_tab_scaled(self->sbyte1, true, -3);
        self->vy = strat_tab_scaled(self->sbyte1, false, -3);
        self->vz = 0;
        self->sbyte1 = (uint8)(self->sbyte1 + 4u);
        break;
    case 2u:
        self->vx = strat_tab_scaled(self->sbyte1, true, -3);
        self->vy = strat_tab_scaled(self->sbyte1, false, -3);
        self->vz = strat_tab_scaled(self->sbyte1, false, 0);
        self->sbyte1 = (uint8)(self->sbyte1 + 4u);
        break;
    case 3u:
        self->vx = strat_tab_scaled(self->sbyte1, true, -3);
        self->vy = strat_tab_scaled(self->sbyte1, false, -3);
        self->vz = strat_tab_scaled(self->sbyte1, false, 1);
        self->sbyte1 = (uint8)(self->sbyte1 + 4u);
        break;
    case 4u:
        self->vx = strat_tab_scaled(self->sbyte1, true, -3);
        self->vy = strat_tab_scaled(self->sbyte1, false, -3);
        self->vz = strat_tab_scaled(self->sbyte1, false, 2);
        self->sbyte1 = (uint8)(self->sbyte1 + 4u);
        break;
    default:
        break;
    }

    if ((g_gameframe & 1u) == 0u) {
        if (self->count > 0u) {
            self->count--;
        }
        if (self->count == 0u) {
            Strat_RemoveObj();
            return;
        }
    }

    Strat_ApplyVelocity(self);
    add_player_z(self);
}

static void item5_init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = item5_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    self->sflags |= ASF_COLLDISABLE;
}

void Strat_Item5_Init(Alien *self) {
    if (!self) {
        return;
    }

    item5_init(self);
    item5_strat(self);
}

static void item5_collect(Alien *self) {
    if (!self) {
        return;
    }

    if (g_specwepcnt < ITEM5_MAX_SPEC) {
        g_specwepcnt++;
        Sound_PlaySE(0x18);
        g_playerscore = (uint16)(g_playerscore + ITEM5_SCORE);
    }

    flashplayer_Istrat(self);
}

static void item5_strat(Alien *self) {
    Alien *player;
    int16 zdist;
    int16 xydist;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        Strat_RemoveObj();
        return;
    }

    if (self->sbyte1 == 0u) {
        self->worldz = (int16)(self->worldz + 20);
    }

    zdist = (int16)abs(self->worldz - player->worldz);
    if (zdist > ITEM5_PICKUP_Z) {
        return;
    }

    xydist = (int16)abs(self->worldx - player->worldx);
    xydist = (int16)(xydist + (int16)abs(self->worldy - player->worldy));
    if (xydist > ITEM5_PICKUP_XY) {
        return;
    }

    item5_collect(self);
}

static void itemtorange_srou(Alien *self) {
    int16 min_y;

    if (!self) {
        return;
    }

    min_y = (int16)(g_minpmoveY + 50);
    if (self->worldy >= min_y) {
        self->worldy = (int16)(self->worldy + 3);
    }
}

static void item_repair_player_wings(void) {
    g_pshipflags &= (uint8)~(PSF_BRKLWING | PSF_LWINGCOLL |
                             PSF_BRKRWING | PSF_RWINGCOLL);
}

static uint16 flashplayer_wire_shape(void) {
    uint8 wing_breaks = g_pshipflags & (PSF_BRKLWING | PSF_BRKRWING);

    if (wing_breaks == 0u) {
        return SH_MY_W_PROXY;
    }
    // `setYplayershape_l` selects the right-wing mesh when the left wing is
    // broken, and vice versa, because the surviving side names the variant.
    if (wing_breaks == PSF_BRKLWING) {
        return SH_MY_R_W_PROXY;
    }
    if (wing_breaks == PSF_BRKRWING) {
        return SH_MY_L_W_PROXY;
    }
    return SH_MY_B_W_PROXY;
}

void Strat_Item7_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = item7_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    self->sflags |= ASF_COLLDISABLE;
}

static void flashplayer_Istrat(Alien *self) {
    if (!self) {
        return;
    }

    if (g_splayerflymode == SPFM_INSIDE) {
        Strat_RemoveObj();
        return;
    }

    self->count = 20u;
    self->sflags |= ASF_COLLDISABLE;
    self->colframe = 0u;
    self->stratptr = flashplayer_strat;
}

static void flashplayer_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active || (g_pshipflags2 & PSF2_PLAYERHP0) != 0u) {
        Strat_RemoveObj();
        return;
    }

    self->rotx = player->rotx;
    self->roty = player->roty;
    self->rotz = player->rotz;
    self->worldx = player->worldx;
    self->worldy = player->worldy;
    self->worldz = player->worldz;

    if ((g_gameframe & 1u) == 0u) {
        self->shape = 0u;
    } else {
        self->shape = flashplayer_wire_shape();
        self->colframe = (uint8)((self->colframe + 1u) & 3u);
    }

    if (self->count > 0u) {
        self->count--;
    }
    if (self->count == 0u) {
        Strat_RemoveObj();
    }
}

static void item7_strat(Alien *self) {
    Alien *player;
    int16 zdist;
    int16 xydist;
    bool needs_repair;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active || (g_pshipflags2 & PSF2_PLAYERHP0) != 0u) {
        Strat_RemoveObj();
        return;
    }

    if (self->sbyte1 == 0u) {
        self->worldz = (int16)(self->worldz + 20);
    }

    itemtorange_srou(self);

    self->roty = (uint8)(self->roty + 4u);
    self->rotz = (uint8)(self->rotz + 4u);

    zdist = (int16)abs(self->worldz - player->worldz);
    if (zdist > ITEM7_PICKUP_Z) {
        return;
    }

    xydist = (int16)abs(self->worldx - player->worldx);
    xydist = (int16)(xydist + (int16)abs(self->worldy - player->worldy));
    if (xydist > ITEM7_PICKUP_XY) {
        return;
    }

    needs_repair = (g_pshipflags & (PSF_BRKLWING | PSF_BRKRWING)) != 0u;
    item_repair_player_wings();

    if (needs_repair) {
        // `ripair_Istrat` is not ported yet; apply its gameplay effect here
        // and keep the collected-item flash follow-up on the pickup object.
        Sound_PlaySE(0x17u);
        flashplayer_Istrat(self);
        return;
    }

    Sound_PlaySE(0x15u);
    g_playerscore = (uint16)(g_playerscore + ITEM7_SCORE);

    if ((g_pshipflags2 & PSF2_DOUBLASER) == 0u) {
        g_pshipflags2 |= PSF2_DOUBLASER;
    } else {
        g_pshipflags3 |= PSF3_BEAMBALL;
    }

    flashplayer_Istrat(self);
}

static void bomwing_move_scroll_only(Alien *self) {
    if (!self) {
        return;
    }
    self->worldz = (int16)(self->worldz + 35);
}

static void bomwing_phase1(Alien *self);
static void bomwing_phase2(Alien *self);

static void bomwing_reset_phase1(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = bomwing_phase1;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = self->expstratptr ? self->expstratptr : Strat_Explode;
    self->sbyte1 = 20u;
}

static void bomwing_enter_phase2(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = bomwing_phase2;
    self->sbyte1 = (uint8)((DEG360 / 3) / 2);
}

static void bomwing_fire(Alien *self, Alien *player) {
    Alien *shot;

    if (!self || !player) {
        return;
    }

    shot = Strat_SpawnProjectile(self,
                                 0, 0, 0,
                                 (uint8)(self->rotx - DEG22), self->roty,
                                 HPLASMA_SPEED, HPLASMA_LIFE, HPLASMA_AP,
                                 ACF_COLLTYPE4);
    if (shot) {
        shot->ptr = strat_obj_index_or_null(player);
    }
}

static void bomwing_phase1(Alien *self) {
    if (!self) {
        return;
    }

    // s_beqdec_alvar branches before the decrement.
    if (self->sbyte1 == 0u) {
        bomwing_enter_phase2(self);
        bomwing_phase2(self);
        return;
    }

    self->sbyte1--;
    Strat_GenVecs2D(self);
    Strat_ApplyVelocity(self);
    bomwing_move_scroll_only(self);
}

static void bomwing_phase2(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (player && player->active) {
        if ((int16)abs(self->worldz - player->worldz) <= 3000 &&
            !strat_points_positive_z(self) &&
            (g_gameframe & 7u) == 0u) {
            bomwing_fire(self, player);
        }
    }

    self->roty = (uint8)(self->roty + 2u);

    // s_beqdec_alvar branches before the decrement and falls through into
    // bomwing_init, which immediately executes phase 1 logic in the same tick.
    if (self->sbyte1 == 0u) {
        bomwing_reset_phase1(self);
        bomwing_phase1(self);
        return;
    }

    self->sbyte1--;
    bomwing_move_scroll_only(self);
}

static void bomwing_die(Alien *self) {
    Alien *drop;

    if (!self) {
        return;
    }

    drop = Strat_MakeObj(SH_ITEM_5);
    if (drop) {
        item5_init(drop);
        drop->worldx = self->worldx;
        drop->worldy = (int16)(self->worldy - 20);
        drop->worldz = self->worldz;
    }

    Strat_Explode(self);
}

void Strat_Bomwing_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->HP = BOMWING_HP;
    self->AP = BOMWING_AP;
    self->vel = BOMWING_SPEED;
    self->roty = DEG45;
    self->collflags |= COLLTYPE_ENEMY1;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = bomwing_die;
    bomwing_reset_phase1(self);
    bomwing_phase1(self);
}

static uint16 strat_obj_index_or_null(const Alien *al) {
    if (!al) {
        return 0u;
    }
    if (al < g_aliens || al >= (g_aliens + NUMBER_AL)) {
        return 0u;
    }
    return (uint16)(al - g_aliens);
}

static Alien *strat_find_near_shape(const Alien *self, uint16 shape_id,
                                    Alien *exclude, int16 max_z, int16 max_xy) {
    Alien *best = NULL;
    int32 best_metric = 0x7FFFFFFF;
    uint16 mapped_shape = shape_id;

    if (!self) {
        return NULL;
    }

    if (shape_id < 256u) {
        mapped_shape = g_shapes_table[(uint8)shape_id];
    }

    for (Alien *it = g_active_list; it; it = it->next) {
        int16 dx;
        int16 dy;
        int16 dz;
        int32 metric;

        if (!it->active || it == self || it == exclude) {
            continue;
        }
        if (!(it->shape == shape_id || it->shape == mapped_shape)) {
            continue;
        }

        dz = (int16)abs(it->worldz - self->worldz);
        dx = (int16)abs(it->worldx - self->worldx);
        dy = (int16)abs(it->worldy - self->worldy);
        if (dz > max_z || dx > max_xy || dy > max_xy) {
            continue;
        }

        metric = (int32)dx + (int32)dy + (int32)dz;
        if (metric < best_metric) {
            best_metric = metric;
            best = it;
        }
    }

    return best;
}

static Alien *strat_obj_from_ptr(uint16 ptr) {
    if (ptr == 0u) {
        return NULL;
    }
    return Obj_GetByIndex((int)ptr - 1);
}

static uint8 strat_pitch_toward(const Alien *src, const Alien *dst) {
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

static void strat_aim_yaw(Alien *self, const Alien *target, int shift) {
    if (!self || !target) {
        return;
    }
    achase_angle(&self->roty, Strat_AngleXZ(self, target), shift);
}

static void strat_aim_3d(Alien *self, const Alien *target, int shift) {
    if (!self || !target) {
        return;
    }
    achase_angle(&self->roty, Strat_AngleXZ(self, target), shift);
    achase_angle(&self->rotx, strat_pitch_toward(self, target), shift);
}

static void strat_move3d(Alien *self, uint8 speed, uint8 accel) {
    if (!self) {
        return;
    }
    if (accel != 0u) {
        (void)Strat_SpeedTo(self, speed, accel);
    } else {
        self->vel = speed;
    }
    Strat_GenVecs3D(self);
    Strat_ApplyVelocity(self);
}

static void strat_fire_relslowlaser(Alien *self, uint8 pitch, uint8 yaw) {
    if (!self) {
        return;
    }
    (void)Strat_SpawnProjectile(self,
                                0, 0, 0,
                                pitch, yaw,
                                52, 55, 2,
                                ACF_COLLTYPE4);
}

static uint8 strat_relslowelaser_speed(void) {
    return (g_currentlevel == 1u) ? 48u : 60u;
}

static void strat_fire_relslowlaserhome(Alien *self, uint8 pitch, uint8 yaw) {
    Alien *shot;

    if (!self) {
        return;
    }

    shot = Strat_SpawnProjectile(self,
                                 0, 0, 0,
                                 pitch, yaw,
                                 strat_relslowelaser_speed(),
                                 RELSLOWELASERHOME_LIFE,
                                 RELSLOWELASERHOME_AP,
                                 ACF_COLLTYPE4);
    if (!shot) {
        return;
    }

    shot->stratptr = relelaserhome_strat;
    shot->rotx = pitch;
    shot->roty = yaw;
    shot->sbyte1 = pitch;
    shot->sbyte2 = yaw;
    shot->animframe = 0u;
}

static void zacos_phase0(Alien *self);
static void zacos_phase1(Alien *self);
static void zacos_phase2(Alien *self);
static void zaco3_attack(Alien *self);
static void zaco3_circle(Alien *self);
static void zaco3_flyaway(Alien *self);
static void zaco3die_init(Alien *self);
static void zaco3die_strat(Alien *self);
static void zaco3go_strat(Alien *self);

static void tadpole_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();

    switch (self->stratstate) {
    case 0u:
        if ((self->sflags2 & TADPOLE_SIDE_FLAG) != 0u) {
            achase_angle(&self->roty, DEG90, 4);
            self->rotx = (uint8)(self->rotx - 2u);
        } else {
            achase_angle(&self->roty, (uint8)-DEG90, 4);
            self->rotx = (uint8)(self->rotx + 2u);
        }

        if (self->sbyte1 == 0u) {
            self->stratstate++;
        } else {
            self->sbyte1--;
        }
        break;

    case 1u:
        self->sbyte1 = TADPOLE_DIVE_FRAMES;
        if (player && player->active) {
            strat_aim_3d(self, player, 3);
            if ((int16)abs(self->worldz - player->worldz) <= TADPOLE_FIRE_ZDIST) {
                strat_fire_relslowlaserhome(self, self->rotx, self->roty);
                self->stratstate++;
            }
        }
        break;

    case 2u:
        self->rotz = (uint8)(self->rotz + 2u);
        self->rotx = (uint8)(self->rotx + 2u);
        if (self->sbyte1 > 0u) {
            self->sbyte1--;
        }
        if (self->sbyte1 == 0u) {
            self->stratstate++;
            self->sbyte1 = TADPOLE_BANK_FRAMES;
        }
        break;

    case 3u:
        if (self->sbyte1 == 0u) {
            self->stratstate++;
            break;
        }

        self->sbyte1--;
        self->rotz = (uint8)(self->rotz + 8u);
        self->rotx = (uint8)(self->rotx - 4u);
        break;

    default:
        self->rotz = (uint8)(self->rotz + 8u);
        self->rotx = (uint8)(self->rotx - 1u);
        if (self->count > 0u) {
            self->count--;
            if (self->count == 0u) {
                Strat_RemoveObj();
                return;
            }
        }
        (void)Strat_SpeedTo(self, TADPOLE_ESCAPE_SPEED, 1u);
        self->roty = (uint8)(self->roty - 2u);
        if ((self->sflags2 & TADPOLE_SIDE_FLAG) != 0u) {
            self->roty = (uint8)(self->roty + 4u);
        }
        break;
    }

    Strat_GenVecs3D(self);
    Strat_ApplyVelocity(self);
    add_player_z(self);
}

void Strat_Tadpole_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = tadpole_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = TADPOLE_HP;
    self->AP = TADPOLE_AP;
    self->vel = TADPOLE_SPEED;
    self->count = TADPOLE_LIFE;
    self->collflags |= COLLTYPE_ENEMY1;
    self->stratstate = 0u;
    self->sbyte1 = TADPOLE_SWIM_FRAMES;

    if (self->worldx >= 0) {
        self->sflags2 |= TADPOLE_SIDE_FLAG;
    }

    tadpole_strat(self);
}

static uint8 strat_phase_offset(const Alien *self) {
    if (!self || self < g_aliens || self >= (g_aliens + NUMBER_AL)) {
        return 0u;
    }
    return (uint8)(self - g_aliens);
}

static void spacebarshoot_apply_spacemist(Alien *self) {
    int16 bucket;
    uint8 frame;

    if (!self) {
        return;
    }

    // s_spacemist x (STRATMAC.INC:7307-7329)
    bucket = (int16)((self->worldz - g_pviewposz + 500) >> 9);
    frame = (uint8)bucket;
    if ((int8)(frame - 8u) >= 0) {
        frame = 7u;
    }
    self->colframe = (uint8)(0x80u | frame);
}

static void spacebarwalker_strat(Alien *self) {
    Alien *player;
    uint8 fire_pitch;
    uint8 fire_yaw;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }

    achase_angle(&self->roty, Strat_AngleXZ(self, player), 1);

    if (player->worldz >= self->worldz) {
        return;
    }

    if ((((uint8)g_gameframe) + strat_phase_offset(self)) & 0x0Fu) {
        return;
    }

    fire_pitch = strat_pitch_toward(self, player);
    fire_yaw = Strat_AngleXZ(self, player);
    (void)Strat_SpawnProjectile(self,
                                0, -20, 0,
                                fire_pitch, fire_yaw,
                                52, 55, 2,
                                ACF_COLLTYPE4);
}

void Strat_Spacebarwalker_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = spacebarwalker_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = 4u;
    self->AP = 4u;
    self->collflags |= COLLTYPE_ENEMY1;
}

static void spacebarshoot_strat(Alien *self) {
    if (!self) {
        return;
    }

    // spacebarshoot_strat (GA2STRAT.ASM:1818-1825)
    self->rotz = (uint8)(self->rotz + self->sbyte1);
    self->worldx += self->sword1;
    self->worldy += self->sword2;
    spacebarshoot_apply_spacemist(self);

    if (self->count > 0u) {
        self->count--;
        if (self->count == 0u) {
            g_aldead = 1u;
        }
    }
}

void Strat_Spacebarshoot_Init(Alien *self) {
    if (!self) {
        return;
    }

    // spacebarshoot_Istrat (GA2STRAT.ASM:1809-1816)
    self->stratptr = spacebarshoot_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = NULL;
    set_hard_vars(self);
    self->collflags |= COLLTYPE_ENEMY1;
    self->count = 80u;
}

static void item0_strat(Alien *self);
static void up1man_strat(Alien *self);
static void up1manhit_Istrat(Alien *self);
static void up1manchild_strat(Alien *self);

static void item0_Istrat(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = item0_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    self->sflags |= ASF_COLLDISABLE;
    self->rotx = (uint8)-DEG90;
}

static void up1man_remove_child_slot(Alien *mother, uint8 child_num) {
    Alien *child;

    if (!mother) {
        return;
    }

    child = boss_find_child_obj(mother, child_num);
    if (!child) {
        return;
    }

    boss_clear_child_link(child);
    boss_prune_family_links(mother);
    Obj_Free(child);
}

static void item0_strat(Alien *self) {
    Alien *player;
    Alien *mother;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active || (g_pshipflags2 & PSF2_PLAYERHP0) != 0u) {
        Strat_RemoveObj();
        return;
    }

    self->worldz = (int16)(self->worldz + UP1MAN_SCROLL_Z);

    if ((int16)abs(self->worldz - player->worldz) > UP1MAN_PICKUP_Z) {
        return;
    }
    if ((int16)abs(self->worldy - player->worldy) > UP1MAN_PICKUP_Y) {
        return;
    }
    if ((int16)abs(self->worldx - player->worldx) > UP1MAN_PICKUP_X) {
        return;
    }

    Sound_PlaySE(0x0Eu);
    g_lives++;

    mother = boss_child_from_index_raw((uint16)self->sword1);
    if (mother && mother->active) {
        up1man_remove_child_slot(mother, 1u);
        up1man_remove_child_slot(mother, 3u);
        up1man_remove_child_slot(mother, 4u);
    }

    Strat_RemoveObj();
}

static Alien *up1man_spawn_child(Alien *mother,
                                 uint8 child_num,
                                 int8 off_x,
                                 int8 off_y,
                                 uint8 rot_off) {
    Alien *child;

    if (!mother) {
        return NULL;
    }

    child = Strat_MakeObj(SH_UP1_MAN_PROXY);
    if (!child) {
        return NULL;
    }

    child->stratptr = up1manchild_strat;
    child->collstratptr = up1manhit_Istrat;
    child->expstratptr = NULL;
    child->HP = HARD_HP;
    child->AP = UP1MAN_AP;
    child->collflags |= COLLTYPE_ENEMYWEAP;
    child->sbyte2 = (uint8)off_x;
    child->sbyte3 = (uint8)off_y;
    child->sbyte4 = rot_off;

    if (!boss_attach_child_to_mother(mother, child, child_num)) {
        Obj_Free(child);
        return NULL;
    }

    up1manchild_strat(child);
    return child;
}

void Strat_Up1man_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = up1man_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
    self->sflags |= ASF_COLLDISABLE;
    self->sbyte2 = UP1MAN_ROT_SPEED;
    self->ptr = 0u;
    self->sword1 = 0;
    self->sbyte1 = 0u;

    (void)up1man_spawn_child(self, 1u, -80, 75, (uint8)(DEG45 + DEG90));
    (void)up1man_spawn_child(self, 2u, 80, 75, (uint8)(DEG45 + DEG180));
    (void)up1man_spawn_child(self, 3u, 0, -90, 0u);
    up1man_strat(self);
}

static void up1man_strat(Alien *self) {
    Alien *player;
    Alien *item;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }

    if (self->sbyte3 != 0u) {
        self->rotz = (uint8)(self->rotz + self->sbyte2);
    }

    if ((int16)abs(self->worldz - player->worldz) <= UP1MAN_ACTIVE_Z) {
        self->worldz = (int16)(self->worldz + UP1MAN_SCROLL_Z);
    }

    if (self->sbyte3 != 3u || (self->sflags2 & UP1MAN_SFLAG1) != 0u) {
        return;
    }

    self->sflags2 |= UP1MAN_SFLAG1;
    item = Strat_MakeObj(SH_MYSHIP_4);
    if (!item) {
        return;
    }

    item->worldx = self->worldx;
    item->worldy = self->worldy;
    item->worldz = self->worldz;
    item->sword1 = (int16)boss_obj_index_or_null(self);
    item0_Istrat(item);
}

static void up1manhit_Istrat(Alien *self) {
    Alien *mother;

    if (!self) {
        return;
    }

    if ((self->sflags2 & UP1MAN_SFLAG1) == 0u) {
        Sound_PlaySE(0x10u);
        mother = boss_get_mother_obj(self);
        if (mother) {
            mother->sbyte2 = (uint8)(mother->sbyte2 + 2u);
            mother->sbyte3++;
        }
        self->sflags2 |= UP1MAN_SFLAG1;
        self->sflags &= (uint8)~ASF_COLLIDE;
        self->sflags |= ASF_COLLDISABLE;
    }

    up1manchild_strat(self);
}

static void up1manchild_strat(Alien *self) {
    Alien *mother;
    float s;
    float c;
    int16 off_x;
    int16 off_y;

    if (!self) {
        return;
    }

    mother = boss_get_mother_obj(self);
    if (!mother) {
        g_aldead = 1u;
        return;
    }

    s = strat_sin(mother->rotz);
    c = strat_cos(mother->rotz);
    off_x = (int16)((float)(int8)self->sbyte2 * c - (float)(int8)self->sbyte3 * s);
    off_y = (int16)((float)(int8)self->sbyte2 * s + (float)(int8)self->sbyte3 * c);

    self->worldx = (int16)(mother->worldx + off_x);
    self->worldy = (int16)(mother->worldy + off_y);
    self->worldz = mother->worldz;
    self->rotz = (uint8)(mother->rotz + self->sbyte4);

    if ((self->sflags2 & UP1MAN_SFLAG1) != 0u &&
        ((((uint8)g_gameframe) + strat_phase_offset(self)) & 1u) == 0u) {
        self->sflags |= ASF_HITFLASH;
    }
}

static void zacos_move(Alien *self) {
    if (!self) {
        return;
    }
    Strat_GenVecs3D(self);
    Strat_ApplyVelocity(self);
    add_player_z(self);
}

void Strat_Zacos_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = zacos_phase0;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = ZACOS_HP;
    self->AP = ZACOS_AP;
    self->vel = 40;
    self->rotx = DEG90;
    self->roty = DEG180;
    self->worldx += g_player_posx;
    self->sflags |= ASF_SHADOW;
    self->collflags |= COLLTYPE_ZENEMY;
    self->snd2 = 0x0Fu;
}

static void zacos_phase0(Alien *self) {
    int16 target_y;

    if (!self) {
        return;
    }

    target_y = (int16)(g_player_posy - 800);
    if (self->worldy <= target_y) {
        if (self->rotx == 0u) {
            strat_fire_relslowlaser(self, self->rotx, self->roty);
            self->stratptr = zacos_phase1;
        } else {
            self->rotx = (uint8)(self->rotx - 2u);
        }
    }

    zacos_move(self);
}

static void zacos_phase1(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (player && abs(self->worldz - player->worldz) < 2000) {
        self->rotx = (uint8)(self->rotx - 4u);
        self->stratptr = zacos_phase2;
    }

    zacos_move(self);
}

static void zacos_phase2(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    (void)Strat_SpeedTo(self, 60, 1);
    player = Obj_GetPlayer();
    if (player) {
        strat_aim_3d(self, player, 3);
    }
    zacos_move(self);
}

static void tower0_strat(Alien *self) {
    if (!self) {
        return;
    }
    self->roty = (uint8)(self->roty + 8u);
}

void Strat_Tower0_Init(Alien *self) {
    if (!self) {
        return;
    }
    self->HP = HARD_HP;
    self->AP = (uint8)(HARD_AP / 2u);
    self->stratptr = tower0_strat;
}

static Alien *strat_find_near_colltype(const Alien *self, uint8 colltype_mask,
                                       int16 max_z, int16 max_xy) {
    Alien *best = NULL;
    int32 best_metric = 0x7FFFFFFF;

    if (!self) {
        return NULL;
    }

    for (Alien *it = g_active_list; it; it = it->next) {
        int16 dx;
        int16 dy;
        int16 dz;
        int32 metric;

        if (!it->active || it == self) {
            continue;
        }
        if ((it->collflags & colltype_mask) == 0u) {
            continue;
        }

        dz = (int16)abs(it->worldz - self->worldz);
        dx = (int16)abs(it->worldx - self->worldx);
        dy = (int16)abs(it->worldy - self->worldy);
        if (dz > max_z || dx > max_xy || dy > max_xy) {
            continue;
        }

        metric = (int32)dx + (int32)dy + (int32)dz;
        if (metric < best_metric) {
            best_metric = metric;
            best = it;
        }
    }

    return best;
}

static Alien *houdai_target(const Alien *self) {
    return self ? strat_obj_from_ptr((uint16)self->sword1) : NULL;
}

static void houdai_fire(Alien *self) {
    Alien *shot;

    if (!self) {
        return;
    }

    shot = Strat_SpawnProjectile(self,
                                 0, (int16)(-62 >> 2), (int16)(40 >> 2),
                                 (uint8)-DEG22, self->roty,
                                 SHORTPLASMA_SPEED, SHORTPLASMA_LIFE,
                                 SHORTPLASMA_AP,
                                 (uint8)(ACF_COLLTYPE1 | ACF_COLLTYPE4));
    if (!shot) {
        return;
    }

    shot->collflags |= (uint8)(ACF_COLLTYPE1 | ACF_COLLTYPE4);
    shot->snd2 = 1u;
}

static void houdai_strat(Alien *self) {
    Alien *target;
    Alien *player;

    if (!self) {
        return;
    }

    target = strat_find_near_colltype(self, COLLTYPE_ENEMY2,
                                      HOUDAI_TRACK_MAX_Z, 10000);
    if (target) {
        self->sword1 = (int16)((target - g_aliens) + 1u);
    }

    target = houdai_target(self);
    if (target && target->active) {
        if ((int16)abs(self->worldz - target->worldz) >= HOUDAI_TRACK_MIN_Z) {
            self->roty = Strat_AngleXZ(self, target);
        }
    }

    player = Obj_GetPlayer();
    if (!player || (int16)abs(self->worldz - player->worldz) < HOUDAI_FIRE_GATE_Z) {
        return;
    }
    if ((g_gameframe & 3u) != 0u) {
        return;
    }

    houdai_fire(self);
}

void Strat_HoudaiNS_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = NULL;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = HOUDAI_HP;
    self->AP = HOUDAI_AP;
    self->collflags |= COLLTYPE_ENEMY1;
}

void Strat_Houdai_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = houdai_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = HOUDAI_HP;
    self->AP = HOUDAI_AP;
    self->collflags |= COLLTYPE_ENEMY1;
}

static Alien *zaco34_target(const Alien *self) {
    Alien *target;

    if (!self) {
        return NULL;
    }

    if (self->ptr >= NUMBER_AL) {
        return NULL;
    }

    target = &g_aliens[self->ptr];
    if (!target || !target->active) {
        return NULL;
    }
    return target;
}

void Strat_Zaco3_Init(Alien *self) {
    Alien *target;

    if (!self) {
        return;
    }

    target = strat_find_near_shape(self, SH_HOUDAI_0, NULL, 10000, 10000);
    if (!target) {
        self->stratptr = NULL;
        return;
    }

    self->ptr = strat_obj_index_or_null(target);
    self->stratptr = zaco3_attack;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = zaco3die_init;
    self->HP = ZACO3_HP;
    self->AP = ZACO3_AP;
    self->rotz = DEG90;
    self->sbyte1 = 2u;
    self->sbyte2 = 140u;
    self->sbyte3 = 3u;
    self->collflags |= COLLTYPE_ENEMY1;
    self->snd2 = 1u;
}

static void zaco3_attack(Alien *self) {
    Alien *target;

    if (!self) {
        return;
    }

    target = zaco34_target(self);
    if (!target) {
        achase_angle(&self->rotx, (uint8)-DEG45, 3);
        strat_move3d(self, 40u, 2u);
        return;
    }

    strat_aim_3d(self, target, 4);
    if (((abs(self->worldx - target->worldx) +
          abs(self->worldy - target->worldy) +
          abs(self->worldz - target->worldz)) < 1300) &&
        ((g_gameframe & 7u) == 0u) &&
        self->sbyte1 > 0u) {
        self->sbyte1--;
        if (self->sbyte1 == 0u) {
            self->stratptr = zaco3_circle;
            self->sbyte1 = 30u;
            self->rotx = 0u;
            zaco3_circle(self);
            return;
        }
        strat_fire_relslowlaser(self, self->rotx, self->roty);
    }

    strat_move3d(self, 40u, 2u);
}

static void zaco3_circle(Alien *self) {
    Alien *target;

    if (!self) {
        return;
    }

    target = zaco34_target(self);
    if (target) {
        strat_aim_yaw(self, target, 4);
    }
    achase_angle(&self->rotx, 0u, 2);

    if (self->sbyte1 > 0u) {
        self->sbyte1--;
        if (self->sbyte1 == 0u) {
            self->stratptr = zaco3_flyaway;
            zaco3_flyaway(self);
            return;
        }
    }

    self->worldy = Strat_Chase(self->worldy, (self->sbyte3 == 3u) ? -60 : -200, 1);
    strat_move3d(self, 30u, 2u);
}

static void zaco3_flyaway(Alien *self) {
    Alien *target;
    Alien *player;
    uint8 target_yaw;

    if (!self) {
        return;
    }

    target = zaco34_target(self);
    player = Obj_GetPlayer();
    target_yaw = (uint8)-30;
    if (!target || (player && self->worldx > player->worldx)) {
        target_yaw = 30;
    }

    achase_angle(&self->roty, target_yaw, 4);
    achase_angle(&self->rotx, (uint8)-30, 2);
    strat_move3d(self, 20u, 2u);
    add_player_z(self);

    if (self->sbyte2 > 0u) {
        self->sbyte2--;
        if (self->sbyte2 == 0u) {
            Strat_RemoveObj();
        }
    }
}

static void zaco3go_init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = zaco3go_strat;
    self->collstratptr = Strat_Explode;
    self->expstratptr = Strat_Explode;
    self->HP = HARD_HP;
}

static void zaco3die_init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = zaco3die_strat;
    self->collstratptr = NULL;
    self->expstratptr = zaco3die_strat;
    Sound_PlaySE(0x10u);
}

static void zaco3die_strat(Alien *self) {
    if (!self) {
        return;
    }

    (void)Strat_SpeedTo(self, 40u, 1u);
    if (self->worldy < -100) {
        zaco3go_init(self);
        zaco3go_strat(self);
        return;
    }
    if (self->rotx <= DEG45) {
        self->rotx = (uint8)(self->rotx + 4u);
    }

    Strat_GenVecs3D(self);
    self->rotz = (uint8)(self->rotz + 4u);
    add_player_z(self);
    add_player_z(self);
    Strat_ApplyVelocity(self);
}

static void zaco3go_strat(Alien *self) {
    Alien *player;
    int16 zdist;

    if (!self) {
        return;
    }

    (void)Strat_SpeedTo(self, 60u, 1u);
    self->rotz = (uint8)(self->rotz + 4u);

    player = Obj_GetPlayer();
    if (player && player->active) {
        zdist = (int16)abs(self->worldz - player->worldz);
        if (zdist >= 3000) {
            self->rotx = 0u;
            self->roty = DEG180;
        } else if (zdist >= 400) {
            strat_aim_3d(self, player, 2);
        }
    }

    Strat_GenVecs3D(self);
    add_player_z(self);
    Strat_ApplyVelocity(self);
}

static void zaco4_attack(Alien *self);
static void zaco4_circle(Alien *self);
static void zaco4_flyaway(Alien *self);

void Strat_Zaco4_Init(Alien *self) {
    Alien *target;

    if (!self) {
        return;
    }

    target = strat_find_near_shape(self, SH_PILLAR3, NULL, 10000, 10000);
    if (!target) {
        self->stratptr = NULL;
        return;
    }

    self->ptr = strat_obj_index_or_null(target);
    self->stratptr = zaco4_attack;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = ZACO4_HP;
    self->AP = ZACO4_AP;
    self->rotz = DEG90;
    self->sbyte1 = 2;
    self->sbyte2 = 140;
    self->sbyte3 = 4;
    self->collflags |= COLLTYPE_ENEMY1;
    self->snd2 = 1;
}

static void zaco4_attack(Alien *self) {
    Alien *target;

    if (!self) {
        return;
    }

    target = zaco34_target(self);
    if (!target) {
        achase_angle(&self->rotx, (uint8)-DEG45, 3);
        strat_move3d(self, 40, 2);
        return;
    }

    strat_aim_3d(self, target, 4);
    if (((abs(self->worldx - target->worldx) +
          abs(self->worldy - target->worldy) +
          abs(self->worldz - target->worldz)) < 1300) &&
        ((g_gameframe & 7u) == 0u)) {
        if (self->sbyte1 > 0u) {
            self->sbyte1--;
            if (self->sbyte1 == 0u) {
                self->sbyte1 = 30;
                self->rotx = 0;
                self->stratptr = zaco4_circle;
            } else {
                strat_fire_relslowlaser(self, self->rotx, self->roty);
            }
        }
    }

    strat_move3d(self, 40, 2);
}

static void zaco4_circle(Alien *self) {
    Alien *target;

    if (!self) {
        return;
    }

    target = zaco34_target(self);
    if (target) {
        strat_aim_yaw(self, target, 4);
    }
    achase_angle(&self->rotx, 0, 2);
    self->worldy = Strat_Chase(self->worldy, -200, 1);
    if (self->sbyte1 > 0u) {
        self->sbyte1--;
        if (self->sbyte1 == 0u) {
            self->stratptr = zaco4_flyaway;
        }
    }
    strat_move3d(self, 30, 2);
}

static void zaco4_flyaway(Alien *self) {
    Alien *target;
    Alien *player;
    uint8 target_yaw;

    if (!self) {
        return;
    }

    target = zaco34_target(self);
    player = Obj_GetPlayer();
    target_yaw = (uint8)-30;
    if (!target || (player && self->worldx > player->worldx)) {
        target_yaw = 30;
    }

    achase_angle(&self->roty, target_yaw, 4);
    achase_angle(&self->rotx, (uint8)-30, 2);
    strat_move3d(self, 20, 2);
    add_player_z(self);

    if (self->sbyte2 > 0u) {
        self->sbyte2--;
        if (self->sbyte2 == 0u) {
            Strat_RemoveObj();
        }
    }
}

static void zaco0_sweep(Alien *self);
static void zaco0_turn_in(Alien *self);
static void zaco0_fire(Alien *self);
static void zaco0_turn_out(Alien *self);
static void zaco0_flyaway(Alien *self);
static void para_strat(Alien *self);
static void para2_strat(Alien *self);
static void parajump_strat(Alien *self);
static void carrier_strat(Alien *self);
static void carrierb_strat(Alien *self);
static void carrierc_strat(Alien *self);
static void carrier_spawn_para(Alien *self);
static void base1_strat(Alien *self);

void Strat_Zaco0_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = zaco0_sweep;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = ZACO0_HP;
    self->AP = ZACO0_AP;
    self->roty = DEG270;
    self->rotx = DEG90;
    self->sbyte1 = 10u;
    self->snd2 = 3u;
    self->collflags |= COLLTYPE_ENEMY1;
}

static void zaco0_sweep(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (player && player->active && self->worldy < player->worldy) {
        self->worldy = (int16)(self->worldy + 20);
        if (self->worldy > -30) {
            self->worldy = -30;
        }
    }

    self->worldx = (int16)(self->worldx + 43);
    if (player && player->active && self->worldx >= player->worldx) {
        self->stratptr = zaco0_turn_in;
        zaco0_turn_in(self);
    }
}

static void zaco0_turn_in(Alien *self) {
    if (!self) {
        return;
    }

    self->rotx = (uint8)(self->rotx - 8u);
    self->roty = (uint8)(self->roty - 8u);
    if (self->roty == DEG180) {
        self->stratptr = zaco0_fire;
        zaco0_fire(self);
    }
}

static void zaco0_fire(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (((g_gameframe & 1u) == 0u) && player && player->active) {
        uint8 fire_yaw = (uint8)(Strat_AngleXZ(self, player) + strat_random_centered(3u));
        uint8 fire_pitch = (uint8)(strat_pitch_toward(self, player) + strat_random_centered(3u));
        strat_fire_relslowlaser(self, fire_pitch, fire_yaw);
    }

    if (self->sbyte1 > 0u) {
        self->sbyte1--;
        if (self->sbyte1 == 0u) {
            self->stratptr = zaco0_turn_out;
            zaco0_turn_out(self);
            return;
        }
    }

    if (player && player->active &&
        (int16)abs(self->worldz - player->worldz) < 300) {
        self->stratptr = zaco0_turn_out;
        zaco0_turn_out(self);
    }
}

static void zaco0_turn_out(Alien *self) {
    if (!self) {
        return;
    }

    self->rotx = (uint8)(self->rotx + 8u);
    self->roty = (uint8)(self->roty + 8u);
    if (self->roty == DEG270) {
        self->stratptr = zaco0_flyaway;
        zaco0_flyaway(self);
    }
}

static void zaco0_flyaway(Alien *self) {
    if (!self) {
        return;
    }

    self->worldy = (int16)(self->worldy - 19);
    self->worldx = (int16)(self->worldx + 40);
    strat_move3d(self, 50u, 2u);
}

void Strat_Para_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = para_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = PARA_HP;
    self->AP = PARA_AP;
    self->sbyte1 = (uint8)(-(int8)PARA_SWINGMAX);
    self->sflags |= ASF_SHADOW;
}

static void para_strat(Alien *self) {
    int8 swing;

    if (!self) {
        return;
    }

    add_player_z(self);
    self->worldy = (int16)(self->worldy + 10);

    swing = (int8)self->sbyte1;
    if (self->rotz < 128u) {
        if (swing != -(int8)PARA_SWINGMAX) {
            swing = (int8)(swing - PARA_SWINGSPD);
        }
    } else if (swing != (int8)PARA_SWINGMAX) {
        swing = (int8)(swing + PARA_SWINGSPD);
    }

    self->sbyte1 = (uint8)swing;
    self->rotz = (uint8)(self->rotz + self->sbyte1);

    if (self->worldy >= 0) {
        self->stratptr = para2_strat;
        self->worldy = 0;
        self->rotz = 0u;
        self->shape = g_shapes_table[SH_PARA_1_PROXY];
        self->vel = 10u;
        para2_strat(self);
    }
}

static void para2_strat(Alien *self) {
    Alien *player;
    bool aligned = true;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (player && player->active) {
        aligned = achase_angle(&self->roty, Strat_AngleXZ(self, player), 1);
        aligned = achase_angle(&self->rotx, strat_pitch_toward(self, player), 1) && aligned;
    }

    if (!aligned) {
        Strat_GenVecs2D(self);
    }

    self->worldx = (int16)(self->worldx + self->vx);
    self->worldz = (int16)(self->worldz + self->vz);

    if (player && player->active && Strat_DistXZ(self, player) < 400) {
        self->stratptr = parajump_strat;
        parajump_strat(self);
        return;
    }

    if ((g_gameframe & 3u) == 0u) {
        self->vy = -15;
    }

    self->worldx = (int16)(self->worldx + self->vx);
    self->worldy = (int16)(self->worldy + self->vy);
    self->worldz = (int16)(self->worldz + self->vz);
    self->vy = (int16)(self->vy + 1);

    if (self->worldy >= 0) {
        self->worldy = 0;
        self->vy = (int16)(-(self->vy / 2));
        if (self->vy > -5) {
            self->vy = 0;
        }
    }
}

static void parajump_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    self->worldy = Strat_Chase(self->worldy, g_player_posy, 2);
    player = Obj_GetPlayer();
    if (player && player->active &&
        (int16)abs(self->worldz - player->worldz) <= 200) {
        self->worldx = Strat_Chase(self->worldx, player->worldx, 3);
    }
}

static void carrier_spawn_para(Alien *self) {
    Alien *child;

    if (!self) {
        return;
    }

    child = Strat_MakeObj(g_shapes_table[SH_PARA_0]);
    if (!child) {
        return;
    }

    child->worldx = self->worldx;
    child->worldy = (int16)(self->worldy + 90);
    child->worldz = self->worldz;
    child->immuneptr = strat_obj_index_or_null(self);
    self->immuneptr = strat_obj_index_or_null(child);
    Strat_Para_Init(child);
}

void Strat_Carrier_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = carrier_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = CARRIER_HP;
    self->AP = CARRIER_AP;
    self->snd2 = 14u;
}

static void carrier_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    self->roty = (uint8)(self->roty + 3u);
    self->worldy = (int16)(self->worldy + 2);
    self->worldz = (int16)(self->worldz + 30);
    if (player && player->active &&
        (int16)abs(self->worldz - player->worldz) > 3000) {
        self->stratptr = carrierb_strat;
        self->sbyte1 = 32u;
        self->sbyte2 = 1u;
        carrierb_strat(self);
        return;
    }

    add_player_z(self);
}

static void carrierb_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    if (self->sbyte2 > 0u) {
        self->sbyte2--;
    }
    if (self->sbyte2 == 0u) {
        self->sbyte2 = CARRIER_RATE;
        carrier_spawn_para(self);
    }

    player = Obj_GetPlayer();
    if (player && player->active) {
        self->worldx = Strat_ChaseProportional(self->worldx, player->worldx, 3);
    }
    (void)achase_angle(&self->rotx, 0u, 4);
    self->worldy = Strat_ChaseProportional(self->worldy, -320, 5);
    self->roty = (uint8)(self->roty + 4u);
    strat_move3d(self, 30u, 1u);
    add_player_z(self);
    self->worldz = (int16)(self->worldz - 15);

    if (self->sbyte1 > 0u) {
        self->sbyte1--;
        if (self->sbyte1 == 0u) {
            self->stratptr = carrierc_strat;
            carrierc_strat(self);
        }
    }
}

static void carrierc_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->worldz = (int16)(self->worldz - 10);
    self->worldy = (int16)(self->worldy - 3);
    self->roty = (uint8)(self->roty + 4u);
    add_player_z(self);
}

void Strat_Base1_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = base1_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->sflags |= ASF_NOHITAFFECT;
    self->HP = HARD_HP;
    self->AP = HARD_AP;
    self->animframe = 0u;
}

static void base1_strat(Alien *self) {
    if (!self) {
        return;
    }

    if ((self->sflags2 & BASE1_PHASE_FLAG) == 0u) {
        if (self->animframe < 8u) {
            if (self->animframe == 0u) {
                Sound_PlaySE(0x59u);
            }
            self->animframe++;
        }
        if (self->animframe < 8u) {
            return;
        }
        self->sbyte1++;
        if (self->sbyte1 < BASE1_WAIT_FRAMES) {
            return;
        }
        self->sbyte1 = 0u;
        self->sflags2 |= BASE1_PHASE_FLAG;
        return;
    }

    if (self->animframe > 0u) {
        if (self->animframe == 8u) {
            Sound_PlaySE(0x5Au);
        }
        self->animframe--;
        return;
    }

    self->sbyte1++;
    if (self->sbyte1 < BASE1_WAIT_FRAMES) {
        return;
    }

    self->sbyte1 = 0u;
    self->sflags2 &= (uint8)~BASE1_PHASE_FLAG;
    base1_strat(self);
}

static void cameleon_phase2(Alien *self);

static void cameleon_phase1(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    if (self->rotx != DEG180) {
        self->rotx = (uint8)(self->rotx + 16u);
        add_player_z(self);
        return;
    }

    if (self->rotz != DEG90) {
        self->rotz = (uint8)(self->rotz + 4u);
        add_player_z(self);
        return;
    }

    if (self->sbyte1 > 0u) {
        self->sbyte1--;
    }
    if (self->sbyte1 == 0u) {
        self->stratptr = cameleon_phase2;
        cameleon_phase2(self);
        return;
    }

    if ((g_gameframe & 3u) == 0u) {
        player = Obj_GetPlayer();
        if (player && player->active) {
            strat_fire_relslowlaser(self,
                                    strat_pitch_toward(self, player),
                                    Strat_AngleXZ(self, player));
        }
    }

    add_player_z(self);
}

static void cameleon_phase2(Alien *self) {
    if (!self) {
        return;
    }

    if (self->roty != DEG180) {
        self->roty = (uint8)(self->roty + 16u);
        add_player_z(self);
        return;
    }

    Strat_RemoveObj();
}

void Strat_Cameleon_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = cameleon_phase1;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->HP = CAMELEON_HP;
    self->AP = CAMELEON_AP;
    self->sbyte1 = 20u;
    self->collflags |= COLLTYPE_ENEMY1;
    Sound_PlaySE(0x2Bu);
}

// ============================================================
// HIT FLASH COLLISION CALLBACK (GSTRATS.ASM)
// Called when an enemy is hit by player weapons.
// ============================================================

void Strat_HitFlash(Alien *self) {
    // hitflash_Istrat (GSTRATS.ASM:896-940)
    // Applies damage, sets hit flash visual, checks if dead.
    // Simplified: decrement HP by 1, set flash, check death.

    // Skip if nohitaffect flag is set
    // s_clr_alsflag x,collide — clear collide state

    // Apply damage (s_docoll x,#framesperAP)
    if (self->HP != HARD_HP) {
        // Destructible enemy: decrement HP
        if (self->HP > 0) {
            self->HP--;
        }
        if (self->HP == 0) {
            if (self->expstratptr) {
                self->expstratptr(self);
            } else {
                Strat_Explode(self);
            }
            return;
        }
    }

    // Set hit flash effect (white flash for one frame)
    self->sflags |= ASF_HITFLASH;

    // Restore the per-frame strategy (collision callback returns to normal)
    // In the ASM, the stratptr is already pointing at the main loop.
}

// ============================================================
// EXPLOSION DEATH CALLBACK (EXPSTRAT.ASM)
// Called when an enemy's HP reaches zero.
// ============================================================

void Strat_Explode(Alien *self) {
    // explode_Istrat (EXPSTRAT.ASM:677+)
    // Special-map objects gate boss-clear flow via GF_BOSSDEAD.
    if (self->sflags4 & (ASF4_SPECIAL | ASF4_CSPECIAL)) {
        if (g_specialobjtotal > 0) {
            g_specialobjtotal--;
        }
        if (self->sflags4 & ASF4_CSPECIAL) {
            if (g_specials_dead < 0xFF) g_specials_dead++;
        }
        if (g_specialobjtotal == 0) {
            g_gameflags |= GF_BOSSDEAD;
        }
    }

    Sound_PlaySE(0x10);

    // Mark for removal by the strategy loop
    g_aldead = 1;
}

// ============================================================
// SZACO2 ENEMY FIGHTER (GA2STRAT.ASM)
// Sector X opening attacker that climbs to a waypoint, then levels out
// and fires straight relslow lasers while crossing the screen.
// ============================================================

static uint8 szaco2_waypoint_yaw(const Alien *self) {
    float dx;
    float dz;
    float angle;

    if (!self) {
        return 0u;
    }

    dx = (float)(self->sWPx1 - self->worldx);
    dz = (float)(self->sWPz1 - self->worldz);
    angle = atan2f(dx, dz);
    if (angle < 0.0f) {
        angle += 2.0f * 3.14159265f;
    }

    // `s_obj2WP_angle` negates the absolute XY angle before chasing yaw.
    return (uint8)(-(int)(angle * (256.0f / (2.0f * 3.14159265f))));
}

static uint8 szaco2_waypoint_pitch(const Alien *self) {
    float dx;
    float dy;
    float dz;
    float dist;
    float pitch;

    if (!self) {
        return 0u;
    }

    dx = (float)(self->sWPx1 - self->worldx);
    dy = (float)(self->sWPy1 - self->worldy);
    dz = (float)(self->sWPz1 - self->worldz);
    dist = sqrtf(dx * dx + dz * dz);
    if (dist <= 1.0f) {
        return self->rotx;
    }

    pitch = atan2f(dy, dist);
    return (uint8)(int)(pitch * (256.0f / (2.0f * 3.14159265f)));
}

static void szaco2_bank_to_player(Alien *self) {
    int16 dx;
    int16 dy;

    if (!self) {
        return;
    }

    // sr_banktoplayer (STRATROU.ASM:2821-2860)
    dx = (int16)(self->worldx - g_player_posx);
    self->rotz = (uint8)(self->rotz + (int8)(dx >> 6));

    // `al1pt` is not modeled as a named runtime field here; use the same
    // per-alien phase staggering already used by other literal ports.
    if ((((uint8)g_gameframe) + strat_phase_offset(self)) & 0x03u) {
        return;
    }

    if (dx >= 0) {
        self->roty++;
    } else {
        self->roty--;
    }

    dy = (int16)(self->worldy - g_player_posy);
    if (dy >= 0) {
        self->rotx++;
    } else {
        self->rotx--;
    }
}

static void szaco2_cont(Alien *self) {
    if (!self) {
        return;
    }

    // szaco2_cont (GA2STRAT.ASM:287-291)
    Strat_GenVecs3D(self);
    Strat_ApplyVelocity(self);
    add_player_z(self);
}

static void szaco2_strat(Alien *self) {
    Alien *player;
    int16 zdist;
    bool yaw_done;
    bool pitch_done;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();

    // szaco2_strat (GA2STRAT.ASM:241-286)
    switch (self->stratstate) {
    case 0:
        self->sWPz1 = (int16)(self->worldz - 10);
        achase_angle(&self->roty, szaco2_waypoint_yaw(self), SZACO2_TURN_SHIFT);
        achase_angle(&self->rotx, szaco2_waypoint_pitch(self), SZACO2_TURN_SHIFT);
        if (self->worldy < self->sWPy1) {
            self->stratstate = 1u;
        }
        break;
    case 1:
        yaw_done = achase_angle(&self->roty, DEG180, SZACO2_FIN_SHIFT);
        pitch_done = achase_angle(&self->rotx, 0u, SZACO2_FIN_SHIFT);
        if (yaw_done && pitch_done) {
            self->stratstate = 2u;
        }
        break;
    case 2:
        if (player && player->active) {
            zdist = (int16)abs(self->worldz - player->worldz);
            if (zdist < SZACO2_BANK_Z) {
                szaco2_bank_to_player(self);
                if (zdist < SZACO2_DASH_Z) {
                    self->stratstate = 3u;
                }
            }
        }
        break;
    case 3:
        (void)achase_angle(&self->roty, DEG180, SZACO2_FIN_SHIFT);
        break;
    default:
        break;
    }

    if (player && player->active) {
        zdist = (int16)abs(self->worldz - player->worldz);
        if (zdist >= SZACO2_FIRE_NEAR_Z && zdist < SZACO2_FIRE_FAR_Z) {
            if ((((uint8)g_gameframe) + strat_phase_offset(self)) & SZACO2_FIRE_MASK) {
                szaco2_cont(self);
                return;
            }

            strat_fire_relslowlaser(self, self->rotx, self->roty);
        }
    }

    szaco2_cont(self);
}

void Strat_Szaco2_Init(Alien *self) {
    if (!self) {
        return;
    }

    // szaco2_Istrat (GA2STRAT.ASM:229-240)
    self->HP = SZACO2_HP;
    self->AP = SZACO2_AP;
    self->stratptr = szaco2_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;
    self->vel = SZACO2_SPEED;
    self->sWPy1 = (int16)(self->sWPy1 + SZACO2_WPY_OFFSET + g_player_posy);
    self->animframe = (uint8)(0x80u | SZACO2_ANIM_INIT);
    self->collflags |= COLLTYPE_ENEMYWEAP;
    self->snd2 = 3u;

    // `relexplode` and `zaco_8p` debris are not modeled in the active runtime's
    // flat enemy path yet; keep the live movement/fire behavior exact and leave
    // those death-visual side effects for the explosion runtime work.
}

// ============================================================
// ZACO1 ENEMY FIGHTER (GASTRATS.ASM)
// "Butterfly type, fly to distance and fly twisting to player."
//
// Three-phase state machine:
// Phase 0 (zaco1_strat): Fly toward waypoint, monitor Z distance
// Phase 1 (zaco1a_strat): Approach — chase rotation to 0 degrees
// Phase 2 (zaco1b_strat): Chase/spiral — fire homing lasers, spiral attack
//
// Alien struct fields used:
//   sword1: Target X offset from player (±1000)
//   sbyte2: Roll rotation delta (±6)
//   sWPx1:  Waypoint X position
//   sWPz1:  Waypoint Z position
//   sword2: Temporary (sine offset for spiral)
//   ptr:    Temporary (cosine offset for spiral)
//   sbyte1: Temporary (rotz-90 for spiral lookup)
// ============================================================

// Forward declarations for phase strategies
static void zaco1_phase0(Alien *self);
static void zaco1_phase1(Alien *self);
static void zaco1_phase2(Alien *self);

// Shared continuation: generate 3D vectors, apply velocity, add player Z
static void zaco1_cont(Alien *self) {
    // zaco1_cont (GASTRATS.ASM:1217-1226)
    // s_gen_3dvecs x,al_roty,al_rotx,al_vel
    Strat_GenVecs3D(self);

    // s_jmp_higher x,#0,.hok — clamp Y to >= 0 (don't go underground)
    if (self->worldy < 0) {
        self->worldy = 0;
    }

    // s_add_alvars W,x,al_worldy,x,al_sword2 — add sine offset to Y
    self->worldy += self->sword2;
    // s_add_alvars W,x,al_worldx,x,al_ptr — add cosine offset to X
    self->worldx += self->ptr;

    // s_add_vecs2pos x — apply vx/vy/vz to worldx/y/z
    Strat_ApplyVelocity(self);

    // s_add_playerZ x — keep up with world scrolling
    add_player_z(self);
}

// --- Common init for both L and R variants ---
static void zaco1_common_init(Alien *self) {
    // zaco1_Icont (GASTRATS.ASM:1192-1206)
    self->sflags |= ASF_SHADOW;

    // s_add_alvar W,x,al_worldx,player_posx — offset world X by player X
    self->worldx += g_player_posx;
    // s_add_alvar W,x,al_sword1,player_posx — offset target X by player X
    self->sword1 += g_player_posx;

    // s_set_alptrs: main strat, hit callback, explosion callback
    self->stratptr = zaco1_phase0;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;

    // s_set_aldata x,#zaco1HP,#zaco1AP
    self->HP = ZACO1_HP;
    self->AP = ZACO1_AP;

    // s_set_speed x,#60
    self->vel = 60;

    // s_setnoremove_behind x — don't remove when behind camera
    self->type &= ~ATZREMOVE;

    // s_set_colltype x,enemy2 + enemyweap + Zenemy
    self->collflags |= (COLLTYPE_ENEMY2 | COLLTYPE_ENEMYWEAP | COLLTYPE_ZENEMY);

    // s_copy_alvar2var W,x,svar_word1,al_sword1
    // s_set_alvar W,x,al_sWPx1,svar_word1
    self->sWPx1 = self->sword1;

    // set_sound2 x,#3
    self->snd2 = 3;
}

void Strat_Zaco1L_Init(Alien *self) {
    // zaco1L_Istrat (GASTRATS.ASM:1182-1186)
    self->sword1 = 1000;    // Target X: right of player
    self->sbyte2 = 6;       // Roll delta: clockwise
    zaco1_common_init(self);
}

void Strat_Zaco1R_Init(Alien *self) {
    // zaco1R_Istrat (GASTRATS.ASM:1187-1190)
    self->sword1 = -1000;   // Target X: left of player
    self->sbyte2 = (uint8)-6;  // Roll delta: counter-clockwise
    zaco1_common_init(self);
}

// Phase 0: Fly toward waypoint, check distance to player
static void zaco1_phase0(Alien *self) {
    // zaco1_strat (GASTRATS.ASM:1208-1226)

    // Set waypoint Z = player Z + 1500
    self->sWPz1 = g_player_posz + 1500;

    // s_obj2WP_angle: chase yaw/pitch toward waypoint
    // Calculates angle from self to (sWPx1, sWPz1) and chases al_roty
    // Then calculates pitch angle and chases al_rotx
    {
        float dx = (float)(self->sWPx1 - self->worldx);
        float dz = (float)(self->sWPz1 - self->worldz);
        float angle_rad = atan2f(dx, dz);
        if (angle_rad < 0) angle_rad += 2.0f * 3.14159265f;
        uint8 target_yaw = (uint8)(-(int)(angle_rad * (256.0f / (2.0f * 3.14159265f))));
        achase_angle(&self->roty, target_yaw, 3);

        // Pitch angle toward waypoint Y (0 since WP has no explicit Y)
        float dy = (float)(0 - self->worldy);
        float dist_xz = sqrtf(dx * dx + dz * dz);
        if (dist_xz > 1.0f) {
            float pitch_rad = atan2f(dy, dist_xz);
            uint8 target_pitch = (uint8)(int)(pitch_rad * (256.0f / (2.0f * 3.14159265f)));
            achase_angle(&self->rotx, target_pitch, 3);
        }
    }

    // s_jmp_Zdistmore x,y,#1000,zaco1a_init
    // Check Z distance to player; if > 1000, transition to approach phase
    Alien *player = Obj_GetPlayer();
    if (player) {
        int16 zdist = abs(self->worldz - player->worldz);
        if (zdist > 1000) {
            // Transition to phase 1 (approach)
            self->stratptr = zaco1_phase1;
            self->type |= ATZREMOVE;  // s_setremove_behind
        }
    }

    zaco1_cont(self);
}

// Phase 1: Approach — chase rotation toward 0 degrees
static void zaco1_phase1(Alien *self) {
    // zaco1a_strat (GASTRATS.ASM:1230-1234)

    // s_Achase_alvar B,x,al_roty,#deg0,3,zaco1b_init
    // Chase yaw toward 0 with shift=3. If reached, go to phase 2.
    bool reached = achase_angle(&self->roty, DEG0, 3);
    if (reached) {
        // Transition to phase 2 (chase/spiral)
        self->stratptr = zaco1_phase2;
    }

    // s_add_alvar B,x,al_rotx,#-1 — pitch up slightly
    self->rotx -= 1;

    zaco1_cont(self);
}

// Phase 2: Chase and spiral attack
static void zaco1_phase2(Alien *self) {
    // zaco1b_strat (GASTRATS.ASM:1238-1276)

    Alien *player = Obj_GetPlayer();
    int16 zdist = 0;
    if (player) {
        zdist = abs(self->worldz - player->worldz);
    }

    if (zdist < 1400) {
        // .circ — Spiral attack pattern (GASTRATS.ASM:1253-1259)

        // s_speedto x,#45,1 — decelerate to speed 45
        Strat_SpeedTo(self, 45, 1);

        // s_copy_alvar2alvar B,x,al_sbyte1,x,al_rotz
        self->sbyte1 = self->rotz;
        // s_sub_alvar B,x,al_sbyte1,#deg90
        self->sbyte1 -= DEG90;

        // s_set_alvar2alvartab: sword2 = sintab[sbyte1] >> 3
        // Sine lookup for vertical oscillation
        self->sword2 = (int16)(strat_sin(self->sbyte1) * 127.0f) >> 3;

        // s_set_alvar2alvartab: ptr = costab[sbyte1] >> 2
        // Cosine lookup for horizontal oscillation
        self->ptr = (int16)(strat_cos(self->sbyte1) * 127.0f) >> 2;

        // s_add_alvars B,x,al_rotz,x,al_sbyte2
        // Advance roll rotation by ±6 each frame
        self->rotz += self->sbyte2;

    } else if (zdist >= 1400 && zdist <= 1800) {
        // Between 1400-1800: fire homing laser every 2 frames
        // s_jmp_notdelay 2,.nocirc — skip if odd frame
        if ((g_gameframe & 1) == 0) {
            // s_weapon_pos #0,#0,#0
            // s_weapon_rots2obj y — aim at player
            // s_fire_weapon x,RELSLOWELASERHOME
            if (player) {
                uint8 fire_yaw = Strat_AngleXZ(self, player);
                (void)Strat_SpawnProjectile(self,
                                            0, 0, 0,
                                            self->rotx, fire_yaw,
                                            52, 55, 2,
                                            ACF_COLLTYPE4);
            }
        }
        // Clear spiral offsets when not spiraling
        self->sword2 = 0;
        self->ptr = 0;
    } else {
        // > 1800: no spiral, no firing
        self->sword2 = 0;
        self->ptr = 0;
    }

    // s_jmp_Zdistless x,y,#700,.nfpl
    if (zdist >= 700 && player) {
        // s_obj2obj_3dangle x,y,al_roty,al_rotx,3 — face player
        uint8 target_yaw = Strat_AngleXZ(self, player);
        achase_angle(&self->roty, target_yaw, 3);

        // Chase pitch toward player too
        float dy = (float)(player->worldy - self->worldy);
        float dx = (float)(player->worldx - self->worldx);
        float dz = (float)(player->worldz - self->worldz);
        float dist_xz = sqrtf(dx * dx + dz * dz);
        if (dist_xz > 1.0f) {
            float pitch_rad = atan2f(dy, dist_xz);
            uint8 target_pitch = (uint8)(int)(pitch_rad * (256.0f / (2.0f * 3.14159265f)));
            achase_angle(&self->rotx, target_pitch, 3);
        }
    }
    // else: .nfpl — too close, don't adjust rotation (just continue)

    zaco1_cont(self);
}

// ============================================================
// FRIEND EXIT BASE (GISTRATS.ASM)
// Wingman ship exit sequence — flies away from the base at
// constant speed, becomes visible after first frame, despawns
// after a set number of frames.
// ============================================================

static void friendexitbase_strat(Alien *self) {
    // friendexitbase_strat (GISTRATS.ASM:321-337)

    // s_decbne_alvar B,x,al_sbyte1,.nowt — decrement frame counter
    if (self->sbyte1 > 0) {
        self->sbyte1--;
        if (self->sbyte1 > 0) return;  // .nowt — skip everything
    }

    // Reset counter to 1 (fires every other frame)
    self->sbyte1 = 1;

    // s_beqdec_alvar B,x,al_sbyte2,.left — decrement sound timer
    if (self->sbyte2 > 0) {
        self->sbyte2--;
        // Right-channel sound
        self->snd1 = 0xB1;  // %10110001
    } else {
        // Left-channel sound
        self->snd1 = 0x51;  // %01010001
    }

    // s_add_alvar W,x,al_worldz,#pexitbasespeed — move forward
    self->worldz += PEXITBASE_SPEED;

    // s_clr_alsflag x,invisible — become visible after first frame
    self->sflags &= ~ASF_INVISIBLE;

    // s_dec_lifecnt x — decrement lifespan, die when exhausted
    if (self->count > 0) {
        self->count--;
    } else {
        g_aldead = 1;  // Remove self
    }
}

void Strat_FriendExitBase_Init(Alien *self) {
    // friendexitbase_Istrat (GISTRATS.ASM:314-320)

    // s_set_alsflag x,colldisable — no collision during exit
    self->sflags |= ASF_COLLDISABLE;

    // s_set_lifecnt x,#1500/pexitbasespeed = 30 frames
    self->count = 1500 / PEXITBASE_SPEED;

    // s_set_strat x,friendexitbase_strat
    self->stratptr = friendexitbase_strat;

    // s_set_alsflag x,shadow
    self->sflags |= ASF_SHADOW;

    // s_set_alsflag x,invisible — start invisible
    self->sflags |= ASF_INVISIBLE;

    // s_set_alvar B,x,al_sbyte2,#11 — time until left sound
    self->sbyte2 = 11;
}

// ============================================================
// CLEAR-DEMO SHIPS (GCSTRATS.ASM)
// Active runtime subset used by CL_GND and CL_WARP.
// ============================================================

static bool frame_tick_mod(uint16 step) {
    return step != 0u && (g_gameframe % step) == 0u;
}

static void clshipboost_step(Alien *self) {
    if (self->sbyte2 != 0u) {
        self->sbyte2--;
        if (self->sbyte2 == 1u) {
            g_aldead = 1u;
            return;
        }
    }

    Strat_GenVecs3D(self);
    Strat_ApplyVelocity(self);
    self->worldz = (int16)(self->worldz + g_psvar_word2);
}

static void clshipboost_enter(Alien *self, bool play_sound) {
    if (play_sound) {
        self->snd2 = 0x32u;
    }
    self->stratptr = clshipboost_step;
    self->vel = 120u;
}

static void clship_flyinleft(Alien *self) {
    if ((self->sflags2 & CLSHIP_FLAG1) == 0u) {
        if (self->worldx >= -30) {
            self->rotz = (uint8)(self->rotz + 2u);
            if (self->vx != -5 && frame_tick_mod(1u)) {
                self->vx--;
            } else if (self->vx == -5) {
                self->sflags2 |= CLSHIP_FLAG1;
            }
        }
    } else if (self->vx != 0 && frame_tick_mod(3u)) {
        self->vx++;
    }

    self->worldx = (int16)(self->worldx + self->vx);
}

static void clship_flyinright(Alien *self) {
    if ((self->sflags2 & CLSHIP_FLAG1) == 0u) {
        if (self->worldx <= 30) {
            self->rotz = (uint8)(self->rotz - 2u);
            if (self->vx != 5 && frame_tick_mod(1u)) {
                self->vx++;
            } else if (self->vx == 5) {
                self->sflags2 |= CLSHIP_FLAG1;
            }
        }
    } else if (self->vx != 0 && frame_tick_mod(3u)) {
        self->vx--;
    }

    self->worldx = (int16)(self->worldx + self->vx);
}

static void clship_warp_cont(Alien *self, int16 zoff, int16 yoff) {
    Alien *player = Obj_GetPlayer();

    if (self->sword1 > 0) {
        self->sword1--;
        if (self->sword1 == 0) {
            clshipboost_enter(self, false);
            clshipboost_step(self);
            return;
        }
    }

    if (player) {
        self->worldz = Strat_Chase(self->worldz, (int16)(player->worldz + zoff), 4);
        self->worldy = Strat_Chase(self->worldy, (int16)(player->worldy + yoff), 4);
        (void)achase_angle(&self->rotx, player->rotx, 5);
    }

    (void)achase_angle(&self->rotz, 0u, 5);
    add_player_z(self);
    self->worldz = (int16)(self->worldz + g_psvar_word2);
}

static void clship_gnd_cont(Alien *self, int16 zoff, int16 yoff) {
    Alien *player = Obj_GetPlayer();

    if (self->sword1 > 0) {
        self->sword1--;
        if (self->sword1 == 0) {
            clshipboost_enter(self, true);
            clshipboost_step(self);
            return;
        }
    }

    if (player) {
        self->worldz = Strat_Chase(self->worldz, (int16)(player->worldz + zoff), 3);
        self->worldy = Strat_Chase(self->worldy, (int16)(player->worldy + yoff), 2);
        if (frame_tick_mod(1u)) {
            (void)achase_angle(&self->rotz, player->rotz, 5);
        }
        if (frame_tick_mod(2u)) {
            (void)achase_angle(&self->rotx, player->rotx, 5);
        }
    }

    add_player_z(self);
}

static void clshipWARPa_strat(Alien *self) {
    clship_flyinleft(self);
    clship_warp_cont(self, 100, -20);
}

static void clshipWARPb_strat(Alien *self) {
    clship_flyinright(self);
    clship_warp_cont(self, 200, -20);
}

static void clshipWARPc_strat(Alien *self) {
    self->worldx = Strat_Chase(self->worldx, 0, 4);
    clship_warp_cont(self, 300, -30);
}

static void clshipGNDa_strat(Alien *self) {
    self->worldx = Strat_Chase(self->worldx, -50, 4);
    clship_gnd_cont(self, -200, 20);
}

static void clshipGNDb_strat(Alien *self) {
    self->worldx = Strat_Chase(self->worldx, 50, 4);
    clship_gnd_cont(self, -100, 40);
}

static void clshipGNDc_strat(Alien *self) {
    self->worldx = Strat_Chase(self->worldx, 0, 4);
    clship_gnd_cont(self, -300, 50);
}

static void clship_common_init(Alien *self, StrategyFunc strat) {
    self->sflags |= ASF_SHADOW;
    self->type &= (uint8)~ATZREMOVE;
    self->stratptr = strat;
}

void Strat_ClshipWARPA_Init(Alien *self) {
    clship_common_init(self, clshipWARPa_strat);
    self->sword1 = (int16)(CLSHIP_WARP_BTIME - CLSHIP_BUNNYWAIT - 5u);
    self->sbyte2 = 20u;
    self->vx = 10;
    self->rotz = (uint8)-DEG90;
}

void Strat_ClshipWARPB_Init(Alien *self) {
    clship_common_init(self, clshipWARPb_strat);
    self->sword1 = (int16)(CLSHIP_WARP_BTIME - CLSHIP_FROGWAIT - 16u);
    self->sbyte2 = 20u;
    self->vx = -10;
    self->rotz = DEG90;
}

void Strat_ClshipWARPC_Init(Alien *self) {
    clship_common_init(self, clshipWARPc_strat);
    self->sword1 = (int16)(CLSHIP_WARP_BTIME - CLSHIP_COCKWAIT - 27u);
    self->sbyte2 = 20u;
}

void Strat_ClshipGNDA_Init(Alien *self) {
    clship_common_init(self, clshipGNDa_strat);
    self->rotz = (uint8)-DEG90;
    self->sword1 = (int16)(CLSHIP_GNDWAIT + 80u - CLSHIP_BUNNYWAIT);
}

void Strat_ClshipGNDB_Init(Alien *self) {
    clship_common_init(self, clshipGNDb_strat);
    self->rotz = DEG90;
    self->sword1 = (int16)(CLSHIP_GNDWAIT + 90u - CLSHIP_FROGWAIT);
}

void Strat_ClshipGNDC_Init(Alien *self) {
    clship_common_init(self, clshipGNDc_strat);
    self->sword1 = (int16)(CLSHIP_GNDWAIT + 100u - CLSHIP_COCKWAIT);
}

static const int8 s_clship_rotz_float[15] = {
    0, 1, 2, 2, 1, 0, -1, -2, -2, -1, 0, 1, 2, 1, 0
};

static const int16 s_clship_view_float[7] = {
    0, 4, 7, 4, 0, -4, -7
};

static void clship_float2(Alien *self) {
    self->sbyte3 = (uint8)((self->sbyte3 + 1u) % 15u);
    self->rotz = (uint8)(self->rotz + s_clship_rotz_float[self->sbyte3]);
    self->sbyte4 = (uint8)((self->sbyte4 + 1u) % 7u);
    self->worldy = (int16)(self->worldy + s_clship_view_float[self->sbyte4]);
}

static void clship_cont(Alien *self, int16 zoff, int16 yoff) {
    Alien *player = Obj_GetPlayer();

    if ((int16)abs(self->worldz - g_pviewposz) >= 4000) {
        g_aldead = 1u;
        return;
    }

    if ((self->sflags2 & CLSHIP_FLAG1) != 0u && player && (player->sflags2 & 0x80u) != 0u) {
        if (self->sbyte1 > 0u) {
            self->sbyte1--;
        }
        if (self->sbyte1 == 0u) {
            self->sbyte1 = 1u;
            self->worldz = (int16)(self->worldz + 100);
            self->worldy = (int16)(self->worldy - 10);
            if ((self->sflags2 & CLSHIP_FLAG2) == 0u) {
                self->sflags2 |= CLSHIP_FLAG2;
                self->snd2 = 0x32u;
            }
            add_player_z(self);
            return;
        }
    }

    if (player) {
        self->worldz = Strat_Chase(self->worldz, (int16)(player->worldz - zoff), 4);
        self->worldy = Strat_Chase(self->worldy, (int16)(player->worldy + yoff), 4);
        if (frame_tick_mod(1u)) {
            (void)achase_angle(&self->rotz, player->rotz, 4);
        }
        if (frame_tick_mod(2u)) {
            (void)achase_angle(&self->rotx, player->rotx, 5);
        }
    }

    add_player_z(self);
}

static void clshipEARTHa_strat(Alien *self) {
    self->worldx = Strat_Chase(self->worldx, -50, 4);
    clship_float2(self);
    clship_cont(self, 100, 20);
}

static void clshipEARTHb_strat(Alien *self) {
    self->worldx = Strat_Chase(self->worldx, 50, 4);
    clship_float2(self);
    clship_cont(self, 200, 50);
}

static void clshipEARTHc_strat(Alien *self) {
    self->worldx = Strat_Chase(self->worldx, 0, 4);
    clship_float2(self);
    clship_cont(self, 300, 50);
}

static void clshipCHASEboost_step(Alien *self) {
    Strat_GenVecs2D(self);
    self->vz = 0;
    Strat_ApplyVelocity(self);

    if (frame_tick_mod(1u)) {
        self->rotz = (uint8)(self->rotz - 1u);
    }
    if (frame_tick_mod(3u)) {
        self->roty = (uint8)(self->roty - 1u);
        self->rotx = (uint8)(self->rotx - 1u);
    }

    if (self->sbyte1 > 0u) {
        self->sbyte1--;
    } else {
        self->sbyte1 = 50u;
        self->snd2 = 0x32u;
        self->vel = 20u;
    }

    add_player_z(self);
}

static void clshipCHASEboost_enter(Alien *self) {
    self->stratptr = clshipCHASEboost_step;
    self->sbyte1 = 50u;
    self->snd2 = 0x32u;
    self->vel = 20u;
}

static void clship_chase_cont(Alien *self, int16 zoff, int16 yoff) {
    Alien *player = Obj_GetPlayer();

    if (self->sword1 > 0) {
        self->sword1--;
        if (self->sword1 == 0) {
            clshipCHASEboost_enter(self);
            clshipCHASEboost_step(self);
            return;
        }
    }

    if (player) {
        self->worldz = Strat_Chase(self->worldz, (int16)(player->worldz + zoff), 4);
        self->worldy = Strat_Chase(self->worldy, (int16)(player->worldy + yoff), 5);
        (void)achase_angle(&self->rotx, player->rotx, 5);
        if (frame_tick_mod(1u)) {
            (void)achase_angle(&self->rotz, player->rotz, 5);
            (void)achase_angle(&self->roty, player->roty, 4);
        }
    }

    add_player_z(self);
    self->worldz = (int16)(self->worldz + g_psvar_word2);
}

static void clshipCHASEa_strat(Alien *self) {
    self->worldx = Strat_Chase(self->worldx, -70, 4);
    clship_chase_cont(self, -100, 20);
}

static void clshipCHASEb_strat(Alien *self) {
    self->worldx = Strat_Chase(self->worldx, 70, 4);
    clship_chase_cont(self, -200, 20);
}

static void clshipCHASEc_strat(Alien *self) {
    self->worldx = Strat_Chase(self->worldx, 0, 4);
    clship_chase_cont(self, -300, 30);
}

void Strat_ClshipEARTHA_Init(Alien *self) {
    clship_common_init(self, clshipEARTHa_strat);
    self->sflags2 |= CLSHIP_FLAG1;
    self->sbyte1 = 10u;
    self->rotz = (uint8)-DEG90;
    self->sbyte3 = (uint8)(SfRtl_Random() & 15u);
    self->sbyte4 = (uint8)(SfRtl_Random() & 7u);
}

void Strat_ClshipEARTHB_Init(Alien *self) {
    clship_common_init(self, clshipEARTHb_strat);
    self->sflags2 |= CLSHIP_FLAG1;
    self->sbyte1 = 20u;
    self->rotz = (uint8)(DEG90 + DEG45);
    self->sbyte3 = (uint8)(SfRtl_Random() & 15u);
    self->sbyte4 = (uint8)(SfRtl_Random() & 7u);
}

void Strat_ClshipEARTHC_Init(Alien *self) {
    clship_common_init(self, clshipEARTHc_strat);
    self->sflags2 |= CLSHIP_FLAG1;
    self->sbyte1 = 30u;
    self->sbyte3 = (uint8)(SfRtl_Random() & 15u);
    self->sbyte4 = (uint8)(SfRtl_Random() & 7u);
}

void Strat_ClshipCHASEA_Init(Alien *self) {
    clship_common_init(self, clshipCHASEa_strat);
    self->rotz = (uint8)-DEG90;
    self->roty = DEG45;
    self->sword1 = (int16)(246u + 5u - CLSHIP_FROGWAIT);
}

void Strat_ClshipCHASEB_Init(Alien *self) {
    clship_common_init(self, clshipCHASEb_strat);
    self->rotz = DEG90;
    self->roty = (uint8)-DEG45;
    self->sword1 = (int16)(246u + 10u - CLSHIP_BUNNYWAIT);
}

void Strat_ClshipCHASEC_Init(Alien *self) {
    clship_common_init(self, clshipCHASEc_strat);
    self->sword1 = (int16)(246u + 15u - CLSHIP_COCKWAIT);
}

// ============================================================
// BOSS EXPLOSION STRATEGIES (EXPSTRAT.ASM)
// These are set as expstratptr by boss strategies.
// ============================================================

// --- BGM id for boss dying fade-out ---
#define BGM_BOSS_DYING  0xF1u

// --- Sound effect for boss dying trigger ---
#define SE_BOSS_DYING   0x1Eu

// Forward declarations for internal per-frame strategies.
static void bossdelayexplode_strat(Alien *self);
static void circdelayexplode_strat(Alien *self);
static void delayexplode_strat(Alien *self);
static void delayremove_strat(Alien *self);

// ============================================================
// Helper: add random signed 8-bit offset to alien X and Y positions.
// Port of addrnd2posy_srou (EXPSTRAT.ASM:342-357)
//   random_l → sign-extend 8-bit to 16-bit → add to worldx
//   random_l → sign-extend 8-bit to 16-bit → add to worldy
// ============================================================
static void addrnd2pos_xy(Alien *al) {
    int8 rx;
    int8 ry;

    if (!al) {
        return;
    }

    rx = (int8)(SfRtl_Random() & 0xFF);
    ry = (int8)(SfRtl_Random() & 0xFF);
    al->worldx = (int16)(al->worldx + (int16)rx);
    al->worldy = (int16)(al->worldy + (int16)ry);
}

// ============================================================
// Helper: copy position from src to dst alien.
// Port of s_copy_pos macro.
// ============================================================
static void copy_pos(Alien *dst, const Alien *src) {
    if (!dst || !src) {
        return;
    }
    dst->worldx = src->worldx;
    dst->worldy = src->worldy;
    dst->worldz = src->worldz;
}

// ============================================================
// Helper: create a delay-explode child object at parent's position.
// Port of makeexpobj_srou (EXPSTRAT.ASM:327-338).
// Creates a non-real, collision-disabled, relexplode child that
// runs delayexplode_Istrat as its strategy.
// Returns NULL if the alien pool is exhausted.
// ============================================================
static Alien *make_exp_obj(const Alien *parent) {
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
    child->stratptr = delayexplode_strat;
    child->collstratptr = NULL;
    child->expstratptr = Strat_Explode;
    copy_pos(child, parent);

    return child;
}

// ============================================================
// Sized explosion object factories.
// Ports of makeLexpobj_srou, makeMEDexpobj_srou,
//         makeSMLexpobj_srou, makeFOLexpobj_srou.
// The shape field encodes the explosion size category.
// We reuse small shape IDs as size markers for the renderer.
// ============================================================
#define EXPSHAPE_SMALL   1u
#define EXPSHAPE_MEDIUM  2u
#define EXPSHAPE_LARGE   3u
#define EXPSHAPE_FOLARGE 4u

static Alien *make_large_exp_obj(const Alien *parent) {
    Alien *child = make_exp_obj(parent);
    if (child) {
        child->shape = EXPSHAPE_LARGE;
    }
    return child;
}

static Alien *make_medium_exp_obj(const Alien *parent) {
    Alien *child = make_exp_obj(parent);
    if (child) {
        child->shape = EXPSHAPE_MEDIUM;
    }
    return child;
}

static Alien *make_small_exp_obj(const Alien *parent) {
    Alien *child = make_exp_obj(parent);
    if (child) {
        child->shape = EXPSHAPE_SMALL;
    }
    return child;
}

static Alien *make_fol_exp_obj(const Alien *parent) {
    Alien *child = make_exp_obj(parent);
    if (child) {
        child->shape = EXPSHAPE_FOLARGE;
    }
    return child;
}

// ============================================================
// s_boss_dying macro (STRATMAC.INC:7758-7767)
// Triggers dying BGM, sets BF_DYING, PSTF_NOTDIE, disables player fire.
// Only triggers sound/BGM change on first call (checks bf_dying).
// ============================================================
static void boss_dying(void) {
    if ((g_bossflags & BF_DYING) == 0u) {
        Sound_PlaySE(SE_BOSS_DYING);
        Sound_PlayMusic(BGM_BOSS_DYING);
        g_bossflags |= BF_DYING;
        g_pstratflags |= PSTF_NOTDIE;
        g_stratflags |= SF_NOFIRING;
    }
}

// ============================================================
// delayexplode_strat (EXPSTRAT.ASM:259-268)
// Per-frame: sets hitflash, decrements lifecnt.
// When lifecnt reaches zero: calls expstratptr (or ends).
// If relexplode flag set, adds player Z each frame.
// ============================================================
static void delayexplode_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->sflags |= ASF_HITFLASH;

    if (self->count > 0u) {
        self->count--;
    }

    if (self->count == 0u) {
        // Timer expired — call explosion strategy and remove.
        g_aldead = 1u;
        if (self->expstratptr) {
            self->expstratptr(self);
        }
        return;
    }

    // Still counting down — scroll with player if relexplode set.
    if (self->sflags2 & ASF2_RELEXPLODE) {
        add_player_z(self);
    }
}

// ============================================================
// delayremove_strat (GSTRATS.ASM:1188-1193)
// Per-frame: decrements lifecnt, adds player Z if relexplode.
// When lifecnt reaches zero: removes self.
// ============================================================
static void delayremove_strat(Alien *self) {
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
        add_player_z(self);
    }
}

// ============================================================
// circdelayexplode_Istrat / circdelayexplode_strat
// (EXPSTRAT.ASM:273-294)
// Init: sets hard vars, collision disabled, assigns strat.
// Per-frame: decrements lifecnt.  When zero: creates circle
// explosion effect, optionally creates BIG particle explosion
// (if sflag1 set), then removes self.
// Otherwise: adds player Z each frame.
// ============================================================
static void circdelayexplode_init(Alien *self) {
    if (!self) {
        return;
    }

    self->HP = HARD_HP;
    self->AP = HARD_AP;
    self->sflags |= ASF_COLLDISABLE;
    self->stratptr = circdelayexplode_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;
}

static void circdelayexplode_strat(Alien *self) {
    if (!self) {
        return;
    }

    if (self->count > 0u) {
        self->count--;
    }

    if (self->count == 0u) {
        // makebosscircexp_srou: triggers a circle/FOL explosion effect.
        // In the HD build the circle-fill visual is handled by the renderer;
        // here we just play the explosion sound.
        Sound_PlaySE(0x1Du);

        // If sflag1 set, also create a BIG particle explosion child.
        if (self->sflags2 & ASF2_SFLAG1) {
            Alien *big = Strat_MakeObj(0u);
            if (big) {
                copy_pos(big, self);
                big->sflags |= ASF_COLLDISABLE;
                big->sflags2 |= ASF2_RELEXPLODE;
                big->flags |= AFEXP;
                big->count = 110u;
                big->stratptr = delayremove_strat;
                big->collstratptr = NULL;
                big->expstratptr = NULL;
            }
        }

        // Remove self.
        g_aldead = 1u;
        return;
    }

    add_player_z(self);
}

// ============================================================
// Bossdelayexplode_Istrat / Bossdelayexplode_strat
// (EXPSTRAT.ASM:46-65)
//
// Init: sets hard vars, assigns per-frame strat, sets
//       expstratptr = explode_Istrat.
// Per-frame: hitflash every frame; decrements lifecnt.
//   When zero → kill obj, make FOL explosion, set GF_BOSSDEAD,
//               jump to expstrat.
//   Otherwise → add player Z, check/call tempstrat.
// ============================================================

static void bossdelayexplode_strat(Alien *self) {
    if (!self) {
        return;
    }

    self->sflags |= ASF_HITFLASH;

    // s_decbpl_lifecnt: decrement count, if still positive → .nd
    if (self->count > 0u) {
        self->count--;
    }

    if (self->count == 0u) {
        // Timer expired — final detonation.
        // s_kill_obj: mark for removal.
        g_aldead = 1u;

        // s_jsl makeFOLexpobj_srou: create a fill-object-large explosion.
        (void)make_fol_exp_obj(self);

        // s_or_var B,gameflags,#gf_bossdead
        g_gameflags |= GF_BOSSDEAD;

        // s_jmpto_expstrat: call explosion strategy if set.
        if (self->expstratptr) {
            self->expstratptr(self);
        }
        return;
    }

    // .nd: still counting down.
    add_player_z(self);

    // Check tempstratptr — if non-NULL, call it.
    // (s_jmpto_tempstrat: jumps to saved strategy if set)
    if (self->tempstratptr) {
        self->tempstratptr(self);
    }
}

void Strat_BossDelayExplode_Init(Alien *self) {
    if (!self) {
        return;
    }

    // s_hardvars
    set_hard_vars(self);

    // s_set_alptrs x,Bossdelayexplode_strat,0,explode_Istrat
    self->stratptr = bossdelayexplode_strat;
    self->collstratptr = NULL;
    self->expstratptr = Strat_Explode;

    // The caller is responsible for setting self->count (lifecnt)
    // before the first frame tick.
}

// ============================================================
// Qbossexplode_Istrat (EXPSTRAT.ASM:68-74)
//
// Quick boss explosion: immediately sets lifecnt=0, GF_BOSSDEAD,
// relexplode flag, sflag1, then enters circdelayexplode.
// ============================================================

void Strat_QBossExplode_Init(Alien *self) {
    if (!self) {
        return;
    }

    // s_set_lifecnt x,#0
    self->count = 0u;

    // s_or_var B,gameflags,#gf_bossdead
    g_gameflags |= GF_BOSSDEAD;

    // s_set_alsflag x,relexplode
    self->sflags2 |= ASF2_RELEXPLODE;

    // s_set_alsflag x,sflag1
    self->sflags2 |= ASF2_SFLAG1;

    // s_jmp circdelayexplode_Istrat
    circdelayexplode_init(self);
}

// ============================================================
// bossexplode_Istrat (EXPSTRAT.ASM:78-138)
//
// Staged multi-part boss explosion:
// 1. s_boss_dying (trigger sound, set BF_DYING, PSTF_NOTDIE,
//    disable player fire)
// 2. Create a sequence of timed small/medium/large explosion
//    child objects at random position offsets.
// 3. Create a circdelayexplode proxy child (with sflag1 set)
//    that will later produce the big circle + particle explosion.
// 4. Set lifecnt = 38 frames, enter bossdelayexplode.
//
// The explosion timeline (lifecnt values are offset by -20 from
// the ASM because the ASM sets them relative to spawn time):
//   Frame  5: small exp (noexpsnd cleared)
//   Frame 10: small exp
//   Frame 15: medium exp
//   Frame 17: large exp
//   Frame 19: medium exp
//   Frame 22: large exp
//   Frame 24: medium exp
//   Frame 26: large exp
//   Frame 28: medium exp
//   Frame 29: large exp
//   Frame 32: medium exp
//   Frame 32: large exp (same frame, different object)
//   Frame 34: large exp
//   Frame 34: large exp (same frame, different object)
// Then a circdelayexplode proxy at frame 15.
// Finally self enters bossdelayexplode with lifecnt = 38.
// ============================================================

void Strat_BossExplode_Init(Alien *self) {
    Alien *child;
    Alien *proxy;

    if (!self) {
        return;
    }

    // s_boss_dying
    boss_dying();

    // --- Create timed explosion children ---
    // Each child gets a lifecnt (count) and a random XY position offset.

    // makeSMLexpobj + addrnd2pos, lifecnt = 25-20 = 5
    child = make_small_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 5u;
        child->sflags2 &= (uint8)~ASF2_NOEXPSND;  // s_clr_alsflag y,noexpsnd
    }

    // makeSMLexpobj + addrnd2pos, lifecnt = 30-20 = 10
    child = make_small_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 10u;
    }

    // makeMEDexpobj + addrnd2pos, lifecnt = 35-20 = 15
    child = make_medium_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 15u;
    }

    // makeLexpobj + addrnd2pos, lifecnt = 37-20 = 17
    child = make_large_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 17u;
    }

    // makeMEDexpobj + addrnd2pos, lifecnt = 39-20 = 19
    child = make_medium_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 19u;
    }

    // makeMEDexpobj + addrnd2pos, lifecnt = 42-20 = 22
    child = make_medium_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 22u;
    }

    // makeLexpobj + addrnd2pos, lifecnt = 44-20 = 24
    child = make_large_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 24u;
    }

    // makeMEDexpobj + addrnd2pos, lifecnt = 46-20 = 26
    child = make_medium_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 26u;
    }

    // makeLexpobj + addrnd2pos, lifecnt = 48-20 = 28
    child = make_large_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 28u;
    }

    // makeMEDexpobj + addrnd2pos, lifecnt = 49-20 = 29
    child = make_medium_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 29u;
    }

    // makeLexpobj + addrnd2pos, lifecnt = 52-20 = 32
    child = make_large_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 32u;
    }

    // makeMEDexpobj + addrnd2pos, lifecnt = 52-20 = 32
    child = make_medium_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 32u;
    }

    // makeLexpobj + addrnd2pos, lifecnt = 54-20 = 34
    child = make_large_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 34u;
    }

    // makeLexpobj + addrnd2pos, lifecnt = 54-20 = 34
    child = make_large_exp_obj(self);
    if (child) {
        addrnd2pos_xy(child);
        child->count = 34u;
    }

    // --- Create circdelayexplode proxy ---
    // s_make_obj #nullshape → copy sflags, clear realobj, set colldisable,
    // set noexpsnd, copy pos, set lifecnt = 35-20 = 15, set sflag1,
    // set strat = circdelayexplode_Istrat.
    proxy = Strat_MakeObj(0u);
    if (proxy) {
        proxy->sflags = self->sflags;
        proxy->sflags2 = self->sflags2;
        proxy->sflags3 = self->sflags3;
        proxy->sflags4 = self->sflags4;
        proxy->sflags3 &= (uint8)~ASF3_REALOBJ;
        proxy->sflags |= ASF_COLLDISABLE;
        proxy->sflags2 |= ASF2_NOEXPSND;
        copy_pos(proxy, self);
        proxy->count = 15u;
        proxy->sflags2 |= ASF2_SFLAG1;
        circdelayexplode_init(proxy);
    }

    // --- Enter bossdelayexplode ---
    // s_set_lifecnt x,#58-20 = 38
    self->count = 38u;

    // s_jmp bossdelayexplode_Istrat
    Strat_BossDelayExplode_Init(self);
}

// ============================================================
// SPACEPILON STRATEGY (GA3STRAT.ASM)
// Three-pronged spinning space enemy with child pilon arms.
// The mother object zooms in from spawn, then rotates and fires
// HPLASMA at the player. Each child arm orbits the mother using
// al_rotz as orbit angle and al_relposy as radial offset.
// ============================================================

#define SH_PILON              615u
#define SPACEPILON_HP         6u
#define SPACEPILON_AP         0u
#define SPACEPILON_ZOOMIN_CNT 40u
#define SPACEPILON_RELEASE_CNT 150u
#define SPACEPILON_ROT_STEP   8u
#define SPACEPILON_FIRE_MASK  31u   // (1<<5)-1 = every 32 frames
#define SPACEPILON_CHILD1_ROT 85    // deg45 + deg180
#define SPACEPILON_CHILD2_ROT (-85) // -deg45 + deg180
#define SPACEPILON_PILON_SBYTE3_INIT 10u
#define SPACEPILON_PILON_RELPOSY_INIT ((int8)(-500/8))  // -62
#define SPACEPILON_PILON_RELPOSY_TGT  ((int8)(-100/8))  // -12
#define SPACEPILON_PILON_EXTEND_STEP  ((int8)(-80/8))    // -10
#define SPACEPILON_PILON_RETRACT_STEP ((int8)(80/8))     // +10
#define SPACEPILON_PILON_ORBIT_SCALE  3
#define SPACEPILON_SPAWN_ZOFF (-500)

// --- Forward declarations ---
static void spacepilon_strat(Alien *self);
static void spacepilonP_strat(Alien *self);
static void spacepiloncol_init(Alien *self);
static void spacepilonexp_init(Alien *self);

// --- Child pilon helper: rotate an (x,y) pair by angle ---
// Equivalent to rotate_8yx_l: rotate around Z axis by 8-bit angle.
static void spacepilon_rotate_z(int16 in_x, int16 in_y,
                                uint8 angle,
                                int16 *out_x, int16 *out_y) {
    float s = strat_sin(angle);
    float c = strat_cos(angle);
    *out_x = (int16)(in_x * c - in_y * s);
    *out_y = (int16)(in_x * s + in_y * c);
}

// --- spacepilonP_strat: child pilon per-frame ---
static void spacepilonP_strat(Alien *self) {
    Alien *mother;
    int8  relposy;
    int16 off_x, off_y;

    if (!self) {
        return;
    }

    // s_set_objtobemother y,x — get mother from ptr
    mother = boss_get_mother_obj(self);
    if (!mother || !mother->active) {
        g_aldead = 1u;
        return;
    }

    // s_copy_alvar2alvar B,x,al_rotz,y,al_rotz — copy mother rotz
    self->rotz = mother->rotz;
    // s_add_alvars B,x,al_rotz,x,al_sbyte2 — add orbit offset
    self->rotz = (uint8)(self->rotz + self->sbyte2);

    // s_copy_alvar2var B,x,svar_byte1,al_relposy
    relposy = (int8)self->relposy;

    // s_add_Roffs2pos B,x,y,x,#0,svar_byte1,#0,0,0,1,3,3,3
    // Rotate (0, relposy, 0) by self->rotz, scale <<3, add to mother pos
    off_x = 0;
    off_y = (int16)relposy;
    spacepilon_rotate_z(off_x, off_y, self->rotz, &off_x, &off_y);
    self->worldx = (int16)(mother->worldx + (off_x << SPACEPILON_PILON_ORBIT_SCALE));
    self->worldy = (int16)(mother->worldy + (off_y << SPACEPILON_PILON_ORBIT_SCALE));
    self->worldz = mother->worldz;

    // --- State machine ---
    switch (self->stratstate) {
    case 0u:
        // s_achase_alvar B,x,al_relposy,#-100/8,3
        {
            int8 tgt = SPACEPILON_PILON_RELPOSY_TGT;
            int8 diff = (int8)(relposy - tgt);
            int8 step = diff >> 3;
            if (step == 0) step = (diff > 0) ? 1 : (diff < 0) ? -1 : 0;
            if (diff != 0) {
                self->relposy = (uint8)(int8)(relposy - step);
            }
        }
        break;

    case 1u:
        // s_set_alsflag y,colldisable — disable mother collision
        mother->sflags |= ASF_COLLDISABLE;
        // s_add_alvar B,x,al_relposy,#-80/8 — extend outward
        self->relposy = (uint8)((int8)self->relposy + SPACEPILON_PILON_EXTEND_STEP);
        // s_decbne_alvar B,x,al_sbyte3
        if (self->sbyte3 > 0u) {
            self->sbyte3--;
        }
        if (self->sbyte3 == 0u) {
            // s_next_state
            self->stratstate++;
            self->sbyte3 = SPACEPILON_PILON_SBYTE3_INIT;
            // trigse $2c
            Strat_TrigSE(0x2Cu);
        }
        break;

    case 2u:
        // s_sub_alvar B,x,al_relposy,#-80/8 — retract (sub negative = add)
        self->relposy = (uint8)((int8)self->relposy + SPACEPILON_PILON_RETRACT_STEP);
        // s_decbne_alvar B,x,al_sbyte3
        if (self->sbyte3 > 0u) {
            self->sbyte3--;
        }
        if (self->sbyte3 == 0u) {
            // s_set_state x,#0
            self->stratstate = 0u;
            self->sbyte3 = SPACEPILON_PILON_SBYTE3_INIT;
            // s_clr_alsflag y,colldisable — re-enable mother collision
            mother->sflags &= (uint8)~ASF_COLLDISABLE;
        }
        break;

    default:
        self->stratstate = 0u;
        break;
    }
}

// --- spacepilonP_Istrat: child pilon init ---
static void spacepilonP_init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = spacepilonP_strat;
    self->type &= (uint8)~ATZREMOVE;  // s_setnoremove_behind
    self->sbyte3 = SPACEPILON_PILON_SBYTE3_INIT;
    self->HP = HARD_HP;  // hardHP = 0xFF
    self->AP = HARD_AP;  // hardAP = 8
    self->relposy = (uint8)SPACEPILON_PILON_RELPOSY_INIT;
}

// --- Spawn a child pilon and attach to mother ---
static Alien *spacepilon_spawn_pilon(Alien *mother, uint8 child_num, int8 orbit_rot) {
    Alien *child;

    child = Strat_MakeObj(SH_PILON);
    if (!child) {
        return NULL;
    }

    if (!boss_attach_child_to_mother(mother, child, child_num)) {
        g_aldead = 1u;
        return NULL;
    }

    child->collflags |= ACF_COLLTYPE4;  // enemyweap collision
    spacepilonP_init(child);
    child->sbyte2 = (uint8)orbit_rot;

    return child;
}

// --- spacepiloncol_Istrat: mother collision handler ---
static void spacepiloncol_init(Alien *self) {
    Alien *child;

    if (!self) {
        return;
    }

    // trigse $27
    Strat_TrigSE(0x27u);

    // s_set_childstate #1,#1; #2,#1; #3,#1
    child = boss_find_child_obj(self, 1u);
    if (child) child->stratstate = 1u;
    child = boss_find_child_obj(self, 2u);
    if (child) child->stratstate = 1u;
    child = boss_find_child_obj(self, 3u);
    if (child) child->stratstate = 1u;

    // s_set_alvar B,x,al_sbyte2,#10
    self->sbyte2 = 10u;

    // s_jsl makeLexpobj_srou — create large explosion child
    {
        Alien *exp = make_large_exp_obj(self);
        if (exp) {
            // s_set_alsflag y,noexpsnd
            exp->sflags2 |= ASF2_NOEXPSND;
        }
    }

    // s_jmp coll_Istrat — fall through to standard hit flash
    Strat_HitFlash(self);
}

// --- spacepilonexp_Istrat: mother explode handler ---
static void spacepilonexp_init(Alien *self) {
    Alien *child;

    if (!self) {
        return;
    }

    // Remove all three child pilons
    child = boss_find_child_obj(self, 1u);
    if (child) {
        child->active = false;
        Obj_Free(child);
    }
    child = boss_find_child_obj(self, 2u);
    if (child) {
        child->active = false;
        Obj_Free(child);
    }
    child = boss_find_child_obj(self, 3u);
    if (child) {
        child->active = false;
        Obj_Free(child);
    }

    // s_jmp explode_Istrat
    Strat_Explode(self);
}

// --- spacepilon_strat: mother per-frame ---
static void spacepilon_strat(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();

    // --- State 0: Zoom in from spawn pos toward saved target pos ---
    if (self->stratstate == 0u) {
        // s_set_alsflag x,colldisable
        self->sflags |= ASF_COLLDISABLE;

        // s_achase_alvar2alvar W,x,al_worldx,x,al_vx,4
        self->worldx = Strat_ChaseProportional(self->worldx, self->vx, 4);
        // s_achase_alvar2alvar W,x,al_worldy,x,al_vy,4
        self->worldy = Strat_ChaseProportional(self->worldy, self->vy, 4);
        // s_achase_alvar2alvar W,x,al_worldz,x,al_vz,4
        self->worldz = Strat_ChaseProportional(self->worldz, self->vz, 4);

        // s_decbne_alvar B,x,al_sbyte2
        if (self->sbyte2 > 0u) {
            self->sbyte2--;
        }
        if (self->sbyte2 == 0u) {
            // s_clr_alsflag x,colldisable
            self->sflags &= (uint8)~ASF_COLLDISABLE;
            // s_next_state
            self->stratstate++;
            // s_set_alvar B,x,al_sbyte2,#1
            self->sbyte2 = 1u;
        }
    }

    // --- State 1: Spin and shoot ---
    if (self->stratstate == 1u) {
        // s_decbne_alvar B,x,al_sbyte2
        if (self->sbyte2 > 0u) {
            self->sbyte2--;
        }
        if (self->sbyte2 == 0u) {
            // s_set_alvar B,x,al_sbyte2,#1
            self->sbyte2 = 1u;
            // s_add_alvar B,x,al_rotz,#8
            self->rotz = (uint8)(self->rotz + SPACEPILON_ROT_STEP);
        }

        // s_jmp_notdelay 5,.nsshoot — fire only when (gameframe & 31) == 0
        if ((g_gameframe & SPACEPILON_FIRE_MASK) == 0u) {
            // s_weapon_pos #0,#0,#0 (weapon at center)
            // s_set_objtobeplayer y
            // s_weapon_rots2obj y — aim at player
            // s_fire_weapon x,HPLASMA
            if (player && player->active) {
                uint8 fire_pitch = strat_pitch_toward(self, player);
                uint8 fire_yaw = Strat_AngleXZ(self, player);

                Alien *shot = Strat_SpawnProjectile(self,
                    0, 0, 0,
                    fire_pitch, fire_yaw,
                    HPLASMA_SPEED, HPLASMA_LIFE, HPLASMA_AP,
                    ACF_COLLTYPE4);
                if (shot) {
                    // s_set_alvar W,y,al_ptr,playpt — track player
                    shot->ptr = (uint16)((player - g_aliens) + 1u);
                }
            }
        }
    }

    // --- Countdown before enabling z-remove ---
    // s_beqdec_alvar B,x,al_sbyte4,.nrel
    if (self->sbyte4 == 0u) {
        // .nrel: enable z-remove on self and all children
        Alien *c;
        self->type |= ATZREMOVE;  // s_setremove_behind x
        c = boss_find_child_obj(self, 1u);
        if (c) c->type |= ATZREMOVE;
        c = boss_find_child_obj(self, 2u);
        if (c) c->type |= ATZREMOVE;
        c = boss_find_child_obj(self, 3u);
        if (c) c->type |= ATZREMOVE;
        return;
    }
    self->sbyte4--;

    // s_add_playerZ x — keep scrolling with world
    add_player_z(self);
    // s_add_alvar W,x,al_vz,pviewvelz — adjust target Z for scrolling
    self->vz = (int16)(self->vz + g_pviewvelz);
}

// --- spacepilon_Istrat: mother init (GA3STRAT.ASM) ---
void Strat_Spacepilon_Init(Alien *self) {
    if (!self) {
        return;
    }

    // s_make_childobj #pilon,#1,spacepilonP_Istrat,enemyweap
    (void)spacepilon_spawn_pilon(self, 1u, (int8)SPACEPILON_CHILD1_ROT);
    // s_make_childobj #pilon,#2,spacepilonP_Istrat,enemyweap
    (void)spacepilon_spawn_pilon(self, 2u, (int8)SPACEPILON_CHILD2_ROT);
    // s_make_childobj #pilon,#3,spacepilonP_Istrat,enemyweap (sbyte2 = 0)
    (void)spacepilon_spawn_pilon(self, 3u, 0);

    // s_setnoremove_behind x
    self->type &= (uint8)~ATZREMOVE;

    // s_add_rnd2pos x,255,255,255,2,2,1 (randomize initial pos slightly)
    // Small random offset to break up repeated spawns — simplified
    {
        uint8 phase = (uint8)(self - g_aliens);
        self->worldx = (int16)(self->worldx + (int8)(phase * 37u));
        self->worldy = (int16)(self->worldy + (int8)(phase * 53u));
        self->worldz = (int16)(self->worldz + (int8)(phase * 17u));
    }

    // Save original spawn position as chase target in velocity fields
    // s_copy_alvar2alvar W,x,al_vx,x,al_worldx etc.
    self->vx = self->worldx;
    self->vy = self->worldy;
    self->vz = self->worldz;

    // Move to player position for zoom-in
    // s_set_alvar W,x,al_worldx,player_posx etc.
    {
        Alien *player = Obj_GetPlayer();
        if (player && player->active) {
            self->worldx = player->worldx;
            self->worldy = player->worldy;
            self->worldz = (int16)(player->worldz + SPACEPILON_SPAWN_ZOFF);
        }
    }

    // s_set_alptrs x,spacepilon_strat,spacepiloncol_Istrat,spacepilonexp_Istrat
    self->stratptr = spacepilon_strat;
    self->collstratptr = spacepiloncol_init;
    self->expstratptr = spacepilonexp_init;

    // s_set_alvar B,x,al_sbyte2,#40
    self->sbyte2 = SPACEPILON_ZOOMIN_CNT;

    // s_set_aldata x,#6,#0
    self->HP = SPACEPILON_HP;
    self->AP = SPACEPILON_AP;

    // s_set_alvar B,x,al_sbyte4,#150
    self->sbyte4 = SPACEPILON_RELEASE_CNT;

    // s_set_colltype x,enemyweap
    self->collflags |= ACF_COLLTYPE4;

    // trigse $2c
    Strat_TrigSE(0x2Cu);

    self->stratstate = 0u;
}

// ============================================================
// BOSS F — VENOM 2 SPACE BOSS (GB2STRAT.ASM)
// Multi-part space boss with two wing children (FA, FB),
// a phase-2 transformation spawning 6 turrets, and a final
// death sequence.
// ============================================================

// --- Shape IDs ---
// boss_f_3 is shape index 107 from def_shape; boss_f_4 is 120.
// boss_F_1, boss_F_2, boss_F_8, boss_F_9, boss_f_8a, boss_f_9a
// are additional SHAPES4 shapes assigned stable flat IDs.
#define SH_BOSS_F_3      107u
#define SH_BOSS_F_4      120u
#define SH_BOSS_F_1      251u
#define SH_BOSS_F_2      252u
#define SH_BOSS_F_8      253u
#define SH_BOSS_F_9      254u
#define SH_BOSS_F_8A     255u
#define SH_BOSS_F_9A     256u

// --- Boss F constants from STRATEQU.INC ---
#define BOSSF_SCALE             3
#define BOSSF_LAUNCHER_AP       10u
#define BOSSF_LAUNCHER_HP       4u   // bossFlauncherHP = 8/2 = 4
#define BOSSF_LAUNCHER2_HP      8u   // bossFlauncher2HP = 16/2 = 8
#define BOSSF_SPACE_VIEWCY      (-60)

// --- Child numbers ---
#define BOSSF_CHILD_A           1u
#define BOSSF_CHILD_B           2u
#define BOSSF_CHILD_TUR1        1u
#define BOSSF_CHILD_TUR2        2u
#define BOSSF_CHILD_TUR3        3u
#define BOSSF_CHILD_TUR4        4u
#define BOSSF_CHILD_TUR5        5u
#define BOSSF_CHILD_TUR6        6u

// --- Weapon scale ---
#define WEAPON_SCALE            2

// Forward declarations for bossF
static void bossFC_strat(Alien *self);
static void bossFC2start_init(Alien *self);
static void bossFC2_init(Alien *self);
static void bossFC2b_init(Alien *self);
static void bossFC2_strat(Alien *self);
static void bossFC2_cont(Alien *self);
static void bossFC2_cont2(Alien *self);
static void bossFC2_cont3(Alien *self);
static void bossFC3_init(Alien *self);
static void bossFC3_strat(Alien *self);
static void bossFCdie_init(Alien *self);
static void bossFCdie_strat(Alien *self);
static void bossFCdie2_init(Alien *self);
static void bossFCdie2_strat(Alien *self);
static void bossFA_Istrat(Alien *self);
static void bossFA_strat(Alien *self);
static void bossFB_Istrat(Alien *self);
static void bossFB_strat(Alien *self);
static void bossFtur1_Istrat(Alien *self);
static void bossFtur2_Istrat(Alien *self);
static void bossFtur3_Istrat(Alien *self);
static void bossFtur4_Istrat(Alien *self);
static void bossFtur5_Istrat(Alien *self);
static void bossFtur6_Istrat(Alien *self);
static void bossFtur_Icont(Alien *self);
static void bossFtur1_strat(Alien *self);
static void bossFtur2_strat(Alien *self);
static void bossFtur3_strat(Alien *self);
static void bossFtur4_strat(Alien *self);
static void bossFtur5_strat(Alien *self);
static void bossFtur6_strat(Alien *self);
static void bossFtur_cont(Alien *self);
static void bossfexp1_Istrat(Alien *self);
static void bossfexp2_Istrat(Alien *self);

// --- Helper: Spawn a child object with a given shape and child number ---
static Alien *bossF_spawn_child(Alien *mother, uint16 shape, uint8 child_num,
                                StrategyFunc init_fn) {
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
    child->collflags |= ACF_COLLTYPE3; // enemy2
    if (!boss_attach_child_to_mother(mother, child, child_num)) {
        Obj_Free(child);
        return NULL;
    }

    init_fn(child);
    return child;
}

// --- Helper: Spawn child at mother pos + offset ---
static Alien *bossF_spawn_child_pos(Alien *mother, uint16 shape, uint8 child_num,
                                    int16 offx, int16 offy, int16 offz,
                                    StrategyFunc init_fn) {
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
    child->collflags |= ACF_COLLTYPE3; // enemy2
    child->worldx = (int16)(mother->worldx + offx);
    child->worldy = (int16)(mother->worldy + offy);
    child->worldz = (int16)(mother->worldz + offz);
    if (!boss_attach_child_to_mother(mother, child, child_num)) {
        Obj_Free(child);
        return NULL;
    }

    init_fn(child);
    return child;
}

// --- Helper: 3D rotated offset (s_add_Roffs2pos with rotx,roty,rotz + scale) ---
// Simplified version: rotate (offx,offy,offz) by ref's rotx/roty/rotz, scale by
// bossF_scale, then add to dst's position.
static void bossF_rotated_offset(Alien *dst, const Alien *ref,
                                 int16 offx, int16 offy, int16 offz,
                                 int scale) {
    float sx;
    float sy;
    float sz;
    float cx;
    float cy;
    float cz;
    float fx;
    float fy;
    float fz;
    float tx;
    float ty;

    if (!dst || !ref) {
        return;
    }

    fx = (float)offx;
    fy = (float)offy;
    fz = (float)offz;

    // Rotate around Z axis (rotz)
    sz = strat_sin(ref->rotz);
    cz = strat_cos(ref->rotz);
    tx = fx * cz - fy * sz;
    ty = fx * sz + fy * cz;
    fx = tx;
    fy = ty;

    // Rotate around X axis (rotx)
    sx = strat_sin(ref->rotx);
    cx = strat_cos(ref->rotx);
    ty = fy * cx - fz * sx;
    fz = fy * sx + fz * cx;
    fy = ty;

    // Rotate around Y axis (roty)
    sy = strat_sin(ref->roty);
    cy = strat_cos(ref->roty);
    tx = fx * cy + fz * sy;
    fz = -fx * sy + fz * cy;
    fx = tx;

    // Apply scale
    if (scale > 0) {
        fx = fx * (float)(1 << scale);
        fy = fy * (float)(1 << scale);
        fz = fz * (float)(1 << scale);
    }

    dst->worldx = (int16)(ref->worldx + (int16)lroundf(fx));
    dst->worldy = (int16)(ref->worldy + (int16)lroundf(fy));
    dst->worldz = (int16)(ref->worldz + (int16)lroundf(fz));
}

// --- Helper: Smoke subroutine (bossFCsmoke_srou) ---
// Every 3rd frame, create a smoke object at offset from self.
static void bossFCsmoke_srou(Alien *self) {
    Alien *smoke;

    if (!self) {
        return;
    }

    if ((g_gameframe % 4u) != 0u) {
        return;
    }

    Sound_PlaySE(0x23u);
    smoke = Strat_MakeObj(0u);
    if (!smoke) {
        return;
    }

    // Position smoke at offset (0,-20,0) rotated by parent
    bossF_rotated_offset(smoke, self, 0, -20, 0, BOSSF_SCALE);
    smoke->shape = EXPSHAPE_LARGE; // Lsmoke placeholder
    smoke->count = 10u;
    smoke->sflags |= ASF_COLLDISABLE;
    smoke->sflags2 |= ASF2_NOEXPSND;
    smoke->stratptr = delayexplode_strat;
    smoke->expstratptr = Strat_Explode;
}

// --- Helper: Fire Hplasma toward player ---
static void bossF_fire_hplasma(Alien *self, int16 wpx, int16 wpy) {
    Alien *player;
    Alien *shot;
    uint8 yaw;
    uint8 pitch;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }

    yaw = Strat_AngleXZ(self, player);
    pitch = strat_pitch_toward(self, player);

    shot = Strat_SpawnProjectile(self, wpx, wpy, 0, pitch, yaw,
                                 HPLASMA_SPEED, HPLASMA_LIFE, HPLASMA_AP,
                                 ACF_COLLTYPE4);
    if (shot) {
        shot->ptr = (uint16)((player - g_aliens) + 1u); // playpt
    }
}

// --- Helper: Remove a specific child by child number ---
static void bossF_remove_child(Alien *mother, uint8 child_num) {
    Alien *child;

    if (!mother) {
        return;
    }

    child = boss_find_child_obj(mother, child_num);
    if (child) {
        boss_clear_child_link(child);
        Obj_Free(child);
        boss_prune_family_links(mother);
    }
}

// ============================================================
// bossF_Istrat — Parent initialization (GB2STRAT.ASM:48)
// Shape: boss_f_3. Sets HP=hardHP, AP=hardAP.
// Spawns child A (boss_F_1) at +400 worldx.
// Sets colltype enemy2, state 0, nohitaffect.
// bossmaxHP = 0 initially. sbyte2 = 200 (countdown).
// ============================================================
void Strat_BossF_Init(Alien *self) {
    Alien *childA;

    if (!self) {
        return;
    }

    // s_set_aldata x,#hardHP,#hardAP
    set_hard_vars(self);
    // s_set_alptrs x,bossFC_strat,hitflash_Istrat,explode_Istrat
    self->stratptr = bossFC_strat;
    self->collstratptr = Strat_HitFlash;
    self->expstratptr = Strat_Explode;

    // s_make_childobj #boss_F_1,#1,bossFA_Istrat,enemy2
    childA = bossF_spawn_child(self, SH_BOSS_F_1, BOSSF_CHILD_A, bossFA_Istrat);
    if (childA) {
        // s_copy_pos y,x; s_add_alvar W,y,al_worldx,#400
        copy_pos(childA, self);
        childA->worldx = (int16)(childA->worldx + 400);
    }

    // s_set_colltype x,enemy2
    self->collflags |= ACF_COLLTYPE3;

    // s_set_state x,#0
    self->stratstate = 0u;
    // s_init_anim x,#0
    self->animframe = 0u;

    // s_set_alsflag x,nohitaffect
    self->sflags |= ASF_NOHITAFFECT;

    // s_set_bossmaxHP #0
    g_bossmaxhp = 0u;
    g_meters = 1u;
    g_gameflags &= (uint8)~GF_BOSSDEAD;
    g_bossflags &= (uint8)~(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3);

    // s_set_alvar B,x,al_sbyte2,#200
    self->sbyte2 = 200u;
    // s_set_altype x,gnd
    self->type |= ATGND;
}

// ============================================================
// bossFC_strat — Phase 1 controller (GB2STRAT.ASM:70)
// Countdown sbyte2: at 200-60=140 spawn child B.
// At sbyte2==0 transition through states: rise, pause, turn.
// When turn complete → bossFC2start_init.
// ============================================================
static void bossFC_strat(Alien *self) {
    if (!self) {
        return;
    }

    // Spawn child B when sbyte2 drops to 140 (200-60)
    if (self->sbyte2 == (uint8)(200u - 60u)) {
        Alien *childB = bossF_spawn_child(self, SH_BOSS_F_2, BOSSF_CHILD_B, bossFB_Istrat);
        if (childB) {
            copy_pos(childB, self);
            childB->worldx = (int16)(childB->worldx - 400);
        }
    }

    // Decrement countdown
    if (self->sbyte2 > 0u) {
        self->sbyte2--;
        if (self->sbyte2 > 0u) {
            goto do_states;
        }
    }
    // When countdown reaches 0, set to 1 (non-zero sentinel)
    self->sbyte2 = 1u;

do_states:
    // State 0: Rise (vy=-10 until below viewcy+300)
    if (self->stratstate == 0u) {
        self->vy = -10;
        if (self->worldy < (int16)(BOSSF_SPACE_VIEWCY + 300)) {
            self->stratstate = 1u;
            self->count = 4u; // next_state delay
        }
        goto movement;
    }

    // State 1: Slow to vy=0
    if (self->stratstate == 1u) {
        self->vy = (int16)(self->vy + 1);
        if (self->vy == 0) {
            self->stratstate = 2u;
            self->count = 4u;
            self->sbyte3 = 100u;
        }
        goto movement;
    }

    // State 2: Chase roty toward 0, count down sbyte3
    if (self->stratstate == 2u) {
        bool reached = achase_angle(&self->roty, 0u, 4);
        if (reached) {
            // Sound when rotation reaches target
            if ((self->sflags2 & ASF2_SFLAG1) == 0u) {
                // First time reaching — no sflag2 check needed for initial
            }
        } else {
            if ((self->sflags2 & ASF2_SFLAG1) == 0u) {
                self->sflags2 |= ASF2_SFLAG1;
                Sound_PlaySE(0x8Eu);
            }
        }
        // s_set_alsflag x,sflag1 — marks "combined" mode for children
        self->sflags2 |= ASF2_SFLAG1;

        // Decrement sbyte3; when 0, transition to phase 2
        if (self->sbyte3 > 0u) {
            self->sbyte3--;
        }
        if (self->sbyte3 == 0u) {
            bossFC2start_init(self);
            return;
        }
        goto movement;
    }

movement:
    // s_add_alvars B,x,al_roty,x,al_vy — add vy to roty (rotation animation)
    self->roty = (uint8)(self->roty + (uint8)(int8)self->vy);

    // s_add_vecs2pos x
    Strat_ApplyVelocity(self);
    // s_add_playerZ x
    add_player_z(self);
}

// ============================================================
// bossFC2start_init — Phase 2 transition (GB2STRAT.ASM:117)
// Remove wing children, change shape, spawn 6 turrets.
// ============================================================
static void bossFC2start_init(Alien *self) {
    if (!self) {
        return;
    }

    // s_clr_altype x,gnd
    self->type &= (uint8)~ATGND;

    // Remove children 1 and 2 (wing A and B)
    bossF_remove_child(self, BOSSF_CHILD_A);
    bossF_remove_child(self, BOSSF_CHILD_B);

    // s_set_alvar W,x,al_shape,#boss_f_4
    self->shape = SH_BOSS_F_4;

    // Spawn 6 turrets with position offsets (scaled by bossF_scale=3)
    (void)bossF_spawn_child_pos(self, SH_BOSS_F_8, BOSSF_CHILD_TUR1,
        (int16)(-20 << BOSSF_SCALE), (int16)(-10 << BOSSF_SCALE), (int16)(30 << BOSSF_SCALE),
        bossFtur1_Istrat);
    (void)bossF_spawn_child_pos(self, SH_BOSS_F_8, BOSSF_CHILD_TUR2,
        (int16)(-20 << BOSSF_SCALE), (int16)(-10 << BOSSF_SCALE), (int16)(-10 << BOSSF_SCALE),
        bossFtur2_Istrat);
    (void)bossF_spawn_child_pos(self, SH_BOSS_F_9, BOSSF_CHILD_TUR3,
        (int16)(20 << BOSSF_SCALE), (int16)(-10 << BOSSF_SCALE), (int16)(30 << BOSSF_SCALE),
        bossFtur3_Istrat);
    (void)bossF_spawn_child_pos(self, SH_BOSS_F_9, BOSSF_CHILD_TUR4,
        (int16)(20 << BOSSF_SCALE), (int16)(-10 << BOSSF_SCALE), (int16)(-10 << BOSSF_SCALE),
        bossFtur4_Istrat);
    (void)bossF_spawn_child_pos(self, SH_BOSS_F_8, BOSSF_CHILD_TUR5,
        (int16)(-20 << BOSSF_SCALE), (int16)(-10 << BOSSF_SCALE), (int16)(10 << BOSSF_SCALE),
        bossFtur5_Istrat);
    (void)bossF_spawn_child_pos(self, SH_BOSS_F_9, BOSSF_CHILD_TUR6,
        (int16)(20 << BOSSF_SCALE), (int16)(-10 << BOSSF_SCALE), (int16)(10 << BOSSF_SCALE),
        bossFtur6_Istrat);

    // s_set_alvar B,x,al_sbyte2,#0
    self->sbyte2 = 0u;

    // s_set_altype x,gnd
    self->type |= ATGND;
    // s_set_colltype x,enemyweap
    self->collflags |= ACF_COLLTYPE4;

    // s_setnoremove_behind x
    self->type &= (uint8)~ATZREMOVE;

    // s_set_alvar B,x,al_roty,#deg180
    self->roty = DEG180;

    // Fall through to bossFC2b_init
    bossFC2b_init(self);
}

// ============================================================
// bossFC2_init — Enter phase 2 from phase 3 (GB2STRAT.ASM:143)
// ============================================================
static void bossFC2_init(Alien *self) {
    if (!self) {
        return;
    }
    // s_set_alvar B,x,al_rotz,#-deg90
    self->rotz = (uint8)(-(int8)DEG90);
    bossFC2b_init(self);
}

// ============================================================
// bossFC2b_init — Common phase 2 entry (GB2STRAT.ASM:145)
// ============================================================
static void bossFC2b_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = bossFC2_strat;
    // s_clr_alsflag x,sflag1
    self->sflags2 &= (uint8)~ASF2_SFLAG1;
    // s_clr_alsflag x,sflag2 (using sflags4 bit)
    self->sflags4 &= (uint8)~ASF4_SFLAG8;
}

// ============================================================
// bossFC2_strat — Phase 2 per-frame (GB2STRAT.ASM:149)
// Chase roty toward deg180, tilt rotz toward 0.
// When player behind and in range → turn player + set Y.
// Fall through to bossFC2_cont for firing and movement.
// ============================================================
static void bossFC2_strat(Alien *self) {
    Alien *player;
    int16 zdist;

    if (!self) {
        return;
    }

    // s_achase_alvar B,x,al_roty,#deg180,5
    {
        bool reached = achase_angle(&self->roty, DEG180, 5);
        if (reached) {
            if ((self->sflags4 & ASF4_SFLAG8) == 0u) {
                self->sflags4 |= ASF4_SFLAG8;
                Sound_PlaySE(0x58u);
            }
        }
    }

    // s_jmp_notdelay 1 — every other frame
    if ((g_gameframe & 1u) == 0u) {
        // Chase rotz toward 0
        (void)achase_angle(&self->rotz, 0u, 6);
        // Chase rotx toward deg5
        if (self->rotx != DEG5) {
            self->rotx = (uint8)(self->rotx + 1u);
        }
    }

    // Check if player is behind us and in Z range
    player = Obj_GetPlayer();
    if (player && player->active) {
        // s_jmp_objinfront x,y — if player is in front, skip turn
        // Object is "in front" if their Z > our Z (facing deg180 = toward negative Z)
        bool player_in_front = (player->worldz > self->worldz);
        if (!player_in_front) {
            zdist = (int16)abs(self->worldz - player->worldz);
            if (zdist > 4000) {
                // Too far behind → switch to phase 3
                bossFC3_init(self);
                return;
            }
            if (zdist >= 2000) {
                // In range for player turn
                if ((self->sflags2 & ASF2_SFLAG1) == 0u) {
                    self->sflags2 |= ASF2_SFLAG1;
                    self->rotx = 0u;
                    // s_jmp_varAND B,pshipflags2,#psf2_playerHP0,.npturn
                    if ((g_pshipflags2 & PSF2_PLAYERHP0) == 0u) {
                        // s_set_strat y,playerturn180_Istrat — not ported, skip
                    }
                    // s_set_alvar W,x,al_worldy,#space_viewCY-500
                    self->worldy = (int16)(BOSSF_SPACE_VIEWCY - 500);
                }
            }
        }
    }

    bossFC2_cont(self);
}

// ============================================================
// bossFC2_cont — Check turret deaths, smoke, fire (GB2STRAT.ASM:183)
// ============================================================
static void bossFC2_cont(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    // s_jmp_alvarEQ B,x,al_sbyte2,#6 → all turrets dead
    if (self->sbyte2 >= 6u) {
        bossFCdie_init(self);
        return;
    }

    // s_jmp_alvarLESS B,x,al_sbyte2,#3 → not enough dead for smoke
    if (self->sbyte2 >= 3u) {
        bossFCsmoke_srou(self);
    }

    // Weapon firing
    player = Obj_GetPlayer();
    if (player && player->active) {
        int16 zdist = (int16)abs(self->worldz - player->worldz);
        if (zdist >= 600) {
            // Fire on specific frame masks
            if ((g_gameframe & 31u) == 10u) {
                int16 wpx = (int16)(((-20) >> WEAPON_SCALE) << BOSSF_SCALE);
                int16 wpy = (int16)(((-20) >> WEAPON_SCALE) << BOSSF_SCALE);
                bossF_fire_hplasma(self, wpx, wpy);
            } else if ((g_gameframe & 31u) == 5u) {
                int16 wpx = (int16)((20 << BOSSF_SCALE) >> WEAPON_SCALE);
                int16 wpy = (int16)(((-20) << BOSSF_SCALE) >> WEAPON_SCALE);
                bossF_fire_hplasma(self, wpx, wpy);
            }
        }
    }

    bossFC2_cont2(self);
}

// ============================================================
// bossFC2_cont2 — Speed up and generate movement (GB2STRAT.ASM:204)
// ============================================================
static void bossFC2_cont2(Alien *self) {
    if (!self) {
        return;
    }

    // s_speedto x,#50,1
    (void)Strat_SpeedTo(self, 50u, 1u);

    bossFC2_cont3(self);
}

// ============================================================
// bossFC2_cont3 — Generate 3D vecs and apply (GB2STRAT.ASM:207)
// ============================================================
static void bossFC2_cont3(Alien *self) {
    if (!self) {
        return;
    }

    // s_gen_3dvecs x,al_roty,al_rotx,al_vel
    Strat_GenVecs3D(self);
    // s_set_alvar W,x,al_vx,#0
    self->vx = 0;
    // s_add_vecs2pos x
    Strat_ApplyVelocity(self);
    // s_add_playerZ x
    add_player_z(self);
}

// ============================================================
// bossFC3_init — Phase 3 entry (GB2STRAT.ASM:224)
// ============================================================
static void bossFC3_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = bossFC3_strat;
    self->sflags2 &= (uint8)~ASF2_SFLAG1;
    self->sflags4 &= (uint8)~ASF4_SFLAG8;
    self->rotz = (uint8)(-(int8)DEG90);
}

// ============================================================
// bossFC3_strat — Phase 3 per-frame (GB2STRAT.ASM:230)
// Chase roty toward 0 (facing forward), same structure as FC2.
// ============================================================
static void bossFC3_strat(Alien *self) {
    Alien *player;
    int16 zdist;

    if (!self) {
        return;
    }

    // Chase roty toward 0
    {
        bool reached = achase_angle(&self->roty, 0u, 5);
        if (reached) {
            if ((self->sflags4 & ASF4_SFLAG8) == 0u) {
                self->sflags4 |= ASF4_SFLAG8;
                Sound_PlaySE(0x58u);
            }
        }
    }

    // Every other frame: chase rotz toward 0, advance rotx
    if ((g_gameframe & 1u) == 0u) {
        (void)achase_angle(&self->rotz, 0u, 6);
        if (self->rotx != DEG5) {
            self->rotx = (uint8)(self->rotx + 1u);
        }
    }

    // Check if player is behind (in front of boss facing 0)
    player = Obj_GetPlayer();
    if (player && player->active) {
        bool player_in_front = (player->worldz < self->worldz);
        if (!player_in_front) {
            zdist = (int16)abs(self->worldz - player->worldz);
            if (zdist > 4000) {
                bossFC2_init(self);
                return;
            }
            if (zdist >= 2000) {
                if ((self->sflags2 & ASF2_SFLAG1) == 0u) {
                    self->sflags2 |= ASF2_SFLAG1;
                    self->rotx = 0u;
                    // playerturn180_Istrat not yet ported
                    self->worldy = (int16)(BOSSF_SPACE_VIEWCY - 500);
                }
            }
        }
    }

    // s_brl bossFC2_cont
    bossFC2_cont(self);
}

// ============================================================
// bossFCdie_init — Begin death sequence (GB2STRAT.ASM:265)
// ============================================================
static void bossFCdie_init(Alien *self) {
    if (!self) {
        return;
    }
    self->stratptr = bossFCdie_strat;
}

// ============================================================
// bossFCdie_strat — Dying phase 1 (GB2STRAT.ASM:267)
// Tilt toward level, create explosions, chase player.
// When player close enough → bossFCdie2_init.
// ============================================================
static void bossFCdie_strat(Alien *self) {
    Alien *player;
    int16 zdist;

    if (!self) {
        return;
    }

    // Every other frame: chase rotz toward 0
    if ((g_gameframe & 1u) == 0u) {
        (void)achase_angle(&self->rotz, 0u, 6);
    }

    // Every other frame: create medium explosion
    if ((g_gameframe & 1u) == 0u) {
        Alien *exp = make_medium_exp_obj(self);
        if (exp) {
            addrnd2pos_xy(exp);
        }
    }

    player = Obj_GetPlayer();
    if (player && player->active) {
        zdist = (int16)abs(self->worldz - player->worldz);
        if (zdist < 1000) {
            bossFCdie2_init(self);
            return;
        }
    }

    bossFCsmoke_srou(self);
    bossFC2_cont2(self);
}

// ============================================================
// bossFCdie2_init — Final death sequence entry (GB2STRAT.ASM:284)
// ============================================================
static void bossFCdie2_init(Alien *self) {
    Alien *player;

    if (!self) {
        return;
    }

    player = Obj_GetPlayer();
    if (player && player->active) {
        // s_set_strat y,playerturn180_Istrat — not yet ported
    }

    self->stratptr = bossFCdie2_strat;
    self->rotx = 0u;
    // s_playerfire off
    g_stratflags |= SF_NOFIRING;
    self->sflags4 &= (uint8)~ASF4_SFLAG8;
}

// ============================================================
// bossFCdie2_strat — Final death per-frame (GB2STRAT.ASM:291)
// Large explosions, roll, eventually remove and set bossdead.
// ============================================================
static void bossFCdie2_strat(Alien *self) {
    Alien *player;
    Alien *exp;

    if (!self) {
        return;
    }

    // Chase rotz toward 0 every other frame
    if ((g_gameframe & 1u) == 0u) {
        (void)achase_angle(&self->rotz, 0u, 6);
    }

    // Create large explosion every frame
    exp = make_large_exp_obj(self);
    if (exp) {
        exp->sflags4 |= ASF4_NOPOLYEXP;
        // s_add_rnd2pos y,255,255,255,1,0,0
        {
            int8 rx = (int8)(SfRtl_Random() & 0xFF);
            int8 ry = (int8)(SfRtl_Random() & 0xFF);
            int8 rz = (int8)(SfRtl_Random() & 0xFF);
            exp->worldx = (int16)(exp->worldx + (int16)rx);
            exp->worldy = (int16)(exp->worldy + (int16)ry);
            exp->worldz = (int16)(exp->worldz + (int16)rz);
        }
    }

    // Check distance to player
    player = Obj_GetPlayer();
    if (player && player->active) {
        int16 zdist = (int16)abs(self->worldz - player->worldz);
        if (zdist >= 4000) {
            // Far enough — create FOL explosion + random offset
            Alien *fexp;

            if ((g_gameframe & 1u) == 0u) {
                fexp = make_fol_exp_obj(self);
                if (fexp) {
                    fexp->sflags4 |= ASF4_NOPOLYEXP;
                    addrnd2pos_xy(fexp);
                }
            }

            // Sound + BGM on first frame of final phase
            if ((self->sflags4 & ASF4_SFLAG8) == 0u) {
                self->sflags4 |= ASF4_SFLAG8;
                Sound_PlayMusic(0xF0u);
                Sound_PlaySE(0x96u);
            }

            // s_add_alvar B,x,al_rotz,#4
            self->rotz = (uint8)(self->rotz + 4u);

            // s_jmp_alvarEQ B,x,al_rotx,#deg90
            if (self->rotx == DEG90) {
                // Reset world tracking variables
                g_lastplayz = self->worldz;
                g_lastzchange = 0;
                g_mapcnt = 0u;

                // Handle player 180 turn
                if ((g_pshipflags2 & PSF2_TURN180) != 0u) {
                    g_pshipflags2 &= (uint8)~PSF2_TURN180;
                    // playerbossFdie_strat not fully ported;
                    // just reverse player
                    player = Obj_GetPlayer();
                    if (player && player->active) {
                        player->vz = (int16)(-player->vz);
                        player->worldx = (int16)(-player->worldx);
                        g_pviewvelz = (int16)(-g_pviewvelz);
                        g_player_turnrot = 0;
                    }
                }

                // s_or_var B,gameflags,#gf_bossdead
                g_gameflags |= GF_BOSSDEAD;
                g_bossflags &= (uint8)~(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3);
                g_bossmaxhp = 0u;
                g_stratflags &= (uint8)~SF_NOFIRING;

                // s_remove_obj x
                g_aldead = 1u;
                return;
            }

            // s_speedto x,#60,1
            (void)Strat_SpeedTo(self, 60u, 1u);
            // s_add_alvar B,x,al_rotx,#1
            self->rotx = (uint8)(self->rotx + 1u);

            bossFCsmoke_srou(self);
            bossFC2_cont3(self);
            return;
        }
    }

    // .ndown — close to player, create smoke + sound
    if ((g_gameframe % 4u) == 0u) {
        Sound_PlaySE(0x21u);
    }

    bossFCsmoke_srou(self);
    bossFC2_cont3(self);
}

// ============================================================
// bossFtur1-6 Istrat — Turret initialization (GB2STRAT.ASM:365-412)
// Each turret has its own HP, fire delay (sbyte3), open/close
// cycle timing (sbyte2), and explosion strategy.
// ============================================================
static void bossFtur1_Istrat(Alien *self) {
    if (!self) { return; }
    self->stratptr = bossFtur1_strat;
    self->sbyte3 = 25u;
    self->HP = BOSSF_LAUNCHER2_HP;
    self->AP = BOSSF_LAUNCHER_AP;
    self->expstratptr = bossfexp1_Istrat;
    self->sbyte2 = 30u;
    bossFtur_Icont(self);
}

static void bossFtur2_Istrat(Alien *self) {
    if (!self) { return; }
    self->stratptr = bossFtur2_strat;
    self->sbyte3 = 25u;
    self->HP = BOSSF_LAUNCHER_HP;
    self->AP = BOSSF_LAUNCHER_AP;
    self->expstratptr = bossfexp1_Istrat;
    self->sbyte2 = 60u;
    bossFtur_Icont(self);
}

static void bossFtur3_Istrat(Alien *self) {
    if (!self) { return; }
    self->stratptr = bossFtur3_strat;
    self->sbyte3 = 50u;
    self->HP = BOSSF_LAUNCHER2_HP;
    self->AP = BOSSF_LAUNCHER_AP;
    self->expstratptr = bossfexp2_Istrat;
    self->sbyte2 = 90u;
    bossFtur_Icont(self);
}

static void bossFtur4_Istrat(Alien *self) {
    if (!self) { return; }
    self->stratptr = bossFtur4_strat;
    self->sbyte3 = 50u;
    self->HP = BOSSF_LAUNCHER_HP;
    self->AP = BOSSF_LAUNCHER_AP;
    self->expstratptr = bossfexp2_Istrat;
    self->sbyte2 = 120u;
    bossFtur_Icont(self);
}

static void bossFtur5_Istrat(Alien *self) {
    if (!self) { return; }
    self->stratptr = bossFtur5_strat;
    self->sbyte3 = 25u;
    self->HP = BOSSF_LAUNCHER_HP;
    self->AP = BOSSF_LAUNCHER_AP;
    self->expstratptr = bossfexp1_Istrat;
    self->sbyte2 = 150u;
    bossFtur_Icont(self);
}

static void bossFtur6_Istrat(Alien *self) {
    if (!self) { return; }
    self->stratptr = bossFtur6_strat;
    self->sbyte3 = 50u;
    self->HP = BOSSF_LAUNCHER_HP;
    self->AP = BOSSF_LAUNCHER_AP;
    self->expstratptr = bossfexp2_Istrat;
    self->sbyte2 = 180u;
    bossFtur_Icont(self);
}

// ============================================================
// bossFtur_Icont — Common turret init (GB2STRAT.ASM:406)
// ============================================================
static void bossFtur_Icont(Alien *self) {
    Alien *mother;

    if (!self) {
        return;
    }

    self->animframe = 0u;
    // s_setnoremove_behind
    self->type &= (uint8)~ATZREMOVE;
    // s_set_colltype x,enemyweap
    self->collflags |= ACF_COLLTYPE4;
    // s_set_collstrat x,hitflash_Istrat
    self->collstratptr = Strat_HitFlash;
    // s_add_bossmaxHP x,al_hp
    mother = boss_get_mother_obj(self);
    if (mother) {
        g_bossmaxhp = (uint16)(g_bossmaxhp + (uint16)self->HP);
    }
}

// ============================================================
// bossFtur1-6 strat — Turret per-frame (GB2STRAT.ASM:413-441)
// Each positions itself relative to mother using rotated offsets.
// ============================================================
static void bossFtur1_strat(Alien *self) {
    Alien *mother;
    if (!self) { return; }
    mother = boss_get_mother_obj(self);
    if (!mother) { g_aldead = 1u; return; }
    bossF_rotated_offset(self, mother, -20, -10, 30, BOSSF_SCALE);
    bossFtur_cont(self);
}

static void bossFtur2_strat(Alien *self) {
    Alien *mother;
    if (!self) { return; }
    mother = boss_get_mother_obj(self);
    if (!mother) { g_aldead = 1u; return; }
    bossF_rotated_offset(self, mother, -20, -10, -10, BOSSF_SCALE);
    bossFtur_cont(self);
}

static void bossFtur3_strat(Alien *self) {
    Alien *mother;
    if (!self) { return; }
    mother = boss_get_mother_obj(self);
    if (!mother) { g_aldead = 1u; return; }
    bossF_rotated_offset(self, mother, 20, -10, 30, BOSSF_SCALE);
    bossFtur_cont(self);
}

static void bossFtur4_strat(Alien *self) {
    Alien *mother;
    if (!self) { return; }
    mother = boss_get_mother_obj(self);
    if (!mother) { g_aldead = 1u; return; }
    bossF_rotated_offset(self, mother, 20, -10, -10, BOSSF_SCALE);
    bossFtur_cont(self);
}

static void bossFtur5_strat(Alien *self) {
    Alien *mother;
    if (!self) { return; }
    mother = boss_get_mother_obj(self);
    if (!mother) { g_aldead = 1u; return; }
    bossF_rotated_offset(self, mother, -20, -10, 10, BOSSF_SCALE);
    bossFtur_cont(self);
}

static void bossFtur6_strat(Alien *self) {
    Alien *mother;
    if (!self) { return; }
    mother = boss_get_mother_obj(self);
    if (!mother) { g_aldead = 1u; return; }
    bossF_rotated_offset(self, mother, 20, -10, 10, BOSSF_SCALE);
    bossFtur_cont(self);
}

// ============================================================
// bossFtur_cont — Common turret logic (GB2STRAT.ASM:442-488)
// If mother gone → remove. Copy rotations from mother.
// Check HP (dead turrets skip combat). Add boss HP.
// Open/close cycle with animation, fire when open.
// ============================================================
static void bossFtur_cont(Alien *self) {
    Alien *mother;
    Alien *player;

    if (!self) {
        return;
    }

    mother = boss_get_mother_obj(self);
    if (!mother || !mother->active) {
        g_aldead = 1u;
        return;
    }

    // Copy rotations from mother
    self->rotx = mother->rotx;
    self->roty = mother->roty;
    self->rotz = mother->rotz;

    // If dead (HP == hardHP / 0xFF), skip combat
    if (self->HP == HARD_HP) {
        return;
    }

    // s_add_bossHP x,al_hp — contribute to boss health bar
    // (The renderer reads accumulated HP each frame — this is implicit)

    // Decrement open/close timer (sbyte2)
    if (self->sbyte2 > 0u) {
        self->sbyte2--;
    }
    if (self->sbyte2 == 0u) {
        // Toggle sflag1 (open/close state) via sflags2
        self->sflags2 ^= ASF2_SFLAG1;
        if ((self->sflags2 & ASF2_SFLAG1) != 0u) {
            // Opening: set open time
            self->sbyte2 = 40u;
        } else {
            // Closing: set closed time
            self->sbyte2 = 70u;
        }
    }

    // Closed state: set nohitaffect, animate closing
    if ((self->sflags2 & ASF2_SFLAG1) == 0u) {
        self->sflags |= ASF_NOHITAFFECT;
        // Animate closing: frame toward 0
        if ((g_gameframe & 1u) == 0u) {
            if (self->animframe > 0u) {
                self->animframe--;
            }
        }
        return;
    }

    // Open state: clear nohitaffect, animate opening
    self->sflags &= (uint8)~ASF_NOHITAFFECT;
    if ((g_gameframe & 1u) == 0u) {
        if (self->animframe < 3u) {
            self->animframe++;
        }
    }

    // Fire countdown (sbyte3)
    if (self->sbyte3 > 0u) {
        self->sbyte3--;
    }
    if (self->sbyte3 == 0u) {
        self->sbyte3 = 50u;
    }

    // Fire when conditions met: sbyte3 <= 20, sbyte2 > 15, player far enough
    if (self->sbyte3 > 20u) {
        return;
    }
    if (self->sbyte2 <= 15u) {
        return;
    }

    player = Obj_GetPlayer();
    if (!player || !player->active) {
        return;
    }

    {
        int16 zdist = (int16)abs(self->worldz - player->worldz);
        if (zdist < 1500) {
            return;
        }
    }

    // Fire every 3rd frame
    if ((g_gameframe % 4u) != 0u) {
        return;
    }

    // s_weapon_pos #0,#0,#0; s_weapon_rots2obj y; s_fire_weapon x,RELSLOWELASER
    {
        uint8 yaw = Strat_AngleXZ(self, player);
        uint8 pitch = strat_pitch_toward(self, player);

        (void)Strat_SpawnProjectile(self, 0, 0, 0, pitch, yaw,
                                     strat_relslowelaser_speed(),
                                     RELSLOWELASERHOME_LIFE,
                                     RELSLOWELASERHOME_AP,
                                     ACF_COLLTYPE4);
    }
}

// ============================================================
// bossfexp1_Istrat — Turret explosion type 1 (GB2STRAT.ASM:490)
// Change shape to boss_f_8a, increment mother's death count.
// ============================================================
static void bossfexp1_Istrat(Alien *self) {
    Alien *mother;

    if (!self) {
        return;
    }

    self->shape = SH_BOSS_F_8A;

    mother = boss_get_mother_obj(self);
    if (mother) {
        mother->sbyte2 = (uint8)(mother->sbyte2 + 1u);
    }

    // Create medium explosion at position
    (void)make_medium_exp_obj(self);

    // Mark as dead (hardHP) and disable collision
    self->HP = HARD_HP;
    self->sflags |= ASF_COLLDISABLE;
}

// ============================================================
// bossfexp2_Istrat — Turret explosion type 2 (GB2STRAT.ASM:494)
// Same as exp1 but with boss_f_9a shape.
// ============================================================
static void bossfexp2_Istrat(Alien *self) {
    Alien *mother;

    if (!self) {
        return;
    }

    self->shape = SH_BOSS_F_9A;

    mother = boss_get_mother_obj(self);
    if (mother) {
        mother->sbyte2 = (uint8)(mother->sbyte2 + 1u);
    }

    (void)make_medium_exp_obj(self);

    self->HP = HARD_HP;
    self->sflags |= ASF_COLLDISABLE;
}

// ============================================================
// bossFA_Istrat — Right wing child init (GB2STRAT.ASM:506)
// ============================================================
static void bossFA_Istrat(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = bossFA_strat;
    // s_set_alvar B,x,al_rotx,#-deg90
    self->rotx = (uint8)(-(int8)DEG90);
    // s_set_speed x,#50
    self->vel = 50u;
    self->stratstate = 0u;
    self->animframe = 0u;
}

// ============================================================
// bossFA_strat — Right wing per-frame (GB2STRAT.ASM:513)
// ============================================================
static void bossFA_strat(Alien *self) {
    Alien *mother;

    if (!self) {
        return;
    }

    mother = boss_get_mother_obj(self);
    if (!mother || !mother->active) {
        g_aldead = 1u;
        return;
    }

    // Check if mother has sflag1 set (combined mode)
    if ((mother->sflags2 & ASF2_SFLAG1) != 0u) {
        // Chase toward mother position + offset
        int16 targZ = (int16)(mother->worldz + (10 << BOSSF_SCALE));
        int16 targY = (int16)(mother->worldy + (-22 << BOSSF_SCALE));

        self->worldz = (int16)(self->worldz + ((targZ - self->worldz) >> 2));
        self->worldx = (int16)(self->worldx + ((mother->worldx - self->worldx) >> 2));
        self->worldy = (int16)(self->worldy + ((targY - self->worldy) >> 2));
        (void)achase_angle(&self->roty, DEG180, 3);
        (void)achase_angle(&self->rotz, 0u, 3);
        add_player_z(self);
        return;
    }

    // State 0: fly upward until below viewcy+800
    if (self->stratstate == 0u) {
        if (self->worldy < (int16)(BOSSF_SPACE_VIEWCY + 800)) {
            self->stratstate = 1u;
            self->count = 4u;
        }
    }

    // State 1: level off, spin
    if (self->stratstate == 1u) {
        (void)achase_angle(&self->rotx, 0u, 4);
        self->roty = (uint8)(self->roty + 4u);
        self->rotz = (uint8)(self->rotz + 2u);
    }

    // Fire when pointing in negative Z direction
    if (strat_points_positive_z(self)) {
        // Not pointing toward player → skip fire
    } else {
        if ((g_gameframe % 4u) == 0u) {
            Alien *player = Obj_GetPlayer();
            if (player && player->active) {
                uint8 yaw = Strat_AngleXZ(self, player);
                uint8 pitch = strat_pitch_toward(self, player);

                // Fire left weapon
                (void)Strat_SpawnProjectile(self,
                    (int16)((-20 << BOSSF_SCALE) >> WEAPON_SCALE),
                    (int16)((-20 << BOSSF_SCALE) >> WEAPON_SCALE), 0,
                    pitch, yaw, strat_relslowelaser_speed(),
                    RELSLOWELASERHOME_LIFE, RELSLOWELASERHOME_AP,
                    ACF_COLLTYPE4);

                // Fire right weapon
                (void)Strat_SpawnProjectile(self,
                    (int16)((20 << BOSSF_SCALE) >> WEAPON_SCALE),
                    (int16)((-20 << BOSSF_SCALE) >> WEAPON_SCALE), 0,
                    pitch, yaw, strat_relslowelaser_speed(),
                    RELSLOWELASERHOME_LIFE, RELSLOWELASERHOME_AP,
                    ACF_COLLTYPE4);
            }
        }
    }

    // s_gen_3dvecs, scale vz by 1, apply
    Strat_GenVecs3D(self);
    self->vz = (int16)(self->vz >> 1);
    Strat_ApplyVelocity(self);
    add_player_z(self);
}

// ============================================================
// bossFB_Istrat — Left wing child init (GB2STRAT.ASM:565)
// ============================================================
static void bossFB_Istrat(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = bossFB_strat;
    self->rotx = (uint8)(-(int8)DEG90);
    self->vel = 30u;
    self->stratstate = 0u;
    self->animframe = 0u;
}

// ============================================================
// bossFB_strat — Left wing per-frame (GB2STRAT.ASM:572)
// ============================================================
static void bossFB_strat(Alien *self) {
    Alien *mother;
    Alien *player;

    if (!self) {
        return;
    }

    mother = boss_get_mother_obj(self);
    if (!mother || !mother->active) {
        g_aldead = 1u;
        return;
    }

    // Check combined mode
    if ((mother->sflags2 & ASF2_SFLAG1) != 0u) {
        int16 targZ = (int16)(mother->worldz + (-40 << BOSSF_SCALE));

        self->worldz = (int16)(self->worldz + ((targZ - self->worldz) >> 2));
        self->worldx = (int16)(self->worldx + ((mother->worldx - self->worldx) >> 2));
        self->worldy = (int16)(self->worldy + ((mother->worldy - self->worldy) >> 2));
        (void)achase_angle(&self->roty, DEG180, 3);
        (void)achase_angle(&self->rotz, 0u, 3);
        add_player_z(self);
        return;
    }

    // State 0: fly upward until below viewcy+600
    if (self->stratstate == 0u) {
        if (self->worldy < (int16)(BOSSF_SPACE_VIEWCY + 600)) {
            self->stratstate = 1u;
            self->count = 4u;
        }
    }

    // State 1: level off, counter-spin
    if (self->stratstate == 1u) {
        (void)achase_angle(&self->rotx, 0u, 4);
        self->roty = (uint8)(self->roty - 2u);
        self->rotz = (uint8)(self->rotz - 1u);
    }

    // Distance-based sound
    player = Obj_GetPlayer();
    if (player && player->active) {
        int16 zdist = (int16)abs(self->worldz - player->worldz);
        if (zdist < 3000) {
            if ((self->sflags4 & ASF4_SFLAG8) == 0u) {
                Sound_PlaySE(0x58u);
                self->sflags4 |= ASF4_SFLAG8;
            }
        } else {
            self->sflags4 &= (uint8)~ASF4_SFLAG8;
        }

        // Drop mines when close enough
        if (zdist < 1400) {
            if ((g_gameframe % 4u) == 0u) {
                // mine_0 not ported — spawn a simple projectile instead
                Alien *mine = Strat_MakeObj(0u);
                if (mine) {
                    copy_pos(mine, self);
                    mine->HP = 2u;
                    mine->AP = 10u;
                    mine->count = 60u;
                    mine->collflags = ACF_COLLTYPE4;
                    mine->sflags |= ASF_COLLDISABLE;
                    mine->stratptr = NULL; // Static mine placeholder
                }
            }
        }
    }

    // Movement
    Strat_GenVecs3D(self);
    self->vz = (int16)(self->vz >> 1);
    Strat_ApplyVelocity(self);
    add_player_z(self);
}

// ===================================================================
// Title screen spinning Arwing (TITLE.ASM tit_istrat)
// ===================================================================

static void strat_title_tick(Alien *self) {
    // Increment Y rotation each tick for a slow spin
    self->roty = (uint8)((self->roty + 1) & 0xFF);
}

void Strat_Title_Init(Alien *self) {
    self->HP = HARD_HP;
    self->AP = 0;
    self->collflags = 0;
    self->roty = 0;
    self->stratptr = strat_title_tick;
}
