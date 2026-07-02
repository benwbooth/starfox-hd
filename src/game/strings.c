// STRINGS.ASM → C conversion
// Text rendering system

#include "strings.h"
#include "sound.h"
#include "game_vars.h"
#include "messages_data.h"
#include "../sf_rtl.h"
#include "../variables.h"
#include "../renderer/font.h"

static uint8 s_active_message;
static const char *s_active_text;
static uint8 s_face_frame;

typedef struct {
    bool used;
    uint8 whichfriend;
    uint8 sound_class;
    const char *text;
} MessageMeta;

static MessageMeta s_messages[256];

// CONTINUE.ASM constants
#define OPENING_FRAMES 5
#define MSG_CLOSE_SFX  0x64

static const uint8 s_face_sounds[24] = {
    0x60, 0x60, 0x60, 0x60, // fox
    0x7C, 0x7D, 0x62, 0x62, // rabbit
    0x7A, 0x7B, 0x61, 0x61, // falcon
    0x7E, 0x7F, 0x63, 0x63, // frog
    0x5F, 0x5F, 0x5F, 0x5F, // pepper
    0x8C, 0x8C, 0x8C, 0x8C  // andross
};

static uint8 strings_get_friend_hp(uint8 whichfriend) {
    switch (whichfriend & 0x7F) {
    case FRIEND_FOX:
        return g_fox_hp;
    case FRIEND_RABBIT:
        return g_bunny_hp;
    case FRIEND_FALCON:
        return g_falcon_hp;
    case FRIEND_FROG:
        return g_frog_hp;
    case FRIEND_PEPPER:
        return g_pepper_hp;
    case FRIEND_ANDROSS:
        return g_andross_hp;
    default:
        // friend_anyone and out-of-range values are treated as alive.
        return 1;
    }
}

static void strings_play_face_sound(void) {
    uint8 table_idx = (uint8)((((uint8)(g_whichfriend & 0x7F)) << 2) + g_friends_sound);
    if (table_idx < ARRAY_SIZE(s_face_sounds)) {
        Sound_PlaySE(s_face_sounds[table_idx]);
    }
}

static void strings_display_face(uint8 frame) {
    // Keep face animation state in sync with message flow for UI rendering.
    s_face_frame = frame;
}

void Strings_Init(void) {
    s_active_message = 0;
    s_active_text = NULL;
    s_face_frame = 0;
    g_whichfriend = FRIEND_FOX;
    g_friends_msg = 0;
    g_friends_sound = 2;
    g_friends_meter = 0;
    g_msg_count1 = 0;
    g_msg_count2 = 0;
    for (int i = 0; i < 256; i++) {
        s_messages[i].used = false;
        s_messages[i].whichfriend = FRIEND_FOX;
        s_messages[i].sound_class = 2;
        s_messages[i].text = NULL;
    }

    // Load literal message metadata/text extracted from ENGLISH.INC.
    for (uint16 msg_id = STRINGS_MESSAGE_ID_MIN; msg_id <= STRINGS_MESSAGE_ID_MAX; msg_id++) {
        const StringsMessageData *msg = Strings_GetEnglishMessageData((uint8)msg_id);
        if (!msg) {
            continue;
        }
        s_messages[msg_id].used = true;
        s_messages[msg_id].whichfriend = msg->whichfriend;
        s_messages[msg_id].sound_class = msg->sound_class;
        s_messages[msg_id].text = msg->text;
    }
}

void Strings_Update(void) {
    // friends_messages_l: do nothing while player is dying/dead.
    if (g_gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD)) {
        return;
    }

    if (g_msg_count1 == 0) {
        // closedown
        if (g_msg_count2 > 0) {
            g_msg_count2--;
            strings_display_face(g_msg_count2);
        }
        return;
    }

    if (g_msg_count2 < OPENING_FRAMES) {
        // openup
        strings_display_face(g_msg_count2);
        g_msg_count2++;
        if (g_msg_count2 >= OPENING_FRAMES) {
            strings_play_face_sound();
        }
        return;
    }

    // normal
    uint8 face_frame;
    uint8 rnd = (uint8)(SfRtl_Random() & 31);
    if (rnd == 0) {
        face_frame = 4;
    } else {
        face_frame = rnd & 1;
    }
    if (g_msg_count1 < 30) {
        face_frame = 0;
    }
    face_frame = (uint8)(face_frame + OPENING_FRAMES + ((g_whichfriend & 0x7F) << 1));
    strings_display_face(face_frame);

    g_msg_count1--;
    if (g_msg_count1 == 0) {
        Sound_PlaySE(MSG_CLOSE_SFX);
    }

    // friends_meter tracking decays toward real HP while visible.
    if (g_friends_meter != 0) {
        uint8 meter_hp = (uint8)(g_friends_meter & 0x7F);
        uint8 real_hp = strings_get_friend_hp(g_whichfriend);
        if (real_hp != meter_hp) {
            g_friends_meter = (uint8)(((uint8)(meter_hp - 1U)) | 0x80);
        }
    }
}

void Strings_DrawText(int x, int y, const char *text) {
    if (!text) return;
    Font_DrawString(x, y, text, 1.0f, 1.0f, 1.0f);
}

void Strings_SendMessage(uint8 msg_id) {
    if (msg_id == 0) return;

    uint8 whichfriend = FRIEND_FOX;
    uint8 sound_class = 2;
    const char *text = NULL;

    if (s_messages[msg_id].used) {
        whichfriend = s_messages[msg_id].whichfriend;
        sound_class = s_messages[msg_id].sound_class;
        text = s_messages[msg_id].text;
    }

    // send_message_l ignores dead speakers.
    if (strings_get_friend_hp(whichfriend) == 0) {
        return;
    }

    // friends_meter handshake: only preserve/start meter if prior value was 0xFF.
    if ((uint8)(g_friends_meter + 1U) == 0) {
        g_friends_meter = (uint8)(strings_get_friend_hp(whichfriend) | 0x80);
    } else {
        g_friends_meter = 0;
    }

    g_whichfriend = whichfriend;
    g_friends_sound = sound_class;
    g_friends_msg = msg_id; // Flat-memory replacement for original text pointer.
    g_msg_count1 = 50;
    g_msg_count2 = 0;
    s_active_text = text;
    s_active_message = msg_id;
}

uint8 Strings_GetActiveMessage(void) {
    return s_active_message;
}

const char *Strings_GetActiveMessageText(void) {
    return s_active_text;
}

void Strings_RegisterMessage(uint8 msg_id, uint8 whichfriend, uint8 sound_class, const char *text) {
    if (msg_id == 0) return;
    s_messages[msg_id].used = true;
    s_messages[msg_id].whichfriend = whichfriend;
    s_messages[msg_id].sound_class = sound_class;
    s_messages[msg_id].text = text;
}
