#ifndef STARFOX_GAME_NMI_H
#define STARFOX_GAME_NMI_H

// Per-frame game update (from NMI.ASM)
// Called during GAME_STATE_PLAYING at 20 FPS
void Nmi_GameTick(void);

#endif // STARFOX_GAME_NMI_H
