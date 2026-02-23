#include "ppu_stub.h"
#include <string.h>

// PPU register shadow (for 2D screen state)
static uint8 s_ppu_regs[0x40];  // $2100-$213F

void PpuStub_Init(void) {
    memset(s_ppu_regs, 0, sizeof(s_ppu_regs));
}

void PpuStub_WriteRegister(uint16 addr, uint8 val) {
    if (addr >= 0x2100 && addr <= 0x213F) {
        s_ppu_regs[addr - 0x2100] = val;
    }
}

uint8 PpuStub_ReadRegister(uint16 addr) {
    if (addr >= 0x2100 && addr <= 0x213F) {
        return s_ppu_regs[addr - 0x2100];
    }
    return 0;
}
