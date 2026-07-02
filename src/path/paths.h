// PATHS.ASM → C conversion
// PATH bytecode interpreter — scripted movement sequences for enemies/wingmen.
//
// Each path-following alien stores its instruction pointer in al_sword2.
// The interpreter reads opcodes from a global "paths" bytecode array,
// dispatches to handlers, then applies physics (velocity, position, rotation).
//
// Flags in al_sflags2 control per-frame behavior:
//   sflag1 = Z-relative-to-player
//   sflag2 = always regenerate velocity vectors
//   sflag3 = helicopter mode (rotx = velocity)
//   sflag4 = space flight (couple roty to rotz)

#ifndef STARFOX_PATH_PATHS_H
#define STARFOX_PATH_PATHS_H

#include "../types.h"
#include "../game/obj.h"

// Path opcode constants (literal indices from PATHS.ASM s_mode_table).
#define P_RELTOPLAYERON        0
#define P_RELTOPLAYEROFF       1
#define P_WAIT                 2
#define P_ALWAYSGENVECSON      3
#define P_ALWAYSGENVECSOFF     4
#define P_SETVEL               5
#define P_LOOP                 6
#define P_ADDB                 7
#define P_ADDW                 8
#define P_FACEPLAYER           9
#define P_WAITFACEPLAYER      10
#define P_ACHASEB             11
#define P_ACHASEW             12
#define P_WAITACHASEB         13
#define P_WAITACHASEW         14
#define P_END                 15
#define P_SETB                16
#define P_SETW                17
#define P_FINDSHAPE           18
#define P_FINDNEXTSHAPE       19
#define P_FACESHAPE           20
#define P_REMOVE              21
#define P_GOTOPOS             22
#define P_GOTOSHAPEPOS        23
#define P_EXPLODE             24
#define P_IMMUNE              25
#define P_SPACESHIPON         26
#define P_SPACESHIPOFF        27
#define P_HELION              28  // p_zacoon
#define P_HELIOFF             29  // p_zacooff
#define P_DISTLESS            30
#define P_SHAPEDISTLESS       31
#define P_GOTO                32
#define P_IGOTO               33
#define P_SETACCEL            34
#define P_HITGROUND           35
#define P_HITWALL             36
#define P_INITANIM            37
#define P_ADDANIM             38
#define P_DEBRIS              39
#define P_FRIEND              40
#define P_LINK                41
#define P_SHAPEDEAD           42
#define P_IFLEVEL             43
#define P_LEFTOFPLAYER        44
#define P_RIGHTOFPLAYER       45
#define P_ABOVEPLAYER         46
#define P_BELOWPLAYER         47
#define P_NOTFRIEND           48
#define P_MSG                 49
#define P_MSG2                50
#define P_DAMAGE              51
#define P_ALMOSTDEAD          52
#define P_SMOKEON             53
#define P_SMOKEOFF            54
#define P_RANDOMGOTO          55
#define P_IFSAMEB             56
#define P_IFSAMEW             57
#define P_IFBETWEENB          58
#define P_IFBETWEENW          59
#define P_INVINCIBLEON        60
#define P_INVINCIBLEOFF       61
#define P_PLAYERDEAD          62
#define P_SPAWN               63
#define P_SPAWNLINK           64
#define P_SPAWNCHILD          65
#define P_ZREMOVEON           66
#define P_ZREMOVEOFF          67
#define P_DEBUG               68
#define P_FIRE                69
#define P_FIRECANHIT          70
#define P_FIREATPLAYER        71
#define P_FIREATPLAYERCANHIT  72
#define P_FIREATSHAPE         73
#define P_FIREATSHAPECANHIT   74
#define P_WEAPON              75
#define P_CHILDDEAD           76
#define P_LINKCHILD           77
#define P_FLAGSHAPE           78
#define P_FLAGCHILD           79
#define P_FLAGMOTHER          80
#define P_IFFLAG              81
#define P_TEXT                82
#define P_TRAIL               83
#define P_GOSUB               84
#define P_RETURN              85
#define P_DO                  86
#define P_NEXT                87
#define P_INEXT               88
#define P_BREAK               89
#define P_BREAKC              90
#define P_INVISIBLEON         91
#define P_INVISIBLEOFF        92
#define P_ALWAYS              93
#define P_ALWAYSOFF           94
#define P_FORCE               95
#define P_SPRITE              96
#define P_SETSTRAT            97
#define P_SETVBB              98
#define P_SETVBW              99
#define P_SETVWW             100
#define P_SETVWB             101
#define P_ADDVBB             102
#define P_ADDVBW             103
#define P_ADDVWW             104
#define P_ADDVWB             105
#define P_NEGB               106
#define P_NEGW               107
#define P_SETRANDOMB         108
#define P_SETRANDOMW         109
#define P_IFHITFLAG          110
#define P_COLLISIONSON       111
#define P_COLLISIONSOFF      112
#define P_START65816         113
#define P_IFNOT              114
#define P_PARTICLE           115
#define P_SHADOWON           116
#define P_SHADOWOFF          117
#define P_SOUND              118
#define P_SOUND2             119
#define P_QSPAWN             120
#define P_UNLINKCHILD        121
#define P_BEHINDPLAYER       122
#define P_IMPORTB            123
#define P_IMPORTW            124
#define P_EXPORTB            125
#define P_EXPORTW            126
#define P_DIV2B              127
#define P_DIV2W              128
#define P_INDEXB             129
#define P_INDEXW             130
#define P_PUSHB              131
#define P_PUSHW              132
#define P_PULLB              133
#define P_PULLW              134
#define P_WITHINRANGE        135
#define P_SHAPEINRANGE       136
#define P_MSGWITHMETER       137
#define P_DOQ                138
#define P_DOALVARB           139
#define P_DOALVARW           140
#define P_GOSUBALVAR         141
#define P_UNLINKSELF         142
#define P_BECOMECHILD        143
#define P_BECOMESHAPE        144
#define P_BECOMEMOTHER       145
#define P_UNBECOME           146
#define P_BECOME             147
#define P_REMOVECHILD        148
#define P_IFZEROB            149
#define P_IFZEROW            150
#define P_IFNOTZEROB         151
#define P_IFNOTZEROW         152
#define P_SET0B              153
#define P_SET0W              154
#define P_INCB               155
#define P_INCW               156
#define P_DECB               157
#define P_DECW               158
#define P_ADDROTX            159
#define P_ADDROTY            160
#define P_ADDROTZ            161
#define P_ADDWORLDX          162
#define P_ADDWORLDY          163
#define P_ADDWORLDZ          164
#define P_ADDWS              165
#define P_WAIT1              166
#define P_SCORE              167

// Number of opcodes in dispatch table.
#define P_NUM_OPCODES       168

// sflags2 bits used by path interpreter
#define PSFLAG1_RELZ     0x01  // Z-relative to player
#define PSFLAG2_GENVECS  0x02  // Always generate velocity vectors
#define PSFLAG3_HELI     0x04  // Helicopter mode
#define PSFLAG4_SPACE    0x08  // Space flight coupling
#define PSFLAG6_SMOKE    0x20  // Emit smoke particles

// Path data — global bytecode array loaded from ROM
extern const uint8 *g_path_data;
extern uint32 g_path_length;
extern const uint16 *g_path_offsets;
extern uint16 g_path_count;

typedef void (*PathInlineFunc)(struct Alien *self);

// Initialize path system
void Paths_Init(void);

// Load path data
void Paths_LoadData(const uint8 *data, uint32 length,
                    const uint16 *offsets, uint16 path_count);

void Paths_RegisterInlineCode(uint16 script_ptr, PathInlineFunc fn);

// Resolve a map/path ID to the bytecode start offset.
uint16 Paths_ResolveStart(uint16 path_id);

// Path strategy init (path_istrat) — called by map executor
void Strat_Path_Init(Alien *self);
void Strat_PathDha_Init(Alien *self);
void Strat_PathText_Init(Alien *self);

// Path strategy per-frame — called by Obj_RunStrategies
void Strat_Path_Tick(Alien *self);

#endif // STARFOX_PATH_PATHS_H
