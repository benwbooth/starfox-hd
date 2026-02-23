#ifndef STARFOX_AUDIO_H
#define STARFOX_AUDIO_H

#include "../types.h"
#include "../config.h"

void Audio_Init(const Config *cfg);
void Audio_Shutdown(void);

// Send a sound command (matching SNES APU port protocol)
void Audio_SendCommand(uint8 cmd, uint8 param);

// Set master volume (0-100)
void Audio_SetVolume(int volume);

#endif // STARFOX_AUDIO_H
