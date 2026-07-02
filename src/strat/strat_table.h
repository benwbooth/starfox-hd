// Strategy table registration
// Maps strategy IDs (from ISTRATS.ASM def_istrat order) to C function pointers.
// Called after World_Init to populate g_istrats[] and g_istrat_shapes[].

#ifndef STARFOX_STRAT_TABLE_H
#define STARFOX_STRAT_TABLE_H

// Flat-memory addresses for non-ISTRAT strategy symbols referenced directly by
// literal inline path code.
#define STRAT_ADDR_TOW0EXPLODE 0x030001u
#define STRAT_ADDR_GATE3       0x030002u
#define STRAT_ADDR_SPACEPILON  0x030004u
#define STRAT_ADDR_TIT         0x050020u
// Synthetic boss addresses referenced by levels.c map data (keep in sync with
// the STRAT_ADDR_* block there).
#define STRAT_ADDR_BOSSF       0x060010u

// Populate g_istrats[] and g_istrat_shapes[] with all known strategy functions.
// Must be called after World_Init() clears the tables.
void Strat_RegisterAll(void);

#endif // STARFOX_STRAT_TABLE_H
