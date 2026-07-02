#ifndef STARFOX_STRAT_COMMON_H
#define STARFOX_STRAT_COMMON_H

#include "../game/obj.h"
#include "../types.h"

// --- Chase / Transition Helpers ---
// From STRATROU.ASM / STRATMAC.INC

// Chase a value toward target with fixed step rate.
// Returns the new value. Equivalent to SNES Fchase_A.
int16 Strat_Chase(int16 current, int16 target, int16 rate);
uint8 Strat_Chase8(uint8 current, uint8 target, uint8 rate);

// Chase with proportional rate (ASM Achase): step = diff >> shift
int16 Strat_ChaseProportional(int16 current, int16 target, int shift);

// Smooth speed transition (sr_speedto)
// Returns true when target is reached.
bool Strat_SpeedTo(Alien *al, uint8 target_speed, uint8 rate);

// --- Percentage Scaling ---
// From STRATROU.ASM perc* functions (bit-shift approximations)

int16 Strat_Perc75(int16 val);   // val * 3/4 = val/2 + val/4
int16 Strat_Perc87(int16 val);   // val * 7/8 = val/2 + val/4 + val/8
int16 Strat_Perc62(int16 val);   // val * 5/8 = val/2 + val/8
int16 Strat_Perc56(int16 val);   // val * 9/16 = val/2 + val/16
int16 Strat_Perc93(int16 val);   // val * 15/16

// --- Velocity Vector Generation ---
// From STRATROU.ASM alvelvecs/alvel3vecs/sidevecs/frontvecs

// 2D velocity from Y rotation + speed (alvelvecs_l)
// Sets al->vx, al->vz from roty and vel. vy = 0.
void Strat_GenVecs2D(Alien *al);

// 3D velocity from X,Y rotation + speed (alvel3vecs_l)
// Sets al->vx, al->vy, al->vz from rotx, roty, vel.
void Strat_GenVecs3D(Alien *al);

// Sideways velocity vector (perpendicular to facing)
void Strat_GenSideVecs(Alien *al, int16 *out_x, int16 *out_z);

// Forward velocity vector
void Strat_GenFrontVecs(Alien *al, int16 *out_x, int16 *out_z);

// --- Position Update ---

// Apply al->vx/vy/vz to al->worldx/y/z (addalvecs_l)
void Strat_ApplyVelocity(Alien *al);

// Apply external dx/dy/dz to alien position (addvecs_l)
void Strat_AddToPos(Alien *al, int16 dx, int16 dy, int16 dz);

// --- Distance / Angle Helpers ---

// Manhattan distance in XZ plane between two aliens
int16 Strat_DistXZ(const Alien *a, const Alien *b);

// Y-angle from alien a toward alien b (0-255 angle)
uint8 Strat_AngleXZ(const Alien *src, const Alien *dst);

// --- Object Management ---

// Mark current alien for death (sets g_aldead)
void Strat_RemoveObj(void);

// Create a new alien with given shape, returns NULL if pool exhausted
Alien *Strat_MakeObj(uint16 shape_id);

// Initialize default alien variables (init_objvars_l)
void Strat_InitObjVars(Alien *al);

// Countdown timer — decrements al->count, returns true when it reaches 0
bool Strat_CountDown(Alien *al);

// Shared lightweight projectile strategy.
void Strat_ProjectileTick(Alien *self);
void Strat_ProjectileOnCollide(Alien *self);
Alien *Strat_SpawnProjectile(const Alien *owner,
                             int16 off_x, int16 off_y, int16 off_z,
                             uint8 rot_x, uint8 rot_y,
                             uint8 speed, uint8 lifetime,
                             uint8 ap, uint8 coll_type_bit);

// --- Sound ---

// Trigger a sound effect (trigse)
void Strat_TrigSE(uint8 sound_id);

#endif // STARFOX_STRAT_COMMON_H
