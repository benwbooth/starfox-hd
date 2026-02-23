#ifndef STARFOX_APU_H
#define STARFOX_APU_H

#include "../types.h"

// APU communication ports ($2140-$2143)
void Apu_Init(void);
void Apu_WritePort(int port, uint8 val);
uint8 Apu_ReadPort(int port);

#endif // STARFOX_APU_H
