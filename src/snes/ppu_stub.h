#ifndef STARFOX_PPU_STUB_H
#define STARFOX_PPU_STUB_H

#include "../types.h"

// PPU stub for 2D screens (title, map selection, etc.)
// 3D gameplay bypasses the PPU entirely via the OpenGL renderer

void PpuStub_Init(void);
void PpuStub_WriteRegister(uint16 addr, uint8 val);
uint8 PpuStub_ReadRegister(uint16 addr);

#endif // STARFOX_PPU_STUB_H
