#include "levels.h"
#include "../game/game_vars.h"
#include "../game/sound.h"
#include "../game/world.h"
#include "../path/path_literals.h"
#include "../strat/strat_table.h"
#include "../variables.h"
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#define MAP_DATA_CAPACITY   8192u
#define MAP_LABEL_CAPACITY  128u
#define MAP_FIXUP_CAPACITY  128u

// LEVEL1_1.ASM constants from STRATEQU.INC / map sources.
#define MEDPSPEED       65
#define PEXITBASE_SPEED 50
#define MYBASE_SCALE    3
#define BOSS7_SCALE     3
#define BOSSA_SCALE     2
#define BGM_BOSS1       5u
#define BGM_FANFARE     7u
#define BGM_FADEOUT     0xF1u
#define BG_3_1C         3u
#define BG_1_1C         4u
#define BG_1_3I         6u
#define BG_1_3E        12u
#define BG_2_3B        24u
#define BG_2_3C        25u
#define BG_1_3B         8u
#define BG_3_4D        35u
#define BG_1_3A         7u
#define BG_1_3C         9u
#define BG_3_4C        34u
#define BG_1_6C        17u
#define BG_2_6C        29u
#define BG_INTRO       40u
#define BG_TITLE       41u
#define BG_CONT        42u
#define BG_CRED        43u
#define BG_TRAINING    44u
#define CL_GND_FRIENDWAIT ((uint16)(MEDPSPEED * 30u))
#define CL_WARP_FRIENDWAIT CL_GND_FRIENDWAIT

// Shape ids ("MACRO-counted" def_shape order: the def_shape MACRO line in
// ISTRATS.ASM counts as id 0, matching g_istrat_shape_defaults and the
// compiled shape catalog in renderer/shape_data.h).
#define SH_NULLSHAPE     0
#define SH_MYSHIP_4      2   // hand-tuned builtin Arwing (shapes.h SHAPE_MYSHIP_4)
#define SH_GATE_0        8
#define SH_KAMIKAZE     10
#define SH_D_HEAD_0     14
#define SH_D_BODY_0     15
#define SH_CAMELEON     16
#define SH_BOSS_1_2     20
#define SH_BOM_WING     49
#define SH_W_L          50
#define SH_ZACO_6       53
#define SH_ZACO_5       54
#define SH_HOUDAI_0     55
#define SH_BOSS_7_1     56
#define SH_BOSS_A_2     58
#define SH_PARA_0       60
#define SH_PILLAR3      28
#define SH_BU_0         61
#define SH_BU_1         62
#define SH_BU_2         63
#define SH_BU_4         65
#define SH_BU_5         66
#define SH_BU_6         67
#define SH_BU_7         68
#define SH_BU_8         69
#define SH_R_BU_1       97
#define SH_SHIPS       111
#define SH_CARRIER     115
#define SH_TOWER_2      59  // wireframe-only: not in the compiled catalog
#define SH_ITEM_5      159
#define SH_ITEM_7      161
#define SH_WALKER_2    165
#define SH_SHIP_S_0    126
#define SH_SHIP_S_1    127
#define SH_ZACO_7      129
#define SH_ROBOT_0     170  // ro_0
#define SH_BASE_0      116
#define SH_MYBASE_0    256  // SHAPE_EXT_MYBASE_0 (extended shape catalog)
#define SH_MYBASE_1    125
#define SH_ZACO_8      105
#define SH_ZACO_4      106
#define SH_ASTEROID2   195
#define SH_B_HOU_0     164
#define SH_ZACO_A      218
#define SH_ZACO_B      202
#define SH_FRIENDSHIP_4 219
#define SH_S_WARK_0    220
#define SH_TADPOLE     228
#define SH_BIG_GATE    234
#define SH_BASE_1      233
#define SH_ARCH_0      229
#define SH_RADER_0      51
#define SH_RADER_1      52
#define SH_TOW_0       248
#define SH_OP_0        551
#define SH_OP_1        552
#define SH_OP_2        553
#define SH_IMYSHIP_4   554
#define SH_MOTHER1     278
#define SH_MY_BIRD     557u
#define SH_ROUND_0      17
#define SH_SPACEPILON  614u
#define SH_XWIRESPACEBAR  137
#define SH_XPWIRESPACEBAR 138
#define SH_SXPWIRESPACEBAR 139
#define SH_YWIRESPACEBAR  140
#define SH_ZWIRESPACEBAR  141
#define SH_SXWIRESPACEBAR 142
#define SH_SYWIRESPACEBAR 143
#define SH_SZWIRESPACEBAR 144
#define SH_XSOLIDSPACEBAR 145
#define SH_XPSOLIDSPACEBAR 146
#define SH_SXPSOLIDSPACEBAR 147
#define SH_YSOLIDSPACEBAR 148
#define SH_ZSOLIDSPACEBAR 149
#define SH_SXSOLIDSPACEBAR 150
#define SH_SYSOLIDSPACEBAR 151
#define SH_SZSOLIDSPACEBAR 152
#define SH_COLONY_0     153
#define SH_COLONY_1     154
#define SH_COLONY_2     155
#define SH_COLONY3L     156
#define SH_COLONY3R     157
#define SH_BWARKER_3    158
// `asteroid1` is still not mapped to a canonical flat-runtime shape id.
// Keep route-2 slow meteors explicit but visually inert until that symbol is
// recovered from the original shape tables.
#define SH_ASTEROID1_PROXY 275  // SHAPE_EXT_ASTEROID1 (USHAPES.ASM)
// `big_meteor` lives in USHAPES.ASM, not in the main def_shape table.
// Proxy through nullshape until the extended shape is recovered.
#define SH_BIG_METEOR_PROXY 279  // SHAPE_EXT_BIG_METEOR (USHAPES.ASM)
// `item_0` is still not mapped to a canonical flat-runtime shape id.
// Keep the literal sword2 write but point it at the same proxy the bounded
// `up1man` runtime currently renders.
#define SH_ITEM_0_PROXY SH_MYSHIP_4
#define SH_ITEM_6       160
#define SH_SHARK         13
#define SH_BIG_M         19
#define SH_FLINGBOSS     12
#define SH_SHIELDR      203
#define SH_RAY_0        204
#define SH_RAY_1        205
#define SH_S_FISH       212
#define SH_IKA          221
#define SH_S_ZACO_0     222
#define SH_BZACO_8      232
// `whale` is not in the def_shape table (lives in SHAPES5.ASM).
// Keep a proxy until the extended shape is recovered.
#define SH_WHALE_PROXY  281  // SHAPE_EXT_WHALE (SHAPES5.ASM)
// `colony_0` and `Sship_0_c` are background shapes in CL_SHIP clear demos.
// Proxy through nullshape until the shapes are recovered.
#define SH_COLONY_0_PROXY SH_NULLSHAPE
#define SH_SSHIP_0_C_PROXY SH_NULLSHAPE

// 3-4-T / 1-3-T tunnel shape ids
// `wall_0` is from def_shape; id 49 same as wall_2 area. Proxy through wall_2.
#define SH_WALL_0_PROXY    SH_WALL_2
// `wall_5` is from def_shape; proxy through wall_2.
#define SH_WALL_5_PROXY    SH_WALL_2
// `open_l` is a door shape from SHAPES2.ASM. Proxy through nullshape.
#define SH_OPEN_L_PROXY    SH_NULLSHAPE
// `up_door` is a door shape from SHAPES2.ASM. Proxy through nullshape.
#define SH_UP_DOOR_PROXY   SH_NULLSHAPE
// `warker_3` is a walker enemy from SHAPES3.ASM. Proxy through bwarker_3.
#define SH_WARKER_3_PROXY  SH_BWARKER_3
// `bou_0` is a boulder/obstacle from def_shape. Proxy through nullshape.
#define SH_BOU_0_PROXY     SH_NULLSHAPE

// MAP1_3C shape ids
// `ship_4` is a cruiser from SHAPES2.ASM. Proxy through nullshape.
#define SH_SHIP_4_PROXY    SH_NULLSHAPE
// `ship_0_c` is the big ship approach model from SHAPES2.ASM.
#define SH_SHIP_0_C_PROXY  SH_NULLSHAPE
// `bshipexitface` is the big ship exit door from SHAPES2.ASM.
#define SH_BSHIPEXITFACE_PROXY SH_NULLSHAPE

// PLANET.ASM shape ids
#define SH_R_BU_4         100   // from def_shape r_bu_4
#define SH_R_BU_7         103   // from def_shape r_bu_7
#define SH_R_BU_6         102   // from def_shape r_bu_6

// MAP3_7A / MAP1_6A (Venom surface) shape ids
// `wall1` is a moving wall obstacle from SHAPES.ASM. Proxy through wall_2.
#define SH_WALL_1_PROXY    SH_WALL_2

// TRANSFOR.ASM (Venom 3 Surface Boss) shape ids
// `boss_f_4` is the Transformer boss from SHAPES3.ASM. Proxy through nullshape.
#define SH_BOSS_F_4_PROXY SH_NULLSHAPE

// TRAINING.ASM shape ids
// `pilon` is a ground pylon from SHAPES.ASM. Proxy through nullshape.
#define SH_PILON_PROXY     SH_NULLSHAPE

// SPECIAL.ASM (Out of This Dimension) shape ids
// `paper_1`, `paper_3` live in SHAPES4.ASM, not in the main def_shape table.
// Proxy through nullshape until the extended shapes are recovered.
#define SH_PAPER_1_PROXY  SH_NULLSHAPE
#define SH_PAPER_3_PROXY  SH_NULLSHAPE
// `pole_0` lives in an extended bank. Proxy through nullshape.
#define SH_POLE_0_PROXY   SH_NULLSHAPE
// `slot_0` is in SHAPES.ASM but its flat index is not yet established.
// Proxy through nullshape until the renderer registers it.
#define SH_SLOT_0_PROXY   SH_NULLSHAPE
// `font_t2` etc. are KSHAPES.ASM letter shapes for "THE END" sequence.
// Proxy through nullshape until the extended shapes are recovered.
#define SH_FONT_T2_PROXY  SH_NULLSHAPE
#define SH_FONT_H2_PROXY  SH_NULLSHAPE
#define SH_FONT_E2_PROXY  SH_NULLSHAPE
#define SH_FONT_E3_PROXY  SH_NULLSHAPE
#define SH_FONT_N2_PROXY  SH_NULLSHAPE
#define SH_FONT_D2_PROXY  SH_NULLSHAPE

// TRUCKER.ASM (Mad Trucker / Venom 2 Highway) shape ids
// `air_1` is in SHAPES.ASM but its flat index is not yet established.
#define SH_AIR_1_PROXY    SH_NULLSHAPE
// `boss_9_5` lives in SHAPES3.ASM. Proxy through nullshape.
#define SH_BOSS_9_5_PROXY SH_NULLSHAPE
// `wall_4` is not in the main def_shape table. Proxy through nullshape.
#define SH_WALL_4_PROXY   SH_NULLSHAPE
// `bou_1b` lives in SHAPES4.ASM. Proxy through nullshape.
#define SH_BOU_1B_PROXY   SH_NULLSHAPE
// `line_2` lives in SHAPES5.ASM. Proxy through nullshape.
#define SH_LINE_2_PROXY   SH_NULLSHAPE

// MAP1_3A1 (Space Armada Ship 1) shape ids
// `ship_1` lives in SHAPES2.ASM. Proxy through nullshape.
#define SH_SHIP_1_PROXY   SH_NULLSHAPE
// `ship_3` lives in SHAPES2.ASM. Proxy through nullshape.
#define SH_SHIP_3_PROXY   SH_NULLSHAPE
// `s_door_1`, `s_door_2` are extended shapes. Proxy through nullshape.
#define SH_S_DOOR_1_PROXY SH_NULLSHAPE
#define SH_S_DOOR_2_PROXY SH_NULLSHAPE

// MAP1_3A2 (Space Armada Ship 2) shape ids
// `ship_5S`, `ship_5m`, `ship_5` are extended shapes from SHAPES2.ASM.
#define SH_SHIP_5S_PROXY SH_NULLSHAPE
#define SH_SHIP_5M_PROXY SH_NULLSHAPE
#define SH_SHIP_5_PROXY  SH_NULLSHAPE

// FINALMAP.ASM shape ids
// `wall_2` is from def_shape, id 89.
#define SH_WALL_2        89
// `bou_1` is from def_shape, id 60 (same as bu_0).
// Proxy through nullshape until the shape is recovered.
#define SH_BOU_1_PROXY   SH_NULLSHAPE
// `face_b` is the Andross boss face from SHAPES3.ASM. Proxy through nullshape.
#define SH_FACE_B_PROXY  SH_NULLSHAPE

// INTRO.ASM shape ids
// `nintendo`, `presents` are text/logo shapes. Proxy through nullshape.
#define SH_NINTENDO_PROXY   SH_NULLSHAPE
#define SH_PRESENTS_PROXY   SH_NULLSHAPE
// `old_type` is the player ship intro shape. Proxy through myship_4.
#define SH_OLD_TYPE_PROXY   SH_MYSHIP_4
// `deboss_1` is the intro boss shape. Proxy through nullshape.
#define SH_DEBOSS_1_PROXY   SH_NULLSHAPE

// TITLE.ASM shape ids
// `my_demo` is the title screen ship. Proxy through myship_4.
#define SH_MY_DEMO_PROXY    SH_MYSHIP_4

// MAP1_5 (Venom 1 Orbital) shape ids
// `zaco_1` is from def_shape. Proxy through s_zaco_0 until recovered.
#define SH_ZACO_1_PROXY  SH_S_ZACO_0
// `warp` is a warp gate shape from SHAPES4.ASM. Proxy through nullshape.
#define SH_WARP_PROXY    SH_NULLSHAPE
// `boss_b_1` is the Venom 1 orbital boss from SHAPES2.ASM. Proxy through nullshape.
#define SH_BOSS_B_1_PROXY SH_NULLSHAPE

// MAP3_6 (Venom 2 Space) shape ids
// `boss_f_3` is the King Joh boss from SHAPES3.ASM. Proxy through nullshape.
#define SH_BOSS_F_3_PROXY SH_NULLSHAPE

// WASHMAP (Washing Machine Boss) shape ids
// `boss_8_0` is the wash boss outer shell from SHAPES3.ASM. Proxy through nullshape.
#define SH_BOSS_8_0_PROXY SH_NULLSHAPE
// `hou_4` is the nucleus launcher from def_shape. Proxy through houdai_0.
#define SH_HOU_4_PROXY   SH_HOUDAI_0
// `boss_8_4` is the wash boss pillar from SHAPES3.ASM. Proxy through nullshape.
#define SH_BOSS_8_4_PROXY SH_NULLSHAPE

// MAP2_5 (Venom 2 Orbital) shape ids
#define SH_WIRE_MAN      48
#define SH_BAZOOKA      132
#define SH_UPER_M       133
#define SH_BOSS_E_4     206

// MAP2_3A / MAP2_3C shape ids
#define SH_M_TANK       7
#define SH_WALKER_0     27
#define SH_CORE_1_1     72
#define SH_R_BU_2       98
#define SH_K_DOOR      118
#define SH_KICHI_3     119
#define SH_KICHI_0     120
#define SH_BOSS_G_0    121
#define SH_HELI        131
#define SH_TANK_1      168
#define SH_HOU_5       169
#define SH_BRO_0       177
#define SH_BRO_1       178
#define SH_BRO_2       179
#define SH_BRO_3       180
#define SH_BRO_4       181
#define SH_BRO_5       182
#define SH_BRO_6       183
#define SH_CLISLA_M    198
#define SH_CLISLA_S    199
#define SH_CLISLA_L    200
#define SH_SEA_0_0      32
#define SH_S_TANK_0    230
// `r_but_2` is a raw bitmap shape from SHAPES4.ASM not in def_shape.
// Proxy through the def_shape entry r_bu_2 (id 97).
#define SH_R_BUT_2_PROXY SH_R_BU_2
// `walk_4_0` is a raw shape from SHAPES3.ASM not in def_shape.
// Proxy through walker_0 (id 26).
#define SH_WALK_4_0_PROXY SH_WALKER_0

// MAP3_3A (Fortuna) shape ids
#define SH_BEEANIM      17
#define SH_BOSS_D_1     78
#define SH_SNAKE_1     201
#define SH_FLOWER_1    207
#define SH_FLOWER_2    208
#define SH_STALK       209
#define SH_BOSS_D_4    239
// `f_fish` lives in SHAPES3.ASM, not in the main def_shape table.
// Proxy through s_fish until the extended shape is recovered.
#define SH_F_FISH_PROXY 271  // SHAPE_EXT_F_FISH (SHAPES3.ASM)

// MAP1_4 (Asteroid Belt 2) shape ids
// `gro_0`..`gro_6` are ground rock shapes from SHAPES3.ASM (extended bank).
// Proxy through bu_0/bu_1 until the extended shapes are recovered.
#define SH_GRO_0_PROXY  SH_BU_0
#define SH_GRO_1_PROXY  SH_BU_0
#define SH_GRO_4_PROXY  SH_BU_1
#define SH_GRO_5_PROXY  SH_BU_1
#define SH_GRO_6_PROXY  SH_BU_2
// `base_0_0`, `base_0_1` are launch-base shapes from SHAPES3.ASM.
// Proxy through nullshape until the extended shapes are recovered.
#define SH_BASE_0_0_PROXY SH_NULLSHAPE
#define SH_BASE_0_1_PROXY SH_NULLSHAPE
// `btank_1` is a ground tank from SHAPES3.ASM. Proxy through s_tank_0.
#define SH_BTANK_1_PROXY  SH_S_TANK_0
// `tank_2` is a ground tank from SHAPES3.ASM. Proxy through s_tank_0.
#define SH_TANK_2_PROXY   SH_S_TANK_0
// `boss_h_0` is the Macbeth spider boss from SHAPES.ASM. Proxy through nullshape.
#define SH_BOSS_H_0_PROXY SH_NULLSHAPE
#define SH_BU_3         64

// MAP3_5 (Venom 1 Surface) shape ids
// `RO_0`..`RO_6` are rock formations from SHAPES3.ASM (extended bank).
// Proxy through bu_0/bu_1 variants until the extended shapes are recovered.
#define SH_RO_0_PROXY   SH_BU_0
#define SH_RO_1_PROXY   SH_BU_1
#define SH_RO_2_PROXY   SH_BU_0
#define SH_RO_3_PROXY   SH_BU_1
#define SH_RO_4_PROXY   SH_BU_0
#define SH_RO_5_PROXY   SH_BU_1
#define SH_RO_6_PROXY   SH_BU_2
// `miss_1_1` is the invisible missile carrier from def_shape, id 37.
#define SH_MISS_1_1      37
// `miss_1_2` is the visible missile body from def_shape, id 9.
#define SH_MISS_1_2       9
// `volcano` is from SHAPES3.ASM. Proxy through nullshape.
#define SH_VOLCANO_PROXY  SH_NULLSHAPE
// `Svolcano` (small volcano) is from SHAPES3.ASM. Proxy through nullshape.
#define SH_SVOLCANO_PROXY SH_NULLSHAPE
// `boss_2_2` is the Venom 1 boss from SHAPES2.ASM. Proxy through nullshape.
#define SH_BOSS_2_2_PROXY SH_NULLSHAPE
// `truck` is the Macbeth train body from def_shape, id 5.
#define SH_TRUCK          5
// `rail_0` is straight track from def_shape, id 36.
#define SH_RAIL_0        36
// `rail_4` is corner track from def_shape, id 6.
#define SH_RAIL_4         6
// `pipe_9_0` is the colony pipe background from SHAPES5.ASM. Proxy through nullshape.
#define SH_PIPE_9_0_PROXY SH_NULLSHAPE
// `pipe_9` is the colony pipe exit from SHAPES5.ASM. Proxy through nullshape.
#define SH_PIPE_9_PROXY   SH_NULLSHAPE

// MAP3_2 tail shape ids
#define SH_R_HOU_0     162
#define SH_METEO_0     193
#define SH_BOSS_0_1     85

// MAP3_3B / BHOLE / MOTHERS shape ids
#define SH_IRIS          4
#define SH_S_HOU_0     163
// `pipe_8_0` and `pipe_8` are extended shapes from SHAPES5.ASM.
// Proxy through nullshape until the shapes are recovered.
#define SH_PIPE_8_0_PROXY SH_NULLSHAPE
#define SH_PIPE_8_PROXY   SH_NULLSHAPE
// MOTHERS.ASM shape proxies — many mother sub-map shapes aren't in the
// main def_shape table or aren't recovered yet. Proxy through nullshape.
#define SH_PETECUBE_PROXY  SH_NULLSHAPE
#define SH_BOUNCYBALL_PROXY SH_NULLSHAPE
#define SH_ASTEROID3_PROXY SH_NULLSHAPE
#define SH_ASTEROID4_PROXY SH_NULLSHAPE
#define SH_MINE_0_PROXY    SH_NULLSHAPE
#define SH_RPILLAR3_PROXY  SH_PILLAR3
#define SH_LINE3_PROXY     SH_NULLSHAPE
#define SH_LINE_0_PROXY    SH_NULLSHAPE
#define SH_LINE_1_PROXY    SH_NULLSHAPE
#define SH_TUNNEL_0        122   // from def_shape tunnel_0
#define SH_TUNNEL_4        123   // from def_shape tunnel_4
#define SH_TUNNEL_7        124   // from def_shape tunnel_7
#define SH_AMOEBA2         104   // from def_shape amoeba2
#define SH_CLASTEROID_PROXY 280  // SHAPE_EXT_CLASTEROID (USHAPES.ASM)
#define SH_MINE_2          210   // from def_shape mine_2

// Strategy ids (ISTRATS.ASM order).
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
#define IS_CLSHIPSHIPA      25
#define IS_CLSHIPSHIPB      26
#define IS_CLSHIPSHIPC      27
#define IS_CLSHIPTURNA      31
#define IS_CLSHIPTURNB      32
#define IS_CLSHIPTURNC      33
#define IS_CLSHIPBRIDGEA    34
#define IS_CLSHIPBRIDGEB    35
#define IS_CLSHIPBRIDGEC    36
#define IS_CLSHIPCHASEA     37
#define IS_CLSHIPCHASEB     38
#define IS_CLSHIPCHASEC     39
#define IS_CLSHIPDIVEA      40
#define IS_CLSHIPDIVEB      41
#define IS_CLSHIPDIVEC      42
#define IS_CLSHIPUNDERA     43
#define IS_CLSHIPUNDERB     44
#define IS_CLSHIPUNDERC     45
#define IS_GATE             53
#define IS_WORMHEAD         52
#define IS_FLINGBOSS        58
#define IS_WORM             61
#define IS_CAMELEON         63
#define IS_BOSS1            69
#define IS_BOSSA            85
#define IS_PILLAR3          79
#define IS_BOMWING          89
#define IS_UP1MAN           90
#define IS_WINGLAZERMAN     91
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
#define IS_PARA            107
#define IS_HARD90YR        127
#define IS_SHIPS           133
#define IS_CARRIER         139
#define IS_FRIENDEXITBASE  152
#define IS_SZACO5          156
#define IS_PATH            157
#define IS_SPACEBARWALKER  173
#define IS_SPACEBARSHOOT   174
#define IS_ITEM5           175
#define IS_ITEM6           176
#define IS_ITEM7           177
#define IS_BLACKHOLE       196
#define IS_SFISH           209
#define IS_HARDROT         210
#define IS_SHIPINTRO       239
#define IS_HARD            226
#define IS_TADPOLE         228
#define IS_BASE_1          230
#define IS_BIG_METEOR      234
#define IS_BREAK_METEOR    235
#define IS_BREAK_METEORT   238
#define IS_PATHDHA         243
#define IS_SKILLFLY        241
#define IS_WINDMILL         66
#define IS_SZACO2          129
#define IS_SZACO0          130
// MAP2_3A strategy ids
#define IS_MISSTANK         50
#define IS_KDOOR           139
#define IS_KICHI2          140
#define IS_KDOOR2          141
#define IS_MASSIVEBASE     142
#define IS_BASE1           181
#define IS_TANK3           186
#define IS_HOUDAI5F        187
#define IS_ROCKHARD        192
// MAP2_5 (Venom 2 Orbital) strategy ids
#define IS_WIREMAN         88
#define IS_CASTANET       124
#define IS_BAZOOKAL       158
#define IS_BAZOOKAR       159
#define IS_UPERM          160
// MAP3_3A (Fortuna) strategy ids
#define IS_CHICKEN         117
#define IS_SEADRAGON2      197
#define IS_LOCHNESSMONSTER 198
#define IS_TREE1           204
#define IS_TREE2           205
#define IS_TREE3           206

// MAP1_5 (Venom 1 Orbital) strategy ids
// `warp_Istrat` is the warp gate strategy. Synthetic address.
#define STRAT_ADDR_WARP             0x06000Eu
// `mine0_istrat` as an inline strategy (not mother-spawned).
#define IS_MINE0           246
// `bossb_Istrat` is the Venom 1 orbital boss strategy. Synthetic address.
#define STRAT_ADDR_BOSSB            0x06000Fu

// MAP3_6 (Venom 2 Space) strategy ids
// `bossf_Istrat` is the King Joh boss strategy. Synthetic address.
#define STRAT_ADDR_BOSSF            0x060010u
// `splayerflymode` check callback. Synthetic.
#define MAP_CB_MAP3_6_NOCTRL_WAIT   0x060011u
#define MAP_CB_MAP3_6_HPCHECK_WAIT  0x060012u
#define MAP_CB_MAP3_6_FLYMODE_CHECK 0x060013u

// WASHMAP (Washing Machine Boss) strategy ids
// `boss8_Istrat` is the wash boss strategy. Synthetic address.
#define STRAT_ADDR_BOSS8            0x060014u
// `nucleuslauncher_Istrat` is the nucleus launcher strategy. Synthetic address.
#define STRAT_ADDR_NUCLEUSLAUNCHER  0x060015u
// `nucleuspillar_Istrat` is the nucleus pillar strategy. Synthetic address.
#define STRAT_ADDR_NUCLEUSPILLAR    0x060016u
// `EscapeNucleus` player mode callback. Synthetic.
#define MAP_CB_ESCAPE_NUCLEUS       0x060017u

// MAP3_4B strategy ids
#define IS_SHARK            60
#define IS_MISSPOD          68
#define IS_COLONY0         170
#define IS_COLONY1         171
#define IS_COLONY2         172

// MAP3_2 tail strategy ids
#define IS_SHOU0A          179
#define IS_SHOU0           178
#define IS_METEO0          195
#define IS_WEBMONSTER      123
// MAP3_3B / BHOLE strategy ids
#define IS_SEAMON           81
#define IS_IRIS             48
#define IS_BHOLEEXIT1      244
#define IS_BHOLEEXIT2      245
#define IS_BHOLEEXIT3      246
#define IS_COLONYEXIT      236
// MAP2_3B strategy ids
#define IS_TORPEDO          80
#define STRAT_ADDR_BOSSSEAMON 0x030005u
// MAP2_3C strategy ids
#define STRAT_ADDR_BOSSG      0x030006u

// MAP1_4 (Asteroid Belt 2) strategy ids
#define IS_WALKING          78
#define IS_TANK1A          183
#define IS_TANK2           162
#define IS_BASE0           138
#define IS_HARD180YRFOG    180
#define IS_ITEM6           176
// `bossh_istrat` is the Macbeth spider boss strategy. Synthetic address.
#define STRAT_ADDR_BOSSH        0x06000Au
// `tank2_istrat` uses the existing IS_TANK2 flat id.
// `base0_istrat` uses the existing IS_BASE0 flat id.
// `base1_istrat` uses the existing IS_BASE1 flat id.
// `kamome` (heli patrol) is a path-based strategy. Synthetic address.
#define STRAT_ADDR_KAMOME        0x06000Bu
// `tmp_tank` (heli-dropped tank) is a path-based strategy. Synthetic address.
#define STRAT_ADDR_TMP_TANK      0x06000Cu

// MAP3_5 (Venom 1 Surface) strategy ids
#define IS_WOODS            54
#define IS_TRUCK            49
#define IS_TRACKCORNER      50
#define IS_VOLCANO         191
#define IS_FIREPILLAR      193
#define IS_BOSS2           108
// `saka_hou` (inverted cannon) uses a synthetic address.
#define STRAT_ADDR_SAKAHOU       0x06000Du
// `truck_istrat` uses the existing IS_TRUCK flat id.
// `trackcorner_istrat` uses the existing IS_TRACKCORNER flat id.
#define BOSS2_SCALE         3

// WASHMAP (Washing Machine Boss) constants from STRATEQU.INC / WASHMAP.ASM
#define BOSS8_SCALE         3
#define NUCLEUSHEIGHT       100
#define BOSS8_CIRC          (210 << BOSS8_SCALE)  // 1680
#define ROTSIZE_WASH        DEG45
#define ROTNUM_WASH         (DEG360 / ROTSIZE_WASH)  // 8

// Train track constants (from STRATEQU.INC)
#define TSIZE             200
#define DIR_EAST          DEG270
#define DIR_NORTH         DEG0
#define DIR_WEST          DEG90
#define DIR_SOUTH         DEG180

// SNES alien-struct field offsets used by MAPMACS setalvar.
#define AL_WORLDX 12
#define AL_SBYTE1 34
#define AL_SBYTE2 35
#define AL_SBYTE3 36
#define AL_PTR     6
#define AL_SWORD1 38
#define AL_SWORD2 40
#define AL_HP     42
#define AL_AP     43
#define AL_ROTX   18
#define AL_ROTY   19
#define AL_ROTZ   20
#define AL_VEL    21
#define ALX_SWPX1 0
#define ALX_SWPY1 2
#define ALX_DEPTHOFFSET 21
#define ALX_PWORD1 52

// External vars mirrored into flat WRAM for map opcodes.
#define WM_MAPVAR1  0x0320u
#define WM_SKILLFLY 0x0304u
#define WM_STAGECLEAR 0x0305u
#define WM_CLB2       0x0306u
#define WM_LEVELFINISHED 0x0307u
#define WM_ONECREDSPR  0x0308u
#define WM_INFOG       0x0309u
#define WM_FADEPAL     0x030Au
#define WM_PALFROM     0x030Bu
#define WM_PALTO       0x030Cu
#define WM_PALLEN      0x030Du
#define WM_PLAYERPOSX  0x030Eu  // 2 bytes
#define WM_GSVAR_BYTE1 0x0310u
#define WM_MAPTRIGGER  0x0311u
#define WM_NUMENDOK    0x0312u
#define WM_NUMPLASERS  0x0313u
#define WM_HPOSJMP     0x0314u  // 2 bytes
#define WM_BOSSMAXHP   0x0316u  // 2 bytes

#define SPACE_VIEWCY (-60)
#define SPACE_MINX   (-240)
#define SPACE_MAXX    240
#define SPACEBAR_BASE_DIST 3000
#define SPACEBAR_UNIT_LEN  125

// Kichi base constants from STRATEQU.INC
#define KICHI0_SCALE 6
#define KICHI0_DOOR  (-10 << KICHI0_SCALE)  // -640
#define KICHI2_SCALE 3
#define KICHI2_LEN   (49 << KICHI2_SCALE)   // 392
#define BIGBASEZ     10000

#define STRAT_ADDR_MOTHER1 0x020000u
#define STRAT_ADDR_MOTHER2 0x020001u
#define STRAT_ADDR_MOTHER_SNAKES 0x020002u
#define STRAT_ADDR_SLOWMETEOR 0x030003u
// `spacepilon_Istrat` lives in GA3STRAT.ASM and is not part of the flat
// ISTRATS.ASM index table yet. Keep this literal user explicit until that
// init routine is assigned a real flat strategy binding.
#define STRAT_ADDR_SPACEPILON 0x030004u
// `ship0cdown_Istrat` is a named strategy in GCSTRATS.ASM used by CL_SHIP
// background objects. Keep a synthetic address until the strategy is ported.
#define STRAT_ADDR_SHIP0CDOWN 0x030007u

// SPECIAL.ASM strategy synthetic addresses.
// `pole0_istrat` is a named strategy in KSTRATS.ASM. Keep synthetic until ported.
#define STRAT_ADDR_POLE0       0x050001u
// `theend_*_istrat` are KSHAPES.ASM letter strategies.
#define STRAT_ADDR_THEEND_T    0x050002u
#define STRAT_ADDR_THEEND_H    0x050003u
#define STRAT_ADDR_THEEND_E    0x050004u
#define STRAT_ADDR_THEEND_E2   0x050005u
#define STRAT_ADDR_THEEND_N    0x050006u
#define STRAT_ADDR_THEEND_D    0x050007u

// TRUCKER.ASM strategy synthetic addresses.
// `madbiker_istrat` and `madtrucker_istrat` are in DSTRATS.ASM.
#define STRAT_ADDR_MADBIKER    0x050008u
#define STRAT_ADDR_MADTRUCKER  0x050009u
// `roadline_istrat` is in DSTRATS.ASM.
#define STRAT_ADDR_ROADLINE    0x05000Au

// MAP1_3A1 strategy synthetic addresses.
// `ship1a_Istrat` is a named strategy for fleet ships.
#define STRAT_ADDR_SHIP1A      0x05000Bu
// `ship2_Istrat` is the big ship strategy.
#define STRAT_ADDR_SHIP2       0x05000Cu
// `sdoor1_Istrat`, `sdoor2_Istrat` are ship door strategies.
#define STRAT_ADDR_SDOOR1      0x05000Du
#define STRAT_ADDR_SDOOR2      0x05000Eu

// MAP1_3A2 strategy synthetic addresses.
// `cruiser2_Istrat` is the cruiser patrol strategy.
#define STRAT_ADDR_CRUISER2     0x05000Fu
// `cruiser2fire_Istrat` fires and patrols.
#define STRAT_ADDR_CRUISER2FIRE 0x050010u

// FINALMAP.ASM strategy synthetic addresses.
// `topright1_istrat`, `topleft1_istrat`, `botright1_istrat`, `botleft1_istrat`
// are tunnel section strategies.
#define STRAT_ADDR_TOPRIGHT1    0x050011u
#define STRAT_ADDR_TOPLEFT1     0x050012u
#define STRAT_ADDR_BOTRIGHT1    0x050013u
#define STRAT_ADDR_BOTLEFT1     0x050014u
// `monolith_istrat` is the Andross final boss strategy.
#define STRAT_ADDR_MONOLITH     0x050015u
// `mes_andross1`, `mes_andross2` are Andross message path strategies.
#define STRAT_ADDR_MES_ANDROSS1 0x050016u
#define STRAT_ADDR_MES_ANDROSS2 0x050017u
// `item5_istrat`, `item7_istrat` are item pickup strategies (also used as IS_ITEM5/7).
// For FINALMAP contexts where setalvar sbyte1 follows, use the IS_* ids directly.

// INTRO.ASM strategy synthetic addresses.
// `dintro1` is the intro text path strategy for Nintendo Presents.
#define STRAT_ADDR_DINTRO1      0x050018u
// `playerdownintro_Istrat`, etc. are intro player ship strategies.
#define STRAT_ADDR_PLAYERDOWNINTRO  0x050019u
#define STRAT_ADDR_PLAYERDOWN2INTRO 0x05001Au
#define STRAT_ADDR_PLAYERDOWN3INTRO 0x05001Bu
// `playerfireintro_Istrat` fires during intro.
#define STRAT_ADDR_PLAYERFIREINTRO  0x05001Cu
// `boss7intro_Istrat` is the intro boss approach.
#define STRAT_ADDR_BOSS7INTRO       0x05001Du
// `zacointro_Istrat`, `zaco2intro_Istrat` are intro enemy strategies.
#define STRAT_ADDR_ZACOINTRO        0x05001Eu
#define STRAT_ADDR_ZACO2INTRO       0x05001Fu

// 3-4-T.ASM / 1-3-T tunnel strategy synthetic addresses.
// `openlr_Istrat` is the left-right door strategy.
#define STRAT_ADDR_OPENLR           0x050021u
// `updoor_Istrat` is the up-down door strategy.
#define STRAT_ADDR_UPDOOR           0x050022u
// `warker3_Istrat` is the walking enemy strategy.
#define STRAT_ADDR_WARKER3          0x050023u
// `twall0_Istrat` is the tunnel wall obstacle strategy.
#define STRAT_ADDR_TWALL0           0x050024u

// MAP1_3C strategy synthetic addresses.
// `cruiser1_Istrat` is the near-side cruiser strategy.
#define STRAT_ADDR_CRUISER1         0x050025u
// `cruiser1f_Istrat` is the far cruiser with sbyte1 strategy.
#define STRAT_ADDR_CRUISER1F        0x050026u
// `ship3a_Istrat` is the big ship approach strategy.
#define STRAT_ADDR_SHIP3A           0x050027u
// `ship3_Istrat` is the big ship docking strategy.
#define STRAT_ADDR_SHIP3            0x050028u
// `exitopensnd2_Istrat` is the exit door open with sound strategy.
#define STRAT_ADDR_EXITOPENSND2     0x050029u
// `gate_Istrat` as used in MAP1_3C.
#define STRAT_ADDR_GATE3            IS_GATE

// TRAINING.ASM strategy synthetic addresses.
// `groundpilon_Istrat` is the ground pylon strategy.
#define STRAT_ADDR_GROUNDPILON      0x05002Au
// `base_1_Istrat` is used for the base object.
#define STRAT_ADDR_BASE_1_TRN       IS_BASE_1

// CREDITS.ASM strategy synthetic addresses.
// `dstarfox` is the Star Fox logo path. Synthetic path placeholder.
// `dpresented`, `dnintendo` are credits text path placeholders.
// `dsideslip` is the credits text sideslip path.
// These are assigned as path IDs in path_literals.h, not strategies.

// TITLE.ASM strategy synthetic addresses.
// `tit_istrat` is the title screen strategy.
#define STRAT_ADDR_TIT              0x050020u

// MAP3_7A / MAP1_6A (Venom surface) strategy IDs
// `walll_ISTRAT` is the left-moving wall strategy. Proxy through IS_HARD180YR.
#define IS_WALLL            IS_HARD180YR
// `wallr_ISTRAT` is the right-moving wall strategy. Proxy through IS_HARD180YR.
#define IS_WALLR            IS_HARD180YR
// `wallleftright_ISTRAT` is the random left/right wall strategy. Proxy through IS_HARD180YR.
#define IS_WALLLEFTRIGHT    IS_HARD180YR
// `flypillars_ISTRAT` is the flying pillar strategy. Proxy through IS_PILLAR3.
#define IS_FLYPILLARS       IS_PILLAR3

// TRANSFOR.ASM (Venom 3 Surface Boss) strategy IDs
// `airship_istrat` is the Transformer/Great Commander boss strategy. Synthetic address.
#define STRAT_ADDR_AIRSHIP  0x05002Bu

// BGM ids for FINALMAP.
#define BGM_BOSS_FINAL  0x13u
#define BGM_FINAL_CONT  0x12u

// Raw shape ids passed through ALX pword1 for carried-log path scripts.
#define SH_RAW_BOSS_7_0 240u
#define SH_RAW_BOSS_7_1 241u
#define SH_RAW_BOSS_7_3 244u

// Local native callback ids used only by the CL_GND literal submap.
#define MAP_CB_CL_GROUND_PRINTLEVELFIN 0x01F101u
#define MAP_CB_CL_GROUND_WIPEOUT       0x01F102u
#define MAP_CB_CL_WARP_PRINTLEVELFIN   0x01F103u
#define MAP_CB_CL_DIVE_CLEAR_ENGINESND 0x01F104u

// ========================================================================
// MOTHERS.ASM — Mother ship sub-map pattern data (bytecode for mother VM)
// ========================================================================
//
// The SNES mother VM is a separate bytecode interpreter run by mother ship
// strategy functions (mother1_istrat, mother2_istrat). Each mother sub-map
// is a byte stream of mother-VM opcodes that tell the mother ship what
// objects to spawn, when to loop, and when to self-destruct.
//
// The mother VM is NOT yet implemented in the C engine. All mapmother calls
// currently pass map_ref=0. These arrays are provided as literal ASM->C
// transcriptions so the data is ready when the VM is ported.
//
// Mother VM opcode constants (from MAPMACS.INC):
#define MO_OBJ    0   // motherobj:  ctrl(1)+frame(2)+x(2)+y(2)+z(2)+shape(2)+strat(3) = 14 bytes
#define MO_LOOP   2   // motherloop: ctrl(1)+frame(2)+addr(2)+count(1)                  = 6 bytes
#define MO_END    4   // motherend:  ctrl(1)                                             = 1 byte
#define MO_RND    6   // motherrnd:  ctrl(1)+frame(2)+x(2)+y(2)+z(2)+shape(2)+strat(3) = 14 bytes
#define MO_GOTO   8   // mothergoto: ctrl(1)+frame(2)+addr(2)                           = 5 bytes
#define MO_WAIT  10   // motherwait: ctrl(1)+frame(2)                                   = 3 bytes
#define MO_COUNT 12   // mothercnt:  ctrl(1)+frame(2)+shape(2)                          = 5 bytes
#define MO_JUMP  14   // motherjump: ctrl(1)+frame(2)+val(2)+addr(2)+func(1)            = 8 bytes

// motherjump condition codes
#define MJ_EQ 0
#define MJ_NE 1
#define MJ_GT 2
#define MJ_LT 3

// Helper macros for building mother sub-map byte arrays.
// All multi-byte values are little-endian (matching SNES byte order).
#define MO_U16(v)  ((uint8)((v) & 0xFF)), ((uint8)(((v) >> 8) & 0xFF))
#define MO_S16(v)  MO_U16((uint16)(int16)(v))
#define MO_U24(v)  ((uint8)((v) & 0xFF)), ((uint8)(((v) >> 8) & 0xFF)), ((uint8)(((v) >> 16) & 0xFF))

// Strategy addresses referenced by MOTHERS.ASM sub-maps.
// These are raw SNES code pointers; none are in the flat ISTRATS.ASM table.
// Use synthetic address space 0x04xxxx for mother-internal strategies.
#define MOSTRAT_HARD           0x040001u  // hard_istrat
#define MOSTRAT_CUBEFALL       0x040002u  // cubefall_istrat
#define MOSTRAT_PARA           0x040003u  // para_istrat (mother-spawned para)
#define MOSTRAT_METEOR         0x040004u  // meteor_istrat
#define MOSTRAT_MINE0          0x040005u  // mine0_istrat
#define MOSTRAT_PILLAR2        0x040006u  // pillar2_istrat
#define MOSTRAT_FLYPILLAR      0x040007u  // flypillar_istrat
#define MOSTRAT_LARGEPLASMA    0x040008u  // largeplasma_istrat
#define MOSTRAT_SPEEDLINES     0x040009u  // speedlines_istrat
#define MOSTRAT_ROADLINE       0x04000Au  // roadline_istrat
#define MOSTRAT_TOPRIGHT1      0x04000Bu  // topright1_istrat
#define MOSTRAT_TOPLEFT1       0x04000Cu  // topleft1_istrat
#define MOSTRAT_BOTRIGHT1      0x04000Du  // botright1_istrat
#define MOSTRAT_BOTLEFT1       0x04000Eu  // botleft1_istrat
#define MOSTRAT_RIGHTWALL      0x04000Fu  // rightwall_istrat
#define MOSTRAT_LEFTWALL       0x040010u  // leftwall_istrat
#define MOSTRAT_DUCT           0x040011u  // duct_istrat
#define MOSTRAT_NOCOLL         0x040012u  // nocoll_istrat (mother-spawned)
#define MOSTRAT_SLOWMETEOR     0x040013u  // slowmeteor_istrat
#define MOSTRAT_BREAK_METEOR   0x040014u  // break_meteor_istrat
#define MOSTRAT_SEARCHMETEOR   0x040015u  // searchmeteor_istrat
#define MOSTRAT_AMOEBA         0x040016u  // amoeba_istrat
#define MOSTRAT_UPERM          0x040017u  // uperm_Istrat
#define MOSTRAT_SHOU0A         0x040018u  // shou0a_istrat
#define MOSTRAT_SHOU0          0x040019u  // shou0_istrat
#define MOSTRAT_METEO0         0x04001Au  // meteo0_Istrat
#define MOSTRAT_SEADRAGON      0x04001Bu  // seadragon_istrat
#define MOSTRAT_MINE2          0x04001Cu  // mine2_istrat
#define MOSTRAT_DAMYSCR        0x04001Du  // damyscr_istrat
#define MOSTRAT_CLASTEROID     0x04001Eu  // clasteroid_Istrat
#define MOSTRAT_HARD180YR      0x04001Fu  // HARD180YR_ISTRAT

// Mother sub-map label offsets (byte offset into each array).
// Used by mothergoto/motherloop to reference loop targets.
// Offsets are relative to the start of each sub-map array.

// mothermap1: motherrnd(14) -> mothergoto back to 0
static const uint8 s_mother_map1[] = {
    MO_RND,  MO_U16(800), MO_U16(512), MO_U16(0), MO_U16(2048),
             MO_U16(SH_PETECUBE_PROXY), MO_U24(MOSTRAT_HARD),
    MO_GOTO, MO_U16(0), MO_U16(0),  // -> mothermap1 (offset 0)
};

// mothermap2: complex loop with cubefall + asteroid3/4
// Layout: [0] motherrnd(14), [14] motherloop(6) -> 0 x10,
//         [20] motherwait(3), [23]=.mloop: motherrnd(14), [37] motherrnd(14),
//         [51] motherloop(6) -> 23 x5, [57] motherwait(3),
//         [60] mothergoto(5) -> 0
static const uint8 s_mother_map2[] = {
    // motherrnd 0250,512,0,512,bouncyball,cubefall_istrat
    MO_RND,  MO_U16(250), MO_U16(512), MO_U16(0), MO_U16(512),
             MO_U16(SH_BOUNCYBALL_PROXY), MO_U24(MOSTRAT_CUBEFALL),
    // motherloop 0,mothermap2,10
    MO_LOOP, MO_U16(0), MO_U16(0), 10,
    // motherwait 250
    MO_WAIT, MO_U16(250),
    // .mloop (offset 23):
    // motherrnd 0250,512,0,512,asteroid3,cubefall_istrat
    MO_RND,  MO_U16(250), MO_U16(512), MO_U16(0), MO_U16(512),
             MO_U16(SH_ASTEROID3_PROXY), MO_U24(MOSTRAT_CUBEFALL),
    // motherrnd 0250,512,0,512,asteroid4,cubefall_istrat
    MO_RND,  MO_U16(250), MO_U16(512), MO_U16(0), MO_U16(512),
             MO_U16(SH_ASTEROID4_PROXY), MO_U24(MOSTRAT_CUBEFALL),
    // motherloop 0,.mloop,5
    MO_LOOP, MO_U16(0), MO_U16(23), 5,
    // motherwait 250
    MO_WAIT, MO_U16(250),
    // mothergoto 0,mothermap2
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// mothermap3: para wave with loop and wait
// [0] motherobj(14), [14] motherloop(6) -> 0 x8, [20] motherwait(3),
// [23] motherend(1) -> back to 0? No: "motherend 0,mothermap3" deletes mother.
// Actually ASM says: motherend 0,mothermap3 - but motherend takes no args in
// the macro (it just deletes the alien). The "0,mothermap3" may be a macro
// variant. Looking at MAPMACS.INC: motherend just emits ctrlmotherend (1 byte).
static const uint8 s_mother_map3[] = {
    // motherobj 1200,0,160,0,para_0,para_istrat
    MO_OBJ,  MO_U16(1200), MO_S16(0), MO_S16(160), MO_S16(0),
             MO_U16(SH_PARA_0), MO_U24(MOSTRAT_PARA),
    // motherloop 0,mothermap3,8
    MO_LOOP, MO_U16(0), MO_U16(0), 8,
    // motherwait 2000
    MO_WAIT, MO_U16(2000),
    // motherend 0,mothermap3  (motherend macro ignores args; just 1 byte)
    MO_END,
};

// mothermap4: asterdist=150, endless asteroid1 meteor loop
static const uint8 s_mother_map4[] = {
    // motherrnd asterdist,1024,1024,0000,asteroid1,meteor_istrat
    MO_RND,  MO_U16(150), MO_U16(1024), MO_U16(1024), MO_U16(0),
             MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    // mothergoto 0,mothermap4
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// mothermap5: bouncyball meteor loop with count-based conditional exit
// [0] motherrnd(14), [14]=.wait: mothercnt(5), [19] motherjump(8) -> 14,
// [27] mothergoto(5) -> 0
static const uint8 s_mother_map5[] = {
    // motherrnd 500,128,128,0000,bouncyball,meteor_istrat
    MO_RND,  MO_U16(500), MO_U16(128), MO_U16(128), MO_U16(0),
             MO_U16(SH_BOUNCYBALL_PROXY), MO_U24(MOSTRAT_METEOR),
    // .wait (offset 14):
    // mothercnt 100,iris
    MO_COUNT, MO_U16(100), MO_U16(SH_IRIS),
    // motherjump 0,NE,.wait
    MO_JUMP, MO_U16(0), MO_U16(0), MO_U16(14), MJ_NE,
    // mothergoto mothermap5  (1-arg form: frame=0, addr=start)
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_ast: dense asteroid field with asteroid3 variant, loops after initial burst
// [0..83]: 6x motherrnd asteroid1 (6*14=84)
// [84]=.here: 4x motherrnd (asteroid3 + 3x asteroid1) (4*14=56) [84..139]
// [140..195]: 3x motherrnd asteroid1 (3*14=42) [140..181]
// [182..237]: 3x motherrnd asteroid1 (3*14=42) [182..223]
// [224]: mothergoto -> .here(84)
// Actually let me recount: each motherrnd is 14 bytes.
// 0: rnd(14), 14: rnd(14), 28: rnd(14), 42: rnd(14), 56: rnd(14), 70: rnd(14)
// 84=.here: rnd(14)=84, rnd(14)=98, rnd(14)=112, rnd(14)=126
// 140: rnd(14), 154: rnd(14), 168: rnd(14)
// 182: rnd(14), 196: rnd(14), 210: rnd(14)
// 224: mothergoto(5) -> 84
static const uint8 s_mother_map_ast[] = {
    // 6x motherrnd 0200,1024,1024,0000,asteroid1,meteor_istrat
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    // .here (offset 84):
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID3_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    // next 3
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    // next 3
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    // mothergoto 0,.here
    MO_GOTO, MO_U16(0), MO_U16(84),
};

// map_mines: single mine spawner, endless loop
static const uint8 s_mother_map_mines[] = {
    MO_RND,  MO_U16(150), MO_U16(512), MO_U16(512), MO_U16(256),
             MO_U16(SH_MINE_0_PROXY), MO_U24(MOSTRAT_MINE0),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_pillars: pillar2 wave pattern, 8 objects then loop
// 8x motherobj(14) = 112, then mothergoto(5) at offset 112
static const uint8 s_mother_map_pillars[] = {
    MO_OBJ, MO_U16(100), MO_S16(-500), MO_S16(0), MO_S16(0),
            MO_U16(SH_PILLAR3), MO_U24(MOSTRAT_PILLAR2),
    MO_OBJ, MO_U16(100), MO_S16(-250), MO_S16(0), MO_S16(0),
            MO_U16(SH_PILLAR3), MO_U24(MOSTRAT_PILLAR2),
    MO_OBJ, MO_U16(100), MO_S16(0), MO_S16(0), MO_S16(0),
            MO_U16(SH_PILLAR3), MO_U24(MOSTRAT_PILLAR2),
    MO_OBJ, MO_U16(100), MO_S16(250), MO_S16(0), MO_S16(0),
            MO_U16(SH_PILLAR3), MO_U24(MOSTRAT_PILLAR2),
    MO_OBJ, MO_U16(100), MO_S16(500), MO_S16(0), MO_S16(0),
            MO_U16(SH_PILLAR3), MO_U24(MOSTRAT_PILLAR2),
    MO_OBJ, MO_U16(100), MO_S16(250), MO_S16(0), MO_S16(0),
            MO_U16(SH_PILLAR3), MO_U24(MOSTRAT_PILLAR2),
    MO_OBJ, MO_U16(100), MO_S16(0), MO_S16(0), MO_S16(0),
            MO_U16(SH_PILLAR3), MO_U24(MOSTRAT_PILLAR2),
    MO_OBJ, MO_U16(100), MO_S16(-250), MO_S16(0), MO_S16(0),
            MO_U16(SH_PILLAR3), MO_U24(MOSTRAT_PILLAR2),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_mountains: empty loop (just a goto back to itself)
static const uint8 s_mother_map_mountains[] = {
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_flypillars: single flying pillar, endless loop
// Note: -150*2 = -300 in the ASM
static const uint8 s_mother_map_flypillars[] = {
    MO_OBJ,  MO_U16(600), MO_S16(-300), MO_S16(-300), MO_S16(-4100),
             MO_U16(SH_RPILLAR3_PROXY), MO_U24(MOSTRAT_FLYPILLAR),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_plasmas: 3 large plasma spawners, endless loop
static const uint8 s_mother_map_plasmas[] = {
    MO_RND, MO_U16(500), MO_U16(256), MO_U16(256), MO_U16(0),
            MO_U16(SH_NULLSHAPE), MO_U24(MOSTRAT_LARGEPLASMA),
    MO_RND, MO_U16(800), MO_U16(256), MO_U16(256), MO_U16(0),
            MO_U16(SH_NULLSHAPE), MO_U24(MOSTRAT_LARGEPLASMA),
    MO_RND, MO_U16(600), MO_U16(256), MO_U16(256), MO_U16(0),
            MO_U16(SH_NULLSHAPE), MO_U24(MOSTRAT_LARGEPLASMA),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_lines: speed lines effect, endless loop
static const uint8 s_mother_map_lines[] = {
    MO_RND, MO_U16(5), MO_U16(256), MO_U16(256), MO_U16(0),
            MO_U16(SH_LINE3_PROXY), MO_U24(MOSTRAT_SPEEDLINES),
    MO_RND, MO_U16(2), MO_U16(256), MO_U16(256), MO_U16(0),
            MO_U16(SH_LINE3_PROXY), MO_U24(MOSTRAT_SPEEDLINES),
    MO_RND, MO_U16(5), MO_U16(256), MO_U16(256), MO_U16(0),
            MO_U16(SH_LINE3_PROXY), MO_U24(MOSTRAT_SPEEDLINES),
    MO_RND, MO_U16(2), MO_U16(256), MO_U16(256), MO_U16(0),
            MO_U16(SH_LINE3_PROXY), MO_U24(MOSTRAT_SPEEDLINES),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_roadlines: road line objects, endless loop
// 5x motherobj(14) = 70, then mothergoto(5) at offset 70
static const uint8 s_mother_map_roadlines[] = {
    MO_OBJ, MO_U16(200), MO_S16(0), MO_S16(0), MO_S16(0),
            MO_U16(SH_LINE_0_PROXY), MO_U24(MOSTRAT_ROADLINE),
    MO_OBJ, MO_U16(200), MO_S16(0), MO_S16(0), MO_S16(0),
            MO_U16(SH_LINE_0_PROXY), MO_U24(MOSTRAT_ROADLINE),
    MO_OBJ, MO_U16(200), MO_S16(0), MO_S16(0), MO_S16(0),
            MO_U16(SH_LINE_0_PROXY), MO_U24(MOSTRAT_ROADLINE),
    MO_OBJ, MO_U16(100), MO_S16(0), MO_S16(0), MO_S16(0),
            MO_U16(SH_LINE_0_PROXY), MO_U24(MOSTRAT_ROADLINE),
    MO_OBJ, MO_U16(100), MO_S16(0), MO_S16(0), MO_S16(0),
            MO_U16(SH_LINE_1_PROXY), MO_U24(MOSTRAT_ROADLINE),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// truckmines_map: 4 mines then end
static const uint8 s_mother_truckmines_map[] = {
    MO_RND, MO_U16(0), MO_U16(64), MO_U16(64), MO_U16(64),
            MO_U16(SH_MINE_0_PROXY), MO_U24(MOSTRAT_MINE0),
    MO_RND, MO_U16(0), MO_U16(64), MO_U16(64), MO_U16(64),
            MO_U16(SH_MINE_0_PROXY), MO_U24(MOSTRAT_MINE0),
    MO_RND, MO_U16(0), MO_U16(64), MO_U16(64), MO_U16(64),
            MO_U16(SH_MINE_0_PROXY), MO_U24(MOSTRAT_MINE0),
    MO_RND, MO_U16(0), MO_U16(64), MO_U16(64), MO_U16(64),
            MO_U16(SH_MINE_0_PROXY), MO_U24(MOSTRAT_MINE0),
    MO_END,
};

// largetunnelbits: tunnel section spawner with walls and ducts
// 4x motherobj(14)=56, motherwait(3)=59, 4x motherobj(14)=115,
// motherwait(3)=118, 3x motherobj(14)=160, mothergoto(5)=165
static const uint8 s_mother_largetunnelbits[] = {
    MO_OBJ, MO_U16(0), MO_S16(120), MO_S16(-120), MO_S16(0),
            MO_U16(SH_TUNNEL_0), MO_U24(MOSTRAT_TOPRIGHT1),
    MO_OBJ, MO_U16(0), MO_S16(-120), MO_S16(-120), MO_S16(0),
            MO_U16(SH_TUNNEL_0), MO_U24(MOSTRAT_TOPLEFT1),
    MO_OBJ, MO_U16(0), MO_S16(120), MO_S16(0), MO_S16(0),
            MO_U16(SH_TUNNEL_0), MO_U24(MOSTRAT_BOTRIGHT1),
    MO_OBJ, MO_U16(0), MO_S16(-120), MO_S16(0), MO_S16(0),
            MO_U16(SH_TUNNEL_0), MO_U24(MOSTRAT_BOTLEFT1),
    MO_WAIT, MO_U16(600),
    MO_OBJ, MO_U16(0), MO_S16(120), MO_S16(-120), MO_S16(0),
            MO_U16(SH_TUNNEL_0), MO_U24(MOSTRAT_TOPRIGHT1),
    MO_OBJ, MO_U16(0), MO_S16(-120), MO_S16(-120), MO_S16(0),
            MO_U16(SH_TUNNEL_0), MO_U24(MOSTRAT_TOPLEFT1),
    MO_OBJ, MO_U16(0), MO_S16(120), MO_S16(0), MO_S16(0),
            MO_U16(SH_TUNNEL_0), MO_U24(MOSTRAT_BOTRIGHT1),
    MO_OBJ, MO_U16(0), MO_S16(-120), MO_S16(0), MO_S16(0),
            MO_U16(SH_TUNNEL_0), MO_U24(MOSTRAT_BOTLEFT1),
    MO_WAIT, MO_U16(600),
    MO_OBJ, MO_U16(0), MO_S16(-120), MO_S16(0), MO_S16(0),
            MO_U16(SH_TUNNEL_4), MO_U24(MOSTRAT_LEFTWALL),
    MO_OBJ, MO_U16(0), MO_S16(120), MO_S16(0), MO_S16(0),
            MO_U16(SH_TUNNEL_4), MO_U24(MOSTRAT_RIGHTWALL),
    MO_OBJ, MO_U16(0), MO_S16(0), MO_S16(-120), MO_S16(0),
            MO_U16(SH_TUNNEL_7), MO_U24(MOSTRAT_DUCT),
    MO_GOTO, MO_U16(600), MO_U16(0),
};

// tunnellines: line objects in tunnel, endless loop
static const uint8 s_mother_tunnellines[] = {
    MO_OBJ,  MO_U16(800), MO_S16(0), MO_S16(0), MO_S16(0),
             MO_U16(SH_LINE_0_PROXY), MO_U24(MOSTRAT_NOCOLL),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// mother_0: asteroid field (asterdist=150), 3 asteroids then loop
static const uint8 s_mother_0[] = {
    MO_RND, MO_U16(150), MO_U16(2048), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(150), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(150), MO_U16(2048), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// mother_1: asterdist=500, slow meteors with one break_meteor
static const uint8 s_mother_1[] = {
    MO_RND, MO_U16(500), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_RND, MO_U16(500), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_BREAK_METEOR),
    MO_RND, MO_U16(500), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_RND, MO_U16(500), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// mother_2: asterdist=500, hard asteroids with break_meteor
static const uint8 s_mother_2[] = {
    MO_RND, MO_U16(500), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_HARD),
    MO_RND, MO_U16(500), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_HARD),
    MO_RND, MO_U16(800), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_BREAK_METEOR),
    MO_RND, MO_U16(500), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_HARD),
    MO_RND, MO_U16(500), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_HARD),
    MO_RND, MO_U16(800), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_BREAK_METEOR),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// mother_3: asterdist=100, slow/break meteors
static const uint8 s_mother_3[] = {
    MO_RND, MO_U16(100), MO_U16(2048), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_RND, MO_U16(100), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_BREAK_METEOR),
    MO_RND, MO_U16(100), MO_U16(2048), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_RND, MO_U16(100), MO_U16(2048), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// mother_5: asterdist=250, mixed slow/break/search meteors with asteroid2
static const uint8 s_mother_5[] = {
    MO_RND, MO_U16(250), MO_U16(2048), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID2), MO_U24(MOSTRAT_SEARCHMETEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_RND, MO_U16(250), MO_U16(2048), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_BREAK_METEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_SLOWMETEOR),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// testmother: asterdist=250, single random asteroid, endless
static const uint8 s_mother_testmother[] = {
    MO_RND,  MO_U16(250), MO_U16(256), MO_U16(2048), MO_U16(0),
             MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_amoebas: asterdist=250, single amoeba spawner, endless
static const uint8 s_mother_map_amoebas[] = {
    MO_RND,  MO_U16(250), MO_U16(1024), MO_U16(1024), MO_U16(0),
             MO_U16(SH_AMOEBA2), MO_U24(MOSTRAT_AMOEBA),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_uperm: uper_m spawner, endless
static const uint8 s_mother_map_uperm[] = {
    MO_RND,  MO_U16(1500), MO_U16(1024), MO_U16(0), MO_U16(0),
             MO_U16(SH_UPER_M), MO_U24(MOSTRAT_UPERM),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_shou0: mixed asteroid1/r_hou_0/s_hou_0, 9 entries then loop
static const uint8 s_mother_map_shou0[] = {
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_R_HOU_0), MO_U24(MOSTRAT_SHOU0A),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_R_HOU_0), MO_U24(MOSTRAT_SHOU0A),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_S_HOU_0), MO_U24(MOSTRAT_SHOU0),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_meteo0: mixed asteroid1/meteo_0 with break_meteor
static const uint8 s_mother_map_meteo0[] = {
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(512), MO_U16(0),
            MO_U16(SH_METEO_0), MO_U24(MOSTRAT_METEO0),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(1024), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_BREAK_METEOR),
    MO_RND, MO_U16(250), MO_U16(1024), MO_U16(2048), MO_U16(0),
            MO_U16(SH_ASTEROID1_PROXY), MO_U24(MOSTRAT_METEOR),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// mother_snakes: snakedist=500, sea dragon spawner, endless
static const uint8 s_mother_snakes[] = {
    MO_RND,  MO_U16(500), MO_U16(1024), MO_U16(0), MO_U16(256),
             MO_U16(SH_NULLSHAPE), MO_U24(MOSTRAT_SEADRAGON),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_mine2: uper_m + mine_2 spawners, endless
static const uint8 s_mother_map_mine2[] = {
    MO_RND, MO_U16(1500), MO_U16(1024), MO_U16(0), MO_U16(0),
            MO_U16(SH_UPER_M), MO_U24(MOSTRAT_UPERM),
    MO_RND, MO_U16(1500), MO_U16(1024), MO_U16(256), MO_U16(0),
            MO_U16(SH_MINE_2), MO_U24(MOSTRAT_MINE2),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// map_bhole: black hole damage scroller, endless
static const uint8 s_mother_map_bhole[] = {
    MO_RND,  MO_U16(800), MO_U16(1024), MO_U16(1024), MO_U16(4000),
             MO_U16(SH_NULLSHAPE), MO_U24(MOSTRAT_DAMYSCR),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// mother_CLasteroids: colony asteroid spawner, endless
static const uint8 s_mother_CLasteroids[] = {
    MO_RND,  MO_U16(200), MO_U16(1024), MO_U16(1024), MO_U16(0),
             MO_U16(SH_CLASTEROID_PROXY), MO_U24(MOSTRAT_CLASTEROID),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// mother_clbuildings: colony building pattern, 4 buildings then loop
static const uint8 s_mother_clbuildings[] = {
    MO_OBJ, MO_U16(1000), MO_S16(-800), MO_S16(0), MO_S16(4000),
            MO_U16(SH_BU_5), MO_U24(MOSTRAT_HARD180YR),
    MO_OBJ, MO_U16(1000), MO_S16(1200), MO_S16(0), MO_S16(4000),
            MO_U16(SH_BU_6), MO_U24(MOSTRAT_HARD180YR),
    MO_OBJ, MO_U16(1000), MO_S16(-1200), MO_S16(0), MO_S16(4000),
            MO_U16(SH_BU_4), MO_U24(MOSTRAT_HARD180YR),
    MO_OBJ, MO_U16(1000), MO_S16(1000), MO_S16(0), MO_S16(4000),
            MO_U16(SH_BU_2), MO_U24(MOSTRAT_HARD180YR),
    MO_GOTO, MO_U16(0), MO_U16(0),
};

// End of MOTHERS.ASM data
// ========================================================================

typedef struct {
    char name[64];
    uint16 offset;
} MapLabel;

typedef struct {
    char label[64];
    uint16 offset;
} MapFixup;

typedef enum {
    MAP_BARSHAPE_WIRE = 0,
    MAP_BARSHAPE_SOLID = 1,
} MapBarShapeMode;

typedef enum {
    MAP_SPACEBAR_SHAPE_X = 0,
    MAP_SPACEBAR_SHAPE_XP,
    MAP_SPACEBAR_SHAPE_Y,
    MAP_SPACEBAR_SHAPE_Z,
    MAP_SPACEBAR_SHAPE_SX,
    MAP_SPACEBAR_SHAPE_SXP,
    MAP_SPACEBAR_SHAPE_SY,
    MAP_SPACEBAR_SHAPE_SZ,
} MapSpacebarShape;

typedef struct {
    uint8 *data;
    uint32 capacity;
    uint32 length;
    MapLabel labels[MAP_LABEL_CAPACITY];
    uint16 label_count;
    MapFixup fixups[MAP_FIXUP_CAPACITY];
    uint16 fixup_count;
    bool failed;
    MapBarShapeMode barshape_mode;
    bool barshape_autowait;
    int16 barshape_swait;
    int16 barshape_pos;
} MapBuilder;

static const uint8 s_map_end_only[] = { MAP_OP_END };
static const MapLevelData s_empty_level = {
    s_map_end_only,
    1u,
};

static uint8 s_level1_1_data[MAP_DATA_CAPACITY];
static MapLevelData s_level1_1 = {
    s_map_end_only,
    1u,
};
static uint8 s_level1_2_data[MAP_DATA_CAPACITY];
static MapLevelData s_level1_2 = {
    s_map_end_only,
    1u,
};
static uint8 s_level1_3_data[MAP_DATA_CAPACITY];
static MapLevelData s_level1_3 = {
    s_map_end_only,
    1u,
};
static uint8 s_level1_4_data[MAP_DATA_CAPACITY * 2];
static MapLevelData s_level1_4 = {
    s_map_end_only,
    1u,
};
static uint8 s_level2_1_data[MAP_DATA_CAPACITY * 2];
static MapLevelData s_level2_1 = {
    s_map_end_only,
    1u,
};
static uint8 s_level2_2_data[MAP_DATA_CAPACITY];
static MapLevelData s_level2_2 = {
    s_map_end_only,
    1u,
};
static uint8 s_level3_1_data[MAP_DATA_CAPACITY * 2];
static MapLevelData s_level3_1 = {
    s_map_end_only,
    1u,
};
static uint8 s_level3_2_data[MAP_DATA_CAPACITY];
static MapLevelData s_level3_2 = {
    s_map_end_only,
    1u,
};
static uint8 s_level2_3_data[MAP_DATA_CAPACITY];
static MapLevelData s_level2_3 = {
    s_map_end_only,
    1u,
};
static uint8 s_level2_4_data[MAP_DATA_CAPACITY];
static MapLevelData s_level2_4 = {
    s_map_end_only,
    1u,
};
static uint8 s_level2_5_data[MAP_DATA_CAPACITY];
static MapLevelData s_level2_5 = {
    s_map_end_only,
    1u,
};
static uint8 s_level3_3_data[MAP_DATA_CAPACITY];
static MapLevelData s_level3_3 = {
    s_map_end_only,
    1u,
};
static uint8 s_level3_5_data[MAP_DATA_CAPACITY * 2];
static MapLevelData s_level3_5 = {
    s_map_end_only,
    1u,
};
static uint8 s_level1_5_data[MAP_DATA_CAPACITY * 2];
static MapLevelData s_level1_5 = {
    s_map_end_only,
    1u,
};
static uint8 s_level3_6_data[MAP_DATA_CAPACITY * 2];
static MapLevelData s_level3_6 = {
    s_map_end_only,
    1u,
};
static uint8 s_level1_6_data[MAP_DATA_CAPACITY * 4];
static MapLevelData s_level1_6 = {
    s_map_end_only,
    1u,
};
static uint8 s_level3_7_data[MAP_DATA_CAPACITY * 5];
static MapLevelData s_level3_7 = {
    s_map_end_only,
    1u,
};
static uint8 s_level2_6_data[MAP_DATA_CAPACITY];
static MapLevelData s_level2_6 = {
    s_map_end_only,
    1u,
};
static uint8 s_level_bh_data_buf[MAP_DATA_CAPACITY];
static MapLevelData s_level_bh = {
    s_map_end_only,
    1u,
};
static uint8 s_level3_4_data[MAP_DATA_CAPACITY];
static MapLevelData s_level3_4 = {
    s_map_end_only,
    1u,
};
static uint8 s_level_special_data[MAP_DATA_CAPACITY];
static MapLevelData s_level_special = {
    s_map_end_only,
    1u,
};
static uint8 s_final_data[MAP_DATA_CAPACITY * 3];
static MapLevelData s_final = {
    s_map_end_only,
    1u,
};
static uint8 s_intro_data[MAP_DATA_CAPACITY];
static MapLevelData s_intro = {
    s_map_end_only,
    1u,
};
static uint8 s_title_data[MAP_DATA_CAPACITY];
static MapLevelData s_title = {
    s_map_end_only,
    1u,
};
static uint8 s_planet_data[MAP_DATA_CAPACITY];
static MapLevelData s_planet = {
    s_map_end_only,
    1u,
};
static uint8 s_credits_data[MAP_DATA_CAPACITY];
static MapLevelData s_credits = {
    s_map_end_only,
    1u,
};
static uint8 s_training_data[MAP_DATA_CAPACITY];
static MapLevelData s_training = {
    s_map_end_only,
    1u,
};
static uint16 s_level1_1_skillfly_bonus_guard_script_ptr;
static uint16 s_level1_1_skillfly_bonus_skip_ptr;
static uint16 s_level1_2_skillfly_bonus_guard_script_ptr;
static uint16 s_level1_2_skillfly_bonus_skip_ptr;
static uint16 s_level1_2_blackhole_bonus_guard_script_ptr;
static uint16 s_level1_2_blackhole_bonus_skip_ptr;
static uint16 s_level1_2_mapwaitboss_trigse_script_ptr;
static uint16 s_level1_2_mapwaitboss_cantdie_script_ptr;
static uint16 s_level1_2_mapwaitboss_cleanup_script_ptr;
static uint16 s_level2_1_mapwaitboss_trigse_script_ptr;
static uint16 s_level2_1_mapwaitboss_cantdie_script_ptr;
static uint16 s_level2_1_mapwaitboss_cleanup_script_ptr;
static uint16 s_level2_1_skillfly_bonus0_guard_script_ptr;
static uint16 s_level2_1_skillfly_bonus0_skip_ptr;
static uint16 s_level2_1_skillfly_bonus1_guard_script_ptr;
static uint16 s_level2_1_skillfly_bonus1_skip_ptr;
static uint16 s_level2_2_mapwaitboss_trigse_script_ptr;
static uint16 s_level2_2_mapwaitboss_cantdie_script_ptr;
static uint16 s_level2_2_mapwaitboss_cleanup_script_ptr;
static uint16 s_level2_3_skillfly_bonus_guard_script_ptr;
static uint16 s_level2_3_skillfly_bonus_skip_ptr;
static uint16 s_level2_3_fog_guard_script_ptr;
static uint16 s_level2_3_fog_guard_continue_ptr;
static uint16 s_level2_3_setvar_inline_script_ptr;
static uint16 s_level2_3b_trigger_script_ptr;
static uint16 s_level2_3b_trigger_carryon_ptr;
static uint16 s_level2_3b_trigger_waitabit_ptr;
static uint16 s_level2_3b_seatest_script_ptr;
static uint16 s_level2_3b_seatest_loop_ptr;
static uint16 s_level2_3b_mapwaitboss_cantdie_script_ptr;
static uint16 s_level2_3b_mapwaitboss_cleanup_script_ptr;
static uint16 s_level2_3c_trigse_script_ptr;
static uint16 s_level2_4_mapwaitboss_trigse_script_ptr;
static uint16 s_level2_4_mapwaitboss_cantdie_script_ptr;
static uint16 s_level2_4_mapwaitboss_cleanup_script_ptr;
static uint16 s_level2_5_skillfly_bonus0_guard_script_ptr;
static uint16 s_level2_5_skillfly_bonus0_skip_ptr;
static uint16 s_level2_5_skillfly_bonus1_guard_script_ptr;
static uint16 s_level2_5_skillfly_bonus1_skip_ptr;
static uint16 s_level2_5_mapwaitboss_trigse_script_ptr;
static uint16 s_level2_5_mapwaitboss_cantdie_script_ptr;
static uint16 s_level2_5_mapwaitboss_cleanup_script_ptr;
static uint16 s_level3_1_mapwaitboss_trigse_script_ptr;
static uint16 s_level3_1_mapwaitboss_cantdie_script_ptr;
static uint16 s_level3_1_mapwaitboss_cleanup_script_ptr;
static uint16 s_level3_1_skillfly_bonus0_guard_script_ptr;
static uint16 s_level3_1_skillfly_bonus0_skip_ptr;
static uint16 s_level3_2_skillfly_bonus_guard_script_ptr;
static uint16 s_level3_2_skillfly_bonus_skip_ptr;
static uint16 s_level3_2_mapwaitboss_trigse_script_ptr;
static uint16 s_level3_2_mapwaitboss_cantdie_script_ptr;
static uint16 s_level3_2_mapwaitboss_cleanup_script_ptr;
static uint16 s_level1_1_keep_player_strat_script_ptr;
static uint16 s_level1_1_mapwaitboss_trigse_script_ptr;
static uint16 s_level1_1_mapwaitboss_cantdie_script_ptr;
static uint16 s_level1_1_mapwaitboss_cleanup_script_ptr;
static uint16 s_level3_3_skillfly_bonus0_guard_script_ptr;
static uint16 s_level3_3_skillfly_bonus0_skip_ptr;
static uint16 s_level3_3_mapwaitboss_trigse_script_ptr;
static uint16 s_level3_3_mapwaitboss_cantdie_script_ptr;
static uint16 s_level3_3_mapwaitboss_cleanup_script_ptr;
static uint16 s_level3_3_pdead2_script_ptr;
static uint16 s_level3_3_pdead_script_ptr;
// MAP1_4 (Asteroid Belt 2) inline callback pointers
static uint16 s_level1_4_mapwaitboss_trigse_script_ptr;
static uint16 s_level1_4_mapwaitboss_cantdie_script_ptr;
static uint16 s_level1_4_mapwaitboss_cleanup_script_ptr;
// MAP3_5 (Venom 1 Surface) inline callback pointers
static uint16 s_level3_5_mapwaitboss_trigse_script_ptr;
static uint16 s_level3_5_mapwaitboss_cantdie_script_ptr;
static uint16 s_level3_5_mapwaitboss_cleanup_script_ptr;
static uint16 s_level3_5_skillfly_bonus_guard_script_ptr;
static uint16 s_level3_5_skillfly_bonus_skip_ptr;
static uint16 s_level2_1_keep_player_strat_script_ptr;
static uint16 s_level3_1_keep_player_strat_script_ptr;
// MAP3_4B (Sector Z Part B) inline callback pointers
static uint16 s_level3_4_skillfly_bonus0_guard_script_ptr;
static uint16 s_level3_4_skillfly_bonus0_skip_ptr;
static uint16 s_level3_4_skillfly_bonus1_guard_script_ptr;
static uint16 s_level3_4_skillfly_bonus1_skip_ptr;
static uint16 s_level3_4_chkstratdone1_loop_ptr;
static uint16 s_level3_4_chkstratdone1_end_ptr;
// SPECIAL.ASM inline callback pointers
static uint16 s_special_boss_cleanup_script_ptr;
static uint16 s_special_theenddead_script_ptr;
static uint16 s_special_theenddead_cont_ptr;
static uint16 s_special_theend_loop_ptr;
// TRUCKER.ASM inline callback pointers
static uint16 s_trucker_biker_check_script_ptr;
static uint16 s_trucker_biker_loop_ptr;
static uint16 s_trucker_trigger_script_ptr;
static uint16 s_trucker_trigger_loop_ptr;
static uint16 s_trucker_rightblock_ptr;
static uint16 s_trucker_continue_ptr;
// MAP1_3A1 inline callback pointers
static uint16 s_map1_3a1_chkstratdone1_loop_ptr;
static uint16 s_map1_3a1_chkstratdone2_restart_ptr;
// MAP1_3A2 inline callback pointers
static uint16 s_map1_3a2_chkstratdone1_loop_ptr;
static uint16 s_map1_3a2_chkstratdone2_restart_ptr;
// FINALMAP inline callback pointers
static uint16 s_final_mapwaitboss_trigse_script_ptr;
static uint16 s_final_mapwaitboss_cantdie_script_ptr;
static uint16 s_final_mapwaitboss_cleanup_script_ptr;
// INTRO inline callback pointers
static uint16 s_intro_init_script_ptr;
// TITLE inline callback pointers
static uint16 s_title_init_script_ptr;
static uint16 s_contmap_init_script_ptr;
// title/contmap label pointers for Levels_GetMapData dispatch
static uint16 s_contmap_ptr;
static uint16 s_waitmap_ptr;
// MAP1_3C inline callback pointers
static uint16 s_map1_3c_chkstratdone1_loop_ptr;
static uint16 s_map1_3c_chkstratdone1_end_ptr;
// CREDITS inline callback pointers
static uint16 s_credits_init_script_ptr;
// TRAINING inline callback pointers
static uint16 s_training_eguchifly_loop_ptr;
// MAP1_5 (Venom 1 Orbital) inline callback pointers
static uint16 s_level1_5_skillfly_bonus0_guard_script_ptr;
static uint16 s_level1_5_skillfly_bonus0_skip_ptr;
static uint16 s_level1_5_skillfly_bonus1_guard_script_ptr;
static uint16 s_level1_5_skillfly_bonus1_skip_ptr;
static uint16 s_level1_5_mapwaitboss_trigse_script_ptr;
static uint16 s_level1_5_mapwaitboss_cantdie_script_ptr;
static uint16 s_level1_5_mapwaitboss_cleanup_script_ptr;
// MAP3_6 (Venom 2 Space) inline callback pointers
static uint16 s_level3_6_skillfly_bonus0_guard_script_ptr;
static uint16 s_level3_6_skillfly_bonus0_skip_ptr;
static uint16 s_level3_6_skillfly_bonus1_guard_script_ptr;
static uint16 s_level3_6_skillfly_bonus1_skip_ptr;
static uint16 s_level3_6_noctrl_wait_script_ptr;
static uint16 s_level3_6_noctrl_wait_boss_ptr;
static uint16 s_level3_6_hpcheck_wait_script_ptr;
static uint16 s_level3_6_hpcheck_wait_owait_ptr;
static uint16 s_level3_6_flymode_check_script_ptr;
static uint16 s_level3_6_flymode_check_cont2_ptr;
static uint16 s_level3_6_mapwaitboss_trigse_script_ptr;
static uint16 s_level3_6_mapwaitboss_cantdie_script_ptr;
static uint16 s_level3_6_mapwaitboss_cleanup_script_ptr;
// LEVEL1_6 (Venom 1 Surface / Final) inline callback pointers
static uint16 s_level1_6_mapwaitboss_cantdie_script_ptr;
static uint16 s_level1_6_mapwaitboss_cleanup_script_ptr;
// LEVEL3_7 (Venom 3 Surface / Final) inline callback pointers
static uint16 s_level3_7_skillfly_bonus0_guard_script_ptr;
static uint16 s_level3_7_skillfly_bonus0_skip_ptr;
static uint16 s_level3_7_skillfly_bonus1_guard_script_ptr;
static uint16 s_level3_7_skillfly_bonus1_skip_ptr;
static uint16 s_level3_7_mapwaitboss_trigse_script_ptr;
static uint16 s_level3_7_mapwaitboss_cantdie_script_ptr;
static uint16 s_level3_7_mapwaitboss_cleanup_script_ptr;
static bool s_literal_levels_ready;
static uint8 s_unported_level_warned[MAP_ID_TRAINING + 1u];

static void level1_1_skillfly_bonus_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level1_1_skillfly_bonus_skip_ptr;
        return;
    }
    *mapptr = (uint16)(*mapptr + 1u);
}

static void level1_1_inline_advance(uint16 *mapptr) {
    if (!mapptr) {
        return;
    }
    *mapptr = (uint16)(*mapptr + 1u);
}

static void level_scramble_keep_player_strat(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    g_pshipflags3 |= PSF3_KEEPPSTRAT;
    g_meters = 1u;
    level1_1_inline_advance(mapptr);
}

static void level1_2_skillfly_bonus_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level1_2_skillfly_bonus_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level1_2_blackhole_bonus_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level1_2_blackhole_bonus_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level2_1_skillfly_bonus0_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level2_1_skillfly_bonus0_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level2_1_skillfly_bonus1_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level2_1_skillfly_bonus1_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level3_1_skillfly_bonus0_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level3_1_skillfly_bonus0_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level3_2_skillfly_bonus_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level3_2_skillfly_bonus_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level1_1_mapwaitboss_trigse(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    Sound_PlaySE(0x0Bu);
    level1_1_inline_advance(mapptr);
}

static void level1_1_mapwaitboss_cantdie(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    g_pstratflags |= PSTF_NOTDIE;
    level1_1_inline_advance(mapptr);
}

static void level1_1_mapwaitboss_cleanup(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    g_pshipflags |= PSF_NOFIRE;
    g_bossmaxhp = 0u;
    g_meters = 0u;
    level1_1_inline_advance(mapptr);
}

static void level1_4_mapwaitboss_trigse(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    Sound_PlaySE(0x0Bu);
    level1_1_inline_advance(mapptr);
}

static void level1_4_mapwaitboss_cantdie(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    g_pstratflags |= PSTF_NOTDIE;
    level1_1_inline_advance(mapptr);
}

static void level1_4_mapwaitboss_cleanup(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    g_pshipflags |= PSF_NOFIRE;
    g_bossmaxhp = 0u;
    g_meters = 0u;
    level1_1_inline_advance(mapptr);
}

static void level3_5_mapwaitboss_trigse(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    Sound_PlaySE(0x0Bu);
    level1_1_inline_advance(mapptr);
}

static void level3_5_mapwaitboss_cantdie(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    g_pstratflags |= PSTF_NOTDIE;
    level1_1_inline_advance(mapptr);
}

static void level3_5_mapwaitboss_cleanup(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    g_pshipflags |= PSF_NOFIRE;
    g_bossmaxhp = 0u;
    g_meters = 0u;
    level1_1_inline_advance(mapptr);
}

static void level3_5_skillfly_bonus_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level3_5_skillfly_bonus_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static bool level1_1_cl_ground_printlevelfin(Alien *last_obj) {
    (void)last_obj;
    // printlevelfin is a WRAM-side state write in the clear-demo maps.
    // Do not touch g_levelfinished here or the remaining wait/wipe script
    // would stop executing immediately in the HD map executor.
    RAM8(WM_LEVELFINISHED) = 3u;
    return true;
}

static bool level1_1_cl_ground_wipeout(Alien *last_obj) {
    (void)last_obj;
    g_circleanim = 1;
    g_oncewipe = 0;
    g_pshipflags3 &= (uint8)~PSF3_ENGINESND;
    return true;
}

static bool cl_dive_clear_enginesnd(Alien *last_obj) {
    // CL_DIVE.ASM inline 65816: lda pshipflags3 / and #~psf3_enginesnd / sta
    (void)last_obj;
    g_pshipflags3 &= (uint8)~PSF3_ENGINESND;
    return true;
}

// MAP1_5 skillfly bonus guards
static void level1_5_skillfly_bonus0_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level1_5_skillfly_bonus0_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level1_5_skillfly_bonus1_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level1_5_skillfly_bonus1_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

// MAP3_6 skillfly bonus guards
static void level3_6_skillfly_bonus0_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level3_6_skillfly_bonus0_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level3_6_skillfly_bonus1_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level3_6_skillfly_bonus1_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

// MAP3_6 boss section: wait while player has noctrl flag set
static void map3_6_noctrl_wait(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (g_pshipflags & PSF_NOCTRL) {
        // Loop back to .boss (re-check next tick)
        *mapptr = s_level3_6_noctrl_wait_boss_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

// MAP3_6 boss section: wait until player HP > 0
static void map3_6_hpcheck_wait(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (g_pshipflags2 & PSF2_PLAYERHP0) {
        // Loop back to .owait (re-check next tick)
        *mapptr = s_level3_6_hpcheck_wait_owait_ptr;
        return;
    }
    // Check fly mode
    if (g_splayerflymode == SPFM_INSIDE) {
        // Need to exit inside mode — set to normal
        g_splayerflymode = SPFM_TONORM;
        // setvar.b splayerflymodeopt,spfmo_AB approximated as no-op
        *mapptr = s_level3_6_flymode_check_cont2_ptr;
        return;
    }
    *mapptr = s_level3_6_flymode_check_cont2_ptr;
}

static void level2_3_skillfly_bonus_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level2_3_skillfly_bonus_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level2_5_skillfly_bonus0_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level2_5_skillfly_bonus0_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level2_5_skillfly_bonus1_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level2_5_skillfly_bonus1_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

// eguchi2fly_goto: if ebyte3 != 0 → continue to mapgoto fogout (break loop).
// If ebyte3 == 0 → skip past mapgoto fogout, fall through to mapwait+mapgoto fogagain.
static void level2_3_fog_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (g_ebyte3 != 0u) {
        // ebyte3 set → let executor see the mapgoto .fogout (break loop)
        level1_1_inline_advance(mapptr);
    } else {
        // ebyte3 == 0 → skip past mapgoto .fogout, continue to mapwait+mapgoto .fogagain
        *mapptr = s_level2_3_fog_guard_continue_ptr;
    }
}

// Inline callback for the post-fog SETVAR.N / setvar / MAPCODE_JSL / start_65816 block.
// Sets INFOG=0, FADEPAL=33, palette vars, dotsflag=1, m_planetstars=0.
static void level2_3_setvar_inline(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    // SETVAR.N FADEPAL,33
    RAM8(WM_FADEPAL) = 33;
    // setvar palfrom,0  palto,0  pallen,32
    RAM8(WM_PALFROM) = 0;
    RAM8(WM_PALTO) = 0;
    RAM8(WM_PALLEN) = 32;
    // SETVAR.N INFOG,0
    RAM8(WM_INFOG) = 0;
    // dotsflag = 1 (a8: lda #1; sta dotsflag; stz dotsflag+1)
    g_dotsflag = 1;
    // m_planetstars = 0 — renderer variable, not yet mirrored in HD port.
    // g_planetstars = 0;
    level1_1_inline_advance(mapptr);
}

// MAP2_3B inline: maptrigger check at .waitabit
// Checks maptrigger bits:
//   bit 2 set → jump to .carryon (boss section)
//   bit 1 not set → loop back to .waitabit
//   bit 1 set → clear bit 0, continue to spawn seamons
static void level2_3b_trigger_check(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    uint8 trig = g_maptrigger;
    if (trig & 2u) {
        // bit 2 set → jump to .carryon
        *mapptr = s_level2_3b_trigger_carryon_ptr;
        return;
    }
    if (!(trig & 1u)) {
        // bit 1 not set → loop back to .waitabit
        *mapptr = s_level2_3b_trigger_waitabit_ptr;
        return;
    }
    // bit 1 set → clear bit 0, continue
    g_maptrigger = trig & 0xFEu;
    level1_1_inline_advance(mapptr);
}

// MAP2_3B inline: gsvar_byte1 == 0 check at .seatest
// If gsvar_byte1 != 0 → loop back to .seatest
// If gsvar_byte1 == 0 → continue
static void level2_3b_seatest_check(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    // Sync RAM mirror → C global so strategy code sees the latest value.
    g_gsvar_byte1 = RAM8(WM_GSVAR_BYTE1);
    if (RAM8(WM_GSVAR_BYTE1) != 0u) {
        *mapptr = s_level2_3b_seatest_loop_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

// BG_1_4B_1: set z depth table to normal, gamepal red, shadow height 0.
static bool level2_3_bg_1_4b_1(Alien *last_obj) {
    (void)last_obj;
    // In the HD port, this is a simplified BG transition callback.
    // The actual depth table/palette/shadow changes are handled elsewhere.
    return true;
}

static void level3_3_skillfly_bonus0_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level3_3_skillfly_bonus0_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

// MAP3_4B skillfly_bonus guards — skip the bonus mapobj if skillfly was achieved.
static void level3_4_skillfly_bonus0_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level3_4_skillfly_bonus0_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level3_4_skillfly_bonus1_guard(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (RAM8(WM_SKILLFLY) != 0u) {
        *mapptr = s_level3_4_skillfly_bonus1_skip_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

// MAP3_4B chkstratdone1 — busy-wait for colony boss defeat.
static void level3_4_chkstratdone1_check(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    // chkstratdone1 checks whether the colony boss is dead.
    // For now, treat it as "always done" to keep the map flowing.
    *mapptr = s_level3_4_chkstratdone1_end_ptr;
}

// mapgotoifplayerdead callback: if player HP=0, goto the dead-loop label.
static void level3_3_pdead2_check(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (g_pshipflags2 & PSF2_PLAYERHP0) {
        *mapptr = s_level3_3_pdead2_script_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

static void level3_3_pdead_check(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (g_pshipflags2 & PSF2_PLAYERHP0) {
        *mapptr = s_level3_3_pdead_script_ptr;
        return;
    }
    level1_1_inline_advance(mapptr);
}

// SPECIAL.ASM inline: after boss death — clear nofire, clear notdie.
static void special_boss_cleanup(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    g_pshipflags &= (uint8)~PSF_NOFIRE;
    g_pstratflags &= (uint8)~PSTF_NOTDIE;
    level1_1_inline_advance(mapptr);
}

// SPECIAL.ASM inline: theenddead check for "THE END" letter loop.
static void special_theenddead_check(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    if (g_numendok == 0xFFu) {
        // All letters destroyed — continue to .cont
        *mapptr = s_special_theenddead_cont_ptr;
        return;
    }
    // Not done — loop back
    *mapptr = s_special_theend_loop_ptr;
}

// MAP1_3C chkstratdone1 — busy-wait for big ship boss defeat.
static void map1_3c_chkstratdone1_check(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    // chkstratdone1 checks whether the big ship boss is dead.
    // For now, treat it as "always done" to keep the map flowing.
    *mapptr = s_map1_3c_chkstratdone1_end_ptr;
}

// CREDITS.ASM inline: disable player wobble and controls.
static void credits_init_inline(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    // Disable wobble
    g_playerflymode &= (uint8)~PFM_WOBBLE;
    // Set noctrl + nofire
    g_pshipflags |= (uint8)(PSF_NOCTRL | PSF_NOFIRE);
    level1_1_inline_advance(mapptr);
}

// TRAINING.ASM inline: eguchifly_goto loop check.
// The original checks whether the training flight is complete.
// For now, always continue (training runs once through).
static void training_eguchifly_check(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    // In the HD engine, eguchifly_goto is approximated by jumping back
    // to the loop label. For now, always advance past the loop.
    level1_1_inline_advance(mapptr);
}

// TRUCKER.ASM inline: check if any air_1 (biker) objects still alive.
// Uses find_y_l equivalent — if no air_1 found, continue; else loop.
static void trucker_biker_check(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    // In the HD engine, we approximate find_y_l by checking
    // if any objects with the biker shape remain alive.
    // For now, always advance (bikers will time out or be killed).
    // TODO: implement proper shape-count check when alien tracking is ported.
    level1_1_inline_advance(mapptr);
}

// TRUCKER.ASM inline: maptrigger bit check at .loop2
static void trucker_trigger_check(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    if (!mapptr) {
        return;
    }
    uint8 trig = g_maptrigger;
    g_maptrigger = 0;
    if (trig & 2u) {
        // bit 2 set — boss dead, jump to .continue
        *mapptr = s_trucker_continue_ptr;
        return;
    }
    if (trig & 1u) {
        // bit 1 set — spawn right road block
        *mapptr = s_trucker_rightblock_ptr;
        return;
    }
    // No trigger bits — loop back to .loop2
    *mapptr = s_trucker_trigger_loop_ptr;
}

// INTRO.ASM inline: disable wobble, set noctrl+nofire, clear position.
static void intro_init_inline(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    g_playerflymode &= (uint8)~PFM_WOBBLE;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    level1_1_inline_advance(mapptr);
}

// TITLE.ASM inline: clear position, disable wobble, set noctrl+nofire.
static void title_init_inline(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    g_playerflymode &= (uint8)~PFM_WOBBLE;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    level1_1_inline_advance(mapptr);
}

// TITLE.ASM contmap inline: clear position, disable wobble.
static void contmap_init_inline(uint16 *mapptr, Alien *last_obj) {
    (void)last_obj;
    g_playerflymode &= (uint8)~PFM_WOBBLE;
    level1_1_inline_advance(mapptr);
}

static void mb_fail(MapBuilder *b, const char *reason) {
    if (!b || b->failed) {
        return;
    }
    b->failed = true;
    fprintf(stderr, "Map literals: %s\n", reason);
}

static void mb_emit8(MapBuilder *b, uint8 value) {
    if (!b || b->failed) {
        return;
    }
    if (b->length >= b->capacity) {
        mb_fail(b, "bytecode buffer overflow");
        return;
    }
    b->data[b->length++] = value;
}

static void mb_emit16(MapBuilder *b, uint16 value) {
    mb_emit8(b, (uint8)(value & 0xFFu));
    mb_emit8(b, (uint8)(value >> 8));
}

static void mb_emit16s(MapBuilder *b, int16 value) {
    mb_emit16(b, (uint16)value);
}

static void mb_label(MapBuilder *b, const char *name) {
    if (!b || b->failed || !name) {
        return;
    }
    if (b->label_count >= MAP_LABEL_CAPACITY) {
        mb_fail(b, "label table overflow");
        return;
    }
    snprintf(b->labels[b->label_count].name, sizeof(b->labels[0].name), "%s", name);
    b->labels[b->label_count].offset = (uint16)b->length;
    b->label_count++;
}

static void mb_fixup16(MapBuilder *b, const char *label) {
    if (!b || b->failed || !label) {
        return;
    }
    if (b->fixup_count >= MAP_FIXUP_CAPACITY) {
        mb_fail(b, "fixup table overflow");
        return;
    }
    snprintf(b->fixups[b->fixup_count].label, sizeof(b->fixups[0].label), "%s", label);
    b->fixups[b->fixup_count].offset = (uint16)b->length;
    b->fixup_count++;
    mb_emit16(b, 0);
}

static void mb_mapwait(MapBuilder *b, uint16 dist);
static void mb_mapcodejsl_builtin(MapBuilder *b, uint32 callback_addr24);
static void mb_setalvarb(MapBuilder *b, uint16 offset, uint8 value);
static void append_trucker_submap(MapBuilder *b);
static void append_map1_3a1_submap(MapBuilder *b);
static void append_map1_3a2_submap(MapBuilder *b);
static void append_map1_3b2_submap(MapBuilder *b);
static void append_cl_ship_submap(MapBuilder *b);
static void register_level1_3_inline_callbacks(void);

static bool mb_lookup_label(const MapBuilder *b, const char *label, uint16 *out) {
    if (!b || !label || !out) {
        return false;
    }
    for (uint16 i = 0; i < b->label_count; i++) {
        if (strcmp(b->labels[i].name, label) == 0) {
            *out = b->labels[i].offset;
            return true;
        }
    }
    return false;
}

static void mb_resolve(MapBuilder *b) {
    if (!b || b->failed) {
        return;
    }
    for (uint16 i = 0; i < b->fixup_count; i++) {
        uint16 target = 0;
        if (!mb_lookup_label(b, b->fixups[i].label, &target)) {
            fprintf(stderr, "Map literals: unresolved label '%s'\n", b->fixups[i].label);
            target = 0;
        }
        b->data[b->fixups[i].offset] = (uint8)(target & 0xFFu);
        b->data[b->fixups[i].offset + 1u] = (uint8)(target >> 8);
    }
}

static void mb_mapnobj(MapBuilder *b, uint16 frame, int16 x, int16 y, int16 z,
                       uint16 shape, uint32 strat) {
    mb_emit8(b, MAP_OP_NORMOBJ);
    mb_emit16(b, frame);
    mb_emit16s(b, x);
    mb_emit16s(b, y);
    mb_emit16s(b, z);
    mb_emit16(b, shape);
    mb_emit16(b, (uint16)(strat & 0xFFFFu));
    mb_emit8(b, (uint8)((strat >> 16) & 0xFFu));
}

static void mb_mapobj(MapBuilder *b, uint16 frame, int16 x, int16 y, int16 z,
                      uint16 shape, uint32 strat) {
    if (shape <= 0xFFu && strat <= 0xFFu) {
        mb_emit8(b, MAP_OP_MAPOBJ);
        mb_emit16(b, frame);
        mb_emit16s(b, x);
        mb_emit16s(b, y);
        mb_emit16s(b, z);
        mb_emit8(b, (uint8)shape);
        mb_emit8(b, (uint8)strat);
        return;
    }
    mb_mapnobj(b, frame, x, y, z, shape, strat);
}

static void mb_maphardrot(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                          uint16 shape, int8 rx, int8 ry, int8 rz) {
    mb_mapobj(b, 0, x, y, z, shape, IS_HARDROT);
    mb_setalvarb(b, AL_SBYTE1, (uint8)rx);
    mb_setalvarb(b, AL_SBYTE2, (uint8)ry);
    mb_setalvarb(b, AL_SBYTE3, (uint8)rz);
    mb_mapwait(b, wait);
}

static void mb_mapwait(MapBuilder *b, uint16 dist) {
    mb_emit8(b, MAP_OP_WAIT);
    mb_emit16(b, dist);
}

static void mb_setbgm(MapBuilder *b, uint8 music_id) {
    mb_emit8(b, MAP_OP_SETBGM);
    mb_emit8(b, music_id);
}

static void mb_mapplayeroutview(MapBuilder *b) {
    mb_mapcodejsl_builtin(b, MAP_CB_PLAYER_OUTVIEW_L);
}

static void mb_setbg(MapBuilder *b, uint16 bg_id) {
    mb_emit8(b, MAP_OP_SETBG);
    mb_emit16(b, bg_id);
}

static void mb_qfadeup(MapBuilder *b) {
    mb_emit8(b, MAP_OP_QFADEUP);
}

static void mb_qfadedown(MapBuilder *b) {
    mb_emit8(b, MAP_OP_QFADEDOWN);
}

static void mb_waitfade(MapBuilder *b) {
    mb_emit8(b, MAP_OP_WAITFADE);
}

static void mb_initbg(MapBuilder *b) {
    mb_emit8(b, MAP_OP_WAITSETBG);
    mb_emit8(b, MAP_OP_SETBGINFO);
}

static void mb_maploop(MapBuilder *b, const char *label, uint16 count) {
    mb_emit8(b, MAP_OP_LOOP);
    mb_fixup16(b, label);
    mb_emit16(b, count);
}

static void mb_mapif_builtin(MapBuilder *b, uint32 callback_addr24, const char *else_label) {
    mb_emit8(b, MAP_OP_IF);
    mb_emit16(b, (uint16)(callback_addr24 & 0xFFFFu));
    mb_emit8(b, (uint8)(callback_addr24 >> 16));
    mb_fixup16(b, else_label);
}

static void mb_mapgoto(MapBuilder *b, const char *label) {
    mb_emit8(b, MAP_OP_GOTO);
    mb_fixup16(b, label);
    mb_emit8(b, 0);
}

static void mb_mapremove(MapBuilder *b, uint16 shape) {
    mb_emit8(b, MAP_OP_REMOVE);
    mb_emit16(b, 0);
    mb_emit16(b, shape);
}

static void mb_setalvarb(MapBuilder *b, uint16 offset, uint8 value) {
    mb_emit8(b, MAP_OP_SETALVARB);
    mb_emit16(b, offset);
    mb_emit8(b, value);
}

static void mb_setalvarw(MapBuilder *b, uint16 offset, uint16 value) {
    mb_emit8(b, MAP_OP_SETALVARW);
    mb_emit16(b, offset);
    mb_emit16(b, value);
}

static void mb_setalxvarb(MapBuilder *b, uint16 offset, uint8 value) {
    mb_emit8(b, MAP_OP_SETALXVARB);
    mb_emit16(b, offset);
    mb_emit8(b, value);
}

static void mb_setalxvarw(MapBuilder *b, uint16 offset, int16 value) {
    mb_emit8(b, MAP_OP_SETALXVARW);
    mb_emit16(b, offset);
    mb_emit16s(b, value);
}

static void mb_setalvarptrw(MapBuilder *b, uint16 offset, uint16 extptr) {
    mb_emit8(b, MAP_OP_SETALVARPW);
    mb_emit16(b, offset);
    mb_emit16(b, extptr);
    mb_emit8(b, 0);
}

static void mb_addalvarptrw(MapBuilder *b, uint16 offset, uint16 extptr) {
    mb_emit8(b, MAP_OP_ADDALVARPW);
    mb_emit16(b, offset);
    mb_emit16(b, extptr);
    mb_emit8(b, 0);
}

static void mb_setvarobj(MapBuilder *b, uint16 extptr) {
    mb_emit8(b, MAP_OP_SETVAROBJ);
    mb_emit16(b, extptr);
    mb_emit8(b, 0);
}

static void mb_setvarb(MapBuilder *b, uint16 extptr, uint8 value) {
    mb_emit8(b, MAP_OP_SETVARB);
    mb_emit8(b, value);
    mb_emit16(b, extptr);
    mb_emit8(b, 0);
}

static void mb_setvarw(MapBuilder *b, uint16 extptr, uint16 value) {
    mb_emit8(b, MAP_OP_SETVARW);
    mb_emit16(b, value);
    mb_emit16(b, extptr);
    mb_emit8(b, 0);
}

static void mb_mapend(MapBuilder *b, uint8 level_finished) {
    mb_setvarb(b, WM_LEVELFINISHED, level_finished);
    mb_emit8(b, MAP_OP_END);
}

static void mb_sendmsg(MapBuilder *b, uint8 msg_id) {
    mb_emit8(b, MAP_OP_SENDMSG);
    mb_emit8(b, msg_id);
}

static void mb_mapsetpath(MapBuilder *b, uint16 path_id) {
    mb_emit8(b, MAP_OP_SETPATH);
    mb_emit16(b, path_id);
}

static void mb_mapcodejsl_builtin(MapBuilder *b, uint32 callback_addr24) {
    uint16 encoded = (uint16)((callback_addr24 - 1u) & 0xFFFFu);
    mb_emit8(b, MAP_OP_CODEJSL);
    mb_emit16(b, encoded);
    mb_emit8(b, (uint8)(callback_addr24 >> 16));
}

static void mb_mapcode65816_inline(MapBuilder *b, uint16 *out_script_ptr) {
    if (out_script_ptr) {
        *out_script_ptr = (uint16)(b->length + 1u);
    }
    mb_emit8(b, MAP_OP_CODE65816);
}

static void mb_mapexploderobot(MapBuilder *b) {
    mb_mapcodejsl_builtin(b, MAP_CB_KILL_ROBOT_L);
}

static void mb_mapjsr(MapBuilder *b, const char *label) {
    mb_emit8(b, MAP_OP_JSR);
    mb_fixup16(b, label);
    mb_emit8(b, 0);
}

static void mb_mapmother(MapBuilder *b, uint16 frame, int16 x, int16 y, int16 z,
                         uint16 shape, uint32 strat_addr24, uint16 map_ref) {
    mb_emit8(b, MAP_OP_MOTHER);
    mb_emit16(b, frame);
    mb_emit16s(b, x);
    mb_emit16s(b, y);
    mb_emit16s(b, z);
    mb_emit16(b, shape);
    mb_emit16(b, (uint16)(strat_addr24 & 0xFFFFu));
    mb_emit8(b, (uint8)((strat_addr24 >> 16) & 0xFFu));
    mb_emit16(b, map_ref);
}

static void mb_maprts(MapBuilder *b) {
    mb_emit8(b, MAP_OP_RTS);
}

static void mb_mapcspecial(MapBuilder *b) {
    mb_emit8(b, MAP_OP_CSPECIAL);
}

static void mb_pathobj(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                       uint16 shape, uint16 path_id, uint8 hp, uint8 ap) {
    if (hp == 10u && ap == 10u) {
        mb_mapobj(b, 0, x, y, z, shape, IS_PATHDHA);
    } else {
        mb_mapobj(b, 0, x, y, z, shape, IS_PATH);
        mb_setalvarb(b, AL_HP, hp);
        mb_setalvarb(b, AL_AP, ap);
    }
    mb_mapsetpath(b, path_id);
    mb_mapwait(b, wait);
}

static void mb_pathspecial(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                           uint16 shape, uint16 path_id, uint8 hp, uint8 ap) {
    if (hp == 10u && ap == 10u) {
        mb_mapobj(b, 0, x, y, z, shape, IS_PATHDHA);
    } else {
        mb_mapobj(b, 0, x, y, z, shape, IS_PATH);
        mb_setalvarb(b, AL_HP, hp);
        mb_setalvarb(b, AL_AP, ap);
    }
    mb_mapsetpath(b, path_id);
    mb_emit8(b, MAP_OP_SPECIAL);
    mb_mapwait(b, wait);
}

static void mb_pathcspecial(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                            uint16 shape, uint16 path_id, uint8 hp, uint8 ap) {
    if (hp == 10u && ap == 10u) {
        mb_mapobj(b, 0, x, y, z, shape, IS_PATHDHA);
    } else {
        mb_mapobj(b, 0, x, y, z, shape, IS_PATH);
        mb_setalvarb(b, AL_HP, hp);
        mb_setalvarb(b, AL_AP, ap);
    }
    mb_mapsetpath(b, path_id);
    mb_mapcspecial(b);
    mb_mapwait(b, wait);
}

static void mb_map_farships_common(MapBuilder *b, uint16 shape, bool face_around,
                                   int16 x, int16 y, int16 z,
                                   int16 x_speed, int16 y_speed, int16 depth) {
    mb_mapobj(b, 300, x, (int16)(SPACE_VIEWCY + y), z, shape, IS_SHIPS);
    mb_setalvarw(b, AL_SWORD1, (uint16)x_speed);
    mb_setalvarw(b, AL_SWORD2, (uint16)y_speed);
    mb_setalxvarw(b, ALX_DEPTHOFFSET, depth);
    if (face_around) {
        mb_setalvarb(b, AL_ROTY, DEG180);
    }
}

static void mb_map_farships0(MapBuilder *b, int16 x, int16 y, int16 z,
                             int16 x_speed, int16 y_speed, int16 depth) {
    mb_map_farships_common(b, SH_SHIP_S_0, true, x, y, z, x_speed, y_speed, depth);
}

static void mb_map_farships1(MapBuilder *b, int16 x, int16 y, int16 z,
                             int16 x_speed, int16 y_speed, int16 depth) {
    mb_map_farships_common(b, SH_SHIP_S_1, true, x, y, z, x_speed, y_speed, depth);
}

static void mb_map_farships2(MapBuilder *b, int16 x, int16 y, int16 z,
                             int16 x_speed, int16 y_speed, int16 depth) {
    mb_map_farships_common(b, SH_SHIPS, false, x, y, z, x_speed, y_speed, depth);
}

static void mb_cspecial(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                        uint16 shape, uint32 strat) {
    mb_mapobj(b, 0, x, y, z, shape, strat);
    mb_mapcspecial(b);
    mb_mapwait(b, wait);
}

static void mb_special(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                       uint16 shape, uint32 strat) {
    mb_mapobj(b, 0, x, y, z, shape, strat);
    mb_emit8(b, MAP_OP_SPECIAL);
    mb_mapwait(b, wait);
}

static void mb_skillfly_init(MapBuilder *b) {
    mb_setvarb(b, WM_SKILLFLY, 0);
}

static void mb_skillfly_set(MapBuilder *b, int16 x, int16 y, int16 z, uint16 radius) {
    mb_mapobj(b, 0, x, y, z, SH_NULLSHAPE, IS_SKILLFLY);
    mb_setalvarw(b, AL_SWORD1, radius);
}

static void mb_skillfly_set_default(MapBuilder *b, int16 x, int16 y, int16 z) {
    mb_mapobj(b, 0, x, y, z, SH_NULLSHAPE, IS_SKILLFLY);
}

// MAP3_3A helpers
static void mb_mapfadetosea(MapBuilder *b) {
    mb_emit8(b, MAP_OP_FADETOSEA);
}

static void mb_mapfadetoground(MapBuilder *b) {
    mb_emit8(b, MAP_OP_FADETOGROUND);
}

// roottree MACRO — expands to: mapobj stalk/tree3 + setalvar sbyte2 + mapwait
static void mb_roottree(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                        int8 angle, uint8 time_to_tail) {
    mb_mapobj(b, 0, x, y, z, SH_STALK, IS_TREE3);
    mb_setalvarb(b, AL_SBYTE2, (uint8)angle);
    mb_mapwait(b, wait);
}

// nessie MACRO — expands to: mapobj nullshape/lochnessmonster + setalvar roty + setalvar sword1+1 + mapwait
static void mb_nessie(MapBuilder *b, uint16 wait, int16 y, int16 z, int16 depth,
                      int8 roty, uint8 tail_time) {
    mb_mapobj(b, 0, y, z, depth, SH_NULLSHAPE, IS_LOCHNESSMONSTER);
    mb_setalvarb(b, AL_ROTY, (uint8)roty);
    mb_setalvarb(b, AL_SWORD1 + 1, tail_time);
    mb_mapwait(b, wait);
}

static uint16 mb_spacebar_shape_id(const MapBuilder *b, MapSpacebarShape shape) {
    bool solid = b && b->barshape_mode == MAP_BARSHAPE_SOLID;

    switch (shape) {
    case MAP_SPACEBAR_SHAPE_X:
        return solid ? SH_XSOLIDSPACEBAR : SH_XWIRESPACEBAR;
    case MAP_SPACEBAR_SHAPE_XP:
        return solid ? SH_XPSOLIDSPACEBAR : SH_XPWIRESPACEBAR;
    case MAP_SPACEBAR_SHAPE_Y:
        return solid ? SH_YSOLIDSPACEBAR : SH_YWIRESPACEBAR;
    case MAP_SPACEBAR_SHAPE_Z:
        return solid ? SH_ZSOLIDSPACEBAR : SH_ZWIRESPACEBAR;
    case MAP_SPACEBAR_SHAPE_SX:
        return solid ? SH_SXSOLIDSPACEBAR : SH_SXWIRESPACEBAR;
    case MAP_SPACEBAR_SHAPE_SXP:
        return solid ? SH_SXPSOLIDSPACEBAR : SH_SXPWIRESPACEBAR;
    case MAP_SPACEBAR_SHAPE_SY:
        return solid ? SH_SYSOLIDSPACEBAR : SH_SYWIRESPACEBAR;
    case MAP_SPACEBAR_SHAPE_SZ:
        return solid ? SH_SZSOLIDSPACEBAR : SH_SZWIRESPACEBAR;
    default:
        return SH_NULLSHAPE;
    }
}

static int16 mb_spacebar_units(int16 value) {
    return (int16)(value * SPACEBAR_UNIT_LEN);
}

static void mb_map_setbarshape(MapBuilder *b, MapBarShapeMode mode, bool autowait) {
    if (!b) {
        return;
    }

    b->barshape_mode = mode;
    b->barshape_autowait = autowait;
    b->barshape_swait = 0;
    b->barshape_pos = 0;
}

static void mb_spacebar_calcsbwait(MapBuilder *b, int16 z) {
    int16 delta;

    if (!b || !b->barshape_autowait) {
        return;
    }

    delta = (int16)(z - b->barshape_swait);
    if (delta > 0) {
        mb_mapwait(b, (uint16)mb_spacebar_units(delta));
        b->barshape_swait = z;
    }
}

static void mb_map_spacebarwait(MapBuilder *b, uint16 wait) {
    mb_mapwait(b, (uint16)mb_spacebar_units((int16)wait));
}

static void mb_map_xspacebar(MapBuilder *b, int16 x, int16 y, int16 z) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST +
                      (b && b->barshape_autowait ? 0 : mb_spacebar_units(z))),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_X),
              MAP_ISTRAT_SPACEBAR);
    mb_spacebar_calcsbwait(b, z);
}

static void mb_map_yspacebar(MapBuilder *b, int16 x, int16 y, int16 z) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST +
                      (b && b->barshape_autowait ? 0 : mb_spacebar_units(z))),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_Y),
              MAP_ISTRAT_SPACEBAR);
    mb_spacebar_calcsbwait(b, z);
}

static void mb_map_zspacebar(MapBuilder *b, int16 x, int16 y, int16 z) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST +
                      (b && b->barshape_autowait ? 0 : mb_spacebar_units(z))),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_Z),
              MAP_ISTRAT_SPACEBAR);
    mb_spacebar_calcsbwait(b, z);
}

static void mb_map_sxspacebar(MapBuilder *b, int16 x, int16 y, int16 z) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST +
                      (b && b->barshape_autowait ? 0 : mb_spacebar_units(z))),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_SX),
              MAP_ISTRAT_SPACEBAR);
    mb_spacebar_calcsbwait(b, z);
}

static void mb_map_syspacebar(MapBuilder *b, int16 x, int16 y, int16 z) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST +
                      (b && b->barshape_autowait ? 0 : mb_spacebar_units(z))),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_SY),
              MAP_ISTRAT_SPACEBAR);
    mb_spacebar_calcsbwait(b, z);
}

static void mb_map_szspacebar(MapBuilder *b, int16 x, int16 y, int16 z) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST +
                      (b && b->barshape_autowait ? 0 : mb_spacebar_units(z))),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_SZ),
              MAP_ISTRAT_SPACEBAR);
    mb_spacebar_calcsbwait(b, z);
}

static void mb_map_spacebarc(MapBuilder *b, int16 x, int16 y, int16 z,
                             int8 init_z_rot, int8 speed) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST + mb_spacebar_units(z)),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_XP),
              MAP_ISTRAT_SPACEBAR1);
    mb_setalvarb(b, AL_ROTZ, (uint8)(init_z_rot * DEG45));
    mb_setalvarb(b, AL_SBYTE1, (uint8)speed);
    mb_setvarobj(b, WM_MAPVAR1);
    if (b) {
        b->barshape_pos = mb_spacebar_units(z);
    }
}

static void mb_map_spacebaric(MapBuilder *b, int16 x, int16 y, int16 z,
                              int8 init_z_rot, int8 speed) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST + mb_spacebar_units(z)),
              SH_NULLSHAPE,
              MAP_ISTRAT_SPACEBAR1);
    mb_setalvarb(b, AL_ROTZ, (uint8)(init_z_rot * DEG45));
    mb_setalvarb(b, AL_SBYTE1, (uint8)speed);
    mb_setalvarb(b, AL_SBYTE2, 1u);
    mb_setvarobj(b, WM_MAPVAR1);
    if (b) {
        b->barshape_pos = mb_spacebar_units(z);
    }
}

static void mb_map_spacebarx(MapBuilder *b, int16 x, int16 y, int16 z, int8 init_z_rot) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST + mb_spacebar_units(z) + (b ? b->barshape_pos : 0)),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_XP),
              MAP_ISTRAT_SPACEBAR3);
    mb_setalvarb(b, AL_ROTZ, (uint8)(init_z_rot * DEG45));
    mb_setalvarptrw(b, AL_PTR, WM_MAPVAR1);
}

static void mb_map_spacebarsx(MapBuilder *b, int16 x, int16 y, int16 z, int8 init_z_rot) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST + mb_spacebar_units(z) + (b ? b->barshape_pos : 0)),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_SXP),
              MAP_ISTRAT_SPACEBAR3);
    mb_setalvarb(b, AL_ROTZ, (uint8)(init_z_rot * DEG45));
    mb_setalvarptrw(b, AL_PTR, WM_MAPVAR1);
}

static void mb_map_spacebarsz(MapBuilder *b, int16 x, int16 y, int16 z, int8 init_z_rot) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST + mb_spacebar_units(z) + (b ? b->barshape_pos : 0)),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_SZ),
              MAP_ISTRAT_SPACEBAR3);
    mb_setalvarb(b, AL_ROTZ, (uint8)(init_z_rot * DEG45));
    mb_setalvarptrw(b, AL_PTR, WM_MAPVAR1);
}

static void mb_map_xpspacebar(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                              int8 init_z_rot, int8 speed) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST + mb_spacebar_units(z)),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_XP),
              MAP_ISTRAT_SPINSPACEBAR);
    mb_setalvarb(b, AL_ROTZ, (uint8)(init_z_rot * DEG45));
    mb_setalvarb(b, AL_SBYTE1, (uint8)speed);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sxpspacebar(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                               int8 init_z_rot, int8 speed) {
    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST + mb_spacebar_units(z)),
              mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_SXP),
              MAP_ISTRAT_SPINSPACEBAR);
    mb_setalvarb(b, AL_ROTZ, (uint8)(init_z_rot * DEG45));
    mb_setalvarb(b, AL_SBYTE1, (uint8)speed);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_ryspacebar(MapBuilder *b, int16 x, int16 y, int16 z, int8 y_rot) {
    mb_map_xspacebar(b, x, y, z);
    mb_setalvarb(b, AL_ROTY, (uint8)(y_rot * DEG45));
    mb_spacebar_calcsbwait(b, z);
}

static void mb_map_sbtype0(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_yspacebar(b, x, y, z);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtype1(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_xspacebar(b, x, y, z);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtype5(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_zspacebar(b, x, y, z);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtype6(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_xpspacebar(b, wait, x, y, z, 2, -4);
}

static void mb_map_sbtype7(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_spacebarc(b, x, y, z, 0, 4);
    mb_map_spacebarx(b, (int16)(x + 2), y, 0, 2);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtype3(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_spacebarc(b, x, y, z, 0, -6);
    mb_map_spacebarx(b, x, y, 0, 2);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtype8(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_szspacebar(b, x, y, z);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtypeA(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_sxpspacebar(b, wait, x, y, z, 2, -4);
}

static void mb_map_sbtypeB(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_syspacebar(b, x, y, z);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtypeC(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_sxspacebar(b, x, y, z);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtypeD(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_xspacebar(b, (int16)(x + 2), (int16)(y - 2), z);
    mb_map_yspacebar(b, (int16)(x + 3), y, z);
    mb_map_sxspacebar(b, (int16)(x + 2), y, z);
    mb_map_zspacebar(b, (int16)(x + 3), (int16)(y + 2), (int16)(z + 2));
    mb_map_xspacebar(b, (int16)(x + 5), (int16)(y + 2), (int16)(z + 4));
    mb_map_yspacebar(b, (int16)(x + 3), (int16)(y + 1), (int16)(z + 4));
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtypeE(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_xspacebar(b, (int16)(x - 1), y, z);
    mb_map_xspacebar(b, (int16)(x - 5), y, z);
    mb_map_yspacebar(b, (int16)(x - 7), (int16)(y + 1), z);
    mb_map_xspacebar(b, (int16)(x - 9), (int16)(y - 1), z);
    mb_map_xspacebar(b, (int16)(x - 9), (int16)(y + 3), z);
    mb_map_zspacebar(b, (int16)(x - 7), y, (int16)(z + 2));
    mb_map_syspacebar(b, (int16)(x - 7), y, (int16)(z + 4));
    mb_map_sxspacebar(b, (int16)(x - 8), (int16)(y - 1), z);
    mb_map_sxspacebar(b, (int16)(x - 8), (int16)(y + 1), z);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtypeF(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_xspacebar(b, x, y, z);
    mb_map_ryspacebar(b, (int16)(x + 2), y, z, 3);
    mb_map_ryspacebar(b, (int16)(x - 1), y, (int16)(z + 3), 3);
    mb_map_xspacebar(b, (int16)(x - 2), y, (int16)(z + 6));
    mb_map_xspacebar(b, (int16)(x + 2), y, (int16)(z + 6));
    mb_map_ryspacebar(b, (int16)(x + 4), y, (int16)(z + 6), 3);
    mb_map_ryspacebar(b, (int16)(x + 1), y, (int16)(z + 9), 3);
    mb_map_xspacebar(b, x, y, (int16)(z + 12));
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtype10(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_spacebarc(b, x, y, z, 0, 6);
    mb_map_spacebarx(b, x, y, 0, 2);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtype11(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_yspacebar(b, x, y, z);
    mb_map_xspacebar(b, (int16)(x + 2), (int16)(y - 2), z);
    mb_map_xspacebar(b, (int16)(x + 2), (int16)(y + 2), z);
    mb_map_yspacebar(b, (int16)(x + 4), (int16)(y + 2), z);
    mb_map_zspacebar(b, x, (int16)(y - 2), (int16)(z - 2));
    mb_map_zspacebar(b, x, (int16)(y + 2), (int16)(z + 2));
    mb_map_syspacebar(b, x, (int16)(y + 1), (int16)(z + 4));
    mb_map_zspacebar(b, (int16)(x + 4), (int16)(y - 2), (int16)(z + 2));
    mb_map_sxspacebar(b, (int16)(x + 3), (int16)(y - 2), (int16)(z + 4));
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtype14(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_spacebarc(b, x, y, z, 0, -3);
    mb_map_spacebarsx(b, (int16)(x - 2), (int16)(y - 1), 0, 2);
    mb_map_zspacebar(b, (int16)(x + 2), y, 2);
    mb_map_spacebarx(b, (int16)(x + 2), (int16)(y + 2), 0, 2);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtype15(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                            int8 init_z_rot, int8 speed) {
    mb_map_spacebaric(b, x, y, z, init_z_rot, speed);
    mb_map_spacebarx(b, x, (int16)(y - 2), 0, init_z_rot);
    mb_map_spacebarx(b, x, (int16)(y + 1), 0, init_z_rot);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtype16(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                            int16 x_vel, int8 spin_speed) {
    uint16 shape = (spin_speed == 0) ? mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_X)
                                     : mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_XP);

    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST + mb_spacebar_units(z)),
              shape,
              IS_SPACEBARSHOOT);
    mb_setalvarw(b, AL_SWORD1, (uint16)x_vel);
    mb_setalvarb(b, AL_SBYTE1, (uint8)spin_speed);
    mb_map_spacebarwait(b, wait);
}

static void mb_map_sbtype17(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                            int16 y_vel, int8 spin_speed) {
    uint16 shape = (spin_speed == 0) ? mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_Y)
                                     : mb_spacebar_shape_id(b, MAP_SPACEBAR_SHAPE_XP);

    mb_mapobj(b, 0,
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST + mb_spacebar_units(z)),
              shape,
              IS_SPACEBARSHOOT);
    if (spin_speed != 0) {
        mb_setalvarb(b, AL_ROTZ, DEG90);
    }
    mb_setalvarw(b, AL_SWORD2, (uint16)y_vel);
    mb_setalvarb(b, AL_SBYTE1, (uint8)spin_speed);
    mb_map_spacebarwait(b, wait);
}

// map_SBtype12: Y/Y/X/X bars + SY/SZ/Z + SY/SZ + RY/SZ  (big frame)
static void mb_map_sbtype12(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_yspacebar(b, x, y, z);
    mb_map_yspacebar(b, (int16)(x + 4), y, z);
    mb_map_xspacebar(b, (int16)(x + 2), (int16)(y - 2), z);
    mb_map_xspacebar(b, (int16)(x + 2), (int16)(y + 2), z);
    mb_map_syspacebar(b, x, (int16)(y - 3), z);
    mb_map_szspacebar(b, x, (int16)(y - 2), (int16)(z + 1));
    mb_map_zspacebar(b, x, (int16)(y + 2), (int16)(z + 2));
    mb_map_syspacebar(b, x, (int16)(y + 2), (int16)(z + 4));
    mb_map_szspacebar(b, (int16)(x + 1), (int16)(y + 2), (int16)(z + 4));
    mb_map_ryspacebar(b, (int16)(x + 4), (int16)(y - 2), z, 3);
    mb_map_szspacebar(b, (int16)(x + 4), (int16)(y - 2), (int16)(z - 1));
    mb_map_spacebarwait(b, wait);
}

// map_SBtype13: just an XP spacebar with init_z_rot=2, speed=4
static void mb_map_sbtype13(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z) {
    mb_map_xpspacebar(b, wait, x, y, z, 2, 4);
}

// map_SBtype18: XP spacebar with custom init/speed
static void mb_map_sbtype18(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                            int8 init_z_rot, int8 speed) {
    mb_map_xpspacebar(b, wait, x, y, z, init_z_rot, speed);
}

// map_SBtype19: SXP spacebar with custom init/speed
static void mb_map_sbtype19(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                            int8 init_z_rot, int8 speed) {
    mb_map_sxpspacebar(b, wait, x, y, z, init_z_rot, speed);
}

// map_SBtypeOBJ: mapobj in spacebar coordinate space
// wait,x,y,z are in spacebar units; shape/strat are literal.
static void mb_map_sbtypeOBJ(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                              uint16 shape, uint16 istrat) {
    mb_mapobj(b, (uint16)mb_spacebar_units((int16)wait),
              mb_spacebar_units(x),
              (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
              (int16)(SPACEBAR_BASE_DIST + mb_spacebar_units(z)),
              shape, istrat);
}

// Variant with raw strat address (for gate3_istrat etc.)
static void mb_map_sbtypeOBJ_nobj(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                                   uint16 shape, uint32 strat_addr) {
    mb_mapnobj(b, (uint16)mb_spacebar_units((int16)wait),
               mb_spacebar_units(x),
               (int16)(SPACE_VIEWCY + mb_spacebar_units(y)),
               (int16)(SPACEBAR_BASE_DIST + mb_spacebar_units(z)),
               shape, strat_addr);
}

static void mb_szaco2_mapobj(MapBuilder *b, int16 x, int16 y, int16 to_x, int16 to_y,
                             uint16 wait) {
    mb_mapobj(b, 0, x, y, 2000, SH_ZACO_8, IS_SZACO2);
    mb_setalxvarw(b, ALX_SWPX1, to_x);
    mb_setalxvarw(b, ALX_SWPY1, to_y);
    mb_setalvarb(b, AL_ROTX, (uint8)-DEG90);

    if (x == 0) {
        mb_setalvarb(b, AL_ROTY, DEG180);
    } else if (x < 0) {
        mb_setalvarb(b, AL_ROTY, (uint8)-DEG90);
    } else {
        mb_setalvarb(b, AL_ROTY, DEG90);
    }

    if (wait != 0u) {
        mb_mapwait(b, wait);
    }
}

// map_sfish MACRO: create a school of linked s_fish objects.
// The first fish is placed at (x,y,z); subsequent fish at (0,0,4000) linked
// via al_ptr -> mapvar1.
static void mb_map_sfish(MapBuilder *b, uint16 wait, int16 x, int16 y, int16 z,
                          uint16 count) {
    uint16 i;
    mb_mapobj(b, 0, x, y, z, SH_S_FISH, IS_SFISH);
    mb_setvarobj(b, WM_MAPVAR1);
    for (i = 1; i < count; i++) {
        mb_mapobj(b, 0, 0, 0, 4000, SH_S_FISH, IS_SFISH);
        mb_setalvarptrw(b, AL_PTR, WM_MAPVAR1);
    }
    mb_mapwait(b, wait);
}

static void append_map1_1a_submap(MapBuilder *b);

static void build_level1_1_opening_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level1_1_data;
    b.capacity = MAP_DATA_CAPACITY;
    s_level1_1_skillfly_bonus_guard_script_ptr = 0u;
    s_level1_1_skillfly_bonus_skip_ptr = 0u;
    s_level1_1_keep_player_strat_script_ptr = 0u;
    s_level1_1_mapwaitboss_trigse_script_ptr = 0u;
    s_level1_1_mapwaitboss_cantdie_script_ptr = 0u;
    s_level1_1_mapwaitboss_cleanup_script_ptr = 0u;

    // Literal LEVEL1_1.ASM slice through `MAP1_1B.ASM`, including the first
    // attack-carrier boss handoff. Opens with the scramble/launch intro:
    // `initlevel 1_1i` runs `pstrat playeropening` (started from boot.c),
    // then the wrapper jsrs into the shared MAP1_1A submap (appended below).
    mb_mapwait(&b, 100);
    mb_mapjsr(&b, "map1_1a");
    mb_qfadedown(&b);
    mb_waitfade(&b);
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);
    mb_mapwait(&b, 1);
    mb_setbg(&b, BG_1_1C);
    mb_initbg(&b);
    mb_mapwait(&b, MEDPSPEED * 2);
    mb_qfadeup(&b);
    mb_mapcode65816_inline(&b, &s_level1_1_keep_player_strat_script_ptr);
    mb_mapif_builtin(&b, MAP_CB_IS_PLAYER_DEAD, "level1_1.after_exitbase_setup");
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_EXITBASE_L);
    mb_label(&b, "level1_1.after_exitbase_setup");

    mb_mapobj(&b, 0, 0, 0, 0, SH_MYBASE_1, IS_NOCOLL);
    mb_mapobj(&b, 0, 0, 0, 0, SH_MYBASE_0, IS_NOCOLL);

    mb_mapobj(&b, 0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200,
              SH_MYSHIP_4, IS_FRIENDEXITBASE);
    mb_setalvarb(&b, AL_SBYTE1, 17);

    mb_mapobj(&b, 0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200,
              SH_MYSHIP_4, IS_FRIENDEXITBASE);
    mb_setalvarb(&b, AL_SBYTE1, (uint8)(17 + (1000 / PEXITBASE_SPEED)));

    mb_pathobj(&b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_MATEMSG, 10, 10);
    mb_pathobj(&b, 0, 100, -90, 1400, SH_FRIENDSHIP_4, PATH_ID_FALCO_LV1, 10, 10);
    mb_pathobj(&b, 0, -80, -140, 1200, SH_FRIENDSHIP_4, PATH_ID_FROG_LV1, 10, 10);

    mb_mapobj(&b, 0, -600, 0, 2000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 600, 0, 2000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -800, 0, 3500, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 800, 0, 3500, SH_BU_1, IS_HARD180YR);

    mb_label(&b, "level1_1.buloop");
    mb_mapobj(&b, 0, -1000, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 1500, 1000, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_maploop(&b, "level1_1.buloop", 3);

    mb_cspecial(&b, 0, -500, -300, 0, SH_ZACO_5, IS_ZACO1L);
    mb_mapobj(&b, 0, -1100, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 1500, 1100, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    mb_pathobj(&b, 0, 0, -400, -100, SH_FRIENDSHIP_4, PATH_ID_FROG1_1, 10, 10);
    mb_mapcodejsl_builtin(&b, MAP_CB_SETRESTART_L);
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_ONPLANET_L);
    mb_mapjsr(&b, "level1_1.map1_1b");

    mb_mapobj(&b, 500, 1000, 0, 8000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 1000, -800, 0, 10000, SH_BU_5, IS_HARD180YR);
    mb_mapobj(&b, 1000, -1200, 0, 12000, SH_BU_4, IS_HARD180YR);
    mb_mapjsr(&b, "cl_ground");
    mb_emit8(&b, MAP_OP_END);

    // CL_GND.ASM shared clear-demo submap used by the LEVEL1_1 tail.
    mb_label(&b, "cl_ground");
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapwait(&b, 2000);
    mb_setbgm(&b, BGM_FANFARE);
    mb_mapwait(&b, 3000);
    mb_setvarb(&b, WM_STAGECLEAR, 1);
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_CLEARDEMO_L);

    mb_sendmsg(&b, 1);
    mb_mapwait(&b, CL_GND_FRIENDWAIT);

    mb_mapif_builtin(&b, MAP_CB_FROG_ALIVE, "cl_ground.frog_alive");
    mb_mapgoto(&b, "cl_ground.nf");
    mb_label(&b, "cl_ground.frog_alive");
    mb_mapcodejsl_builtin(&b, MAP_CB_CLFRIENDMSG_FROG);
    mb_mapobj(&b, CL_GND_FRIENDWAIT, 500, -50, 50, SH_MYSHIP_4, IS_CLSHIPGNDB);
    mb_label(&b, "cl_ground.nf");

    mb_mapif_builtin(&b, MAP_CB_BUNNY_ALIVE, "cl_ground.bunny_alive");
    mb_mapgoto(&b, "cl_ground.nb");
    mb_label(&b, "cl_ground.bunny_alive");
    mb_mapcodejsl_builtin(&b, MAP_CB_CLFRIENDMSG_BUNNY);
    mb_mapobj(&b, CL_GND_FRIENDWAIT, -500, -50, 50, SH_MYSHIP_4, IS_CLSHIPGNDA);
    mb_label(&b, "cl_ground.nb");

    mb_mapif_builtin(&b, MAP_CB_COCK_ALIVE, "cl_ground.cock_alive");
    mb_mapgoto(&b, "cl_ground.nc");
    mb_label(&b, "cl_ground.cock_alive");
    mb_mapcodejsl_builtin(&b, MAP_CB_CLFRIENDMSG_COCK);
    mb_mapobj(&b, CL_GND_FRIENDWAIT, 0, -500, -300, SH_MYSHIP_4, IS_CLSHIPGNDC);
    mb_label(&b, "cl_ground.nc");

    mb_mapwait(&b, 3800);
    mb_setvarb(&b, WM_CLB2, 0);
    mb_setvarb(&b, WM_STAGECLEAR, 0);
    mb_mapcodejsl_builtin(&b, MAP_CB_CL_GROUND_PRINTLEVELFIN);
    mb_label(&b, "cl_ground.eswait");
    mb_mapwait(&b, 1);
    mb_maploop(&b, "cl_ground.eswait", 100);
    mb_mapcodejsl_builtin(&b, MAP_CB_CL_GROUND_WIPEOUT);
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapwait(&b, (uint16)(32u * MEDPSPEED));
    mb_setvarb(&b, WM_CLB2, 1);
    mb_maprts(&b);

    mb_label(&b, "level1_1.map1_1b");

    // MAP1_1B.ASM -> INCMAP <1-1>, first bounded chunk from 1-1.ASM:7-15.
    mb_pathobj(&b, 1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    mb_cspecial(&b, 0, -700, -500, 0, SH_ZACO_5, IS_ZACO1L);
    mb_mapobj(&b, 0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 1500, 1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 1500, 1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    mb_mapobj(&b, 0, 0, 0, 4000, SH_ARCH_0, IS_HARD);
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, -60, 4000, 100);
    mb_mapobj(&b, 0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 1500, 1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    mb_mapobj(&b, 0, 200, 0, 4000, SH_ARCH_0, IS_HARD);
    mb_skillfly_set(&b, 200, -60, 4000, 100);
    mb_mapobj(&b, 0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 1500, 1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    mb_skillfly_set(&b, -200, -60, 4000, 100);
    mb_mapobj(&b, 800, -200, 0, 4000, SH_ARCH_0, IS_HARD);

    mb_special(&b, 200, 400, -400, 0, SH_ZACO_A, IS_ZACO1R);
    mb_mapobj(&b, 0, 350, -30 << 2, 4000, SH_RADER_0, IS_RADER0);
    mb_mapobj(&b, 1000, 350, 0, 4000, SH_RADER_1, IS_RADER1);

    mb_cspecial(&b, 1500, 400, -400, -250, SH_ZACO_5, IS_ZACO1R);
    mb_skillfly_set(&b, 0, -60, 4000, 100);
    mb_mapobj(&b, 500, 0, 0, 4000, SH_ARCH_0, IS_HARD);

    mb_mapobj(&b, 0, -600, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 1500, 600, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 2000, 0, 0, 4500, SH_BIG_GATE, IS_HARD);

    mb_mapcode65816_inline(&b, &s_level1_1_skillfly_bonus_guard_script_ptr);
    mb_mapobj(&b, 0, 0, -50, 2500, SH_ITEM_7, IS_ITEM7);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level1_1.map1_1b.skillfly_bonus_0_skip");

    mb_pathcspecial(&b, 500, 200, -30, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    mb_mapobj(&b, 0, -600, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 3000, 600, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_pathobj(&b, 0, -500, 0, 4000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    mb_pathobj(&b, 1500, 500, 0, 4000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    mb_special(&b, 0, 0, -1000, 2300, SH_ZACO_A, IS_ZACOS);
    mb_cspecial(&b, 0, -200, -1300, 2300, SH_ZACO_6, IS_ZACOS);
    mb_cspecial(&b, 3500, 200, -1300, 2300, SH_ZACO_6, IS_ZACOS);
    mb_mapobj(&b, 0, 800, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 3000, -800, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_cspecial(&b, 0, -800, -300, 3000, SH_KAMIKAZE, IS_ZACO4);
    mb_pathobj(&b, 0, 0, 0, 4000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    mb_mapobj(&b, 0, 1200, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 600, -1200, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_cspecial(&b, 0, 800, -250, 3000, SH_KAMIKAZE, IS_ZACO4);
    mb_mapobj(&b, 0, 400, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 3500, -400, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_pathobj(&b, 0, 1200, 0, 3500, SH_NULLSHAPE, PATH_ID_ROBOTSWITHLOG, 6, 4);
    mb_setalvarb(&b, AL_ROTY, 64);
    mb_mapwait(&b, 0x0800);

    mb_mapobj(&b, 0, 200, 0, 5000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 2400, -200, 0, 5000, SH_BU_8, IS_HARD180YR);
    mb_pathobj(&b, 0, 750, -100, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE8_1, 10, 10);
    mb_pathcspecial(&b, 0, 3800, -3600, 4260, SH_ZACO_A, PATH_ID_CHASE8_2, 10, 10);
    mb_mapobj(&b, 0, 800, 0, 5000, SH_BU_6, IS_HARD180YR);
    mb_pathcspecial(&b, 0, 0, 0, 5000, SH_WALKER_2, PATH_ID_KORORI, 6, 4);
    mb_mapobj(&b, 0, 200, -50, 5000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -200, -50, 5000, SH_R_BU_1, IS_HARD180YR);
    mb_pathcspecial(&b, 2000, 750, -100, 0, SH_ZACO_A, PATH_ID_CHASE8_3, 10, 10);
    mb_mapobj(&b, 0, 200, -50, 5000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -200, -50, 5000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 2000, -800, 0, 5000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, 200, -50, 5000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -200, -50, 5000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 200, -50, 5000, SH_R_BU_1, IS_HARD180YR);
    mb_pathobj(&b, 0, -400, 0, 5000, SH_ROBOT_0, PATH_ID_ROBOT, 6, 4);
    mb_setalvarb(&b, AL_ROTY, (uint8)(-DEG45 - DEG22));
    mb_mapobj(&b, 1000, -200, -50, 5000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 800, 0, 5000, SH_BU_5, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, DEG90);
    mb_mapwait(&b, 0x2000);

    mb_mapexploderobot(&b);
    mb_mapobj(&b, 0, 820, 0, 4500, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 1400, -1200, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_cspecial(&b, 0, 300, -30, 4000, SH_BOM_WING, IS_BOMWING);
    mb_mapobj(&b, 0, -820, 0, 4500, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 2000, 820, 0, 4500, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -900, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 2800, 900, 0, 5000, SH_BU_0, IS_HARD180YR);

    mb_mapobj(&b, 0, -1000, 0, 4500, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 1000, 0, 4500, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0, -800, 0, 5000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, 500, 0, 5000, SH_BU_5, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, DEG90);
    mb_mapobj(&b, 2000, -350, 0, 5000, SH_BU_4, IS_HARD180YR);
    mb_mapobj(&b, 0, -400, 0, 4000, SH_BU_4, IS_HARD180YR);
    mb_mapobj(&b, 0, 400, 0, 4000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, DEG90);
    mb_mapwait(&b, 1400);
    mb_mapobj(&b, 1200, 0, 0, 4000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, -1000, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 600, 1000, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0, -450, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 700, 450, 0, 4000, SH_BU_6, IS_HARD180YR);

    mb_pathcspecial(&b, 1000, -1800, -600, 2000, SH_ZACO_5, PATH_ID_PATROL, 10, 10);
    mb_mapobj(&b, 1200, 100, 0, 4000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, -1000, 0, 4000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 600, 1000, 0, 4000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0, -400, 0, 4000, SH_BU_4, IS_HARD180YR);
    mb_mapobj(&b, 600, 450, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 600, -400, 0, 4000, SH_BU_4, IS_HARD180YR);
    mb_mapobj(&b, 0, 450, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 1400, -400, 0, 4000, SH_BU_4, IS_HARD180YR);
    mb_mapobj(&b, 1000, 0, 0, 4000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, -900, 0, 4000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 600, 900, 0, 4000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0, -400, 0, 4000, SH_BU_4, IS_HARD180YR);
    mb_mapobj(&b, 1000, 400, 0, 4000, SH_BU_5, IS_HARD90YR);
    mb_mapobj(&b, 0, 440, -230, 4050, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_mapobj(&b, 0, 400, 0, 4000, SH_BU_5, IS_HARD90YR);
    mb_mapobj(&b, 800, -400, 0, 4000, SH_BU_4, IS_HARD180YR);
    mb_pathcspecial(&b, 400, -1500, -700, 2000, SH_ZACO_5, PATH_ID_PATROL, 10, 10);
    mb_mapobj(&b, 0, 1000, 0, 4000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 500, -1000, 0, 4000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0, 350, 0, 4000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, DEG90);
    mb_pathcspecial(&b, 0, 0, 0, 5000, SH_WALKER_2, PATH_ID_KORORI, 6, 4);
    mb_mapobj(&b, 800, -350, 0, 4000, SH_BU_4, IS_HARD180YR);
    mb_mapobj(&b, 0, 350, 0, 4000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, DEG90);
    mb_mapobj(&b, 0, -350, 0, 4000, SH_BU_4, IS_HARD180YR);
    mb_pathcspecial(&b, 500, 2000, -500, 2000, SH_ZACO_5, PATH_ID_PATROL, 10, 10);
    mb_mapwait(&b, 800);
    mb_pathobj(&b, 0, 1300, 0, 3800, SH_NULLSHAPE, PATH_ID_ROBOTWITHLOG2, 6, 4);
    mb_setalvarb(&b, AL_ROTY, DEG90);
    mb_mapwait(&b, 500);
    mb_pathcspecial(&b, 0, 0, 0, 5000, SH_WALKER_2, PATH_ID_KORORI, 6, 4);
    mb_pathcspecial(&b, 2000, -200, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    mb_mapnobj(&b, 1000, 0, -100, 4000, SH_GATE_0, STRAT_ADDR_GATE3);

    mb_mapobj(&b, 0, -900, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 1000, 900, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 800, 350, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 800, -350, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, -900, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0, 900, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 800, 300, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 800, -250, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, -900, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0, 900, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 800, 250, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 100, -200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1500, 200, 0, 4000, SH_PILLAR3, IS_PILLAR3);

    mb_cspecial(&b, 100, 400, -600, -200, SH_ZACO_5, IS_ZACO1R);
    mb_cspecial(&b, 800, -400, -800, -200, SH_ZACO_5, IS_ZACO1L);

    mb_pathobj(&b, 0, -1000, 0, 3500, SH_NULLSHAPE, PATH_ID_ROBOTWITHLOG, 6, 4);
    mb_setalvarb(&b, AL_ROTY, (uint8)(-DEG45 - DEG22));

    mb_mapobj(&b, 0, -1000, 0, 6000, SH_BU_5, IS_HARD180YR);
    mb_mapobj(&b, 0, 1000, 0, 6000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 64);
    mb_mapwait(&b, 1000);
    mb_pathobj(&b, 0, -750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    mb_pathcspecial(&b, 2000, -720, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);
    mb_mapobj(&b, 1000, -1000, 0, 6000, SH_BU_5, IS_HARD180YR);
    mb_mapobj(&b, 0, 1300, 0, 6000, SH_BU_5, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 64);
    mb_mapwait(&b, 2000);
    mb_pathcspecial(&b, 0, 200, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);

    mb_pathobj(&b, 0, 800, 0, 3500, SH_NULLSHAPE, PATH_ID_ROBOTWITHLOG, 6, 4);
    mb_setalvarb(&b, AL_ROTY, (uint8)(DEG45 + DEG22));
    mb_mapobj(&b, 1000, -1000, 0, 6000, SH_BU_4, IS_HARD180YR);
    mb_mapobj(&b, 0, 1300, 0, 6000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 64);

    mb_mapobj(&b, 0, 0, -150, 4000, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    mb_pathcspecial(&b, 1000, -250, -1800, 0, SH_CARRIER, PATH_ID_E_UFO, 10, 10);
    mb_mapobj(&b, 0, 1300, 0, 6000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 2000, -1300, 0, 6000, SH_BU_2, IS_HARD180YR);
    mb_special(&b, 400, -400, -200, -200, SH_ZACO_A, IS_ZACO1L);
    mb_mapobj(&b, 0, -1300, 0, 7000, SH_BU_5, IS_HARD180YR);
    mb_mapobj(&b, 0, 1300, 0, 7000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 120);
    mb_mapwait(&b, 3000);
    mb_mapobj(&b, 0, 1300, 0, 7000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 3000, -1300, 0, 7000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0, 1300, 0, 6000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 120);
    mb_mapobj(&b, 4000, -1300, 0, 6000, SH_BU_5, IS_HARD180YR);
    mb_pathobj(&b, 0, -350, 0, 4000, SH_ROBOT_0, PATH_ID_ROBOT, 6, 4);
    mb_setalvarb(&b, AL_ROTY, (uint8)-DEG45);
    mb_mapwait(&b, 4000);

    // MAP1_1B.ASM boss block.
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapwait(&b, MEDPSPEED * 30);
    mb_setbgm(&b, BGM_BOSS1);
    mb_mapobj(&b, 0, 0, -(70 << BOSS7_SCALE), -200, SH_BOSS_7_1, IS_BOSS7);

    mb_mapcode65816_inline(&b, &s_level1_1_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level1_1.map1_1b.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level1_1.map1_1b.bosswait.cont");
    mb_mapgoto(&b, "level1_1.map1_1b.bosswait.loop");
    mb_label(&b, "level1_1.map1_1b.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level1_1_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level1_1_mapwaitboss_cleanup_script_ptr);
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);

    mb_maprts(&b);

    append_map1_1a_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level1_1 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level1_1.map1_1b.skillfly_bonus_0_skip",
                         &s_level1_1_skillfly_bonus_skip_ptr)) {
        s_level1_1 = s_empty_level;
        return;
    }

    s_level1_1.data = s_level1_1_data;
    s_level1_1.length = b.length;
}

static void build_level1_2_wrapper_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level1_2_data;
    b.capacity = sizeof(s_level1_2_data);
    s_level1_2_skillfly_bonus_guard_script_ptr = 0u;
    s_level1_2_skillfly_bonus_skip_ptr = 0u;
    s_level1_2_blackhole_bonus_guard_script_ptr = 0u;
    s_level1_2_blackhole_bonus_skip_ptr = 0u;
    s_level1_2_mapwaitboss_trigse_script_ptr = 0u;
    s_level1_2_mapwaitboss_cantdie_script_ptr = 0u;
    s_level1_2_mapwaitboss_cleanup_script_ptr = 0u;

    // LEVEL1_2.ASM wrapper. Keep the generic level init approximation already
    // used by this file, then hand off into the map body and clear-demo warp.
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);
    mb_mapwait(&b, 1);
    mb_mapwait(&b, 1);
    mb_mapcodejsl_builtin(&b, MAP_CB_SETRESTART_L);
    mb_mapjsr(&b, "level1_2.map1_2");
    mb_mapjsr(&b, "cl_warp");
    mb_mapend(&b, 4u);

    // MAP1_2.ASM:7-109 through the cameleon/item/friend block. Mother-map refs
    // are still placeholders until the MOTHERS.ASM submaps are ported.
    mb_label(&b, "level1_2.map1_2");
    mb_mapwait(&b, 1000);

    mb_cspecial(&b, 1800, 0, SPACE_VIEWCY - 1000, 800, SH_ZACO_4, IS_SZACO0);
    mb_pathobj(&b, 5000, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_ASTEMSG, 10, 10);
    mb_cspecial(&b, 2000, 1000, SPACE_VIEWCY, 800, SH_ZACO_4, IS_SZACO0);
    mb_cspecial(&b, 5000, 1000, SPACE_VIEWCY + 1000, 800, SH_ZACO_4, IS_SZACO0);

    mb_szaco2_mapobj(&b, 0, 2000, 0, 0, 100);
    mb_mapwait(&b, 500);
    mb_szaco2_mapobj(&b, -500, 2000, -300, 100, 0);
    mb_mapwait(&b, 500);
    mb_szaco2_mapobj(&b, -1000, 2000, -400, -100, 0);
    mb_mapwait(&b, 2000);
    mb_szaco2_mapobj(&b, 0, 2000, 0, 0, 100);
    mb_mapwait(&b, 500);
    mb_szaco2_mapobj(&b, 500, 2000, 300, 100, 100);
    mb_mapwait(&b, 500);
    mb_szaco2_mapobj(&b, 1000, 2000, 400, -100, 100);
    mb_mapwait(&b, 1500);

    mb_special(&b, 0, -250, SPACE_VIEWCY, 2500, SH_D_HEAD_0, IS_WORMHEAD);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapwait(&b, 150);

    mb_cspecial(&b, 0, -250, SPACE_VIEWCY, 2500, SH_D_BODY_0, IS_WORM);
    mb_setalvarptrw(&b, AL_SWORD1, WM_MAPVAR1);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapwait(&b, 150);
    mb_cspecial(&b, 0, -250, SPACE_VIEWCY, 2500, SH_D_BODY_0, IS_WORM);
    mb_setalvarptrw(&b, AL_SWORD1, WM_MAPVAR1);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapwait(&b, 150);
    mb_cspecial(&b, 0, -250, SPACE_VIEWCY, 2500, SH_D_BODY_0, IS_WORM);
    mb_setalvarptrw(&b, AL_SWORD1, WM_MAPVAR1);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapwait(&b, 150);
    mb_cspecial(&b, 0, -250, SPACE_VIEWCY, 2500, SH_D_BODY_0, IS_WORM);
    mb_setalvarptrw(&b, AL_SWORD1, WM_MAPVAR1);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapwait(&b, 150);
    mb_cspecial(&b, 0, -250, SPACE_VIEWCY, 2500, SH_D_BODY_0, IS_WORM);
    mb_setalvarptrw(&b, AL_SWORD1, WM_MAPVAR1);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapwait(&b, 150);

    mb_mapwait(&b, 4500);
    mb_mapmother(&b, 3500, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapremove(&b, SH_MOTHER1);

    mb_mapobj(&b, 2000, 200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 1000, 0, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    mb_pathobj(&b, 0, 1200, 200, 600, SH_FRIENDSHIP_4, PATH_ID_CHASE1_1, 200, 10);
    mb_pathcspecial(&b, 2000, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);
    mb_mapnobj(&b, 400, -400, SPACE_VIEWCY, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_mapobj(&b, 200, 200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 2000, 0, SPACE_VIEWCY - 1000, 800, SH_ZACO_4, IS_SZACO0);
    mb_mapnobj(&b, 1400, -400, SPACE_VIEWCY + 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_cspecial(&b, 1200, -200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_mapnobj(&b, 1400, 300, SPACE_VIEWCY - 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_mapobj(&b, 2000, -100, -200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);

    mb_special(&b, 0, -128, SPACE_VIEWCY + 128, 2000, SH_D_HEAD_0, IS_WORMHEAD);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapwait(&b, 150);
    for (uint8 i = 0; i < 5u; i++) {
        mb_cspecial(&b, 0, -128, SPACE_VIEWCY + 128, 2000, SH_D_BODY_0, IS_WORM);
        mb_setalvarptrw(&b, AL_SWORD1, WM_MAPVAR1);
        mb_setvarobj(&b, WM_MAPVAR1);
        mb_mapwait(&b, 150);
    }

    mb_mapnobj(&b, 1400, -300, SPACE_VIEWCY - 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_mapobj(&b, 2000, 100, 0, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 0, 200, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    mb_special(&b, 2000, -200, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    mb_mapnobj(&b, 400, 300, SPACE_VIEWCY - 300, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_cspecial(&b, 0, 0, SPACE_VIEWCY + 200, 800, SH_CAMELEON, IS_CAMELEON);
    mb_special(&b, 4000, 0, SPACE_VIEWCY - 200, 800, SH_CAMELEON, IS_CAMELEON);

    mb_mapmother(&b, 3000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_cspecial(&b, 4000, -200, 0, 4000, SH_ASTEROID2, IS_BREAK_METEORT);
    mb_mapremove(&b, SH_MOTHER1);

    mb_mapobj(&b, 1000, 100, SPACE_VIEWCY + 100, 3000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_mapmother(&b, 4000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapremove(&b, SH_MOTHER1);
    mb_mapobj(&b, 0, 0, SPACE_VIEWCY - 100, 6800, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_pathspecial(&b, 1000, 250, SPACE_VIEWCY, 7000, SH_WALKER_2, PATH_ID_PYONTA, 10, 10);
    mb_mapnobj(&b, 800, -300, SPACE_VIEWCY + 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_mapobj(&b, 800, 300, -200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);

    mb_pathobj(&b, 0, 900, -60, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE4_1, 200, 10);
    mb_pathcspecial(&b, 0, 900, -60, 0, SH_ZACO_B, PATH_ID_CHASE4_2, 200, 10);
    mb_pathcspecial(&b, 2000, 900, -60, 0, SH_ZACO_B, PATH_ID_CHASE4_3, 200, 10);
    mb_mapnobj(&b, 200, -400, SPACE_VIEWCY, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_mapobj(&b, 1800, 100, 200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);

    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, -50, 4000, 100);
    mb_cspecial(&b, 0, 180, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 0, -180, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 400, 0, -200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_mapnobj(&b, 300, 200, SPACE_VIEWCY + 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_mapmother(&b, 2000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapremove(&b, SH_MOTHER1);
    mb_mapmother(&b, 1300, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapremove(&b, SH_MOTHER1);

    mb_mapcode65816_inline(&b, &s_level1_2_skillfly_bonus_guard_script_ptr);
    mb_mapobj(&b, 0, 100, SPACE_VIEWCY - 100, 1500, SH_ITEM_7, IS_ITEM7);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level1_2.map1_2.skillfly_bonus_0_skip");

    mb_special(&b, 0, -128, SPACE_VIEWCY + 128, 2000, SH_D_HEAD_0, IS_WORMHEAD);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapwait(&b, 150);
    for (uint8 i = 0; i < 15u; i++) {
        mb_cspecial(&b, 0, -128, SPACE_VIEWCY + 128, 2000, SH_D_BODY_0, IS_WORM);
        mb_setalvarptrw(&b, AL_SWORD1, WM_MAPVAR1);
        mb_setvarobj(&b, WM_MAPVAR1);
        mb_mapwait(&b, 150);
    }
    mb_mapobj(&b, 0, -128, SPACE_VIEWCY + 128, 2000, SH_D_BODY_0, IS_WORM);
    mb_setalvarptrw(&b, AL_SWORD1, WM_MAPVAR1);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapwait(&b, 2500);

    mb_cspecial(&b, 1000, 200, SPACE_VIEWCY - 500, 3000, SH_TADPOLE, IS_TADPOLE);
    mb_skillfly_init(&b);
    mb_skillfly_set_default(&b, 0, SPACE_VIEWCY - 100, 4000);
    mb_pathcspecial(&b, 1000, 0, -100, 4000, SH_NULLSHAPE, PATH_ID_INSEKIKUN, 10, 10);
    mb_special(&b, 1000, 1000, SPACE_VIEWCY + 100, 3000, SH_TADPOLE, IS_TADPOLE);
    mb_pathcspecial(&b, 400, -200, 200, 4000, SH_B_HOU_0, PATH_ID_SCREW, 10, 10);
    mb_mapmother(&b, 200, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapremove(&b, SH_MOTHER1);
    mb_pathcspecial(&b, 200, 100, -100, 4000, SH_B_HOU_0, PATH_ID_DAMYSCR, 10, 10);
    mb_mapmother(&b, 200, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapremove(&b, SH_MOTHER1);

    mb_pathcspecial(&b, 2000, 200, -200, 4000, SH_B_HOU_0, PATH_ID_SCREW, 10, 10);
    mb_mapobj(&b, 2000, -200, -200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_pathcspecial(&b, 1800, 300, -100, 4000, SH_B_HOU_0, PATH_ID_DAMYSCR, 10, 10);
    mb_pathcspecial(&b, 400, -300, 0, 4000, SH_B_HOU_0, PATH_ID_SCREW, 10, 10);
    mb_mapobj(&b, 800, 300, -100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_skillfly_set_default(&b, 0, SPACE_VIEWCY - 100, 4000);
    mb_pathcspecial(&b, 1000, 0, -100, 4000, SH_NULLSHAPE, PATH_ID_INSEKIKUN, 10, 10);
    mb_mapobj(&b, 1000, -100, 0, 4000, SH_ASTEROID2, IS_BREAK_METEORT);
    mb_mapmother(&b, 1000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapremove(&b, SH_MOTHER1);
    mb_skillfly_set_default(&b, -200, SPACE_VIEWCY - 100, 4000);
    mb_pathcspecial(&b, 1000, -200, -100, 4000, SH_NULLSHAPE, PATH_ID_INSEKIKUN, 10, 10);
    mb_pathcspecial(&b, 1000, 0, -200, 3500, SH_B_HOU_0, PATH_ID_DAMYSCR, 10, 10);
    mb_mapobj(&b, 1000, -400, -100, 4000, SH_ASTEROID2, IS_BREAK_METEORT);
    mb_mapobj(&b, 1000, 200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEORT);

    mb_mapcode65816_inline(&b, &s_level1_2_blackhole_bonus_guard_script_ptr);
    mb_mapobj(&b, 0, -300, SPACE_VIEWCY + 100, 3000, SH_ASTEROID2, IS_BLACKHOLE);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level1_2.map1_2.blackhole_bonus_skip");
    mb_cspecial(&b, 1500, -100, 0, 4000, SH_ASTEROID2, IS_BREAK_METEORT);
    mb_mapnobj(&b, 1200, -100, SPACE_VIEWCY - 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_maprts(&b);

    // MAP1_2.ASM:195 map12boss subroutine.
    mb_label(&b, "level1_2.map12boss");
    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_BOSS1);
    mb_mapobj(&b, 0, 0, SPACE_VIEWCY + 1000, 1500, SH_BOSS_1_2, IS_BOSS1);

    mb_mapwait(&b, 100);
    mb_mapcode65816_inline(&b, &s_level1_2_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level1_2.map12boss.waitboss.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level1_2.map12boss.waitboss.cont");
    mb_mapgoto(&b, "level1_2.map12boss.waitboss.loop");
    mb_label(&b, "level1_2.map12boss.waitboss.cont");
    mb_mapcode65816_inline(&b, &s_level1_2_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level1_2_mapwaitboss_cleanup_script_ptr);
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);
    mb_mapwait(&b, 1800);
    mb_maprts(&b);

    // CL_WARP.ASM clear-demo slice.
    mb_label(&b, "cl_warp");
    mb_mapplayeroutview(&b);
    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_FANFARE);
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_WARP_L);
    mb_mapwait(&b, 2800);

    mb_setvarb(&b, WM_STAGECLEAR, 1);
    mb_sendmsg(&b, 1);
    mb_mapwait(&b, CL_WARP_FRIENDWAIT);

    mb_mapif_builtin(&b, MAP_CB_FROG_ALIVE, "cl_warp.frog_alive");
    mb_mapgoto(&b, "cl_warp.nf");
    mb_label(&b, "cl_warp.frog_alive");
    mb_mapobj(&b, CL_WARP_FRIENDWAIT, 300, -60, 50, SH_MYSHIP_4, IS_CLSHIPWARPB);
    mb_mapcodejsl_builtin(&b, MAP_CB_CLFRIENDMSG_FROG);
    mb_label(&b, "cl_warp.nf");

    mb_mapif_builtin(&b, MAP_CB_BUNNY_ALIVE, "cl_warp.bunny_alive");
    mb_mapgoto(&b, "cl_warp.nb");
    mb_label(&b, "cl_warp.bunny_alive");
    mb_mapobj(&b, CL_WARP_FRIENDWAIT, -300, -60, 50, SH_MYSHIP_4, IS_CLSHIPWARPA);
    mb_mapcodejsl_builtin(&b, MAP_CB_CLFRIENDMSG_BUNNY);
    mb_label(&b, "cl_warp.nb");

    mb_mapif_builtin(&b, MAP_CB_COCK_ALIVE, "cl_warp.cock_alive");
    mb_mapgoto(&b, "cl_warp.nc");
    mb_label(&b, "cl_warp.cock_alive");
    mb_mapobj(&b, CL_WARP_FRIENDWAIT, 0, -100, -3000, SH_MYSHIP_4, IS_CLSHIPWARPC);
    mb_mapcodejsl_builtin(&b, MAP_CB_CLFRIENDMSG_COCK);
    mb_label(&b, "cl_warp.nc");

    mb_mapwait(&b, 500);

    // `mother_1` from MOTHERS.ASM and `mother1_istrat` are still unported.
    // Keep the wrapper structure literal, but use a bounded placeholder map ref.
    mb_mapmother(&b, 10000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_setvarb(&b, WM_CLB2, 0);
    mb_setvarb(&b, WM_STAGECLEAR, 0);
    mb_mapcodejsl_builtin(&b, MAP_CB_CL_WARP_PRINTLEVELFIN);
    mb_label(&b, "cl_warp.eswait");
    mb_mapwait(&b, 1);
    mb_maploop(&b, "cl_warp.eswait", 100);
    mb_setvarb(&b, WM_CLB2, 2);
    mb_setvarb(&b, WM_ONECREDSPR, 0);
    mb_mapwait(&b, 2000);
    mb_mapremove(&b, SH_MOTHER1);
    mb_mapwait(&b, 9000);
    mb_setvarb(&b, WM_CLB2, 1);
    mb_maprts(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level1_2 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level1_2.map1_2.skillfly_bonus_0_skip",
                         &s_level1_2_skillfly_bonus_skip_ptr)) {
        s_level1_2 = s_empty_level;
        return;
    }
    if (!mb_lookup_label(&b, "level1_2.map1_2.blackhole_bonus_skip",
                         &s_level1_2_blackhole_bonus_skip_ptr)) {
        s_level1_2 = s_empty_level;
        return;
    }

    s_level1_2.data = s_level1_2_data;
    s_level1_2.length = b.length;
}

static void build_level1_3_opening_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level1_3_data;
    b.capacity = sizeof(s_level1_3_data);

    // LEVEL1_3.ASM — Space Armada level wrapper.
    // Opening: initlevel 1_3i,whitefadeout,0
    mb_qfadedown(&b);
    mb_waitfade(&b);
    mb_setbg(&b, BG_1_3I);
    mb_initbg(&b);
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);
    mb_mapcodejsl_builtin(&b, MAP_CB_INITFADEWHITE2NORM_L);

    // Line 4: mapjsr cl_warpout
    mb_mapjsr(&b, "level1_3.cl_warpout");

    // Line 10: mapjsr map1_3a (SPACE section)
    mb_mapjsr(&b, "level1_3.map1_3a");

    // LEVEL1_3.ASM lines 16-23: SHIP1 bounded section
    // .start1: mapjsr map1_3a1 (ship1 interior)
    mb_mapjsr(&b, "level1_3.map1_3a1");

    // LEVEL1_3.ASM lines 24-26: setbg 1_3b, initbg, mapjsr map1_3b1 (tunnel)
    // map1_3b1 is incmap 1-3-t1 + mapjsr mtunnelexit; stub for now.
    mb_setbg(&b, BG_1_3B);
    mb_initbg(&b);
    mb_mapwait(&b, 500);  // placeholder for incmap 1-3-t1 tunnel data
    mb_mapwait(&b, 100);  // placeholder for mtunnelexit

    // LEVEL1_3.ASM lines 34-47: SHIP2 bounded section
    // .start2: mapjsr map1_3a2 (ship2 interior)
    mb_mapjsr(&b, "level1_3.map1_3a2");

    // setbg 1_3b, initbg, mapjsr map1_3b2 (tunnel)
    mb_setbg(&b, BG_1_3B);
    mb_initbg(&b);
    mb_mapjsr(&b, "level1_3.map1_3b2");

    // LEVEL1_3.ASM lines 49-67: .bigship section
    mb_setbg(&b, BG_1_3C);
    mb_mapwait(&b, 100);  // maptexitwait -100 placeholder
    mb_initbg(&b);
    mb_mapjsr(&b, "level1_3.map1_3c");

    // .washroom: 8x bou_1 HARD180yr obstacles
    mb_mapobj(&b, 0x0000, 0x0070, -060, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0070, -060, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0070, -060, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0070, -060, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0070, -060, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0070, -060, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0070, -060, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0070, -060, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
    // incmap washent — WASHENT.ASM: 1-3 Boss Entry Cutscene (colony pipe entrance)
    // Lines 7-8: mapplayercantdie / mapplayermode toCslow
    //   — These opcodes are not yet implemented in the map executor. Skipped.
    // TODO: mb_mapplayercantdie(&b);
    // TODO: mb_mapplayermode(&b, PLAYER_MODE_TOCSLOW);

    // Lines 10-12: three pipe background objects (mapobjnomem)
    //   — mapobjnomem is not yet implemented. Emit as regular mapobj.
    mb_mapobj(&b, 0, 0, -60, 4200, SH_PIPE_9_0_PROXY, IS_NOCOLL);
    mb_mapobj(&b, 400, 0, -60, 4200, SH_PIPE_9_0_PROXY, IS_NOCOLL);
    mb_mapobj(&b, 0, 0, -60, 4200, SH_PIPE_9_PROXY, IS_COLONYEXIT);
    // Line 13: mapwait 4000
    mb_mapwait(&b, 4000);

    // Lines 17-18: setbg 1_3da / initbg — background setup
    //   — setbg/initbg for bg id '1_3da' is not mapped yet. Skip for now.

    // Lines 20-45: mappipe sequence (colony pipe path, 16 segments)
    //   — The mappipe opcode is not yet implemented in the map executor.
    //     TODO: Implement mappipe opcode in world.c and port these calls.
    // mappipe 0,0,0,0,0
    // mappipe -11,40,-1,0,2
    // mappipe -40,70,-2,1,2
    // mappipewait
    // mappipe -69,100,-1,0,3
    // mappipewait
    // mappipe -80,140,0,1,0
    // mappipewait
    // mappipe -69,180,1,0,3
    // mappipewait
    // mappipe -40,210,2,0,2,nognd
    // mappipewait
    // mappipe -11,240,1,0,1,nognd
    // mappipewait / mappipewait
    // mappipe 0,280,0,0,4,nognd
    // mappipewait
    // mappipe 0,320,0,0,5,nognd
    // mappipe 0,360,0,0,4,nognd
    // mappipe 0,400,0,0,5,nognd
    // mappipe 0,440,0,0,4,nognd
    // mappipe 0,480,0,0,5,nognd
    // mappipewait x4
    // End of WASHENT.ASM inline

    // mapjsr map1_3d (washing machine room) — stub
    mb_mapjsr(&b, "level1_3.map1_3d");

    // .fin: mapjsr cl_ship1_3, mapend
    mb_mapjsr(&b, "cl_ship1_3");
    mb_mapend(&b, 1u);

    // CL_WARPO.ASM:1-7.
    mb_label(&b, "level1_3.cl_warpout");
    mb_mapplayeroutview(&b);
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_WARP_L);
    mb_mapwait(&b, 10000);
    mb_maprts(&b);

    // MAP1_3A.ASM:2-33.
    mb_label(&b, "level1_3.map1_3a");
    mb_cspecial(&b, 1000, 100, SPACE_VIEWCY - 100, 3000, SH_ZACO_7, IS_SZACO5);

    mb_map_farships2(&b, -2000, -500, 9000, -30, 8, 2);
    mb_map_farships2(&b, -1000, 0, 9000, -10, 20, 4);
    mb_map_farships1(&b, 1500, -300, 9000, 20, 18, 2);
    mb_map_farships0(&b, 2000, 0, 9000, 10, 10, 3);
    mb_map_farships2(&b, -800, 500, 8000, -20, 16, 2);
    mb_map_farships2(&b, -1500, 1200, 8000, -30, 20, 2);
    mb_map_farships0(&b, 1000, -800, 7700, 10, -8, 2);
    mb_map_farships1(&b, 0, -1200, 7700, 16, -40, 2);
    mb_map_farships2(&b, 500, -1000, 7700, 20, -16, 1);

    mb_mapobj(&b, 2500, -500, -300, 3000, SH_W_L, IS_WINGLAZERMAN);

    mb_map_farships2(&b, -2500, -300, 8000, -30, 15, 2);
    mb_mapwait(&b, 1000);
    mb_map_farships2(&b, 0, -1200, 8000, 16, -40, 1);
    mb_mapwait(&b, 1000);
    mb_map_farships1(&b, 500, -1000, 6000, 30, -20, 2);
    mb_mapwait(&b, 3000);

    mb_map_farships0(&b, 500, -500, 6000, 50, -30, 1);

    mb_cspecial(&b, 1000, 0, SPACE_VIEWCY - 200, 3000, SH_ZACO_7, IS_SZACO5);
    mb_cspecial(&b, 0, 400, SPACE_VIEWCY + 200, 3000, SH_ZACO_7, IS_SZACO5);
    mb_cspecial(&b, 3000, -400, SPACE_VIEWCY + 200, 3000, SH_ZACO_7, IS_SZACO5);

    mb_mapobj(&b, 0, 100, -100, 5000, SH_NULLSHAPE, IS_UP1MAN);
    mb_maprts(&b);

    // MAP1_3A1.ASM — ship1 interior subroutine
    append_map1_3a1_submap(&b);

    // MAP1_3A2.ASM — ship2 interior subroutine
    append_map1_3a2_submap(&b);

    // MAP1_3B2.ASM — ship2 tunnel subroutine
    append_map1_3b2_submap(&b);

    // MAP1_3C subroutine — Space Armada part C (big ship interior)
    s_map1_3c_chkstratdone1_loop_ptr = 0u;
    s_map1_3c_chkstratdone1_end_ptr = 0u;
    mb_label(&b, "level1_3.map1_3c");

    // Lines 4-6: near_side cruiser
    mb_mapnobj(&b, 0, -1000, SPACE_VIEWCY, 350, SH_SHIP_4_PROXY, STRAT_ADDR_CRUISER1);
    mb_setalvarb(&b, AL_VEL, 200u);
    mb_setalvarb(&b, AL_ROTZ, 230u);

    // Lines 8-11: normal far cruiser
    mb_mapnobj(&b, 0, -3400, SPACE_VIEWCY + 100, 3000, SH_SHIP_4_PROXY, STRAT_ADDR_CRUISER1F);
    mb_setalvarb(&b, AL_SBYTE1, 25);
    mb_setalvarb(&b, AL_VEL, 55);
    mb_setalvarb(&b, AL_ROTZ, 20);
    mb_mapwait(&b, 2000);

    // Lines 14-16: far_big_ship
    mb_mapnobj(&b, 0, 600, SPACE_VIEWCY, 8000, SH_SHIP_0_C_PROXY, STRAT_ADDR_SHIP3A);
    mb_setalvarb(&b, AL_VEL, 125);
    mb_setalvarb(&b, AL_ROTX, 10);

    // Line 18: cspecial r_hou_0
    mb_cspecial(&b, 0, -100, -200, 5000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 20-23: from_top cruiser
    mb_mapnobj(&b, 0, SPACE_MINX - 2000, SPACE_VIEWCY - 3000, 3000,
               SH_SHIP_4_PROXY, STRAT_ADDR_CRUISER1);
    mb_setalvarb(&b, AL_VEL, 100);
    mb_setalvarb(&b, AL_ROTX, 25);
    mb_setalvarb(&b, AL_ROTZ, 230u);
    mb_mapwait(&b, 3500);

    // Lines 26-28: reverse cruiser
    mb_mapnobj(&b, 0, -2500, SPACE_VIEWCY - 100, 4000,
               SH_SHIP_4_PROXY, STRAT_ADDR_CRUISER1F);
    mb_setalvarb(&b, AL_VEL, 55);
    mb_setalvarb(&b, AL_ROTZ, 150);
    mb_mapwait(&b, 1000);

    // Lines 30-31: cspecial r_hou_0 pair
    mb_cspecial(&b, 2000, 0x0200, 0x0300, 5000, SH_R_HOU_0, IS_SHOU0A);
    mb_cspecial(&b, 9000, -0x0200, -0x0200, 5000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 33-34: gate + pathobj e_gate
    mb_mapobj(&b, 2000, 0, 100, 5000, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 4000, 3000, 3000, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 36-38: pathspecial/pathcspecial escorts
    mb_pathspecial(&b, 800, 600, 400, -100, SH_S_ZACO_0, PATH_ID_PATRET, 10, 10);
    mb_pathcspecial(&b, 800, 500, -100, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    mb_pathcspecial(&b, 4000, -400, 200, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);

    // Line 39: cspecial s_hou_0
    mb_cspecial(&b, 1000, 0, 0x0200, 4000, SH_S_HOU_0, IS_SHOU0);

    // Lines 46-48: big ship approach (spsdist=13000, sphigh=6000)
    mb_mapnobj(&b, 0, 0, 6000, 13000, SH_SHIP_0_C_PROXY, STRAT_ADDR_SHIP3);
    mb_setvarobj(&b, WM_MAPVAR1);

    // Lines 50-53: bshipexitface door 1 (below)
    mb_mapnobj(&b, 0, 0, 6000 - 140, 13000 - 240,
               SH_BSHIPEXITFACE_PROXY, STRAT_ADDR_EXITOPENSND2);
    mb_setalvarw(&b, AL_SWORD1, 400);
    mb_setalvarptrw(&b, AL_SWORD2, WM_MAPVAR1);
    mb_setalvarb(&b, AL_SBYTE1, (uint8)(int8)-10);

    // Lines 56-59: bshipexitface door 2 (above)
    mb_mapnobj(&b, 0, 0, 6000 + 140, 13000 - 240,
               SH_BSHIPEXITFACE_PROXY, STRAT_ADDR_EXITOPENSND2);
    mb_setalvarw(&b, AL_SWORD1, 400);
    mb_setalvarptrw(&b, AL_SWORD2, WM_MAPVAR1);
    mb_setalvarb(&b, AL_SBYTE1, 10);

    // Lines 62-71: wait, fade music, boss music
    mb_mapwait(&b, 4000);
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapwait(&b, (uint16)(MEDPSPEED * 7u));
    mb_setbgm(&b, BGM_BOSS1);

    // Lines 75-77: .loop — chkstratdone1 busy-wait
    mb_label(&b, "level1_3.map1_3c.loop");
    mb_mapwait(&b, 16);
    mb_mapcode65816_inline(&b, &s_map1_3c_chkstratdone1_loop_ptr);
    mb_mapgoto(&b, "level1_3.map1_3c.loop");
    mb_label(&b, "level1_3.map1_3c.cont");

    // Lines 79-80: setbg 1_3b, initbg
    mb_setbg(&b, BG_1_3B);
    mb_initbg(&b);

    // Line 83: incmap 1-3-t3 — tunnel transition data (stub for now)
    // TODO: port 1-3-T3.ASM tunnel data when tunnel door shapes are available
    mb_mapwait(&b, 500);

    mb_maprts(&b);

    // MAP1_3D subroutine — Space Armada part D (washing machine boss)
    mb_label(&b, "level1_3.map1_3d");
    // INCMAP washmape — wash entrance map data (stub)
    mb_mapwait(&b, 500);
    // markboss boss13
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);
    mb_maprts(&b);

    // CL_SHIP1_3 — shared clear-demo for ship levels (already appended elsewhere)
    // Append the cl_ship submap so the label resolves.
    append_cl_ship_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level1_3 = s_empty_level;
        return;
    }

    s_level1_3.data = s_level1_3_data;
    s_level1_3.length = b.length;

    // Look up MAP1_3C inline callback pointers.
    if (!mb_lookup_label(&b, "level1_3.map1_3c.cont",
                         &s_map1_3c_chkstratdone1_end_ptr)) {
        s_map1_3c_chkstratdone1_end_ptr = 0u;
    }
}

static void register_level1_3_inline_callbacks(void) {
    if (s_map1_3c_chkstratdone1_loop_ptr != 0u) {
        World_RegisterInlineMapCode(s_map1_3c_chkstratdone1_loop_ptr,
                                    map1_3c_chkstratdone1_check);
    }
}

static void append_cl_earth_submap(MapBuilder *b) {
    if (!b) {
        return;
    }

    // CL_EARTH.ASM shared clear-demo helper for LEVEL2_2.
    mb_label(b, "cl_earth");
    mb_mapmother(b, 0, 0, 0, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapplayeroutview(b);
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(b, MAP_CB_SET_PLAYER_CLEAR_EARTH_L);
    mb_setbgm(b, BGM_FANFARE);
    mb_mapwait(b, 3300);

    mb_setvarb(b, WM_STAGECLEAR, 1);
    mb_sendmsg(b, 1);
    mb_mapwait(b, CL_GND_FRIENDWAIT);

    mb_mapif_builtin(b, MAP_CB_FROG_ALIVE, "cl_earth.frog_alive");
    mb_mapgoto(b, "cl_earth.nf");
    mb_label(b, "cl_earth.frog_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, 2000, -50, 50, SH_MYSHIP_4, IS_CLSHIPEARTHB);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_FROG);
    mb_label(b, "cl_earth.nf");

    mb_mapif_builtin(b, MAP_CB_BUNNY_ALIVE, "cl_earth.bunny_alive");
    mb_mapgoto(b, "cl_earth.nb");
    mb_label(b, "cl_earth.bunny_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, -2000, -50, 50, SH_MYSHIP_4, IS_CLSHIPEARTHA);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_BUNNY);
    mb_label(b, "cl_earth.nb");

    mb_mapif_builtin(b, MAP_CB_COCK_ALIVE, "cl_earth.cock_alive");
    mb_mapgoto(b, "cl_earth.nc");
    mb_label(b, "cl_earth.cock_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, 0, 1000, -700, SH_MYSHIP_4, IS_CLSHIPEARTHC);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_COCK);
    mb_label(b, "cl_earth.nc");

    mb_mapwait(b, 5000);
    mb_label(b, "cl_earth.sdloop");
    mb_mapif_builtin(b, MAP_CB_CHKSTAGEDONE, "cl_earth.sdcont");
    mb_mapgoto(b, "cl_earth.sdloop");
    mb_label(b, "cl_earth.sdcont");
    mb_setvarb(b, WM_CLB2, 0);
    mb_setvarb(b, WM_STAGECLEAR, 0);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_PRINTLEVELFIN);
    mb_label(b, "cl_earth.eswait");
    mb_mapwait(b, 1);
    mb_maploop(b, "cl_earth.eswait", 100);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_WIPEOUT);
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapwait(b, (uint16)(90u * MEDPSPEED));
    mb_setvarb(b, WM_CLB2, 1);
    mb_maprts(b);
}

static void append_cl_chase_submap(MapBuilder *b) {
    if (!b) {
        return;
    }

    // CL_CHASE.ASM shared clear-demo helper for LEVEL3_2.
    mb_label(b, "cl_chase");
    mb_mapmother(b, 0, 0, 0, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapplayeroutview(b);
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(b, MAP_CB_SET_PLAYER_CLEAR_CHASE_L);
    mb_setbgm(b, BGM_FANFARE);
    mb_mapwait(b, 3800);

    mb_setvarb(b, WM_STAGECLEAR, 1);
    mb_sendmsg(b, 1);
    mb_mapwait(b, CL_GND_FRIENDWAIT);

    mb_mapif_builtin(b, MAP_CB_FROG_ALIVE, "cl_chase.frog_alive");
    mb_mapgoto(b, "cl_chase.nf");
    mb_label(b, "cl_chase.frog_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, 1000, -300, 50, SH_MYSHIP_4, IS_CLSHIPCHASEA);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_FROG);
    mb_label(b, "cl_chase.nf");

    mb_mapif_builtin(b, MAP_CB_BUNNY_ALIVE, "cl_chase.bunny_alive");
    mb_mapgoto(b, "cl_chase.nb");
    mb_label(b, "cl_chase.bunny_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, -2000, -300, 50, SH_MYSHIP_4, IS_CLSHIPCHASEB);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_BUNNY);
    mb_label(b, "cl_chase.nb");

    mb_mapif_builtin(b, MAP_CB_COCK_ALIVE, "cl_chase.cock_alive");
    mb_mapgoto(b, "cl_chase.nc");
    mb_label(b, "cl_chase.cock_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, 0, 0, -2000, SH_MYSHIP_4, IS_CLSHIPCHASEC);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_COCK);
    mb_label(b, "cl_chase.nc");

    mb_mapwait(b, 7000);
    mb_setvarb(b, WM_CLB2, 0);
    mb_setvarb(b, WM_STAGECLEAR, 0);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_PRINTLEVELFIN);
    mb_label(b, "cl_chase.eswait");
    mb_mapwait(b, 1);
    mb_maploop(b, "cl_chase.eswait", 100);
    mb_maprts(b);
}

// ----------------------------------------------------------------
// CL_SHIP.ASM — clear demo for route-1 ship levels (3_4 and 1_3).
// Two entry points: cl_ship3_4 (colony bg) and cl_ship1_3 (Sship bg).
// Both jump to shared cl_ship_cont.
// ----------------------------------------------------------------
static void append_cl_ship_submap(MapBuilder *b) {
    if (!b) {
        return;
    }

    // cl_ship3_4 entry point
    mb_label(b, "cl_ship3_4");
    mb_setbg(b, BG_3_4D);
    mb_initbg(b);
    mb_mapcodejsl_builtin(b, MAP_CB_SET_PLAYER_CLEAR_SHIP2_L);
    mb_setbgm(b, BGM_FANFARE);
    mb_mapobj(b, 0, 0, SPACE_VIEWCY, 0, SH_COLONY_0_PROXY, STRAT_ADDR_SHIP0CDOWN);
    mb_setalvarb(b, AL_ROTY, DEG180);
    mb_mapgoto(b, "cl_ship.cont");

    // cl_ship1_3 entry point
    mb_label(b, "cl_ship1_3");
    mb_setbg(b, BG_1_3E);
    mb_initbg(b);
    mb_mapcodejsl_builtin(b, MAP_CB_SET_PLAYER_CLEAR_SHIP2_L);
    mb_setbgm(b, BGM_FANFARE);
    mb_mapobj(b, 0, 0, SPACE_VIEWCY, 0, SH_SSHIP_0_C_PROXY, STRAT_ADDR_SHIP0CDOWN);

    // cl_ship_cont shared continuation
    mb_label(b, "cl_ship.cont");
    mb_mapwait(b, (uint16)(9000u - CL_GND_FRIENDWAIT));
    mb_mapmother(b, 0, 0, 0, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);

    mb_setvarb(b, WM_STAGECLEAR, 1);
    mb_sendmsg(b, 1);
    mb_mapwait(b, CL_GND_FRIENDWAIT);

    mb_mapif_builtin(b, MAP_CB_FROG_ALIVE, "cl_ship.frog_alive");
    mb_mapgoto(b, "cl_ship.nf");
    mb_label(b, "cl_ship.frog_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, -1000, -50, 50, SH_MYSHIP_4, IS_CLSHIPSHIPA);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_FROG);
    mb_label(b, "cl_ship.nf");

    mb_mapif_builtin(b, MAP_CB_BUNNY_ALIVE, "cl_ship.bunny_alive");
    mb_mapgoto(b, "cl_ship.nb");
    mb_label(b, "cl_ship.bunny_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, 1000, -50, 50, SH_MYSHIP_4, IS_CLSHIPSHIPB);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_BUNNY);
    mb_label(b, "cl_ship.nb");

    mb_mapif_builtin(b, MAP_CB_COCK_ALIVE, "cl_ship.cock_alive");
    mb_mapgoto(b, "cl_ship.nc");
    mb_label(b, "cl_ship.cock_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, 0, 200, -500, SH_MYSHIP_4, IS_CLSHIPSHIPC);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_COCK);
    mb_label(b, "cl_ship.nc");

    mb_mapwait(b, 3000);
    mb_setvarb(b, WM_CLB2, 0);
    mb_setvarb(b, WM_STAGECLEAR, 0);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_PRINTLEVELFIN);
    mb_label(b, "cl_ship.sdloop");
    mb_mapif_builtin(b, MAP_CB_CHKSTAGEDONE, "cl_ship.sdcont");
    mb_mapgoto(b, "cl_ship.sdloop");
    mb_label(b, "cl_ship.sdcont");
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_WIPEOUT);
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapwait(b, (uint16)(45u * MEDPSPEED * 2u));
    mb_maprts(b);
}

// ----------------------------------------------------------------
// CL_UNDER.ASM — clear demo for underwater levels.
// ----------------------------------------------------------------
static void append_cl_under_submap(MapBuilder *b) {
    if (!b) {
        return;
    }

    mb_label(b, "cl_under");
    mb_mapplayeroutview(b);
    mb_mapwait(b, 1000);
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapwait(b, 2000);
    mb_setbgm(b, BGM_FANFARE);
    mb_mapwait(b, 3000);

    mb_setvarb(b, WM_STAGECLEAR, 1);
    mb_mapcodejsl_builtin(b, MAP_CB_SET_PLAYER_CLEAR_UNDER_L);

    mb_sendmsg(b, 1);
    mb_mapwait(b, CL_GND_FRIENDWAIT);

    mb_mapif_builtin(b, MAP_CB_FROG_ALIVE, "cl_under.frog_alive");
    mb_mapgoto(b, "cl_under.nf");
    mb_label(b, "cl_under.frog_alive");
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_FROG);
    mb_mapobj(b, CL_GND_FRIENDWAIT, 1000, -300, 50, SH_MYSHIP_4, IS_CLSHIPUNDERA);
    mb_label(b, "cl_under.nf");

    mb_mapif_builtin(b, MAP_CB_BUNNY_ALIVE, "cl_under.bunny_alive");
    mb_mapgoto(b, "cl_under.nb");
    mb_label(b, "cl_under.bunny_alive");
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_BUNNY);
    mb_mapobj(b, CL_GND_FRIENDWAIT, -2000, -300, 50, SH_MYSHIP_4, IS_CLSHIPUNDERB);
    mb_label(b, "cl_under.nb");

    mb_mapif_builtin(b, MAP_CB_COCK_ALIVE, "cl_under.cock_alive");
    mb_mapgoto(b, "cl_under.nc");
    mb_label(b, "cl_under.cock_alive");
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_COCK);
    mb_mapobj(b, CL_GND_FRIENDWAIT, 0, 0, -2000, SH_MYSHIP_4, IS_CLSHIPUNDERC);
    mb_label(b, "cl_under.nc");

    mb_mapwait(b, 3800);
    mb_setvarb(b, WM_CLB2, 0);
    mb_setvarb(b, WM_STAGECLEAR, 0);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_PRINTLEVELFIN);
    mb_label(b, "cl_under.eswait");
    mb_mapwait(b, 1);
    mb_maploop(b, "cl_under.eswait", 100);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_WIPEOUT);
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapwait(b, (uint16)(32u * MEDPSPEED));
    mb_maprts(b);
}

// ----------------------------------------------------------------
// CL_DIVE.ASM — clear demo for dive levels.
// Has inline 65816 to clear engine sound, handled via callback.
// ----------------------------------------------------------------
static void append_cl_dive_submap(MapBuilder *b) {
    if (!b) {
        return;
    }

    mb_label(b, "cl_dive");
    mb_mapplayeroutview(b);
    mb_setbgm(b, BGM_FADEOUT);
    mb_setbgm(b, BGM_FANFARE);
    mb_mapcodejsl_builtin(b, MAP_CB_SET_PLAYER_DIVE_L);
    mb_mapwait(b, 2800);

    mb_setvarb(b, WM_STAGECLEAR, 1);
    mb_sendmsg(b, 1);
    mb_mapwait(b, CL_GND_FRIENDWAIT);

    mb_mapif_builtin(b, MAP_CB_FROG_ALIVE, "cl_dive.frog_alive");
    mb_mapgoto(b, "cl_dive.nf");
    mb_label(b, "cl_dive.frog_alive");
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_FROG);
    mb_mapobj(b, CL_GND_FRIENDWAIT, 200, SPACE_VIEWCY, 50, SH_MYSHIP_4, IS_CLSHIPDIVEB);
    mb_label(b, "cl_dive.nf");

    mb_mapif_builtin(b, MAP_CB_BUNNY_ALIVE, "cl_dive.bunny_alive");
    mb_mapgoto(b, "cl_dive.nb");
    mb_label(b, "cl_dive.bunny_alive");
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_BUNNY);
    mb_mapobj(b, CL_GND_FRIENDWAIT, -200, SPACE_VIEWCY, 50, SH_MYSHIP_4, IS_CLSHIPDIVEA);
    mb_label(b, "cl_dive.nb");

    mb_mapif_builtin(b, MAP_CB_COCK_ALIVE, "cl_dive.cock_alive");
    mb_mapgoto(b, "cl_dive.nc");
    mb_label(b, "cl_dive.cock_alive");
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_COCK);
    mb_mapobj(b, CL_GND_FRIENDWAIT, 0, (int16)(SPACE_VIEWCY - 40), -50, SH_MYSHIP_4, IS_CLSHIPDIVEC);
    mb_label(b, "cl_dive.nc");

    mb_mapwait(b, 5000);
    mb_setvarb(b, WM_CLB2, 0);
    mb_setvarb(b, WM_STAGECLEAR, 0);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_PRINTLEVELFIN);
    mb_label(b, "cl_dive.eswait");
    mb_mapwait(b, 1);
    mb_maploop(b, "cl_dive.eswait", 100);

    // Inline 65816: clear engine sound flag (pshipflags3 &= ~psf3_enginesnd)
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_DIVE_CLEAR_ENGINESND);
    mb_qfadedown(b);
    mb_waitfade(b);
    mb_setvarb(b, WM_CLB2, 1);
    mb_maprts(b);
}

// ----------------------------------------------------------------
// CL_BRIDG.ASM — clear demo for bridge levels.
// ----------------------------------------------------------------
static void append_cl_bridge_submap(MapBuilder *b) {
    if (!b) {
        return;
    }

    mb_label(b, "cl_bridge");
    mb_mapplayeroutview(b);
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapwait(b, 2200);

    mb_mapcodejsl_builtin(b, MAP_CB_SET_PLAYER_CLEAR_BRIDGE_L);
    mb_setbgm(b, BGM_FANFARE);
    mb_mapwait(b, 2900);

    mb_setvarb(b, WM_STAGECLEAR, 1);
    mb_sendmsg(b, 1);
    mb_mapwait(b, CL_GND_FRIENDWAIT);

    mb_mapif_builtin(b, MAP_CB_FROG_ALIVE, "cl_bridge.frog_alive");
    mb_mapgoto(b, "cl_bridge.nf");
    mb_label(b, "cl_bridge.frog_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, -1000, -300, 50, SH_MYSHIP_4, IS_CLSHIPBRIDGEB);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_FROG);
    mb_label(b, "cl_bridge.nf");

    mb_mapif_builtin(b, MAP_CB_BUNNY_ALIVE, "cl_bridge.bunny_alive");
    mb_mapgoto(b, "cl_bridge.nb");
    mb_label(b, "cl_bridge.bunny_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, 1000, -300, 50, SH_MYSHIP_4, IS_CLSHIPBRIDGEA);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_BUNNY);
    mb_label(b, "cl_bridge.nb");

    mb_mapif_builtin(b, MAP_CB_COCK_ALIVE, "cl_bridge.cock_alive");
    mb_mapgoto(b, "cl_bridge.nc");
    mb_label(b, "cl_bridge.cock_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, 0, 0, -2000, SH_MYSHIP_4, IS_CLSHIPBRIDGEC);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_COCK);
    mb_label(b, "cl_bridge.nc");

    mb_mapwait(b, 5000);
    mb_setvarb(b, WM_CLB2, 0);
    mb_setvarb(b, WM_STAGECLEAR, 0);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_PRINTLEVELFIN);
    mb_label(b, "cl_bridge.eswait");
    mb_mapwait(b, 1);
    mb_maploop(b, "cl_bridge.eswait", 100);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_WIPEOUT);
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapwait(b, (uint16)(32u * MEDPSPEED));
    mb_maprts(b);
}

// ----------------------------------------------------------------
// CL_TURN.ASM — clear demo for turn levels.
// Has clfish subroutine using map_sfish.
// ----------------------------------------------------------------
static void append_cl_turn_submap(MapBuilder *b) {
    if (!b) {
        return;
    }

    mb_label(b, "cl_turn");
    mb_mapplayeroutview(b);
    mb_setbgm(b, BGM_FADEOUT);
    mb_setbgm(b, BGM_FANFARE);
    mb_mapwait(b, 1800);

    mb_mapjsr(b, "cl_turn.clfish");

    mb_mapcodejsl_builtin(b, MAP_CB_SET_PLAYER_CLEAR_TURN_L);
    mb_mapwait(b, 1000);

    mb_setvarb(b, WM_STAGECLEAR, 1);
    mb_sendmsg(b, 1);
    mb_mapwait(b, CL_GND_FRIENDWAIT);

    mb_mapif_builtin(b, MAP_CB_FROG_ALIVE, "cl_turn.frog_alive");
    mb_mapgoto(b, "cl_turn.nf");
    mb_label(b, "cl_turn.frog_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, 700, SPACE_VIEWCY, 50, SH_MYSHIP_4, IS_CLSHIPTURNB);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_FROG);
    mb_label(b, "cl_turn.nf");

    mb_mapif_builtin(b, MAP_CB_BUNNY_ALIVE, "cl_turn.bunny_alive");
    mb_mapgoto(b, "cl_turn.nb");
    mb_label(b, "cl_turn.bunny_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, -500, SPACE_VIEWCY, 50, SH_MYSHIP_4, IS_CLSHIPTURNA);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_BUNNY);
    mb_label(b, "cl_turn.nb");

    mb_mapif_builtin(b, MAP_CB_COCK_ALIVE, "cl_turn.cock_alive");
    mb_mapgoto(b, "cl_turn.nc");
    mb_label(b, "cl_turn.cock_alive");
    mb_mapobj(b, CL_GND_FRIENDWAIT, 0, (int16)(SPACE_VIEWCY + 400), -3000, SH_MYSHIP_4, IS_CLSHIPTURNC);
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_COCK);
    mb_label(b, "cl_turn.nc");

    mb_mapwait(b, 4000);
    mb_mapjsr(b, "cl_turn.clfish");
    mb_mapwait(b, 4000);

    mb_setvarb(b, WM_CLB2, 0);
    mb_setvarb(b, WM_STAGECLEAR, 0);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_PRINTLEVELFIN);
    mb_mapwait(b, 9000);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_WIPEOUT);
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapwait(b, (uint16)(32u * MEDPSPEED));
    mb_maprts(b);

    // clfish subroutine: map_sfish 0,0000,100,1000,9
    mb_label(b, "cl_turn.clfish");
    mb_map_sfish(b, 0, 0, 100, 1000, 9);
    mb_maprts(b);
}

// ----------------------------------------------------------------
// CL_WARPO.ASM — clear demo for warp-out (simplest clear demo).
// ----------------------------------------------------------------
static void append_cl_warpout_submap(MapBuilder *b) {
    if (!b) {
        return;
    }

    mb_label(b, "cl_warpout");
    mb_mapcodejsl_builtin(b, MAP_CB_SET_PLAYER_WARPOUT_L);
    mb_mapwait(b, 10000);
    mb_maprts(b);
}

// ----------------------------------------------------------------
// MAP1_1A.ASM — shared scramble submap (intro flyover sequence).
// Appended as a callable subroutine into any level that references it.
// ----------------------------------------------------------------
static void append_map1_1a_submap(MapBuilder *b) {
    if (!b) {
        return;
    }

    mb_label(b, "map1_1a");
    mb_mapobj(b, 0, 0, 0, 250, SH_OP_0, IS_GND);
    mb_setalxvarb(b, ALX_DEPTHOFFSET, 1);
    mb_mapobj(b, 0, 0, 0, 250, SH_OP_1, IS_NOCOLL);
    mb_setalxvarb(b, ALX_DEPTHOFFSET, 1);
    mb_mapobj(b, 0, 0, 0, 250 + (100 << 3), SH_OP_0, IS_GND);
    mb_setalxvarb(b, ALX_DEPTHOFFSET, 1);
    mb_mapobj(b, 0, 0, 0, 250 + (100 << 3), SH_OP_1, IS_NOCOLL);
    mb_setalxvarb(b, ALX_DEPTHOFFSET, 1);
    mb_mapobj(b, 0, 0, 0, 250 + (200 << 3), SH_OP_0, IS_GND);
    mb_setalxvarb(b, ALX_DEPTHOFFSET, 1);
    mb_mapobj(b, 0, 0, 0, 250 + (200 << 3), SH_OP_1, IS_NOCOLL);
    mb_setalxvarb(b, ALX_DEPTHOFFSET, 1);

    mb_mapobj(b, 0, -40, 0, -200, SH_IMYSHIP_4, IS_SHIPINTRO);
    mb_setalvarw(b, AL_SWORD1, (uint16)-70);
    mb_setalvarb(b, AL_SBYTE1, 60);
    mb_mapobj(b, 0, 40, 0, -200, SH_IMYSHIP_4, IS_SHIPINTRO);
    mb_setalvarw(b, AL_SWORD1, (uint16)-70);
    mb_setalvarb(b, AL_SBYTE1, 50);
    mb_mapobj(b, 0, 0, 0, -300, SH_IMYSHIP_4, IS_SHIPINTRO);
    mb_setalvarw(b, AL_SWORD1, (uint16)-100);
    mb_setalvarb(b, AL_SBYTE1, (uint8)-1);

    mb_label(b, "map1_1a.here2");
    mb_mapwait(b, (100 << 3) - MEDPSPEED);
    mb_mapobj(b, 0, 0, 0, 250 + (200 << 3), SH_OP_0, IS_GND);
    mb_setalxvarb(b, ALX_DEPTHOFFSET, 1);
    mb_mapobj(b, 0, 0, 0, 250 + (200 << 3), SH_OP_1, IS_NOCOLL);
    mb_setalxvarb(b, ALX_DEPTHOFFSET, 1);
    mb_maploop(b, "map1_1a.here2", 8);

    mb_label(b, "map1_1a.here3");
    mb_mapwait(b, (100 << 3) - MEDPSPEED);
    mb_mapobj(b, 0, 0, 0, 250 + (200 << 3), SH_OP_2, IS_GND);
    mb_setalxvarb(b, ALX_DEPTHOFFSET, 1);
    mb_mapif_builtin(b, MAP_CB_CHKSTRATDONE1, "map1_1a.fin");
    mb_mapgoto(b, "map1_1a.here3");
    mb_label(b, "map1_1a.fin");
    mb_maprts(b);
}

// ----------------------------------------------------------------
// CL_GND.ASM — clear demo for ground levels.
// ----------------------------------------------------------------
static void append_cl_ground_submap(MapBuilder *b) {
    if (!b) {
        return;
    }

    mb_label(b, "cl_ground");
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapwait(b, 2000);
    mb_setbgm(b, BGM_FANFARE);
    mb_mapwait(b, 3000);
    mb_setvarb(b, WM_STAGECLEAR, 1);
    mb_mapcodejsl_builtin(b, MAP_CB_SET_PLAYER_CLEARDEMO_L);

    mb_sendmsg(b, 1);
    mb_mapwait(b, CL_GND_FRIENDWAIT);

    mb_mapif_builtin(b, MAP_CB_FROG_ALIVE, "cl_ground.frog_alive");
    mb_mapgoto(b, "cl_ground.nf");
    mb_label(b, "cl_ground.frog_alive");
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_FROG);
    mb_mapobj(b, CL_GND_FRIENDWAIT, 500, -50, 50, SH_MYSHIP_4, IS_CLSHIPGNDB);
    mb_label(b, "cl_ground.nf");

    mb_mapif_builtin(b, MAP_CB_BUNNY_ALIVE, "cl_ground.bunny_alive");
    mb_mapgoto(b, "cl_ground.nb");
    mb_label(b, "cl_ground.bunny_alive");
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_BUNNY);
    mb_mapobj(b, CL_GND_FRIENDWAIT, -500, -50, 50, SH_MYSHIP_4, IS_CLSHIPGNDA);
    mb_label(b, "cl_ground.nb");

    mb_mapif_builtin(b, MAP_CB_COCK_ALIVE, "cl_ground.cock_alive");
    mb_mapgoto(b, "cl_ground.nc");
    mb_label(b, "cl_ground.cock_alive");
    mb_mapcodejsl_builtin(b, MAP_CB_CLFRIENDMSG_COCK);
    mb_mapobj(b, CL_GND_FRIENDWAIT, 0, -500, -300, SH_MYSHIP_4, IS_CLSHIPGNDC);
    mb_label(b, "cl_ground.nc");

    mb_mapwait(b, 3800);
    mb_setvarb(b, WM_CLB2, 0);
    mb_setvarb(b, WM_STAGECLEAR, 0);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_PRINTLEVELFIN);
    mb_label(b, "cl_ground.eswait");
    mb_mapwait(b, 1);
    mb_maploop(b, "cl_ground.eswait", 100);
    mb_mapcodejsl_builtin(b, MAP_CB_CL_GROUND_WIPEOUT);
    mb_setbgm(b, BGM_FADEOUT);
    mb_mapwait(b, (uint16)(32u * MEDPSPEED));
    mb_setvarb(b, WM_CLB2, 1);
    mb_maprts(b);
}

static void build_level2_1_wrapper_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level2_1_data;
    b.capacity = sizeof(s_level2_1_data);
    s_level2_1_keep_player_strat_script_ptr = 0u;
    s_level2_1_mapwaitboss_trigse_script_ptr = 0u;
    s_level2_1_mapwaitboss_cantdie_script_ptr = 0u;
    s_level2_1_mapwaitboss_cleanup_script_ptr = 0u;
    s_level2_1_skillfly_bonus0_guard_script_ptr = 0u;
    s_level2_1_skillfly_bonus0_skip_ptr = 0u;
    s_level2_1_skillfly_bonus1_guard_script_ptr = 0u;
    s_level2_1_skillfly_bonus1_skip_ptr = 0u;

    // LEVEL2_1.ASM through the first handoff into MAP2_1B.
    mb_mapwait(&b, 100);
    mb_mapjsr(&b, "map1_1a");
    mb_qfadedown(&b);
    mb_waitfade(&b);
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);
    mb_mapwait(&b, 1);
    mb_setbg(&b, BG_1_1C);
    mb_initbg(&b);
    mb_mapwait(&b, MEDPSPEED * 2);
    mb_qfadeup(&b);
    mb_mapcode65816_inline(&b, &s_level2_1_keep_player_strat_script_ptr);
    mb_mapif_builtin(&b, MAP_CB_IS_PLAYER_DEAD, "level2_1.after_exitbase_setup");
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_EXITBASE_L);
    mb_label(&b, "level2_1.after_exitbase_setup");

    mb_mapobj(&b, 0, 0, 0, 0, SH_MYBASE_1, IS_NOCOLL);
    mb_mapobj(&b, 0, 0, 0, 0, SH_MYBASE_0, IS_NOCOLL);

    mb_mapobj(&b, 0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200,
              SH_MYSHIP_4, IS_FRIENDEXITBASE);
    mb_setalvarb(&b, AL_SBYTE1, 17);
    mb_mapobj(&b, 0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200,
              SH_MYSHIP_4, IS_FRIENDEXITBASE);
    mb_setalvarb(&b, AL_SBYTE1, (uint8)(17 + (1000 / PEXITBASE_SPEED)));

    mb_pathobj(&b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_MATEMSG, 10, 10);
    mb_pathobj(&b, 0, 100, -90, 1400, SH_FRIENDSHIP_4, PATH_ID_FALCO_LV1, 10, 10);
    mb_pathobj(&b, 0, -80, -140, 1200, SH_FRIENDSHIP_4, PATH_ID_FROG_LV1, 10, 10);

    mb_mapobj(&b, 0, -600, 0, 2000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 600, 0, 2000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -700, 0, 3500, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 700, 0, 3500, SH_BU_1, IS_HARD180YR);

    mb_label(&b, "level2_1.tower");
    mb_mapobj(&b, 0, -1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 2000, 1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_maploop(&b, "level2_1.tower", 2);
    mb_cspecial(&b, 0, -500, -300, 0, SH_ZACO_5, IS_ZACO1L);
    mb_mapobj(&b, 0, -1200, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 2000, 1200, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 0, -1200, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 0, 1200, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_cspecial(&b, 0, 500, -300, 0, SH_ZACO_5, IS_ZACO1R);

    mb_mapcodejsl_builtin(&b, MAP_CB_SETRESTART_L);
    mb_mapif_builtin(&b, MAP_CB_IS_PLAYER_DEAD, "level2_1.after_onplanet_setup");
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_ONPLANET_L);
    mb_label(&b, "level2_1.after_onplanet_setup");
    mb_mapjsr(&b, "level2_1.map2_1b");
    mb_emit8(&b, MAP_OP_END);

    mb_label(&b, "level2_1.map2_1b");
    // MAP2_1B / 2-1.ASM literal body slice through the opening 2-1-3 block.
    mb_mapcodejsl_builtin(&b, MAP_CB_SETRESTART_L);
    mb_pathobj(&b, 1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    mb_cspecial(&b, 0, -700, -500, 0, SH_ZACO_5, IS_ZACO1L);
    mb_mapobj(&b, 0, -1200, 0, 5200, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 2000, 1200, 0, 5200, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 0, -1200, 0, 5500, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 2000, 1200, 0, 5500, SH_TOWER_2, IS_TOWER0);

    mb_cspecial(&b, 0, -200, -600, -500, SH_ZACO_5, IS_ZACO1L);
    mb_pathobj(&b, 0, -500, 0, 5000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    mb_pathobj(&b, 0, 500, 0, 5000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    mb_mapobj(&b, 0, -600, 0, 4000, SH_HOUDAI_0, IS_HOUDAINS);
    mb_cspecial(&b, 1000, -500, -200, 4000, SH_KAMIKAZE, IS_ZACO3);

    mb_pathobj(&b, 0, -850, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    mb_pathcspecial(&b, 0, -820, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);

    mb_pathcspecial(&b, 400, 0, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    mb_mapobj(&b, 0, -1000, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 2000, 1000, 0, 5000, SH_BU_0, IS_HARD180YR);

    mb_mapobj(&b, 0, -300, -110, 4000, SH_RADER_0, IS_RADER0);
    mb_mapobj(&b, 0, -300, 0, 4000, SH_RADER_1, IS_RADER1);
    mb_mapobj(&b, 0, 300, -110, 4000, SH_RADER_0, IS_RADER0);
    mb_mapobj(&b, 1000, 300, 0, 4000, SH_RADER_1, IS_RADER1);
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, -60, 4000, 100);
    mb_mapobj(&b, 1000, 0, 0, 4000, SH_ARCH_0, IS_HARD);

    mb_pathobj(&b, 0, -800, 0, 5000, SH_ROBOT_0, PATH_ID_ROBOT, 6, 4);
    mb_setalvarb(&b, AL_ROTY, (uint8)-DEG45);
    mb_mapobj(&b, 0, 1000, 0, 5000, SH_BU_5, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapwait(&b, 2000);
    mb_mapobj(&b, 0, -1000, 0, 5000, SH_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 64);
    mb_mapobj(&b, 0, -200, 0, 4000, SH_ARCH_0, IS_HARD);
    mb_skillfly_set(&b, -200, -60, 4000, 100);
    mb_mapwait(&b, 2000);

    mb_pathobj(&b, 0, 1600, 0, 4000, SH_NULLSHAPE, PATH_ID_ROBOTWITHLOG, 6, 4);
    mb_setalvarb(&b, AL_ROTY, (uint8)(DEG45 + DEG22));
    mb_mapobj(&b, 1200, 800, 0, 5000, SH_BU_6, IS_HARD180YR);
    mb_skillfly_set(&b, -300, -60, 4000, 100);
    mb_mapobj(&b, 1200, -300, 0, 4000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 2000, -700, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 700, 0, 5000, SH_BU_4, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_skillfly_set(&b, 200, -60, 4000, 100);
    mb_mapobj(&b, 2000, 200, 0, 4000, SH_ARCH_0, IS_HARD);

    mb_mapobj(&b, 400, -800, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 400, 800, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_special(&b, 0, -300, -1300, 1800, SH_CARRIER, IS_CARRIER);
    mb_mapobj(&b, 0, -600, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 400, 600, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -500, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 400, 500, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -400, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 400, 400, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 200, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 3000, -200, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_cspecial(&b, 0, 0, -400, 1000, SH_PARA_0, IS_PARA);
    mb_cspecial(&b, 0, 100, -500, 1000, SH_PARA_0, IS_PARA);
    mb_cspecial(&b, 500, -100, -500, 1000, SH_PARA_0, IS_PARA);

    mb_mapobj(&b, 0, -400, 0, 2000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 500, 400, 0, 2000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -600, 0, 4000, SH_BU_5, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapobj(&b, 0, 600, 0, 4000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapwait(&b, 1000);
    mb_mapobj(&b, 0, -600, 0, 4000, SH_BU_5, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapobj(&b, 0, 600, 0, 4000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapwait(&b, 1000);
    mb_mapobj(&b, 0, 400, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapobj(&b, 0, -400, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapwait(&b, 800);
    mb_mapobj(&b, 1000, 0, 0, 4000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 200, 0, 4000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 1000, -200, 0, 4000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, -700, 0, 3500, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, 700, 0, 3500, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, 400, 0, 4000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapobj(&b, 0, -400, 0, 4000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapwait(&b, 1000);
    mb_mapobj(&b, 0, 400, 0, 4000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 96);

    mb_cspecial(&b, 0, -1800, -600, 4400, SH_ZACO_5, IS_ZACO0);
    mb_mapobj(&b, 0, -400, 0, 4000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapwait(&b, 2000);

    mb_cspecial(&b, 100, 0, -1400, 2100, SH_ZACO_6, IS_ZACOS);
    mb_special(&b, 0, -150, -1500, 2300, SH_ZACO_A, IS_ZACOS);
    mb_special(&b, 100, 150, -1500, 2300, SH_ZACO_A, IS_ZACOS);
    mb_cspecial(&b, 0, 300, -1700, 2600, SH_ZACO_6, IS_ZACOS);
    mb_cspecial(&b, 2000, -300, -1700, 2600, SH_ZACO_6, IS_ZACOS);
    mb_mapobj(&b, 0, -1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 2000, 1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 0, -1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 2000, 1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_pathobj(&b, 0, 300, 0, 4000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    mb_pathobj(&b, 3000, -300, 0, 4000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    mb_mapobj(&b, 0, 600, 0, 4000, SH_BU_4, IS_HARD180YR);
    mb_mapobj(&b, 1000, -600, 0, 4000, SH_BU_5, IS_HARD180YR);
    mb_mapobj(&b, 0, 300, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 400, -300, 0, 4000, SH_BU_6, IS_HARD180YR);

    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, -60, 4000, 20 << 2);
    mb_mapobj(&b, 200, 0, 0, 4000, SH_ARCH_0, IS_HARD);
    mb_pathobj(&b, 0, 0, -350, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    mb_pathcspecial(&b, 3000, 0, -350, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);
    mb_pathcspecial(&b, 0, 150, -50, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);

    mb_maphardrot(&b, 0, 400, 0, 4000, SH_TOWER_2, 0, 4, 0);
    mb_maphardrot(&b, 800, -400, 0, 4000, SH_TOWER_2, 0, -4, 0);

    mb_mapcode65816_inline(&b, &s_level2_1_skillfly_bonus0_guard_script_ptr);
    mb_mapnobj(&b, 0, 0, -80, 1500, SH_GATE_0, STRAT_ADDR_GATE3);
    mb_label(&b, "level2_1.map2_1b.skillfly_bonus_0_skip");

    mb_skillfly_set(&b, 0, -60, 4000, 20 << 2);
    mb_mapobj(&b, 1500, 0, 0, 4000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 0, 100, 0, 4000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 500, -100, 0, 4000, SH_BU_8, IS_HARD180YR);
    mb_maphardrot(&b, 0, 480, 0, 4000, SH_TOWER_2, 0, 6, 0);
    mb_maphardrot(&b, 1000, -480, 0, 4000, SH_TOWER_2, 0, -6, 0);
    mb_skillfly_set(&b, 0, -60, 4000, 20 << 2);
    mb_mapobj(&b, 1000, 0, 0, 4000, SH_ARCH_0, IS_HARD);

    mb_skillfly_set(&b, 250, -60, 4000, 100);
    mb_pathobj(&b, 0, -1500, 0, 4000, SH_NULLSHAPE, PATH_ID_ROBOTSWITHLOG, 6, 4);
    mb_setalvarb(&b, AL_ROTY, (uint8)-DEG90);
    mb_pathcspecial(&b, 0, 300, -50, 3500, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    mb_cspecial(&b, 2200, -100, -30, 3500, SH_BOM_WING, IS_BOMWING);

    mb_mapobj(&b, 0, 1400, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1200, -1400, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, 1200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1200, -1200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapcode65816_inline(&b, &s_level2_1_skillfly_bonus1_guard_script_ptr);
    mb_mapobj(&b, 0, 100, -80, 1500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level2_1.map2_1b.skillfly_bonus_1_skip");
    mb_mapobj(&b, 0, 1000, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1200, -1000, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, 800, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1200, -800, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, 600, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1400, -500, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, -900, 0, 5000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, 900, 0, 5000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, 400, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1200, -400, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, -300, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1000, 300, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, 250, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1000, -250, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, 200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 900, -200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, -700, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0, 700, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0, 150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 800, -150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, 150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 700, -150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, 200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 600, -200, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_pathcspecial(&b, 0, 300, -50, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    mb_mapobj(&b, 0, 150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1000, -150, 0, 4000, SH_PILLAR3, IS_PILLAR3);

    mb_mapobj(&b, 0, 0, -150, 4000, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    mb_mapwait(&b, 1000);
    mb_mapobj(&b, 0, 240, 0, 4000, SH_BU_4, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 64);
    mb_mapobj(&b, 0, -240, 0, 4000, SH_BU_4, IS_HARD180YR);
    mb_mapobj(&b, 0, 0, -120, 4200, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_pathcspecial(&b, 1400, -1500, -700, 2000, SH_ZACO_5, PATH_ID_PATROL, 2, 10);
    // Finish the remaining 2-1.ASM tail, then fall through into MAP2_1B.ASM.
    mb_pathcspecial(&b, 1000, 0, -50, 3500, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    mb_mapwait(&b, 1000);
    mb_special(&b, 0, 0, -1300, 2000, SH_ZACO_A, IS_ZACOS);
    mb_cspecial(&b, 0, 200, -1500, 2200, SH_ZACO_6, IS_ZACOS);
    mb_cspecial(&b, 100, -200, -1500, 2200, SH_ZACO_6, IS_ZACOS);
    mb_cspecial(&b, 0, 300, -1700, 2400, SH_ZACO_6, IS_ZACOS);
    mb_cspecial(&b, 2500, -300, -1700, 2400, SH_ZACO_6, IS_ZACOS);

    mb_mapobj(&b, 0, 1400, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapobj(&b, 0, -1400, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTY, 96);
    mb_mapwait(&b, 2500);
    mb_pathobj(&b, 0, 800, 0, 5000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    mb_pathobj(&b, 4500, -800, 0, 5000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);

    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_BOSS1);
    mb_mapobj(&b, 0, 0, -(70 << BOSS7_SCALE), -200, SH_BOSS_7_1, IS_BOSS7);
    mb_mapcode65816_inline(&b, &s_level2_1_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level2_1.map2_1b.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level2_1.map2_1b.bosswait.cont");
    mb_mapgoto(&b, "level2_1.map2_1b.bosswait.loop");
    mb_label(&b, "level2_1.map2_1b.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level2_1_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level2_1_mapwaitboss_cleanup_script_ptr);
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);
    mb_maprts(&b);

    append_cl_earth_submap(&b);
    append_map1_1a_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level2_1 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level2_1.map2_1b.skillfly_bonus_0_skip",
                         &s_level2_1_skillfly_bonus0_skip_ptr)) {
        s_level2_1 = s_empty_level;
        return;
    }
    if (!mb_lookup_label(&b, "level2_1.map2_1b.skillfly_bonus_1_skip",
                         &s_level2_1_skillfly_bonus1_skip_ptr)) {
        s_level2_1 = s_empty_level;
        return;
    }

    s_level2_1.data = s_level2_1_data;
    s_level2_1.length = b.length;
}

static void build_level2_2_wrapper_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level2_2_data;
    b.capacity = sizeof(s_level2_2_data);
    s_level2_2_mapwaitboss_trigse_script_ptr = 0u;
    s_level2_2_mapwaitboss_cantdie_script_ptr = 0u;
    s_level2_2_mapwaitboss_cleanup_script_ptr = 0u;

    // LEVEL2_2.ASM wrapper around MAP2_2.ASM and CL_EARTH.ASM.
    mb_mapjsr(&b, "map2_2");
    mb_mapjsr(&b, "cl_earth");
    mb_mapend(&b, 1u);

    // MAP2_2.ASM literal port through the moving shooter wall and boss handoff.
    mb_label(&b, "map2_2");
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);

    mb_mapwait(&b, 600);

    mb_cspecial(&b, 1500, 0, SPACE_VIEWCY - 1000, 800, SH_ZACO_4, IS_SZACO0);
    mb_cspecial(&b, 1500, 1000, SPACE_VIEWCY - 500, 800, SH_ZACO_4, IS_SZACO0);
    mb_pathobj(&b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_ASTEMSG, 10, 10);
    mb_cspecial(&b, 5000, 1000, SPACE_VIEWCY, 800, SH_ZACO_4, IS_SZACO0);
    mb_pathobj(&b, 0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    mb_pathcspecial(&b, 9000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);

    mb_pathcspecial(&b, 0, 2500, -2000, 1800, SH_ZACO_8, PATH_ID_EGU6, 10, 10);
    mb_pathcspecial(&b, 12000, -2500, -2000, 2100, SH_ZACO_8, PATH_ID_EGU6, 10, 10);

    mb_pathobj(&b, 0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    mb_pathcspecial(&b, 0,
                    2 * SPACEBAR_UNIT_LEN,
                    SPACE_VIEWCY - SPACEBAR_UNIT_LEN,
                    SPACEBAR_BASE_DIST,
                    SH_WALKER_2, PATH_ID_PYONTA, 10, 10);
    mb_map_sbtype1(&b, 0, 0, -1, 0);
    mb_map_sbtype7(&b, 0, -5, 0, 0);
    mb_map_sbtype7(&b, 0, 5, 0, 0);
    mb_label(&b, "level2_2.solidbar1");
    mb_map_sbtype1(&b, 2, 0, 1, 0);
    mb_map_sbtype1(&b, 0, 0, -1, 0);
    mb_maploop(&b, "level2_2.solidbar1", 3);

    mb_map_sbtype7(&b, 0, -6, 0, 0);
    mb_map_sbtype7(&b, 0, 6, 0, 0);
    mb_map_sbtype1(&b, 2, 0, 1, 0);
    mb_map_sbtype1(&b, 0, 0, -1, 0);
    mb_map_sbtype1(&b, 2, 0, 1, 0);
    mb_special(&b, 0, 0, SPACE_VIEWCY + SPACEBAR_UNIT_LEN, SPACEBAR_BASE_DIST,
               SH_S_WARK_0, IS_SPACEBARWALKER);
    mb_map_sbtype1(&b, 4 * 2, 0, 1, 0);

    mb_pathobj(&b, 0, 900, -60, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE4_1, 200, 10);
    mb_pathcspecial(&b, 0, 900, -60, 0, SH_ZACO_B, PATH_ID_CHASE4_2, 200, 10);
    mb_pathcspecial(&b, 1000, 900, -60, 0, SH_ZACO_B, PATH_ID_CHASE4_3, 200, 10);

    mb_pathcspecial(&b, 0,
                    2 * SPACEBAR_UNIT_LEN,
                    SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
                    SPACEBAR_BASE_DIST,
                    SH_WALKER_2, PATH_ID_PYONTA, 10, 10);
    mb_map_sbtype1(&b, 4 * 2, 0, 1, 0);
    mb_map_sbtype0(&b, 1 * 2, -3, 0, 0);
    mb_map_sbtype0(&b, 2 * 2, 2, 1, 0);
    mb_map_sbtype0(&b, 1 * 2, -4, 0, 0);
    mb_map_sbtype0(&b, 2 * 2, 4, 1, 0);
    mb_map_sbtype6(&b, 4 * 2, 0, 0, 0);

    mb_mapobj(&b, 0, 100, -100, 3000, SH_NULLSHAPE, IS_UP1MAN);
    mb_setalvarw(&b, AL_SWORD2, SH_ITEM_0_PROXY);
    mb_mapwait(&b, 2000);

    mb_map_sbtype7(&b, 4 * 2, 1, 1, 0);
    mb_map_sbtype5(&b, 1 * 2, -1, -1, 0);
    mb_map_sbtype5(&b, 6 * 2, 1, 1, 0);

    mb_map_sbtype0(&b, 0, -1, 0, 0);
    mb_map_sbtype0(&b, 0, 6, 0, 0);
    mb_map_sbtype0(&b, 3, 1, 0, 0);
    mb_map_sbtype0(&b, 0, -1, 0, 0);
    mb_map_sbtype0(&b, 0, -6, 0, 0);
    mb_map_sbtype0(&b, 3, 1, 0, 0);
    mb_map_sbtype0(&b, 0, -1, 0, 0);
    mb_map_sbtype0(&b, 0, 4, 0, 0);
    mb_map_sbtype0(&b, 3, 1, 0, 0);
    mb_map_sbtype0(&b, 0, -1, 0, 0);
    mb_map_sbtype0(&b, 0, -4, 0, 0);
    mb_label(&b, "level2_2.solidbar2");
    mb_map_sbtype0(&b, 3, 1, 0, 0);
    mb_map_sbtype0(&b, 0, -1, 0, 0);
    mb_maploop(&b, "level2_2.solidbar2", 2);

    mb_map_sbtype0(&b, 0, 1, 0, 0);
    mb_map_sbtype0(&b, 0, -2, 0, 0);
    mb_map_sbtype0(&b, 0, 2, 0, 0);
    mb_map_sbtype0(&b, 0, -3, 0, 0);
    mb_map_sbtype0(&b, 4, 3, 0, 0);

    mb_pathobj(&b, 0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    mb_map_sbtype15(&b, 0, 0, 0, 0, 0, 4);
    mb_map_sbtype10(&b, 5, -5, 0, 0);
    mb_map_sbtype15(&b, 5, 0, 0, 0, 0, 4);
    mb_map_sbtype15(&b, 5, 1, 0, 0, 0, 4);
    mb_map_sbtype15(&b, 5, 2, 0, 0, 0, 4);
    mb_map_sbtype15(&b, 5, 1, 0, 0, 0, 4);
    mb_map_sbtype15(&b, 5, 0, 0, 0, 0, 4);
    mb_mapobj(&b, 0, 50, SPACE_VIEWCY + 50, SPACEBAR_BASE_DIST, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1u);
    mb_map_sbtype15(&b, 5, 0, 0, 0, 0, 4);
    mb_map_sbtype15(&b, 5, -1, 0, 0, 0, 4);
    mb_map_sbtype15(&b, 0, -2, 0, 0, 0, 4);
    mb_map_sbtype10(&b, 5, 5, 0, 0);
    mb_map_sbtype15(&b, 5, -1, 0, 0, 0, 4);
    mb_map_sbtype15(&b, 5, 0, 0, 0, 0, 4);

    mb_mapobj(&b, 0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY, SPACEBAR_BASE_DIST,
              SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 2000, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY, SPACEBAR_BASE_DIST,
              SH_COLONY3L, IS_NOCOLL);

    mb_cspecial(&b, 0, -500, -300, 4000, SH_W_L, IS_WINGLAZERMAN);
    mb_special(&b, 0, 300, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    mb_cspecial(&b, 1000, -300, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    mb_cspecial(&b, 0, 0, SPACE_VIEWCY + 250, 800, SH_CAMELEON, IS_CAMELEON);
    mb_cspecial(&b, 2000, 0, SPACE_VIEWCY - 250, 800, SH_CAMELEON, IS_CAMELEON);

    mb_pathcspecial(&b, 0, 2500, -2000, 1800, SH_ZACO_8, PATH_ID_EGU6_IFAL, 10, 10);
    mb_pathcspecial(&b, 6000, -2500, -2000, 2400, SH_ZACO_8, PATH_ID_EGU6_IRAB, 10, 10);

    mb_cspecial(&b, 0, 250, SPACE_VIEWCY + 250, 800, SH_CAMELEON, IS_CAMELEON);
    mb_cspecial(&b, 1000, -250, SPACE_VIEWCY - 250, 800, SH_CAMELEON, IS_CAMELEON);
    mb_cspecial(&b, 0, -250, SPACE_VIEWCY + 250, 800, SH_CAMELEON, IS_CAMELEON);
    mb_special(&b, 4000, 250, SPACE_VIEWCY - 250, 800, SH_CAMELEON, IS_CAMELEON);

    mb_map_sbtype8(&b, 1 * 2, -2, 0, 0);
    mb_map_sbtype8(&b, 1 * 2, 1, 0, 0);
    mb_map_sbtypeA(&b, 1 * 2, -2, 0, 0);
    mb_map_sbtypeD(&b, 6 * 2, 2, 0, 0);

    mb_mapobj(&b, 0, 0, SPACE_VIEWCY, SPACEBAR_BASE_DIST, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1u);

    mb_special(&b, 0, -2 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
               SPACEBAR_BASE_DIST, SH_S_WARK_0, IS_SPACEBARWALKER);
    mb_cspecial(&b, 0, 2 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
                SPACEBAR_BASE_DIST, SH_BWARKER_3, IS_SPACEBARWALKER);
    mb_map_sbtypeE(&b, 2 * 2, 4, 1, 0);
    mb_map_sbtypeC(&b, 6 * 2, 3, 0, 0);
    mb_map_sbtype3(&b, 3 * 2, 0, 0, 0);
    mb_map_sbtype6(&b, 3 * 2, 0, 0, 0);

    mb_mapobj(&b, 0, SPACEBAR_UNIT_LEN, SPACE_VIEWCY, SPACEBAR_BASE_DIST + (2 * SPACEBAR_UNIT_LEN),
              SH_ITEM_6, IS_ITEM6);

    mb_special(&b, 0, 350, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    mb_cspecial(&b, 3000, -350, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    mb_setalvarb(&b, AL_SBYTE1, 1u);

    mb_map_sbtype11(&b, 4 * 2, 0, 0, 0);
    mb_map_sbtypeC(&b, 0, -1, 0, 0);
    mb_map_sbtypeB(&b, 1 * 2, -1, 0, 0);
    mb_map_sbtypeB(&b, 1 * 2, 1, -1, 0);
    mb_map_sbtype8(&b, 1 * 2, -1, 0, 0);

    mb_cspecial(&b, 0, -2 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
                SPACEBAR_BASE_DIST + (4 * SPACEBAR_UNIT_LEN), SH_BWARKER_3, IS_SPACEBARWALKER);
    mb_special(&b, 0, 0, SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
               SPACEBAR_BASE_DIST + (6 * SPACEBAR_UNIT_LEN), SH_S_WARK_0, IS_SPACEBARWALKER);
    mb_cspecial(&b, 0, 3 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
                SPACEBAR_BASE_DIST + (7 * SPACEBAR_UNIT_LEN), SH_BWARKER_3, IS_SPACEBARWALKER);
    mb_map_sbtypeF(&b, 15 * 2, 0, 1, 0);

    mb_mapobj(&b, 0, 0, -60, 2800, SH_GATE_0, IS_GATE);

    mb_mapwait(&b, 1000);
    mb_mapwait(&b, 3000);

    mb_map_sbtype10(&b, 8 * 2, 0, 0, 0);

    mb_mapobj(&b, 0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY, SPACEBAR_BASE_DIST,
              SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 2000, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY, SPACEBAR_BASE_DIST,
              SH_COLONY3L, IS_NOCOLL);

    mb_pathobj(&b, 0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 200, 10);
    mb_pathcspecial(&b, 3000, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);

    mb_pathcspecial(&b, 200, 0, -450, 4000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    mb_pathcspecial(&b, 200, 0, -200, 4000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    mb_pathcspecial(&b, 200, 0, 50, 4000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    mb_pathspecial(&b, 15000, 0, 300, 4000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);

    mb_map_sbtype7(&b, 3 * 2, -3, 1, 0);
    mb_map_sbtype7(&b, 3 * 2, 3, -1, 0);
    mb_map_sbtype14(&b, 5 * 2, 0, 0, 0);
    mb_map_sbtype10(&b, 2 * 2, 4, 0, 0);
    mb_map_sbtype10(&b, 4 * 2, -4, 0, 0);
    mb_map_sbtype6(&b, 4 * 2, 0, 0, 0);
    mb_map_sbtype7(&b, 4 * 2, -3, 0, 0);
    mb_map_sbtype7(&b, 4 * 2, 3, 0, 0);
    mb_map_sbtype7(&b, 4 * 2, 0, 0, 0);
    mb_map_sbtype10(&b, 4 * 2, 0, 0, 0);
    mb_map_sbtype5(&b, 1 * 2, -2, 0, 0);
    mb_map_sbtype1(&b, 3 * 2, 0, 0, 0);

    {
        const int16 speed = 30;

        mb_pathobj(&b, 0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
        mb_map_sbtype17(&b, 6, 0, 10, 0, -speed, 0);
        mb_map_sbtype16(&b, 6, -10, -1, 0, speed, 0);
        mb_map_sbtype17(&b, 6, -1, -10, 0, speed, 0);
        mb_map_sbtype16(&b, 6, 10, 1, 0, -speed, 0);

        mb_map_sbtype17(&b, 0, 5, -10, 0, speed, 0);
        mb_map_sbtype17(&b, 0, -5, -10, 0, speed, 0);
        mb_map_sbtype17(&b, 3, 0, 10, 0, -speed, 0);
        mb_map_sbtype16(&b, 0, -20, -2, 0, speed, 0);
        mb_map_sbtype16(&b, 0, -10, -1, 0, speed, 0);
        mb_map_sbtype16(&b, 3, 0, 0, 0, speed, 0);
        mb_map_sbtype17(&b, 0, 4, 10, 0, -speed, 0);
        mb_map_sbtype17(&b, 0, -6, 10, 0, -speed, 0);
        mb_map_sbtype17(&b, 3, -1, -10, 0, speed, 0);
        mb_map_sbtype16(&b, 0, 20, 0, 0, -speed, 0);
        mb_map_sbtype16(&b, 0, 10, 1, 0, -speed, 0);
        mb_map_sbtype16(&b, 3, 0, 2, 0, -speed, 0);

        mb_map_sbtype17(&b, 0, 6, -10, 0, speed, 0);
        mb_map_sbtype17(&b, 0, -4, -10, 0, speed, 0);
        mb_map_sbtype17(&b, 3, 1, 10, 0, -speed, 0);
        mb_map_sbtype16(&b, 0, -20, -1, 0, speed, 0);
        mb_map_sbtype16(&b, 0, -10, 0, 0, speed, 0);
        mb_map_sbtype16(&b, 3, 0, 0, 1, speed, 0);
        mb_map_sbtype17(&b, 0, 3, 10, 0, -speed, 0);
        mb_map_sbtype17(&b, 0, -7, 10, 0, -speed, 0);
        mb_map_sbtype17(&b, 3, -2, -10, 0, speed, 0);
        mb_map_sbtype16(&b, 0, 20, 0, 0, -speed, 0);
        mb_map_sbtype16(&b, 0, 10, -1, 0, -speed, 0);
        mb_map_sbtype16(&b, 3, 0, -2, 0, -speed, 0);

        mb_map_sbtype17(&b, 0, 7, 10, 0, speed, 0);
        mb_map_sbtype17(&b, 0, -3, 10, 0, speed, 0);
        mb_map_sbtype17(&b, 3, 2, 10, 0, -speed, 0);
        mb_map_sbtype16(&b, 0, -20, -1, 0, speed, 0);
        mb_map_sbtype16(&b, 0, -10, 0, 0, speed, 0);
        mb_map_sbtype16(&b, 3, 0, 1, 0, speed, 0);
        mb_map_sbtype17(&b, 0, 5, 10, 0, -speed, 0);
        mb_map_sbtype17(&b, 0, -5, 10, 0, -speed, 0);
        mb_map_sbtype17(&b, 3, 0, -10, 0, speed, 0);
        mb_map_sbtype16(&b, 0, 20, 2, 0, -speed, 0);
        mb_map_sbtype16(&b, 0, 10, 1, 0, -speed, 0);
        mb_map_sbtype16(&b, 3, 0, 0, 0, -speed, 0);

        mb_map_sbtype17(&b, 1, 0, 10, 0, -speed, 0);
        mb_map_sbtype16(&b, 1, -10, -1, 0, speed, 0);
        mb_map_sbtype17(&b, 1, -1, -10, 0, speed, 0);
        mb_map_sbtype16(&b, 1, 10, 1, 0, -speed, 0);
        mb_map_sbtype17(&b, 1, 1, 10, 0, -speed, 0);
        mb_map_sbtype16(&b, 1, -10, 0, 0, speed, 0);
        mb_map_sbtype17(&b, 1, -2, -10, 0, speed, 0);
        mb_map_sbtype16(&b, 1, 10, -1, 0, -speed, 0);
        mb_map_sbtype17(&b, 1, 2, 10, 0, -speed, 0);
        mb_map_sbtype16(&b, 1, -10, 0, 0, speed, 0);
        mb_map_sbtype17(&b, 1, 0, -10, 0, speed, 0);
        mb_map_sbtype16(&b, 3000, 10, 1, 0, -speed, 0);
    }

    mb_pathobj(&b, 0, 1200, 200, 600, SH_FRIENDSHIP_4, PATH_ID_CHASE1_1, 200, 10);
    mb_pathcspecial(&b, 1000, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);
    mb_mapobj(&b, 4000, 0, 0, 2000, SH_SPACEPILON, STRAT_ADDR_SPACEPILON);

    mb_pathspecial(&b, 200, 0, -200, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    mb_pathspecial(&b, 200, 0, 200, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    mb_pathcspecial(&b, 200, 200, 0, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    mb_pathobj(&b, 0, -250, -350, 0, SH_MY_BIRD, PATH_ID_MY_BIRD, 10, 10);
    mb_pathcspecial(&b, 12000, -200, 0, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);

    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_BOSS1);
    mb_mapobj(&b, 0, 0, SPACE_VIEWCY + 1000, 1500, SH_BOSS_1_2, IS_BOSS1);

    mb_mapwait(&b, 100);
    mb_mapcode65816_inline(&b, &s_level2_2_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level2_2.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level2_2.bosswait.cont");
    mb_mapgoto(&b, "level2_2.bosswait.loop");
    mb_label(&b, "level2_2.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level2_2_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level2_2_mapwaitboss_cleanup_script_ptr);
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);
    mb_mapwait(&b, 2000);
    mb_maprts(&b);

    append_cl_earth_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level2_2 = s_empty_level;
        return;
    }

    s_level2_2.data = s_level2_2_data;
    s_level2_2.length = b.length;
}

static void build_level3_1_wrapper_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level3_1_data;
    b.capacity = sizeof(s_level3_1_data);
    s_level3_1_keep_player_strat_script_ptr = 0u;
    s_level3_1_mapwaitboss_trigse_script_ptr = 0u;
    s_level3_1_mapwaitboss_cantdie_script_ptr = 0u;
    s_level3_1_mapwaitboss_cleanup_script_ptr = 0u;
    s_level3_1_skillfly_bonus0_guard_script_ptr = 0u;
    s_level3_1_skillfly_bonus0_skip_ptr = 0u;

    // LEVEL3_1.ASM through the first handoff into MAP3_1B.
    mb_mapwait(&b, 100);
    mb_mapjsr(&b, "map1_1a");
    mb_qfadedown(&b);
    mb_waitfade(&b);
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);
    mb_mapwait(&b, 1);
    mb_setbg(&b, BG_3_1C);
    mb_initbg(&b);
    mb_mapwait(&b, MEDPSPEED * 2);
    mb_qfadeup(&b);
    mb_mapcode65816_inline(&b, &s_level3_1_keep_player_strat_script_ptr);
    mb_mapif_builtin(&b, MAP_CB_IS_PLAYER_DEAD, "level3_1.after_exitbase_setup");
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_EXITBASE_L);
    mb_label(&b, "level3_1.after_exitbase_setup");

    mb_mapobj(&b, 0, 0, 0, 0, SH_MYBASE_1, IS_NOCOLL);
    mb_mapobj(&b, 0, 0, 0, 0, SH_MYBASE_0, IS_NOCOLL);

    mb_mapobj(&b, 0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200,
              SH_MYSHIP_4, IS_FRIENDEXITBASE);
    mb_setalvarb(&b, AL_SBYTE1, 17);
    mb_mapobj(&b, 0, -27 << MYBASE_SCALE, -39 << MYBASE_SCALE, -200,
              SH_MYSHIP_4, IS_FRIENDEXITBASE);
    mb_setalvarb(&b, AL_SBYTE1, (uint8)(17 + (1000 / PEXITBASE_SPEED)));

    mb_pathobj(&b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_MATEMSG, 10, 10);
    mb_pathobj(&b, 0, 100, -90, 1400, SH_FRIENDSHIP_4, PATH_ID_FALCO_LV1, 10, 10);
    mb_pathobj(&b, 0, -80, -140, 1200, SH_FRIENDSHIP_4, PATH_ID_FROG_LV1, 10, 10);

    mb_mapobj(&b, 0, -600, 0, 2000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 600, 0, 2000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -700, 0, 3500, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 700, 0, 3500, SH_BU_1, IS_HARD180YR);

    mb_mapobj(&b, 0, -900, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 2000, 900, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -1100, 0, 4800, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 1100, 0, 4800, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -500, 0, 5000, SH_HOUDAI_0, IS_HOUDAINS);
    mb_mapobj(&b, 2000, 500, 0, 5000, SH_HOUDAI_0, IS_HOUDAINS);

    mb_cspecial(&b, 0, -500, -300, 0, SH_ZACO_5, IS_ZACO1L);
    mb_mapobj(&b, 0, -1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 2000, 1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    mb_cspecial(&b, 0, 500, -300, 0, SH_ZACO_5, IS_ZACO1R);
    mb_mapobj(&b, 0, -1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -600, 0, 5000, SH_HOUDAI_0, IS_HOUDAINS);
    mb_mapobj(&b, 0, 600, 0, 5000, SH_HOUDAI_0, IS_HOUDAI);

    mb_mapcodejsl_builtin(&b, MAP_CB_SETRESTART_L);
    mb_mapif_builtin(&b, MAP_CB_IS_PLAYER_DEAD, "level3_1.after_onplanet_setup");
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_ONPLANET_L);
    mb_label(&b, "level3_1.after_onplanet_setup");
    mb_mapjsr(&b, "level3_1.map3_1b");
    mb_emit8(&b, MAP_OP_END);

    mb_label(&b, "level3_1.map3_1b");
    // 3-1.ASM literal opening through the first 3-1-2 friend block.
    mb_mapcodejsl_builtin(&b, MAP_CB_SETRESTART_L);
    mb_pathobj(&b, 1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    mb_cspecial(&b, 0, -200, -700, -500, SH_ZACO_5, IS_ZACO1L);
    mb_special(&b, 0, 200, -900, -500, SH_ZACO_A, IS_ZACO1R);
    mb_label(&b, "level3_1.map3_1b.houdai");
    mb_mapobj(&b, 0, -700, 0, 5000, SH_HOUDAI_0, IS_HOUDAINS);
    mb_mapobj(&b, 0, 700, 0, 5000, SH_HOUDAI_0, IS_HOUDAINS);
    mb_mapobj(&b, 0, -1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 1500, 1200, 0, 4800, SH_BU_1, IS_HARD180YR);
    mb_maploop(&b, "level3_1.map3_1b.houdai", 2);

    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, -60, 4000, 100);
    mb_mapobj(&b, 0, 0, 0, 4000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 0, -1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 1500, 1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    mb_mapobj(&b, 0, 400, -110, 4000, SH_RADER_0, IS_RADER0);
    mb_mapobj(&b, 0, 400, 0, 4000, SH_RADER_1, IS_RADER1);
    mb_mapobj(&b, 0, -400, -110, 4000, SH_RADER_0, IS_RADER0);
    mb_mapobj(&b, 1500, -400, 0, 4000, SH_RADER_1, IS_RADER1);

    mb_mapobj(&b, 0, 0, -110, 4000, SH_RADER_0, IS_RADER0);
    mb_mapobj(&b, 2000, 0, 0, 4000, SH_RADER_1, IS_RADER1);
    mb_mapobj(&b, 0, -250, 0, 3200, SH_HOUDAI_0, IS_HOUDAI);
    mb_mapobj(&b, 1200, 250, 0, 3200, SH_HOUDAI_0, IS_HOUDAI);
    mb_cspecial(&b, 200, -600, -700, -200, SH_ZACO_5, IS_ZACO1L);
    mb_special(&b, 200, 400, -700, -300, SH_ZACO_A, IS_ZACO1R);
    mb_cspecial(&b, 1000, 600, -900, -500, SH_ZACO_5, IS_ZACO1R);
    mb_skillfly_set(&b, -300, -60, 4000, 100);
    mb_mapobj(&b, 0, -300, 0, 4000, SH_ARCH_0, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, DEG45);

    mb_label(&b, "level3_1.map3_1b.bu_0");
    mb_mapobj(&b, 0, -1200, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 1000, 1200, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_maploop(&b, "level3_1.map3_1b.bu_0", 3);
    mb_skillfly_set(&b, 300, -60, 4000, 100);
    mb_mapobj(&b, 0, 300, 0, 4000, SH_ARCH_0, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, (uint8)-DEG45);

    mb_pathcspecial(&b, 500, 0, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    mb_mapobj(&b, 0, 900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 3000, -900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_pathobj(&b, 0, -350, 0, 5000, SH_ROBOT_0, PATH_ID_ROBOT, 6, 4);
    mb_pathobj(&b, 0, 350, 0, 5000, SH_ROBOT_0, PATH_ID_ROBOT, 6, 4);
    mb_mapobj(&b, 0, 900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 1000, -900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_skillfly_set(&b, 0, -60, 4000, 100);
    mb_mapobj(&b, 3000, 0, 0, 4000, SH_ARCH_0, IS_HARD);

    mb_pathobj(&b, 0, 0, -400, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    mb_pathcspecial(&b, 1000, 0, -400, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);

    mb_mapobj(&b, 0, -400, 0, 5000, SH_BASE_1, IS_BASE_1);
    mb_mapobj(&b, 0, 400, -50, 5200, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_mapcode65816_inline(&b, &s_level3_1_skillfly_bonus0_guard_script_ptr);
    mb_mapobj(&b, 0, -400, -50, 5200, SH_ITEM_7, IS_ITEM7);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level3_1.map3_1b.skillfly_bonus_0_skip");
    mb_mapobj(&b, 2000, 400, 0, 5000, SH_BASE_1, IS_BASE_1);
    mb_mapobj(&b, 0, 900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 1500, -900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 0, 900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 1500, -900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 0, 900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 1500, -900, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_pathcspecial(&b, 1500, 0, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    mb_mapobj(&b, 0, 300, 0, 4000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 1500, -300, 0, 4000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 0, 1000, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 1000, -1000, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_special(&b, 0, -200, -1300, 2300, SH_ZACO_A, IS_ZACOS);
    mb_cspecial(&b, 1500, 200, -1300, 2300, SH_ZACO_6, IS_ZACOS);
    mb_mapobj(&b, 0, 300, 0, 4000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 1500, -300, 0, 4000, SH_BU_8, IS_HARD180YR);
    mb_pathcspecial(&b, 3000, 0, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    mb_mapobj(&b, 0, 1000, 0, 4000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 1200, -1000, 0, 4000, SH_TOWER_2, IS_TOWER0);

    mb_cspecial(&b, 300, -600, -300, 5000, SH_KAMIKAZE, IS_ZACO4);
    mb_mapobj(&b, 0, 300, 0, 4000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 0, -300, 0, 4000, SH_BU_8, IS_HARD180YR);
    mb_cspecial(&b, 1000, 700, -300, 5000, SH_KAMIKAZE, IS_ZACO4);
    mb_mapobj(&b, 500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, -600, 0, 4000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0, 600, 0, 4000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, -600, 0, 4000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0, 600, 0, 4000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, -150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 500, 150, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, -300, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 3000, 300, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_cspecial(&b, 0, 200, -1300, 2000, SH_ZACO_6, IS_ZACOS);
    mb_cspecial(&b, 0, -100, -1500, 2300, SH_ZACO_6, IS_ZACOS);
    mb_cspecial(&b, 0, 500, -1500, 2300, SH_ZACO_6, IS_ZACOS);
    mb_special(&b, 0, 50, -1700, 2500, SH_ZACO_A, IS_ZACOS);
    mb_special(&b, 1000, 350, -1700, 2500, SH_ZACO_A, IS_ZACOS);
    mb_label(&b, "level3_1.map3_1b.tow");
    mb_pathobj(&b, 0, 800, 0, 4000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    mb_pathobj(&b, 3000, -800, 0, 4000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);
    mb_maploop(&b, "level3_1.map3_1b.tow", 2);

    mb_mapobj(&b, 0, 380, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -380, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 180, 0, 5000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 2000, -180, 0, 5000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 0, 0, 4000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 480, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 1000, -480, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0, 230, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, 230, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_cspecial(&b, 1000, -1500, -600, 4400, SH_ZACO_5, IS_ZACO0);
    mb_mapobj(&b, 0, 400, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 1000, -400, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, 280, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 1000, -280, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_pathobj(&b, 0, -130, -150, -100, SH_FRIENDSHIP_4, PATH_ID_FALCON3_1, 10, 10);
    mb_mapobj(&b, 0, 0, 0, 4000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 400, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 1000, -400, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, 280, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 1000, -280, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 1000, 0, 0, 4000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 340, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 2000, -340, 0, 4000, SH_BU_6, IS_HARD180YR);
    mb_label(&b, "level3_1.map3_1b.torii");
    mb_mapobj(&b, 300, 0, 0, 3000, SH_ARCH_0, IS_HARD);
    mb_maploop(&b, "level3_1.map3_1b.torii", 5);
    mb_mapobj(&b, 0, 0, -30, 2500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_mapobj(&b, 400, 0, 0, 3000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 0, 120, 0, 3000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 300, -120, 0, 3000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 0, 170, -30, 2800, SH_ITEM_5, IS_ITEM5);
    mb_mapobj(&b, 0, 150, 0, 3000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 300, -150, 0, 3000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 0, 170, 0, 3000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 300, -170, 0, 3000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 0, 200, 0, 3000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 100, -200, 0, 3000, SH_ARCH_0, IS_HARD);
    mb_mapobj(&b, 0, -200, -100, 3000, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 1000, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    mb_pathobj(&b, 0, -300, 0, 5000, SH_NULLSHAPE, PATH_ID_ROBOTWITHLOG, 6, 4);
    mb_setalxvarw(&b, ALX_PWORD1, SH_RAW_BOSS_7_0);
    mb_setalvarb(&b, AL_ROTY, (uint8)-DEG22);
    mb_pathobj(&b, 3500, -200, 0, 5000, SH_TOW_0, PATH_ID_TOW_0, 10, 10);

    mb_pathobj(&b, 0, -1000, 0, 5000, SH_NULLSHAPE, PATH_ID_ROBOTWITHLOG, 6, 4);
    mb_setalxvarw(&b, ALX_PWORD1, SH_RAW_BOSS_7_3);
    mb_setalvarb(&b, AL_ROTY, (uint8)(-(DEG45 + DEG22)));
    mb_mapwait(&b, 1500);

    mb_pathobj(&b, 0, -1400, 0, 5000, SH_NULLSHAPE, PATH_ID_ROBOTSWITHLOG, 6, 4);
    mb_setalxvarw(&b, ALX_PWORD1, SH_RAW_BOSS_7_3);
    mb_setalvarb(&b, AL_ROTY, (uint8)(-(DEG45 + DEG22)));
    mb_mapwait(&b, 3000);

    mb_mapobj(&b, 1500, 0, 0, 4000, SH_BIG_GATE, IS_HARD);
    mb_mapobj(&b, 0, 360, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, -360, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_label(&b, "level3_1.map3_1b.bupillar");
    mb_mapobj(&b, 0, 180, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -180, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 1000, 180, 0, 4400, SH_PILLAR3, IS_PILLAR3);
    mb_maploop(&b, "level3_1.map3_1b.bupillar", 2);
    mb_mapobj(&b, 0, 0, -50, 4000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 180, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -180, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 1000, -180, 0, 4400, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 0, 180, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -180, 0, 4000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 180, 0, 4400, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1000, -180, 0, 4400, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(&b, 1800, 0, 0, 5000, SH_BIG_GATE, IS_HARD);
    mb_pathobj(&b, 0, -1400, 0, 5000, SH_NULLSHAPE, PATH_ID_ROBOTWITHLOG, 6, 4);
    mb_setalxvarw(&b, ALX_PWORD1, SH_RAW_BOSS_7_1);
    mb_setalvarb(&b, AL_ROTY, (uint8)(-(DEG45 + DEG22)));

    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_BOSS1);
    mb_mapobj(&b, 0, 3000, 0, 375 << BOSSA_SCALE, SH_BOSS_A_2, IS_BOSSA);
    mb_mapcode65816_inline(&b, &s_level3_1_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level3_1.map3_1b.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level3_1.map3_1b.bosswait.cont");
    mb_mapgoto(&b, "level3_1.map3_1b.bosswait.loop");
    mb_label(&b, "level3_1.map3_1b.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level3_1_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level3_1_mapwaitboss_cleanup_script_ptr);
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);
    mb_maprts(&b);

    append_cl_chase_submap(&b);
    append_map1_1a_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level3_1 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level3_1.map3_1b.skillfly_bonus_0_skip",
                         &s_level3_1_skillfly_bonus0_skip_ptr)) {
        s_level3_1 = s_empty_level;
        return;
    }

    s_level3_1.data = s_level3_1_data;
    s_level3_1.length = b.length;
}

static void register_level1_1_inline_callbacks(void) {
    World_RegisterNativeCallback(MAP_CB_CL_GROUND_PRINTLEVELFIN,
                                 level1_1_cl_ground_printlevelfin);
    World_RegisterNativeCallback(MAP_CB_CL_GROUND_WIPEOUT,
                                 level1_1_cl_ground_wipeout);
    World_RegisterNativeCallback(MAP_CB_CL_DIVE_CLEAR_ENGINESND,
                                 cl_dive_clear_enginesnd);
    if (s_level1_1_keep_player_strat_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_1_keep_player_strat_script_ptr,
                                    level_scramble_keep_player_strat);
    }
    if (s_level1_1_skillfly_bonus_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_1_skillfly_bonus_guard_script_ptr,
                                    level1_1_skillfly_bonus_guard);
    }
    if (s_level1_1_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_1_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level1_1_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_1_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level1_1_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_1_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

static void register_level1_2_inline_callbacks(void) {
    World_RegisterNativeCallback(MAP_CB_CL_WARP_PRINTLEVELFIN,
                                 level1_1_cl_ground_printlevelfin);
    if (s_level1_2_skillfly_bonus_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_2_skillfly_bonus_guard_script_ptr,
                                    level1_2_skillfly_bonus_guard);
    }
    if (s_level1_2_blackhole_bonus_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_2_blackhole_bonus_guard_script_ptr,
                                    level1_2_blackhole_bonus_guard);
    }
    if (s_level1_2_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_2_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level1_2_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_2_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level1_2_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_2_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

static void register_level2_1_inline_callbacks(void) {
    if (s_level2_1_keep_player_strat_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_1_keep_player_strat_script_ptr,
                                    level_scramble_keep_player_strat);
    }
    if (s_level2_1_skillfly_bonus0_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_1_skillfly_bonus0_guard_script_ptr,
                                    level2_1_skillfly_bonus0_guard);
    }
    if (s_level2_1_skillfly_bonus1_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_1_skillfly_bonus1_guard_script_ptr,
                                    level2_1_skillfly_bonus1_guard);
    }
    if (s_level2_1_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_1_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level2_1_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_1_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level2_1_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_1_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

static void register_level2_2_inline_callbacks(void) {
    if (s_level2_2_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_2_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level2_2_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_2_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level2_2_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_2_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

static void register_level3_1_inline_callbacks(void) {
    if (s_level3_1_keep_player_strat_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_1_keep_player_strat_script_ptr,
                                    level_scramble_keep_player_strat);
    }
    if (s_level3_1_skillfly_bonus0_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_1_skillfly_bonus0_guard_script_ptr,
                                    level3_1_skillfly_bonus0_guard);
    }
    if (s_level3_1_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_1_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level3_1_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_1_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level3_1_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_1_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

static void register_level3_2_inline_callbacks(void) {
    if (s_level3_2_skillfly_bonus_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_2_skillfly_bonus_guard_script_ptr,
                                    level3_2_skillfly_bonus_guard);
    }
    if (s_level3_2_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_2_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level3_2_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_2_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level3_2_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_2_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

static void build_level3_2_wrapper_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level3_2_data;
    b.capacity = sizeof(s_level3_2_data);
    s_level3_2_skillfly_bonus_guard_script_ptr = 0u;
    s_level3_2_skillfly_bonus_skip_ptr = 0u;
    s_level3_2_mapwaitboss_trigse_script_ptr = 0u;
    s_level3_2_mapwaitboss_cantdie_script_ptr = 0u;
    s_level3_2_mapwaitboss_cleanup_script_ptr = 0u;

    // LEVEL3_2.ASM wrapper around MAP3_2.ASM.
    mb_mapjsr(&b, "level3_2.map3_2");
    mb_mapjsr(&b, "cl_chase");
    mb_emit8(&b, MAP_OP_END);

    // MAP3_2.ASM:5-33 – Asteroid Belt 3 opening (M formation through
    // the first asteroid/itachi block, stopping before mapmother).
    mb_label(&b, "level3_2.map3_2");
    mb_mapwait(&b, 3300);

    // M formation
    mb_szaco2_mapobj(&b, 0, 2000, 0, 0, 100);
    mb_szaco2_mapobj(&b, -500, 2000, -300, 100, 0);
    mb_szaco2_mapobj(&b, 500, 2000, 300, 100, 100);
    mb_pathobj(&b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_ASTEMSG, 10, 10);
    mb_szaco2_mapobj(&b, -1000, 2000, -500, -100, 0);
    mb_szaco2_mapobj(&b, 1000, 2000, 500, -100, 100);
    mb_mapwait(&b, 2000);
    mb_pathcspecial(&b, 2000, -200, 100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_pathcspecial(&b, 4000, 200, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);

    // friend
    mb_pathcspecial(&b, 0, 0, -90, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_pathobj(&b, 0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    mb_pathcspecial(&b, 1000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);
    mb_cspecial(&b, 1000, 0, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_mapnobj(&b, 500, 400, SPACE_VIEWCY - 100, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_pathcspecial(&b, 500, 200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_mapnobj(&b, 1000, -400, SPACE_VIEWCY + 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_cspecial(&b, 1000, -200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 1000, -400, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_mapnobj(&b, 1000, 200, SPACE_VIEWCY - 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_cspecial(&b, 1000, 0, 300, 4000, SH_ASTEROID2, IS_BREAK_METEORT);
    mb_pathcspecial(&b, 500, 250, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_pathcspecial(&b, 500, -100, -200, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_mapnobj(&b, 1000, -300, SPACE_VIEWCY + 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_pathcspecial(&b, 500, 200, 100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);

    // MAP3_2.ASM:34-45 – mother ship pattern with break meteors and big meteors.
    mb_mapmother(&b, 1300, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_cspecial(&b, 1300, -350, -100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 1300, 0, 0, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_mapobj(&b, 1300, -700, -300, 7000, SH_BIG_METEOR_PROXY, IS_BIG_METEOR);
    mb_cspecial(&b, 1300, 450, 50, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 1300, 50, -150, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 1300, -350, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_pathobj(&b, 1300, 970, -100, 7000, SH_BIG_METEOR_PROXY, PATH_ID_BIRD_METEOR, 10, 10);
    mb_cspecial(&b, 1300, 550, 0, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 1300, -250, -120, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 1300, 450, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_mapremove(&b, SH_MOTHER1);

    // MAP3_2.ASM:47-52 – friend chase pair with itachi formations.
    mb_pathcspecial(&b, 2000, 50, -70, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
    mb_pathcspecial(&b, 3000, -50, -140, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
    mb_pathobj(&b, 0, 1200, 200, 600, SH_FRIENDSHIP_4, PATH_ID_CHASE1_1, 200, 10);
    mb_pathcspecial(&b, 1500, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);
    mb_pathcspecial(&b, 500, -100, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);

    // MAP3_2.ASM:55-60 – asteroid/itachi block then mother_2.
    mb_mapnobj(&b, 1000, -300, SPACE_VIEWCY + 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    mb_pathcspecial(&b, 1000, -200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_cspecial(&b, 1000, -400, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_mapmother(&b, 2000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_pathcspecial(&b, 1000, 200, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_mapremove(&b, SH_MOTHER1);

    // MAP3_2.ASM:61-70 – skillfly block with bonus item.
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, -50, 4500, 120);
    mb_cspecial(&b, 0, 180, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 0, -180, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 400, 0, -200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_mapmother(&b, 4000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapremove(&b, SH_MOTHER1);
    mb_mapcode65816_inline(&b, &s_level3_2_skillfly_bonus_guard_script_ptr);
    mb_mapobj(&b, 0, 0, SPACE_VIEWCY, 1500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level3_2.map3_2.skillfly_bonus_0_skip");

    // MAP3_2.ASM:71-73 – winglazerman and cameleons.
    mb_cspecial(&b, 2000, -400, SPACE_VIEWCY, 3000, SH_W_L, IS_WINGLAZERMAN);
    mb_special(&b, 0, -200, SPACE_VIEWCY + 100, 800, SH_CAMELEON, IS_CAMELEON);
    mb_cspecial(&b, 1500, 200, SPACE_VIEWCY - 100, 800, SH_CAMELEON, IS_CAMELEON);

    // MAP3_2.ASM:74-79 – meteo & launcher mother with itachi formations.
    mb_mapmother(&b, 4000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_pathcspecial(&b, 2000, 0, -130, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_pathcspecial(&b, 2000, -200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_pathcspecial(&b, 2000, 200, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
    mb_mapremove(&b, SH_MOTHER1);

    // MAP3_2.ASM:80-86 – gate with big meteors and break meteor.
    mb_mapobj(&b, 500, 400, -200, 7000, SH_BIG_METEOR_PROXY, IS_BIG_METEOR);
    mb_mapobj(&b, 3000, -400, 200, 7000, SH_BIG_METEOR_PROXY, IS_BIG_METEOR);

    mb_mapobj(&b, 0, 0, 0, 4000, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    mb_cspecial(&b, 3500, -400, 200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);

    // MAP3_2.ASM:88-100 – windmill (round_1) with rotation/velocity sequence.
    mb_special(&b, 0, -1500, SPACE_VIEWCY, 4000, SH_ROUND_0, IS_WINDMILL);
    mb_setalvarb(&b, AL_ROTY, 160);
    mb_setalvarb(&b, AL_VEL, 120);
    mb_mapwait(&b, 1200);
    mb_setalvarb(&b, AL_ROTY, 140);
    mb_setalvarb(&b, AL_VEL, 100);
    mb_mapwait(&b, 1200);
    mb_setalvarb(&b, AL_VEL, 0);
    mb_setalvarb(&b, AL_ROTY, 127);
    mb_mapwait(&b, 1500);
    mb_setalvarb(&b, AL_VEL, 120);
    mb_setalvarw(&b, AL_SWORD1, (uint16)-2);

    // MAP3_2.ASM:101-111 – mini_worm (head + 5 body segments).
    mb_special(&b, 0, -200, SPACE_VIEWCY - 100, 2500, SH_D_HEAD_0, IS_WORMHEAD);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapwait(&b, 150);
    for (uint8 i = 0; i < 5u; i++) {
        mb_cspecial(&b, 0, -200, SPACE_VIEWCY - 100, 2500, SH_D_BODY_0, IS_WORM);
        mb_setalvarptrw(&b, AL_SWORD1, WM_MAPVAR1);
        mb_setvarobj(&b, WM_MAPVAR1);
        mb_mapwait(&b, 150);
    }

    // MAP3_2.ASM:113-114 – spacepilon and itachi_b formation.
    mb_mapobj(&b, 2000, 0, 100, 2000, SH_SPACEPILON, STRAT_ADDR_SPACEPILON);
    mb_pathcspecial(&b, 2000, -200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);

    // MAP3_2.ASM:115-125 – mini_worm #2 (head + 5 body segments).
    mb_special(&b, 0, 200, SPACE_VIEWCY + 100, 2300, SH_D_HEAD_0, IS_WORMHEAD);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapwait(&b, 150);
    for (uint8 i = 0; i < 5u; i++) {
        mb_cspecial(&b, 0, 200, SPACE_VIEWCY + 100, 2300, SH_D_BODY_0, IS_WORM);
        mb_setalvarptrw(&b, AL_SWORD1, WM_MAPVAR1);
        mb_setvarobj(&b, WM_MAPVAR1);
        mb_mapwait(&b, 150);
    }

    // MAP3_2.ASM:126-127 – set bar shape solid, itachi_a formation.
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_pathcspecial(&b, 2000, 200, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);

    // MAP3_2.ASM:129-131 – friend chase3 pair.
    mb_pathobj(&b, 0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 200, 10);
    mb_pathcspecial(&b, 1000, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);

    // MAP3_2.ASM:133-134 – colony pair.
    mb_mapobj(&b, 0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY, 5000,
              SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 1600, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY, 5000,
              SH_COLONY3L, IS_NOCOLL);

    // MAP3_2.ASM:136-137 – colony pair.
    mb_mapobj(&b, 0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY, 5000,
              SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 2000, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY, 5000,
              SH_COLONY3L, IS_NOCOLL);

    // MAP3_2.ASM:139-141 – up1man + itachi_b formation.
    mb_mapobj(&b, 0, 0, 0, 5000, SH_NULLSHAPE, IS_UP1MAN);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_pathcspecial(&b, 2000, 200, -200, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);

    // MAP3_2.ASM:143-144 – itachi_a + spacepilon.
    mb_pathcspecial(&b, 2000, 0, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_mapobj(&b, 3000, 0, 200, 2000, SH_SPACEPILON, STRAT_ADDR_SPACEPILON);

    // MAP3_2.ASM:146-148 – meteo_0 triple.
    mb_mapobj(&b, 2250, 0, 0, 4000, SH_METEO_0, IS_METEO0);
    mb_mapobj(&b, 2250, 200, -100, 4000, SH_METEO_0, IS_METEO0);
    mb_mapobj(&b, 2250, -200, -160, 4000, SH_METEO_0, IS_METEO0);

    // MAP3_2.ASM:150 – screw path.
    mb_pathcspecial(&b, 400, 200, -100, 4000, SH_B_HOU_0, PATH_ID_SCREW, 10, 10);

    // MAP3_2.ASM:152 – r_hou_0 special.
    mb_special(&b, 0, -200, 0, 4000, SH_R_HOU_0, IS_SHOU0A);

    // MAP3_2.ASM:154-155 – item_5 with sbyte1.
    mb_mapobj(&b, 0, 100, SPACE_VIEWCY - 100, 4500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);

    // MAP3_2.ASM:158-160 – friend chase5 trio.
    mb_pathobj(&b, 0, 0, -600, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE5_1, 10, 10);
    mb_pathcspecial(&b, 0, 1500, 100, 1300, SH_ZACO_B, PATH_ID_CHASE5_2, 10, 10);
    mb_pathcspecial(&b, 3000, 0, -600, 0, SH_ZACO_B, PATH_ID_CHASE5_3, 10, 10);

    // MAP3_2.ASM:161-164 – break meteors + mother_1.
    mb_cspecial(&b, 1000, 0, 300, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_cspecial(&b, 1000, -200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    mb_mapmother(&b, 400, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapremove(&b, SH_MOTHER1);
    mb_mapwait(&b, 2000);

    // MAP3_2.ASM:167-168 – mother_5.
    mb_mapmother(&b, 4000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapremove(&b, SH_MOTHER1);

    // MAP3_2.ASM:170-171 – hider (map_meteo0 mother).
    mb_mapmother(&b, 5000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_mapremove(&b, SH_MOTHER1);

    // MAP3_2.ASM:173-180 – mother_5 with itachi formations.
    mb_mapmother(&b, 1500, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    mb_pathcspecial(&b, 1000, 200, -200, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
    mb_pathcspecial(&b, 1000, 0, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_pathcspecial(&b, 1000, -200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
    mb_pathcspecial(&b, 1000, -200, -200, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_pathcspecial(&b, 1000, 0, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    mb_pathcspecial(&b, 1000, 200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
    mb_mapremove(&b, SH_MOTHER1);

    // MAP3_2.ASM:182-183 – supply bird + amebmsg2.
    mb_pathobj(&b, 6000, -380, -150, 0, SH_MY_BIRD, PATH_ID_MY_BIRD, 10, 10);
    mb_pathobj(&b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_AMEBMSG2, 10, 10);

    // MAP3_2.ASM:186-194 – boss section (propeller boss).
    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_BOSS1);
    // incmap webmonst: mapobj 0000,000,000,1200,boss_0_1,webmonster_istrat
    mb_mapobj(&b, 0, 0, 0, 1200, SH_BOSS_0_1, IS_WEBMONSTER);

    // mapwaitboss
    mb_mapwait(&b, 100);
    mb_mapcode65816_inline(&b, &s_level3_2_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level3_2.map3_2.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level3_2.map3_2.bosswait.cont");
    mb_mapgoto(&b, "level3_2.map3_2.bosswait.loop");
    mb_label(&b, "level3_2.map3_2.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level3_2_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level3_2_mapwaitboss_cleanup_script_ptr);

    // markboss boss32
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);

    // MAP3_2.ASM:196 – mapwait 1800
    mb_mapwait(&b, 1800);

    // MAP3_2.ASM:198 – maprts
    mb_maprts(&b);

    // CL_CHASE.ASM – clear demo (chase type) appended as subroutine.
    append_cl_chase_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level3_2 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level3_2.map3_2.skillfly_bonus_0_skip",
                         &s_level3_2_skillfly_bonus_skip_ptr)) {
        s_level3_2_skillfly_bonus_skip_ptr = 0u;
    }

    s_level3_2.data = s_level3_2_data;
    s_level3_2.length = b.length;
}

// ============================================================
// MAP3_3A.ASM — Fortuna Part A (level 3-3)
// ============================================================
static void build_level3_3_wrapper_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level3_3_data;
    b.capacity = sizeof(s_level3_3_data);
    s_level3_3_skillfly_bonus0_guard_script_ptr = 0u;
    s_level3_3_skillfly_bonus0_skip_ptr = 0u;
    s_level3_3_mapwaitboss_trigse_script_ptr = 0u;
    s_level3_3_mapwaitboss_cantdie_script_ptr = 0u;
    s_level3_3_mapwaitboss_cleanup_script_ptr = 0u;
    s_level3_3_pdead2_script_ptr = 0u;
    s_level3_3_pdead_script_ptr = 0u;

    // LEVEL3_3.ASM wrapper: mapjsr map3_3a, 4 flower mapobjs, mapjsr cl_ground, mapend.
    mb_mapjsr(&b, "level3_3.map3_3a");

    // 4 flower mapobjs after map3_3a returns
    mb_mapobj(&b, 0, 800, 0, 8000, SH_FLOWER_1, IS_HARD180YR);
    mb_mapobj(&b, 0, -1000, 0, 10000, SH_FLOWER_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 1000, 0, 12000, SH_FLOWER_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 1200, 0, 12000, SH_FLOWER_1, IS_HARD180YR);

    mb_mapjsr(&b, "cl_ground");
    mb_mapend(&b, 1u);

    // MAP3_3A.ASM — Fortuna Part A subroutine.
    mb_label(&b, "level3_3.map3_3a");
    mb_mapwait(&b, 2500);

    // Lines 6-8: three e_flower path objects
    mb_pathobj(&b, 1000, 0, 0, 2500, SH_NULLSHAPE, PATH_ID_E_FLOWER, 10, 8);
    mb_pathobj(&b, 1000, -200, 0, 2500, SH_NULLSHAPE, PATH_ID_E_FLOWER, 10, 8);
    mb_pathobj(&b, 1000, 200, 0, 2500, SH_NULLSHAPE, PATH_ID_E_FLOWER, 10, 8);

    // Lines 10-12: three tree1 objects
    mb_mapobj(&b, 1000, -200, 0, 2500, SH_STALK, IS_TREE1);
    mb_mapobj(&b, 1000, 200, 0, 2500, SH_STALK, IS_TREE1);
    mb_mapobj(&b, 1000, 0, 0, 2500, SH_STALK, IS_TREE1);

    // Lines 14-16: flower mapobjs
    mb_mapobj(&b, 0x0400, -300, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    mb_mapobj(&b, 0x0900, 100, 0, 4000, SH_FLOWER_1, IS_HARD180YR);
    mb_mapobj(&b, 1000, -800, 0, 4000, SH_FLOWER_1, IS_HARD180YR);

    // Lines 17-18: bee pathcspecials
    mb_pathcspecial(&b, 0x0800, 300, -150, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);
    mb_pathcspecial(&b, 0x0400, -400, -170, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);

    // Lines 19-20: more flowers
    mb_mapobj(&b, 1400, -1000, 0, 4000, SH_FLOWER_1, IS_HARD180YR);
    mb_mapobj(&b, 2400, -800, 0, 4000, SH_FLOWER_1, IS_HARD180YR);

    // Lines 21-22: friend chase6 pair
    mb_pathobj(&b, 0, -750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    mb_pathcspecial(&b, 0, -720, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);

    // Lines 23-27: tomset paths, flowers, bees
    mb_pathobj(&b, 3000, 400, -40, 4000, SH_STALK, PATH_ID_TOMSET, 10, 10);
    mb_mapobj(&b, 2000, 100, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    mb_pathobj(&b, 2000, -300, -40, 4000, SH_STALK, PATH_ID_TOMSET, 10, 10);
    mb_pathcspecial(&b, 0x0600, 100, -120, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);
    mb_pathcspecial(&b, 1400, -100, -120, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);

    // Lines 28-31: more flowers
    mb_mapobj(&b, 500, 200, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    mb_mapobj(&b, 500, -200, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    mb_mapobj(&b, 500, -900, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    mb_mapobj(&b, 1000, 900, 0, 4000, SH_FLOWER_2, IS_HARD180YR);

    // Line 32: tomhaha path
    mb_pathobj(&b, 3000, 0, -1000, 4000, SH_NULLSHAPE, PATH_ID_TOMHAHA, 10, 10);

    // Lines 33-36: flowers
    mb_mapobj(&b, 1000, 400, 0, 4000, SH_FLOWER_1, IS_HARD180YR);
    mb_mapobj(&b, 1000, -500, 0, 4000, SH_FLOWER_1, IS_HARD180YR);
    mb_mapobj(&b, 1000, 100, 0, 4000, SH_FLOWER_2, IS_HARD180YR);
    mb_mapobj(&b, 2000, -200, 0, 4000, SH_FLOWER_2, IS_HARD180YR);

    // Lines 38-39: trees
    mb_mapobj(&b, 300, 300, 0, 1500, SH_STALK, IS_TREE1);
    mb_mapobj(&b, 0, 0, 0, 1500, SH_STALK, IS_TREE1);

    // Line 40: cspecial bom_wing
    mb_cspecial(&b, 0, 0, 0, 4000, SH_BOM_WING, IS_BOMWING);

    // Lines 41-43: ponpon + bees
    mb_pathspecial(&b, 500, -400, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    mb_pathcspecial(&b, 400, 100, -120, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);
    mb_pathcspecial(&b, 400, -100, -100, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);

    // Lines 44-45: ponpon + bee
    mb_pathspecial(&b, 1000, 400, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    mb_pathcspecial(&b, 400, 100, -120, 2000, SH_BEEANIM, PATH_ID_E_BEE, 10, 10);

    // Lines 46-52: trees (tree1 and tree2)
    mb_mapobj(&b, 3500, 200, 0, 1500, SH_STALK, IS_TREE1);
    mb_mapobj(&b, 900, -200, 0, 1300, SH_STALK, IS_TREE1);
    mb_mapobj(&b, 900, 0, 0, 1300, SH_STALK, IS_TREE1);
    mb_mapobj(&b, 900, 200, 0, 1300, SH_STALK, IS_TREE1);
    mb_mapobj(&b, 1200, -300, 0, 1300, SH_STALK, IS_TREE1);
    mb_mapobj(&b, 500, 300, 0, 1800, SH_STALK, IS_TREE2);
    mb_mapobj(&b, 2500, 0, 0, 1800, SH_STALK, IS_TREE2);

    // Line 54: gate
    mb_mapobj(&b, 1000, 0, -100, 2000, SH_GATE_0, IS_GATE);

    // Line 56: e_gate path
    mb_pathobj(&b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 59-60: .pdead2 — dead-check loop
    mb_label(&b, "level3_3.pdead2");
    mb_mapwait(&b, 1000);
    mb_mapcode65816_inline(&b, &s_level3_3_pdead2_script_ptr);

    // Line 61: mapfadetosea
    mb_mapfadetosea(&b);

    // Lines 62-64: transition to water phase
    mb_mapwait(&b, 600);
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_ONWATER_L);
    mb_mapwait(&b, 1000);

    // Lines 66-68: three flyfish pathcspecials
    mb_pathcspecial(&b, 1000, 200, 0, 4000, SH_F_FISH_PROXY, PATH_ID_E_FLYFISH, 10, 10);
    mb_pathcspecial(&b, 5000, 0, 0, 4000, SH_F_FISH_PROXY, PATH_ID_E_FLYFISH, 10, 10);
    mb_pathcspecial(&b, 1000, -200, 0, 4000, SH_F_FISH_PROXY, PATH_ID_E_FLYFISH, 10, 10);

    // Lines 70-72: torpedo spawners
    mb_mapobj(&b, 500, 0, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 500, -300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 2000, 300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);

    // Lines 74-79: kamome + torpedoes
    mb_pathcspecial(&b, 1000, 1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    mb_mapobj(&b, 0, 0, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    mb_pathcspecial(&b, 1000, -1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    mb_mapobj(&b, 0, -300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    mb_pathcspecial(&b, 2000, 1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    mb_mapobj(&b, 2000, 300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);

    // Lines 81-82: seadragon friend chase7 pair
    mb_pathobj(&b, 0, 0, -400, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    mb_pathcspecial(&b, 4000, 0, -400, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);

    // Lines 83-85: kamome trio
    mb_pathcspecial(&b, 1000, -1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    mb_pathcspecial(&b, 1000, 1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    mb_pathcspecial(&b, 2000, -1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);

    // Line 86: nessie 6000,-400,0000,5000,deg180,40
    mb_nessie(&b, 6000, -400, 0, 5000, (int8)128, 40);

    // Lines 87-88: item_6 + setalvar sbyte1
    mb_mapobj(&b, 0, 100, -50, 4700, SH_ITEM_6, IS_ITEM6);
    mb_setalvarb(&b, AL_SBYTE1, 1);

    // Line 89: nessie 3000,-400,0000,5000,deg45,40
    mb_nessie(&b, 3000, -400, 0, 5000, (int8)32, 40);

    // Line 90: mapmother — mother_snakes pattern
    mb_mapmother(&b, 4000, 0, 0, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);

    // Lines 91-92: seadragon2 snakes
    mb_mapobj(&b, 3000, 300, 0, 4000, SH_SNAKE_1, IS_SEADRAGON2);
    mb_mapobj(&b, 2500, -400, 0, 4000, SH_SNAKE_1, IS_SEADRAGON2);

    // Line 93: nessie 3000,-200,0000,5000,deg22,10
    mb_nessie(&b, 3000, -200, 0, 5000, (int8)16, 10);

    // Line 94: maprem mother1
    mb_mapremove(&b, SH_MOTHER1);

    // Lines 95-98: snakes, up1man, more snakes + nessie
    mb_mapobj(&b, 3000, 150, 0, 4000, SH_SNAKE_1, IS_SEADRAGON2);
    mb_mapobj(&b, 2500, 0, -140, 4000, SH_NULLSHAPE, IS_UP1MAN);
    mb_mapobj(&b, 2000, 0, 0, 4000, SH_SNAKE_1, IS_SEADRAGON2);
    mb_nessie(&b, 2000, -200, 0, 5000, (int8)32, 60);

    // Lines 99-103: kamome pair + friend chase6 + kamome
    mb_pathcspecial(&b, 1500, -1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    mb_pathcspecial(&b, 1500, 1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);
    mb_pathobj(&b, 0, -750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    mb_pathcspecial(&b, 0, -720, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);
    mb_pathcspecial(&b, 2500, -1000, -100, 0, SH_BOSS_D_4, PATH_ID_KAMOME, 10, 10);

    // Lines 105-107: three flyfish
    mb_pathcspecial(&b, 1000, 200, 0, 4000, SH_F_FISH_PROXY, PATH_ID_E_FLYFISH, 10, 10);
    mb_pathcspecial(&b, 1000, 0, 0, 4000, SH_F_FISH_PROXY, PATH_ID_E_FLYFISH, 10, 10);
    mb_pathcspecial(&b, 3000, -200, 0, 4000, SH_F_FISH_PROXY, PATH_ID_E_FLYFISH, 10, 10);

    // Lines 108-110: torpedo spawners
    mb_mapobj(&b, 1000, 0, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 1000, -300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 4000, 300, 0, 2000, SH_NULLSHAPE, IS_TORPEDO);

    // Lines 112-113: underwater gate
    mb_mapobj(&b, 1000, 200, -200, 2000, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 0, 3000, -100, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 115-116: .pdead — dead-check loop
    mb_label(&b, "level3_3.pdead");
    mb_mapwait(&b, 1000);
    mb_mapcode65816_inline(&b, &s_level3_3_pdead_script_ptr);

    // Lines 117-120: mapfadetoground + onplanet transition
    mb_mapfadetoground(&b);
    mb_mapwait(&b, 500);
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_ONPLANET_L);
    mb_mapwait(&b, 4000);

    // Lines 122-126: second ground phase opening — trees + ponpon
    mb_mapobj(&b, 500, 150, 0, 1500, SH_STALK, IS_TREE1);
    mb_pathspecial(&b, 0, -300, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    mb_mapobj(&b, 500, 300, 0, 1500, SH_STALK, IS_TREE1);
    mb_mapobj(&b, 500, -300, 0, 1500, SH_STALK, IS_TREE1);
    mb_pathspecial(&b, 0, 300, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);

    // Lines 128-129: skillfly init + set
    mb_skillfly_init(&b);
    mb_skillfly_set_default(&b, -300, -50, 2300);

    // Line 130: roottree 0800,-300,0000,2400,-deg45,30
    mb_roottree(&b, 0x0800, -300, 0, 2400, (int8)(-32), 30);

    // Lines 131-132: tree2 objects
    mb_mapobj(&b, 500, 100, 0, 2500, SH_STALK, IS_TREE2);
    mb_mapobj(&b, 500, -200, 0, 1800, SH_STALK, IS_TREE2);

    // Line 133: ponpon
    mb_pathspecial(&b, 0, 200, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);

    // Line 134: tree2
    mb_mapobj(&b, 500, 0, 0, 2500, SH_STALK, IS_TREE2);

    // Lines 135-136: skillfly_bonus item_5 + setalvar
    mb_mapcode65816_inline(&b, &s_level3_3_skillfly_bonus0_guard_script_ptr);
    mb_mapobj(&b, 0, -250, -80, 1500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level3_3.skillfly_bonus_0_skip");

    // Lines 137-140: tree2 + ponpon + tree1 + tree2
    mb_mapobj(&b, 500, 350, 0, 1800, SH_STALK, IS_TREE2);
    mb_pathspecial(&b, 0, -300, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    mb_mapobj(&b, 500, -150, 0, 1800, SH_STALK, IS_TREE1);
    mb_mapobj(&b, 500, 300, 0, 1800, SH_STALK, IS_TREE2);

    // Line 141: roottree 1500,-200,0000,2400,deg11,40
    mb_roottree(&b, 1500, -200, 0, 2400, (int8)8, 40);

    // Lines 142-143: tree2 + roottree
    mb_mapobj(&b, 500, 300, 0, 1800, SH_STALK, IS_TREE2);
    mb_roottree(&b, 1500, 0, 0, 2400, (int8)16, 10);

    // Lines 144-146: tree2 + item_7 + setalvar
    mb_mapobj(&b, 500, -100, 0, 1800, SH_STALK, IS_TREE2);
    mb_mapobj(&b, 0, 200, -50, 2200, SH_ITEM_7, IS_ITEM7);
    mb_setalvarb(&b, AL_SBYTE1, 1);

    // Line 147: roottree 1500,0200,0000,2400,-deg45,30
    mb_roottree(&b, 1500, 200, 0, 2400, (int8)(-32), 30);

    // Lines 148-149: ponpon + tree2
    mb_pathspecial(&b, 0, 0, 0, 3100, SH_BOM_WING, PATH_ID_PONPON, 10, 10);
    mb_mapobj(&b, 500, -100, 0, 1800, SH_STALK, IS_TREE2);

    // Lines 150-151: roottree + tree2
    mb_roottree(&b, 1500, -200, 0, 3000, (int8)8, 10);
    mb_mapobj(&b, 500, 300, 0, 1800, SH_STALK, IS_TREE2);

    // Lines 152-155: roottree + tree1 + tree2 + roottree pair
    mb_roottree(&b, 1500, -200, 0, 3000, (int8)8, 10);
    mb_mapobj(&b, 500, 100, 0, 1500, SH_STALK, IS_TREE1);
    mb_mapobj(&b, 500, -100, 0, 2200, SH_STALK, IS_TREE2);
    mb_roottree(&b, 0, 0, 0, 3000, (int8)8, 10);

    // Lines 156-158: roottree + tree2 pair
    mb_roottree(&b, 1500, 200, 0, 3000, (int8)8, 40);
    mb_mapobj(&b, 500, -200, 0, 1800, SH_STALK, IS_TREE2);
    mb_mapobj(&b, 3500, 300, 0, 1800, SH_STALK, IS_TREE2);

    // Line 160: dragonmsg path
    mb_pathobj(&b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_DRAGONMSG, 10, 10);

    // Lines 165-168: boss section — fadeoutbgm, setbgm boss, chicken spawn
    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_BOSS1);
    mb_mapobj(&b, 0, -100, 0, 4000, SH_BOSS_D_1, IS_CHICKEN);
    mb_setalvarb(&b, AL_ROTY, 128);  // deg180

    // Lines 169-170: mapwaitboss + markboss boss33
    mb_mapwait(&b, 100);
    mb_mapcode65816_inline(&b, &s_level3_3_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level3_3.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level3_3.bosswait.cont");
    mb_mapgoto(&b, "level3_3.bosswait.loop");
    mb_label(&b, "level3_3.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level3_3_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level3_3_mapwaitboss_cleanup_script_ptr);

    // markboss boss33
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);

    // Line 172: mapwait 1000
    mb_mapwait(&b, 1000);

    // Line 174: maprts
    mb_maprts(&b);

    // ================================================================
    // MAP3_3B.ASM — Fortuna Part B (boss sea-monster torpedo gauntlet)
    // Standalone callable subroutine (INCMAP'd in MAPLIST.ASM).
    // ================================================================
    mb_label(&b, "level3_3.map3_3b");

    // Lines 4-17: torpedo spawners (alternating left/right)
    mb_mapobj(&b, 3000, -400, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 3000,  400, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 1000, -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 1000,  200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 500,  -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 500,   200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 300,  -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 300,   200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 200,  -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 200,   200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 200,  -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 200,   200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 200,  -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 3000,  200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);

    // Line 19: sea monster
    mb_mapobj(&b, 3000, 0, 0, 400, SH_SEA_0_0, IS_SEAMON);

    // Lines 21-27: sea monster V-formation (z = 3000-2000 .. 3300-2000)
    mb_mapobj(&b, 50,    0,    0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 0,    -100,  0, 1100, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 50,    100,  0, 1100, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 0,    -200,  0, 1200, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 50,    200,  0, 1200, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 0,    -300,  0, 1300, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 8000,  300,  0, 1300, SH_SEA_0_0, IS_SEAMON);

    // Lines 29-33: more torpedo spawners
    mb_mapobj(&b, 500,  -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 500,   200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 500,  -200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 500,   200, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);

    // Lines 36-42: descending sea monster arc (left to right)
    mb_mapobj(&b, 300,  -300, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 250,  -200, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 200,  -100, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 150,     0, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 100,   100, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 50,    200, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 8000,  300, 0, 1000, SH_SEA_0_0, IS_SEAMON);

    // Lines 44-50: ascending sea monster arc (right to left)
    mb_mapobj(&b, 300,   300, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 250,   200, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 200,   100, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 150,     0, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 100,  -100, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 50,   -200, 0, 1000, SH_SEA_0_0, IS_SEAMON);
    mb_mapobj(&b, 8000, -300, 0, 1000, SH_SEA_0_0, IS_SEAMON);

    // Line 52: maprts
    mb_maprts(&b);

    // CL_GND.ASM — clear demo (ground type) appended as subroutine.
    append_cl_ground_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level3_3 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level3_3.skillfly_bonus_0_skip",
                         &s_level3_3_skillfly_bonus0_skip_ptr)) {
        s_level3_3_skillfly_bonus0_skip_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level3_3.pdead2",
                         &s_level3_3_pdead2_script_ptr)) {
        s_level3_3_pdead2_script_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level3_3.pdead",
                         &s_level3_3_pdead_script_ptr)) {
        s_level3_3_pdead_script_ptr = 0u;
    }

    s_level3_3.data = s_level3_3_data;
    s_level3_3.length = b.length;
}

static void register_level3_3_inline_callbacks(void) {
    if (s_level3_3_skillfly_bonus0_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_3_skillfly_bonus0_guard_script_ptr,
                                    level3_3_skillfly_bonus0_guard);
    }
    if (s_level3_3_pdead2_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_3_pdead2_script_ptr,
                                    level3_3_pdead2_check);
    }
    if (s_level3_3_pdead_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_3_pdead_script_ptr,
                                    level3_3_pdead_check);
    }
    if (s_level3_3_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_3_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level3_3_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_3_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level3_3_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_3_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

// ============================================================
// LEVEL2_6.ASM — Venom 2 Highway (level 2-6)
// Includes inlined CL_COLON.ASM (colony pipe clear demo).
// map2_6a and final_tunnel are not yet ported.
// ============================================================
static void build_level2_6_wrapper_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level2_6_data;
    b.capacity = sizeof(s_level2_6_data);

    // LEVEL2_6.ASM: initlevel 2_6a,0
    // mapjsr map2_6a — not yet ported
    // mapwait 4000
    mb_mapwait(&b, 4000);

    // ============================================================
    // incmap CL_COLON.ASM — Colony pipe clear demo (inlined)
    // ============================================================
    // CL_COLON.ASM lines 1-2: gpipescale=16
    // Lines 3-4: mapplayercantdie / mapplayermode toCslow
    //   — These opcodes (mapplayercantdie, mapplayermode) are not yet
    //     implemented in the map executor. Skipped for now.
    // TODO: mb_mapplayercantdie(&b);
    // TODO: mb_mapplayermode(&b, PLAYER_MODE_TOCSLOW);

    // Lines 6-8: three pipe background objects (mapobjnomem)
    //   — mapobjnomem is not yet implemented. Emit as regular mapobj.
    mb_mapobj(&b, 0, 0, -60, 4200, SH_PIPE_8_0_PROXY, IS_NOCOLL);
    mb_mapobj(&b, 400, 0, -60, 4200, SH_PIPE_8_0_PROXY, IS_NOCOLL);
    mb_mapobj(&b, 0, 0, -60, 4200, SH_PIPE_8_PROXY, IS_COLONYEXIT);
    // Line 9: mapwait 4000
    mb_mapwait(&b, 4000);

    // Lines 11-14: pdist, setbg, initbg — background setup
    //   — setbg/initbg opcodes exist but bg id '2_6b' is not mapped yet.
    //     Skip background changes for now.

    // Lines 16-64: mappipe sequence (colony pipe path)
    //   — The mappipe opcode is not yet implemented in the map executor.
    //     This is the pipe-following clear demo sequence with 25+ pipe
    //     segments and setalvar rotz calls for camera rotation.
    //     TODO: Implement mappipe opcode in world.c and port these calls.
    // mappipe 0,0,0,0,0
    // mappipe -11,40,-1,0,2
    // mappipe -40,70,-2,1,2
    // ... (full sequence in CL_COLON.ASM)
    // setalvar rotz,0 / rotz,-12 / rotz,-25 / rotz,-42 / rotz,-56
    // End of CL_COLON.ASM inline
    // ============================================================

    // mapwait 2000
    mb_mapwait(&b, 2000);

    // MAP2_6A.ASM line 151: incmap trucker
    // The trucker boss sequence is inlined at this point in the map stream.
    // We emit it as a subroutine for modularity, called via mapjsr.
    mb_mapjsr(&b, "level2_6.trucker");

    // MAP2_6A.ASM lines after trucker: setbgm $f1, mapwait, setbgm $12, markboss
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapwait(&b, (uint16)(MEDPSPEED * 15u * 2u));
    // setbgm $12 — victory fanfare variant (not standard BGM_FANFARE)
    mb_setbgm(&b, 0x12u);
    // markboss boss26 — mark as completed
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);

    mb_emit8(&b, MAP_OP_END);

    // TRUCKER.ASM — Mad Trucker subroutine
    append_trucker_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level2_6 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level2_6.trucker.carryon",
                         &s_trucker_continue_ptr)) {
        // Fallback: if label not found, the trucker callbacks won't branch correctly.
        // This shouldn't happen if the builder succeeded.
    }
    // Look up the rightblockbit label for the trigger callback
    if (!mb_lookup_label(&b, "level2_6.trucker.rightblockbit",
                         &s_trucker_rightblock_ptr)) {
        s_trucker_rightblock_ptr = 0u;
    }
    // Look up the continue label for boss death
    if (!mb_lookup_label(&b, "level2_6.trucker.continue",
                         &s_trucker_continue_ptr)) {
        s_trucker_continue_ptr = 0u;
    }
    // Look up the loop labels
    if (!mb_lookup_label(&b, "level2_6.trucker.loop",
                         &s_trucker_biker_loop_ptr)) {
        s_trucker_biker_loop_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level2_6.trucker.loop2",
                         &s_trucker_trigger_loop_ptr)) {
        s_trucker_trigger_loop_ptr = 0u;
    }

    s_level2_6.data = s_level2_6_data;
    s_level2_6.length = b.length;
}

// ============================================================
// LEVEL_BH.ASM / BHOLE.ASM — Black Hole arena (MAP_ID_BLACKHOLE)
// ============================================================
static void build_level_bh_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level_bh_data_buf;
    b.capacity = sizeof(s_level_bh_data_buf);

    // LEVEL_BH.ASM: initlevel hole,0
    // mapjsr blackholemap
    mb_mapjsr(&b, "blackhole.blackholemap");
    mb_mapend(&b, 1u);

    // BHOLE.ASM — blackholemap subroutine
    mb_label(&b, "blackhole.blackholemap");

    // Line 5: mapwait 2000
    mb_mapwait(&b, 2000);

    // Line 7: mapmother 08000,0000,0,5000,mother1,mother1_istrat,map_bhole
    mb_mapmother(&b, 0x8000, 0, 0, 5000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    // Line 8: maprem mother1
    mb_mapremove(&b, SH_MOTHER1);
    // Line 9: mapobj 1000,000,00,5000,nullshape,up1man_Istrat
    mb_mapobj(&b, 1000, 0, 0, 5000, SH_NULLSHAPE, IS_UP1MAN);

    // .bhole — loop target
    mb_label(&b, "blackhole.bhole");

    // Line 11: mapmother 04000,0000,0,5000,mother1,mother1_istrat,map_bhole
    mb_mapmother(&b, 0x4000, 0, 0, 5000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    // Line 12: cspecial 4000,0000,0000,4500,zaco_0,shou0a_istrat
    mb_cspecial(&b, 4000, 0, 0, 4500, SH_NULLSHAPE, IS_SHOU0A);
    // Line 13: maprem mother1
    mb_mapremove(&b, SH_MOTHER1);

    // Line 14: mapobj 0200,000,000,4000,iris,iris_ISTRAT
    mb_mapobj(&b, 200, 0, 0, 4000, SH_IRIS, IS_IRIS);
    // Line 15: mapobj 0000,000,000,4000,item_7,item7_ISTRAT
    mb_mapobj(&b, 0, 0, 0, 4000, SH_ITEM_7, IS_ITEM7);
    // Line 16: setalvar sbyte1,1
    mb_setalvarb(&b, AL_SBYTE1, 1);

    // Line 17: mapmother 04000,0000,0,5000,mother1,mother1_istrat,map_bhole
    mb_mapmother(&b, 0x4000, 0, 0, 5000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    // Line 18: maprem mother1
    mb_mapremove(&b, SH_MOTHER1);

    // Lines 20-21: exitgate2_4
    mb_mapobj(&b, 0, 0x0100, 0, 5400, SH_GATE_0, IS_BHOLEEXIT2);
    mb_pathobj(&b, 4500, 3000, 3000, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Line 22: mapmother 04000,0000,0,4000,mother1,mother1_istrat,map_bhole
    mb_mapmother(&b, 0x4000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    // Line 23: special 1000,0000,0000,4500,para_0,shou0a_istrat
    mb_special(&b, 1000, 0, 0, 4500, SH_PARA_0, IS_SHOU0A);
    // Line 24: maprem mother1
    mb_mapremove(&b, SH_MOTHER1);

    // Line 25: mapmother 0400,0000,0000,4000,mother1,mother2_istrat,map_amoebas
    mb_mapmother(&b, 0x0400, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    // Line 26: maprem mother1
    mb_mapremove(&b, SH_MOTHER1);

    // Line 27: mapmother 02000,0000,0,5000,mother1,mother1_istrat,map_bhole
    mb_mapmother(&b, 0x2000, 0, 0, 5000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    // Line 28: maprem mother1
    mb_mapremove(&b, SH_MOTHER1);

    // Line 29: mapobj 0200,000,000,4000,iris,iris_ISTRAT
    mb_mapobj(&b, 200, 0, 0, 4000, SH_IRIS, IS_IRIS);
    // Line 30: mapobj 0000,000,000,4000,item_5,item5_ISTRAT
    mb_mapobj(&b, 0, 0, 0, 4000, SH_ITEM_5, IS_ITEM5);
    // Line 31: setalvar sbyte1,1
    mb_setalvarb(&b, AL_SBYTE1, 1);

    // Line 32: mapmother 04000,0000,0,5000,mother1,mother1_istrat,map_bhole
    mb_mapmother(&b, 0x4000, 0, 0, 5000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    // Line 33: maprem mother1
    mb_mapremove(&b, SH_MOTHER1);

    // Lines 35-36: exitgate3_4
    mb_mapobj(&b, 0, -0x0200, -100, 5400, SH_GATE_0, IS_BHOLEEXIT3);
    mb_pathobj(&b, 4500, 3000, 3000, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Line 37: mapmother 04000,0000,0,4000,mother1,mother1_istrat,map_bhole
    mb_mapmother(&b, 0x4000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    // Line 38: special 3000,0000,0000,4500,shieldr,shou0a_istrat
    mb_special(&b, 3000, 0, 0, 4500, SH_SHIELDR, IS_SHOU0A);
    // Line 39: maprem mother1
    mb_mapremove(&b, SH_MOTHER1);

    // Line 40: mapobj 0200,000,000,4000,iris,iris_ISTRAT
    mb_mapobj(&b, 200, 0, 0, 4000, SH_IRIS, IS_IRIS);
    // Line 41: mapobj 0000,000,000,4000,item_5,item5_ISTRAT
    mb_mapobj(&b, 0, 0, 0, 4000, SH_ITEM_5, IS_ITEM5);
    // Line 42: setalvar sbyte1,1
    mb_setalvarb(&b, AL_SBYTE1, 1);

    // Line 43: mapmother 04000,0000,0,5000,mother1,mother1_istrat,map_bhole
    mb_mapmother(&b, 0x4000, 0, 0, 5000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    // Line 44: maprem mother1
    mb_mapremove(&b, SH_MOTHER1);

    // Lines 46-47: exitgate1_5
    mb_mapobj(&b, 0, 0x0200, 0x0100, 5400, SH_GATE_0, IS_BHOLEEXIT1);
    mb_pathobj(&b, 4500, 3000, 3000, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Line 48: mapgoto .bhole
    mb_mapgoto(&b, "blackhole.bhole");

    // Line 49: maprts
    mb_maprts(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level_bh = s_empty_level;
        return;
    }

    s_level_bh.data = s_level_bh_data_buf;
    s_level_bh.length = b.length;
}

static void build_level2_4_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level2_4_data;
    b.capacity = sizeof(s_level2_4_data);
    s_level2_4_mapwaitboss_trigse_script_ptr = 0u;
    s_level2_4_mapwaitboss_cantdie_script_ptr = 0u;
    s_level2_4_mapwaitboss_cleanup_script_ptr = 0u;

    // LEVEL2_4.ASM wrapper: mapjsr map2_4, mapjsr cl_turn, mapend.
    mb_mapjsr(&b, "level2_4.map2_4");
    mb_mapjsr(&b, "cl_turn");
    mb_mapend(&b, 1u);

    // MAP2_4.ASM — Sector Y subroutine.
    mb_label(&b, "level2_4.map2_4");

    mb_mapwait(&b, 600);

    mb_pathobj(&b, 0, 180, -300, -200, SH_WHALE_PROXY, PATH_ID_E_WHALE, 10, 10);

    mb_map_sfish(&b, 2800, 0, -100, 1000, 10);

    mb_pathobj(&b, 1000, 0, -150, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    mb_pathobj(&b, 1000, 150, 0, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    mb_pathobj(&b, 1000, -150, 0, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);

    mb_pathobj(&b, 300, 1500, 900, 3200, SH_IKA, PATH_ID_IKA_2, 10, 10);
    mb_pathobj(&b, 5000, 1800, 1100, 2800, SH_IKA, PATH_ID_E_IKA, 10, 10);

    mb_pathcspecial(&b, 200, 100, -300, 0, SH_ZACO_7, PATH_ID_EGU1, 4, 10);
    mb_pathcspecial(&b, 200, 300, -600, 0, SH_ZACO_7, PATH_ID_EGU1, 4, 10);
    mb_pathcspecial(&b, 6000, 500, -900, 0, SH_ZACO_7, PATH_ID_EGU1, 4, 10);

    mb_pathobj(&b, 1000, -1500, 900, 2800, SH_IKA, PATH_ID_E_IKA, 10, 10);
    mb_pathobj(&b, 1000, 1500, 900, 3200, SH_IKA, PATH_ID_IKA_2, 10, 10);
    mb_pathobj(&b, 0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    mb_pathcspecial(&b, 4000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);

    mb_pathcspecial(&b, 500, 300, -1400, 2000, SH_ZACO_B, PATH_ID_EGU3, 10, 10);
    mb_pathcspecial(&b, 12000, -300, -1400, 2000, SH_ZACO_B, PATH_ID_EGU3, 10, 10);

    // .amoebas1 loop: 3 iterations of mapmother + maprem
    mb_label(&b, "level2_4.amoebas1");
    mb_mapmother(&b, 200, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_mapremove(&b, SH_MOTHER1);

    mb_mapwait(&b, 1000);
    mb_maploop(&b, "level2_4.amoebas1", 3);
    mb_pathobj(&b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_AMEBMSG, 10, 10);

    mb_mapmother(&b, 200, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_mapremove(&b, SH_MOTHER1);

    mb_mapobj(&b, 0, 100, -100, 4500, SH_NULLSHAPE, IS_UP1MAN);
    mb_setalvarw(&b, AL_SWORD2, SH_ITEM_0_PROXY);
    mb_mapwait(&b, 1000);

    mb_mapmother(&b, 200, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_mapremove(&b, SH_MOTHER1);

    mb_mapwait(&b, 1000);

    mb_mapmother(&b, 8000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_pathcspecial(&b, 300, 300, -300, 0, SH_ZACO_7, PATH_ID_EGU1_IFRO, 4, 10);
    mb_pathcspecial(&b, 300, 500, -600, 0, SH_ZACO_7, PATH_ID_EGU1_IRAB, 4, 10);
    mb_pathcspecial(&b, 4000, 700, -900, 0, SH_ZACO_7, PATH_ID_EGU1_IFAL, 4, 10);
    mb_mapremove(&b, SH_MOTHER1);

    mb_mapwait(&b, 5000);

    mb_cspecial(&b, 4000, -700, -300, 3000, SH_W_L, IS_WINGLAZERMAN);
    mb_pathobj(&b, 0, 0, 0, 3000, SH_NULLSHAPE, PATH_ID_BRAYMSG, 10, 10);
    mb_pathobj(&b, 6700, 0, -250, 0, SH_RAY_1, PATH_ID_E_RAY_1, 10, 10);
    mb_pathobj(&b, 0, 0, -600, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE5_1, 10, 10);
    mb_pathcspecial(&b, 0, 1500, 100, 1300, SH_ZACO_B, PATH_ID_CHASE5_2, 10, 10);
    mb_pathcspecial(&b, 5000, 0, -600, 0, SH_ZACO_B, PATH_ID_CHASE5_3, 10, 10);

    mb_pathobj(&b, 5000, 0, 250, 0, SH_RAY_1, PATH_ID_E_RAY_1, 10, 10);
    mb_pathspecial(&b, 500, 0, -1400, 2000, SH_S_ZACO_0, PATH_ID_EGU3, 10, 10);
    mb_pathcspecial(&b, 500, -300, 1400, 2000, SH_ZACO_B, PATH_ID_EGU3, 10, 10);
    mb_pathcspecial(&b, 8000, 300, 1400, 2000, SH_ZACO_B, PATH_ID_EGU3, 10, 10);

    mb_pathobj(&b, 0, 1200, 200, 600, SH_FRIENDSHIP_4, PATH_ID_CHASE1_1, 10, 10);
    mb_pathcspecial(&b, 3000, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);

    mb_mapobj(&b, 0, 0, 0, 4000, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    mb_mapwait(&b, 1000);

    // bzaco_8 patret trio + sfish school
    mb_pathcspecial(&b, 200, 0, 200, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    mb_pathcspecial(&b, 200, 800, -200, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    mb_pathcspecial(&b, 200, -800, -200, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    mb_map_sfish(&b, 800, 0, -100, 1000, 8);

    mb_pathobj(&b, 5000, -1500, 1100, 2800, SH_IKA, PATH_ID_IKA_2, 10, 10);

    mb_pathobj(&b, 0, -100, -250, 0, SH_RAY_1, PATH_ID_E_RAY_1, 10, 10);

    mb_pathobj(&b, 500, -150, -120, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    mb_pathobj(&b, 2000, -200, 0, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    mb_pathobj(&b, 500, 50, -150, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    mb_pathobj(&b, 500, 50, 150, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);

    // (line 91 commented out in original ASM)
    mb_pathobj(&b, 3000, -200, 250, 0, SH_RAY_1, PATH_ID_E_RAY_1, 10, 10);

    mb_pathobj(&b, 500, -150, -120, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    mb_pathobj(&b, 500, 2100, 1000, 3000, SH_IKA, PATH_ID_IKA_2, 10, 10);
    mb_pathobj(&b, 500, -200, 0, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    mb_map_sfish(&b, 0, 0, -100, 1000, 4);
    mb_pathobj(&b, 5000, 80, -200, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    mb_pathcspecial(&b, 0, -600, -600, 0, SH_ZACO_7, PATH_ID_EGU1, 10, 10);
    mb_pathspecial(&b, 0, 300, 1400, 2000, SH_S_ZACO_0, PATH_ID_EGU3, 10, 10);
    mb_pathspecial(&b, 5000, -300, -1400, 2000, SH_S_ZACO_0, PATH_ID_EGU3, 10, 10);

    mb_pathobj(&b, 0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 10, 10);
    mb_pathcspecial(&b, 13000, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);

    mb_pathobj(&b, 0, 0, 0, 3000, SH_NULLSHAPE, PATH_ID_REM_WHALE, 10, 10);

    mb_pathobj(&b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_HANDMSG, 10, 10);

    // fadeoutbgm + setbgm 5
    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_BOSS1);

    // mapjsr armsmap (inline: flingboss + maprts)
    mb_mapjsr(&b, "level2_4.armsmap");

    // mapwaitboss
    mb_mapwait(&b, 100);
    mb_mapcode65816_inline(&b, &s_level2_4_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level2_4.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level2_4.bosswait.cont");
    mb_mapgoto(&b, "level2_4.bosswait.loop");
    mb_label(&b, "level2_4.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level2_4_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level2_4_mapwaitboss_cleanup_script_ptr);

    // markboss boss24
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);

    mb_mapwait(&b, 2000);

    mb_maprts(&b);

    // armsmap subroutine: flingboss at (0, -80, 2000)
    mb_label(&b, "level2_4.armsmap");
    mb_mapobj(&b, 0, 0, -80, 2000, SH_FLINGBOSS, IS_FLINGBOSS);
    mb_maprts(&b);

    // CL_TURN.ASM — clear demo (turn type) appended as subroutine.
    append_cl_turn_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level2_4 = s_empty_level;
        return;
    }

    s_level2_4.data = s_level2_4_data;
    s_level2_4.length = b.length;
}

static void register_level2_4_inline_callbacks(void) {
    if (s_level2_4_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_4_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level2_4_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_4_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level2_4_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_4_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

// ============================================================
// MAP2_5.ASM — Venom 2 Orbital (level 2-5)
// ============================================================
static void build_level2_5_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level2_5_data;
    b.capacity = sizeof(s_level2_5_data);
    s_level2_5_skillfly_bonus0_guard_script_ptr = 0u;
    s_level2_5_skillfly_bonus0_skip_ptr = 0u;
    s_level2_5_skillfly_bonus1_guard_script_ptr = 0u;
    s_level2_5_skillfly_bonus1_skip_ptr = 0u;
    s_level2_5_mapwaitboss_trigse_script_ptr = 0u;
    s_level2_5_mapwaitboss_cantdie_script_ptr = 0u;
    s_level2_5_mapwaitboss_cleanup_script_ptr = 0u;

    // LEVEL2_5.ASM wrapper: mapjsr map2_5, mapjsr cl_dive, mapend__not level2_6.
    mb_mapjsr(&b, "level2_5.map2_5");
    mb_mapjsr(&b, "cl_dive");
    // mapend__not level2_6: sets levelfinished=7, game loop handles transition.
    mb_mapend(&b, 7u);

    // MAP2_5.ASM — Venom 2 Orbital subroutine.
    mb_label(&b, "level2_5.map2_5");

    // mapwait 600
    mb_mapwait(&b, 600);

    // Lines 4-6: pathspecial / pathcspecial trio (egu6)
    mb_pathspecial(&b, 0, 2700, 2000, 1500, SH_S_ZACO_0, PATH_ID_EGU6, 10, 10);
    mb_pathcspecial(&b, 0, 2500, 2000, 1800, SH_ZACO_8, PATH_ID_EGU6, 10, 10);
    mb_pathcspecial(&b, 3000, 2900, 2000, 2100, SH_ZACO_8, PATH_ID_EGU6, 10, 10);

    // Lines 8-10: pathspecial / pathcspecial trio (egu6 variants)
    mb_pathspecial(&b, 0, -2700, 2000, 1500, SH_S_ZACO_0, PATH_ID_EGU6_IFAL, 10, 10);
    mb_pathcspecial(&b, 0, -2500, 2000, 1800, SH_ZACO_8, PATH_ID_EGU6_IRAB, 10, 10);
    mb_pathcspecial(&b, 9000, -2900, 2000, 2100, SH_ZACO_8, PATH_ID_EGU6_IFRO, 10, 10);

    // Lines 12-14: pathcspecial / pathspecial / pathcspecial trio (egu5)
    mb_pathcspecial(&b, 400, -300, 2200, 2800, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    mb_pathspecial(&b, 400, 0, 2200, 2500, SH_S_ZACO_0, PATH_ID_EGU5, 10, 10);
    mb_pathcspecial(&b, 7000, 300, 2200, 3100, SH_BZACO_8, PATH_ID_EGU5, 10, 10);

    // Lines 16-17: friendship_4 chase + zaco_b chase
    mb_pathobj(&b, 0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    mb_pathcspecial(&b, 8000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);

    // Lines 19-25: check + minicas2 group
    mb_pathobj(&b, 0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    mb_pathobj(&b, 700, -200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 700, 200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 700, 0, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 700, 100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 2500, -100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);

    // Lines 26-32: mapmother + cspecial uper_m group + maprem
    mb_mapmother(&b, 1000, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_cspecial(&b, 1000, 0, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1000, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1000, -100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1000, 200, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1000, -200, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_mapremove(&b, SH_MOTHER1);

    // Lines 34-35: pathspecial egu6 pair
    mb_pathspecial(&b, 400, -2700, 2200, 1500, SH_S_ZACO_0, PATH_ID_EGU6, 10, 10);
    mb_pathspecial(&b, 400, 2700, 2200, 1500, SH_S_ZACO_0, PATH_ID_EGU6, 10, 10);

    // Lines 37-38: pathcspecial egu6 pair
    mb_pathcspecial(&b, 400, -2500, 2200, 1800, SH_ZACO_8, PATH_ID_EGU6, 10, 10);
    mb_pathcspecial(&b, 400, 2500, 2200, 1800, SH_ZACO_8, PATH_ID_EGU6, 10, 10);

    // Lines 40-41: pathcspecial egu6 variants pair
    mb_pathcspecial(&b, 400, -2900, 2200, 2100, SH_ZACO_8, PATH_ID_EGU6_IRAB, 10, 10);
    mb_pathcspecial(&b, 6000, 2900, 2200, 2100, SH_ZACO_8, PATH_ID_EGU6_IFAL, 10, 10);

    // Lines 43-47: check + minicas2 group
    mb_pathobj(&b, 0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    mb_pathobj(&b, 800, -200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 800, 200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 800, 0, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);

    // === Skillfly section 1 (lines 48-68) ===
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, -50, 4000, 100);

    // Lines 51-56: damyscr pathcspecials with setalvar
    mb_pathcspecial(&b, 0, 180, 100, 4000, SH_BOSS_E_4, PATH_ID_DAMYSCR, 10, 10);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_pathcspecial(&b, 0, -180, 100, 4000, SH_BOSS_E_4, PATH_ID_DAMYSCR, 10, 10);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_pathcspecial(&b, 0, 0, -200, 4000, SH_BOSS_E_4, PATH_ID_DAMYSCR, 10, 10);
    mb_setalvarb(&b, AL_SBYTE1, 1);

    // Lines 57-61: minicas2 group
    mb_pathobj(&b, 800, 100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 800, -100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 800, -200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 800, 200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 800, 0, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);

    // Line 62: skillfly_bonus item_7
    mb_mapcode65816_inline(&b, &s_level2_5_skillfly_bonus0_guard_script_ptr);
    mb_mapobj(&b, 0, 0, -50, 2000, SH_ITEM_7, IS_ITEM7);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level2_5.skillfly_bonus_0_skip");

    // Lines 63-67: setalvar + more minicas2 + item_5 mapobj
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_pathobj(&b, 800, 100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_mapobj(&b, 0, -100, -100, 3500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_pathobj(&b, 800, -100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);

    // Line 69: gate_0 mapobj
    mb_mapobj(&b, 0, 0, 0, 4000, SH_GATE_0, IS_GATE);

    // Lines 71-72: e_gate pathobj + mapwait 1600
    mb_pathobj(&b, 300, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    mb_mapwait(&b, 1600);

    // Lines 74-81: check + minicas2 group + chase1 + more minicas2
    mb_pathobj(&b, 0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    mb_pathobj(&b, 700, -200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 700, 200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 0, 1200, 200, 600, SH_FRIENDSHIP_4, PATH_ID_CHASE1_1, 10, 10);
    mb_pathcspecial(&b, 0, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);
    mb_pathobj(&b, 700, 0, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 700, 100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    mb_pathobj(&b, 4000, -100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);

    // Lines 83-84: zaco_4 egu3 pair
    mb_pathcspecial(&b, 0, 800, 1300, 2000, SH_ZACO_4, PATH_ID_EGU3, 10, 10);
    mb_pathcspecial(&b, 1000, -800, 1300, 2300, SH_ZACO_4, PATH_ID_EGU3, 10, 10);

    // Line 86: cspecial wait=500,x=0,y=Space_viewCY-500,z=800
    mb_cspecial(&b, 500, 0, (int16)(SPACE_VIEWCY - 500), 800, SH_ZACO_4, IS_SZACO0);

    // Lines 88-89: zaco_4 egu3 pair
    mb_pathcspecial(&b, 0, 200, 1900, 2000, SH_ZACO_4, PATH_ID_EGU3, 10, 10);
    mb_pathcspecial(&b, 4000, -200, 1900, 2300, SH_ZACO_4, PATH_ID_EGU3, 10, 10);

    // Lines 91-93: bzaco_8 + s_zaco_0 egu5 trio
    mb_pathcspecial(&b, 300, -300, 2200, 2000, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    mb_pathspecial(&b, 300, 0, 2200, 1700, SH_S_ZACO_0, PATH_ID_EGU5, 10, 10);
    mb_pathcspecial(&b, 8000, 300, 2200, 2300, SH_BZACO_8, PATH_ID_EGU5, 10, 10);

    // Lines 95-99: mapmother + cspecial uper_m group (second mother)
    mb_mapmother(&b, 1000, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_cspecial(&b, 1000, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1000, -100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1000, 200, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1000, -200, 2000, 3000, SH_UPER_M, IS_UPERM);

    // Lines 101-102: friendship_4 chase3 + zaco_b chase3
    mb_pathobj(&b, 0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 10, 10);
    mb_pathcspecial(&b, 10000, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);

    // Lines 104-110: bazookaL + uper_m group + maprem + bazookaR
    mb_cspecial(&b, 1000, -150, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAL);
    mb_cspecial(&b, 1000, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1000, -100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1000, 200, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1000, -200, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_mapremove(&b, SH_MOTHER1);
    mb_cspecial(&b, 5500, 150, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAR);

    // Lines 112-115: egu6 pathspecial + pathcspecial pairs
    mb_pathspecial(&b, 0, -2000, 2000, 2000, SH_S_ZACO_0, PATH_ID_EGU6, 10, 10);
    mb_pathspecial(&b, 0, 2000, 2000, 2300, SH_S_ZACO_0, PATH_ID_EGU6, 10, 10);
    mb_pathcspecial(&b, 0, -2000, -2000, 2600, SH_ZACO_8, PATH_ID_EGU6, 10, 10);
    mb_pathcspecial(&b, 9000, 2000, -2000, 2900, SH_ZACO_8, PATH_ID_EGU6_IFAL, 10, 10);

    // Lines 117-119: wire_man + bazooka pair
    mb_cspecial(&b, 6000, 0, 1500, 2000, SH_WIRE_MAN, IS_WIREMAN);
    mb_cspecial(&b, 4000, -150, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAL);
    mb_cspecial(&b, 4000, 150, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAR);

    // === Skillfly section 2 (lines 121-143) ===
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, -100, 2500, 120);

    mb_pathcspecial(&b, 1000, 0, -100, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    mb_pathcspecial(&b, 1000, -500, -200, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    mb_pathcspecial(&b, 300, -300, 2200, 2800, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    mb_pathspecial(&b, 300, 0, 2200, 2500, SH_S_ZACO_0, PATH_ID_EGU5, 10, 10);

    // Line 128: skillfly_bonus item_5
    mb_mapcode65816_inline(&b, &s_level2_5_skillfly_bonus1_guard_script_ptr);
    mb_mapobj(&b, 0, 0, -100, 2000, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level2_5.skillfly_bonus_1_skip");

    mb_pathcspecial(&b, 300, 300, 2200, 3100, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    mb_pathcspecial(&b, 1000, -200, 50, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    mb_pathcspecial(&b, 800, 500, 0, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    mb_pathcspecial(&b, 800, -300, -150, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    mb_pathcspecial(&b, 800, 100, -200, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    mb_pathcspecial(&b, 300, -300, 2200, 2800, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    mb_pathspecial(&b, 300, 0, 2200, 2500, SH_S_ZACO_0, PATH_ID_EGU5, 10, 10);
    mb_pathcspecial(&b, 300, 300, 2200, 3100, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    mb_pathcspecial(&b, 600, -200, 50, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    mb_pathobj(&b, 0, -250, -350, 0, SH_MY_BIRD, PATH_ID_MY_BIRD, 10, 10);
    mb_pathcspecial(&b, 600, 0, -150, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    mb_pathcspecial(&b, 600, 500, -130, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    mb_pathcspecial(&b, 400, -500, -200, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    mb_pathcspecial(&b, 3400, 200, -100, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);

    // Line 145: kastmsg
    mb_pathobj(&b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_KASTMSG, 10, 10);

    // Lines 151-152: fadeoutbgm + setbgm 5
    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_BOSS1);

    // Line 153: incmap castanet — spawn Metal Smasher boss
    mb_mapobj(&b, 0, 0, 0, 2000, SH_NULLSHAPE, IS_CASTANET);

    // Lines 154-155: mapwaitboss / markboss boss25
    mb_mapwait(&b, 100);
    mb_mapcode65816_inline(&b, &s_level2_5_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level2_5.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level2_5.bosswait.cont");
    mb_mapgoto(&b, "level2_5.bosswait.loop");
    mb_label(&b, "level2_5.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level2_5_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level2_5_mapwaitboss_cleanup_script_ptr);

    // markboss boss25
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);

    // Line 157: mapwait 2400
    mb_mapwait(&b, 2400);

    // Line 159: maprts
    mb_maprts(&b);

    // CL_DIVE.ASM — clear demo (dive type) appended as subroutine.
    append_cl_dive_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level2_5 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level2_5.skillfly_bonus_0_skip",
                         &s_level2_5_skillfly_bonus0_skip_ptr)) {
        s_level2_5_skillfly_bonus0_skip_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level2_5.skillfly_bonus_1_skip",
                         &s_level2_5_skillfly_bonus1_skip_ptr)) {
        s_level2_5_skillfly_bonus1_skip_ptr = 0u;
    }

    s_level2_5.data = s_level2_5_data;
    s_level2_5.length = b.length;
}

static void register_level2_5_inline_callbacks(void) {
    if (s_level2_5_skillfly_bonus0_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_5_skillfly_bonus0_guard_script_ptr,
                                    level2_5_skillfly_bonus0_guard);
    }
    if (s_level2_5_skillfly_bonus1_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_5_skillfly_bonus1_guard_script_ptr,
                                    level2_5_skillfly_bonus1_guard);
    }
    if (s_level2_5_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_5_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level2_5_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_5_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level2_5_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_5_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

// ============================================================
// MAP2_3A.ASM — Titania Part A (level 2-3)
// ============================================================
static void build_level2_3a_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level2_3_data;
    b.capacity = MAP_DATA_CAPACITY;
    s_level2_3_skillfly_bonus_guard_script_ptr = 0u;
    s_level2_3_skillfly_bonus_skip_ptr = 0u;
    s_level2_3_fog_guard_script_ptr = 0u;
    s_level2_3_fog_guard_continue_ptr = 0u;
    s_level2_3_setvar_inline_script_ptr = 0u;

    // Sync player_posx into WRAM mirror for ADDALVARPW usage.
    RAM16(WM_PLAYERPOSX) = 0;

    // 2-3-1
    mb_setvarb(&b, WM_INFOG, 1);
    mb_mapwait(&b, 2000);
    // -----------------------------------------------------------------------
    mb_pathobj(&b, 0, 0x0400, -120, 2500, SH_R_BUT_2_PROXY, PATH_ID_PINITA_B, 10, 10);
    mb_pathobj(&b, 0, -0x0400, -120, 2500, SH_R_BUT_2_PROXY, PATH_ID_PINITA_B, 10, 10);
    mb_pathobj(&b, 2000, 0, -120, 2500, SH_R_BUT_2_PROXY, PATH_ID_PINITA_A, 10, 10);
    mb_mapobj(&b, 0, -0x0600, 0, 2000, SH_BRO_4, IS_ROCKHARD);
    mb_mapobj(&b, 0x0500, 0x0600, 0, 2000, SH_BRO_5, IS_ROCKHARD);
    mb_maphardrot(&b, 0, -150, -75, 2000, SH_CLISLA_M, 0, 8, 0);
    mb_pathobj(&b, 0x0500, 0x0050, -75, 2000, SH_CLISLA_S, PATH_ID_L_CLISLA, 10, 10);
    mb_mapobj(&b, 0, -0x0550, 0, 2000, SH_BRO_0, IS_ROCKHARD);
    mb_mapobj(&b, 0x1000, 0x0350, 0, 2000, SH_BRO_5, IS_ROCKHARD);
    mb_mapobj(&b, 0x0500, -200, 0, 2000, SH_HOU_5, IS_HOUDAI5F);

    mb_mapobj(&b, 0, -0x0700, 0, 2000, SH_BRO_0, IS_ROCKHARD);
    mb_mapobj(&b, 0, 0x0150, 0, 2000, SH_BRO_5, IS_ROCKHARD);
    mb_pathcspecial(&b, 0x1000, 0x0150, 0, 2600, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);
    mb_mapobj(&b, 0, -0x0600, 0, 2000, SH_BRO_2, IS_ROCKHARD);
    mb_mapobj(&b, 0x1000, 0x0500, 0, 2000, SH_BRO_1, IS_ROCKHARD);
    mb_mapobj(&b, 0, -0x0400, 0, 2000, SH_BRO_4, IS_ROCKHARD);
    mb_mapobj(&b, 0x1000, 0x0550, 0, 2000, SH_BRO_1, IS_ROCKHARD);

    mb_mapobj(&b, 0, -0x0650, 0, 2000, SH_BRO_0, IS_ROCKHARD);
    mb_mapobj(&b, 0, 0x0650, 0, 2000, SH_BRO_1, IS_ROCKHARD);
    mb_mapobj(&b, 0, 0, 0, 2000, SH_BRO_6, IS_ROCKHARD);
    mb_mapobj(&b, 0, -160, -190, 2500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_mapwait(&b, 200);
    mb_pathspecial(&b, 0x1000, 0, 0, 2350, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);
    mb_maphardrot(&b, 0, 0, -75, 2000, SH_CLISLA_M, 0, -8, 0);
    mb_pathobj(&b, 0, -200, -75, 2000, SH_CLISLA_S, PATH_ID_R_CLISLA, 10, 10);
    mb_mapobj(&b, 0, -0x0500, 0, 2000, SH_BRO_2, IS_ROCKHARD);
    mb_mapobj(&b, 0x1000, 0x0500, 0, 2000, SH_BRO_3, IS_ROCKHARD);
    mb_mapobj(&b, 0, -0x0500, 0, 2000, SH_BRO_4, IS_ROCKHARD);
    mb_mapobj(&b, 0, 0x0500, 0, 2000, SH_BRO_5, IS_ROCKHARD);
    mb_mapobj(&b, 0x1000, 0, 0, 2000, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0, 250, 0, 2000, SH_BRO_6, IS_HARD180YR);
    mb_mapobj(&b, 0x0500, -250, 0, 2000, SH_BRO_6, IS_HARD180YR);
    mb_pathobj(&b, 0x1500, 0, -120, 2500, SH_R_BUT_2_PROXY, PATH_ID_PINITA_B, 10, 10);

    // 2-3-2
    mb_mapobj(&b, 0, -300, 0, 2000, SH_BRO_6, IS_HARD180YR);
    mb_pathspecial(&b, 0x1000, -300, 0, 2500, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);
    mb_mapobj(&b, 0, 0x0600, 0, 2000, SH_BRO_6, IS_HARD180YR);
    mb_pathspecial(&b, 0x1000, 0x0600, 0, 2500, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);

    mb_pathobj(&b, 0, -750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    mb_pathcspecial(&b, 0, -720, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);

    // misstank
    mb_cspecial(&b, 0, -1000, 0, 3000, SH_M_TANK, IS_MISSTANK);
    mb_setalvarb(&b, AL_ROTY, (uint8)(-64));  // -deg90
    mb_addalvarptrw(&b, AL_WORLDX, WM_PLAYERPOSX);

    mb_pathobj(&b, 0x0700, 300, -200, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x0700, -200, -45, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x0700, 0x0100, -30, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0, 250, -120, 2500, SH_R_BUT_2_PROXY, PATH_ID_PINITA_B, 10, 10);
    mb_pathobj(&b, 0x0700, -400, -100, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);

    mb_cspecial(&b, 0, 1000, 0, 3000, SH_M_TANK, IS_MISSTANK);
    mb_setalvarb(&b, AL_ROTY, 64);  // deg90
    mb_addalvarptrw(&b, AL_WORLDX, WM_PLAYERPOSX);

    mb_pathobj(&b, 0x0700, -300, -200, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x0700, 400, -100, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x0700, -100, -30, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x0700, 200, -45, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);

    mb_cspecial(&b, 0, -1000, 0, 3000, SH_M_TANK, IS_MISSTANK);
    mb_setalvarb(&b, AL_ROTY, (uint8)(-64));  // -deg90
    mb_addalvarptrw(&b, AL_WORLDX, WM_PLAYERPOSX);

    mb_pathobj(&b, 0, 0x0100, -120, 2500, SH_R_BUT_2_PROXY, PATH_ID_PINITA_B, 10, 10);

    // .fogagain
    mb_label(&b, "level2_3.fogagain");
    mb_pathobj(&b, 0x0700, -300, -200, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x0700, 400, -100, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x0700, -100, -30, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 3000, 200, -45, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x1000, 0, -170, 3000, SH_WALK_4_0_PROXY, PATH_ID_E_KANI_0, 10, 10);

    mb_pathobj(&b, 0x0700, -300, -200, 3000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x0700, 400, -100, 3000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x0700, -100, -30, 3000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x1000, 200, -45, 3000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);

    // base
    mb_mapobj(&b, 0, 0x0350, 0, 3000, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0x1400, -350, 0, 3000, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0, 0, -50, 4200, SH_ITEM_7, IS_ITEM7);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_mapobj(&b, 0x0500, 0, 0, 4000, SH_BASE_0, IS_BASE1);

    mb_pathspecial(&b, 0, 0x0500, 0, 4400, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    mb_mapobj(&b, 0, 0x0500, 0, 4000, SH_BASE_0, IS_BASE1);
    // skillfly_init / skillfly_set are commented out in the ASM
    mb_pathobj(&b, 0, 0x0500, -100, 4030, SH_CORE_1_1, PATH_ID_TENKI_ON, 10, 10);
    mb_pathobj(&b, 0x0500, 0x0500, 0, 4030, SH_RADER_1, PATH_ID_TENKI_DM, 10, 10);

    mb_pathspecial(&b, 0, -0x0500, 0, 4400, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    mb_pathobj(&b, 0, 0, -120, 4000, SH_R_BUT_2_PROXY, PATH_ID_PINITA_B, 10, 10);
    mb_mapobj(&b, 3500, -0x0500, 0, 4000, SH_BASE_0, IS_BASE1);

    // eguchi2fly_goto .fogout
    mb_mapcode65816_inline(&b, &s_level2_3_fog_guard_script_ptr);
    mb_mapgoto(&b, "level2_3.fogout");
    mb_label(&b, "level2_3.fog_guard_continue");
    mb_mapwait(&b, 500);
    mb_mapgoto(&b, "level2_3.fogagain");

    // .fogout
    mb_label(&b, "level2_3.fogout");

    // Post-fog transition: SETVAR.N FADEPAL,33 / setvar palfrom..pallen / INFOG=0 /
    // MAPCODE_JSL BG_1_4B_1 / start_65816 dotsflag+planetstars end_65816
    mb_setvarb(&b, WM_FADEPAL, 33);
    mb_setvarb(&b, WM_PALFROM, 0);
    mb_setvarb(&b, WM_PALTO, 0);
    mb_setvarb(&b, WM_PALLEN, 32);
    mb_setvarb(&b, WM_INFOG, 0);
    mb_mapcodejsl_builtin(&b, MAP_CB_BG_1_4B_1_L);
    mb_mapcode65816_inline(&b, &s_level2_3_setvar_inline_script_ptr);

    mb_pathspecial(&b, 0, -2100, -200, 3500, SH_ZACO_A, PATH_ID_EGU4, 10, 10);
    mb_pathcspecial(&b, 4000, -2300, -100, 2500, SH_ZACO_5, PATH_ID_EGU4, 10, 10);

    mb_pathcspecial(&b, 0, -150, 0, 5000, SH_HELI, PATH_ID_E_HELI, 10, 10);
    mb_pathcspecial(&b, 4200, 150, 0, 5800, SH_HELI, PATH_ID_E_HELI, 10, 10);

    mb_pathobj(&b, 0, 0, -400, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    mb_pathcspecial(&b, 0x1600, 0, -400, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);
    mb_pathobj(&b, 0, 260, -120, 3000, SH_R_BUT_2_PROXY, PATH_ID_PINITA_B, 10, 10);
    mb_pathobj(&b, 3400, -260, -120, 3000, SH_R_BUT_2_PROXY, PATH_ID_PINITA_B, 10, 10);
    mb_pathobj(&b, 0x0500, 200, -120, 3000, SH_R_BUT_2_PROXY, PATH_ID_PINITA_B, 10, 10);
    mb_mapobj(&b, 0, -200, -150, 3200, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_pathobj(&b, 2100, -200, -120, 3000, SH_R_BUT_2_PROXY, PATH_ID_PINITA_A, 10, 10);

    mb_maphardrot(&b, 0, 300, -75, 4000, SH_CLISLA_M, 0, -8, 0);
    mb_pathobj(&b, 3000, 0x0500, -75, 4000, SH_CLISLA_S, PATH_ID_L_CLISLA, 10, 10);
    mb_pathobj(&b, 0x1500, 200, -170, 4500, SH_WALK_4_0_PROXY, PATH_ID_E_KANI_0, 10, 10);
    mb_maphardrot(&b, 0, -300, -75, 4000, SH_CLISLA_M, 0, -8, 0);
    mb_pathobj(&b, 4000, -0x0500, -75, 4000, SH_CLISLA_S, PATH_ID_R_CLISLA, 10, 10);
    mb_pathobj(&b, 0x1000, -200, -170, 4500, SH_WALK_4_0_PROXY, PATH_ID_E_KANI_0, 10, 10);
    mb_pathobj(&b, 0x0400, -300, -200, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x0400, 400, -100, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);

    mb_pathobj(&b, 0x0400, -100, -30, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);
    mb_pathobj(&b, 0x0400, 200, -45, 2000, SH_CLISLA_S, PATH_ID_MINI_CLI, 10, 10);

    mb_mapobj(&b, 0x1000, -600, 0, 5000, SH_CLISLA_L, IS_HARD180YR);

    // skillfly_init + skillfly_set
    mb_skillfly_init(&b);
    mb_skillfly_set_default(&b, 0, -150, 3000);
    mb_pathobj(&b, 0x1000, 0, -120, 3000, SH_R_BUT_2_PROXY, PATH_ID_PINITA_A, 10, 10);
    mb_pathobj(&b, 0x1000, 260, -120, 3000, SH_R_BUT_2_PROXY, PATH_ID_PINITA_B, 10, 10);
    mb_skillfly_set_default(&b, -300, -120, 3000);
    mb_pathobj(&b, 0x1500, -300, -120, 3000, SH_R_BUT_2_PROXY, PATH_ID_PINITA_B, 10, 10);
    mb_pathobj(&b, 0x1500, 0x0100, -120, 3000, SH_R_BUT_2_PROXY, PATH_ID_PINITA_A, 10, 10);

    // skillfly_bonus
    mb_mapcode65816_inline(&b, &s_level2_3_skillfly_bonus_guard_script_ptr);
    mb_mapobj(&b, 0, 0x0100, -120, 1700, SH_GATE_0, IS_GATE);
    mb_label(&b, "level2_3.skillfly_bonus_0_skip");

    // misstank pair
    mb_cspecial(&b, 0, 1000, 0, 3000, SH_M_TANK, IS_MISSTANK);
    mb_setalvarb(&b, AL_ROTY, 64);  // deg90
    mb_addalvarptrw(&b, AL_WORLDX, WM_PLAYERPOSX);
    mb_cspecial(&b, 0, -1000, 0, 3000, SH_M_TANK, IS_MISSTANK);
    mb_setalvarb(&b, AL_ROTY, (uint8)(-64));  // -deg90
    mb_addalvarptrw(&b, AL_WORLDX, WM_PLAYERPOSX);

    mb_mapwait(&b, 2500);

    mb_special(&b, 0x0400, 0x0550, 0, 4000, SH_S_TANK_0, IS_TANK3);
    mb_pathcspecial(&b, 0x0400, 0, 0, 4000, SH_TANK_1, PATH_ID_E_TANK, 10, 10);
    mb_special(&b, 6000, -0x0550, 0, 4000, SH_S_TANK_0, IS_TANK3);
    mb_pathobj(&b, 0, -750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    mb_pathcspecial(&b, 4000, -720, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);
    mb_cspecial(&b, 0x0500, 300, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 4500, -300, 0, 4000, SH_HOU_5, IS_HOUDAI5F);

    mb_pathobj(&b, 18000, 0, -170, 5000, SH_NULLSHAPE, PATH_ID_KANIHAHA, 10, 10);

    mb_mapwait(&b, 10000);

    // kichi base entrance sequence
    {
        int32 kichi2_pos = BIGBASEZ + KICHI0_DOOR;  // 9360
        mb_mapobj(&b, 0, 0, 0, (int16)kichi2_pos, SH_K_DOOR, IS_KDOOR);
        kichi2_pos += KICHI2_LEN / 2;  // 9556
        mb_mapobj(&b, 0, 0, 0, (int16)kichi2_pos, SH_KICHI_3, IS_KICHI2);
        kichi2_pos += KICHI2_LEN;  // 9948
        mb_mapobj(&b, 0, 0, 0, (int16)kichi2_pos, SH_KICHI_3, IS_KICHI2);
        kichi2_pos += KICHI2_LEN;  // 10340
        mb_mapobj(&b, 0, 0, 0, (int16)kichi2_pos, SH_KICHI_3, IS_KICHI2);
        kichi2_pos += KICHI2_LEN / 2;  // 10536
        mb_mapobj(&b, 0, 0, 0, (int16)kichi2_pos, SH_K_DOOR, IS_KDOOR2);
        // kichi_0 (massivebase): placed at kichi2_pos - kichi2_len - kichi2_len/2 - medpspeed*20
        {
            int32 massive_wait = (int32)(kichi2_pos - KICHI2_LEN - KICHI2_LEN / 2 - MEDPSPEED * 20);
            mb_mapobj(&b, (uint16)massive_wait, 0, 0, (int16)BIGBASEZ, SH_KICHI_0, IS_MASSIVEBASE);
        }
    }

    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapwait(&b, (uint16)(MEDPSPEED * 20));

    // LEVEL2_3.ASM transition: setbg 2_3c / initbg / setrestart
    mb_setbg(&b, BG_2_3C);
    mb_initbg(&b);
    mb_mapcodejsl_builtin(&b, MAP_CB_SETRESTART_L);

    // ============================================================
    // MAP2_3C.ASM — Titania Part C (boss room)
    // ============================================================
    s_level2_3c_trigse_script_ptr = 0u;

    // setrestart (MAP2_3C's own restart checkpoint)
    mb_mapcodejsl_builtin(&b, MAP_CB_SETRESTART_L);
    // mapobj 0000,0000,-060,3000,boss_g_0,bossg_istrat
    mb_mapobj(&b, 0, 0, -0x60, 3000, SH_BOSS_G_0, STRAT_ADDR_BOSSG);
    // start_65816 / trigse $0b / end_65816
    mb_mapcode65816_inline(&b, &s_level2_3c_trigse_script_ptr);
    // mapwait 1
    mb_mapwait(&b, 1);
    // setbgm 5 (BGM_BOSS1)
    mb_setbgm(&b, BGM_BOSS1);
    // mapwait 5000
    mb_mapwait(&b, 5000);
    // incmap airlock1 (airlock_pos = 4000)
    {
        int32 airlock_pos = 4000;
        // mapobj 0000,0000,0000,airlock_pos,k_door,kdoor_istrat
        mb_mapobj(&b, 0, 0, 0, (int16)airlock_pos, SH_K_DOOR, IS_KDOOR);
        // airlock_pos = airlock_pos + kichi2_len/2
        airlock_pos += KICHI2_LEN / 2;
        // mapobj 0000,0000,0000,airlock_pos,kichi_3,kichi2_istrat
        mb_mapobj(&b, 0, 0, 0, (int16)airlock_pos, SH_KICHI_3, IS_KICHI2);
        // airlock_pos = airlock_pos + kichi2_len
        airlock_pos += KICHI2_LEN;
        // mapobj 0000,0000,0000,airlock_pos,kichi_3,kichi2_istrat
        mb_mapobj(&b, 0, 0, 0, (int16)airlock_pos, SH_KICHI_3, IS_KICHI2);
        // airlock_pos = airlock_pos + kichi2_len
        airlock_pos += KICHI2_LEN;
        // mapobj 0000,0000,0000,airlock_pos,kichi_3,kichi2_istrat
        mb_mapobj(&b, 0, 0, 0, (int16)airlock_pos, SH_KICHI_3, IS_KICHI2);
        // airlock_pos = airlock_pos + kichi2_len
        airlock_pos += KICHI2_LEN;
        // mapobj 0000,0000,0000,airlock_pos,kichi_3,kichi2_istrat
        mb_mapobj(&b, 0, 0, 0, (int16)airlock_pos, SH_KICHI_3, IS_KICHI2);
        // airlock_pos = airlock_pos + kichi2_len/2
        airlock_pos += KICHI2_LEN / 2;
        // mapobj 0000,0000,0000,airlock_pos,k_door,kdoor2_istrat
        mb_mapobj(&b, 0, 0, 0, (int16)airlock_pos, SH_K_DOOR, IS_KDOOR2);
        // mapwait airlock_pos - kichi2_len*2
        mb_mapwait(&b, (uint16)(airlock_pos - KICHI2_LEN * 2));
    }
    // maprts — end of MAP2_3C (inlined, so just continue)

    // LEVEL2_3.ASM transition: setbg 2_3b / mapwait kichi2_len*2 / initbg
    mb_setbg(&b, BG_2_3B);
    mb_mapwait(&b, (uint16)(KICHI2_LEN * 2));
    mb_initbg(&b);

    // ============================================================
    // MAP2_3B.ASM — Titania Part B (boss section)
    // ============================================================
    // Inlined here rather than as a subroutine; the original LEVEL2_3.ASM
    // calls map2_3a, map2_3c, then map2_3b via mapjsr.

    s_level2_3b_trigger_script_ptr = 0u;
    s_level2_3b_trigger_carryon_ptr = 0u;
    s_level2_3b_trigger_waitabit_ptr = 0u;
    s_level2_3b_seatest_script_ptr = 0u;
    s_level2_3b_seatest_loop_ptr = 0u;
    s_level2_3b_mapwaitboss_cantdie_script_ptr = 0u;
    s_level2_3b_mapwaitboss_cleanup_script_ptr = 0u;

    mb_mapwait(&b, 2000);

    // .waitabit
    mb_label(&b, "level2_3b.waitabit");
    mb_mapwait(&b, 100);

    // Inline 65816: maptrigger check
    mb_mapcode65816_inline(&b, &s_level2_3b_trigger_script_ptr);

    // setvar gsvar_byte1, 5
    mb_setvarb(&b, WM_GSVAR_BYTE1, 5);
    // 5 bossSeamon objects
    mb_mapobj(&b, 500, -200, 0, 3300, SH_SEA_0_0, STRAT_ADDR_BOSSSEAMON);
    mb_mapobj(&b, 500, 0, 0, 3000, SH_SEA_0_0, STRAT_ADDR_BOSSSEAMON);
    mb_mapobj(&b, 500, 200, 0, 3300, SH_SEA_0_0, STRAT_ADDR_BOSSSEAMON);
    mb_mapobj(&b, 500, 0x0400, 0, 3500, SH_SEA_0_0, STRAT_ADDR_BOSSSEAMON);
    mb_mapobj(&b, 500, 0x0400, 0, 3500, SH_SEA_0_0, STRAT_ADDR_BOSSSEAMON);
    // 4 torpedo spawners
    mb_mapobj(&b, 500, -0x0600, 0, 1200, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 500, 0x0600, 0, 1200, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 500, -0x0400, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);
    mb_mapobj(&b, 500, 0x0400, 0, 1000, SH_NULLSHAPE, IS_TORPEDO);

    // .seatest — wait for all seamons destroyed (gsvar_byte1 == 0)
    mb_label(&b, "level2_3b.seatest");
    mb_mapwait(&b, 1);
    mb_mapcode65816_inline(&b, &s_level2_3b_seatest_script_ptr);

    // loop back to .waitabit
    mb_mapgoto(&b, "level2_3b.waitabit");

    // .carryon — boss phase
    mb_label(&b, "level2_3b.carryon");

    // mapwaitboss nosound — no trigse, no bgm fadeout/boss music
    mb_mapwait(&b, 100);
    // chkbossdead loop
    mb_label(&b, "level2_3b.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level2_3b.bosswait.cont");
    mb_mapgoto(&b, "level2_3b.bosswait.loop");
    mb_label(&b, "level2_3b.bosswait.cont");
    // cantdie + cleanup inline blocks
    mb_mapcode65816_inline(&b, &s_level2_3b_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level2_3b_mapwaitboss_cleanup_script_ptr);

    // markboss boss23
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);

    // IFEQ 1 block is disabled (conditional assembly = false), skipped.

    mb_maprts(&b);

    // LEVEL2_3.ASM wrapper tail: mapwait 1000, mapjsr cl_bridge, mapend.
    mb_mapwait(&b, 1000);
    mb_mapjsr(&b, "cl_bridge");
    mb_mapend(&b, 1u);

    // CL_BRIDG.ASM — clear demo (bridge type) appended as subroutine.
    append_cl_bridge_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level2_3 = s_empty_level;
        return;
    }

    // Resolve label pointers for inline callbacks
    if (!mb_lookup_label(&b, "level2_3.skillfly_bonus_0_skip",
                         &s_level2_3_skillfly_bonus_skip_ptr)) {
        s_level2_3_skillfly_bonus_skip_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level2_3.fog_guard_continue",
                         &s_level2_3_fog_guard_continue_ptr)) {
        s_level2_3_fog_guard_continue_ptr = 0u;
    }
    // MAP2_3B label pointers
    if (!mb_lookup_label(&b, "level2_3b.carryon",
                         &s_level2_3b_trigger_carryon_ptr)) {
        s_level2_3b_trigger_carryon_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level2_3b.waitabit",
                         &s_level2_3b_trigger_waitabit_ptr)) {
        s_level2_3b_trigger_waitabit_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level2_3b.seatest",
                         &s_level2_3b_seatest_loop_ptr)) {
        s_level2_3b_seatest_loop_ptr = 0u;
    }

    s_level2_3.data = s_level2_3_data;
    s_level2_3.length = b.length;
}

static void register_level2_3_inline_callbacks(void) {
    // Sync player_posx into WRAM mirror before each level load.
    RAM16(WM_PLAYERPOSX) = (uint16)g_player_posx;

    // Reset ebyte3 fog latch for this level.
    g_ebyte3 = 0;

    if (s_level2_3_skillfly_bonus_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_3_skillfly_bonus_guard_script_ptr,
                                    level2_3_skillfly_bonus_guard);
    }
    if (s_level2_3_fog_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_3_fog_guard_script_ptr,
                                    level2_3_fog_guard);
    }
    if (s_level2_3_setvar_inline_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_3_setvar_inline_script_ptr,
                                    level2_3_setvar_inline);
    }
    World_RegisterNativeCallback(MAP_CB_BG_1_4B_1_L, level2_3_bg_1_4b_1);

    // MAP2_3C inline callbacks
    if (s_level2_3c_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_3c_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }

    // MAP2_3B inline callbacks
    // Reset maptrigger for this level
    g_maptrigger = 0;
    if (s_level2_3b_trigger_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_3b_trigger_script_ptr,
                                    level2_3b_trigger_check);
    }
    if (s_level2_3b_seatest_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_3b_seatest_script_ptr,
                                    level2_3b_seatest_check);
    }
    if (s_level2_3b_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_3b_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level2_3b_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level2_3b_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

// ============================================================
// MAP3_4A.ASM + MAP3_4B.ASM — Sector Z (level 3-4)
// ============================================================
// LEVEL3_4.ASM wrapper: initlevel 3_4b, mapjsr map3_4b, setbg 1_3d,
// INCMAP washmap, markboss boss34, mapjsr cl_ship3_4, mapend 1.
// MAP3_4A.ASM is a tiny sub that just does mapwait 2000 / maprts.
// MAP3_4B.ASM is the 504-line bulk of the level (ported below).
static void build_level3_4_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level3_4_data;
    b.capacity = sizeof(s_level3_4_data);
    s_level3_4_skillfly_bonus0_guard_script_ptr = 0u;
    s_level3_4_skillfly_bonus0_skip_ptr = 0u;
    s_level3_4_skillfly_bonus1_guard_script_ptr = 0u;
    s_level3_4_skillfly_bonus1_skip_ptr = 0u;
    s_level3_4_chkstratdone1_loop_ptr = 0u;
    s_level3_4_chkstratdone1_end_ptr = 0u;

    // LEVEL3_4.ASM: mapjsr map3_4b (no map3_4a in the original wrapper).
    mb_mapjsr(&b, "level3_4.map3_4b");
    // After map3_4b returns: setbg 1_3d, INCMAP washmap, markboss, cl_ship3_4
    mb_setbg(&b, BG_3_4D);
    mb_initbg(&b);
    // INCMAP washmap — Giant Washing Machine Boss (Sector Z Boss)
    // WASHMAP.ASM: setbgm 6
    mb_setbgm(&b, 6);
    mb_mapwait(&b, 300);

    // boss_8_0: main boss shell
    // mapobj 0,0,(-50<<boss8_scale)+nucleusheight,210<<boss8_scale,boss_8_0,boss8_Istrat
    mb_mapobj(&b, 0, 0, (int16)((-50 << BOSS8_SCALE) + NUCLEUSHEIGHT),
              (int16)(210 << BOSS8_SCALE), SH_BOSS_8_0_PROXY, STRAT_ADDR_BOSS8);

    // 4 nucleus launchers at various angles
    mb_mapobj(&b, 0, 0, (int16)((-50 << BOSS8_SCALE) + NUCLEUSHEIGHT),
              BOSS8_CIRC, SH_HOU_4_PROXY, STRAT_ADDR_NUCLEUSLAUNCHER);
    mb_setalvarb(&b, AL_SBYTE2, (uint8)(DEG90 + DEG22));

    mb_mapobj(&b, 0, 0, (int16)((-50 << BOSS8_SCALE) + NUCLEUSHEIGHT),
              BOSS8_CIRC, SH_HOU_4_PROXY, STRAT_ADDR_NUCLEUSLAUNCHER);
    mb_setalvarb(&b, AL_SBYTE2, (uint8)(DEG135 + DEG22));

    mb_mapobj(&b, 0, 0, (int16)((-50 << BOSS8_SCALE) + NUCLEUSHEIGHT),
              BOSS8_CIRC, SH_HOU_4_PROXY, STRAT_ADDR_NUCLEUSLAUNCHER);
    mb_setalvarb(&b, AL_SBYTE2, (uint8)(DEG270 - DEG22));

    mb_mapobj(&b, 0, 0, (int16)((-50 << BOSS8_SCALE) + NUCLEUSHEIGHT),
              BOSS8_CIRC, SH_HOU_4_PROXY, STRAT_ADDR_NUCLEUSLAUNCHER);
    mb_setalvarb(&b, AL_SBYTE2, (uint8)((uint16)0u - DEG22));

    // REPT rotnum: 8 nucleus pillars at rotsize*prot angles
    {
        uint16 prot;
        for (prot = 0; prot < ROTNUM_WASH; prot++) {
            mb_mapobj(&b, 0, 0, (int16)(0 + NUCLEUSHEIGHT),
                      BOSS8_CIRC, SH_BOSS_8_4_PROXY, STRAT_ADDR_NUCLEUSPILLAR);
            mb_setalvarb(&b, AL_SBYTE2, (uint8)(ROTSIZE_WASH * prot));
        }
    }

    // maptexitwait -300 (stub: mapwait 300)
    mb_mapwait(&b, 300);
    // initbg
    mb_initbg(&b);

    // .loop: mapif chkstagedone,.cont / mapgoto .loop
    mb_label(&b, "washmap.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKSTAGEDONE, "washmap.cont");
    mb_mapgoto(&b, "washmap.loop");

    // .cont: clear boss HP + wait
    mb_label(&b, "washmap.cont");
    mb_setvarw(&b, WM_BOSSMAXHP, 0);
    mb_mapwait(&b, 1000);

    // setbgm $f1 (fadeout)
    mb_setbgm(&b, BGM_FADEOUT);

    // mapplayermode EscapeNucleus — approximated as player outview
    mb_mapplayeroutview(&b);

    // mapwait 4360 (first arg used)
    mb_mapwait(&b, 4360);

    // mapcode_jsl clearrealobjmap_l
    mb_mapcodejsl_builtin(&b, MAP_CB_CLEARREALOBJMAP_L);
    // mapwait medpspeed
    mb_mapwait(&b, MEDPSPEED);

    // markboss boss34
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);
    // mapjsr cl_ship3_4
    mb_mapjsr(&b, "cl_ship3_4");
    mb_mapend(&b, 1u);

    // ---- MAP3_4B subroutine (504 lines) ----
    mb_label(&b, "level3_4.map3_4b");

    // Line 3: mapwait 2000
    mb_mapwait(&b, 2000);

    // Lines 4-6: szaco2_mapobj trio
    mb_szaco2_mapobj(&b, 0, 1800, 0, (int16)-100, 100);
    mb_szaco2_mapobj(&b, 400, 1800, 400, 100, 0);
    mb_szaco2_mapobj(&b, -400, 1800, -400, 100, 0);

    // Line 7: mapwait 3000
    mb_mapwait(&b, 3000);

    // Lines 9-10: swinger sharks
    mb_pathcspecial(&b, 400, -1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 4, 4);
    mb_pathcspecial(&b, 1000, -1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 4, 4);

    // Lines 11-14: space houses and zaco
    mb_cspecial(&b, 1500, 200, 0, 4000, SH_R_HOU_0, IS_SHOU0A);
    mb_special(&b, 1500, 0, -100, 4000, SH_S_HOU_0, IS_SHOU0);
    mb_cspecial(&b, 1000, -400, 200, 4000, SH_R_HOU_0, IS_SHOU0A);
    mb_pathcspecial(&b, 1000, 0, -200, 4000, SH_ZACO_8, PATH_ID_ITACHI_B, 2, 4);

    // Lines 17-18: friend pair (chase2)
    mb_pathobj(&b, 0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    mb_pathobj(&b, 1000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);

    // Lines 19-20: spacebar setup, wire mode
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_sbtype8(&b, 8, 2, -1, 0);

    // Line 21: cspecial house
    mb_cspecial(&b, 1200, -100, -200, 4000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 22-27: spacebar patterns
    mb_map_sbtypeB(&b, 6, -2, 0, 0);
    mb_map_sbtype8(&b, 4, -4, 2, 0);
    mb_map_sbtypeA(&b, 4, 2, 3, 0);
    mb_map_sbtypeC(&b, 2, 0, -3, 0);
    mb_pathcspecial(&b, 1000, -300, -100, 4000, SH_ZACO_8, PATH_ID_ITACHI_A, 2, 4);
    mb_map_sbtype13(&b, 2, -5, -1, 0);

    // Lines 28-34: skillfly init + set + spacebar
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, -10, 3000, 100);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_map_sbtypeB(&b, 0, 1, 0, 0);
    mb_map_sbtypeB(&b, 0, -1, 0, 0);
    mb_map_sbtypeC(&b, 0, 0, 1, 0);
    mb_map_sbtypeC(&b, 0, 0, -1, 0);

    // Line 36: shark
    mb_pathcspecial(&b, 3000, -1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 10, 10);

    // Line 37: skillfly_bonus (item_5)
    mb_mapcode65816_inline(&b, &s_level3_4_skillfly_bonus0_guard_script_ptr);
    mb_mapobj(&b, 0, 0, 0, 1500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level3_4.skillfly_bonus_0_skip");

    // Lines 42-48: big_missile section
    mb_mapnobj(&b, 500, -200, 0, 4000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    mb_mapobj(&b, 2000, 0, SPACE_VIEWCY, 4000, SH_BIG_M, IS_MISSPOD);
    mb_map_sbtype7(&b, 1, 5, 1, 0);
    mb_mapnobj(&b, 500, 200, 100, 4000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    mb_mapobj(&b, 2000, 200, (int16)(SPACE_VIEWCY - 100), 4000, SH_BIG_M, IS_MISSPOD);
    mb_mapobj(&b, 1000, -100, (int16)(SPACE_VIEWCY + 200), 4000, SH_BIG_M, IS_MISSPOD);
    mb_map_sbtype6(&b, 1, 4, -1, 0);

    // Lines 50-51: colony pair
    mb_mapobj(&b, 0, (int16)(-4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 200, (int16)(4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3L, IS_NOCOLL);

    // Lines 52-53: spacebar
    mb_map_sbtype13(&b, 10, -3, 0, 0);
    mb_map_sbtype6(&b, 4, -4, -1, 0);

    // Lines 55-70: windmill section
    mb_special(&b, 0, 1800, (int16)(SPACE_VIEWCY - 100), 4000, SH_ROUND_0, IS_WINDMILL);
    mb_setalvarb(&b, AL_ROTY, 64);  // deg90
    mb_setalvarb(&b, AL_VEL, 120);
    mb_mapwait(&b, 400);
    mb_setalvarb(&b, AL_ROTY, 80);
    mb_setalvarb(&b, AL_VEL, 120);
    mb_mapwait(&b, 400);
    mb_setalvarb(&b, AL_ROTY, 100);
    mb_setalvarb(&b, AL_VEL, 120);
    mb_mapwait(&b, 400);
    mb_setalvarb(&b, AL_ROTY, 120);
    mb_setalvarb(&b, AL_VEL, 100);
    mb_mapwait(&b, 400);
    mb_setalvarb(&b, AL_VEL, 0);
    mb_setalvarb(&b, AL_ROTY, DEG180);
    mb_mapwait(&b, 1500);
    mb_setalvarb(&b, AL_VEL, 100);
    mb_setalvarw(&b, AL_SWORD1, (uint16)(int16)-2);

    // Lines 73-76
    mb_map_sbtype8(&b, (uint16)(int16)(-4), 2, 0, 0);
    mb_mapwait(&b, 500);
    mb_map_sbtypeA(&b, 4, 1, 0, 0);
    mb_mapwait(&b, 1000);

    // Line 77: solid bars
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);

    // Lines 79-80: colony pair
    mb_mapobj(&b, 0, (int16)(-4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 200, (int16)(4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3L, IS_NOCOLL);

    // Line 82: wire mode
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);

    // Line 84: pathobj check
    mb_pathobj(&b, 0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);

    // Lines 86-89: rotation_bar (4x SBtype18)
    mb_map_sbtype18(&b, 4, 0, 0, 0, 0, 0);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 0, -4);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 0, 0);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 0, 4);

    // Lines 91-92: colony pair
    mb_mapobj(&b, 0, (int16)(-4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 200, (int16)(4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3L, IS_NOCOLL);

    // Lines 94-103: solid bars then wire bars
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_map_sbtype0(&b, 0, 5, 0, 0);
    mb_map_sbtype0(&b, 0, -5, 0, 0);
    mb_map_sbtype0(&b, 0, 4, 0, 0);
    mb_map_sbtype0(&b, 0, -4, 0, 0);
    mb_map_sbtype0(&b, 0, 3, 0, 0);
    mb_map_sbtype0(&b, 0, -3, 0, 0);
    mb_map_sbtype0(&b, 0, 2, 0, 0);
    mb_map_sbtype0(&b, 1, -2, 0, 0);
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);

    // Lines 105-108: rotation_bar (4x SBtype18)
    mb_map_sbtype18(&b, 4, 0, 0, 0, 0, 0);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 0, -4);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 0, 0);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 0, 4);

    // Lines 109-117: solid bars
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_map_sbtype0(&b, 0, 5, 0, 0);
    mb_map_sbtype0(&b, 0, -5, 0, 0);
    mb_map_sbtype0(&b, 0, 4, 0, 0);
    mb_map_sbtype0(&b, 0, -4, 0, 0);
    mb_map_sbtype0(&b, 0, 3, 0, 0);
    mb_map_sbtype0(&b, 0, -3, 0, 0);
    mb_map_sbtype0(&b, 0, 2, 0, 0);
    mb_map_sbtype0(&b, 1, -2, 0, 0);

    // Lines 118-124: wire rotation bars with init offsets
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 2, 0);
    mb_map_sbtypeOBJ(&b, 0, 2, 0, 0, SH_ITEM_7, IS_ITEM7);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 2, -4);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 2, 0);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 2, 4);

    // Lines 125-128: solid + colony pair
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_mapobj(&b, 0, (int16)(-4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 0, (int16)(4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3L, IS_NOCOLL);

    // Lines 130-137: solid bars
    mb_map_sbtype0(&b, 0, 5, 0, 0);
    mb_map_sbtype0(&b, 0, -5, 0, 0);
    mb_map_sbtype0(&b, 0, 4, 0, 0);
    mb_map_sbtype0(&b, 0, -4, 0, 0);
    mb_map_sbtype0(&b, 0, 3, 0, 0);
    mb_map_sbtype0(&b, 0, -3, 0, 0);
    mb_map_sbtype0(&b, 0, 2, 0, 0);
    mb_map_sbtype0(&b, 1, -2, 0, 0);

    // Lines 138-145: wire bars + special house
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 2, 0);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 2, -4);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 2, 0);
    mb_special(&b, 0, 100, -300, 4000, SH_R_HOU_0, IS_SHOU0A);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 2, 4);
    mb_map_sbtype18(&b, 4, 0, 0, 0, 2, 0);
    mb_map_sbtype10(&b, 4, 0, 0, 0);

    // Lines 146-155: solid bars + SBtype12 + gate
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_map_sbtype0(&b, 0, 5, 0, 0);
    mb_map_sbtype0(&b, 0, -5, 0, 0);
    mb_map_sbtype0(&b, 0, 4, 0, 0);
    mb_map_sbtype0(&b, 0, -4, 0, 0);
    mb_map_sbtype0(&b, 0, 3, 0, 0);
    mb_map_sbtype0(&b, 0, -3, 0, 0);
    mb_map_sbtype0(&b, 0, 2, 0, 0);
    mb_map_sbtype0(&b, 8, -2, 0, 0);
    mb_map_sbtype12(&b, 8, -2, 0, 0);

    // Line 157: gate (SBtypeOBJ with gate3_istrat = raw strat address)
    mb_map_sbtypeOBJ_nobj(&b, 8, 1, -1, 1, SH_GATE_0, STRAT_ADDR_GATE3);
    mb_pathobj(&b, 1000, 3000, 3000, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 160-161: SBtype11 + spacebarwalker
    mb_map_sbtype11(&b, 1, 0, -1, 1);
    mb_special(&b, 0, (int16)(2 * SPACEBAR_UNIT_LEN),
               (int16)(SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN)),
               (int16)(SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN)),
               SH_S_WARK_0, IS_SPACEBARWALKER);
    mb_mapwait(&b, 2000);

    // Lines 164-165: friend pair (chase3)
    mb_pathobj(&b, 0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 200, 10);
    mb_pathobj(&b, 0, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);

    // Lines 167-168: colony pair
    mb_mapobj(&b, 0, (int16)(-4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 0, (int16)(4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3L, IS_NOCOLL);

    // Line 170: itachi_a
    mb_pathcspecial(&b, 0, 400, -150, 5000, SH_ZACO_8, PATH_ID_ITACHI_A, 2, 4);

    // Lines 171-174: .sbbar1 loop (6 iterations)
    mb_label(&b, "level3_4.sbbar1");
    mb_map_sbtype8(&b, 0, -1, 0, 0);
    mb_map_sbtype8(&b, 4, 1, 0, 0);
    mb_maploop(&b, "level3_4.sbbar1", 6);

    // Lines 176-177: colony pair
    mb_mapobj(&b, 0, (int16)(-4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 0, (int16)(4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3L, IS_NOCOLL);

    // Line 179: itachi_a
    mb_pathcspecial(&b, 0, -300, -100, 4000, SH_ZACO_8, PATH_ID_ITACHI_A, 2, 4);

    // Lines 180-183: .sbbar4 loop (2 iterations)
    mb_label(&b, "level3_4.sbbar4");
    mb_map_sbtype8(&b, 0, -1, 1, 0);
    mb_map_sbtype8(&b, 4, 1, 1, 0);
    mb_maploop(&b, "level3_4.sbbar4", 2);

    // Line 184: Bwarker spacebarwalker
    mb_cspecial(&b, 0, (int16)(1 * SPACEBAR_UNIT_LEN),
                (int16)(SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN)),
                (int16)(SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN)),
                SH_BWARKER_3, IS_SPACEBARWALKER);

    // Lines 186-189: .sbbar5 loop (2 iterations)
    mb_label(&b, "level3_4.sbbar5");
    mb_map_sbtype8(&b, 0, -1, 1, 0);
    mb_map_sbtype8(&b, 4, 1, 1, 0);
    mb_maploop(&b, "level3_4.sbbar5", 2);

    // Line 190: cspecial house
    mb_cspecial(&b, 0, 300, 200, 4000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 191-195: wire bars .sbbar6 loop (2 iterations)
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_label(&b, "level3_4.sbbar6");
    mb_map_sbtype8(&b, 0, -1, 0, 0);
    mb_map_sbtype8(&b, 4, 1, 0, 0);
    mb_maploop(&b, "level3_4.sbbar6", 2);

    // Lines 197-198: colony pair
    mb_mapobj(&b, 0, (int16)(-4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 0, (int16)(4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3L, IS_NOCOLL);

    // Line 200: solid bars
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);

    // Lines 202-206: Bwarker + .sbbar7 loop (4 iterations)
    mb_cspecial(&b, 0, 0,
                (int16)(SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN)),
                (int16)(SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN)),
                SH_BWARKER_3, IS_SPACEBARWALKER);
    mb_label(&b, "level3_4.sbbar7");
    mb_map_sbtype8(&b, 0, 0, 1, 0);
    mb_map_sbtype8(&b, 4, 0, -1, 0);
    mb_maploop(&b, "level3_4.sbbar7", 4);

    // Line 207: Bwarker
    mb_cspecial(&b, 0, 0,
                (int16)(SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN)),
                (int16)(SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN)),
                SH_BWARKER_3, IS_SPACEBARWALKER);

    // Lines 209-212: .sbbar9 (no loop — just 2 bars + spacebarwalker)
    mb_map_sbtype8(&b, 0, 0, 1, 0);
    mb_map_sbtype8(&b, 4, 0, -1, 0);
    mb_special(&b, 0, 0,
               (int16)(SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN)),
               (int16)(SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN)),
               SH_S_WARK_0, IS_SPACEBARWALKER);

    // Lines 215-216: colony pair
    mb_mapobj(&b, 0, (int16)(-4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3R, IS_NOCOLL);
    mb_mapobj(&b, 0, (int16)(4 * SPACEBAR_UNIT_LEN), (int16)(SPACE_VIEWCY + 0), 5000, SH_COLONY3L, IS_NOCOLL);

    // Lines 218-221: .sbbarb loop (4 iterations)
    mb_label(&b, "level3_4.sbbarb");
    mb_map_sbtype8(&b, 0, 0, 1, 0);
    mb_map_sbtype8(&b, 4, 0, -1, 0);
    mb_maploop(&b, "level3_4.sbbarb", 4);

    // Lines 223-224: two more bars
    mb_map_sbtype8(&b, 0, 0, 1, 0);
    mb_map_sbtype8(&b, 8, 0, -1, 0);

    // Lines 226-229: skillfly init + set + house + setalvar
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, -280, 0, 3000, 150);
    mb_mapobj(&b, 0, -280, 0, 3000, SH_R_HOU_0, IS_SHOU0A);
    mb_setalvarb(&b, AL_SBYTE1, 1);

    // Lines 231-237: SBtype12, spacebarwait, houses, skillfly_set
    mb_map_sbtype12(&b, 8, -5, 0, 0);
    mb_map_spacebarwait(&b, 5);
    mb_special(&b, 0, -150, -100, 4500, SH_S_HOU_0, IS_SHOU0);
    mb_skillfly_set(&b, 200, 0, 3500, 150);
    mb_mapobj(&b, 0, 200, 0, 3500, SH_R_HOU_0, IS_SHOU0A);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_map_sbtype12(&b, 8, 0, 0, 5);

    // Line 239: mapwait 3000
    mb_mapwait(&b, 3000);

    // Line 240: skillfly_bonus (gate3)
    mb_mapcode65816_inline(&b, &s_level3_4_skillfly_bonus1_guard_script_ptr);
    mb_mapnobj(&b, 0, -50, 0, 2000, SH_GATE_0, STRAT_ADDR_GATE3);
    mb_label(&b, "level3_4.skillfly_bonus_1_skip");

    // Lines 241-245: spacebar patterns
    mb_map_sbtype14(&b, 4, 0, 0, 0);
    mb_map_sbtypeA(&b, 2, -2, 1, 5);
    mb_map_sbtype8(&b, 2, 4, 0, 5);
    mb_map_sbtypeC(&b, 2, 2, -1, 5);
    mb_map_sbtypeB(&b, 2, -1, 0, 5);

    // Line 248: cspecial house
    mb_cspecial(&b, 1000, 200, 200, 4000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 250-253: wire&solid section
    mb_map_sbtype1(&b, 1, 0, 1, 0);
    mb_special(&b, 0, (int16)(-2 * SPACEBAR_UNIT_LEN),
               (int16)(SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN)),
               (int16)(SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN)),
               SH_S_WARK_0, IS_SPACEBARWALKER);
    mb_cspecial(&b, 0, (int16)(2 * SPACEBAR_UNIT_LEN),
                (int16)(SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN)),
                (int16)(SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN)),
                SH_BWARKER_3, IS_SPACEBARWALKER);

    // Lines 254-258: more spacebar
    mb_map_sbtype3(&b, 0, 4, -1, 1);
    mb_map_sbtype10(&b, 0, -4, 1, 1);
    mb_map_sbtype5(&b, 0, 2, 1, 1);
    mb_map_sbtype5(&b, 0, -2, 1, 1);

    // Lines 259-260: SBtype1 + wire
    mb_map_sbtype1(&b, 2, -2, -1, 4);
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);

    // Lines 262-263: SBtype19
    mb_map_sbtype19(&b, 0, 1, 0, 0, 7, 0);
    mb_map_sbtype19(&b, 8, -3, 0, 0, 7, 0);

    // Lines 265-268: house + solid bars
    mb_cspecial(&b, 200, 0, 0, 4000, SH_R_HOU_0, IS_SHOU0A);
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_map_sbtypeB(&b, 0, -2, 0, 0);
    mb_map_sbtypeB(&b, 0, 2, 0, 0);

    // Lines 269-274: wire/solid alternation
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_sbtype1(&b, 0, 0, -1, 0);
    mb_map_sbtype1(&b, 2, 0, 1, 0);
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_map_sbtype5(&b, 0, 2, 1, 1);
    mb_map_sbtype5(&b, 8, -2, -1, 1);

    // Line 276: item_6
    mb_mapobj(&b, 1000, (int16)(1 * SPACEBAR_UNIT_LEN),
              (int16)(SPACE_VIEWCY + (0 * SPACEBAR_UNIT_LEN)),
              (int16)(SPACEBAR_BASE_DIST + (2 * SPACEBAR_UNIT_LEN)),
              SH_ITEM_6, IS_ITEM6);

    // Lines 278-285: repeat pattern
    mb_map_sbtypeB(&b, 0, -2, 0, 0);
    mb_map_sbtypeB(&b, 0, 2, 0, 0);
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_sbtype1(&b, 0, 0, -1, 0);
    mb_map_sbtype1(&b, 2, 0, 1, 0);
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_map_sbtype5(&b, 0, 2, 1, 1);
    mb_map_sbtype5(&b, 8, -2, -1, 1);

    // Lines 287-296: more bars
    mb_map_sbtypeB(&b, 0, -2, 1, 0);
    mb_map_sbtypeB(&b, 0, 2, 1, 0);
    mb_map_sbtype18(&b, 0, 4, 0, 0, 0, 4);
    mb_map_sbtype18(&b, 0, 4, 0, 0, 0, -4);
    mb_map_sbtype18(&b, 0, -4, 0, 0, 0, 4);
    mb_map_sbtype18(&b, 0, -4, 0, 0, 0, -4);
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_sbtype1(&b, 0, 0, 0, 0);
    mb_map_sbtype1(&b, 2, 0, 2, 0);
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_map_sbtype5(&b, 0, 2, 2, 1);
    mb_map_sbtype5(&b, 8, -2, 0, 1);

    // Lines 300-308: special house + bars
    mb_special(&b, 0, 200, -100, 4000, SH_S_HOU_0, IS_SHOU0);
    mb_map_sbtypeB(&b, 0, -1, 0, 0);
    mb_map_sbtypeB(&b, 0, 3, 0, 0);
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_sbtype1(&b, 0, 1, -1, 0);
    mb_map_sbtype1(&b, 2, 1, 1, 0);
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_map_sbtype5(&b, 0, 3, 1, 1);
    mb_map_sbtype5(&b, 8, -1, -1, 1);

    // Lines 310-319: more bars
    mb_map_sbtypeC(&b, 0, 0, -2, 0);
    mb_map_sbtypeC(&b, 0, 0, 2, 0);
    mb_map_sbtype18(&b, 0, 4, 0, 0, 0, 4);
    mb_map_sbtype18(&b, 0, 4, 0, 0, 0, -4);
    mb_map_sbtype18(&b, 0, -4, 0, 0, 0, 4);
    mb_map_sbtype18(&b, 0, -4, 0, 0, 0, -4);
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_sbtype0(&b, 0, 1, 0, 0);
    mb_map_sbtype0(&b, 4, -1, 0, 0);
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_map_sbtypeB(&b, 0, -2, 0, 0);
    mb_map_sbtypeB(&b, 0, 2, 0, 0);
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_sbtype1(&b, 0, 0, -1, 0);
    mb_map_sbtype1(&b, 6, 0, 1, 0);
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_map_sbtypeC(&b, 0, 2, -2, 0);
    mb_map_sbtypeC(&b, 0, 2, 2, 0);
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_sbtype0(&b, 0, 3, 0, 0);
    mb_map_sbtype0(&b, 0, 1, 0, 0);

    // Line 331: pathobj check
    mb_pathobj(&b, 0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);

    // Lines 333-334: gate (gate_Istrat = IS_GATE)
    mb_map_sbtypeOBJ(&b, 0, 2, 0, 0, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 1500, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 335-341: solid bars + SBtype18 quad
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);
    mb_map_sbtypeB(&b, 0, -2, 0, 0);
    mb_map_sbtypeB(&b, 0, 2, 0, 0);
    mb_map_sbtype18(&b, 0, 4, 0, 0, 0, 4);
    mb_map_sbtype18(&b, 0, 4, 0, 0, 0, -4);
    mb_map_sbtype18(&b, 0, -4, 0, 0, 0, 4);
    mb_map_sbtype18(&b, 0, -4, 0, 0, 0, -4);

    // Lines 342-344: wire bars
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_sbtype1(&b, 0, 0, -1, 0);
    mb_map_sbtype1(&b, 8, 0, 1, 0);

    // Line 346: sbtypeA
    mb_map_sbtypeA(&b, 2, -2, 1, 5);

    // Lines 351-374: horiz and vert moving (speed=30)
    {
        const int16 speed = 30;
        mb_map_sbtype17(&b, 6, 0, 12, 0, -speed, -4);
        mb_map_sbtype16(&b, 6, -12, -1, 0, speed, 3);
        mb_map_sbtype17(&b, 6, 0, 10, 0, -speed, -4);
        mb_map_sbtype16(&b, 6, -10, -1, 0, speed, 3);
        mb_map_sbtype17(&b, 6, -1, -10, 0, speed, -3);
        mb_map_sbtype16(&b, 6, 10, 1, 0, -speed, 4);

        mb_map_sbtype17(&b, 2, 0, 12, 0, -speed, -4);
        mb_map_sbtype16(&b, 2, -12, -1, 0, speed, 3);
        mb_map_sbtype17(&b, 2, 0, 10, 0, -speed, -6);
        mb_map_sbtype16(&b, 2, -10, -1, 0, speed, 5);
        mb_map_sbtype17(&b, 2, -1, -10, 0, speed, -4);
        mb_map_sbtype16(&b, 2, 10, 1, 0, -speed, 3);
        mb_map_sbtype17(&b, 2, 1, 10, 0, -speed, -2);
        mb_map_sbtype16(&b, 2, -10, 0, 0, speed, 7);
        mb_map_sbtype17(&b, 2, -2, -10, 0, speed, -6);
        mb_map_sbtype17(&b, 2, 1, 12, 0, -speed, -2);
        mb_map_sbtype16(&b, 2, -12, 0, 0, speed, 7);
        mb_map_sbtype16(&b, 2, 10, -1, 0, -speed, 4);
        mb_map_sbtype17(&b, 2, 2, 10, 0, -speed, -5);
        mb_map_sbtype16(&b, 2, -10, 0, 0, speed, 3);
        mb_map_sbtype17(&b, 2, 0, -10, 0, speed, -3);
        mb_map_sbtype16(&b, 4, 10, 1, 0, -speed, 2);
    }

    // Lines 375-380: more bars + poles
    mb_map_sbtypeB(&b, 0, 1, 0, 0);
    mb_map_sbtypeB(&b, 4, -4, 0, 0);
    mb_map_sbtype8(&b, 8, 5, 0, 0);
    mb_mapnobj(&b, 800, 0, 0, 2500, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    mb_mapnobj(&b, 800, 400, 100, 2500, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    mb_mapnobj(&b, 800, -400, -100, 2500, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);

    // Lines 382-383: friend pair (chase1)
    mb_pathobj(&b, 0, 1200, 200, 600, SH_FRIENDSHIP_4, PATH_ID_CHASE1_1, 200, 10);
    mb_pathobj(&b, 1000, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);

    // Lines 384-385: poles
    mb_mapnobj(&b, 1000, 200, 100, 3000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    mb_mapnobj(&b, 1000, -200, -200, 3000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);

    // Lines 386-392: more horiz/vert moving bars
    {
        const int16 speed = 30;
        mb_map_sbtype16(&b, 4, -12, -1, 0, speed, 5);
        mb_map_sbtype17(&b, 2, -1, -12, 0, speed, -4);
        mb_map_sbtype16(&b, 2, 10, 1, 0, -speed, 3);
        mb_map_sbtype17(&b, 4, 0, -12, 0, speed, -3);
        mb_map_sbtype16(&b, 4, 12, -1, 0, -speed, 4);
        mb_map_sbtype17(&b, 4, 0, 12, 0, -speed, -6);
        mb_map_sbtype16(&b, 4, -10, 0, 0, speed, 7);
    }

    // Lines 393-394: houses
    mb_special(&b, 1500, -200, -100, 4000, SH_S_HOU_0, IS_SHOU0);
    mb_cspecial(&b, 1500, 200, 120, 4000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 396-400: cameleons + pole
    mb_special(&b, 0, 0, -100, 800, SH_CAMELEON, IS_CAMELEON);
    mb_cspecial(&b, 0, -100, 100, 800, SH_CAMELEON, IS_CAMELEON);
    mb_special(&b, 1000, 100, 100, 800, SH_CAMELEON, IS_CAMELEON);
    mb_mapnobj(&b, 0, 0, 0, 3000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);

    // Lines 404-406: big iron flame (XPspacebar)
    mb_map_xpspacebar(&b, 1000, 0, 0, 3000, 0, 6);
    mb_map_xpspacebar(&b, 1000, -200, 0, 3000, 2, -6);
    mb_map_xpspacebar(&b, 1000, 200, 0, 3000, 4, 6);

    // Lines 409-415: spacebarC + spacebarZ
    mb_map_spacebarc(&b, 0, 0, 3000, 1, 3);
    mb_map_spacebarc(&b, 0, 0, 3000, 3, 3);
    mb_map_zspacebar(&b, -1, -1, 4);
    mb_map_zspacebar(&b, 1, -1, 4);
    mb_map_zspacebar(&b, -1, 1, 4);
    mb_map_zspacebar(&b, 1, 1, 4);

    // Line 418: mapwait 1000
    mb_mapwait(&b, 1000);

    // Lines 421-422: spacebarC + spacebarX
    mb_map_spacebarc(&b, -1, 0, 3000, 4, 3);
    mb_map_spacebarx(&b, -1, -1, 0, 2);
    mb_mapwait(&b, 1000);

    // Lines 426-427: spacebarC + spacebarX
    mb_map_spacebarc(&b, 1, 0, 3000, 2, -4);
    mb_map_spacebarx(&b, 2, -1, 0, 4);
    mb_mapwait(&b, 1000);

    // Lines 431-432: spacebarC + spacebarX
    mb_map_spacebarc(&b, 0, -2, 3000, 4, 5);
    mb_map_spacebarx(&b, 0, -3, 0, 2);
    mb_mapwait(&b, 1000);

    // Lines 436-437: spacebarC + spacebarX
    mb_map_spacebarc(&b, 0, 1, 3000, 2, -2);
    mb_map_spacebarx(&b, 0, 1, 0, 4);
    mb_mapwait(&b, 1000);

    // Lines 442-453: large bit — Zspacebar, spacebarwait, Xspacebar, Yspacebar
    mb_map_zspacebar(&b, -2, -2, 0);
    mb_map_zspacebar(&b, 2, -2, 0);
    mb_map_zspacebar(&b, -2, 2, 0);
    mb_map_zspacebar(&b, 2, 2, 0);
    mb_map_spacebarwait(&b, 2);
    mb_map_xspacebar(&b, 0, -2, 0);
    mb_map_xspacebar(&b, 0, 2, 0);
    mb_map_yspacebar(&b, -2, 0, 0);
    mb_map_yspacebar(&b, 2, 0, 0);
    mb_map_spacebarwait(&b, 2);

    // Lines 456-463: Zspacebar + SBtype18 quad
    mb_map_zspacebar(&b, -2, -2, 0);
    mb_map_zspacebar(&b, 2, -2, 0);
    mb_map_zspacebar(&b, -2, 2, 0);
    mb_map_zspacebar(&b, 2, 2, 0);
    mb_map_sbtype18(&b, 0, 4, 0, 0, 0, 4);
    mb_map_sbtype18(&b, 0, 4, 0, 0, 0, -4);
    mb_map_sbtype18(&b, 0, -4, 0, 0, 0, 4);
    mb_map_sbtype18(&b, 0, -4, 0, 0, 0, -4);

    // Lines 466-468: setbgm boss music transition
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapwait(&b, (uint16)(MEDPSPEED * 7u));
    mb_setbgm(&b, BGM_BOSS1);

    // Lines 470-475: colony boss objects
    mb_mapobj(&b, 0, 0, SPACE_VIEWCY, 4000, SH_COLONY_0, IS_COLONY0);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapobj(&b, 0, (int16)(100 << 2), SPACE_VIEWCY, 4000, SH_COLONY_1, IS_COLONY1);
    mb_mapobj(&b, 0, 0, SPACE_VIEWCY, 4000, SH_COLONY_2, IS_COLONY2);
    mb_setalvarptrw(&b, AL_PTR, WM_MAPVAR1);

    // Lines 477-479: mapwait + item_5 + setalvar
    mb_mapwait(&b, 1000);
    mb_mapobj(&b, 0, 0, SPACE_VIEWCY, 5000, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);

    // Lines 481-484: SBtype18 quad
    mb_map_sbtype18(&b, 0, 4, 0, 0, 0, 4);
    mb_map_sbtype18(&b, 0, 4, 0, 0, 0, -4);
    mb_map_sbtype18(&b, 0, -4, 0, 0, 0, 4);
    mb_map_sbtype18(&b, 0, -4, 0, 0, 0, -4);

    // Lines 485-489: .wait loop — busy-wait for boss defeat (chkstratdone1)
    mb_label(&b, "level3_4.wait");
    mb_mapwait(&b, 16);
    mb_mapcode65816_inline(&b, &s_level3_4_chkstratdone1_loop_ptr);
    mb_mapgoto(&b, "level3_4.wait");
    mb_label(&b, "level3_4.end");

    // Line 491: setbg 1_3b
    mb_setbg(&b, BG_1_3B);
    mb_initbg(&b);

    // Line 494: incmap 3-4-t — 3-4 Sector Z Base Tunnel Map (3-4-T.ASM)
    // Line 3: mapwait 1000
    mb_mapwait(&b, 1000);

    // Lines 5-8: spacebar pattern — wire mode Z bars + wait
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    mb_map_zspacebar(&b, -1, 0, 0);
    mb_map_zspacebar(&b, 1, 0, 0);
    mb_mapwait(&b, 800);

    // Lines 10-26: SY spacebar pattern
    mb_map_syspacebar(&b, 1, 0, 0);
    mb_map_syspacebar(&b, -1, 0, 0);
    mb_mapwait(&b, 500);
    mb_map_syspacebar(&b, 1, 0, 0);
    mb_map_syspacebar(&b, -1, 0, 0);
    mb_mapwait(&b, 500);
    mb_map_syspacebar(&b, 0, 0, 0);
    mb_mapwait(&b, 500);
    mb_map_syspacebar(&b, 1, 0, 0);
    mb_map_syspacebar(&b, -1, 0, 0);
    mb_mapwait(&b, 500);
    mb_map_syspacebar(&b, 1, 0, 0);
    mb_map_syspacebar(&b, -1, 0, 0);
    mb_mapwait(&b, 500);
    mb_map_syspacebar(&b, 1, 0, 0);
    mb_map_syspacebar(&b, -1, 0, 0);
    mb_mapwait(&b, 1000);

    // Line 27: SX spacebar
    mb_map_sxspacebar(&b, 0, 0, 1);

    // Line 28: special warker
    mb_special(&b, 1000, 0x0060, 0, 3500, SH_S_WARK_0, STRAT_ADDR_WARKER3);

    // Lines 29-33: .tunnel0 loop — 4 tunnel_0 objects x3 + trailing set
    mb_label(&b, "level3_4.t.tunnel0");
    mb_mapobj(&b, 0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    mb_mapobj(&b, 0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    mb_mapobj(&b, 0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    mb_mapobj(&b, 1000, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);
    mb_maploop(&b, "level3_4.t.tunnel0", 3);
    // Lines 34-37: trailing tunnel_0 set
    mb_mapobj(&b, 0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    mb_mapobj(&b, 0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    mb_mapobj(&b, 0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    mb_mapobj(&b, 400, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Line 39: mapUPDNdoor 1500,4000
    mb_mapobj(&b, 1500, 0, -60, 4000, SH_UP_DOOR_PROXY, STRAT_ADDR_UPDOOR);

    // Line 40: WALL_0 obstacle
    mb_mapobj(&b, 1000, 0, -100, 5000, SH_WALL_0_PROXY, IS_HARD180YR);

    // Lines 41-48: two sets of 4 tunnel_0 objects
    mb_mapobj(&b, 0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    mb_mapobj(&b, 0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    mb_mapobj(&b, 0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    mb_mapobj(&b, 1000, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);
    mb_mapobj(&b, 0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    mb_mapobj(&b, 0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    mb_mapobj(&b, 0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    mb_mapobj(&b, 1000, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Lines 49-50: WALL_2 pair
    mb_mapobj(&b, 0, 0x0060, -60, 4000, SH_WALL_2, IS_HARD180YR);
    mb_mapobj(&b, 1000, -0x0060, -60, 4000, SH_WALL_2, IS_HARD180YR);

    // Lines 51-54: tunnel_0 set
    mb_mapobj(&b, 0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    mb_mapobj(&b, 0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    mb_mapobj(&b, 0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    mb_mapobj(&b, 1000, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Line 56: mapLRdoor 0,4000
    mb_mapobj(&b, 0, -45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    mb_mapobj(&b, 0, 45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    // mapwait 0 (from LRdoor macro first arg)

    // Lines 57-59: warker enemies
    mb_special(&b, 300, 0, 0, 4300, SH_S_WARK_0, STRAT_ADDR_WARKER3);
    mb_mapobj(&b, 300, -70, 0, 4050, SH_WARKER_3_PROXY, STRAT_ADDR_WARKER3);
    mb_special(&b, 1000, 0x0070, 0, 4550, SH_S_WARK_0, STRAT_ADDR_WARKER3);

    // Lines 60-64: .tunnel1 loop — 4 tunnel_0 objects x5
    mb_label(&b, "level3_4.t.tunnel1");
    mb_mapobj(&b, 0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    mb_mapobj(&b, 0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    mb_mapobj(&b, 0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    mb_mapobj(&b, 1000, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);
    mb_maploop(&b, "level3_4.t.tunnel1", 5);

    // Lines 65-68: trailing tunnel_0 set (last botleft has 0 wait)
    mb_mapobj(&b, 0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    mb_mapobj(&b, 0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    mb_mapobj(&b, 0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    mb_mapobj(&b, 0, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Line 69: bou_0 wall obstacle
    mb_mapobj(&b, 1000, 0, -60, 4100, SH_BOU_0_PROXY, STRAT_ADDR_TWALL0);

    // Lines 70-73: tunnel_0 set
    mb_mapobj(&b, 0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    mb_mapobj(&b, 0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    mb_mapobj(&b, 0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    mb_mapobj(&b, 200, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Lines 75-77: three mapLRdoor calls
    // mapLRdoor 400,4000
    mb_mapobj(&b, 0, -45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    mb_mapobj(&b, 0, 45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapwait(&b, 400);
    // mapLRdoor 400,4000
    mb_mapobj(&b, 0, -45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    mb_mapobj(&b, 0, 45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapwait(&b, 400);
    // mapLRdoor 500,4000
    mb_mapobj(&b, 0, -45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    mb_mapobj(&b, 0, 45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapwait(&b, 500);

    // Lines 78-81: final tunnel_0 set
    mb_mapobj(&b, 0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    mb_mapobj(&b, 0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    mb_mapobj(&b, 0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    mb_mapobj(&b, 500, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Line 496: mapjsr Mtunnelexit
    // TODO: port Mtunnelexit (medium tunnel exit sequence) as a subroutine.
    // For now, inline a simplified version.
    mb_mapwait(&b, 100);

    // Line 497: setbgm $f1 (fade out music)
    mb_setbgm(&b, BGM_FADEOUT);

    // Line 500: maprts (end of map3_4b)
    mb_maprts(&b);

    // ---- MAP3_4C subroutine (boss wait section) ----
    mb_label(&b, "level3_4.map3_4c");
    mb_setbg(&b, BG_3_4C);
    mb_initbg(&b);
    mb_label(&b, "level3_4.map3_4c.wait");
    mb_mapwait(&b, 2000);
    mb_mapgoto(&b, "level3_4.map3_4c.wait");
    mb_maprts(&b);

    // CL_SHIP3_4 — clear-demo for colony ship levels
    append_cl_ship_submap(&b);

    // ---- Resolve ----
    mb_resolve(&b);

    if (b.failed) {
        s_level3_4 = s_empty_level;
        return;
    }

    // Look up label pointers for inline callbacks.
    if (!mb_lookup_label(&b, "level3_4.skillfly_bonus_0_skip",
                         &s_level3_4_skillfly_bonus0_skip_ptr)) {
        s_level3_4_skillfly_bonus0_skip_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level3_4.skillfly_bonus_1_skip",
                         &s_level3_4_skillfly_bonus1_skip_ptr)) {
        s_level3_4_skillfly_bonus1_skip_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level3_4.end",
                         &s_level3_4_chkstratdone1_end_ptr)) {
        s_level3_4_chkstratdone1_end_ptr = 0u;
    }

    s_level3_4.data = s_level3_4_data;
    s_level3_4.length = b.length;
}

static void register_level3_4_inline_callbacks(void) {
    if (s_level3_4_skillfly_bonus0_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_4_skillfly_bonus0_guard_script_ptr,
                                    level3_4_skillfly_bonus0_guard);
    }
    if (s_level3_4_skillfly_bonus1_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_4_skillfly_bonus1_guard_script_ptr,
                                    level3_4_skillfly_bonus1_guard);
    }
    if (s_level3_4_chkstratdone1_loop_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_4_chkstratdone1_loop_ptr,
                                    level3_4_chkstratdone1_check);
    }
}

// ============================================================
// SPECIAL.ASM / LEVEL_S.ASM — Out of This Dimension
// (MAP_ID_SPECIAL)
// ============================================================
static void build_level_special_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level_special_data;
    b.capacity = sizeof(s_level_special_data);
    s_special_boss_cleanup_script_ptr = 0u;
    s_special_theenddead_script_ptr = 0u;
    s_special_theenddead_cont_ptr = 0u;
    s_special_theend_loop_ptr = 0u;

    // LEVEL_S.ASM wrapper:
    //   initlevel special,0
    //   mapwait 100
    //   setvar dospacesc,2     — not yet implemented (background effect)
    //   setvar.W bg2Yscroll,-64 — not yet implemented (background scroll)
    //   mapjsr specialmap
    //   mapend
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);
    mb_mapwait(&b, 100);
    // TODO: setvar dospacesc,2 and setvar.W bg2Yscroll,-64 when BG engine is ported
    mb_mapjsr(&b, "special.specialmap");
    mb_mapend(&b, 1u);

    // SPECIAL.ASM — specialmap subroutine
    mb_label(&b, "special.specialmap");

    // Lines 3: mapwait 5000
    mb_mapwait(&b, 5000);

    // Lines 5-9: paper plane wave 1 + pole_0
    mb_pathobj(&b, 5000, 0, 0, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 5000, 0x0300, -0x100, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 6000, -0x200, 0x100, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 6000, 0x0200, -0x100, 4000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 10000, 0x0100, -0x400, 1500, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_mapobj(&b, 8000, 0, 0, 4000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);

    // Lines 12-24: paper plane wave 2 + poles
    mb_pathobj(&b, 5000, -0x200, 0x0200, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 5000, 0x0100, -0x100, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 5000, -0x200, 0x400, 1500, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 6000, -0x400, 0x150, 3000, SH_PAPER_3_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_mapobj(&b, 6000, 0, -0x200, 4000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    mb_mapobj(&b, 6000, -0x100, 0x100, 4000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    mb_pathobj(&b, 6000, 0x0400, 0, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 6000, 0, -0x400, 1500, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 8000, 0x0200, 0x200, 2000, SH_PAPER_3_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 1000, 0, 0x0100, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 1000, -0x300, 0x200, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 5000, -0x100, -0x400, 1000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 4000, 0, -0x400, 1500, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);

    // Lines 26-27: paper pair
    mb_pathobj(&b, 2000, -0x300, 0, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 2000, -0x300, 0x100, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);

    // Lines 29-34: mixed paper_3 / paper_1 wave
    mb_pathobj(&b, 5000, -0x200, 0x200, 4000, SH_PAPER_3_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 3000, 0, 0x200, 4000, SH_PAPER_3_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 10000, 0x200, 0x200, 4000, SH_PAPER_3_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 5000, 0x0300, 0x100, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 6000, -0x200, 0x100, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    mb_pathobj(&b, 10000, 0x0200, 0x100, 4000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);

    // Line 37-38: fadeoutbgm + setbgm 5 (boss music)
    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_BOSS1);

    // Line 40: slot machine boss pathobj
    mb_pathobj(&b, 0, 3000, 0, 4200, SH_SLOT_0_PROXY, PATH_ID_SLOTMACHINE, 10, 10);

    // Line 41: mapwaitboss 7
    // Standard mapwaitboss pattern: wait, chkbossdead loop, cleanup
    mb_mapwait(&b, 100);
    mb_label(&b, "special.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "special.bosswait.cont");
    mb_mapgoto(&b, "special.bosswait.loop");
    mb_label(&b, "special.bosswait.cont");

    // Lines 45-57: endofspecialmap — inline 65816 block
    // Clears nofire and notdie flags, hides HUD on boss death.
    mb_mapcode65816_inline(&b, &s_special_boss_cleanup_script_ptr);

    // Line 59: mapwait 2000
    mb_mapwait(&b, 2000);

    // Line 61: setvar.w hposjmp,rotate_hof
    //   — hposjmp/rotate_hof are special camera rotation vars.
    //     Not yet wired in the HD engine. Skip for now.
    // TODO: mb_setvarw(&b, WM_HPOSJMP, rotate_hof_addr);

    // Line 64: mapjsr cutcreds
    //   — cutcreds is a complex credits subroutine using textpath
    //     (text rendering along 3D paths). Not yet implemented.
    //     Emit as a stub subroutine that just returns.
    mb_mapjsr(&b, "special.cutcreds");

    // Line 66: mapwait 6000
    mb_mapwait(&b, 6000);

    // Lines 68-73: "THE END" letter objects
    mb_label(&b, "special.theend_loop");
    mb_mapobj(&b, 0, 972, -969, 1000, SH_FONT_T2_PROXY, STRAT_ADDR_THEEND_T);
    mb_mapobj(&b, 0, -1120, 1377, 1000, SH_FONT_H2_PROXY, STRAT_ADDR_THEEND_H);
    mb_mapobj(&b, 0, -1019, -1530, 1000, SH_FONT_E2_PROXY, STRAT_ADDR_THEEND_E);
    mb_mapobj(&b, 0, 1070, -1326, 1000, SH_FONT_E3_PROXY, STRAT_ADDR_THEEND_E2);
    mb_mapobj(&b, 0, 1550 + 29, 1323 + 54, 1000, SH_FONT_N2_PROXY, STRAT_ADDR_THEEND_N);
    mb_mapobj(&b, 0, -1050 + 129, 1428, 1000, SH_FONT_D2_PROXY, STRAT_ADDR_THEEND_D);

    // Lines 74-76: theenddead check loop
    mb_label(&b, "special.theenddead_check");
    mb_mapcode65816_inline(&b, &s_special_theenddead_script_ptr);
    // If theenddead false, goto .ll (theenddead_check)
    // theenddead_check callback handles the branching.

    // Lines 77-84: .cont — clear + restart THE END sequence
    mb_label(&b, "special.theenddead_cont");
    mb_mapwait(&b, 2500);
    mb_mapcodejsl_builtin(&b, MAP_CB_CLEARMAP_L);
    // stz numplasers (inline 65816)
    // setvar.b numendok,0
    mb_setvarb(&b, WM_NUMENDOK, 0);
    mb_mapwait(&b, 1000);
    mb_mapgoto(&b, "special.theend_loop");

    // maprts (end of specialmap subroutine — unreachable due to loop)
    mb_maprts(&b);

    // cutcreds stub subroutine
    mb_label(&b, "special.cutcreds");
    // cutcreds uses textpath (3D text rendering) which is not yet implemented.
    // Return immediately for now.
    mb_maprts(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level_special = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "special.theenddead_cont",
                         &s_special_theenddead_cont_ptr)) {
        s_special_theenddead_cont_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "special.theenddead_check",
                         &s_special_theend_loop_ptr)) {
        s_special_theend_loop_ptr = 0u;
    }

    s_level_special.data = s_level_special_data;
    s_level_special.length = b.length;
}

static void register_level_special_inline_callbacks(void) {
    if (s_special_boss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_special_boss_cleanup_script_ptr,
                                    special_boss_cleanup);
    }
    if (s_special_theenddead_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_special_theenddead_script_ptr,
                                    special_theenddead_check);
    }
    // Reset numendok for the level.
    g_numendok = 0;
}

// ============================================================
// TRUCKER.ASM — Mad Trucker / Galactic Rider (Venom 2 Highway)
// Inlined in MAP2_6A.ASM after the main highway section.
// ============================================================
//
// This replaces the placeholder in build_level2_6_wrapper_slice.
// The existing level2_6 build stub is left as-is; TRUCKER is
// appended as a standalone callable subroutine label that the
// level2_6 wrapper can mapjsr into once MAP2_6A is ported further.
//
// For now, build it as a standalone labeled subroutine within the
// level2_6 data buffer.

static void append_trucker_submap(MapBuilder *b) {
    s_trucker_biker_check_script_ptr = 0u;
    s_trucker_biker_loop_ptr = 0u;
    s_trucker_trigger_script_ptr = 0u;
    s_trucker_trigger_loop_ptr = 0u;
    s_trucker_rightblock_ptr = 0u;
    s_trucker_continue_ptr = 0u;

    mb_label(b, "level2_6.trucker");

    // Lines 2-3: initial biker pair
    mb_mapobj(b, 0x1000, -0x400, -60, 1000, SH_AIR_1_PROXY, STRAT_ADDR_MADBIKER);
    mb_mapobj(b, 0x1000, -0x300, -60, 0x0300, SH_AIR_1_PROXY, STRAT_ADDR_MADBIKER);

    // Lines 4-6: .mad loop — wall/boulder obstacles x6
    mb_label(b, "level2_6.trucker.mad");
    mb_mapobj(b, 0x1000, -0x050, -060, 4000, SH_WALL_4_PROXY, IS_HARD180YR);
    mb_mapobj(b, 0x1000, -0x200, -060, 4000, SH_BOU_1B_PROXY, IS_HARD180YR);
    mb_maploop(b, "level2_6.trucker.mad", 6);

    // Lines 8-9: more bikers
    mb_mapobj(b, 0, -50, -60, -0x200, SH_AIR_1_PROXY, STRAT_ADDR_MADBIKER);
    mb_mapobj(b, 0x100, 50, -10, -0x400, SH_AIR_1_PROXY, STRAT_ADDR_MADBIKER);

    // Lines 11-23: .loop — wait for all bikers destroyed
    mb_label(b, "level2_6.trucker.loop");
    mb_mapcode65816_inline(b, &s_trucker_biker_check_script_ptr);

    mb_mapwait(b, 100);
    mb_mapgoto(b, "level2_6.trucker.loop");

    // Lines 24-30: .carryon — boss entrance
    mb_label(b, "level2_6.trucker.carryon");
    mb_setbgm(b, BGM_FADEOUT);
    mb_setbgm(b, BGM_BOSS1);
    // trigse $0b (boss approach sound)
    mb_mapwait(b, 3000);

    // Line 31: boss spawn
    mb_mapobj(b, 0, -0x200, -70, -0x300, SH_BOSS_9_5_PROXY, STRAT_ADDR_MADTRUCKER);

    // Line 32: mapwait 1
    mb_mapwait(b, 1);

    // Lines 33-49: .loop2 — maptrigger check loop
    mb_label(b, "level2_6.trucker.loop2");
    mb_mapcode65816_inline(b, &s_trucker_trigger_script_ptr);

    mb_mapwait(b, 500);
    mb_mapgoto(b, "level2_6.trucker.loop2");

    // Lines 50-52: .rightblockbit — dispatch to rightblock subroutine
    // The trigger callback jumps here, which calls the rightblock subroutine.
    mb_label(b, "level2_6.trucker.rightblockbit");
    mb_mapjsr(b, "level2_6.trucker.rightblock");
    mb_mapgoto(b, "level2_6.trucker.loop2");

    // Lines 56-62: .rightblock subroutine — road obstacle spawns
    mb_label(b, "level2_6.trucker.rightblock");
    mb_mapobj(b, 0, 60, 0, 1600, SH_LINE_2_PROXY, STRAT_ADDR_ROADLINE);
    mb_mapobj(b, 0, 40, 0, 2400, SH_LINE_2_PROXY, STRAT_ADDR_ROADLINE);
    mb_mapobj(b, 0, 20, 0, 3100, SH_LINE_2_PROXY, STRAT_ADDR_ROADLINE);
    mb_mapobj(b, 0, 0, 0, 3400, SH_LINE_2_PROXY, STRAT_ADDR_ROADLINE);
    mb_mapobj(b, 0, 90, -60, 3600, SH_BOU_1B_PROXY, IS_HARD180YR);
    mb_maprts(b);

    // Lines 65-73: .continue — boss defeated
    mb_label(b, "level2_6.trucker.continue");
    // mapplayercantdie — not yet implemented as an opcode
    // TODO: mb_mapplayercantdie(b);
    // Original 65816 block: lda #0 / sta.l m_bossmaxHP
    // Use setvarw to clear boss max HP (equivalent to the inline block).
    mb_setvarw(b, WM_BOSSMAXHP, 0);
    mb_setbgm(b, BGM_FADEOUT);

    mb_maprts(b);
}

// ============================================================
// MAP1_3A1.ASM — Space Armada Part A1 (Ship 1 interior)
// ============================================================
// Extends the existing build_level1_3_opening_slice with the
// SHIP1 bounded section (LEVEL1_3.ASM lines 16-23 + MAP1_3A1).
// This is appended as a subroutine within the level1_3 data.

static void append_map1_3a1_submap(MapBuilder *b) {
    s_map1_3a1_chkstratdone1_loop_ptr = 0u;
    s_map1_3a1_chkstratdone2_restart_ptr = 0u;

    // MAP1_3A1.ASM — map1_3a1 subroutine
    mb_label(b, "level1_3.map1_3a1");

    // Line 4: mapwait 2500
    mb_mapwait(b, 2500);

    // Line 6: ship_1 with setalvar vel,roty,rotx,rotz
    mb_mapobj(b, 0, SPACE_MINX + 2000, SPACE_VIEWCY + 600, 9000,
              SH_SHIP_1_PROXY, STRAT_ADDR_SHIP1A);
    mb_setalvarb(b, AL_VEL, 60);
    mb_setalvarb(b, AL_ROTY, 115);
    mb_setalvarb(b, AL_ROTX, 250u);
    mb_setalvarb(b, AL_ROTZ, 20);

    // Lines 11-12: pathcspecial escorts
    mb_pathcspecial(b, 0x0300, SPACE_MINX + 1000, SPACE_VIEWCY + 400, 8000,
                    SH_ZACO_7, PATH_ID_PATCOM, 10, 10);
    mb_pathcspecial(b, 0, SPACE_MINX + 500, SPACE_VIEWCY + 500, 7500,
                    SH_ZACO_7, PATH_ID_PATCOM, 10, 10);

    // Line 13: mapwait 6000
    mb_mapwait(b, 6000);

    // Lines 15-17: pathspecial + pathcspecials
    mb_pathspecial(b, 0x0600, 0, -600, -100,
                   SH_S_ZACO_0, PATH_ID_PATRET_IFAL, 10, 10);
    mb_pathcspecial(b, 0x0600, -500, 100, -100,
                    SH_BZACO_8, PATH_ID_PATRET_IRAB, 10, 10);
    mb_pathcspecial(b, 2500, 500, 100, -100,
                    SH_BZACO_8, PATH_ID_PATRET_IFRO, 10, 10);

    // Lines 20-25: second ship_1 with escorts
    mb_mapobj(b, 0, SPACE_MAXX - 300, SPACE_VIEWCY + 200, 10000,
              SH_SHIP_1_PROXY, STRAT_ADDR_SHIP1A);
    mb_setalvarb(b, AL_VEL, 50);
    mb_setalvarb(b, AL_ROTY, 134);
    mb_setalvarb(b, AL_ROTZ, 250u);
    mb_pathcspecial(b, 0x0300, SPACE_MAXX, SPACE_VIEWCY + 800, 8000,
                    SH_ZACO_7, PATH_ID_PATCOM, 10, 10);
    mb_pathcspecial(b, 0, SPACE_MAXX + 200, SPACE_VIEWCY + 700, 7500,
                    SH_ZACO_7, PATH_ID_PATCOM, 10, 10);

    // Line 26: map_farships2
    mb_map_farships2(b, -500, -300, 8000, -16, -25, 2);

    // Line 27: mapwait 8000
    mb_mapwait(b, 8000);

    // Line 28: map_farships1
    mb_map_farships1(b, 0, -500, 8000, 20, -40, 1);

    // Line 29: mapcspecial (zaco_7 fly out of ship2)
    mb_cspecial(b, 0, -350, SPACE_VIEWCY - 300, 4000, SH_ZACO_7, IS_SZACO5);

    // Line 30: mapwait 1000
    mb_mapwait(b, 1000);

    // Line 31: map_farships0
    mb_map_farships0(b, 500, -1000, 6000, 30, -20, 2);

    // Lines 32-33: pathspecial + pathcspecial
    mb_pathspecial(b, 0x0500, -700, -400, -100,
                   SH_S_ZACO_0, PATH_ID_PATRET, 10, 10);
    mb_pathcspecial(b, 0x0500, -800, 200, -100,
                    SH_BZACO_8, PATH_ID_PATRET, 10, 10);

    // Line 34: mapwait 1000
    mb_mapwait(b, 1000);

    // Line 36: cspecial (zaco_7 fly out of ship2)
    mb_cspecial(b, 0, -300, SPACE_VIEWCY - 200, 3400, SH_ZACO_7, IS_SZACO5);

    // Line 37: mapwait 2000
    mb_mapwait(b, 2000);

    // Lines 41-47: totumsg + ship_3 + doors
#define SPSDIST 6000
    mb_pathobj(b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_TOTUMSG, 10, 10);
    mb_mapobj(b, 0, 0x0300, SPACE_VIEWCY - 1500, SPSDIST,
              SH_SHIP_3_PROXY, STRAT_ADDR_SHIP2);
    mb_setvarobj(b, WM_MAPVAR1);
    mb_mapobj(b, 0, 0x0300, SPACE_VIEWCY - 1500, SPSDIST,
              SH_S_DOOR_1_PROXY, STRAT_ADDR_SDOOR1);
    mb_setalvarptrw(b, AL_SWORD1, WM_MAPVAR1);
    mb_mapobj(b, 0, 0x0300, SPACE_VIEWCY - 1500, SPSDIST,
              SH_S_DOOR_2_PROXY, STRAT_ADDR_SDOOR2);
    mb_setalvarptrw(b, AL_SWORD1, WM_MAPVAR1);
#undef SPSDIST

    // Lines 50-54: .loop1 — chkstratdone1/2 check loop
    mb_label(b, "level1_3.map1_3a1.loop1");
    mb_mapif_builtin(b, MAP_CB_CHKSTRATDONE1, "level1_3.map1_3a1.cont1");
    mb_mapif_builtin(b, MAP_CB_CHKSTRATDONE2, "level1_3.map1_3a1");
    mb_mapwait(b, 1);
    mb_mapgoto(b, "level1_3.map1_3a1.loop1");

    // Line 58: .cont1 — DO TUNNEL
    mb_label(b, "level1_3.map1_3a1.cont1");

    // maprts
    mb_maprts(b);
}

// ============================================================
// MAP1_3A2.ASM — Space Armada Part A2 (Ship 2 interior)
// ============================================================
static void append_map1_3a2_submap(MapBuilder *b) {
    s_map1_3a2_chkstratdone1_loop_ptr = 0u;
    s_map1_3a2_chkstratdone2_restart_ptr = 0u;

    mb_label(b, "level1_3.map1_3a2");

    // Line 3: mapwait 2000
    mb_mapwait(b, 2000);

    // Line 4: cspecial r_hou_0
    mb_cspecial(b, 0, (int16)-250, 300, 4000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 6-7: friend + pathobj (chase3)
    mb_pathobj(b, 0, 0, 0x0400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 200, 10);
    mb_pathobj(b, 3000, 0, 0x0400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);

    // Lines 9-10: ship_5S cruiser2 with setalvar vel,roty
    mb_mapnobj(b, 0, (int16)-0x0800, SPACE_VIEWCY - 200, 4000,
               SH_SHIP_5S_PROXY, STRAT_ADDR_CRUISER2);
    mb_setalvarb(b, AL_VEL, 18);

    // Lines 12-14: ship_5S cruiser2 with roty, vel
    mb_mapnobj(b, 0, (int16)-0x1000, SPACE_VIEWCY + 400, 3000,
               SH_SHIP_5S_PROXY, STRAT_ADDR_CRUISER2);
    mb_setalvarb(b, AL_ROTY, 20);
    mb_setalvarb(b, AL_VEL, 20);

    // Lines 16-18: ship_5m cruiser2 with vel, rotx
    mb_mapnobj(b, 0, (int16)-500, SPACE_VIEWCY + 400, 2000,
               SH_SHIP_5M_PROXY, STRAT_ADDR_CRUISER2);
    mb_setalvarb(b, AL_VEL, 20);
    mb_setalvarb(b, AL_ROTX, 240u);

    // Line 19: mapwait 3000
    mb_mapwait(b, 3000);

    // Line 20: cspecial r_hou_0
    mb_cspecial(b, 0, (int16)-200, -300, 4000, SH_R_HOU_0, IS_SHOU0A);

    // Line 21: pathspecial s_zaco_0, patret
    mb_pathspecial(b, 0, 1500, -600, -100, SH_S_ZACO_0, PATH_ID_PATRET, 10, 10);

    // Lines 22-25: ship_5S cruiser2 with vel, ship_5m with vel,rotx
    mb_mapnobj(b, 0, (int16)-2500, SPACE_VIEWCY - 100, 3000,
               SH_SHIP_5S_PROXY, STRAT_ADDR_CRUISER2);
    mb_setalvarb(b, AL_VEL, 20);
    mb_mapnobj(b, 0, (int16)-700, SPACE_VIEWCY - 100, 4000,
               SH_SHIP_5M_PROXY, STRAT_ADDR_CRUISER2);
    mb_setalvarb(b, AL_VEL, 25);
    mb_setalvarb(b, AL_ROTX, 15);

    // Lines 28-29: spacepilon + r_hou_0
    mb_mapnobj(b, 3000, 0, -100, 2000, SH_SPACEPILON, STRAT_ADDR_SPACEPILON);
    mb_mapobj(b, 0, (int16)-300, 0x0300, 4000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 31-33: ship_5 cruiser2fire with vel, rotx
    mb_mapnobj(b, 0, (int16)-1800, SPACE_VIEWCY, 5500,
               SH_SHIP_5_PROXY, STRAT_ADDR_CRUISER2FIRE);
    mb_setalvarb(b, AL_VEL, 40);
    mb_setalvarb(b, AL_ROTX, 254u);

    // Line 35: gate
    mb_mapnobj(b, 2000, (int16)-150, 0, 5000, SH_GATE_0, STRAT_ADDR_GATE3);

    // Line 38: mapwait 4000
    mb_mapwait(b, 4000);

    // Line 39: cspecial zaco_7 fly out of ship2
    mb_cspecial(b, 0, 0, (int16)(SPACE_VIEWCY + 100), 4000, SH_ZACO_7, IS_SZACO5);
    mb_setalvarb(b, AL_ROTZ, 240u);

    // Line 41: mapwait 2000
    mb_mapwait(b, 2000);

    // Line 43: pathspecial s_zaco_0, patret
    mb_pathspecial(b, 0, 2500, -600, -400, SH_S_ZACO_0, PATH_ID_PATRET, 10, 10);

    // Line 46: r_hou_0
    mb_mapobj(b, 1000, (int16)-250, 0x0100, 6000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 48-55: totumsg + ship_3 + doors (spsdist=6000)
#define SPSDIST2 6000
    mb_pathobj(b, 0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_TOTUMSG, 10, 10);
    mb_mapobj(b, 0, (int16)-300, SPACE_VIEWCY - 1500, SPSDIST2,
              SH_SHIP_3_PROXY, STRAT_ADDR_SHIP2);
    mb_setvarobj(b, WM_MAPVAR1);
    mb_mapobj(b, 0, (int16)-300, SPACE_VIEWCY - 1500, SPSDIST2,
              SH_S_DOOR_1_PROXY, STRAT_ADDR_SDOOR1);
    mb_setalvarptrw(b, AL_SWORD1, WM_MAPVAR1);
    mb_mapobj(b, 0, (int16)-300, SPACE_VIEWCY - 1500, SPSDIST2,
              SH_S_DOOR_2_PROXY, STRAT_ADDR_SDOOR2);
    mb_setalvarptrw(b, AL_SWORD1, WM_MAPVAR1);
#undef SPSDIST2

    // Lines 58-62: .loop2 — chkstratdone1/2 check loop
    mb_label(b, "level1_3.map1_3a2.loop2");
    mb_mapif_builtin(b, MAP_CB_CHKSTRATDONE1, "level1_3.map1_3a2.cont2");
    mb_mapif_builtin(b, MAP_CB_CHKSTRATDONE2, "level1_3.map1_3a2");
    mb_mapwait(b, 1);
    mb_mapgoto(b, "level1_3.map1_3a2.loop2");

    // Line 69: .cont2 — DO TUNNEL
    mb_label(b, "level1_3.map1_3a2.cont2");
    mb_maprts(b);
}

// ============================================================
// MAP1_3B2.ASM — Space Armada Part B2 (Ship 2 tunnel)
// ============================================================
static void append_map1_3b2_submap(MapBuilder *b) {
    mb_label(b, "level1_3.map1_3b2");

    // incmap 1-3-t2 — tunnel data (stub/placeholder)
    mb_mapwait(b, 500);

    // mapjsr mtunnelexit — medium tunnel exit (stub)
    mb_mapwait(b, 100);

    mb_maprts(b);
}

static void register_level2_6_inline_callbacks(void) {
    // Reset maptrigger for the trucker boss section.
    g_maptrigger = 0;

    if (s_trucker_biker_check_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_trucker_biker_check_script_ptr,
                                    trucker_biker_check);
    }
    if (s_trucker_trigger_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_trucker_trigger_script_ptr,
                                    trucker_trigger_check);
    }
}

// ============================================================
// LEVEL1_4.ASM + MAP1_4.ASM — Asteroid Belt 2 (Macbeth)
// (MAP_ID_1_4)
// ============================================================
static void build_level1_4_wrapper_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level1_4_data;
    b.capacity = sizeof(s_level1_4_data);
    s_level1_4_mapwaitboss_trigse_script_ptr = 0u;
    s_level1_4_mapwaitboss_cantdie_script_ptr = 0u;
    s_level1_4_mapwaitboss_cleanup_script_ptr = 0u;

    // LEVEL1_4.ASM: initlevel 1_4,mscramwipe_circle
    // Generic level init approximation.
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);
    mb_mapwait(&b, 1);
    mb_mapcodejsl_builtin(&b, MAP_CB_SETRESTART_L);
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_ONPLANET_L);

    // LEVEL1_4.ASM:5 — mapjsr map1_4 (inlined below)
    mb_mapjsr(&b, "level1_4.map1_4");

    // LEVEL1_4.ASM:7-9 — three gro_6 ground objects flanking the exit path
    mb_mapobj(&b, 0x0000, (int16)-0x0800, 0, 0x8000u, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x1000, 0, 0x10000u, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x1200, 0, 0x12000u, SH_GRO_6_PROXY, IS_HARD180YR);

    // LEVEL1_4.ASM:10 — mapjsr cl_ground
    mb_mapjsr(&b, "cl_ground");
    // LEVEL1_4.ASM:11 — mapend
    mb_mapend(&b, 1u);

    // =================================================================
    // MAP1_4.ASM inlined — Asteroid Belt 2 map content
    // =================================================================
    mb_label(&b, "level1_4.map1_4");

    // MAP1_4.ASM:11 — setvar.n infog,1
    mb_setvarb(&b, WM_INFOG, 1);
    mb_mapwait(&b, 2000);

    // MAP1_4.ASM:13-14 — walkers
    mb_cspecial(&b, 0x0200, 0x0750, 0, 0, SH_WALKER_0, IS_WALKING);
    mb_cspecial(&b, 0x5000, 0x0450, 0, 0, SH_WALKER_0, IS_WALKING);

    // MAP1_4.ASM:17-18 — houdai (turrets)
    mb_cspecial(&b, 0x0000, 0x0650, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x3000, (int16)-0x0650, 0, 4000, SH_HOU_5, IS_HOUDAI5F);

    // MAP1_4.ASM:21-22 — tanks (path)
    mb_pathspecial(&b, 0x0000, (int16)-0x0800, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    mb_pathspecial(&b, 0x4000, 0x0800, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);

    // MAP1_4.ASM:24-25 — friend chase6
    mb_pathobj(&b, 0x0000, (int16)-0x0750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    mb_pathcspecial(&b, 0x2500, (int16)-0x0720, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);

    // MAP1_4.ASM:27-29 — more tanks
    mb_pathspecial(&b, 0x1500, 0, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    mb_pathcspecial(&b, 0x0000, (int16)-0x0450, 0, 3000, SH_TANK_1, PATH_ID_E_TANK, 10, 10);
    mb_pathcspecial(&b, 0x8000u, 0x0450, 0, 3000, SH_TANK_1, PATH_ID_E_TANK, 10, 10);

    // MAP1_4.ASM:32-57 — pillar field (r_bu_7 rocks with items)
    mb_mapobj(&b, 0x0000, 0x0250, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0150, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0100, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0400, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0200, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x0400, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    // item_5 ring
    mb_mapobj(&b, 0x0000, (int16)-0x0250, -120, 1250, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_mapobj(&b, 0x0000, (int16)-0x0100, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, 0x0200, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0000, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0300, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0300, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    // houdai in field
    mb_cspecial(&b, 0x0000, (int16)-0x0700, 0, 3000, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0x0000, 0x0400, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0100, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0200, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_cspecial(&b, 0x0000, 0x0000, 0, 2800, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0x0000, 0x0050, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    // item_7 twin laser
    mb_mapobj(&b, 0x0000, 0x0200, -120, 1250, SH_ITEM_7, IS_ITEM7);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_mapobj(&b, 0x0000, 0x0350, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0250, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x0100, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0200, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0400, -120, 1250, SH_R_BU_7, IS_HARD180YR);

    mb_mapobj(&b, 0x0000, 0x0400, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0100, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0200, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0500, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x0500, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0300, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0100, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x0100, -120, 1250, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, (int16)-0x0300, -120, 1250, SH_R_BU_7, IS_HARD180YR);

    // MAP1_4.ASM:70-86 — rock section (gro shapes with fog)
    mb_mapobj(&b, 0x0100, 0x0050, 0, 1500, SH_GRO_6_PROXY, IS_HARD180YRFOG);
    mb_pathcspecial(&b, 0x1000, (int16)-0x0050, 0, 2050, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);

    mb_mapobj(&b, 0x0000, (int16)-0x0800, 0, 2000, SH_GRO_4_PROXY, IS_HARD180YRFOG);
    mb_mapobj(&b, 0x1000, 0x0600, 0, 2000, SH_GRO_5_PROXY, IS_HARD180YRFOG);
    mb_mapobj(&b, 0x0000, (int16)-0x0600, 0, 2000, SH_GRO_4_PROXY, IS_HARD180YRFOG);
    mb_mapobj(&b, 0x0000, 0x0400, 0, 2000, SH_GRO_5_PROXY, IS_HARD180YRFOG);
    mb_pathcspecial(&b, 0x1000, 0x0350, 0, 2550, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);

    mb_mapobj(&b, 0x0000, (int16)-0x0300, 0, 2000, SH_GRO_4_PROXY, IS_HARD180YRFOG);
    mb_mapobj(&b, 0x1000, 0x0400, 0, 2000, SH_GRO_5_PROXY, IS_HARD180YRFOG);
    mb_pathcspecial(&b, 0x0000, 0, 0, 3000, SH_TANK_1, PATH_ID_E_TANK, 10, 10);
    mb_mapobj(&b, 0x0000, (int16)-0x0280, 0, 2000, SH_GRO_0_PROXY, IS_HARD180YRFOG);
    mb_mapobj(&b, 0x1000, 0x0280, 0, 2000, SH_GRO_1_PROXY, IS_HARD180YRFOG);
    mb_mapobj(&b, 0x0000, (int16)-0x0250, 0, 2000, SH_GRO_0_PROXY, IS_HARD180YRFOG);
    mb_pathspecial(&b, 0x0000, (int16)-0x0300, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    mb_mapobj(&b, 0x1000, 0x0250, 0, 2000, SH_GRO_1_PROXY, IS_HARD180YRFOG);

    // MAP1_4.ASM:88-89 — walkers
    mb_cspecial(&b, 0x0200, 0x0700, 0, 0, SH_WALKER_0, IS_WALKING);
    mb_cspecial(&b, 0x2500, 0x0400, 0, 0, SH_WALKER_0, IS_WALKING);

    // MAP1_4.ASM:92-101 — palette fade (fog transition)
    mb_setvarb(&b, WM_FADEPAL, 32);
    mb_setvarw(&b, WM_PALFROM, 64);
    mb_setvarw(&b, WM_PALTO, 1 * 32);
    mb_setvarw(&b, WM_PALLEN, 16);
    mb_mapwait(&b, 1500);
    mb_setvarb(&b, WM_FADEPAL, 32);
    mb_setvarw(&b, WM_PALFROM, 96);
    mb_setvarw(&b, WM_PALTO, 5 * 32);
    mb_setvarw(&b, WM_PALLEN, 15);

    // MAP1_4.ASM:104-111 — heli section with ground rocks
    mb_pathspecial(&b, 0x1000, 0x0450, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    mb_mapobj(&b, 0x1000, 0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_pathcspecial(&b, 0x0000, (int16)-0x0800, -170, -100, SH_HELI, PATH_ID_KAMOME, 10, 10);
    // .groloop: 2x gro_6 pair, loop 2 times
    mb_label(&b, "level1_4.groloop");
    mb_mapobj(&b, 0x1000, 0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_maploop(&b, "level1_4.groloop", 2);

    // MAP1_4.ASM:113-123 — heli + bom_wing + houdai
    mb_pathcspecial(&b, 0x0000, (int16)-0x0800, -170, -100, SH_HELI, PATH_ID_KAMOME, 10, 10);
    mb_mapobj(&b, 0x1000, 0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_cspecial(&b, 0x0800, 0, -30 , 3000, SH_BOM_WING, IS_BOMWING);

    mb_cspecial(&b, 0x0800, 0, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x0000, (int16)-0x0700, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0x1000, 0x0400, -100, 4000, SH_NULLSHAPE, IS_UP1MAN);
    mb_mapobj(&b, 0x1000, (int16)-0x1700, 0, 5000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0600, 0, 5000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_pathcspecial(&b, 0x1500, 0x0600, 0, 5300, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);

    // MAP1_4.ASM:125-132 — friend chase7 + more heli
    mb_pathobj(&b, 0x0000, 0, -400, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    mb_pathcspecial(&b, 0x1000, 0, -400, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);
    mb_pathcspecial(&b, 0x0000, (int16)-0x0800, -170, -100, SH_HELI, PATH_ID_KAMOME, 10, 10);
    mb_mapobj(&b, 0x1000, 0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_pathcspecial(&b, 0x0000, 0x0800, -170, -100, SH_HELI, PATH_ID_KAMOME, 10, 10);
    mb_mapobj(&b, 0x1000, (int16)-0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_pathcspecial(&b, 0x0000, (int16)-0x0800, -170, -100, SH_HELI, PATH_ID_KAMOME, 10, 10);
    mb_mapobj(&b, 0x1000, 0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);

    // MAP1_4.ASM:134-135 — houdai pair
    mb_cspecial(&b, 0x1000, 0x0400, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x1000, (int16)-0x0400, 0, 4000, SH_HOU_5, IS_HOUDAI5F);

    // MAP1_4.ASM:137-139 — bu_3 corridor + walker
    mb_mapobj(&b, 0x0000, 0x0400, 0, 5500, SH_BU_3, IS_HARD180YR);
    mb_pathcspecial(&b, 0x0000, 0x0450, 0, 6000, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);
    mb_mapobj(&b, 0x0000, 0x0500, 0, 6500, SH_BU_3, IS_HARD180YR);

    // MAP1_4.ASM:142-152 — base & tank section
    mb_mapobj(&b, 0x1000, 0x0700, 0, 7000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0600, 0, 7000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x1500, 0, 8000, SH_BASE_0_0_PROXY, IS_BASE0);
    mb_special(&b, 0x0000, (int16)-0x1500, 0, 8004, SH_S_TANK_0, IS_TANK1A);
    mb_setalvarb(&b, AL_SBYTE1, 50);
    mb_cspecial(&b, 0x0000, (int16)-0x1200, 0, 8004, SH_BTANK_1_PROXY, IS_TANK1A);
    mb_setalvarb(&b, AL_SBYTE1, 55);
    mb_cspecial(&b, 0x0000, (int16)-0x0900, 0, 8004, SH_BTANK_1_PROXY, IS_TANK1A);
    mb_setalvarb(&b, AL_SBYTE1, 60);
    mb_mapobj(&b, 0x3000, (int16)-0x1500, 0, 8005, SH_BASE_0_1_PROXY, IS_BASE0);
    mb_mapobj(&b, 0x7500, 0x1500, 0, 7000, SH_GRO_6_PROXY, IS_HARD180YR);

    // MAP1_4.ASM:154-156 — houdai + base
    mb_cspecial(&b, 0x1000, (int16)-0x0400, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x3500, (int16)-0x0400, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0x0000, 0x0200, 0, 5000, SH_BASE_0_0_PROXY, IS_BASE1);

    // MAP1_4.ASM:158-160 — friend chase8
    mb_pathobj(&b, 0x0000, 0x0750, -100, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE8_1, 10, 10);
    mb_pathcspecial(&b, 0x0000, 0x3800, -3600, 4260, SH_ZACO_A, PATH_ID_CHASE8_2, 10, 10);
    mb_pathcspecial(&b, 0x0000, 0x0750, -100, 0, SH_ZACO_A, PATH_ID_CHASE8_3, 10, 10);

    // MAP1_4.ASM:162-163 — gate
    mb_mapobj(&b, 0x0000, 0x0200, -100, 5500, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 0x1000, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    mb_mapwait(&b, 3000);

    // MAP1_4.ASM:167-176 — palette fade (second transition)
    mb_setvarb(&b, WM_FADEPAL, 32);
    mb_setvarw(&b, WM_PALFROM, 64);
    mb_setvarw(&b, WM_PALTO, 1 * 32);
    mb_setvarw(&b, WM_PALLEN, 16);
    mb_mapwait(&b, 1500);
    mb_setvarb(&b, WM_FADEPAL, 32);
    mb_setvarw(&b, WM_PALFROM, 96);
    mb_setvarw(&b, WM_PALTO, 5 * 32);
    mb_setvarw(&b, WM_PALLEN, 15);

    // MAP1_4.ASM:178-208 — tank2 + heli section + bases + gro_6 corridors
    mb_mapobj(&b, 0x1000, (int16)-0x1000, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_cspecial(&b, 0x1000, 0x0300, 0, 4000, SH_TANK_2_PROXY, IS_TANK2);
    mb_pathobj(&b, 0x0000, 0x0300, -600, 3000, SH_HELI, PATH_ID_KAMOME, 10, 10);
    mb_pathobj(&b, 0x0000, (int16)-0x0300, -600, 3000, SH_HELI, PATH_ID_KAMOME, 10, 10);
    mb_mapobj(&b, 0x1000, 0x1100, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x1100, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, 0x1100, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x1100, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, 0x1100, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0600, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, (int16)-0x1650, 0, 4500, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x3000, 0x1150, 0, 5000, SH_GRO_6_PROXY, IS_HARD180YR);

    mb_mapobj(&b, 0x0000, (int16)-0x0300, 0, 5000, SH_BASE_0_0_PROXY, IS_BASE1);
    mb_mapobj(&b, 0x0000, 0x0300, 0, 5000, SH_BASE_0_0_PROXY, IS_BASE1);
    mb_mapobj(&b, 0x0000, (int16)-0x0300, -50, 5300, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_mapobj(&b, 0x1300, 0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1300, (int16)-0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1300, 0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1300, 0x1000, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_pathobj(&b, 0x0000, (int16)-0x0100, -400, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    mb_pathcspecial(&b, 0x0000, (int16)-0x0100, -400, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);
    mb_cspecial(&b, 0x0000, (int16)-0x0100, 0, 4000, SH_TANK_2_PROXY, IS_TANK2);
    mb_pathobj(&b, 0x0000, 0x0300, -600, 3000, SH_HELI, PATH_ID_KAMOME, 10, 10);
    mb_pathobj(&b, 0x0000, (int16)-0x0300, -600, 2500, SH_HELI, PATH_ID_KAMOME, 10, 10);
    mb_pathobj(&b, 0x1000, 0, -600, 2000, SH_HELI, PATH_ID_KAMOME, 10, 10);
    mb_mapobj(&b, 0x1000, 0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, 0x0900, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x1000, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x6000, (int16)-0x0500, 0, 4000, SH_GRO_6_PROXY, IS_HARD180YR);

    // MAP1_4.ASM:214 — setbgm bgm_boss1
    mb_setbgm(&b, BGM_BOSS1);

    // MAP1_4.ASM:217 — boss_h_0
    mb_mapobj(&b, 0x0000, 0x2000, -600, 1000, SH_BOSS_H_0_PROXY, IS_BOSS2);

    // mapwaitboss pattern
    mb_mapcode65816_inline(&b, &s_level1_4_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level1_4.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level1_4.bosswait.cont");
    mb_mapgoto(&b, "level1_4.bosswait.loop");
    mb_label(&b, "level1_4.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level1_4_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level1_4_mapwaitboss_cleanup_script_ptr);
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);

    // MAP1_4.ASM:222-224 — markboss boss14
    mb_mapwait(&b, 1000);
    mb_maprts(&b);

    // Shared clear-demo subroutine (called via mapjsr cl_ground above)
    append_cl_ground_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level1_4 = s_empty_level;
        return;
    }

    s_level1_4.data = s_level1_4_data;
    s_level1_4.length = b.length;
}

static void register_level1_4_inline_callbacks(void) {
    if (s_level1_4_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_4_mapwaitboss_trigse_script_ptr,
                                    level1_4_mapwaitboss_trigse);
    }
    if (s_level1_4_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_4_mapwaitboss_cantdie_script_ptr,
                                    level1_4_mapwaitboss_cantdie);
    }
    if (s_level1_4_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_4_mapwaitboss_cleanup_script_ptr,
                                    level1_4_mapwaitboss_cleanup);
    }
}

// ============================================================
// MAP3_5.ASM — Venom 1 Surface (Route 3 Ground Stage)
// (MAP_ID_3_5)
// ============================================================

// Helper: emit a train truck (body) mapobj at grid position (tx,tz) facing angle ta.
// ttruck: mapobj 0000,-500+tsize*tx,0000,4096+(tz*tsize),truck,truck_Istrat
//         setalvar roty,ta
static void mb_ttruck(MapBuilder *b, int16 tx, int16 tz, uint8 ta) {
    mb_mapobj(b, 0, (int16)(-500 + TSIZE * tx), 0, (int16)(4096 + tz * TSIZE),
              SH_TRUCK, IS_TRUCK);
    mb_setalvarb(b, AL_ROTY, ta);
}

// Helper: emit a horizontal rail at grid position (tx,tz).
static void mb_thoriz(MapBuilder *b, int16 tx, int16 tz) {
    mb_mapobj(b, 0, (int16)(-500 + TSIZE * tx), 0, (int16)(4096 + tz * TSIZE),
              SH_RAIL_0, IS_NOCOLL);
}

// Helper: emit a vertical rail at grid position (tx,tz).
static void mb_tvert(MapBuilder *b, int16 tx, int16 tz) {
    mb_mapobj(b, 0, (int16)(-500 + TSIZE * tx), 0, (int16)(4096 + tz * TSIZE),
              SH_RAIL_0, IS_NOCOLL);
    mb_setalvarb(b, AL_ROTY, DEG90);
}

// Helper: emit a corner rail at grid position (tx,tz) facing angle ta, direction dir.
static void mb_tcorner(MapBuilder *b, int16 tx, int16 tz, uint8 ta, uint8 dir) {
    mb_mapobj(b, 0, (int16)(-500 + TSIZE * tx), 0, (int16)(4096 + tz * TSIZE),
              SH_RAIL_4, IS_TRACKCORNER);
    mb_setalvarb(b, AL_ROTY, ta);
    mb_setalvarb(b, AL_SBYTE1, dir);
}

static void build_level3_5_wrapper_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level3_5_data;
    b.capacity = sizeof(s_level3_5_data);
    s_level3_5_mapwaitboss_trigse_script_ptr = 0u;
    s_level3_5_mapwaitboss_cantdie_script_ptr = 0u;
    s_level3_5_mapwaitboss_cleanup_script_ptr = 0u;
    s_level3_5_skillfly_bonus_guard_script_ptr = 0u;
    s_level3_5_skillfly_bonus_skip_ptr = 0u;

    // LEVEL3_5.ASM — 3-5 Macbeth wrapper (Venom 1 Surface)
    // Generic level init for ground stage.
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);
    mb_mapwait(&b, 1);
    mb_mapcodejsl_builtin(&b, MAP_CB_SETRESTART_L);
    mb_mapcodejsl_builtin(&b, MAP_CB_SET_PLAYER_ONPLANET_L);

    // LEVEL3_5.ASM:5 — mapjsr map3_5
    mb_mapjsr(&b, "level3_5.map3_5");

    // LEVEL3_5.ASM:7-12 — ro_6 objects flanking exit path
    mb_mapobj(&b, 0x0000, 800, 0, 8000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 800, 0, 8150, SH_RO_6_PROXY, IS_HARD);
    mb_mapobj(&b, 0x0000, -800, 0, 10000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, -800, 0, 10150, SH_RO_6_PROXY, IS_HARD);
    mb_mapobj(&b, 0x0000, 800, 0, 12000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 800, 0, 12150, SH_RO_6_PROXY, IS_HARD);

    // LEVEL3_5.ASM:14-15 — mapjsr cl_under / mapend
    mb_mapjsr(&b, "cl_under");
    mb_mapend(&b, 1u);

    // === MAP3_5.ASM subroutine — Venom 1 Surface map content ===
    mb_label(&b, "level3_5.map3_5");

    // MAP3_5.ASM high = -600

    // MAP3_5.ASM:8-14 — initial rock corridor
    mb_mapobj(&b, 0x0000, (int16)-0x0600, 0, 0x0800, SH_RO_4_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, 0x0600, 0, 0x0800, SH_RO_5_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, (int16)-0x0500, 0, 0x1800, SH_RO_4_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, 0x0500, 0, 0x1800, SH_RO_5_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, (int16)-0x0400, 0, 0x2800, SH_RO_4_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, 0x0400, 0, 0x2800, SH_RO_5_PROXY, IS_ROCKHARD);

    // MAP3_5.ASM:16-20 — mixed rocks
    mb_mapobj(&b, 0x0000, (int16)-0x0400, 0, 0x3800, SH_RO_4_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, 0x0550, 0, 0x3800, SH_RO_1_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, (int16)-0x0400, 0, 0x4800, SH_RO_0_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, 0x0400, 0, 0x4800, SH_RO_5_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, (int16)-0x0900, 0, 0x5800, SH_RO_2_PROXY, IS_ROCKHARD);

    // MAP3_5.ASM:22-23 — tumble_robot: item_5
    mb_mapobj(&b, 0x0000, (int16)-0x0150, -100, 0x5000, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_mapobj(&b, 0x3000, 0x0100, 0, 0x5800, SH_RO_5_PROXY, IS_ROCKHARD);

    // MAP3_5.ASM:26-37 — more rocks + walker + houdai
    mb_mapobj(&b, 0x0000, (int16)-0x1000, 0, 0x3800, SH_RO_0_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x1000, (int16)-0x0100, 0, 0x3800, SH_RO_5_PROXY, IS_ROCKHARD);
    mb_pathcspecial(&b, 0x0000, (int16)-0x0100, 0, 3350, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);
    mb_mapobj(&b, 0x0000, (int16)-0x0800, 0, 0x3800, SH_RO_4_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x2000, (int16)-0x0500, 0, 0x4800, SH_RO_4_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, 0x0500, 0, 0x2800, SH_RO_1_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, (int16)-0x0200, 0, 0x3800, SH_RO_4_PROXY, IS_ROCKHARD);
    mb_cspecial(&b, 0x0000, 0x0200, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0x1000, 0x0600, 0, 0x3800, SH_RO_1_PROXY, IS_ROCKHARD);
    mb_pathobj(&b, 0x0500, 0x0600, 0, 3350, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);
    // ceiling column (RO_6 with deg180 z-rot)
    mb_mapobj(&b, 0x0000, 0x0450, -600, 4000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapwait(&b, 0x0500);

    // MAP3_5.ASM:39-56 — rock field section 2
    mb_mapobj(&b, 0x0000, 0, 0, 0x2800, SH_RO_4_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x1000, 0x0800, 0, 0x2800, SH_RO_1_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, 0x0100, 0, 0x2800, SH_RO_0_PROXY, IS_ROCKHARD);
    mb_pathcspecial(&b, 0x0000, 0x0100, 0, 3350, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);
    mb_mapobj(&b, 0x0000, 0x1000, 0, 0x2800, SH_RO_5_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, 0x0800, 0, 0x3800, SH_RO_5_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x2500, 0x0500, 0, 0x4800, SH_RO_5_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, (int16)-0x0500, 0, 0x2300, SH_RO_0_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, 0, 0, 0x3100, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x0800, 0, 0x3300, SH_RO_0_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, 0x0800, 0, 0x3300, SH_RO_1_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, (int16)-0x0600, 0, 0x4100, SH_RO_2_PROXY, IS_ROCKHARD);
    // skillfly_init + set
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, -280, -50, 3000, 100);
    mb_mapobj(&b, 0x0500, 0x0600, 0, 0x4100, SH_RO_3_PROXY, IS_ROCKHARD);
    mb_mapobj(&b, 0x0000, 0x0100, -600, 4000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);

    // MAP3_5.ASM:58-63 — exit_of_rocks: tanks
    mb_pathspecial(&b, 0x0000, 250, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    mb_mapobj(&b, 0x0000, (int16)-0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, 0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_pathspecial(&b, 0x0000, (int16)-0x0100, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    mb_mapobj(&b, 0x0000, (int16)-0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x1500, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);

    // MAP3_5.ASM:65-68 — friend chase6
    mb_pathobj(&b, 0x0000, (int16)-0x0750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    mb_pathobj(&b, 0x1500, (int16)-0x0750, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);
    // skillfly_bonus item_5
    mb_mapcode65816_inline(&b, &s_level3_5_skillfly_bonus_guard_script_ptr);
    mb_mapobj(&b, 0x0000, 0, -120, 1300, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level3_5.skillfly_bonus_skip");

    // MAP3_5.ASM:69-75 — more ceiling rocks
    mb_mapobj(&b, 0x0000, 0x0700, -600, 4000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x0000, (int16)-0x1200, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapwait(&b, 0x1000);
    mb_mapobj(&b, 0x0000, 0x0600, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);

    // MAP3_5.ASM:77-82 — across_robot: ceiling + walker
    mb_mapobj(&b, 0x1000, (int16)-0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0400, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x0700, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_pathobj(&b, 0x0000, (int16)-0x0400, 0, 4500, SH_WALKER_0, PATH_ID_E_WALK_1, 6, 4);
    mb_mapobj(&b, 0x2000, 0x0900, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);

    // MAP3_5.ASM:85-90 — ceiling_town: houdai pair + inverted houdai
    mb_mapobj(&b, 0x0000, (int16)-0x1200, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x2000, 0x0500, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_cspecial(&b, 0x1000, (int16)-0x0300, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_pathcspecial(&b, 0x1000, 0, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    mb_cspecial(&b, 0x2000, 0x0200, 0, 3300, SH_HOU_5, IS_HOUDAI5F);

    // MAP3_5.ASM:91-105 — train track (tstart/teast/tsouth pattern)
    // tstart 0,1,east => tx=0, tz=1, ta=dirEAST=DEG270
    {
        int16 tx = 0, tz = 1;
        uint8 ta = (uint8)DIR_EAST;
        mb_ttruck(&b, tx, tz, ta);   // tstart

        // teast: from east, go east => straight east
        mb_thoriz(&b, tx, tz); tx = (int16)(tx + 1);
        ta = (uint8)DIR_EAST;
        // teast
        mb_thoriz(&b, tx, tz); tx = (int16)(tx + 1);
        ta = (uint8)DIR_EAST;
        // tsouth: from east, turn south => right corner
        ta = (uint8)(ta - DEG90);
        mb_tcorner(&b, tx, tz, ta, 1); tz = (int16)(tz - 1);
        // tsouth straight
        mb_tvert(&b, tx, tz); tz = (int16)(tz - 1);
        ta = (uint8)DIR_SOUTH;
        // tsouth
        mb_tvert(&b, tx, tz); tz = (int16)(tz - 1);
        // tsouth
        mb_tvert(&b, tx, tz); tz = (int16)(tz - 1);
        // tanothertruck
        mb_ttruck(&b, tx, tz, ta);
        // tsouth
        mb_tvert(&b, tx, tz); tz = (int16)(tz - 1);
        // teast: from south, turn east => left corner
        ta = (uint8)(ta + DEG90);
        mb_tcorner(&b, tx, tz, ta, 0); tx = (int16)(tx + 1);
        ta = (uint8)DIR_EAST;
        // tsouth: from east, turn south => right corner
        ta = (uint8)(ta - DEG90);
        mb_tcorner(&b, tx, tz, ta, 1); tz = (int16)(tz - 1);
        ta = (uint8)DIR_SOUTH;
        // tsouth
        mb_tvert(&b, tx, tz); tz = (int16)(tz - 1);
        // tsouth
        mb_tvert(&b, tx, tz); tz = (int16)(tz - 1);
        // teast: from south, turn east => left corner
        ta = (uint8)(ta + DEG90);
        mb_tcorner(&b, tx, tz, ta, 0); tx = (int16)(tx + 1);
        ta = (uint8)DIR_EAST;
        // teast
        mb_thoriz(&b, tx, tz); tx = (int16)(tx + 1);
    }

    // MAP3_5.ASM:106-117 — ceiling buildings (bu_2 + bu_0)
    mb_mapobj(&b, 0x0000, (int16)-0x0700, -600, 4000, SH_BU_2, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x0000, 0x0700, -600, 4000, SH_BU_2, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapwait(&b, 0x1000);

    mb_mapobj(&b, 0x0000, (int16)-0x0500, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, 0x0500, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x0700, -600, 4000, SH_BU_2, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x0000, 0x0700, -600, 4000, SH_BU_2, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);

    // MAP3_5.ASM:118-123 — miss_1_1/miss_1_2 pair (missile launcher)
    mb_mapobj(&b, 0x0000, 0, -600, 3000, SH_MISS_1_1, IS_NOCOLL);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_setalvarb(&b, AL_ROTX, 127);
    mb_mapobj(&b, 0x0000, 0, -580, 3000, SH_MISS_1_2, IS_WOODS);
    mb_setalvarptrw(&b, AL_PTR, WM_MAPVAR1);
    mb_setalvarb(&b, AL_ROTX, DEG90);

    // MAP3_5.ASM:124-132 — tanks + bu_0 ceiling + bu_3 ground
    mb_pathspecial(&b, 0x0000, (int16)-0x0150, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    mb_pathspecial(&b, 0x2000, 0x0150, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    mb_mapobj(&b, 0x0000, (int16)-0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x0000, 0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_pathspecial(&b, 0x1000, 0x0150, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 10, 10);
    mb_mapobj(&b, 0x0000, (int16)-0x0800, 0, 4500, SH_BU_3, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, 0x0800, 0, 4500, SH_BU_3, IS_HARD180YR);

    // MAP3_5.ASM:134-139 — second miss_1_1/miss_1_2 pair
    mb_mapobj(&b, 0x0000, 0, -600, 3000, SH_MISS_1_1, IS_NOCOLL);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_setalvarb(&b, AL_ROTX, 127);
    mb_mapobj(&b, 0x0000, 0, -580, 3000, SH_MISS_1_2, IS_WOODS);
    mb_setalvarptrw(&b, AL_PTR, WM_MAPVAR1);
    mb_setalvarb(&b, AL_ROTX, DEG90);

    mb_mapobj(&b, 0x0000, 0x1200, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_pathcspecial(&b, 0x0000, 0, -600, 3000, SH_TANK_1, PATH_ID_E_TANK, 10, 10);
    mb_setalvarb(&b, AL_ROTZ, DEG180);

    // MAP3_5.ASM:144-151 — .ceiltown loop (3 iterations)
    mb_label(&b, "level3_5.ceiltown");
    mb_mapobj(&b, 0x0000, (int16)-0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x0000, 0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x0000, (int16)-0x0800, 0, 4500, SH_BU_3, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, 0x0800, 0, 4500, SH_BU_3, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, 0, 0, 4000, SH_BU_8, IS_HARD180YR);
    mb_maploop(&b, "level3_5.ceiltown", 3);

    // MAP3_5.ASM:153-162 — fall_walker section
    mb_mapobj(&b, 0x0000, (int16)-0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x0000, 0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x1000, (int16)-0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_cspecial(&b, 0x0000, 0x0200, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0x1000, 0x1500, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x0600, 0, 5000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, 0x0600, 0, 5000, SH_BU_8, IS_HARD180YR);
    mb_pathcspecial(&b, 0x0000, 0, 0, 5400, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);

    // MAP3_5.ASM:164-176 — twin_lazer section
    mb_mapobj(&b, 0x2000, 0, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0, -120, 3800, SH_ITEM_7, IS_ITEM7);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_mapobj(&b, 0x0000, (int16)-0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x0000, 0x0400, -600, 4000, SH_BU_0, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_pathcspecial(&b, 0x0000, 0, 0, 5400, SH_WALKER_0, PATH_ID_E_WALK_1, 10, 10);
    mb_mapobj(&b, 0x3000, 0, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x1200, -600, 7000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x0000, (int16)-0x1200, -600, 8000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);

    // MAP3_5.ASM:177-184 — .volcs0 loop (small volcanoes, 2 iterations)
    mb_label(&b, "level3_5.volcs0");
    mb_mapobj(&b, 0x0500, (int16)-0x0300, -600, 3000, SH_SVOLCANO_PROXY, IS_FIREPILLAR);
    mb_pathobj(&b, 0x0500, 0, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    mb_mapobj(&b, 0x0500, (int16)-0x0200, -600, 3000, SH_SVOLCANO_PROXY, IS_FIREPILLAR);
    mb_mapobj(&b, 0x0500, 0x0200, -600, 3000, SH_SVOLCANO_PROXY, IS_FIREPILLAR);
    mb_maploop(&b, "level3_5.volcs0", 2);
    mb_mapobj(&b, 0x2000, 0x0800, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x1000, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapwait(&b, 0x3000);

    // MAP3_5.ASM:187-189 — big_volcano
    mb_mapobj(&b, 0x0000, (int16)-0x0080, -50, 4200, SH_ITEM_6, IS_ITEM6);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_mapobj(&b, 0x3000, (int16)-0x0300, 0, 4000, SH_VOLCANO_PROXY, IS_VOLCANO);

    // MAP3_5.ASM:192-204 — missile pairs + inverted houdai
    mb_mapobj(&b, 0x0000, (int16)-0x0200, 0, 3000, SH_MISS_1_1, IS_NOCOLL);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapobj(&b, 0x0000, (int16)-0x0200, -20, 3000, SH_MISS_1_2, IS_WOODS);
    mb_setalvarptrw(&b, AL_PTR, WM_MAPVAR1);
    mb_setalvarb(&b, AL_ROTX, (uint8)(-((int8)DEG90)));
    mb_mapwait(&b, 0x1000);
    mb_mapobj(&b, 0x0000, 0, 0, 3000, SH_MISS_1_1, IS_NOCOLL);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapobj(&b, 0x0000, 0, -20, 3000, SH_MISS_1_2, IS_WOODS);
    mb_setalvarptrw(&b, AL_PTR, WM_MAPVAR1);
    mb_setalvarb(&b, AL_ROTX, (uint8)(-((int8)DEG90)));
    mb_pathcspecial(&b, 0x2000, (int16)-0x0300, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    mb_mapobj(&b, 0x1000, 0x0700, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);

    // MAP3_5.ASM:206-216 — .volcs2 loop (volcanoes + rocks, 2 iterations)
    mb_label(&b, "level3_5.volcs2");
    mb_mapobj(&b, 0x0500, (int16)-0x0400, -600, 3000, SH_SVOLCANO_PROXY, IS_FIREPILLAR);
    mb_pathobj(&b, 0x0200, 0x0200, -50, 3200, SH_BOM_WING, PATH_ID_PONPON, 2, 8);
    mb_mapobj(&b, 0x0200, 0, -600, 3000, SH_SVOLCANO_PROXY, IS_FIREPILLAR);
    mb_mapobj(&b, 0x0400, 0x1200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0500, (int16)-0x0200, -600, 3000, SH_SVOLCANO_PROXY, IS_FIREPILLAR);
    mb_mapobj(&b, 0x0200, (int16)-0x1000, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0500, 0x0400, -600, 3000, SH_SVOLCANO_PROXY, IS_FIREPILLAR);
    mb_maploop(&b, "level3_5.volcs2", 2);
    mb_mapobj(&b, 0x0000, 0x0400, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x0400, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapwait(&b, 0x1000);

    // MAP3_5.ASM:218-220 — gate
    mb_mapobj(&b, 0x0000, 0, -150, 4000, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 0x1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    mb_mapwait(&b, 0x2000);
    mb_mapobj(&b, 0x0000, 0x0800, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);

    // MAP3_5.ASM:223-241 — .woodmiss loop (missile pairs, 4 iterations)
    mb_label(&b, "level3_5.woodmiss");
    mb_mapobj(&b, 0x0000, (int16)-0x0200, 0, 3000, SH_MISS_1_1, IS_NOCOLL);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapobj(&b, 0x0000, (int16)-0x0200, -20, 3000, SH_MISS_1_2, IS_WOODS);
    mb_setalvarptrw(&b, AL_PTR, WM_MAPVAR1);
    mb_setalvarb(&b, AL_ROTX, (uint8)(-((int8)DEG90)));
    mb_mapobj(&b, 0x0000, 0x0200, 0, 3000, SH_MISS_1_1, IS_NOCOLL);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapobj(&b, 0x0000, 0x0200, -20, 3000, SH_MISS_1_2, IS_WOODS);
    mb_setalvarptrw(&b, AL_PTR, WM_MAPVAR1);
    mb_setalvarb(&b, AL_ROTX, (uint8)(-((int8)DEG90)));
    mb_mapwait(&b, 0x0800);
    mb_mapobj(&b, 0x0000, 0, -600, 3500, SH_MISS_1_1, IS_NOCOLL);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_setalvarb(&b, AL_ROTX, 127);
    mb_mapobj(&b, 0x0000, 0, -580, 3500, SH_MISS_1_2, IS_WOODS);
    mb_setalvarptrw(&b, AL_PTR, WM_MAPVAR1);
    mb_setalvarb(&b, AL_ROTX, DEG90);
    mb_mapwait(&b, 0x0800);
    mb_maploop(&b, "level3_5.woodmiss", 4);
    mb_mapobj(&b, 0x1000, 0x0700, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);

    // MAP3_5.ASM:244-256 — friend chase6 + tanks + houdai
    mb_pathobj(&b, 0x0000, (int16)-0x0750, -400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    mb_pathobj(&b, 0x0400, (int16)-0x0750, -400, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);
    mb_pathcspecial(&b, 0x0000, 0x0150, -600, 3000, SH_TANK_1, PATH_ID_E_TANK, 10, 10);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_pathcspecial(&b, 0x0000, (int16)-0x0150, -600, 3000, SH_TANK_1, PATH_ID_E_TANK, 10, 10);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x1000, (int16)-0x0800, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, 0, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x1000, (int16)-0x0800, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, 0x0600, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_cspecial(&b, 0x1000, 0, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0x1000, (int16)-0x0900, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);

    // MAP3_5.ASM:259-291 — fire_balls: houdai gauntlet + inverted cannons
    mb_mapobj(&b, 0x2000, 0x0800, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_pathcspecial(&b, 0x1000, (int16)-0x0300, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    mb_pathcspecial(&b, 0x1000, (int16)-0x0100, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    mb_mapobj(&b, 0x0000, (int16)-0x0800, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x0000, 0x0700, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_cspecial(&b, 0x1000, 0, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x1000, (int16)-0x0200, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x1000, 0x0200, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_pathcspecial(&b, 0x1000, (int16)-0x0300, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    mb_pathcspecial(&b, 0x1000, 0x0300, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    mb_cspecial(&b, 0x1000, (int16)-0x0400, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x1000, 0x0400, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x1000, 0, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x1000, (int16)-0x0500, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x1000, 0x0500, 0, 3300, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0x2000, 0, 0, 4000, SH_VOLCANO_PROXY, IS_VOLCANO);

    mb_pathcspecial(&b, 0x1000, 0, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    mb_pathcspecial(&b, 0x0000, (int16)-0x0400, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    mb_pathcspecial(&b, 0x1000, 0x0400, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    mb_pathcspecial(&b, 0x0000, (int16)-0x0200, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    mb_pathcspecial(&b, 0x0000, 0x0200, -600, 3300, SH_HOU_5, PATH_ID_E_TANK, 10, 10);
    mb_pathcspecial(&b, 0x0000, 0, -600, 3000, SH_TANK_1, PATH_ID_E_TANK, 10, 10);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x0400, (int16)-0x0500, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0800, 0x0500, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x0300, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);
    mb_mapobj(&b, 0x2000, 0x0200, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x0800, -600, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setalvarb(&b, AL_ROTZ, DEG180);

    // MAP3_5.ASM:294-308 — boss section
    // fadeoutbgm
    mb_setbgm(&b, BGM_FADEOUT);
    // MSU1 conditional fade/wait — approximated with mapwait
    mb_mapwait(&b, 2000);
    // setbgm 5
    mb_setbgm(&b, BGM_BOSS1);

    // boss_2_2 spawn (0<<boss2_scale = 0)
    mb_mapobj(&b, 0x0000, 0, 0, 4000, SH_BOSS_2_2_PROXY, IS_BOSS2);

    // mapwaitboss
    mb_mapcode65816_inline(&b, &s_level3_5_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level3_5.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level3_5.bosswait.cont");
    mb_mapgoto(&b, "level3_5.bosswait.loop");
    mb_label(&b, "level3_5.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level3_5_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level3_5_mapwaitboss_cleanup_script_ptr);

    // post-boss: rocks + markboss boss35
    mb_mapobj(&b, 0x0000, 0x1000, 0, 5000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_mapobj(&b, 0x0000, (int16)-0x1000, 0, 8000, SH_RO_6_PROXY, IS_HARD180YR);
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);
    mb_mapwait(&b, (uint16)(0x0500 + 15u * MEDPSPEED));
    mb_maprts(&b);

    // CL_UNDER.ASM — clear demo (under type) appended as subroutine.
    append_cl_under_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level3_5 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level3_5.skillfly_bonus_skip",
                         &s_level3_5_skillfly_bonus_skip_ptr)) {
        s_level3_5 = s_empty_level;
        return;
    }

    s_level3_5.data = s_level3_5_data;
    s_level3_5.length = b.length;
}

static void register_level3_5_inline_callbacks(void) {
    if (s_level3_5_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_5_mapwaitboss_trigse_script_ptr,
                                    level3_5_mapwaitboss_trigse);
    }
    if (s_level3_5_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_5_mapwaitboss_cantdie_script_ptr,
                                    level3_5_mapwaitboss_cantdie);
    }
    if (s_level3_5_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_5_mapwaitboss_cleanup_script_ptr,
                                    level3_5_mapwaitboss_cleanup);
    }
    if (s_level3_5_skillfly_bonus_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_5_skillfly_bonus_guard_script_ptr,
                                    level3_5_skillfly_bonus_guard);
    }
}

// ============================================================
// LEVEL1_5.ASM + MAP1_5.ASM — Venom 1 Orbital (Route 1 Space)
// (MAP_ID_1_5)
// ============================================================

static void build_level1_5_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level1_5_data;
    b.capacity = sizeof(s_level1_5_data);
    s_level1_5_skillfly_bonus0_guard_script_ptr = 0u;
    s_level1_5_skillfly_bonus0_skip_ptr = 0u;
    s_level1_5_skillfly_bonus1_guard_script_ptr = 0u;
    s_level1_5_skillfly_bonus1_skip_ptr = 0u;
    s_level1_5_mapwaitboss_trigse_script_ptr = 0u;
    s_level1_5_mapwaitboss_cantdie_script_ptr = 0u;
    s_level1_5_mapwaitboss_cleanup_script_ptr = 0u;

    // LEVEL1_5.ASM wrapper: initlevel 1_5,mstarwipe_circle
    // mapwait 100 / setvar dospacesc,1 / setvar.W bg2Yscroll,172
    // mapjsr map1_5 / mapjsr cl_dive / mapend__not level1_6
    mb_mapjsr(&b, "level1_5.map1_5");
    mb_mapjsr(&b, "cl_dive");
    // mapend__not level1_6: sets levelfinished=6 (next is 1_6).
    mb_mapend(&b, 6u);

    // === MAP1_5.ASM subroutine — Venom 1 Orbital space content ===
    mb_label(&b, "level1_5.map1_5");

    // Line 3: mapwait 3000
    mb_mapwait(&b, 3000);

    // Lines 15-18: pathcspecial + pathcspecial + pathcspecial + pathcspecial
    mb_pathcspecial(&b, 0, 400, 100, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    mb_pathcspecial(&b, 0, -400, 100, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    mb_pathcspecial(&b, 0, 200, 500, 900, SH_ZACO_1_PROXY, PATH_ID_E_SHAWERR, 10, 10);
    mb_pathcspecial(&b, 4500, -200, 500, 1200, SH_ZACO_1_PROXY, PATH_ID_E_SHAWERR, 10, 10);

    // Lines 20-24: friendship_4 chase2 + zaco_b chase2 + bzaco_8 patret trio
    mb_pathobj(&b, 0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    mb_pathcspecial(&b, 5000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);
    mb_pathcspecial(&b, 0, 0, 200, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    mb_pathcspecial(&b, 0, 400, -400, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    mb_pathcspecial(&b, 5000, -400, -400, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);

    // Lines 26-30: uper_missile — mapmother + cspecial loop x2
    mb_mapmother(&b, 1200, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_label(&b, "level1_5.uperloop");
    mb_cspecial(&b, 1200, 180, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1200, -180, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_maploop(&b, "level1_5.uperloop", 2);

    // Lines 32-37: check + e_shawer pair
    mb_pathobj(&b, 0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    mb_pathcspecial(&b, 0, 200, 1000, 700, SH_ZACO_1_PROXY, PATH_ID_E_SHAWERR, 10, 10);
    mb_pathspecial(&b, 1000, -200, 1000, 800, SH_ZACO_1_PROXY, PATH_ID_E_SHAWERL, 10, 10);

    // Lines 39-53: maprem mother1, big_m missile, windmill
    mb_mapremove(&b, SH_MOTHER1);
    mb_cspecial(&b, 2800, -100, (int16)(SPACE_VIEWCY - 100), 4000, SH_BIG_M, IS_MISSPOD);
    // windmill: special 0,-200,space_viewCY+1500,4000,round_0,windmill_istrat
    mb_special(&b, 0, -200, (int16)(SPACE_VIEWCY + 1500), 4000, SH_ROUND_0, IS_WINDMILL);
    mb_setalvarb(&b, AL_VEL, 120);
    mb_setalvarb(&b, AL_ROTY, 134);
    mb_setalvarb(&b, AL_ROTX, 230);
    mb_mapwait(&b, 3000);
    mb_setalvarb(&b, AL_VEL, 0);
    mb_setalvarb(&b, AL_ROTX, 0);
    mb_setalvarb(&b, AL_ROTY, 127);
    mb_mapwait(&b, 2000);
    mb_setalvarb(&b, AL_VEL, 120);
    mb_setalvarw(&b, AL_SWORD1, (uint16)(int16)-2);

    // Lines 54-55: check + warp
    mb_pathobj(&b, 0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    mb_cspecial(&b, 0, 100, 0, 1500, SH_WARP_PROXY, STRAT_ADDR_WARP);

    // Lines 56-80: second windmill with rotation sequence
    mb_mapwait(&b, 2000);
    mb_special(&b, 0, 500, (int16)(SPACE_VIEWCY - 300), 1000, SH_ROUND_0, IS_WINDMILL);
    mb_setalvarb(&b, AL_VEL, 127);
    mb_setalvarb(&b, AL_ROTY, 64);
    mb_setalvarb(&b, AL_ROTX, 5);
    mb_mapwait(&b, 300);
    mb_setalvarb(&b, AL_ROTY, 40);
    mb_mapwait(&b, 200);
    mb_setalvarb(&b, AL_ROTY, 20);
    mb_mapwait(&b, 200);
    mb_setalvarb(&b, AL_ROTY, 0);
    mb_mapwait(&b, 1400);
    mb_setalvarb(&b, AL_ROTY, 250);
    mb_mapwait(&b, 100);
    mb_setalvarb(&b, AL_ROTY, 230);
    mb_mapwait(&b, 100);
    mb_setalvarb(&b, AL_ROTY, 200);
    mb_mapwait(&b, 100);
    mb_setalvarb(&b, AL_ROTY, 170);
    mb_mapwait(&b, 100);
    mb_setalvarb(&b, AL_ROTY, 140);
    mb_mapwait(&b, 100);
    mb_setalvarb(&b, AL_VEL, 0);
    mb_setalvarb(&b, AL_ROTX, 0);
    mb_mapwait(&b, 3000);
    mb_setalvarb(&b, AL_VEL, 100);
    mb_setalvarw(&b, AL_SWORD1, 4);

    // Lines 84-91: big_missile section
    mb_pathobj(&b, 0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    mb_cspecial(&b, 2000, 0, SPACE_VIEWCY, 4000, SH_BIG_M, IS_MISSPOD);
    mb_pathcspecial(&b, 0, 0, -300, -100, SH_BZACO_8, PATH_ID_PATRET_IFAL, 10, 10);
    mb_cspecial(&b, 2000, 100, (int16)(SPACE_VIEWCY - 100), 4000, SH_BIG_M, IS_MISSPOD);
    mb_cspecial(&b, 1000, -100, (int16)(SPACE_VIEWCY + 100), 4000, SH_BIG_M, IS_MISSPOD);
    mb_cspecial(&b, 1000, -100, (int16)(SPACE_VIEWCY - 100), 4000, SH_BIG_M, IS_MISSPOD);
    mb_cspecial(&b, 1000, 100, (int16)(SPACE_VIEWCY + 100), 4000, SH_BIG_M, IS_MISSPOD);

    // Lines 92-94: warp
    mb_pathcspecial(&b, 0, 0, -300, -100, SH_BZACO_8, PATH_ID_PATRET_IRAB, 10, 10);
    mb_cspecial(&b, 3000, 0, -200, 1500, SH_WARP_PROXY, STRAT_ADDR_WARP);

    // Lines 96-112: skillfly ring section
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, 0, 4000, 100);
    mb_mapobj(&b, 0, -150, 0, 4000, SH_MINE_0_PROXY, IS_MINE0);
    mb_mapobj(&b, 0, 150, 0, 4000, SH_MINE_0_PROXY, IS_MINE0);
    mb_mapobj(&b, 0, -100, 100, 4000, SH_MINE_0_PROXY, IS_MINE0);
    mb_mapobj(&b, 0, -100, -100, 4000, SH_MINE_0_PROXY, IS_MINE0);
    mb_mapobj(&b, 0, 100, 100, 4000, SH_MINE_0_PROXY, IS_MINE0);
    mb_mapobj(&b, 0, 100, -100, 4000, SH_MINE_0_PROXY, IS_MINE0);
    mb_mapobj(&b, 0, 0, 150, 4000, SH_MINE_0_PROXY, IS_MINE0);
    mb_mapobj(&b, 4000, 0, -150, 4000, SH_MINE_0_PROXY, IS_MINE0);
    // skillfly_bonus item_5
    mb_mapcode65816_inline(&b, &s_level1_5_skillfly_bonus0_guard_script_ptr);
    mb_mapobj(&b, 0, 0, 0, 1500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level1_5.skillfly_bonus_0_skip");
    mb_skillfly_set_default(&b, 0, 0, 1500);
    mb_mapwait(&b, 1500);
    // skillfly_bonus item_5 (second)
    mb_mapcode65816_inline(&b, &s_level1_5_skillfly_bonus1_guard_script_ptr);
    mb_mapobj(&b, 0, 0, 0, 1500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level1_5.skillfly_bonus_1_skip");
    mb_mapwait(&b, 1000);

    // Lines 115-117: bazooka + winglazerman
    mb_cspecial(&b, 5000, 0, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAR);
    mb_cspecial(&b, 1000, -400, SPACE_VIEWCY, 3000, SH_W_L, IS_WINGLAZERMAN);

    // Lines 119-130: mapmother + chase3 + uper_m group + maprem
    mb_mapmother(&b, 2000, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_pathobj(&b, 0, 0, 500, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 10, 10);
    mb_pathcspecial(&b, 6000, 0, 500, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);
    mb_cspecial(&b, 1200, 0, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1200, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1200, -100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_mapremove(&b, SH_MOTHER1);

    // Lines 132-135: gate_0 + e_gate path
    mb_mapobj(&b, 500, 100, (int16)(SPACE_VIEWCY + 100), 3000, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 2500, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    mb_mapwait(&b, 1000);

    // Lines 137-147: warp + uper_m series
    mb_cspecial(&b, 3000, 0, 0, 1500, SH_WARP_PROXY, STRAT_ADDR_WARP);
    mb_cspecial(&b, 0, 0, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_special(&b, 2000, 0, -200, 1500, SH_WARP_PROXY, STRAT_ADDR_WARP);
    mb_cspecial(&b, 0, 0, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1000, 0, 0, 1500, SH_WARP_PROXY, STRAT_ADDR_WARP);
    mb_cspecial(&b, 1300, -100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1300, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1300, 200, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1300, -300, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1300, 300, 2000, 3000, SH_UPER_M, IS_UPERM);

    // Lines 152-159: chase1 + uper_m series + mapmother + supply_bird
    mb_pathobj(&b, 0, 1200, 200, 600, SH_FRIENDSHIP_4, PATH_ID_CHASE1_1, 10, 10);
    mb_pathcspecial(&b, 1000, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);
    mb_cspecial(&b, 1300, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1300, -200, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1300, 200, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_mapmother(&b, 2500, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    // supply_bird
    mb_pathobj(&b, 5000, -350, -400, 0, SH_MY_BIRD, PATH_ID_MY_BIRD, 10, 10);

    // Lines 161-168: maprem + bazooka + bzaco_8 patret pair
    mb_mapremove(&b, SH_MOTHER1);
    mb_cspecial(&b, 1000, 0, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAL);
    mb_pathcspecial(&b, 1500, 400, -100, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    mb_pathcspecial(&b, 3000, -400, -100, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);

    // Lines 169-179: boss section
    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_BOSS1);
    mb_mapobj(&b, 0, 0, 1000, 3500, SH_BOSS_B_1_PROXY, STRAT_ADDR_BOSSB);

    // mapwaitboss / markboss boss15
    mb_mapcode65816_inline(&b, &s_level1_5_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level1_5.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level1_5.bosswait.cont");
    mb_mapgoto(&b, "level1_5.bosswait.loop");
    mb_label(&b, "level1_5.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level1_5_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level1_5_mapwaitboss_cleanup_script_ptr);

    // markboss boss15
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);
    mb_mapwait(&b, 3000);
    mb_maprts(&b);

    // CL_DIVE.ASM — clear demo (dive type) appended as subroutine.
    append_cl_dive_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level1_5 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level1_5.skillfly_bonus_0_skip",
                         &s_level1_5_skillfly_bonus0_skip_ptr)) {
        s_level1_5_skillfly_bonus0_skip_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level1_5.skillfly_bonus_1_skip",
                         &s_level1_5_skillfly_bonus1_skip_ptr)) {
        s_level1_5_skillfly_bonus1_skip_ptr = 0u;
    }

    s_level1_5.data = s_level1_5_data;
    s_level1_5.length = b.length;
}

static void register_level1_5_inline_callbacks(void) {
    if (s_level1_5_skillfly_bonus0_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_5_skillfly_bonus0_guard_script_ptr,
                                    level1_5_skillfly_bonus0_guard);
    }
    if (s_level1_5_skillfly_bonus1_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_5_skillfly_bonus1_guard_script_ptr,
                                    level1_5_skillfly_bonus1_guard);
    }
    if (s_level1_5_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_5_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level1_5_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_5_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level1_5_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_5_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

// ============================================================
// MAP3_6.ASM — Venom 2 Space (Route 3 Orbital)
// (MAP_ID_3_6)
// ============================================================

static void build_level3_6_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level3_6_data;
    b.capacity = sizeof(s_level3_6_data);
    s_level3_6_skillfly_bonus0_guard_script_ptr = 0u;
    s_level3_6_skillfly_bonus0_skip_ptr = 0u;
    s_level3_6_skillfly_bonus1_guard_script_ptr = 0u;
    s_level3_6_skillfly_bonus1_skip_ptr = 0u;
    s_level3_6_noctrl_wait_script_ptr = 0u;
    s_level3_6_noctrl_wait_boss_ptr = 0u;
    s_level3_6_hpcheck_wait_script_ptr = 0u;
    s_level3_6_hpcheck_wait_owait_ptr = 0u;
    s_level3_6_flymode_check_script_ptr = 0u;
    s_level3_6_flymode_check_cont2_ptr = 0u;
    s_level3_6_mapwaitboss_trigse_script_ptr = 0u;
    s_level3_6_mapwaitboss_cantdie_script_ptr = 0u;
    s_level3_6_mapwaitboss_cleanup_script_ptr = 0u;

    // MAP3_6.ASM wrapper: space level, mapjsr map3_6, mapjsr cl_dive, mapend.
    mb_mapjsr(&b, "level3_6.map3_6");
    mb_mapjsr(&b, "cl_dive");
    // mapend (level3_6 transitions to level3_7 = Venom surface).
    mb_mapend(&b, 1u);

    // === MAP3_6.ASM subroutine — Venom 2 Space content ===
    mb_label(&b, "level3_6.map3_6");

    // Line 3: mapwait 2000
    mb_mapwait(&b, 2000);

    // Lines 6-10: big_missile group
    mb_cspecial(&b, 2000, 0, SPACE_VIEWCY, 4000, SH_BIG_M, IS_MISSPOD);
    mb_cspecial(&b, 2000, 100, (int16)(SPACE_VIEWCY - 100), 4000, SH_BIG_M, IS_MISSPOD);
    mb_cspecial(&b, 3000, -100, (int16)(SPACE_VIEWCY + 100), 4000, SH_BIG_M, IS_MISSPOD);
    mb_cspecial(&b, 2000, -100, (int16)(SPACE_VIEWCY - 100), 4000, SH_BIG_M, IS_MISSPOD);
    mb_cspecial(&b, 2000, 100, (int16)(SPACE_VIEWCY + 100), 4000, SH_BIG_M, IS_MISSPOD);

    // Lines 13-20: zacos — mapmother mine2 + shark call_fol paths + bzaco_8 + maprem
    mb_mapmother(&b, 6000, 0, (int16)(1035 + SPACE_VIEWCY), 1800, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_pathcspecial(&b, 2000, 1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 10, 10);
    mb_pathcspecial(&b, 2000, -1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 10, 10);
    mb_pathcspecial(&b, 2000, 1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 10, 10);
    mb_pathcspecial(&b, 2000, -1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 10, 10);
    mb_pathcspecial(&b, 4000, -700, 100, -100, SH_BZACO_8, PATH_ID_PATRET_IFAL, 10, 10);
    mb_mapremove(&b, SH_MOTHER1);

    // Line 22: mapwait 2000
    mb_mapwait(&b, 2000);

    // Lines 25-31: M formation (szaco2_mapobj)
    mb_szaco2_mapobj(&b, 0, 2000, 0, 0, 100);
    mb_szaco2_mapobj(&b, -500, 1000, -300, 100, 0);
    mb_szaco2_mapobj(&b, 500, 1000, 300, 100, 100);
    mb_szaco2_mapobj(&b, -1000, 1000, -400, -100, 0);
    mb_szaco2_mapobj(&b, 1000, 1000, 400, -100, 100);

    // Lines 33-43: mapmother + friend chase3 + spacepilon + uper_m group + maprem
    mb_mapwait(&b, 2000);
    mb_mapmother(&b, 9000, 0, (int16)(1035 + SPACE_VIEWCY), 1800, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_pathobj(&b, 0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 200, 10);
    mb_pathobj(&b, 2000, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);
    mb_mapobj(&b, 2000, 200, -200, 2000, SH_SPACEPILON, STRAT_ADDR_SPACEPILON);
    mb_cspecial(&b, 1200, 0, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1200, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1200, -100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_mapremove(&b, SH_MOTHER1);

    // Lines 47-55: mothers + windmill
    mb_mapmother(&b, 5000, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_mapremove(&b, SH_MOTHER1);
    // windmill
    mb_special(&b, 0, 0, 0, 4000, SH_ROUND_0, IS_WINDMILL);
    mb_setalvarb(&b, AL_ROTY, DEG180);
    mb_setalvarw(&b, AL_SWORD1, 1);

    // Lines 54-56: mapmother mine2 + maprem + asteroid cspecial
    mb_mapmother(&b, 6000, 0, (int16)(1035 + SPACE_VIEWCY), 1800, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_mapremove(&b, SH_MOTHER1);
    mb_cspecial(&b, 3000, 0, SPACE_VIEWCY, 4000, SH_ASTEROID1_PROXY, IS_MISSPOD);

    // Lines 58-74: skillfly section
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, SPACE_VIEWCY, 6000, 100);
    mb_mapobj(&b, 0, 0, SPACE_VIEWCY, 6000, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_special(&b, 0, -180, -250, 6000, SH_R_HOU_0, IS_SHOU0A);
    mb_pathcspecial(&b, 0, 180, -250, 6000, SH_B_HOU_0, PATH_ID_DAMYSCR, 2, 4);
    mb_cspecial(&b, 0, 300, 0, 6000, SH_R_HOU_0, IS_SHOU0A);
    mb_special(&b, 0, 180, 250, 6000, SH_R_HOU_0, IS_SHOU0A);
    mb_pathcspecial(&b, 0, -180, 250, 6000, SH_B_HOU_0, PATH_ID_DAMYSCR, 2, 4);
    mb_cspecial(&b, 6000, -300, 0, 6000, SH_R_HOU_0, IS_SHOU0A);
    // skillfly_bonus item_6
    mb_mapcode65816_inline(&b, &s_level3_6_skillfly_bonus0_guard_script_ptr);
    mb_mapobj(&b, 0, 0, SPACE_VIEWCY, 1500, SH_ITEM_6, IS_ITEM6);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level3_6.skillfly_bonus_0_skip");
    mb_skillfly_set_default(&b, 0, SPACE_VIEWCY, 1500);
    mb_mapwait(&b, 1500);
    // skillfly_bonus item_7
    mb_mapcode65816_inline(&b, &s_level3_6_skillfly_bonus1_guard_script_ptr);
    mb_mapobj(&b, 0, 0, SPACE_VIEWCY, 1500, SH_ITEM_7, IS_ITEM7);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level3_6.skillfly_bonus_1_skip");

    // Lines 75-82: friend chase2 + gate + e_gate
    mb_mapwait(&b, 1000);
    mb_pathobj(&b, 0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    mb_pathobj(&b, 2000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);
    mb_mapobj(&b, 0, -280, SPACE_VIEWCY, 3000, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 2000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 85-89: warp section + mapmother
    mb_mapmother(&b, 2000, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_cspecial(&b, 2000, 0, 0, 1500, SH_WARP_PROXY, STRAT_ADDR_WARP);
    mb_special(&b, 2000, 0, 0, 1500, SH_WARP_PROXY, STRAT_ADDR_WARP);
    mb_cspecial(&b, 4000, 0, 0, 1500, SH_WARP_PROXY, STRAT_ADDR_WARP);
    mb_mapremove(&b, SH_MOTHER1);

    // Lines 91-102: bazooka + uper_m + skillfly + supply_bird
    mb_cspecial(&b, 1000, -100, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAL);
    mb_cspecial(&b, 1200, 0, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 1200, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    mb_cspecial(&b, 3000, 100, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAR);
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, SPACE_VIEWCY, 4000, 100);
    mb_pathcspecial(&b, 0, 0, 0, 4000, SH_B_HOU_0, PATH_ID_DAMYSCR, 2, 4);
    mb_mapmother(&b, 4000, 0, (int16)(1035 + SPACE_VIEWCY), 1500, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    // skillfly_bonus — reuse bonus0 guard (already consumed, so just mapobj)
    mb_mapobj(&b, 4000, 0, SPACE_VIEWCY, 1500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    // supply_bird
    mb_pathobj(&b, 4000, -400, -300, 0, SH_MY_BIRD, PATH_ID_MY_BIRD, 10, 10);
    mb_mapremove(&b, SH_MOTHER1);

    // Lines 104-115: mapmother + big_m missiles + bazooka + shieldr + spacepilon
    mb_mapmother(&b, 3000, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, 0);
    mb_cspecial(&b, 3000, 100, (int16)(SPACE_VIEWCY - 100), 3000, SH_BIG_M, IS_MISSPOD);
    mb_cspecial(&b, 2000, -100, (int16)(SPACE_VIEWCY + 100), 3000, SH_BIG_M, IS_MISSPOD);
    mb_cspecial(&b, 4000, -200, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAL);
    mb_pathspecial(&b, 200, 100, 100, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    mb_pathcspecial(&b, 200, 0, 0, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    mb_pathcspecial(&b, 5000, -100, 100, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    mb_mapobj(&b, 10000, 200, -200, 2000, SH_SPACEPILON, STRAT_ADDR_SPACEPILON);
    mb_mapremove(&b, SH_MOTHER1);
    mb_mapwait(&b, 4500);

    // Lines 117-160: boss section
    // .boss: busy-wait for noctrl flag to clear
    mb_label(&b, "level3_6.boss");
    mb_mapwait(&b, 1);
    mb_mapcode65816_inline(&b, &s_level3_6_noctrl_wait_script_ptr);

    // .tcont: wait for player HP > 0 and fly mode check
    mb_label(&b, "level3_6.owait");
    mb_mapwait(&b, 5);
    mb_mapcode65816_inline(&b, &s_level3_6_hpcheck_wait_script_ptr);

    // .cont2: boss spawn
    mb_label(&b, "level3_6.cont2");
    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, BGM_BOSS1);
    mb_mapobj(&b, 0, 0, 2000, 2500, SH_BOSS_F_3_PROXY, STRAT_ADDR_BOSSF);

    // mapwaitboss / markboss boss36
    mb_mapcode65816_inline(&b, &s_level3_6_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level3_6.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level3_6.bosswait.cont");
    mb_mapgoto(&b, "level3_6.bosswait.loop");
    mb_label(&b, "level3_6.bosswait.cont");
    mb_mapcode65816_inline(&b, &s_level3_6_mapwaitboss_cantdie_script_ptr);
    mb_mapcode65816_inline(&b, &s_level3_6_mapwaitboss_cleanup_script_ptr);

    // markboss boss36
    mb_setbgm(&b, BGM_FADEOUT);
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);
    mb_maprts(&b);

    // CL_DIVE.ASM — clear demo (dive type) appended as subroutine.
    append_cl_dive_submap(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level3_6 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level3_6.skillfly_bonus_0_skip",
                         &s_level3_6_skillfly_bonus0_skip_ptr)) {
        s_level3_6_skillfly_bonus0_skip_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level3_6.skillfly_bonus_1_skip",
                         &s_level3_6_skillfly_bonus1_skip_ptr)) {
        s_level3_6_skillfly_bonus1_skip_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level3_6.boss",
                         &s_level3_6_noctrl_wait_boss_ptr)) {
        s_level3_6_noctrl_wait_boss_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level3_6.owait",
                         &s_level3_6_hpcheck_wait_owait_ptr)) {
        s_level3_6_hpcheck_wait_owait_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level3_6.cont2",
                         &s_level3_6_flymode_check_cont2_ptr)) {
        s_level3_6_flymode_check_cont2_ptr = 0u;
    }

    s_level3_6.data = s_level3_6_data;
    s_level3_6.length = b.length;
}

static void register_level3_6_inline_callbacks(void) {
    if (s_level3_6_skillfly_bonus0_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_6_skillfly_bonus0_guard_script_ptr,
                                    level3_6_skillfly_bonus0_guard);
    }
    if (s_level3_6_skillfly_bonus1_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_6_skillfly_bonus1_guard_script_ptr,
                                    level3_6_skillfly_bonus1_guard);
    }
    if (s_level3_6_noctrl_wait_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_6_noctrl_wait_script_ptr,
                                    map3_6_noctrl_wait);
    }
    if (s_level3_6_hpcheck_wait_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_6_hpcheck_wait_script_ptr,
                                    map3_6_hpcheck_wait);
    }
    if (s_level3_6_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_6_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level3_6_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_6_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level3_6_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_6_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

// ============================================================
// LEVEL1_6.ASM — Venom 1 Surface (Route 1 final approach)
// (MAP_ID_1_6)
// ============================================================

// append_finalmap_content: Shared helper that appends FINALMAP.ASM content
// inline into a builder. Used by build_level1_6_slice, build_level3_7_slice,
// and build_final_slice. The `prefix` disambiguates labels across maps.
// `cantdie_ptr` and `cleanup_ptr` receive the inline callback offsets.
static void append_finalmap_content(MapBuilder *b, const char *prefix,
                                    uint16 *cantdie_ptr, uint16 *cleanup_ptr) {
    char label[64];
    if (!b) return;

    // incmap dm_lb1 — stub for level base 1 demo intro
    mb_mapwait(b, 500);

    // final_tunnel entry point
    snprintf(label, sizeof(label), "%s.tunnel", prefix);
    mb_label(b, label);
    mb_mapwait(b, 2000);

    // set BG to 2_6c
    mb_setbg(b, BG_2_6C);

    // setrestart finalmap_restart
    mb_mapcodejsl_builtin(b, MAP_CB_SETRESTART_L);

    // finalmap_cont entry point
    snprintf(label, sizeof(label), "%s.cont", prefix);
    mb_label(b, label);
    mb_mapplayeroutview(b);
    mb_mapwait(b, 2000);

    // pathobj mes_andross1 message
    mb_pathobj(b, 0, 0, 0, 4000, SH_NULLSHAPE, PATH_ID_MES_ANDROSS1, 10, 10);

    // .finalt: tunnel sections (4 iterations)
    snprintf(label, sizeof(label), "%s.finalt", prefix);
    mb_label(b, label);
    mb_mapnobj(b, 0, 0x0120, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    mb_mapnobj(b, 0, (int16)-0x0120, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    mb_mapnobj(b, 0, 0x0120, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    mb_mapnobj(b, 0x0600, (int16)-0x0120, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);
    mb_maploop(b, label, 4);

    // wall/gate obstacles
    mb_mapobj(b, 0, (int16)-0x0090, -060, 4000, SH_WALL_2, IS_HARD180YR);
    mb_mapobj(b, 0x0500, 0x0090, -060, 4000, SH_WALL_2, IS_HARD180YR);
    mb_mapnobj(b, 0, 0, -60, 4000, SH_GATE_0, STRAT_ADDR_GATE3);
    mb_setalvarb(b, AL_SBYTE1, 1);
    mb_mapwait(b, 1000);

    // pillar pairs
    mb_mapnobj(b, 0, 0, -20, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0800, 0, -20, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0, 0, -100, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0800, 0, -100, 4000, SH_PILLAR3, IS_PILLAR3);

    // item_5 + walls + pillars
    mb_mapnobj(b, 0, 0, -60, 4000, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(b, AL_SBYTE1, 1);
    mb_mapobj(b, 0, (int16)-0x0090, -060, 4000, SH_WALL_2, IS_HARD180YR);
    mb_mapobj(b, 0x1000, 0x0090, -060, 4000, SH_WALL_2, IS_HARD180YR);
    mb_mapnobj(b, 0, 0, -100, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0800, 0, -100, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0, 0, -20, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0800, 0, -20, 4000, SH_PILLAR3, IS_PILLAR3);

    // level 3 conditional pillar section (emitted unconditionally)
    mb_mapnobj(b, 0x0600, 0, -60, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0, 0, -100, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0600, 0, -20, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0200, 0, -20, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0200, 0, -40, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0600, 0, -60, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0200, 0, -100, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0200, 0, -80, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0600, 0, -60, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0, 0, -100, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x0600, 0, -20, 4000, SH_PILLAR3, IS_PILLAR3);

    // common pillar section
    mb_mapnobj(b, 0, 0, -100, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0, 0, -100, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0, 0, -20, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapnobj(b, 0x1500, 0, -20, 4000, SH_PILLAR3, IS_PILLAR3);

    // level 3 half-door section
    snprintf(label, sizeof(label), "%s.level3t", prefix);
    mb_label(b, label);
    mb_mapobj(b, 0, 100, -060, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
    mb_mapnobj(b, 0x1500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(b, 0, (int16)-100, -060, 4000, SH_BOU_1_PROXY, IS_HARD180YR);
    mb_mapnobj(b, 0x1500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_maploop(b, label, 2);
    mb_mapwait(b, 500);

    // level 1 half-door section
    snprintf(label, sizeof(label), "%s.level1t", prefix);
    mb_label(b, label);
    mb_mapobj(b, 0, 110, -060, 4000, SH_WALL_4_PROXY, IS_HARD180YR);
    mb_mapnobj(b, 0x1500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(b, 0, (int16)-110, -060, 4000, SH_WALL_4_PROXY, IS_HARD180YR);
    mb_mapnobj(b, 0x1500, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_maploop(b, label, 2);
    mb_mapwait(b, 500);

    // final corridor: item_7 + wall_4 + halfdL/R
    mb_mapnobj(b, 0, 0x0060, -60, 4000, SH_ITEM_7, IS_ITEM7);
    mb_setalvarb(b, AL_SBYTE1, 1);
    mb_mapobj(b, 0, 110, -060, 4000, SH_WALL_4_PROXY, IS_HARD180YR);
    mb_mapnobj(b, 0x2000, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);
    mb_mapobj(b, 0, (int16)-110, -060, 4000, SH_WALL_4_PROXY, IS_HARD180YR);
    mb_mapnobj(b, 0x1000, 0, 0, 4000, SH_PILLAR3, IS_PILLAR3);

    // tunnel exit
    mb_mapwait(b, 2000);
    mb_pathobj(b, 0, 0, 0, 3000, SH_NULLSHAPE, PATH_ID_MES_ANDROSS2, 10, 10);
    mb_mapwait(b, 200);

    // BG transition
    mb_mapwait(b, 100);
    mb_setbg(b, BG_1_6C);
    mb_initbg(b);
    mb_mapwait(b, 200);

    // boss final music
    mb_setbgm(b, BGM_BOSS_FINAL);
    mb_mapwait(b, 2000);

    // face_b monolith boss
    mb_mapnobj(b, 0x1000, 0, SPACE_VIEWCY, -200, SH_FACE_B_PROXY, STRAT_ADDR_MONOLITH);

    // mapwaitboss nosound
    mb_mapwait(b, 100);
    snprintf(label, sizeof(label), "%s.bosswait.loop", prefix);
    mb_label(b, label);
    {
        char contlabel[64];
        snprintf(contlabel, sizeof(contlabel), "%s.bosswait.cont", prefix);
        mb_mapif_builtin(b, MAP_CB_CHKBOSSDEAD, contlabel);
        mb_mapgoto(b, label);
        mb_label(b, contlabel);
    }
    mb_mapcode65816_inline(b, cantdie_ptr);
    mb_mapcode65816_inline(b, cleanup_ptr);

    // markboss bossfinal
    mb_mapcodejsl_builtin(b, MAP_CB_MARKBOSS_L);
    mb_mapwait(b, 5000);

    // finalmap_end: incmap dm_end — end demo (stub)
    mb_mapwait(b, 500);

    // .wait1: infinite wait
    snprintf(label, sizeof(label), "%s.wait1", prefix);
    mb_label(b, label);
    mb_mapwait(b, 1000);
    mb_mapgoto(b, label);

    // finalmap_restart: setbgm $12, goto finalmap_cont
    snprintf(label, sizeof(label), "%s.restart", prefix);
    mb_label(b, label);
    mb_mapwait(b, 1000);
    mb_setbgm(b, BGM_FINAL_CONT);
    {
        char contlabel[64];
        snprintf(contlabel, sizeof(contlabel), "%s.cont", prefix);
        mb_mapgoto(b, contlabel);
    }
}

static void build_level1_6_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level1_6_data;
    b.capacity = sizeof(s_level1_6_data);
    s_level1_6_mapwaitboss_cantdie_script_ptr = 0u;
    s_level1_6_mapwaitboss_cleanup_script_ptr = 0u;

    // LEVEL1_6.ASM: initlevel 1_6a,0
    // mapjsr map1_6a — Route 1 Venom surface content (MAP1_6A.ASM)
    mb_mapjsr(&b, "level1_6.map1_6a");

    // level1_end: incmap finalmap — Andross final tunnel & boss
    append_finalmap_content(&b, "level1_6.final",
                            &s_level1_6_mapwaitboss_cantdie_script_ptr,
                            &s_level1_6_mapwaitboss_cleanup_script_ptr);

    // ---- MAP1_6A.ASM subroutine (stub — content not yet ported) ----
    mb_label(&b, "level1_6.map1_6a");
    // MAP1_6A.ASM is 304 lines of Venom 1 surface content.
    // Stub: mapwait 2000 / maprts (to be ported in a future batch).
    mb_mapwait(&b, 2000);
    mb_maprts(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level1_6 = s_empty_level;
        return;
    }

    s_level1_6.data = s_level1_6_data;
    s_level1_6.length = b.length;
}

static void register_level1_6_inline_callbacks(void) {
    if (s_level1_6_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_6_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level1_6_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level1_6_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

// ============================================================
// MAP3_7A/B/C.ASM + LEVEL3_7.ASM — Venom 3 Surface (Route 3 final)
// (MAP_ID_3_7)
// ============================================================

// CLEN = SXspacebarlen/2 = 250/2 = 125 = SPACEBAR_UNIT_LEN
#define MAP37_CLEN SPACEBAR_UNIT_LEN

static void build_level3_7_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_level3_7_data;
    b.capacity = sizeof(s_level3_7_data);
    s_level3_7_skillfly_bonus0_guard_script_ptr = 0u;
    s_level3_7_skillfly_bonus0_skip_ptr = 0u;
    s_level3_7_skillfly_bonus1_guard_script_ptr = 0u;
    s_level3_7_skillfly_bonus1_skip_ptr = 0u;
    s_level3_7_mapwaitboss_trigse_script_ptr = 0u;
    s_level3_7_mapwaitboss_cantdie_script_ptr = 0u;
    s_level3_7_mapwaitboss_cleanup_script_ptr = 0u;

    // LEVEL3_7.ASM: initlevel 3_7a,0
    // mapjsr map3_7a
    mb_mapjsr(&b, "level3_7.map3_7a");
    // mapgoto level1_end — jump to finalmap content
    mb_mapgoto(&b, "level3_7.final.tunnel");

    // Dead code in original ASM (after mapgoto):
    // setbg 3_7b / initbg / mapjsr map3_7b / setbg 3_7c / initbg / mapjsr map3_7c
    // mapwait 10000 / mapend
    // We still emit the subroutines so labels resolve.

    // ---- incmap finalmap (level1_end target) ----
    append_finalmap_content(&b, "level3_7.final",
                            &s_level3_7_mapwaitboss_cantdie_script_ptr,
                            &s_level3_7_mapwaitboss_cleanup_script_ptr);

    // ---- MAP3_7A.ASM subroutine — Venom 3 Surface Part A (383 lines) ----
    mb_label(&b, "level3_7.map3_7a");

    // Lines 2-4: restart3_7 — mapwait 2000, mapgoto cont3_7
    mb_label(&b, "level3_7.restart3_7");
    mb_mapwait(&b, 2000);
    mb_mapgoto(&b, "level3_7.cont3_7");

    // Line 8: map3_7a label
    // Line 10: incmap planet — inline planet scenery objects
    // PLANET.ASM: mapnozremove + scenery objects
    mb_mapobj(&b, 0, 0x0220, -1000, -200, SH_R_BU_4, IS_HARD);
    mb_mapobj(&b, 0, 0x0220, -500, -200, SH_R_BU_4, IS_HARD);
    mb_mapobj(&b, 0, 0x0220, -10, -200, SH_R_BU_4, IS_HARD);
    mb_mapobj(&b, 0, -500, 0, 0x0400, SH_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, (uint8)(256u - 64u));
    mb_mapobj(&b, 0, 500, 0, 0x0400, SH_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 64);
    mb_mapobj(&b, 0, 800, 0, -300, SH_BU_0, IS_HARD);
    mb_mapobj(&b, 0, -800, 0, -300, SH_BU_0, IS_HARD);
    mb_mapobj(&b, 0, -300, 0, -800, SH_BU_2, IS_HARD);
    mb_mapobj(&b, 0, 300, 0, -800, SH_BU_2, IS_HARD);
    mb_mapobj(&b, 0, -0x0220, -1000, -200, SH_R_BU_4, IS_HARD);
    mb_mapobj(&b, 0, -0x0220, -500, -200, SH_R_BU_4, IS_HARD);
    mb_mapobj(&b, 0x0200, -0x0220, -10, -200, SH_R_BU_4, IS_HARD);
    mb_mapobj(&b, 0, -200, -300, 600, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0200, 200, -300, 600, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 200, -300, 800, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0200, -200, -300, 800, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 180, -250, 1000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0400, -180, -250, 1000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 150, -200, 1000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0300, -150, -200, 1000, SH_R_BU_7, IS_HARD180YR);

    // Lines 11-18: r_bu_7 pairs
    mb_mapobj(&b, 0, 0x0200, -0x0125, 2000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0400, -0x0200, -0x0125, 2000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 0x0200, -0x0125, 2000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0400, -0x0200, -0x0125, 2000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 0x0200, -0x0125, 2000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0400, -0x0200, -0x0125, 2000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 0x0200, -0x0125, 2000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0400, -0x0200, -0x0125, 2000, SH_R_BU_7, IS_HARD180YR);

    // Line 21: setrestart restart3_7
    mb_mapcodejsl_builtin(&b, MAP_CB_SETRESTART_L);

    // Line 22: cont3_7
    mb_label(&b, "level3_7.cont3_7");

    // Lines 24-25: bu_0 pair
    mb_mapobj(&b, 0, 0x0300, 0, 3000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0x0200, -0x0300, 0, 3000, SH_BU_0, IS_HARD180YR);

    // Line 26: map_setbarshape solid
    mb_map_setbarshape(&b, MAP_BARSHAPE_SOLID, false);

    // Lines 28-47: .ougi spacebar section
    // map_spacebarIC 0,0,15,0,-2 / map_spacebarX -2,0,-15,0
    mb_map_spacebaric(&b, 0, 0, 15, 0, -2);
    mb_map_spacebarx(&b, -2, 0, -15, 0);
    mb_setalvarw(&b, AL_WORLDX, (uint16)(int16)(-(int16)(MAP37_CLEN * 150 / 100)));
    mb_map_spacebarwait(&b, 2);

    mb_map_spacebaric(&b, 0, 0, 15, 0, 2);
    mb_map_spacebarx(&b, 2, 0, -15, 0);
    mb_setalvarw(&b, AL_WORLDX, (uint16)(int16)(MAP37_CLEN * 150 / 100));
    mb_map_spacebarwait(&b, 2);

    mb_map_spacebaric(&b, 0, 0, 15, 0, -2);
    mb_map_spacebarx(&b, -2, 0, -15, 0);
    mb_setalvarw(&b, AL_WORLDX, (uint16)(int16)(-(int16)(MAP37_CLEN * 150 / 100)));
    mb_map_spacebarwait(&b, 2);

    mb_map_spacebaric(&b, 0, 0, 15, 0, 2);
    mb_map_spacebarx(&b, 2, 0, -15, 0);
    mb_setalvarw(&b, AL_WORLDX, (uint16)(int16)(MAP37_CLEN * 150 / 100));
    mb_map_spacebarwait(&b, 3);

    // Lines 48-49: bu_0 pair
    mb_mapobj(&b, 0, 0x0300, 0, 2970, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0, -0x0300, 0, 2970, SH_BU_0, IS_HARD180YR);

    // Lines 50-62: .bars loop (6 iterations)
    mb_label(&b, "level3_7.bars");
    mb_map_spacebaric(&b, 0, 0, 15, 0, -2);
    mb_map_spacebarx(&b, -2, 0, -15, 0);
    mb_setalvarw(&b, AL_WORLDX, (uint16)(int16)(-(int16)(MAP37_CLEN * 150 / 100)));
    mb_map_spacebarwait(&b, 2);

    mb_map_spacebaric(&b, 0, 0, 15, 0, 2);
    mb_map_spacebarx(&b, 2, 0, -15, 0);
    mb_setalvarw(&b, AL_WORLDX, (uint16)(int16)(MAP37_CLEN * 150 / 100));
    mb_map_spacebarwait(&b, 2);
    mb_maploop(&b, "level3_7.bars", 6);

    // Lines 61-63: final bar + mapwait 1000
    mb_map_spacebaric(&b, 0, 0, 15, 0, 0);
    mb_map_spacebarx(&b, 0, -2, -15, 2);
    mb_mapwait(&b, 1000);

    // Lines 67-72: dossun pathobjs + item_6
    mb_pathobj(&b, 0, -450, -150, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, 450, -200, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_mapobj(&b, 0, 0, -50, 4000, SH_ITEM_6, IS_ITEM6);
    mb_pathobj(&b, 0, -200, -350, 3000, SH_R_BU_1, PATH_ID_E_DOSUN, 10, 8);
    mb_pathobj(&b, 0, 200, -300, 3000, SH_R_BU_1, PATH_ID_ITADOSUN, 10, 8);
    mb_pathobj(&b, 0x0400, 0, -250, 3000, SH_R_BU_1, PATH_ID_E_DOSUN, 10, 8);

    // Lines 73-86: .boards loop (3 iterations)
    mb_label(&b, "level3_7.boards");
    mb_pathobj(&b, 0, -450, -350, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    mb_pathobj(&b, 0, -200, -200, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    mb_pathobj(&b, 0x0400, 200, -350, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, 450, -200, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    mb_pathobj(&b, 0x0400, 100, -200, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    mb_pathobj(&b, 0, -300, -150, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    mb_pathobj(&b, 0, 300, -200, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0x0400, 0, -350, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, 450, -200, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0x0400, -200, -200, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, -450, -350, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0x0400, -100, -200, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    mb_maploop(&b, "level3_7.boards", 3);
    mb_pathobj(&b, 0x4000, 0, -200, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);

    // Lines 89-98: flypillar + pillar3 objects
    mb_mapobj(&b, 0, 0x0800, 0, 4000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    mb_mapobj(&b, 0, -0x03E8, 0, 5000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    mb_mapobj(&b, 0, 0x04B0, 0, 6000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    mb_mapobj(&b, 0, -0x0384, 0, 6000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    mb_mapobj(&b, 0, 0, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    mb_mapobj(&b, 0, 0x0200, 0, 3000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    mb_mapobj(&b, 0x0400, -0x0200, 0, 3000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    mb_mapobj(&b, 0, 0x0500, 0, 3000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    mb_mapobj(&b, 0x0200, -0x0500, 0, 3000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    mb_mapobj(&b, 0x1800, 0, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);

    // Line 100: mapmother flypillars
    mb_mapmother(&b, 0x8000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);

    // Lines 101-107: pillar3 objects
    mb_mapobj(&b, 0x0800, 0, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    mb_mapobj(&b, 0x0800, -0x0200, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    mb_mapobj(&b, 0x0800, 0x0100, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    mb_mapobj(&b, 0x0800, 0x0300, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    mb_mapobj(&b, 0x0800, -0x0340, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    mb_mapobj(&b, 0x0800, 0x0050, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    mb_mapobj(&b, 0x0800, -0x0100, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);

    // Line 108: maprem mother1
    mb_mapremove(&b, SH_MOTHER1);

    // Lines 110-111: friend chase6
    mb_pathobj(&b, 0, -750, -480, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    mb_pathobj(&b, 0x0600, -720, -480, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);

    // Line 112: pathspecial s_tank_0 e_tank
    mb_pathspecial(&b, 0x0600, 0, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 4, 2);

    // Line 113: mapobj bu_0
    mb_mapobj(&b, 0x03E8, -600, 0, 4000, SH_BU_0, IS_HARD180YR);

    // Lines 115-116: pathcspecial tank_1 e_tank
    mb_pathcspecial(&b, 0x07D0, 0x0300, 0, 3000, SH_TANK_1, PATH_ID_E_TANK, 4, 2);
    mb_pathcspecial(&b, 0x07D0, -0x00FA, 0, 3000, SH_TANK_1, PATH_ID_E_TANK, 4, 2);

    // Line 118: pathspecial patrol
    mb_pathspecial(&b, 0x0500, -1100, -600, 2000, SH_ZACO_A, PATH_ID_PATROL, 10, 10);

    // Lines 119-140: R_BU_6 arch with roty settings
    mb_mapobj(&b, 0x0100, -500, -50, 1800, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 127);
    mb_mapobj(&b, 0x0100, -400, -150, 1800, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 110);
    mb_mapobj(&b, 0x0100, -300, -250, 1800, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 97);
    mb_mapobj(&b, 0x0100, -200, -300, 1800, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 86);
    mb_mapobj(&b, 0x0100, -100, -350, 1800, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 75);
    mb_mapobj(&b, 0x0100, 0, -400, 1800, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 64);
    mb_mapobj(&b, 0x0100, 100, -350, 1800, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 180);
    mb_mapobj(&b, 0x0100, 200, -300, 1800, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 168);
    mb_mapobj(&b, 0x0100, 300, -250, 1800, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 155);
    mb_mapobj(&b, 0x0100, 400, -150, 1800, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 140);
    mb_mapobj(&b, 0x0100, 500, -50, 1800, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 127);

    // Line 141: pathcspecial tank_1 e_tank
    mb_pathcspecial(&b, 0x09C4, 0, 0, 3000, SH_TANK_1, PATH_ID_E_TANK, 4, 2);

    // Lines 142-143: bu_0 pair
    mb_mapobj(&b, 0x07D0, -400, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0x07D0, 400, 0, 5000, SH_BU_0, IS_HARD180YR);

    // Lines 146-176: movingwalls section
    mb_mapobj(&b, 0, 0, 0, 4000, SH_WALL_1_PROXY, IS_WALLL);

    // map_setbarshape wire for the SBtype16 sections
    mb_map_setbarshape(&b, MAP_BARSHAPE_WIRE, false);
    {
        const int16 speed = 30;
        mb_map_sbtype16(&b, 0, 10, -4, 0, (int16)-speed, 0);
        mb_map_sbtype16(&b, 5, -10, -3, 0, speed, 0);
        mb_map_sbtype16(&b, 0, 10, -4, 0, (int16)-speed, 0);
        mb_map_sbtype16(&b, 5, -10, -3, 0, speed, 0);
    }
    mb_mapwait(&b, 3000);

    mb_mapobj(&b, 0x05DC, -200, 0, 4000, SH_WALL_1_PROXY, IS_WALLR);
    mb_mapobj(&b, 0x07D0, 400, 0, 4000, SH_WALL_1_PROXY, IS_WALLL);
    mb_mapobj(&b, 0x0190, 0, 0, 4000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    mb_mapobj(&b, 0x07D0, 100, 0, 4000, SH_WALL_1_PROXY, IS_WALLLEFTRIGHT);

    {
        const int16 speed = 30;
        mb_map_sbtype16(&b, 5, -10, -3, 0, speed, 0);
        mb_map_sbtype16(&b, 0, 10, -4, 0, (int16)-speed, 0);
    }
    mb_mapobj(&b, 0, 0, -50, 4000, SH_ITEM_7, IS_ITEM7);

    mb_mapobj(&b, 0x05DC, -350, 0, 4000, SH_WALL_1_PROXY, IS_WALLR);
    mb_mapobj(&b, 0x01F4, 350, 0, 4200, SH_WALL_1_PROXY, IS_WALLL);
    {
        const int16 speed = 30;
        mb_map_sbtype16(&b, 0, 10, -4, 0, (int16)-speed, 0);
        mb_map_sbtype16(&b, 5, -10, -3, 0, speed, 0);
        mb_map_sbtype16(&b, 0, 10, -4, 0, (int16)-speed, 0);
        mb_map_sbtype16(&b, 5, -10, -3, 0, speed, 0);
    }
    mb_mapwait(&b, 1000);

    mb_mapobj(&b, 0x05DC, 0, 0, 4000, SH_WALL_1_PROXY, IS_WALLLEFTRIGHT);
    mb_mapobj(&b, 0x05DC, 400, 0, 4200, SH_WALL_1_PROXY, IS_WALLLEFTRIGHT);
    mb_mapobj(&b, 0x0320, -400, 0, 4200, SH_WALL_1_PROXY, IS_WALLLEFTRIGHT);
    {
        const int16 speed = 30;
        mb_map_sbtype16(&b, 0, 10, -4, 0, (int16)-speed, 0);
        mb_map_sbtype16(&b, 5, -10, -3, 0, speed, 0);
        mb_map_sbtype16(&b, 0, 10, -4, 0, (int16)-speed, 0);
        mb_map_sbtype16(&b, 5, -10, -3, 0, speed, 0);
        mb_map_sbtype16(&b, 0, 10, -4, 0, (int16)-speed, 0);
        mb_map_sbtype16(&b, 4, -10, -3, 0, speed, 0);
    }
    mb_mapobj(&b, 0x05DC, -450, 0, 4000, SH_WALL_1_PROXY, IS_WALLR);
    mb_mapobj(&b, 0x05DC, 450, 0, 4200, SH_WALL_1_PROXY, IS_WALLL);

    // Lines 180-181: gate + e_gate
    mb_mapobj(&b, 0x07D0, 0, -200, 4000, SH_GATE_0, IS_GATE);
    mb_pathobj(&b, 0x03E8, 0, -200, 4000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 184-186: friend + pathspecial tank + chase7
    mb_pathspecial(&b, 0, 0x0300, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 4, 2);
    mb_pathobj(&b, 0, 0, -370, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    mb_pathobj(&b, 0x03E8, 0, -370, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);

    // Line 187: special s_wark_0 spacebarwalker
    mb_special(&b, 0, 0, -270, 3000, SH_S_WARK_0, IS_SPACEBARWALKER);

    // Lines 188-189: r_bu_7 pair
    mb_mapobj(&b, 0, -100, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, 100, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 190-191: skillfly_init + skillfly_set
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0, -150, 3000, 150);

    // Lines 192-194: more r_bu objects
    mb_mapobj(&b, 0x0100, 0, -260, 3000, SH_R_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, -100, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x03E8, 100, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 196-201
    mb_mapobj(&b, 0, -300, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, -100, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_skillfly_set(&b, -200, -150, 3000, 150);
    mb_mapobj(&b, 0x0100, -200, -260, 3000, SH_R_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, -300, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x05DC, -100, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 203-211
    mb_mapobj(&b, 0, 0, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, -200, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, 200, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, -100, -260, 3000, SH_R_BU_2, IS_HARD180YR);
    mb_skillfly_set(&b, 0x0100, -150, 3000, 150);
    mb_mapobj(&b, 0x0100, 100, -260, 3000, SH_R_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, 0, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, -200, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x03E8, 200, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 213-224: bwarker + r_bu_7/r_bu_6 sequence + roty settings
    mb_mapobj(&b, 0, 350, -300, 3000, SH_BWARKER_3, IS_SPACEBARWALKER);
    mb_mapobj(&b, 0x0096, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0096, 350, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0x0096, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0096, 350, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 200, -280, 3000, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, DEG90);
    mb_mapobj(&b, 0x0096, 50, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0096, 50, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0x03E8, 50, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Line 224: skillfly_bonus item_5
    mb_mapcode65816_inline(&b, &s_level3_7_skillfly_bonus0_guard_script_ptr);
    mb_mapobj(&b, 0, 200, -100, 1500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level3_7.skillfly_bonus_0_skip");

    // Lines 227-238: more r_bu_7/r_bu_6 with roty
    mb_mapobj(&b, 0x0096, -50, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0096, -50, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0, -50, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, -200, -280, 3000, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, DEG90);
    mb_mapobj(&b, 0, -350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_pathcspecial(&b, 0, 300, 0, 3000, SH_TANK_1, PATH_ID_E_TANK, 4, 2);
    mb_mapobj(&b, 0, -200, -280, 3000, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, DEG90);
    mb_mapobj(&b, 0x0096, -350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0096, -350, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0x03E8, -350, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 240-253
    mb_mapobj(&b, 0x0096, 450, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0096, 450, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0, 450, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_skillfly_init(&b);
    mb_skillfly_set(&b, 0x0300, -150, 3000, 150);
    mb_mapobj(&b, 0, 300, -280, 3000, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, DEG90);
    mb_mapobj(&b, 0x0096, 150, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0096, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0096, 350, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0x0096, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0096, 350, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    mb_mapobj(&b, 0x05DC, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 255-264
    mb_mapobj(&b, 0, 500, 0, 3000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0x05DC, -200, 0, 3000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, -300, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_skillfly_set(&b, -150, -150, 3000, 150);
    mb_mapobj(&b, 0, -150, -280, 3000, SH_R_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, DEG90);
    mb_mapobj(&b, 0x05DC, 0, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x01F4, -300, 0, 4000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 0x04B0, -600, 0, 4000, SH_BU_0, IS_HARD180YR);

    // Line 264: skillfly_bonus item_5
    mb_mapcode65816_inline(&b, &s_level3_7_skillfly_bonus1_guard_script_ptr);
    mb_mapobj(&b, 0, 0, -180, 1500, SH_ITEM_5, IS_ITEM5);
    mb_setalvarb(&b, AL_SBYTE1, 1);
    mb_label(&b, "level3_7.skillfly_bonus_1_skip");

    // Lines 267-275: patrol + cspecial houdai/walker
    mb_pathspecial(&b, 0x01F4, 1100, -600, 3000, SH_ZACO_A, PATH_ID_PATROL, 10, 10);
    mb_cspecial(&b, 0x03E8, -600, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x03E8, 600, 0, 1000, SH_WALKER_0, IS_WALKING);
    mb_cspecial(&b, 0x03E8, 600, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x03E8, -600, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    mb_cspecial(&b, 0x03E8, -300, 0, 1000, SH_WALKER_0, IS_WALKING);

    // Lines 274-275: friend chase6
    mb_pathobj(&b, 0, -750, -480, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    mb_pathobj(&b, 0x0320, -720, -480, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);

    // Lines 277-306: .block loop (2 iterations) — mapblocksnd + r_bu_1 blocks
    mb_label(&b, "level3_7.block");
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, -250, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, -150, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, -250, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, -150, -50, 1000, SH_R_BU_1, IS_HARD180YR);

    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, 250, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, 150, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, 250, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0800, 150, -50, 1000, SH_R_BU_1, IS_HARD180YR);

    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, 0, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0, 100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, -100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, 0, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0, 100, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x0800, -100, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_maploop(&b, "level3_7.block", 2);

    // Lines 308-327: post-block V pattern
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0, -500, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, 500, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0, -400, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, 400, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0, -300, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, 300, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0, -200, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, 200, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0, -100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, 100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, 0, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0, -100, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x0800, 100, -50, 1000, SH_R_BU_1, IS_HARD180YR);

    // Lines 328-346: .block2 loop (2 iterations)
    mb_label(&b, "level3_7.block2");
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0, -300, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, 300, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0, -200, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, 200, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0, -100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x0100, 100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, 0, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, 0, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, 0, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_mapcodejsl_builtin(&b, MAP_CB_BLOCKSND_L);
    mb_mapobj(&b, 0x0100, 0, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    mb_maploop(&b, "level3_7.block2", 2);

    // Lines 348-354: post-block enemies + buildings
    mb_cspecial(&b, 0x03E8, 600, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    mb_mapobj(&b, 0x03E8, -1200, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0x05DC, 1200, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0x03E8, 300, 0, 4000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 0x03E8, 1000, 0, 4000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0x03E8, -1400, 0, 4000, SH_BU_2, IS_HARD180YR);

    // Lines 355-364: e_kururi formation (10 pathobjs)
    mb_pathobj(&b, 0, -465, -420, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, -465, -120, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, -220, -220, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, -220, -520, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, 0, -420, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, 0, -120, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, 220, -220, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, 220, -520, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0, 465, -420, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    mb_pathobj(&b, 0x0DAC, 465, -120, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);

    // Lines 365-367: patrol specials
    mb_pathspecial(&b, 0, 1100, -600, 2000, SH_ZACO_A, PATH_ID_PATROL, 10, 10);
    mb_pathcspecial(&b, 0, 1300, -800, 2500, SH_ZACO_A, PATH_ID_PATROL, 10, 10);
    mb_pathcspecial(&b, 0x01F4, 1500, -1000, 3000, SH_ZACO_A, PATH_ID_PATROL, 10, 10);

    // Lines 369-374: bu_0 + walker pairs
    mb_mapobj(&b, 0, 400, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_pathcspecial(&b, 0, 350, 0, 5250, SH_WALKER_0, PATH_ID_E_WALK_1, 4, 4);
    mb_mapobj(&b, 0x07D0, -400, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0, 400, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_pathcspecial(&b, 0, -350, 0, 5250, SH_WALKER_0, PATH_ID_E_WALK_1, 4, 4);
    mb_mapobj(&b, 0x07D0, -400, 0, 5000, SH_BU_0, IS_HARD180YR);

    // Lines 377-381: pre-boss transition
    mb_mapwait(&b, 2000);
    mb_setbgm(&b, BGM_FADEOUT);
    mb_setbgm(&b, 6);

    // incmap transfor — TRANSFOR.ASM: boss spawn + mapwaitboss
    // boss_f_4 at -100,-500,0
    mb_mapobj(&b, 0, -100, -500, 0, SH_BOSS_F_4_PROXY, STRAT_ADDR_AIRSHIP);

    // mapwaitboss + markboss boss37
    mb_mapcode65816_inline(&b, &s_level3_7_mapwaitboss_trigse_script_ptr);
    mb_label(&b, "level3_7.bosswait.loop");
    mb_mapif_builtin(&b, MAP_CB_CHKBOSSDEAD, "level3_7.bosswait.cont");
    mb_mapgoto(&b, "level3_7.bosswait.loop");
    mb_label(&b, "level3_7.bosswait.cont");
    mb_mapcodejsl_builtin(&b, MAP_CB_MARKBOSS_L);
    mb_maprts(&b);

    // ---- MAP3_7B.ASM subroutine (stub — empty in original) ----
    mb_label(&b, "level3_7.map3_7b");
    mb_maprts(&b);

    // ---- MAP3_7C.ASM subroutine (stub — empty in original) ----
    mb_label(&b, "level3_7.map3_7c");
    mb_maprts(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_level3_7 = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "level3_7.skillfly_bonus_0_skip",
                         &s_level3_7_skillfly_bonus0_skip_ptr)) {
        s_level3_7_skillfly_bonus0_skip_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "level3_7.skillfly_bonus_1_skip",
                         &s_level3_7_skillfly_bonus1_skip_ptr)) {
        s_level3_7_skillfly_bonus1_skip_ptr = 0u;
    }

    s_level3_7.data = s_level3_7_data;
    s_level3_7.length = b.length;
}

static void register_level3_7_inline_callbacks(void) {
    if (s_level3_7_skillfly_bonus0_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_7_skillfly_bonus0_guard_script_ptr,
                                    level3_6_skillfly_bonus0_guard);
    }
    if (s_level3_7_skillfly_bonus1_guard_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_7_skillfly_bonus1_guard_script_ptr,
                                    level3_6_skillfly_bonus1_guard);
    }
    if (s_level3_7_mapwaitboss_trigse_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_7_mapwaitboss_trigse_script_ptr,
                                    level1_1_mapwaitboss_trigse);
    }
    if (s_level3_7_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_7_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_level3_7_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_level3_7_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

// ============================================================
// FINALMAP.ASM — Andross final map
// (MAP_ID_FINAL)
// ============================================================
static void build_final_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_final_data;
    b.capacity = sizeof(s_final_data);
    s_final_mapwaitboss_trigse_script_ptr = 0u;
    s_final_mapwaitboss_cantdie_script_ptr = 0u;
    s_final_mapwaitboss_cleanup_script_ptr = 0u;

    // Reuse the shared helper with "final" prefix.
    append_finalmap_content(&b, "final",
                            &s_final_mapwaitboss_cantdie_script_ptr,
                            &s_final_mapwaitboss_cleanup_script_ptr);

    mb_resolve(&b);

    if (b.failed) {
        s_final = s_empty_level;
        return;
    }

    s_final.data = s_final_data;
    s_final.length = b.length;
}

static void register_final_inline_callbacks(void) {
    if (s_final_mapwaitboss_cantdie_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_final_mapwaitboss_cantdie_script_ptr,
                                    level1_1_mapwaitboss_cantdie);
    }
    if (s_final_mapwaitboss_cleanup_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_final_mapwaitboss_cleanup_script_ptr,
                                    level1_1_mapwaitboss_cleanup);
    }
}

// ============================================================
// INTRO.ASM — Opening intro cutscene
// (MAP_ID_INTRO)
// ============================================================
static void build_intro_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_intro_data;
    b.capacity = sizeof(s_intro_data);
    s_intro_init_script_ptr = 0u;

    // Lines 2-3: setfadedown quick / mapwaitfade
    mb_qfadedown(&b);
    mb_waitfade(&b);

    // Line 4-5: setbg intro / initbg
    mb_setbg(&b, BG_INTRO);
    mb_initbg(&b);

    // Lines 7-14: start_65816 block — clear position, disable wobble
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);

    // Line 16-17: mapcode_jsl initblack_l / setvar.b stayblack,10
    mb_setvarb(&b, WM_GSVAR_BYTE1, 10);  // stayblack proxy

    // Line 19: setfadeup quick
    mb_qfadeup(&b);

    // Line 21: mapwait 800 (originally "mapwait 246 800")
    mb_mapwait(&b, 800);

    // Lines 23-33: start_65816 block — disable wobble, set noctrl+nofire
    mb_mapcode65816_inline(&b, &s_intro_init_script_ptr);

    // Lines 36-37: textpath nintendo/presents — text rendering (stub)
    // TODO: implement textpath for NINTENDO PRESENTS text when text renderer is ported
    mb_mapwait(&b, 2000);

    // Lines 41-47: player intro ships
    mb_mapnobj(&b, 0x1000, 50, -400, -700, SH_OLD_TYPE_PROXY, STRAT_ADDR_PLAYERDOWN2INTRO);
    mb_mapnobj(&b, 0x1000, 50, -400, -700, SH_OLD_TYPE_PROXY, STRAT_ADDR_PLAYERDOWN3INTRO);
    mb_mapnobj(&b, (uint16)(MEDPSPEED * 5), 50, -400, -700,
               SH_OLD_TYPE_PROXY, STRAT_ADDR_PLAYERDOWNINTRO);
    mb_setvarobj(&b, WM_MAPVAR1);
    mb_mapnobj(&b, 0, 0, -400, -700, SH_NULLSHAPE, STRAT_ADDR_PLAYERFIREINTRO);
    mb_setalvarptrw(&b, AL_SWORD1, WM_MAPVAR1);

    // Line 50: mapwait 2000
    mb_mapwait(&b, 2000);

    // Line 52: deboss_1 boss7intro
    mb_mapnobj(&b, 0, 0, -800, -400, SH_DEBOSS_1_PROXY, STRAT_ADDR_BOSS7INTRO);

    // Line 54: mapwait 8000
    mb_mapwait(&b, 8000);

    // Lines 55-63: zaco waves
    mb_mapnobj(&b, 600, -400, -800, 2000, SH_ZACO_A, STRAT_ADDR_ZACOINTRO);
    mb_mapnobj(&b, 600, 400, -800, 2000, SH_ZACO_A, STRAT_ADDR_ZACOINTRO);
    mb_mapnobj(&b, 400, -400, -800, 2000, SH_ZACO_5, STRAT_ADDR_ZACOINTRO);
    mb_mapnobj(&b, 400, 400, -800, 2000, SH_ZACO_5, STRAT_ADDR_ZACOINTRO);
    mb_mapnobj(&b, 400, -400, -800, 2000, SH_ZACO_5, STRAT_ADDR_ZACOINTRO);
    mb_mapnobj(&b, 400, 400, -800, 2000, SH_ZACO_5, STRAT_ADDR_ZACOINTRO);
    mb_mapnobj(&b, 400, -400, -800, 2000, SH_ZACO_5, STRAT_ADDR_ZACOINTRO);
    mb_mapnobj(&b, 400, 400, -800, 2000, SH_ZACO_5, STRAT_ADDR_ZACOINTRO);

    // Line 65: zaco2intro
    mb_mapnobj(&b, 400, 0, -800, 2000, SH_ZACO_5, STRAT_ADDR_ZACO2INTRO);

    // Lines 67-70: more zacos
    mb_mapnobj(&b, 400, -400, -800, 2000, SH_ZACO_5, STRAT_ADDR_ZACOINTRO);
    mb_mapnobj(&b, 400, 400, -800, 2000, SH_ZACO_5, STRAT_ADDR_ZACOINTRO);
    mb_mapnobj(&b, 400, -400, -800, 2000, SH_ZACO_5, STRAT_ADDR_ZACOINTRO);
    mb_mapnobj(&b, 400, 400, -800, 2000, SH_ZACO_5, STRAT_ADDR_ZACOINTRO);

    // .lp: infinite wait
    mb_label(&b, "intro.lp");
    mb_mapwait(&b, 5000);
    mb_mapgoto(&b, "intro.lp");

    mb_resolve(&b);

    if (b.failed) {
        s_intro = s_empty_level;
        return;
    }

    s_intro.data = s_intro_data;
    s_intro.length = b.length;
}

static void register_intro_inline_callbacks(void) {
    if (s_intro_init_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_intro_init_script_ptr,
                                    intro_init_inline);
    }
}

// ============================================================
// TITLE.ASM — Title screen, continue screen, wait map
// (MAP_ID_TITLE, MAP_ID_CONTINUE, MAP_ID_WAIT)
// ============================================================
static void build_title_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_title_data;
    b.capacity = sizeof(s_title_data);
    s_title_init_script_ptr = 0u;
    s_contmap_init_script_ptr = 0u;
    s_contmap_ptr = 0u;
    s_waitmap_ptr = 0u;

    // ---- titlemap ----
    // Lines 2-3: setfadedown quick / mapwaitfade
    mb_qfadedown(&b);
    mb_waitfade(&b);
    // Lines 5-6: setbg title / initbg
    mb_setbg(&b, BG_TITLE);
    mb_initbg(&b);
    // bg_title_1 in BGS.ASM: bgm title — load title music
    mb_setbgm(&b, 2); /* SND_TITLE */

    // Lines 8-16: timeovercont — start_65816 block: clear pos
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);
    mb_setvarb(&b, WM_GSVAR_BYTE1, 10);  // stayblack proxy

    // Line 22: mapwait 800
    mb_mapwait(&b, 800);

    // Line 23: mapobj my_demo tit_istrat
    mb_mapnobj(&b, 0, 20, 20, 70, SH_MY_DEMO_PROXY, STRAT_ADDR_TIT);

    // Line 25: setfadeup quick
    mb_qfadeup(&b);

    // Lines 29-36: start_65816 block — disable wobble, set noctrl+nofire
    mb_mapcode65816_inline(&b, &s_title_init_script_ptr);

    // .ll: infinite wait
    mb_label(&b, "title.ll");
    mb_mapwait(&b, 4000);
    mb_mapgoto(&b, "title.ll");

    // ---- contmap ----
    mb_label(&b, "title.contmap");
    mb_qfadedown(&b);
    mb_waitfade(&b);
    mb_setbg(&b, BG_CONT);
    mb_initbg(&b);

    // Lines 48-57: start_65816 block — clear pos, disable wobble
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);

    // Line 60: mapwait medpspeed*4
    mb_mapwait(&b, (uint16)(MEDPSPEED * 4u));

    // Lines 62-63: initblack + stayblack
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);
    mb_setvarb(&b, WM_GSVAR_BYTE1, 3);  // stayblack proxy

    // Line 64: setfadeup quick
    mb_qfadeup(&b);

    // Lines 65-66: .ll infinite wait
    mb_label(&b, "title.cont.ll");
    mb_mapwait(&b, 4000);
    mb_mapgoto(&b, "title.cont.ll");

    // ---- waitmap ----
    mb_label(&b, "title.waitmap");
    mb_label(&b, "title.wait.ll");
    mb_mapwait(&b, 4000);
    mb_mapgoto(&b, "title.wait.ll");

    mb_resolve(&b);

    if (b.failed) {
        s_title = s_empty_level;
        return;
    }

    if (!mb_lookup_label(&b, "title.contmap", &s_contmap_ptr)) {
        s_contmap_ptr = 0u;
    }
    if (!mb_lookup_label(&b, "title.waitmap", &s_waitmap_ptr)) {
        s_waitmap_ptr = 0u;
    }

    s_title.data = s_title_data;
    s_title.length = b.length;
}

static void register_title_inline_callbacks(void) {
    if (s_title_init_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_title_init_script_ptr,
                                    title_init_inline);
    }
    if (s_contmap_init_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_contmap_init_script_ptr,
                                    contmap_init_inline);
    }
}

// ============================================================
// PLANET.ASM — Planet selection screen
// (MAP_ID_PLANET)
// ============================================================
static void build_planet_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_planet_data;
    b.capacity = sizeof(s_planet_data);

    // PLANET.ASM — planet selection screen background objects.
    // Note: mapnozremove is a runtime flag that prevents Z-removal of objects.
    // In the HD engine, this is handled by the renderer. Emit as a comment only.
    // mapnozremove — objects persist regardless of Z distance

    // Lines 7-9: r_bu_4 row (right side)
    mb_mapobj(&b, 0, 0x0220, -1000, -200, SH_R_BU_4, IS_HARD);
    mb_mapobj(&b, 0, 0x0220, -500, -200, SH_R_BU_4, IS_HARD);
    mb_mapobj(&b, 0, 0x0220, -10, -200, SH_R_BU_4, IS_HARD);

    // Lines 11-12: bu_6 pair with roty
    mb_mapobj(&b, 0, -500, 0, 400, SH_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, (uint8)(int8)-64);
    mb_mapobj(&b, 0, 500, 0, 400, SH_BU_6, IS_HARD);
    mb_setalvarb(&b, AL_ROTY, 64);

    // Lines 15-18: bu_0 and bu_2 pairs
    mb_mapobj(&b, 0, 800, 0, -300, SH_BU_0, IS_HARD);
    mb_mapobj(&b, 0, -800, 0, -300, SH_BU_0, IS_HARD);
    mb_mapobj(&b, 0, -300, 0, -800, SH_BU_2, IS_HARD);
    mb_mapobj(&b, 0, 300, 0, -800, SH_BU_2, IS_HARD);

    // Lines 19-21: r_bu_4 row (left side)
    mb_mapobj(&b, 0, -0x0220, -1000, -200, SH_R_BU_4, IS_HARD);
    mb_mapobj(&b, 0, -0x0220, -500, -200, SH_R_BU_4, IS_HARD);
    mb_mapobj(&b, 0x0200, -0x0220, -10, -200, SH_R_BU_4, IS_HARD);

    // Lines 24-31: r_bu_7 grid
    mb_mapobj(&b, 0, -200, -300, 600, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0200, 200, -300, 600, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 200, -300, 800, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0200, -200, -300, 800, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 180, -250, 1000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0400, -180, -250, 1000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0, 150, -200, 1000, SH_R_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x0300, -150, -200, 1000, SH_R_BU_7, IS_HARD180YR);

    // Infinite wait — planet screen runs until the player selects a level.
    mb_label(&b, "planet.wait");
    mb_mapwait(&b, 4000);
    mb_mapgoto(&b, "planet.wait");

    mb_resolve(&b);

    if (b.failed) {
        s_planet = s_empty_level;
        return;
    }

    s_planet.data = s_planet_data;
    s_planet.length = b.length;
}

// ============================================================
// CREDITS.ASM — End credits sequence
// (MAP_ID_CREDITS)
// ============================================================
//
// The credits sequence uses textpath extensively for rendering
// 3D text along paths. Since textpath is not yet implemented in
// the HD engine, the text portions are stubbed with mapwait delays
// matching the original timing. The structural elements (fade,
// background, player disable, THE END letters) are fully ported.
// ============================================================
static void build_credits_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_credits_data;
    b.capacity = sizeof(s_credits_data);
    s_credits_init_script_ptr = 0u;

    // Lines 3-4: setfadedown quick, mapwaitfade
    mb_qfadedown(&b);
    mb_waitfade(&b);

    // Lines 6-7: meters_off trans (runtime only), setbg cred, initbg
    mb_setbg(&b, BG_CRED);
    mb_initbg(&b);

    // Lines 10-17: start_65816 block — clear player Z, clear viewpos
    // Approximated as inline callback that disables wobble + controls.
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);

    // Lines 19-20: mapcode_jsl initblack_l, setvar.b stayblack,10
    mb_mapcodejsl_builtin(&b, MAP_CB_INITBLACK_L);
    mb_setvarb(&b, WM_GSVAR_BYTE1, 10);

    // Lines 26-27: setfadeup quick, mapwait 200
    mb_qfadeup(&b);
    mb_mapwait(&b, 200);

    // Lines 28-38: start_65816 block — disable wobble, set noctrl+nofire
    mb_mapcode65816_inline(&b, &s_credits_init_script_ptr);

    // Line 40: mapjsr actualcreds
    mb_mapjsr(&b, "credits.actualcreds");

    // Lines 42-47: THE END letter pathobjs
    mb_pathobj(&b, 0, 972, -969, 1000, SH_FONT_T2_PROXY, PATH_ID_THEENDT, 6, 4);
    mb_pathobj(&b, 0, -1120, 1377, 1000, SH_FONT_H2_PROXY, PATH_ID_THEENDH, 6, 4);
    mb_pathobj(&b, 0, -1019, -1530, 1000, SH_FONT_E2_PROXY, PATH_ID_THEENDE, 6, 4);
    mb_pathobj(&b, 0, 1070, -1326, 1000, SH_FONT_E3_PROXY, PATH_ID_THEENDE2, 6, 4);
    mb_pathobj(&b, 0, 1550 + 29, 1323 + 54, 1000, SH_FONT_N2_PROXY, PATH_ID_THEENDN, 6, 4);
    mb_pathobj(&b, 0, -1050 + 129, 1428, 1000, SH_FONT_D2_PROXY, PATH_ID_THEENDD, 6, 4);

    // Line 48: mapwait 6000
    mb_mapwait(&b, 6000);

    // Line 49: setvar.b levelfinished,le_endofcreds
    // le_endofcreds is a level-finished sentinel value.
    // Use 2 as a proxy (normal level finished is 1).
    mb_setvarb(&b, WM_LEVELFINISHED, 2u);

    // Lines 50-53: .lp infinite wait (IFEQ EXITCREDITS path — credits loop)
    mb_label(&b, "credits.lp");
    mb_mapwait(&b, 5000);
    mb_mapgoto(&b, "credits.lp");

    // ---- actualcreds subroutine ----
    mb_label(&b, "credits.actualcreds");

    // Line 85: mapjsr cutcreds
    mb_mapjsr(&b, "credits.cutcreds");

    // Lines 86-131: textpath credit blocks — stubbed as timed waits.
    // credwait = 5000 (NTSC default)
    mb_mapwait(&b, 5000);  // cutcreds wait
    // superfxstaff + names
    mb_mapwait(&b, 5000);
    // software + names
    mb_mapwait(&b, 5000);
    // english + names
    mb_mapwait(&b, 5000);
    // japanese + names
    mb_mapwait(&b, 5000);

    // Line 131: mapwait 9000-31*medpspeed
    mb_mapwait(&b, (uint16)(9000u - 31u * MEDPSPEED));
    mb_maprts(&b);

    // ---- cutcreds subroutine ----
    mb_label(&b, "credits.cutcreds");

    // Line 136: mapwait 2000
    mb_mapwait(&b, 2000);

    // Lines 137-140: Star Fox / presented by / Nintendo pathobjs
    mb_pathobj(&b, 1200, 0, -1500, 3500, SH_NULLSHAPE, PATH_ID_DSTARFOX, 10, 10);
    // textpath stubs — not yet implemented
    mb_mapwait(&b, 0);  // placeholder for presented/by textpaths
    mb_pathobj(&b, 1200, 0, 1500, 3500, SH_NULLSHAPE, PATH_ID_DNINTENDO, 10, 10);

    // Lines 142-191: executive through developed by — textpath credit blocks
    mb_mapwait(&b, 3000);  // executive + yamauchi
    mb_mapwait(&b, 5000);  // producer + miyamoto
    mb_mapwait(&b, 5000);  // director + eguchi
    mb_mapwait(&b, 5000);  // assistant director + yamada
    mb_mapwait(&b, 5000);  // programmed by + dylan/giles/krister
    mb_mapwait(&b, 5000);  // 3d system + pete/carl
    mb_mapwait(&b, 5000);  // graphic designer + imamura
    mb_mapwait(&b, 5000);  // shape designer + watanabe
    mb_mapwait(&b, 5000);  // effects + kondo
    mb_mapwait(&b, 5000);  // composer + hirasawa
    mb_mapwait(&b, 5000);  // developed by + argonaut

    mb_maprts(&b);

    mb_resolve(&b);

    if (b.failed) {
        s_credits = s_empty_level;
        return;
    }

    s_credits.data = s_credits_data;
    s_credits.length = b.length;
}

static void register_credits_inline_callbacks(void) {
    if (s_credits_init_script_ptr != 0u) {
        World_RegisterInlineMapCode(s_credits_init_script_ptr,
                                    credits_init_inline);
    }
}

// ============================================================
// TRAINING.ASM — Training mode
// (MAP_ID_TRAINING)
// ============================================================
//
// The IFNE messagetest block is compile-time conditional test code
// and is skipped. The ELSEIF block is the actual training level
// content (the normal gameplay path).
// ============================================================
static void build_training_slice(void) {
    MapBuilder b;

    memset(&b, 0, sizeof(b));
    b.data = s_training_data;
    b.capacity = sizeof(s_training_data);
    s_training_eguchifly_loop_ptr = 0u;

    // TRAINING.ASM line 2: initlevel training,mstarwipe_circle
    // initlevel handled by runtime; we just emit the map data.

    // Line 3: setvar.n prttraining,1 — runtime training flag, not a map opcode.

    // Line 5: mapwait 2000
    mb_mapwait(&b, 2000);

    // ELSEIF block (actual training content):
    // Line 34: pathobj zaco_5,trn_ck
    mb_pathobj(&b, 0, 0, 0, 3000, SH_ZACO_5, PATH_ID_TRN_CK, 10, 10);

    // Lines 35-36: mapobj BU_8 and BU_1
    mb_mapobj(&b, 0, 0x1200, 0, 5000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, -0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    // Line 38: pilon ground obstacle
    mb_mapobj(&b, 0, 0, 0x0500, 5000, SH_PILON_PROXY, STRAT_ADDR_GROUNDPILON);

    // Lines 40-45: more building objects
    mb_mapobj(&b, 0, 0x1200, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, -0x1200, 0, 5000, SH_BU_2, IS_HARD180YR);
    mb_mapobj(&b, 0, 0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, -0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 0x1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 0x2000, -0x1000, 0, 5000, SH_TOWER_2, IS_TOWER0);

    // Line 46 = .et label (eguchifly_goto loop target)
    mb_label(&b, "training.et");

    // Lines 49-55: training rings and buildings
    mb_pathobj(&b, 0, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    mb_mapobj(&b, 0, 0x1200, 0, 5000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, -0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    mb_pathobj(&b, 0, 0x0200, -150, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING2, 10, 10);
    mb_mapobj(&b, 0, 0x1200, 0, 5000, SH_BU_0, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, -0x1200, 0, 5000, SH_BU_2, IS_HARD180YR);

    mb_pathobj(&b, 0, 0, -200, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    mb_mapobj(&b, 0, 0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, -0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);

    mb_pathobj(&b, 0, -0x0200, -150, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING2, 10, 10);
    mb_mapobj(&b, 0, 0x1000, 0, 5000, SH_TOWER_2, IS_TOWER0);
    mb_mapobj(&b, 0x2000, -0x1000, 0, 5000, SH_TOWER_2, IS_TOWER0);

    // Lines 66-76: more rings and buildings
    mb_pathobj(&b, 0, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    mb_mapobj(&b, 0, 0x1200, 0, 5000, SH_PILLAR3, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, -0x1200, 0, 5000, SH_PILLAR3, IS_HARD180YR);

    mb_pathobj(&b, 0, 0x0200, -200, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING2, 10, 10);
    mb_mapobj(&b, 0, 0x1200, 0, 5000, SH_ROBOT_0, IS_HARD180YR);
    mb_mapobj(&b, 0x1200, -0x1200, 0, 5000, SH_ROBOT_0, IS_HARD180YR);

    mb_pathobj(&b, 0, -0x0330, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    mb_mapobj(&b, 0, 0x1200, 0, 5000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x2000, -0x1200, 0, 5000, SH_BU_7, IS_HARD180YR);

    // Lines 78-87: solo rings
    mb_pathobj(&b, 1000, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING2, 10, 10);
    mb_mapobj(&b, 0x0800, 0, 0, 5000, SH_BU_7, IS_HARD180YR);

    mb_pathobj(&b, 1000, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    mb_pathobj(&b, 1000, 0x0100, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING2, 10, 10);
    mb_pathobj(&b, 800, -0x0200, -300, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    mb_pathobj(&b, 800, -0x0100, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING2, 10, 10);
    mb_pathobj(&b, 800, 0, -300, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);
    mb_pathobj(&b, 2000, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING2, 10, 10);

    // Line 92: base_1 object
    mb_mapobj(&b, 0x0300, 0, 0, 5000, SH_BASE_1, IS_BASE_1);

    // Line 94: long ring stretch
    mb_pathobj(&b, 8000, 0, -100, 5000, SH_NULLSHAPE, PATH_ID_TRN_RING, 10, 10);

    // Line 95: eguchifly_goto .et — loop back
    // Approximated: in the original this checks if eguchifly training is
    // complete; for now we just continue past (single pass).
    mb_mapcode65816_inline(&b, &s_training_eguchifly_loop_ptr);

    // Lines 97-99: friend ship pathobjs
    mb_pathobj(&b, 0, 0, -570, -100, SH_FRIENDSHIP_4, PATH_ID_HENTAI_FAL, 10, 10);
    mb_pathobj(&b, 0, 100, -470, -100, SH_FRIENDSHIP_4, PATH_ID_HENTAI_FRO, 10, 10);
    mb_pathobj(&b, 1000, -100, -470, -100, SH_FRIENDSHIP_4, PATH_ID_HENTAI_RAB, 10, 10);

    // Line 101: mapmsg 123
    mb_sendmsg(&b, 123);

    // Lines 102-108: .etlop — building loop
    mb_label(&b, "training.etlop");
    mb_mapobj(&b, 0, 0x1200, 0, 5000, SH_BU_8, IS_HARD180YR);
    mb_mapobj(&b, 0x4200, -0x1200, 0, 5000, SH_BU_1, IS_HARD180YR);
    mb_mapobj(&b, 0, 0x1200, 0, 5000, SH_BU_7, IS_HARD180YR);
    mb_mapobj(&b, 0x4200, -0x1200, 0, 5000, SH_BU_7, IS_HARD180YR);
    mb_maploop(&b, "training.etlop", 4);

    // Lines 109-114: pilon obstacles
    mb_mapobj(&b, 0, -0x0200, -70, 5000, SH_PILON_PROXY, STRAT_ADDR_GROUNDPILON);
    mb_mapobj(&b, 0, 0, -70, 5000, SH_PILON_PROXY, STRAT_ADDR_GROUNDPILON);
    mb_mapobj(&b, 0x6000, 0x0200, -70, 5000, SH_PILON_PROXY, STRAT_ADDR_GROUNDPILON);
    mb_mapobj(&b, 0, 0, -70, 5000, SH_PILON_PROXY, STRAT_ADDR_GROUNDPILON);
    mb_mapobj(&b, 0, 0, -140, 5000, SH_PILON_PROXY, STRAT_ADDR_GROUNDPILON);
    mb_mapobj(&b, 0x8000, 0, -210, 5000, SH_PILON_PROXY, STRAT_ADDR_GROUNDPILON);

    // Line 116: mapgoto .et — loop back to main section
    mb_mapgoto(&b, "training.et");

    mb_resolve(&b);

    if (b.failed) {
        s_training = s_empty_level;
        return;
    }

    s_training.data = s_training_data;
    s_training.length = b.length;
}

static void register_training_inline_callbacks(void) {
    if (s_training_eguchifly_loop_ptr != 0u) {
        World_RegisterInlineMapCode(s_training_eguchifly_loop_ptr,
                                    training_eguchifly_check);
    }
}

static void ensure_literal_levels_built(void) {
    if (s_literal_levels_ready) {
        return;
    }
    memset(s_unported_level_warned, 0, sizeof(s_unported_level_warned));
    build_level1_1_opening_slice();
    build_level1_2_wrapper_slice();
    build_level1_3_opening_slice();
    build_level1_4_wrapper_slice();
    build_level2_1_wrapper_slice();
    build_level2_2_wrapper_slice();
    build_level2_3a_slice();
    build_level2_4_slice();
    build_level2_5_slice();
    build_level3_1_wrapper_slice();
    build_level3_2_wrapper_slice();
    build_level3_3_wrapper_slice();
    build_level1_5_slice();
    build_level3_5_wrapper_slice();
    build_level3_6_slice();
    build_level1_6_slice();
    build_level3_7_slice();
    build_level2_6_wrapper_slice();
    build_level_bh_slice();
    build_level3_4_slice();
    build_level_special_slice();
    build_final_slice();
    build_intro_slice();
    build_title_slice();
    build_planet_slice();
    build_credits_slice();
    build_training_slice();
    s_literal_levels_ready = true;
}

static const MapLevelData *warn_unported_level(uint32 map_id) {
    if (map_id < ARRAY_SIZE(s_unported_level_warned) && !s_unported_level_warned[map_id]) {
        s_unported_level_warned[map_id] = 1u;
        printf("Levels: map id %u is not ported yet; using end-stub\n", (unsigned)map_id);
    }
    return &s_empty_level;
}

const MapLevelData *Levels_GetMapData(uint32 map_id) {
    ensure_literal_levels_built();

    switch (map_id) {
    case MAP_ID_NONE:
        return &s_empty_level;
    case MAP_ID_1_1:
        register_level1_1_inline_callbacks();
        return &s_level1_1;
    case MAP_ID_1_2:
        register_level1_2_inline_callbacks();
        return &s_level1_2;
    case MAP_ID_1_3:
        register_level1_3_inline_callbacks();
        return &s_level1_3;
    case MAP_ID_1_4:
        register_level1_4_inline_callbacks();
        return &s_level1_4;
    case MAP_ID_1_5:
        register_level1_5_inline_callbacks();
        return &s_level1_5;
    case MAP_ID_2_1:
        register_level2_1_inline_callbacks();
        return &s_level2_1;
    case MAP_ID_2_2:
        register_level2_2_inline_callbacks();
        return &s_level2_2;
    case MAP_ID_2_3:
        register_level2_3_inline_callbacks();
        return &s_level2_3;
    case MAP_ID_2_4:
        register_level2_4_inline_callbacks();
        return &s_level2_4;
    case MAP_ID_2_5:
        register_level2_5_inline_callbacks();
        return &s_level2_5;
    case MAP_ID_3_1:
        register_level3_1_inline_callbacks();
        return &s_level3_1;
    case MAP_ID_3_2:
        register_level3_2_inline_callbacks();
        return &s_level3_2;
    case MAP_ID_3_3:
        register_level3_3_inline_callbacks();
        return &s_level3_3;
    case MAP_ID_2_6:
        register_level2_6_inline_callbacks();
        return &s_level2_6;
    case MAP_ID_BLACKHOLE:
        return &s_level_bh;
    case MAP_ID_3_4:
        register_level3_4_inline_callbacks();
        return &s_level3_4;
    case MAP_ID_3_5:
        register_level3_5_inline_callbacks();
        return &s_level3_5;
    case MAP_ID_1_6:
        register_level1_6_inline_callbacks();
        return &s_level1_6;
    case MAP_ID_3_6:
        register_level3_6_inline_callbacks();
        return &s_level3_6;
    case MAP_ID_3_7:
        register_level3_7_inline_callbacks();
        return &s_level3_7;
    case MAP_ID_SPECIAL:
        register_level_special_inline_callbacks();
        return &s_level_special;
    case MAP_ID_FINAL:
        register_final_inline_callbacks();
        return &s_final;
    case MAP_ID_INTRO:
        register_intro_inline_callbacks();
        return &s_intro;
    case MAP_ID_TITLE:
        register_title_inline_callbacks();
        return &s_title;
    case MAP_ID_CONTINUE:
        register_title_inline_callbacks();
        return &s_title;
    case MAP_ID_WAIT:
        return &s_title;
    case MAP_ID_PLANET:
        return &s_planet;
    case MAP_ID_CREDITS:
        register_credits_inline_callbacks();
        return &s_credits;
    case MAP_ID_TRAINING:
        register_training_inline_callbacks();
        return &s_training;
    default:
        return warn_unported_level(map_id);
    }
}
