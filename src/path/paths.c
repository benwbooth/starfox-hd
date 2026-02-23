// PATHS.ASM → C conversion
// PATH bytecode interpreter
// Paths define predefined movement sequences for objects

#include "../types.h"
#include "../game/obj.h"

// PATH opcodes
// TODO: Define all ~70 path opcodes from PATHMACS.INC

void Paths_Init(void) {
    // TODO: Load path data from extracted assets
}

void Paths_Execute(Alien *al) {
    // TODO: Interpret path bytecode for this alien
    // Uses al->memptr as instruction pointer
    // Uses al->stackptr for call/return stack
    (void)al;
}
