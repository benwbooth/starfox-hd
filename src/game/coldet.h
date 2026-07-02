#ifndef STARFOX_GAME_COLDET_H
#define STARFOX_GAME_COLDET_H

// Collision detection (from COLDET.ASM + STRATROU.ASM generate_collist_l)

void Coldet_Init(void);

// Generate collision list from active aliens (from generate_collist_l)
// Builds the list of aliens eligible for collision checks.
void Coldet_GenerateList(void);

// Run AABB collision detection between entries in the collision list
void Coldet_Run(void);

#endif // STARFOX_GAME_COLDET_H
