#ifndef STARFOX_GAME_BGS_H
#define STARFOX_GAME_BGS_H

// Background/tilemap management (from BGS.ASM)
// Used for 2D screens (title, map, etc.)
void Bgs_Init(void);
void Bgs_Update(void);

// bgflags bits (from KALCS.INC)
#define BGF_RESTART 0x01
#define BGF_FOXY    0x02
#define BGF_BG      0x04
#define BGF_INFO    0x08
#define BGF_TEXT    0x10

#endif // STARFOX_GAME_BGS_H
