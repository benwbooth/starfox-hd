#ifndef STARFOX_GAME_MESSAGES_DATA_H
#define STARFOX_GAME_MESSAGES_DATA_H

#include "../types.h"

// Generated from reference/ultrastarfox/SF/MSG/ENGLISH.INC.
// whichfriend stores FRIEND_* in bits 0-6, with bit 7 set for *3 variants.
#define STRINGS_MESSAGE_ID_MIN         1u
#define STRINGS_MESSAGE_ID_MAX         142u
#define STRINGS_FRIEND_VARIANT3_BIT    0x80u

#define STRINGS_SOUND_CLASS_HELP       0u
#define STRINGS_SOUND_CLASS_DOWN       1u
#define STRINGS_SOUND_CLASS_OTHER      2u

typedef struct {
    uint8 whichfriend;
    uint8 sound_class;
    const char *text;
} StringsMessageData;

const StringsMessageData *Strings_GetEnglishMessageData(uint8 msg_id);

#endif // STARFOX_GAME_MESSAGES_DATA_H
