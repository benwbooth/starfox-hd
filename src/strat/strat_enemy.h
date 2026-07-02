// Enemy strategies for Corneria (Level 1)
// Decompiled from GASTRATS.ASM, GSTRATS.ASM, GISTRATS.ASM, EXPSTRAT.ASM
//
// Each "Istrat" is an init-only function called once when the map executor
// spawns the object. It sets HP/AP, collision type, initial rotation, and
// assigns the per-frame strategy function (or NULL for static objects).

#ifndef STARFOX_STRAT_ENEMY_H
#define STARFOX_STRAT_ENEMY_H

#include "../game/obj.h"

// --- HP/AP constants from STRATEQU.INC ---
#define HARD_HP         0xFF    // -1 as uint8 = indestructible
#define HARD_AP         8
#define ROCKHARD_AP     20
#define ZACO1_HP        2
#define ZACO1_AP        4
#define PEXITBASE_SPEED 50

// --- Collision type helpers ---
// On SNES these set bits in al_collflags via lookup tables.
// In the HD build we use al_type bits for broad categories.
#define COLLTYPE_ENEMY1     0x01
#define COLLTYPE_ENEMY2     0x02
#define COLLTYPE_ENEMYWEAP  0x04
#define COLLTYPE_ZENEMY     0x08

// --- Init strategies (called once by map executor) ---

// Static building facing 180 degrees (GSTRATS.ASM hard180YR_Istrat)
void Strat_Hard180yr_Init(Alien *self);

// KSTRATS.ASM hard90YR_Istrat. Name is preserved from ASM.
void Strat_Hard90yr_Init(Alien *self);

// Static building, no Z-remove (stays even when behind player)
void Strat_Hard180yrNZR_Init(Alien *self);

// Indestructible static building facing forward (hard_Istrat)
void Strat_Hard_Init(Alien *self);

// Rotating static object (hardrot_Istrat)
void Strat_HardRot_Init(Alien *self);

// No-collision decorative object (nocoll_Istrat)
void Strat_NoColl_Init(Alien *self);

// Enemy fighter approaching from the left (zaco1L_Istrat)
void Strat_Zaco1L_Init(Alien *self);

// Enemy fighter approaching from the right (zaco1R_Istrat)
void Strat_Zaco1R_Init(Alien *self);

// Formation flyer used in early Corneria swarms.
void Strat_Zacos_Init(Alien *self);

// Spinning tower object used in Corneria towns.
void Strat_Tower0_Init(Alien *self);

// Route-2/3 gun emplacements from GASTRATS.ASM.
void Strat_Houdai_Init(Alien *self);
void Strat_HoudaiNS_Init(Alien *self);

// Krister route-2 attacker that hunts houdai emplacements.
void Strat_Zaco3_Init(Alien *self);
void Strat_Zaco0_Init(Alien *self);
void Strat_Szaco2_Init(Alien *self);
void Strat_Para_Init(Alien *self);
void Strat_Carrier_Init(Alien *self);
void Strat_Base1_Init(Alien *self);

// Kamikaze attacker that circles pillar targets.
void Strat_Zaco4_Init(Alien *self);

// Rotating radar dishes from the Corneria map scripts.
void Strat_Rader0_Init(Alien *self);
void Strat_Rader1_Init(Alien *self);

// Falling Corneria pillar (`pillar3_ISTRAT`).
void Strat_Pillar3_Init(Alien *self);

// Asteroid-belt cameleon ambusher.
void Strat_Cameleon_Init(Alien *self);

// Asteroid belt worms from GASTRATS.ASM.
void Strat_Wormhead_Init(Alien *self);
void Strat_Worm_Init(Alien *self);
void Strat_Worm2_Init(Alien *self);

// Invisible skillfly trigger volume used by map skill bonuses.
void Strat_Skillfly_Init(Alien *self);

// Explosion callback used by `tow_0` path objects.
void Strat_Tow0Explode(Alien *self);

// Heal gate used by Corneria map scripts (`gate3_Istrat`).
void Strat_Gate3_Init(Alien *self);
void Strat_Gate2_Init(Alien *self);
void Strat_Gate_Init(Alien *self);

// Sector X walker mounted on the space bar (`spacebarwalker_Istrat`).
void Strat_Spacebarwalker_Init(Alien *self);

// Sector X live shooter mounted on the space bar (`spacebarshoot_Istrat`).
void Strat_Spacebarshoot_Init(Alien *self);

// Black-hole slice enemies from GA2STRAT.ASM.
void Strat_Tadpole_Init(Alien *self);

// Attack carrier boss used by MAP1_1B / MAP2_1B (`boss7_Istrat`).
void Strat_Boss7_Init(Alien *self);

// Corneria route-3 boss from GB3STRAT.ASM (`bossA_Istrat`).
void Strat_BossA_Init(Alien *self);

// Asteroid belt boss from GBSTRATS.ASM (`boss1_Istrat`).
void Strat_Boss1_Init(Alien *self);

// Bomber wing enemy from GASTRATS.ASM.
void Strat_Bomwing_Init(Alien *self);

// Special weapon pickup (item5_Istrat).
void Strat_Item5_Init(Alien *self);
void Strat_Item7_Init(Alien *self);
void Strat_Up1man_Init(Alien *self);

// Wingman exit sequence (friendexitbase_Istrat)
void Strat_FriendExitBase_Init(Alien *self);
void Strat_ClshipGNDA_Init(Alien *self);
void Strat_ClshipGNDB_Init(Alien *self);
void Strat_ClshipGNDC_Init(Alien *self);
void Strat_ClshipWARPA_Init(Alien *self);
void Strat_ClshipWARPB_Init(Alien *self);
void Strat_ClshipWARPC_Init(Alien *self);
void Strat_ClshipEARTHA_Init(Alien *self);
void Strat_ClshipEARTHB_Init(Alien *self);
void Strat_ClshipEARTHC_Init(Alien *self);
void Strat_ClshipCHASEA_Init(Alien *self);
void Strat_ClshipCHASEB_Init(Alien *self);
void Strat_ClshipCHASEC_Init(Alien *self);

// Hit flash collision callback (hitflash_Istrat)
void Strat_HitFlash(Alien *self);

// Explosion death callback (explode_Istrat)
void Strat_Explode(Alien *self);

// Boss explosion callbacks (EXPSTRAT.ASM)
// These are set as expstratptr by boss strategies, not spawned from maps.

// Delayed boss explosion: countdown then final detonation.
// (Bossdelayexplode_Istrat / Bossdelayexplode_strat)
void Strat_BossDelayExplode_Init(Alien *self);

// Quick boss explosion: immediately triggers circle-delay explosion.
// (Qbossexplode_Istrat)
void Strat_QBossExplode_Init(Alien *self);

// Staged multi-part boss explosion with debris sequence.
// (bossexplode_Istrat)
void Strat_BossExplode_Init(Alien *self);

// Spinning space pilon enemy from GA3STRAT.ASM (`spacepilon_Istrat`).
void Strat_Spacepilon_Init(Alien *self);

// Venom 2 space boss from GB2STRAT.ASM (`bossF_Istrat`).
void Strat_BossF_Init(Alien *self);

// Title screen spinning Arwing (TITLE.ASM `tit_istrat`).
void Strat_Title_Init(Alien *self);

#endif // STARFOX_STRAT_ENEMY_H
