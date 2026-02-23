// SOUND.ASM → C conversion
// Sound command interface to SPC700

#include "sound.h"
#include "../audio/audio.h"

void Sound_Init(void) {
    // TODO: Upload SPC700 driver and sample data
}

void Sound_Update(void) {
    // TODO: Process queued sound commands
}

void Sound_Play(uint8 sound_id) {
    Audio_SendCommand(sound_id, 0);
}

void Sound_PlayMusic(uint8 track_id) {
    Audio_SendCommand(0x01, track_id);  // TODO: actual command protocol
}

void Sound_StopMusic(void) {
    Audio_SendCommand(0x00, 0);
}
