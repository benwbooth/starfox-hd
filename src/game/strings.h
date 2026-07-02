#ifndef STARFOX_GAME_STRINGS_H
#define STARFOX_GAME_STRINGS_H

#include "../types.h"

// STRINGS/GAMETEXT runtime hooks.
void Strings_Init(void);
void Strings_Update(void);
void Strings_DrawText(int x, int y, const char *text);

// Minimal send_message_l port entrypoint.
void Strings_SendMessage(uint8 msg_id);
uint8 Strings_GetActiveMessage(void);
const char *Strings_GetActiveMessageText(void);

// Optional registration API for extracted message metadata.
void Strings_RegisterMessage(uint8 msg_id, uint8 whichfriend, uint8 sound_class, const char *text);

#endif // STARFOX_GAME_STRINGS_H
