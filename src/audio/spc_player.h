#ifndef STARFOX_SPC_PLAYER_H
#define STARFOX_SPC_PLAYER_H

#include "../types.h"

// SPC700 audio emulation via blargg's snes_spc library
// This will be integrated when we vendor snes_spc

void SpcPlayer_Init(int sample_rate);
void SpcPlayer_Shutdown(void);

// Load SPC700 program and BRR sample data from ROM
bool SpcPlayer_LoadFromRom(const uint8 *spc_data, int spc_size,
                            const uint8 *brr_data, int brr_size);

// Generate stereo samples (interleaved int16 L/R pairs)
void SpcPlayer_Generate(int16 *buffer, int num_samples);

// Send command to SPC700 (matching APU port protocol)
void SpcPlayer_SendCommand(uint8 cmd, uint8 param);

#endif // STARFOX_SPC_PLAYER_H
