// Enemy strategies (from various *STRATS.ASM files)

#include "../game/obj.h"
#include "../sf_rtl.h"

// Basic enemy — flies forward and shoots
void Strat_EnemyBasic(Alien *self) {
    // Move forward
    self->worldz += self->vel;

    // Simple animation
    self->count++;

    // TODO: Fire at player when in range
    // TODO: Explosion when flags & afexp
}

// Granga-type enemy (Corneria boss)
void Strat_EnemyBoss(Alien *self) {
    (void)self;
    // TODO: Boss behavior patterns
}
