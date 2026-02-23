// Ground object strategies (buildings, terrain obstacles)

#include "../game/obj.h"

// Static ground object — just sits there
void Strat_GroundStatic(Alien *self) {
    // Ground objects scroll toward the player
    // Their Z position is updated relative to camera movement
    (void)self;
}
