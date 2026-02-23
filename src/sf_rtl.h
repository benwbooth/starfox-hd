#ifndef STARFOX_SF_RTL_H
#define STARFOX_SF_RTL_H

#include "types.h"
#include <SDL2/SDL.h>

// SNES runtime library — abstracts hardware for the decompiled game code.
// PPU writes become no-ops (renderer handles display).
// Controller reads map to SDL input.
// DMA becomes memcpy.

// Initialize the runtime
void SfRtl_Init(void);

// Process SDL events for input
void SfRtl_HandleEvent(const SDL_Event *event);

// Called at start of each game tick
void SfRtl_BeginFrame(void);

// --- SNES Memory ---
extern uint8 g_ram[WRAM_SIZE];     // 128KB WRAM ($7E0000-$7FFFFF)
extern uint8 g_sram[SRAM_SIZE];    // 64KB shared SRAM ($700000)
extern uint8 g_vram[VRAM_SIZE];    // 64KB VRAM

// --- Controller State ---
// 16-bit pad format matching SNES $4218/$421A
extern uint16 g_pad1;          // Current frame buttons
extern uint16 g_pad1_new;      // Newly pressed this frame
extern uint16 g_pad1_prev;     // Previous frame

// Pad bit definitions (matching VARS.INC)
#define PAD_B       (1 << 15)
#define PAD_Y       (1 << 14)
#define PAD_SELECT  (1 << 13)
#define PAD_START   (1 << 12)
#define PAD_UP      (1 << 11)
#define PAD_DOWN    (1 << 10)
#define PAD_LEFT    (1 << 9)
#define PAD_RIGHT   (1 << 8)
#define PAD_A       (1 << 7)
#define PAD_X       (1 << 6)
#define PAD_TLEFT   (1 << 5)
#define PAD_TRIGHT  (1 << 4)

// --- PPU Register Stubs ---
// These are no-ops for 3D gameplay; only needed for 2D screens
void SfRtl_PpuWrite(uint16 reg, uint8 val);
uint8 SfRtl_PpuRead(uint16 reg);

// --- DMA ---
void SfRtl_DmaTransfer(int channel, uint32 src, uint16 dst, uint16 size);

// --- Hardware Multiply/Divide ---
// SNES $4202/$4203 multiply, $4204-$4206 divide
uint16 SfRtl_Multiply(uint8 a, uint8 b);
uint16 SfRtl_Divide(uint16 dividend, uint8 divisor);
uint16 SfRtl_DivRemainder(void);

// --- Random Number Generator ---
// Exact SNES PRNG: rndval = (rndval * 91 + 0x61D7) & 0xFFFF
extern uint16 g_rndval;
uint16 SfRtl_Random(void);

// --- Timing ---
extern uint32 g_frame_count;

#endif // STARFOX_SF_RTL_H
