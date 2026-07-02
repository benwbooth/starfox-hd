// Strategy table registration
// Maps strategy IDs (matching def_istrat order from ISTRATS.ASM) to C functions.
//
// The SNES istrats[] table is a ROM array of 24-bit function pointers.
// Each map script's "strat byte" indexes into this table.
// Here we populate g_istrats[] with corresponding C function pointers.
//
// The strategy IDs are sequential starting from 0, in the order they appear
// in ISTRATS.ASM. Only strategies we have decompiled are registered;
// unimplemented ones remain NULL (the alien will have no per-frame behavior).

#include "strat_table.h"
#include "strat_player.h"
#include "strat_enemy.h"
#include "strat_ground.h"
#include "strat_boss2.h"
#include "strat_boss_sea.h"
#include "strat_boss8.h"
#include "istrat_shapes.h"
#include "../game/world.h"
#include "../game/obj.h"
#include "../path/paths.h"
#include <string.h>
#include <stdio.h>

#define STRAT_ADDR_FLAT_ID_BASE 0x000000u
#define STRAT_ADDR_SYNTH_BASE   0x020000u

// Strategy ID constants — indices into g_istrats[]
// Derived from ISTRATS.ASM def_istrat enumeration order.
#define IS_PLAYER            0
#define IS_STAYREL           8
#define IS_STAYDIST          9
#define IS_NOCOLL           10
#define IS_GND              15
#define IS_CLSHIPGNDA       19
#define IS_CLSHIPGNDB       20
#define IS_CLSHIPGNDC       21
#define IS_CLSHIPWARPA      22
#define IS_CLSHIPWARPB      23
#define IS_CLSHIPWARPC      24
#define IS_CLSHIPEARTHA     28
#define IS_CLSHIPEARTHB     29
#define IS_CLSHIPEARTHC     30
#define IS_CLSHIPCHASEA     37
#define IS_CLSHIPCHASEB     38
#define IS_CLSHIPCHASEC     39
#define IS_WORMHEAD         52
#define IS_GATE             53
#define IS_WORM             61
#define IS_WORM2            62
#define IS_CAMELEON         63
#define IS_BOSS1           69
#define IS_BOSSA           85
#define IS_BOSSF          116
#define IS_PILLAR3          79
#define IS_BOMWING          89
#define IS_UP1MAN           90
#define IS_RADER0           92
#define IS_RADER1           93
#define IS_ZACOS            94
#define IS_ZACO1L           95
#define IS_ZACO1R           96
#define IS_HOUDAI           97
#define IS_HOUDAINS         98
#define IS_BOSS7            99
#define IS_ZACO3           100
#define IS_TOWER0          101
#define IS_ZACO0           102
#define IS_ZACO4           103
#define IS_HARD180YR       105
#define IS_HARD180YRNZR    106
#define IS_PARA            107
#define IS_HARD90YR        127
#define IS_SZACO2          129
#define IS_STAYRELHARD180YR 137
#define IS_CARRIER         139
#define IS_FRIENDEXITBASE  152
#define IS_PATH            157
#define IS_SPACEBARWALKER  173
#define IS_SPACEBARSHOOT   174
#define IS_ITEM5           175
#define IS_ITEM7           177
#define IS_HARD180YRFOG    181
#define IS_GATE2           208
#define IS_HARDROT         210
#define IS_NOCOLLANIM0     221
#define IS_HARD            226
#define IS_TADPOLE         228
#define IS_PATHT           229
#define IS_BASE_1          230
#define IS_SHIPINTRO       239
#define IS_SKILLFLY        241
#define IS_PATHDHA         243

// Init wrapper: the map executor calls assign_strategy which sets
// al->stratptr = g_istrats[strat_idx]. For init-only strategies,
// we need the init function to be called first, then it sets stratptr
// to the per-frame function (or NULL for static objects).
//
// So g_istrats[] entries are init functions for init-only strategies,
// or the per-frame function if there's no init. The map executor calls
// it as the alien's first strategy tick.
static void Strat_RegisterAddressMap(void) {
    // Flat-memory runtime encoding:
    //   - Literal C-authored map/path references use flat 24-bit strategy IDs.
    //   - Some unresolved/raw literals use synthetic 0x02:xxxx addresses.
    // Register both forms so pointer-style ops resolve without index fallback.
    for (uint32 i = 0; i < ISTRAT_COUNT; i++) {
        StrategyFunc fn = g_istrats[i];
        if (!fn) continue;
        World_RegisterStrategyAddress(STRAT_ADDR_FLAT_ID_BASE | i, fn);
        World_RegisterStrategyAddress(STRAT_ADDR_SYNTH_BASE | i, fn);
    }

    World_RegisterStrategyAddress(STRAT_ADDR_TOW0EXPLODE, Strat_Tow0Explode);
    World_RegisterStrategyAddress(STRAT_ADDR_GATE3, Strat_Gate3_Init);
    World_RegisterStrategyAddress(STRAT_ADDR_SPACEPILON, (StrategyFunc)Strat_Spacepilon_Init);
    World_RegisterStrategyAddress(STRAT_ADDR_TIT, (StrategyFunc)Strat_Title_Init);
    // Level 3_6 references BossF by synthetic address; without this the boss
    // spawns inert and `mapwaitboss` never completes.
    World_RegisterStrategyAddress(STRAT_ADDR_BOSSF, (StrategyFunc)Strat_BossF_Init);
}

void Strat_RegisterAll(void) {
    // Populate strategy->shape mapping directly from ISTRATS.ASM extraction.
    memcpy(g_istrat_shapes, g_istrat_shape_defaults, sizeof(g_istrat_shape_defaults));

    // Leave unported strategies explicit and inert.
    for (uint32 i = 0; i < ISTRAT_CAPACITY; i++) {
        g_istrats[i] = NULL;
    }

    g_istrats[IS_PLAYER]          = Strat_Player;

    // Ground / static objects
    g_istrats[IS_STAYREL]         = (StrategyFunc)Strat_StayRel_Init;
    g_istrats[IS_STAYDIST]        = (StrategyFunc)Strat_StayDist_Init;
    g_istrats[IS_STAYRELHARD180YR]= (StrategyFunc)Strat_StayRelHard180yr_Init;
    g_istrats[IS_GND]             = (StrategyFunc)Strat_Gnd_Init;

    // Decorative (no collision)
    g_istrats[IS_NOCOLL]          = (StrategyFunc)Strat_NoColl_Init;
    g_istrats[IS_NOCOLLANIM0]     = (StrategyFunc)Strat_NoColl_Init;
    g_istrats[IS_GATE]            = (StrategyFunc)Strat_Gate_Init;
    g_istrats[IS_GATE2]           = (StrategyFunc)Strat_Gate2_Init;
    g_istrats[IS_WORMHEAD]        = (StrategyFunc)Strat_Wormhead_Init;
    g_istrats[IS_WORM]            = (StrategyFunc)Strat_Worm_Init;
    g_istrats[IS_WORM2]           = (StrategyFunc)Strat_Worm2_Init;

    // Indestructible buildings
    g_istrats[IS_HARD]            = (StrategyFunc)Strat_Hard_Init;
    g_istrats[IS_HARD180YR]       = (StrategyFunc)Strat_Hard180yr_Init;
    g_istrats[IS_HARD180YRNZR]    = (StrategyFunc)Strat_Hard180yrNZR_Init;
    g_istrats[IS_HARD90YR]        = (StrategyFunc)Strat_Hard90yr_Init;
    g_istrats[IS_HARD180YRFOG]    = (StrategyFunc)Strat_Hard180yr_Init;
    g_istrats[IS_HARDROT]         = (StrategyFunc)Strat_HardRot_Init;

    // Enemy fighters
    g_istrats[IS_BOMWING]         = (StrategyFunc)Strat_Bomwing_Init;
    g_istrats[IS_UP1MAN]          = (StrategyFunc)Strat_Up1man_Init;
    g_istrats[IS_ZACOS]           = (StrategyFunc)Strat_Zacos_Init;
    g_istrats[IS_RADER0]          = (StrategyFunc)Strat_Rader0_Init;
    g_istrats[IS_RADER1]          = (StrategyFunc)Strat_Rader1_Init;
    g_istrats[IS_BOSS1]           = (StrategyFunc)Strat_Boss1_Init;
    g_istrats[IS_BOSSA]           = (StrategyFunc)Strat_BossA_Init;
    g_istrats[IS_BOSS7]           = (StrategyFunc)Strat_Boss7_Init;
    g_istrats[IS_BOSSF]           = (StrategyFunc)Strat_BossF_Init;
    g_istrats[IS_CAMELEON]        = (StrategyFunc)Strat_Cameleon_Init;
    g_istrats[IS_PILLAR3]         = (StrategyFunc)Strat_Pillar3_Init;
    g_istrats[IS_ZACO1L]          = (StrategyFunc)Strat_Zaco1L_Init;
    g_istrats[IS_ZACO1R]          = (StrategyFunc)Strat_Zaco1R_Init;
    g_istrats[IS_HOUDAI]          = (StrategyFunc)Strat_Houdai_Init;
    g_istrats[IS_HOUDAINS]        = (StrategyFunc)Strat_HoudaiNS_Init;
    g_istrats[IS_ZACO3]           = (StrategyFunc)Strat_Zaco3_Init;
    g_istrats[IS_ZACO0]           = (StrategyFunc)Strat_Zaco0_Init;
    g_istrats[IS_SZACO2]          = (StrategyFunc)Strat_Szaco2_Init;
    g_istrats[IS_TOWER0]          = (StrategyFunc)Strat_Tower0_Init;
    g_istrats[IS_TADPOLE]         = (StrategyFunc)Strat_Tadpole_Init;
    g_istrats[IS_ZACO4]           = (StrategyFunc)Strat_Zaco4_Init;
    g_istrats[IS_PARA]            = (StrategyFunc)Strat_Para_Init;
    g_istrats[IS_CARRIER]         = (StrategyFunc)Strat_Carrier_Init;
    g_istrats[IS_BASE_1]          = (StrategyFunc)Strat_Base1_Init;
    g_istrats[IS_SKILLFLY]        = (StrategyFunc)Strat_Skillfly_Init;

    // Wingman exit
    g_istrats[IS_FRIENDEXITBASE]  = (StrategyFunc)Strat_FriendExitBase_Init;
    g_istrats[IS_CLSHIPGNDA]      = (StrategyFunc)Strat_ClshipGNDA_Init;
    g_istrats[IS_CLSHIPGNDB]      = (StrategyFunc)Strat_ClshipGNDB_Init;
    g_istrats[IS_CLSHIPGNDC]      = (StrategyFunc)Strat_ClshipGNDC_Init;
    g_istrats[IS_CLSHIPWARPA]     = (StrategyFunc)Strat_ClshipWARPA_Init;
    g_istrats[IS_CLSHIPWARPB]     = (StrategyFunc)Strat_ClshipWARPB_Init;
    g_istrats[IS_CLSHIPWARPC]     = (StrategyFunc)Strat_ClshipWARPC_Init;
    g_istrats[IS_CLSHIPEARTHA]    = (StrategyFunc)Strat_ClshipEARTHA_Init;
    g_istrats[IS_CLSHIPEARTHB]    = (StrategyFunc)Strat_ClshipEARTHB_Init;
    g_istrats[IS_CLSHIPEARTHC]    = (StrategyFunc)Strat_ClshipEARTHC_Init;
    g_istrats[IS_CLSHIPCHASEA]    = (StrategyFunc)Strat_ClshipCHASEA_Init;
    g_istrats[IS_CLSHIPCHASEB]    = (StrategyFunc)Strat_ClshipCHASEB_Init;
    g_istrats[IS_CLSHIPCHASEC]    = (StrategyFunc)Strat_ClshipCHASEC_Init;
    g_istrats[IS_SHIPINTRO]       = (StrategyFunc)Strat_ShipIntro_Init;
    g_istrats[IS_SPACEBARWALKER]  = (StrategyFunc)Strat_Spacebarwalker_Init;
    g_istrats[IS_SPACEBARSHOOT]   = (StrategyFunc)Strat_Spacebarshoot_Init;
    g_istrats[IS_ITEM5]           = (StrategyFunc)Strat_Item5_Init;
    g_istrats[IS_ITEM7]           = (StrategyFunc)Strat_Item7_Init;

    // Path-following enemies/wingmen
    g_istrats[IS_PATH]            = (StrategyFunc)Strat_Path_Init;
    g_istrats[IS_PATHT]           = (StrategyFunc)Strat_PathText_Init;
    g_istrats[IS_PATHDHA]         = (StrategyFunc)Strat_PathDha_Init;

    // Stage bosses ported in their own translation units.
    StratBoss2_Register();
    StratBossSea_Register();
    StratBoss8_Register();

    Strat_RegisterAddressMap();

    printf("Strat_RegisterAll: registered literal subset (path=%u patht=%u pathdha=%u)\n",
           IS_PATH, IS_PATHT, IS_PATHDHA);
}
