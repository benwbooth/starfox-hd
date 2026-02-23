#ifndef STARFOX_GAME_SOUND_H
#define STARFOX_GAME_SOUND_H

#include "../types.h"

// Sound command system (from SOUND.ASM)
void Sound_Init(void);
void Sound_Update(void);
void Sound_Play(uint8 sound_id);
void Sound_PlayMusic(uint8 track_id);
void Sound_StopMusic(void);

#endif // STARFOX_GAME_SOUND_H
