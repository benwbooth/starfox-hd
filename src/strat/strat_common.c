// STRATROU.ASM → C conversion
// Common strategy routines — movement, chase, velocity helpers

#include "../game/obj.h"
#include "../sf_rtl.h"

// Move alien by its velocity vector
void Strat_MoveByVelocity(Alien *al) {
    al->worldx += al->xv;
    al->worldy += al->yv;
    al->worldz += al->zv;
}

// Apply acceleration to velocity
void Strat_Accelerate(Alien *al) {
    al->xv += al->xa;
    al->yv += al->ya;
    al->zv += al->za;
}

// Simple chase toward a target position
void Strat_ChasePoint(Alien *al, int16 tx, int16 ty, int16 tz, uint8 speed) {
    int16 dx = tx - al->worldx;
    int16 dy = ty - al->worldy;
    int16 dz = tz - al->worldz;

    // Normalize and apply speed (simplified)
    if (dx > 0) al->xv = speed;
    else if (dx < 0) al->xv = -(int16)speed;
    if (dy > 0) al->yv = speed;
    else if (dy < 0) al->yv = -(int16)speed;
    if (dz > 0) al->zv = speed;
    else if (dz < 0) al->zv = -(int16)speed;
}

// Check if alien has reached a countdown
bool Strat_CountDown(Alien *al) {
    if (al->count > 0) {
        al->count--;
        return false;
    }
    return true;
}
