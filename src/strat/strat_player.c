// PSTRATS.ASM → C conversion
// Player (Arwing) controls, weapons, barrel roll

#include "../game/obj.h"
#include "../sf_rtl.h"

// Player strategy function — called every game tick for the Arwing
void Strat_Player(Alien *self) {
    // Read controller input
    uint16 pad = g_pad1;

    // Movement
    if (pad & PAD_UP)    self->worldy += 4;
    if (pad & PAD_DOWN)  self->worldy -= 4;
    if (pad & PAD_LEFT)  self->worldx -= 4;
    if (pad & PAD_RIGHT) self->worldx += 4;

    // Forward movement (always moving forward in Star Fox)
    self->worldz += self->vel;

    // Banking rotation from left/right input
    if (pad & PAD_LEFT)       self->rotz += 4;
    else if (pad & PAD_RIGHT) self->rotz -= 4;
    else {
        // Return to center
        if (self->rotz > 0) self->rotz -= 2;
        else if (self->rotz < 0) self->rotz += 2;
    }

    // Speed boost (Y button)
    if (pad & PAD_Y) {
        self->vel = 8;
    } else if (pad & PAD_B) {
        // Brake
        self->vel = 2;
    } else {
        self->vel = 4;
    }

    // Fire laser (A button)
    if (g_pad1_new & PAD_A) {
        // TODO: Spawn laser bolt alien
    }

    // Bomb (X button)
    if (g_pad1_new & PAD_X) {
        // TODO: Launch nova bomb
    }

    // Barrel roll (double-tap L or R)
    if (g_pad1_new & PAD_TLEFT) {
        // TODO: Barrel roll left
    }
    if (g_pad1_new & PAD_TRIGHT) {
        // TODO: Barrel roll right
    }
}
