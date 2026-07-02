#ifndef STARFOX_SPC_BOOT_H
#define STARFOX_SPC_BOOT_H

#include "../types.h"

// Track IDs matching sndtbl order in SOUND.ASM
#define SND_INIT       0
#define SND_INTRO      1
#define SND_TITLE      2
#define SND_OPS        3
#define SND_TRAINING   4
#define SND_MAP        5
#define SND_CONTINUE   6
#define SND_BHOLE      7
#define SND_10         8
#define SND_11         9
#define SND_12        10
#define SND_13        11
#define SND_13B       12
#define SND_14        13
#define SND_15        14
#define SND_16        15
#define SND_20        16
#define SND_21        17
#define SND_22        18
#define SND_23        19
#define SND_24        20
#define SND_25        21
#define SND_26        22
#define SND_30        23
#define SND_31        24
#define SND_32        25
#define SND_33        26
#define SND_34        27
#define SND_35        28
#define SND_36        29
#define SND_37        30
#define SND_ENDSEQ    31
#define SND_STAFF     32
#define SND_GAMEOVER  33
#define SND_SPECIAL   34
#define SND_TRACK_COUNT 35

void SpcBoot_Init(void);
bool SpcBoot_LoadTrack(uint8 track_id);

// Set the asset directory (default "data"); sound data is <asset_dir>/snd/.
void SpcBoot_SetAssetDir(const char *dir);

// BGM start command for a track (snd_data 2nd arg in SOUND.ASM sndtbl).
// After the upload, the game writes this to APU port 0 to start the music.
// Values >= SND_TRACK_COUNT are returned unchanged (raw driver commands).
uint8 SpcBoot_TrackCommand(uint8 track_id);

#endif
