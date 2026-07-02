// STRATROU.ASM → C conversion
// Common strategy routines used by all object behaviors.
//
// Decompiled from STRATROU.ASM: velocity generation (alvelvecs, alvel3vecs),
// position updates (addvecs, addalvecs), chase functions (Fchase, speedto),
// percentage scaling (perc75/62/87), distance/angle helpers, and object
// management (makeobj, removeobj).

#include "strat_common.h"
#include "../game/game_vars.h"
#include "../game/sound.h"
#include "../sf_rtl.h"
#include "../variables.h"
#include <math.h>
#include <stdlib.h>

// SNES sin/cos table (256 entries, 0-255 angle → -127..+127 fixed-point)
// On SNES these were in ROM as signed bytes. We use floats for the HD build
// and convert to int16 where the ASM did.
static float s_sin[256];
static float s_cos[256];
static bool s_tables_init = false;

static void init_tables(void) {
    if (s_tables_init) return;
    for (int i = 0; i < 256; i++) {
        float rad = (float)i * (2.0f * 3.14159265f / 256.0f);
        s_sin[i] = sinf(rad);
        s_cos[i] = cosf(rad);
    }
    s_tables_init = true;
}

// --- Chase / Transition ---

int16 Strat_Chase(int16 current, int16 target, int16 rate) {
    // Decompiled from Fchase_A (STRATMAC.INC).
    if (current < target) {
        current += rate;
        if (current > target) current = target;
    } else if (current > target) {
        current -= rate;
        if (current < target) current = target;
    }
    return current;
}

uint8 Strat_Chase8(uint8 current, uint8 target, uint8 rate) {
    if (current < target) {
        current += rate;
        if (current > target) current = target;
    } else if (current > target) {
        if (current - target < rate) current = target;
        else current -= rate;
    }
    return current;
}

int16 Strat_ChaseProportional(int16 current, int16 target, int shift) {
    // Decompiled from Achase macros (STRATMAC.INC).
    // Step size = (current - target) >> shift
    if (current == target) return current;
    int16 diff = current - target;
    int16 step = diff >> shift;
    if (step == 0) step = (diff > 0) ? 1 : -1;
    return current - step;
}

bool Strat_SpeedTo(Alien *al, uint8 target_speed, uint8 rate) {
    // Decompiled from sr_speedto (STRATROU.ASM:2707-2733).
    if (al->vel == target_speed) return true;
    if (al->vel < target_speed) {
        al->vel += rate;
        if (al->vel > target_speed) al->vel = target_speed;
    } else {
        if (al->vel - target_speed < rate) al->vel = target_speed;
        else al->vel -= rate;
    }
    return (al->vel == target_speed);
}

// --- Percentage Scaling ---

int16 Strat_Perc75(int16 val) {
    // val * 3/4 = val/2 + val/4 (STRATROU.ASM:2512)
    return (val >> 1) + (val >> 2);
}

int16 Strat_Perc87(int16 val) {
    // val * 7/8 = val/2 + val/4 + val/8 (STRATROU.ASM:2519)
    return (val >> 1) + (val >> 2) + (val >> 3);
}

int16 Strat_Perc62(int16 val) {
    // val * 5/8 = val/2 + val/8 (STRATROU.ASM:2503)
    return (val >> 1) + (val >> 3);
}

int16 Strat_Perc56(int16 val) {
    // val * 9/16 = val/2 + val/16
    return (val >> 1) + (val >> 4);
}

int16 Strat_Perc93(int16 val) {
    // val * 15/16 = val - val/16
    return val - (val >> 4);
}

// --- Velocity Vector Generation ---

void Strat_GenVecs2D(Alien *al) {
    // Decompiled from alvelvecs_l (STRATROU.ASM:100-148).
    // 2D velocity in XZ plane from Y rotation and speed magnitude.
    init_tables();
    uint8 angle = al->roty;
    float speed = (float)(int16)al->vel;

    al->vx = (int16)(speed * s_sin[angle]);
    al->vy = 0;
    al->vz = (int16)(speed * s_cos[angle]);
}

void Strat_GenVecs3D(Alien *al) {
    // Decompiled from alvel3vecs_l (STRATROU.ASM:221-283).
    // 3D velocity from X rotation (pitch) and Y rotation (yaw).
    init_tables();
    uint8 yaw = al->roty;
    uint8 pitch = (uint8)(-(int8)al->rotx);  // Negate pitch (ASM convention)
    float speed = (float)(int16)al->vel;

    float cy = s_cos[pitch];
    float sy = s_sin[pitch];

    // X = speed * sin(yaw) * cos(pitch)
    al->vx = (int16)(speed * s_sin[yaw] * cy);
    // Y = speed * sin(pitch)
    al->vy = (int16)(speed * sy);
    // Z = speed * cos(yaw) * cos(pitch)
    al->vz = (int16)(speed * s_cos[yaw] * cy);
}

void Strat_GenSideVecs(Alien *al, int16 *out_x, int16 *out_z) {
    // Decompiled from sidevecs_l (STRATROU.ASM:370-418).
    // Sideways velocity (perpendicular to facing direction).
    init_tables();
    uint8 angle = (uint8)(64 - al->roty);  // Rotate 90 degrees
    float speed = (float)(int16)al->vel;

    *out_x = (int16)(speed * s_sin[angle]);
    *out_z = (int16)(speed * s_cos[angle]);
}

void Strat_GenFrontVecs(Alien *al, int16 *out_x, int16 *out_z) {
    // Decompiled from frontvecs_l (STRATROU.ASM:424-471).
    init_tables();
    uint8 angle = al->roty;
    float speed = (float)(int16)al->vel;

    *out_x = (int16)(speed * s_sin[angle]);
    *out_z = (int16)(speed * s_cos[angle]);
}

// --- Position Update ---

void Strat_ApplyVelocity(Alien *al) {
    // Decompiled from addalvecs_l (STRATROU.ASM:518-535).
    al->worldx += al->vx;
    al->worldy += al->vy;
    al->worldz += al->vz;
}

void Strat_AddToPos(Alien *al, int16 dx, int16 dy, int16 dz) {
    // Decompiled from addvecs_l (STRATROU.ASM:497).
    al->worldx += dx;
    al->worldy += dy;
    al->worldz += dz;
}

// --- Distance / Angle ---

int16 Strat_DistXZ(const Alien *a, const Alien *b) {
    // Decompiled from xzdiffs_l (STRATROU.ASM).
    // Approximate Manhattan distance in XZ plane.
    int16 dx = abs(b->worldx - a->worldx);
    int16 dz = abs(b->worldz - a->worldz);
    return dx + dz;
}

uint8 Strat_AngleXZ(const Alien *src, const Alien *dst) {
    // Decompiled from anglexy_l (STRATROU.ASM).
    // Returns 0-255 angle from src to dst in XZ plane.
    init_tables();
    float dx = (float)(dst->worldx - src->worldx);
    float dz = (float)(dst->worldz - src->worldz);
    float angle_rad = atan2f(dx, dz);
    if (angle_rad < 0) angle_rad += 2.0f * 3.14159265f;
    return (uint8)(angle_rad * (256.0f / (2.0f * 3.14159265f)));
}

// --- Object Management ---

void Strat_RemoveObj(void) {
    // Decompiled from s_remove_obj / s_doremove macros.
    // Sets the global death flag so dostrats removes the current alien.
    g_aldead = 1;
}

Alien *Strat_MakeObj(uint16 shape_id) {
    // Decompiled from s_make_obj macro + makeobj_l (STRATROU.ASM).
    Alien *al = Obj_Alloc();
    if (!al) return NULL;

    Strat_InitObjVars(al);
    al->shape = shape_id;
    return al;
}

void Strat_InitObjVars(Alien *al) {
    // Decompiled from init_objvars_l (STRATROU.ASM).
    // Zero-init the strategy fields. The allocation already zeroed the struct,
    // but this explicitly sets important defaults.
    al->sflags = 0;
    al->sflags2 = 0;
    al->HP = 0;
    al->AP = 0;
    al->vx = 0;
    al->vy = 0;
    al->vz = 0;
    al->count = 0;
    al->count1 = 0;
    al->animframe = 0xFF;  // Default: use gameframe
    al->colframe = 0xFF;
    al->collflags = ACF_FIRSTFRAME;
}

bool Strat_CountDown(Alien *al) {
    if (al->count > 0) {
        al->count--;
        return false;
    }
    return true;
}

static void projectile_mark_dead(Alien *self) {
    if ((self->sbyte6 & 1u) != 0u) {
        self->sbyte6 &= (uint8)~1u;
        if (g_numplasers > 0) {
            g_numplasers--;
        }
    }
    g_aldead = 1;
}

void Strat_ProjectileOnCollide(Alien *self) {
    projectile_mark_dead(self);
}

void Strat_ProjectileTick(Alien *self) {
    Strat_ApplyVelocity(self);

    if (self->count > 0) {
        self->count--;
    }
    if (self->count == 0) {
        projectile_mark_dead(self);
        return;
    }

    Alien *player = Obj_GetPlayer();
    if (player) {
        int16 dz = (int16)(self->worldz - player->worldz);
        if (dz < -12000 || dz > 12000) {
            projectile_mark_dead(self);
        }
    }
}

Alien *Strat_SpawnProjectile(const Alien *owner,
                             int16 off_x, int16 off_y, int16 off_z,
                             uint8 rot_x, uint8 rot_y,
                             uint8 speed, uint8 lifetime,
                             uint8 ap, uint8 coll_type_bit) {
    Alien *al = Obj_Alloc();
    if (!al) return NULL;

    Strat_InitObjVars(al);
    al->shape = 0;  // Visual weapon meshes are not wired yet.
    al->sflags |= ASF_INVISIBLE;
    al->type |= (ATLASER | ATZREMOVE);

    al->stratptr = Strat_ProjectileTick;
    al->collstratptr = Strat_ProjectileOnCollide;
    al->expstratptr = Strat_ProjectileOnCollide;

    al->count = lifetime ? lifetime : 40;
    al->HP = 1;
    al->AP = ap;
    al->vel = speed;
    al->rotx = rot_x;
    al->roty = rot_y;

    al->collflags = (uint8)(ACF_FIRSTFRAME | ACF_WEAPON | coll_type_bit);

    if (owner) {
        al->worldx = owner->worldx + off_x;
        al->worldy = owner->worldy + off_y;
        al->worldz = owner->worldz + off_z;
        if (owner >= g_aliens && owner < (g_aliens + NUMBER_AL)) {
            al->immuneptr = (uint16)(owner - g_aliens);
        }
    } else {
        al->worldx = off_x;
        al->worldy = off_y;
        al->worldz = off_z;
    }

    Strat_GenVecs3D(al);
    return al;
}

// --- Sound ---

void Strat_TrigSE(uint8 sound_id) {
    // Decompiled from trigse macro (MACROS.INC).
    Sound_PlaySE(sound_id);
}
