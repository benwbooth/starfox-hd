#ifndef STARFOX_SPC_PLAYER_H
#define STARFOX_SPC_PLAYER_H

#include "../types.h"

// SPC700 audio emulation via blargg's snes_spc library

void SpcPlayer_Init(int sample_rate);
void SpcPlayer_Shutdown(void);

// Generate stereo samples (interleaved int16 L/R pairs)
void SpcPlayer_Generate(int16 *buffer, int num_samples);

// Send command to SPC700 (matching APU port protocol)
void SpcPlayer_SendCommand(uint8 cmd, uint8 param);

// Queue a BGM start command; the audio thread runs the port-0 write/echo
// handshake (startmus in IRQ.ASM).
void SpcPlayer_StartBgm(uint8 cmd);

// Direct APU port access (for SNES-style communication)
uint8 SpcPlayer_ReadPort(int port);
void SpcPlayer_WritePort(int port, uint8 val);

// Internal access for boot protocol
struct SNES_SPC;
struct SNES_SPC *SpcPlayer_GetSPC(void);

// Load a sound track via IPL boot protocol
void SpcPlayer_LoadTrack(uint8 track_id);

#endif // STARFOX_SPC_PLAYER_H
