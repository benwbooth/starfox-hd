// SOUND.ASM → C conversion
// Literal port of dosounds_l / playersnd / nearobjs / do_obstacles / setport3_l.

#include "sound.h"
#include "game_vars.h"
#include "obj.h"
#include "world.h"
#include "boot.h"
#include "../audio/audio.h"
#include "../audio/spc_player.h"
#include "../audio/spc_boot.h"
#include "../map/levels.h"
#include "../strat/strat_common.h"
#include "../sf_rtl.h"
#include "../variables.h"

#define SOUND_OBSDIST        100
#define SOUND_CUTOFFSND      3150
#define SOUND_DIST3SND       1150
#define SOUND_DIST2SND       650
#define SOUND_DIST1SND       250

#define SOUND_SHIP1_CUTOFF   11000
#define SOUND_SHIP1_DIST3    10000
#define SOUND_SHIP1_DIST2    10000
#define SOUND_SHIP1_DIST1    5000
#define SOUND_SHIP1_MINDIST  500

#define SOUND_PLAYACCEL1     2
#define SOUND_PLAYACCEL2     4
#define SOUND_PLAYACCEL3     8

#define SOUND_SE_DOPRIGHT    0x6D
#define SOUND_SE_DOPCENTRE   0x6E
#define SOUND_SE_DOPLEFT     0x6F

// ISTRATS def_shape indices used by SOUND.ASM force-sound path.
#define SOUND_SHAPE_SHIP_1   21
#define SOUND_SHAPE_SHIP_0_C 23
#define SOUND_SHAPE_SHIP_5   107
#define SOUND_SHAPE_SHIP_5M  109
#define SOUND_SHAPE_SHIP_5S  110

static void nearobjs(void);
static void do_obstacles(void);
static void playersnd(void);

static Alien *sound_get_player(void) {
    if (g_internalPLAYPT >= 0 && g_internalPLAYPT < NUMBER_AL) {
        Alien *al = &g_aliens[g_internalPLAYPT];
        if (al->active) return al;
    }
    return Obj_GetPlayer();
}

static uint16 sound_encode_alien(const Alien *al) {
    if (!al) return 0;
    int idx = (int)(al - g_aliens);
    if (idx < 0 || idx >= NUMBER_AL) return 0;
    return (uint16)(idx + 1);
}

static void sound_write_port1(uint8 value) {
    SpcPlayer_WritePort(1, value);
}

static void sound_write_port2(uint8 value) {
    SpcPlayer_WritePort(2, value);
}

static void sound_write_port3(uint8 value) {
    SpcPlayer_WritePort(3, value);
}

static void sound_set_port2_nothing(Alien *nearest) {
    sound_write_port2(0);
    g_lastblock = sound_encode_alien(nearest);
}

static uint8 sound_pan_from_angle(uint8 angle) {
    if (angle <= 124) return 0x40;
    if (angle <= 129) return 0x80;
    return 0xC0;
}

static bool sound_shape_matches_mapped(uint16 shape, uint8 shape_idx) {
    uint16 mapped = g_shapes_table[shape_idx];
    return (mapped != 0 && shape == mapped);
}

static bool sound_is_forcesnd_shape(uint16 shape) {
    if (shape == SOUND_SHAPE_SHIP_1 ||
        shape == SOUND_SHAPE_SHIP_0_C ||
        shape == SOUND_SHAPE_SHIP_5 ||
        shape == SOUND_SHAPE_SHIP_5M ||
        shape == SOUND_SHAPE_SHIP_5S) {
        return true;
    }

    return sound_shape_matches_mapped(shape, SOUND_SHAPE_SHIP_1) ||
           sound_shape_matches_mapped(shape, SOUND_SHAPE_SHIP_0_C) ||
           sound_shape_matches_mapped(shape, SOUND_SHAPE_SHIP_5) ||
           sound_shape_matches_mapped(shape, SOUND_SHAPE_SHIP_5M) ||
           sound_shape_matches_mapped(shape, SOUND_SHAPE_SHIP_5S);
}

// SFX dispatch with consumption handshake (IRQ.ASM startmus .trig3,
// lines 1612-1644): only send the next queued SFX once the SPC driver has
// echoed the previous one back on port 3.
static void sound_drain_port3_queue(void) {
    if (g_sdpck3 != 0) {
        if (SpcPlayer_ReadPort(3) != g_sdpck3)
            return;                     // .reject — SPC hasn't consumed it yet
        g_sdpck3 = 0;
        sound_write_port3(0);
    }

    if (g_pausesnd != 0) {              // .pause — flush queue, force command
        sound_write_port3(g_pausesnd);
        g_sdpck3 = g_pausesnd;
        g_sdspt3 = 0;
        g_sdgpt3 = 0;
        g_pausesnd = 0;
        return;
    }

    if (g_sdgpt3 == g_sdspt3) return;
    uint8 snd = g_sdport3[g_sdgpt3 & 0x0F];
    sound_write_port3(snd);
    g_sdpck3 = snd;
    g_sdgpt3 = (uint8)((g_sdgpt3 + 1) & 0x0F);
}

// ---------------------------------------------------------------------------
// Level-entry BGM boot (BGS.ASM `bgm N` / SOUND.ASM bootapu).
//
// The original boots the per-level sound bank (sndtbl row) from the BG init
// blocks in BGS.ASM when a level's background is set up; the map stream's
// later `setbgm` opcodes are raw driver song commands within that bank.
// The port's bgs.c has no music hooks, so we replicate the BGS boot here:
// Sound_Update (the dosounds_l tick, gameplay only) watches for level entry
// and boots the matching track.
// ---------------------------------------------------------------------------
static uint32 s_music_map = 0xFFFFFFFFu;   // map id music was booted for
static bool   s_music_booted = false;

// BGS.ASM bg_* -> `bgm N` mapping (ground/main variant per level).
// bg_x_1c uses `bgm 11` for all Corneria levels; the tunnel intro bank
// (`bgm 10`, sound5) is skipped because the port can't observe BG switches.
static uint8 sound_track_for_map(uint32 map_id) {
    switch (map_id) {
    case MAP_ID_1_1:
    case MAP_ID_2_1:
    case MAP_ID_3_1:       return SND_11;      // BGS.ASM:106/127 `bgm 11`
    case MAP_ID_1_2:       return SND_12;
    case MAP_ID_1_3:       return SND_13;
    case MAP_ID_1_4:       return SND_14;
    case MAP_ID_1_5:       return SND_15;
    case MAP_ID_1_6:       return SND_16;
    case MAP_ID_2_2:       return SND_22;
    case MAP_ID_2_3:       return SND_23;
    case MAP_ID_2_4:       return SND_24;
    case MAP_ID_2_5:       return SND_25;
    case MAP_ID_2_6:       return SND_26;
    case MAP_ID_3_2:       return SND_32;
    case MAP_ID_3_3:       return SND_33;
    case MAP_ID_3_4:       return SND_34;
    case MAP_ID_3_5:       return SND_35;
    case MAP_ID_3_6:       return SND_36;
    case MAP_ID_3_7:       return SND_37;
    case MAP_ID_BLACKHOLE: return SND_BHOLE;
    case MAP_ID_SPECIAL:   return SND_SPECIAL;
    case MAP_ID_TRAINING:  return SND_TRAINING;
    case MAP_ID_FINAL:     return SND_16;      // FINALMAP.ASM drives $12/$13
                                               // within the venom/andross bank
    default:               return 0xFF;        // no auto boot
    }
}

// bootapu: upload the sndtbl row and queue its start command (bgm_music).
static void sound_boot_track(uint8 track_id) {
    SpcPlayer_LoadTrack(track_id);
    SpcPlayer_StartBgm(SpcBoot_TrackCommand(track_id));
}

static void sound_level_music_tick(void) {
    // While the player is dead (death anim / game over / continue), drop the
    // boot latch so the respawn reboots the level bank — mirroring the
    // original's initlevel -> BGS `bgm N` on restart.
    if (g_gameflags & GF_PLAYERDEAD) {
        s_music_booted = false;
        return;
    }

    if (s_music_booted && s_music_map == g_newmap)
        return;

    s_music_map = g_newmap;
    s_music_booted = true;

    uint8 track = sound_track_for_map(g_newmap);
    if (track != 0xFF)
        sound_boot_track(track);
}

void Sound_Init(void) {
    // Keep SPC upload/handshake out of this pass.
    g_sdspt3 = 0;
    g_sdgpt3 = 0;
    g_sdpck3 = 0;
    g_pausesnd = 0;
    g_lastblock = 0;
    g_lastplayx = 0;
    g_port1bolox = 0;
    s_music_map = 0xFFFFFFFFu;
    s_music_booted = false;
    sound_write_port1(0);
    sound_write_port2(0);
}

void Sound_Update(void) {
    // BGS.ASM level-entry `bgm N` boot (see sound_level_music_tick above).
    sound_level_music_tick();

    // dosounds_l
    bool mute_port2 = false;

    if (g_gameflags2 & GF2_INGAME) {
        if (g_pshipflags2 & PSF2_PLAYERHP0) {
            mute_port2 = true;
        }
    }

    if (!mute_port2) {
        if (g_levelfinished == LE_ENTERSPEC ||
            g_levelfinished == LE_ENTERBHOLE ||
            g_levelfinished == LE_BHOLE1 ||
            g_levelfinished == LE_BHOLE2 ||
            g_levelfinished == LE_BHOLE3) {
            mute_port2 = true;
        }
    }

    if (!mute_port2) {
        nearobjs();
        do_obstacles();
    } else {
        sound_write_port2(0);
    }

    playersnd();
    sound_drain_port3_queue();
}

void Sound_Play(uint8 sound_id) {
    Audio_SendCommand(sound_id, 0);
}

void Sound_PlaySE(uint8 sound_id) {
    // setport3_l ring queue write.
    if (g_nosetport3) return;

    if (g_gameflags2 & GF2_INGAME) {
        if (g_pshipflags2 & PSF2_PLAYERHP0) return;
    }

    g_sdport3[g_sdspt3 & 0x0F] = sound_id;
    g_sdspt3 = (uint8)((g_sdspt3 + 1) & 0x0F);
}

void Sound_PlayMusic(uint8 music_id) {
    // During gameplay this matches the original exactly: map `setbgm` ops
    // (WORLD.ASM setbgmdo) and strat `startbgm` macros write a RAW driver
    // song command into bgm_music — 5 = boss, 7 = fanfare, $F0/$F1 = fades —
    // played from the bank booted at level entry.  No sbootapu reboot.
    if (g_game_state == GAME_STATE_PLAYING || music_id >= SND_TRACK_COUNT) {
        SpcPlayer_StartBgm(music_id);
        return;
    }

    // Outside gameplay the port's UI/map scripts pass sndtbl track ids
    // (e.g. the title map's setbgm 2 -> SND_TITLE): full bootapu semantics.
    sound_boot_track(music_id);
}

void Sound_StopMusic(void) {
    // startbgm 0 — tell the driver to stop the current song
    SpcPlayer_StartBgm(0);
}

static void playersnd(void) {
    Alien *player = sound_get_player();
    if (!player || (player->collflags & ACF_FIRSTFRAME)) {
        sound_write_port1(0);
        g_port1bolox = 0;
        return;
    }

    if (g_gameflags & GF_PLAYERDEAD) {
        sound_write_port1(0);
        g_port1bolox = 0;
        return;
    }

    if ((g_pshipflags3 & PSF3_ENGINESND) == 0) {
        sound_write_port1(0);
        g_port1bolox = 0;
        return;
    }

    if (g_pshipflags2 & PSF2_PLAYERHP0) {
        sound_write_port1(0x4B);
        g_port1bolox = 0x4B;
        return;
    }

    uint8 tpa;
    if (g_inatunnel != 0) {
        tpa = (g_inatunnel == 2) ? 0xC0 : 0x80;
    } else if (g_game_mode & SPACE_MODE) {
        tpa = 0x00;
    } else {
        tpa = 0xC0;
    }

    tpa = (uint8)(tpa | g_playersndflag);

    uint8 contl0 = (uint8)(g_pad1 & 0xFF);
    if (contl0 & (PAD_TLEFT | PAD_TRIGHT)) {
        int16 accel = (int16)(player->worldx - g_lastplayx);
        g_lastplayx = player->worldx;
        if (accel < 0) accel = (int16)-accel;

        if (accel < SOUND_PLAYACCEL1) {
            tpa |= 0x00;
        } else if (accel < SOUND_PLAYACCEL2) {
            tpa |= 0x01;
        } else if (accel < SOUND_PLAYACCEL3) {
            tpa |= 0x02;
        } else {
            tpa |= 0x03;
        }
    }

    g_tpa = tpa;
    sound_write_port1(tpa);
    g_port1bolox = tpa;
}

static void do_obstacles(void) {
    Alien *player = sound_get_player();
    if (!player) return;

    for (Alien *al = g_active_list; al != NULL; al = al->next) {
        if (al->HP != 0xFF) continue;
        if ((al->sflags3 & ASF3_REALOBJ) == 0) continue;
        if (al->sflags4 & ASF4_DONESND) continue;

        int16 dx = (int16)(g_pviewposx - al->worldx);
        if (dx >= 0) {
            if (dx >= 300) continue;
        } else {
            if (dx < -300) continue;
        }

        int16 dz = (int16)(al->worldz - player->worldz);
        if (dz < 0 || dz >= SOUND_OBSDIST) continue;

        al->sflags4 |= ASF4_DONESND;

        if (dx >= 0) {
            if (dx < 80) {
                Sound_PlaySE(SOUND_SE_DOPCENTRE);
            } else {
                Sound_PlaySE(SOUND_SE_DOPRIGHT);
            }
        } else {
            if (dx >= -80) {
                Sound_PlaySE(SOUND_SE_DOPCENTRE);
            } else {
                Sound_PlaySE(SOUND_SE_DOPLEFT);
            }
        }
        return;
    }
}

static void nearobjs_forcesnd(Alien *al, Alien *player) {
    uint8 snd = al->snd2;
    g_lastblock = sound_encode_alien(al);

    int16 range = (int16)(al->worldz - player->worldz);
    if (range < 0) {
        sound_write_port2(0);
        return;
    }

    int16 dx = (int16)(g_pviewposx - al->worldx);
    if (dx >= 0) {
        snd |= (dx < 80) ? 0x80 : 0x40;
    } else {
        snd |= (dx >= -80) ? 0x80 : 0xC0;
    }

    if (range < SOUND_SHIP1_MINDIST) {
        sound_write_port2(0);
        return;
    }
    if (range < SOUND_SHIP1_DIST1) {
        snd |= 0x00;
    } else if (range < SOUND_SHIP1_DIST2) {
        snd |= 0x10;
    } else if (range < SOUND_SHIP1_DIST3) {
        snd |= 0x20;
    } else if (range < SOUND_SHIP1_CUTOFF) {
        snd |= 0x30;
    } else {
        sound_write_port2(0);
        return;
    }

    sound_write_port2(snd);
}

static void nearobjs(void) {
    Alien *player = sound_get_player();
    if (!player) {
        sound_set_port2_nothing(NULL);
        return;
    }

    int16 nearest_range = 0x7FFF;
    Alien *nearest = NULL;

    for (Alien *al = g_active_list; al != NULL; al = al->next) {
        if (sound_is_forcesnd_shape(al->shape)) {
            nearobjs_forcesnd(al, player);
            return;
        }

        if (al->flags & AFEXP) continue;
        if (al->snd1 == 0 && al->snd2 == 0) continue;

        int16 range = (int16)(al->worldz - player->worldz);
        if (range < 0) range = (int16)-range;

        if (nearest_range < range) continue;
        nearest = al;
        nearest_range = range;
    }

    if (!nearest) {
        sound_set_port2_nothing(NULL);
        return;
    }

    uint16 nearest_code = sound_encode_alien(nearest);
    if (nearest_code != g_lastblock) {
        sound_set_port2_nothing(nearest);
        return;
    }

    if (nearest->snd1 != 0) {
        sound_write_port2(nearest->snd1);
        return;
    }

    uint8 snd = nearest->snd2;
    uint8 angle = Strat_AngleXZ(player, nearest);
    snd = (uint8)(snd | sound_pan_from_angle(angle));

    if (nearest_range < SOUND_DIST1SND) {
        snd |= 0x00;
    } else if (nearest_range < SOUND_DIST2SND) {
        snd |= 0x10;
    } else if (nearest_range < SOUND_DIST3SND) {
        snd |= 0x20;
    } else if (nearest_range >= SOUND_CUTOFFSND) {
        sound_set_port2_nothing(nearest);
        return;
    } else {
        snd |= 0x30;
    }

    sound_write_port2(snd);
}
