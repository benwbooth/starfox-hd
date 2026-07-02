#ifndef STARFOX_PARTICLES_H
#define STARFOX_PARTICLES_H

#include "../types.h"

void Particles_Init(void);
void Particles_Shutdown(void);
void Particles_Update(float dt);
void Particles_Render(void);
void Particles_Spawn(float x, float y, float z,
                     float vx, float vy, float vz,
                     float life, uint8 type);

#endif
