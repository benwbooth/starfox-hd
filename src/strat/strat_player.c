// PSTRATS.ASM -> C conversion
// Player (Arwing) strategy: controls, movement, weapons, barrel roll.
//
// This is a direct, flat-memory translation of the core do_player_* flow:
// playermove_srou -> gen_3dvecs -> mode velocity transform -> add_vecs2pos
// -> playerlimitx_srou -> playerfire_srou -> checkarrows_srou -> viewmove_srou.

#include "strat_player.h"
#include "strat_common.h"
#include "../game/game_vars.h"
#include "../game/sound.h"
#include "../game/windows.h"
#include "../sf_rtl.h"
#include "../variables.h"
#include "../renderer/shapes.h"

// --- Internal rotation state (16-bit fixed-point, 8.8 format) ---
// These are plrotx/plroty/plrotz in PSTRATS.ASM.
static int16 s_plrotx = 0;
static int16 s_plroty = 0;
static int16 s_plrotz = 0;

// Rotation speeds (PSTRATS.ASM)
#define XROT_SPEED  0x200
#define ZROT_SPEED  0x200

// Speed constants (STRATEQU.INC:345-348)
#define MIN_PSPEED   20
#define MED_PSPEED   65
#define MAX_PSPEED   85

// Table lengths (byte count for pZrotfloattab, byte count for viewfloattab)
// pZrotfloattab: 28 db entries = 28 bytes
// viewfloattab:  36 dw entries = 72 bytes (index increments by 2)
#define PZROTFLOATTAB_LEN  28
#define VIEWFLOATTAB_BYTELEN  72

// Ltunnel fly mode constants (STRATEQU.INC)
#define LTUNNEL_VIEWCY       (-60)
#define LTUNNEL_MINX         (-120)
#define LTUNNEL_MAXX         120
#define LTUNNEL_MMINX        (-120)
#define LTUNNEL_MMAXX        120
#define LTUNNEL_MMAXY        (60 + LTUNNEL_VIEWCY)
#define LTUNNEL_MINY         (-60 + LTUNNEL_VIEWCY)
#define LTUNNEL_MAXY         (60 + LTUNNEL_VIEWCY)
#define PLAYERB_YSTOP        (-20)

// Weapon offsets/constants (STRATEQU.INC)
#define PLAYER_W_X           33
#define PLAYER_W_Y           13
#define PLAYER_W_X_SCALED    (PLAYER_W_X >> 2)
#define PLAYER_W_Y_SCALED    (PLAYER_W_Y >> 2)
#define INVIEW_LASER_Y_OFF   50
#define NUKE_Z_OFFSET        (80 >> 2)
#define PLAYER_BODY_HP       40
#define PLAYER_HITFLASH_FRMS 7
#define SCREENFLASH_BODY_FRMS 4
#define SCREENFLASH_BODY_TYPE 0
#define PLAYER_DEAD_FRAMES   60
#define SHIPINTRO_LIFE       40u
#define SHIPINTRO_BOOST_Z    50

static const int8 s_shipintro_rotz_float[] = {
    0,
    1, 2, 3, 4, 4, 5, 5, 5, 4, 4, 3, 2, 1,
    0,
    -1, -2, -3, -4, -4, -5, -5, -5, -4, -4, -3, -2, -1
};

static const int16 s_shipintro_view_float[] = {
    0,
    1, 2, 3, 4, 4, 5, 5, 6, 6, 6, 5, 5, 4, 4, 3, 2, 1,
    0,
    -1, -2, -3, -4, -4, -5, -5, -6, -6, -6, -5, -5, -4, -4, -3, -2, -1
};

static int16 clamp16(int16 val, int16 lo, int16 hi) {
    if (val < lo) return lo;
    if (val > hi) return hi;
    return val;
}

static void playerdead_istrat(Alien *self);

static int16 player_obj_index_or_null(const Alien *self) {
    if (!self || self < g_aliens || self >= (g_aliens + NUMBER_AL)) {
        return 0;
    }
    return (int16)(self - g_aliens);
}

static void shipintro_dec_lifecnt(Alien *self) {
    self->count = (uint8)(self->count - 1u);
    if (self->count == 0u) {
        Strat_RemoveObj();
    }
}

static void shipintro_float(Alien *self) {
    self->sbyte3 = (uint8)(self->sbyte3 + 1u);
    if (self->sbyte3 >= (uint8)(sizeof(s_shipintro_rotz_float) / sizeof(s_shipintro_rotz_float[0]))) {
        self->sbyte3 = 0u;
    }
    self->rotz = (uint8)s_shipintro_rotz_float[self->sbyte3];

    self->sbyte4 = (uint8)(self->sbyte4 + 1u);
    if (self->sbyte4 >= (uint8)(sizeof(s_shipintro_view_float) / sizeof(s_shipintro_view_float[0]))) {
        self->sbyte4 = 0u;
    }
    self->worldy = s_shipintro_view_float[self->sbyte4];
}

static void shipintro_strat(Alien *self) {
    if (!self) {
        return;
    }

    if ((g_gameflags & GF_STRATDONE2) != 0u) {
        if ((int8)self->sbyte1 < 0) {
            shipintro_dec_lifecnt(self);
        } else {
            self->sbyte1 = (uint8)(self->sbyte1 - 1u);
            if (self->sbyte1 == 0u) {
                self->sbyte1 = 1u;
                self->worldz = (int16)(self->worldz + SHIPINTRO_BOOST_Z);
                shipintro_dec_lifecnt(self);
            }
        }
    }

    if (self->sbyte1 == 2u) {
        g_boostobj = player_obj_index_or_null(self);
        Sound_PlaySE(0x32u);
    }

    shipintro_float(self);
    self->worldy = (int16)(self->worldy + self->sword1);
    self->worldz = (int16)(self->worldz + MED_PSPEED);
}

void Strat_ShipIntro_Init(Alien *self) {
    if (!self) {
        return;
    }

    self->stratptr = shipintro_strat;
    self->sbyte3 = (uint8)(SfRtl_Random() & 15u);
    self->sbyte4 = (uint8)(SfRtl_Random() & 7u);
    self->sbyte2 = (uint8)(self->sbyte2 << 1);
    g_gameflags &= (uint8)~GF_STRATDONE2;
    self->sflags |= ASF_SHADOW;
    self->count = SHIPINTRO_LIFE;
    self->type &= (uint8)~ATZREMOVE;
    shipintro_strat(self);
}

static void player_hitflash_update(Alien *self) {
    if (self->sbyte1 == 0) {
        self->sflags &= (uint8)~ASF_NOHITAFFECT;
        g_viewshakeX = 0;
        g_viewshakeY = 0;
        g_viewshakeZ = 0;
        return;
    }

    self->sbyte1--;

    // s_set_var2rnd viewshake{X,Y,Z},#15 / -7
    g_viewshakeX = (uint8)(((SfRtl_Random() & 0x0F) - 7) & 0xFF);
    g_viewshakeY = (uint8)(((SfRtl_Random() & 0x0F) - 7) & 0xFF);
    g_viewshakeZ = (uint8)(((SfRtl_Random() & 0x0F) - 7) & 0xFF);

    // Toggle hitflash every other frame while invulnerable.
    if ((g_gameframe & 1u) == 0u) {
        self->sflags |= ASF_HITFLASH;
        self->sflags |= ASF_NOHITAFFECT;
    }
}

static void playercoll_istrat(Alien *self) {
    if ((g_pshipflags3 & PSF3_NOCOLLISIONS) != 0) {
        self->sflags &= (uint8)~ASF_COLLIDE;
        return;
    }

    if ((self->sflags & ASF_NOHITAFFECT) != 0) {
        self->sflags &= (uint8)~ASF_COLLIDE;
        return;
    }

    if (g_pnumhits < 0xFF) {
        g_pnumhits++;
    }

    self->sbyte1 = PLAYER_HITFLASH_FRMS;
    self->sflags |= ASF_HITFLASH;
    self->sflags |= ASF_NOHITAFFECT;
    g_screenflashcnt = SCREENFLASH_BODY_FRMS;
    g_screenflashtype = SCREENFLASH_BODY_TYPE;

    if (self->HP == 0) {
        playerdead_istrat(self);
    }

    self->sflags &= (uint8)~ASF_COLLIDE;
}

static void playerdead_strat(Alien *self) {
    if ((g_gameflags & GF_PLAYERDEAD) != 0) {
        return;
    }

    if ((g_gameframe & 1u) == 0u && g_player_speed > 0) {
        g_player_speed = (int16)Strat_Chase(g_player_speed, 0, 1);
    }
    self->vel = (uint8)clamp16(g_player_speed, 0, MAX_PSPEED);

    g_player_zstratadd = (uint8)(g_player_zstratadd + 4);
    self->sflags |= ASF_COLLDISABLE;
    self->sflags &= (uint8)~ASF_SHADOW;

    if (self->sbyte1 < 0xFF) {
        self->sbyte1++;
    }
    if (self->sbyte1 < PLAYER_DEAD_FRAMES) {
        return;
    }

    g_gameflags &= (uint8)~GF_PLAYERDYING;
    g_gameflags |= GF_PLAYERDEAD;
    if (g_lives > 0) {
        g_lives--;
    }
}

static void playerdead_istrat(Alien *self) {
    if ((g_pshipflags2 & PSF2_PLAYERHP0) != 0) {
        return;
    }

    g_pshipflags2 |= PSF2_PLAYERHP0;
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags |= PSTF_INSEQ;
    g_pshipflags3 &= (uint8)~PSF3_ENGINESND;
    g_gameflags |= GF_PLAYERDYING;
    g_gameflags &= (uint8)~GF_PLAYERDEAD;

    // Mirror playerdead_Istrat: transition to a dedicated death state object.
    self->HP = 10;
    self->AP = 0;
    self->sbyte1 = 0;
    self->sflags &= (uint8)~ASF_COLLIDE;
    self->stratptr = playerdead_strat;
    self->collstratptr = playercoll_istrat;
    self->expstratptr = playerdead_istrat;

    // Trigger a downed/explosion cue.
    Sound_PlaySE(0x03);
}

static void setcurrpshape(Alien *self) {
    uint8 damage = g_pshipflags & (PSF_BRKLWING | PSF_BRKRWING);

    if (damage == 0) {
        self->shape = g_playershape ? g_playershape : SHAPE_ARWING;
    } else if (damage == PSF_BRKLWING) {
        self->shape = g_playershapeL ? g_playershapeL : SHAPE_ARWING;
    } else if (damage == PSF_BRKRWING) {
        self->shape = g_playershapeR ? g_playershapeR : SHAPE_ARWING;
    } else {
        self->shape = g_playershapeLR ? g_playershapeLR : SHAPE_ARWING;
    }
}

// Barrel roll port from playermove_srou (.nroll/.zroll blocks).
static void barrel_roll_update(bool allow_start) {
    uint16 lr_mask = (PAD_LEFT | PAD_RIGHT | PAD_TLEFT | PAD_TRIGHT);
    bool lr_down = (g_pad1 & lr_mask) != 0;
    bool lr_prev = (g_pad1_prev & lr_mask) != 0;

    if ((int8)g_player_rollZvel == 0) {
        if (g_player_rolldelay > 0) {
            g_player_rolldelay--;
        }

        if (allow_start && g_player_rolldelay == 0 && lr_down && !lr_prev) {
            if ((g_pad1 & (PAD_RIGHT | PAD_TRIGHT)) != 0) {
                g_player_rollZvel = 32;
            } else {
                g_player_rollZvel = (uint8)-32;
            }
            g_player_rollZoff = 0;
        }

        if (lr_down) {
            g_player_rolldelay = BARREL_ROLL_DELAY;
        }

        g_player_rollZoff = (uint8)Strat_ChaseProportional((int8)g_player_rollZoff, 0, 3);
        return;
    }

    g_player_rollZoff = (uint8)((int8)g_player_rollZoff + (int8)g_player_rollZvel);
    if ((int8)g_player_rollZvel < 0) {
        g_player_rollZvel = (uint8)((int8)g_player_rollZvel + 2);
    } else {
        g_player_rollZvel = (uint8)((int8)g_player_rollZvel - 2);
    }
}

// Boost/brake port from playermove_srou boost section.
static void boost_brake_update(Alien *self) {
    if (self->sbyte2 > 0) {
        self->sbyte2--;
        if (self->sbyte2 == 0) {
            g_pshipflags2 &= (uint8)~(PSF2_BOOSTING | PSF2_BRAKING);
        }
    }

    if ((g_pshipflags2 & PSF2_FORCEBOOST) != 0) {
        g_pshipflags2 &= (uint8)~PSF2_FORCEBOOST;
        g_pshipflags2 |= PSF2_BOOSTING;
        g_boostcnt = 1;
        self->vel = MAX_PSPEED;
        g_player_tospeed = MAX_PSPEED;
        self->sbyte2 = 20;
        Sound_PlaySE(0x32);
        return;
    }

    if ((g_pshipflags3 & PSF3_FORCEBRAKE) != 0) {
        g_pshipflags3 &= (uint8)~PSF3_FORCEBRAKE;
        g_pshipflags2 |= PSF2_BRAKING;
        g_boostcnt = 1;
        g_player_tospeed = MIN_PSPEED;
        self->sbyte2 = 30;
        Sound_PlaySE(0x33);
        return;
    }

    if ((g_pad1 & PAD_X) != 0) {
        g_pshipflags2 |= PSF2_BOOSTING;
        g_boostcnt = 1;
        self->vel = MAX_PSPEED;
        g_player_tospeed = MAX_PSPEED;
        self->sbyte2 = 20;
        Sound_PlaySE(0x32);
        return;
    }

    if ((g_pad1 & PAD_B) != 0) {
        g_pshipflags2 |= PSF2_BRAKING;
        g_boostcnt = 1;
        g_player_tospeed = MIN_PSPEED;
        self->sbyte2 = 30;
        Sound_PlaySE(0x33);
    }
}

// playermove_srou literal flow (core control + rotation compose path).
static void playermove_srou(Alien *self) {
    player_hitflash_update(self);

    bool no_ctrl = (g_pshipflags & PSF_NOCTRL) != 0 ||
                   g_stayblack != -1 ||
                   g_doingwipe != 0;

    if (g_player_noctrlcnt > 0) {
        g_player_noctrlcnt--;
        no_ctrl = true;
    }

    if (!no_ctrl) {
        if ((g_pad1 & PAD_LEFT) != 0) {
            s_plrotz += ZROT_SPEED;
            s_plroty += ZROT_SPEED;
            g_player_Ztilt = (uint8)((int8)g_player_Ztilt + (DEG45 / 15));
            if ((int8)g_player_Ztilt > (DEG90)) g_player_Ztilt = DEG90;
        }
        if ((g_pad1 & PAD_RIGHT) != 0) {
            s_plrotz -= ZROT_SPEED;
            s_plroty -= ZROT_SPEED;
            g_player_Ztilt = (uint8)((int8)g_player_Ztilt - (DEG45 / 15));
            if ((int8)g_player_Ztilt < -(DEG90)) g_player_Ztilt = (uint8)-(DEG90);
        }

        if ((g_pshipflags & PSF_NOYCTRL) == 0) {
            if ((g_pad1 & PAD_UP) != 0) {
                s_plrotx += XROT_SPEED;
            }
            if ((g_pad1 & PAD_DOWN) != 0) {
                s_plrotx -= XROT_SPEED;
            }
        }
    }

    barrel_roll_update(!no_ctrl);

    g_player_Ztilt = (uint8)Strat_ChaseProportional((int8)g_player_Ztilt, 0, 3);
    s_plroty = Strat_ChaseProportional(s_plroty, 0, 3);
    s_plrotz = Strat_ChaseProportional(s_plrotz, 0, 4);
    s_plrotx = Strat_ChaseProportional(s_plrotx, 0, 3);

    self->vel = (uint8)clamp16(self->vel, MIN_PSPEED, MAX_PSPEED);
    s_plrotz = clamp16(s_plrotz, -0x600, 0x600);

    self->rotx = (uint8)(s_plrotx >> 8);
    self->roty = (uint8)((s_plroty >> 8) + (g_player_turnrot >> 8));
    self->rotz = (uint8)((s_plrotz >> 8)
                       + (int8)g_player_Ztilt
                       + (g_player_zshake >> 8)
                       + (int8)g_player_zstratadd
                       + (int8)g_player_rollZoff);

    g_hudrot = (int16)(int8)self->rotz;
    g_player_speed = self->vel;
    g_pmovelimit = 0;

    setcurrpshape(self);
}

// playerlimitx_srou
static void playerlimitX_srou(Alien *self) {
    g_arrows &= (uint8)~(SPRAR_RIGHT | SPRAR_LEFT);

    if (self->worldx < g_minpmoveX) {
        self->worldx = g_minpmoveX;
        g_arrows |= SPRAR_LEFT;
    }
    if (self->worldx > g_maxpmoveX) {
        self->worldx = g_maxpmoveX;
        g_arrows |= SPRAR_RIGHT;
    }

    // Keep vertical gameplay stable in the HD runtime.
    if (self->worldy < g_minpmoveY) {
        self->worldy = g_minpmoveY;
        g_arrows |= SPRAR_UP;
    }
    if (self->worldy > g_maxpmoveY) {
        self->worldy = g_maxpmoveY;
        g_arrows |= SPRAR_DOWN;
    }
}

// checkarrows_srou
static void checkarrows_srou(void) {
    if ((g_pstratflags & PSTF_INSEQ) != 0) {
        g_arrows = 0;
        return;
    }

    if ((g_pad1 & PAD_DOWN) == 0 || (g_pmovelimitAND & PML_BTOP) == 0) {
        g_arrows &= (uint8)~SPRAR_UP;
    }
    if ((g_pad1 & PAD_UP) == 0 || (g_pmovelimitAND & PML_BBOTTOM) == 0) {
        g_arrows &= (uint8)~SPRAR_DOWN;
    }
    if ((g_pad1 & PAD_LEFT) == 0 || (g_pmovelimitAND & PML_LWLEFT) == 0) {
        g_arrows &= (uint8)~SPRAR_LEFT;
    }
    if ((g_pad1 & PAD_RIGHT) == 0 || (g_pmovelimitAND & PML_RWRIGHT) == 0) {
        g_arrows &= (uint8)~SPRAR_RIGHT;
    }
}

static Alien *spawn_player_projectile(Alien *self,
                                      int16 off_x, int16 off_y, int16 off_z,
                                      uint8 speed, uint8 lifetime,
                                      uint8 ap,
                                      bool track_in_numplasers,
                                      bool inside_beam_extra_z) {
    int16 dx = off_x;
    int16 dy = off_y;
    int16 dz = off_z;

    if (g_splayerflymode == SPFM_INSIDE) {
        dx += (int16)(g_pviewposx - self->worldx);
        dy += (int16)((g_pviewposy + INVIEW_LASER_Y_OFF) - self->worldy);
        dz += (int16)(g_pviewposz - self->worldz);
        if (inside_beam_extra_z) {
            dz += 200;
        }
    }

    Alien *shot = Strat_SpawnProjectile(self,
                                        dx, dy, dz,
                                        self->rotx, self->roty,
                                        speed, lifetime, ap,
                                        ACF_COLLTYPE1);
    if (!shot) {
        return NULL;
    }

    if (track_in_numplasers) {
        shot->sbyte6 |= 1;
        if (g_numplasers < 0xFF) {
            g_numplasers++;
        }
    }

    return shot;
}

// playerfire_srou
static void playerfire_srou(Alien *self) {
    if ((g_pshipflags & (PSF_BRKLWING | PSF_BRKRWING)) != 0) {
        g_pshipflags2 &= (uint8)~PSF2_DOUBLASER;
        g_pshipflags3 &= (uint8)~PSF3_BEAMBALL;
    }

    if ((g_pshipflags & PSF_NOFIRE) != 0) {
        return;
    }
    if (g_stayblack != -1 || g_doingwipe != 0) {
        return;
    }

    // A button = nova bomb
    bool can_use_special = true;
    if (g_specialdelay > 0) {
        g_specialdelay--;
        can_use_special = (g_specialdelay == 0);
    }

    if (can_use_special && (g_pad1_new & PAD_A) != 0 && g_specwepcnt > 0) {
        g_specialdelay = SPECIAL_DELAY_FRMS;
        g_specwepcnt--;

        Alien *nuke = spawn_player_projectile(self,
                                              0, 0, NUKE_Z_OFFSET,
                                              56, 80, 8,
                                              false,
                                              false);
        if (nuke) {
            nuke->sbyte4 = 2;
        }
        Sound_PlaySE(0x31);
    }
    if (g_specialdelay == 0) {
        g_specialdelay = 1;
    }

    // Y button = laser/beam
    if ((g_pad1 & PAD_Y) == 0) {
        g_firecnt = 3;
        g_firedelay = 1;
        return;
    }

    if (g_firedelay > 0) {
        g_firedelay--;
        return;
    }
    g_firedelay = PLAYER_FIRESPEED;

    if (g_firecnt == 0) {
        return;
    }
    g_firecnt--;

    bool beam = (g_pshipflags3 & PSF3_BEAMBALL) != 0;
    bool dbl = (g_pshipflags2 & PSF2_DOUBLASER) != 0;
    uint8 max_active = (beam || dbl) ? 7 : 3;
    if (g_numplasers > max_active) {
        return;
    }

    if (beam) {
        (void)spawn_player_projectile(self,
                                      -PLAYER_W_X_SCALED, PLAYER_W_Y_SCALED, 10,
                                      64, 55, 4,
                                      true,
                                      true);
        (void)spawn_player_projectile(self,
                                      PLAYER_W_X_SCALED, PLAYER_W_Y_SCALED, 10,
                                      64, 55, 4,
                                      true,
                                      true);
        Sound_PlaySE(0x36);
        return;
    }

    if (dbl) {
        (void)spawn_player_projectile(self,
                                      -PLAYER_W_X_SCALED, PLAYER_W_Y_SCALED, 80,
                                      80, 45, 2,
                                      true,
                                      false);
        (void)spawn_player_projectile(self,
                                      PLAYER_W_X_SCALED, PLAYER_W_Y_SCALED, 80,
                                      80, 45, 2,
                                      true,
                                      false);
        Sound_PlaySE(0x34);
        return;
    }

    (void)spawn_player_projectile(self,
                                  0, 0, 80,
                                  80, 45, 2,
                                  true,
                                  false);
    Sound_PlaySE(0x60);
}

static void framescalevecs(Alien *self) {
    // The SNES path scales vectors when framerate drops.
    // Runtime is currently fixed-step, so this is a no-op.
    (void)self;
}

static void do_player_limitX(Alien *self) {
    playermove_srou(self);
    Strat_GenVecs3D(self);
    framescalevecs(self);
    Strat_ApplyVelocity(self);
    playerlimitX_srou(self);
    playerfire_srou(self);
    checkarrows_srou();
}

static void do_player_yvel125(Alien *self) {
    playermove_srou(self);
    Strat_GenVecs3D(self);

    int16 vy = self->vy;
    self->vy = (int16)(vy + (vy >> 2));

    framescalevecs(self);
    Strat_ApplyVelocity(self);
    playerlimitX_srou(self);
    playerfire_srou(self);
    checkarrows_srou();
}

static void do_player_yvelD2(Alien *self) {
    playermove_srou(self);
    Strat_GenVecs3D(self);

    self->vx = Strat_Perc62(self->vx);
    self->vy = (int16)(self->vy >> 1);

    framescalevecs(self);
    Strat_ApplyVelocity(self);
    playerlimitX_srou(self);
    playerfire_srou(self);
    checkarrows_srou();
}

// viewmove_srou (Z chase/speed logic)
static void viewmove_srou(Alien *self) {
    if ((g_pstratflags & PSTF_NOVIEWMOVE) != 0) {
        g_bgsscrollZ = g_pviewposz;
        return;
    }

    g_pviewposz += g_pviewvelz;
    g_pviewvelz = Strat_Chase(g_pviewvelz, self->vz, 1);

    int16 z_diff = (int16)(g_pviewposz - g_player_posz);
    z_diff = clamp16(z_diff, -200, 50);
    g_pviewposz = (int16)(g_player_posz + z_diff);

    if (self->sbyte2 <= 10) {
        g_player_tospeed = g_player_medspeed;
        g_pviewposz = Strat_ChaseProportional(g_pviewposz, g_player_posz, 3);
    }

    (void)Strat_SpeedTo(self, g_player_tospeed, 2);

    if (g_splayerflymode == SPFM_INSIDE) {
        g_pviewposx = self->worldx;
        g_pviewposy = self->worldy;
        g_pviewposz = self->worldz;
    }

    g_bgsscrollZ = g_pviewposz;
}

static void update_viewxy_for_mode(Alien *self) {
    if (g_game_mode == SPACE_MODE) {
        if (g_splayerflymode == SPFM_INSIDE) {
            g_pviewposx = self->worldx;
            g_pviewposy = self->worldy;
            return;
        }

        g_pviewposx = Strat_Perc75(self->worldx);
        g_pviewposy = (int16)(Strat_Perc62((int16)(self->worldy - g_viewCY)) + g_viewCY);
        return;
    }

    g_pviewposx = Strat_Perc87(self->worldx);
    g_pviewposy = (int16)(Strat_Perc75((int16)(self->worldy - g_viewCY)) + g_viewCY);
}

void Strat_Player(Alien *self) {
    if (self->HP == 0) {
        playerdead_istrat(self);
    }

    if ((g_gameflags & GF_PLAYERDYING) != 0 || (g_gameflags & GF_PLAYERDEAD) != 0) {
        if (self->stratptr == playerdead_strat) {
            playerdead_strat(self);
        }
        return;
    }

    boost_brake_update(self);

    if (g_game_mode == SPACE_MODE) {
        do_player_limitX(self);
    } else if ((g_playerflymode & PFM_DIEFALL) != 0 || (g_playerflymode & PFM_DIEYROT) != 0) {
        do_player_yvelD2(self);
    } else {
        do_player_yvel125(self);
    }

    update_viewxy_for_mode(self);
    viewmove_srou(self);
}

Alien *Strat_SpawnPlayer(void) {
    Alien *player = Obj_Alloc();
    if (!player) return NULL;

    player->shape = SHAPE_ARWING;
    player->stratptr = Strat_Player;
    player->worldx = 0;
    player->worldy = 0;
    player->worldz = 0;
    player->vel = MED_PSPEED;
    player->HP = PLAYER_BODY_HP;
    player->AP = 0;
    player->type = 0;
    player->sflags = ASF_SHADOW;
    player->sflags4 = ASF4_PLAYEROBJ;
    player->collstratptr = playercoll_istrat;
    player->expstratptr = playerdead_istrat;
    player->endcollstratptr = NULL;
    player->animframe = 0xFF;
    player->colframe = 0xFF;

    s_plrotx = 0;
    s_plroty = 0;
    s_plrotz = 0;

    g_player_tospeed = MED_PSPEED;
    g_player_medspeed = MED_PSPEED;
    g_player_speed = MED_PSPEED;
    g_playervelZ = MED_PSPEED;
    g_pviewvelz = MED_PSPEED;
    g_pviewposx = 0;
    g_pviewposy = 0;
    g_pviewposz = 0;

    g_player_turnrot = 0;
    g_player_Ztilt = 0;
    g_player_zshake = 0;
    g_player_zstratadd = 0;
    g_player_rollZvel = 0;
    g_player_rollZoff = 0;
    g_player_rolldelay = 0;
    g_player_noctrlcnt = 0;
    g_viewCY = 0;

    g_numplasers = 0;
    g_firecnt = 3;
    g_firedelay = 1;
    g_specialdelay = 1;
    g_specwepcnt = DEFAULT_NUKE_COUNT;
    g_pnumhits = 0;
    g_arrows = 0;

    g_playershape = SHAPE_ARWING;
    g_playershapeL = SHAPE_ARWING;
    g_playershapeR = SHAPE_ARWING;
    g_playershapeLR = SHAPE_ARWING;

    g_internalPLAYPT = (int16)(player - g_aliens);

    return player;
}

// ============================================================
// playerExitBase (PISTRATS.ASM:620-720) — full hangar launch port.
//
// set_playerExitBase_l parks the invisible player in the hangar with a
// fixed camera in front of the base (viewtype fpos|toobj), holds for
// psvar_byte1 ticks (wait), launches at pexitbasespeed (Go — ship becomes
// visible flying out of the hangar towards/past the camera), then the
// camera chases and re-attaches behind the ship (Follow) and normal
// planet flight resumes, spawning the 4th wingman (friendstart3).
// ============================================================

#define PEXITBASE_SPEED  50           // STRATEQU.INC:350
#define MYBASE_SCALE     3            // STRATEQU.INC:305

// exitbase fly mode (STRATEQU.INC:581-597)
#define EXITBASE_VIEWCY  (-50)
#define EXITBASE_MINX    (-500)
#define EXITBASE_MAXX    500
#define EXITBASE_MMINX   (-500)
#define EXITBASE_MMAXX   500
#define EXITBASE_MINY    (-600)
#define EXITBASE_MAXY    0
#define EXITBASE_MMAXY   0

static void playerExitBasewait_strat(Alien *self);
static void playerExitBaseGo_init(Alien *self);
static void playerExitBaseGo_strat(Alien *self);
static void playerExitBaseFollow_init(Alien *self);
static void playerExitBaseFollow_strat(Alien *self);
static void friendstart3_Istrat(Alien *self);

// s_playerfly_mode exitbase (STRATEQU.INC:581-603 + STRATMAC s_playerfly_mode)
static void playerflymode_exitbase(Alien *self) {
    g_viewCY = EXITBASE_VIEWCY;
    g_minpmoveX = EXITBASE_MINX;
    g_maxpmoveX = EXITBASE_MAXX;
    g_minMmoveX = EXITBASE_MMINX;
    g_maxMmoveX = EXITBASE_MMAXX;
    g_maxMmoveY = EXITBASE_MMAXY;
    g_minpWmoveY = EXITBASE_MINY;
    g_maxpWmoveY = EXITBASE_MAXY;
    g_minpmoveY = EXITBASE_MINY;
    g_maxpmoveY = (int16)(EXITBASE_MAXY + PLAYERB_YSTOP);
    g_playerflymode = (PFM_DIEFALL | PFM_DIEYROT | PFM_SHADOWS);
    g_pmovelimitAND = (PML_LWBOTTOM | PML_RWBOTTOM | PML_BBOTTOM);
    g_missboundflags = MB_BOTTOM;
    g_gameflags |= GF_VIEWROT;        // exitbase_gameflagsON
    // exitbase_macro
    g_pshipflags2 &= (uint8)~PSF2_NOSPARK;
    g_pstratflags &= (uint8)~(PSTF_INSEQ | PSTF_NOTDIE);
    g_pstratflags |= PSTF_NOTDIE;
    self->sflags |= ASF_SHADOW;
    g_pshipflags3 &= (uint8)~PSF3_INTUNNEL;
}

// set_playerExitBase_l (PISTRATS.ASM:621-655)
void Strat_PlayerExitBase(Alien *player) {
    if (!player) return;

    // (s_set_var W,lastplayZ,#0 and jsl clearmap_l are performed by the
    //  world map callback before this is invoked.)

    // s_set_var W,Bg2Yscroll,#232
    g_bg2Yscroll = 232;

    // s_or_var B,gameflags,#gf_nozremove
    g_gameflags |= GF_NOZREMOVE;

    // s_playerfly_mode exitbase
    playerflymode_exitbase(player);

    // s_set_alptrs x,playerExitBasewait_strat,playercoll_Istrat,playerdead_Istrat
    player->stratptr = playerExitBasewait_strat;
    player->collstratptr = playercoll_istrat;
    player->expstratptr = playerdead_istrat;

    // s_and_var B,gameflags,#~gf_viewrot
    g_gameflags &= (uint8)~GF_VIEWROT;
    // s_playerctrl off
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);

    // s_set_var B,viewtype,#viewtype_fpos!viewtype_toobj
    // Fixed camera in front of the base, aimed at the (player) ship.
    g_viewtype = (VIEWTYPE_FPOS | VIEWTYPE_TOOBJ);
    g_viewtoobj = (int16)(player - g_aliens);
    // s_set_var W,viewposx,#-400 / viewposy,#-145 / viewposz,#1000
    g_viewposx = -400;
    g_viewposy = -145;
    g_viewposz = 1000;

    // s_set_var B,psvar_byte1,#15+((1000/pexitbasespeed)*2)
    g_psvar_byte1 = (uint8)(15 + ((1000 / PEXITBASE_SPEED) * 2));
    // s_set_var B,player_medspeed,#0 — hold the ship in the hangar
    g_player_medspeed = 0;

    // s_set_pos x,#-27<<mybase_scale,#-39<<mybase_scale,#-200
    player->worldx = (int16)(-27 << MYBASE_SCALE);
    player->worldy = (int16)(-39 << MYBASE_SCALE);
    player->worldz = -200;

    // s_set_alsflag x,invisible — hidden inside the hangar
    player->sflags |= ASF_INVISIBLE;

    // s_set_var B,nomaxbg2Yscroll,#1
    g_nomaxbg2Yscroll = 1;

    // s_and_var B,pshipflags3,#~psf3_enginesnd
    g_pshipflags3 &= (uint8)~PSF3_ENGINESND;
}

// playerExitBasewait_strat (PISTRATS.ASM:658-664)
static void playerExitBasewait_strat(Alien *self) {
    // s_set_pos x,#-27<<mybase_scale,#-39<<mybase_scale,#-200 — pin in hangar
    self->worldx = (int16)(-27 << MYBASE_SCALE);
    self->worldy = (int16)(-39 << MYBASE_SCALE);
    self->worldz = -200;

    // s_decbne_var B,psvar_byte1,playeronplanet_strat — while counting
    // down, run the standard flight tick; at zero fall into Go_init.
    g_psvar_byte1--;
    if (g_psvar_byte1 != 0) {
        Strat_Player(self);
        return;
    }
    playerExitBaseGo_init(self);
}

// playerExitBaseGo_init (PISTRATS.ASM:667-675)
static void playerExitBaseGo_init(Alien *self) {
    // s_set_var B,psvar_byte1,#((80-pexitbasespeed)*2)
    g_psvar_byte1 = (uint8)((80 - PEXITBASE_SPEED) * 2);
    // s_set_strat x,playerExitBaseGo_strat
    self->stratptr = playerExitBaseGo_strat;
    // s_clr_alsflag x,invisible — the ship emerges from the hangar
    self->sflags &= (uint8)~ASF_INVISIBLE;
    // s_set_var B,stagecnt,#50
    g_stagecnt = 50;
    // set_sound2 x,#0
    self->snd2 = 0;
    // s_set_var B,psvar_byte2,#0
    g_psvar_byte2 = 0;
    // s_set_alvar B,x,al_snd1,#%10110001 — engine roar, panned
    self->snd1 = 0xB1;
    // ASM falls through into playerExitBaseGo_strat
    playerExitBaseGo_strat(self);
}

// playerExitBaseGo_strat (PISTRATS.ASM:676-693)
static void playerExitBaseGo_strat(Alien *self) {
    // Engine sound pan/attenuation steps as the ship pulls away
    g_psvar_byte2++;
    if (g_psvar_byte2 == 12) self->snd1 = 0x91;   // %10010001
    if (g_psvar_byte2 == 20) self->snd1 = 0x81;   // %10000001
    if (g_psvar_byte2 == 30) self->snd1 = 0xA1;   // %10100001

    // s_set_var B,player_medspeed,#pexitbasespeed — launch speed
    g_player_medspeed = PEXITBASE_SPEED;

    // s_decbne_var B,psvar_byte1,playeronplanet_strat
    g_psvar_byte1--;
    if (g_psvar_byte1 != 0) {
        Strat_Player(self);
        return;
    }
    playerExitBaseFollow_init(self);
}

// playerExitBaseFollow_init (PISTRATS.ASM:695-697)
static void playerExitBaseFollow_init(Alien *self) {
    self->stratptr = playerExitBaseFollow_strat;
    // s_or_var B,pshipflags3,#psf3_enginesnd
    g_pshipflags3 |= PSF3_ENGINESND;
    playerExitBaseFollow_strat(self);
}

// playeronplanet_Istrat (PSTRATS.ASM:751-760) — resume normal flight.
static void playeronplanet_init(Alien *self) {
    // s_playerctrl on
    g_pshipflags &= (uint8)~(PSF_NOCTRL | PSF_NOFIRE);

    // s_playerfly_mode planet (STRATEQU.INC:558-578)
    g_viewCY = -50;                    // planet_viewCY
    g_minpmoveX = -500;
    g_maxpmoveX = 500;
    g_minMmoveX = -500;
    g_maxMmoveX = 500;
    g_maxMmoveY = 0;
    g_minpWmoveY = (-210 - 45);
    g_maxpWmoveY = 0;
    g_minpmoveY = (-210 - 45);
    g_maxpmoveY = (int16)(0 + PLAYERB_YSTOP);
    g_playerflymode = (PFM_DIEFALL | PFM_DIEYROT | PFM_SHADOWS | PFM_WOBBLE);
    g_pmovelimitAND = (PML_LWBOTTOM | PML_RWBOTTOM | PML_BBOTTOM);
    g_missboundflags = MB_BOTTOM;
    g_gameflags |= GF_VIEWROT;         // planet_gameflagsON
    // planet_macro
    g_pshipflags2 &= (uint8)~PSF2_NOSPARK;
    g_pstratflags &= (uint8)~(PSTF_INSEQ | PSTF_NOTDIE);
    self->sflags |= ASF_SHADOW;
    g_pshipflags3 &= (uint8)~PSF3_INTUNNEL;

    // s_set_alptrs x,playeronplanet_strat,playercoll_Istrat,playerdead_Istrat
    self->stratptr = Strat_Player;
    self->collstratptr = playercoll_istrat;
    self->expstratptr = playerdead_istrat;
}

// playerExitBaseFollow_strat (PISTRATS.ASM:698-720)
static void playerExitBaseFollow_strat(Alien *self) {
    // Center the ship: s_achase_alvar W,x,al_worldx,#0,2 / al_worldy,viewcy,2
    self->worldx = Strat_ChaseProportional(self->worldx, 0, 2);
    self->worldy = Strat_ChaseProportional(self->worldy, g_viewCY, 2);

    // Camera chases in behind the ship:
    // s_Achase_var W,viewposx,#0,2 / viewposy,viewcy,2 / viewposz,player_posz,3
    g_viewposx = Strat_ChaseProportional(g_viewposx, 0, 2);
    g_viewposy = Strat_ChaseProportional(g_viewposy, g_viewCY, 2);
    g_viewposz = Strat_ChaseProportional(g_viewposz, g_player_posz, 3);

    // s_varadd_alvar W,x,viewposz,al_vz — camera keeps pace with the ship
    g_viewposz = (int16)(g_viewposz + self->vz);

    // s_jmp_Zdistmore x,y(viewpt),#outviewdist+pexitbasespeed,playeronplanet_strat
    // While the ship is still further than the normal follow distance from
    // the camera, keep flying (standard tick).
    {
        int16 zdist = (int16)(self->worldz - g_viewposz);
        if (zdist < 0) zdist = (int16)(-zdist);
        if (zdist > (int16)(OUTVIEWDIST + PEXITBASE_SPEED)) {
            Strat_Player(self);
            return;
        }
    }

    // Camera caught up — hand off to the normal follow camera.
    // s_set_var B,viewtype,#viewtype_norm
    g_viewtype = VIEWTYPE_NORM;
    // s_AND_var B,gameflags,#~gf_nozremove
    g_gameflags &= (uint8)~GF_NOZREMOVE;
    // s_set_var B,player_medspeed,#medpspeed
    g_player_medspeed = MED_PSPEED;
    // (pviewpos stays continuous automatically: viewmove_srou has been
    //  tracking player_posz on every wait/Go/Follow tick via Strat_Player.)

    // s_make_obj #myship_4 — the 4th wingman catches up from behind
    {
        Alien *w = Strat_MakeObj(SHAPE_MYSHIP_4);
        if (w) {
            // s_set_alsflag y,colldisable / s_copy_pos y,x
            w->sflags |= ASF_COLLDISABLE;
            w->worldx = -50;                       // lda #-50 / sta al_worldx
            w->worldy = -100;                      // lda #-100 / sta al_worldy
            w->worldz = (int16)(self->worldz - 200);
            w->roty = self->roty;
            friendstart3_Istrat(w);
        }
    }

    // lda #0 / sta.l alx_snd1,x — stop the hangar engine channel
    self->snd1 = 0;

    // s_set_var B,nomaxbg2Yscroll,#0
    g_nomaxbg2Yscroll = 0;

    // s_jmp playeronplanet_Istrat
    playeronplanet_init(self);
    Strat_Player(self);
}

// ============================================================
// friendstart3 (GISTRATS.ASM:275-305) — 4th wingman catch-up.
// ============================================================
static void friendstart3go_strat(Alien *self) {
    Alien *player = Obj_GetPlayer();

    // s_jmp_objinfront y(player),x,.nch — only steer/boost while the
    // player is not yet ahead of the wingman.
    bool player_infront = (player && player->active &&
                           player->worldz > self->worldz);
    if (!player_infront) {
        // One-shot boost effect (sflag1 latch)
        if ((self->sflags2 & ASF2_SFLAG1) == 0) {
            self->sflags2 |= ASF2_SFLAG1;
            g_boostobj = (int16)(self - g_aliens);
            Sound_PlaySE(0x32);
        }
        // s_achase_alvar B,x,al_rotz,#0,4
        self->rotz = (uint8)Strat_ChaseProportional((int8)self->rotz, 0, 4);
        // s_jmp_notdelay 2,.nch — every 4th frame
        if ((g_gameframe & 3) == 0) {
            // s_achase_alvar B,x,al_rotx,#-deg90,4
            self->rotx = (uint8)Strat_ChaseProportional((int8)self->rotx,
                                                        (int8)(uint8)(-DEG90), 4);
        }
    }

    // s_gen_3dvecs / s_add_vecs2pos / s_add_playerZ
    Strat_GenVecs3D(self);
    Strat_ApplyVelocity(self);
    self->worldz = (int16)(self->worldz + g_pviewvelz);

    // s_dec_lifecnt x
    if (self->count > 0) {
        self->count--;
    } else {
        Strat_RemoveObj();
    }
}

static void friendstart3_Istrat(Alien *self) {
    // s_setnoremove_behind x
    self->type &= (uint8)~ATZREMOVE;
    // s_set_alsflag x,colldisable
    self->sflags |= ASF_COLLDISABLE;
    // s_set_strat x,friendstart3go_strat
    self->stratptr = friendstart3go_strat;
    // s_set_lifecnt x,#70
    self->count = 70;
    // s_set_speed x,#30
    self->vel = 30;
    // s_set_alvar B,x,al_rotz,#-deg90
    self->rotz = (uint8)(-DEG90);
}

// ============================================================
// PCSTRATS.ASM — Player Clear / Escape Strategies
// Port of playerclearbridge_Istrat through playerEscapeNucleus_strat
// ============================================================

// --- Constants from STRATEQU.INC ---
#define SPACE_VIEWCY     (-60)
#define PLANET_VIEWCY    (-50)
#define UNDERGND_VIEWCY  (-60)

// --- centoutrots (PCSTRATS.ASM:1151-1154) ---
// Chase outvx and outvy toward 0 with proportional rate 3.
static void centoutrots(void) {
    g_outvx = Strat_ChaseProportional(g_outvx, 0, 3);
    g_outvy = Strat_ChaseProportional(g_outvy, 0, 3);
}

// --- dupplayer (PSTRATS.ASM:3516-3525) ---
// Create a copy of the player ship object. Returns the duplicate (Y in ASM).
// Makes the player invisible, the duplicate gets colldisable+shadow.
static Alien *dupplayer(Alien *player) {
    Alien *dup = Strat_MakeObj(0);
    if (!dup) return NULL;
    dup->shape = player->shape;
    dup->worldx = player->worldx;
    dup->worldy = player->worldy;
    dup->worldz = player->worldz;
    dup->rotx = player->rotx;
    dup->roty = player->roty;
    dup->rotz = player->rotz;
    dup->sflags |= (ASF_COLLDISABLE | ASF_SHADOW);
    player->sflags |= ASF_INVISIBLE;
    return dup;
}

// --- playeronbridge_strat (PSTRATS.ASM:1002-1027) ---
// Resumes normal bridge control: do_player_bridge, vy/=2, perc62 view, viewmove.
static void playeronbridge_strat(Alien *self) {
    // Restore player control and fly mode (bridge).
    g_pshipflags &= (uint8)~(PSF_NOCTRL | PSF_NOFIRE);
    g_pstratflags &= (uint8)~(PSTF_INSEQ | PSTF_NOVDISTC);

    // do_player_bridge → same as do_player_yvelD2
    playermove_srou(self);
    Strat_GenVecs3D(self);
    self->vx = Strat_Perc62(self->vx);
    self->vy = (int16)(self->vy >> 1);
    Strat_ApplyVelocity(self);
    playerlimitX_srou(self);
    playerfire_srou(self);
    checkarrows_srou();

    // view X = perc62(worldx), view Y = viewCY
    g_pviewposx = Strat_Perc62(self->worldx);
    g_pviewposy = g_viewCY;

    viewmove_srou(self);
}

// ============================================================
// playerclearbridge_Istrat / playerclearbridge_strat / playerclearbridge3
// (PCSTRATS.ASM:48-86)
// ============================================================

static void playerclearbridge3_strat(Alien *self);
static void playerclearbridge_strat(Alien *self);

// playerclearbridge2_init (PCSTRATS.ASM:77-83)
static void playerclearbridge2_init(Alien *self) {
    // dupplayer — create wingman copy for bridge boost
    Alien *dup = dupplayer(self);
    if (dup) {
        // s_set_strat y,clshipbridgeboost_Istrat
        // clshipbridgeboost not yet ported; set colldisable+shadow as placeholder
        dup->sflags |= ASF_COLLDISABLE;
    }
    // s_set_strat x,playerclearbridge3_strat
    self->stratptr = playerclearbridge3_strat;
    playerclearbridge3_strat(self);
}

// playerclearbridge3_strat (PCSTRATS.ASM:83-86)
static void playerclearbridge3_strat(Alien *self) {
    // s_add_alvar W,x,al_worldx,#3
    self->worldx = (int16)(self->worldx + 3);
    // s_jmp playeronbridge_strat
    playeronbridge_strat(self);
}

// playerclearbridge_strat (PCSTRATS.ASM:64-76)
static void playerclearbridge_strat(Alien *self) {
    // jsr centoutrots
    centoutrots();

    // s_beqdec_var B,psvar_byte1,playerclearbridge2_init
    if (g_psvar_byte1 == 0) {
        playerclearbridge2_init(self);
        return;
    }
    g_psvar_byte1--;

    // s_achase_alvar.w W,x,al_worldy,ViewCY,4
    self->worldy = Strat_ChaseProportional(self->worldy, g_viewCY, 4);

    // s_achase_alvar.w W,x,al_worldx,#0,4
    self->worldx = Strat_ChaseProportional(self->worldx, 0, 4);

    // s_set_var B,player_tospeed,#maxpspeed
    g_player_tospeed = MAX_PSPEED;

    // s_jmp_varmore W,outdist,#500,.nda
    if (g_outdist <= 500) {
        // s_add_var W,outdist,#4
        g_outdist = (int16)(g_outdist + 4);
    }
    // .nda: s_jmp playeronbridge_strat
    playeronbridge_strat(self);
}

// playerclearbridge_Istrat (PCSTRATS.ASM:48-62)
void Strat_PlayerClearBridge_Init(Alien *self) {
    // s_set_strat x,playerclearbridge_strat
    self->stratptr = playerclearbridge_strat;

    // s_playerctrl off
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);

    // s_set_var W,minPmoveY,#-10000
    g_minpmoveY = -10000;
    // s_set_var W,minPWmoveY,#-10000
    g_minpWmoveY = -10000;

    // s_or_var B,pstratflags,#pstf_novdistC
    g_pstratflags |= PSTF_NOVDISTC;
    // s_or_var B,pstratflags,#pstf_inseq
    g_pstratflags |= PSTF_INSEQ;

    // s_set_var W,psvar_word1,#0
    g_psvar_word1 = 0;
    // s_set_var W,psvar_word2,#0
    g_psvar_word2 = 0;

    // s_set_var B,psvar_byte1,#125+38
    g_psvar_byte1 = 125 + 38;
}

// ============================================================
// playerEscapeNucleus_Istrat / playerEscapeNucleus_strat
// (PCSTRATS.ASM:97-150)
// ============================================================

static void playerEscapeNucleus_cont(Alien *self);
static void playerEscapeNucleus_strat(Alien *self);
static void playerEscapeNucleus2_init(Alien *self);
static void playerEscapeNucleus2_start(Alien *self);

// playerEscapeNucleus_cont (PCSTRATS.ASM:137-150)
static void playerEscapeNucleus_cont(Alien *self) {
    // s_speedto x,#30,1
    (void)Strat_SpeedTo(self, 30, 1);

    // s_set_var W,pviewvelz,#medpspeed
    g_pviewvelz = MED_PSPEED;

    // s_add_var W,pviewposz,pviewvelz
    g_pviewposz = (int16)(g_pviewposz + g_pviewvelz);

    // s_gen_3dvecs x,al_roty,al_rotx,al_vel
    Strat_GenVecs3D(self);

    // s_add_alvar W,x,al_vz,#medpspeed
    self->vz = (int16)(self->vz + MED_PSPEED);

    // s_add_vecs2pos x
    Strat_ApplyVelocity(self);
}

// playerEscapeNucleus2_start (PCSTRATS.ASM:121-134)
static void playerEscapeNucleus2_start(Alien *self) {
    // s_decbne_alvar B,x,al_sbyte3,playerEscapeNucleus_cont
    // Decrement sbyte3; if result is non-zero, branch to cont.
    self->sbyte3--;
    if (self->sbyte3 != 0) {
        playerEscapeNucleus_cont(self);
        return;
    }

    // sbyte3 reached 0: set to 1 and do adjustments
    // s_set_alvar B,x,al_sbyte3,#1
    self->sbyte3 = 1;

    // s_jmp_alvarEQ B,x,al_rotx,#-deg11,.nxdec
    if ((int8)self->rotx != (int8)(-(int8)DEG11)) {
        // s_dec_alvar B,x,al_rotx
        self->rotx = (uint8)(self->rotx - 1);
    }
    // .nxdec

    // s_jmp_alvarEQ B,x,al_rotz,#deg45,.nzinc
    if (self->rotz != DEG45) {
        // s_add_alvar B,x,al_rotz,#2
        self->rotz = (uint8)(self->rotz + 2);
    }
    // .nzinc

    // s_jmp_alvarEQ B,x,al_roty,#deg180+(deg22+deg11),.boost
    if (self->roty == (uint8)(DEG180 + DEG22 + DEG11)) {
        // .boost — fall through to playerEscapeNucleus_cont
        playerEscapeNucleus_cont(self);
        return;
    }

    // s_add_alvar B,x,al_roty,#8
    self->roty = (uint8)(self->roty + 8);

    // brl playerEscapeNucleus_cont
    playerEscapeNucleus_cont(self);
}

// playerEscapeNucleus2_init (PCSTRATS.ASM:116-120)
static void playerEscapeNucleus2_init(Alien *self) {
    // s_set_strat x,playerEscapeNucleus2_start
    self->stratptr = playerEscapeNucleus2_start;

    // s_set_alvar B,x,al_sbyte3,#15
    self->sbyte3 = 15;

    // s_set_vartobeobj boostobj,x
    g_boostobj = (int16)(self - g_aliens);

    // boost_sprite — create boost visual effect (not yet ported, skip)

    // Run first frame
    playerEscapeNucleus2_start(self);
}

// playerEscapeNucleus_strat (PCSTRATS.ASM:107-114)
static void playerEscapeNucleus_strat(Alien *self) {
    // s_jmp_alvarEQ B,x,al_roty,#-(deg45-deg22),playerEscapeNucleus2_init
    // -(deg45-deg22) = -(32-16) = -16, as uint8 = 240
    if (self->roty == (uint8)(-(DEG45 - DEG22))) {
        playerEscapeNucleus2_init(self);
        return;
    }

    // s_add_alvar B,x,al_roty,#-1
    self->roty = (uint8)(self->roty - 1);

    // s_add_alvar B,x,al_rotz,#-1
    self->rotz = (uint8)(self->rotz - 1);

    // s_jmp_alvarEQ B,x,al_rotx,#deg11,.nxinc
    if (self->rotx != DEG11) {
        // s_inc_alvar B,x,al_rotx
        self->rotx = (uint8)(self->rotx + 1);
    }
    // .nxinc: s_brl playerEscapeNucleus_cont
    playerEscapeNucleus_cont(self);
}

// playerEscapeNucleus_Istrat (PCSTRATS.ASM:97-106)
void Strat_PlayerEscapeNucleus_Init(Alien *self) {
    // s_or_var B,pstratflags,#pstf_inseq
    g_pstratflags |= PSTF_INSEQ;

    // s_set_strat x,playerEscapeNucleus_strat
    self->stratptr = playerEscapeNucleus_strat;

    // s_set_speed x,#0
    self->vel = 0;

    // s_set_alvar B,x,al_roty,#0
    self->roty = 0;

    // s_set_alsflag x,colldisable
    self->sflags |= ASF_COLLDISABLE;
}

// ============================================================
// PISTRATS.ASM — Player Opening (Intro Sequence) Strategies
// Port of playeropening_Istrat through playerpening_strat
// and playeropeningboost_init / playeropeningboost_strat
// ============================================================

// --- viewopening_Istrat / viewopening_strat (GISTRATS.ASM:617-649) ---
// Camera fly-in object spawned during player opening.
// This is a separate invisible object that tracks the player position
// and drives the camera through a multi-state sequence.
static void viewopening_strat(Alien *self);

static void viewopening_Istrat(Alien *self) {
    // s_set_alsflag x,invisible
    self->sflags |= ASF_INVISIBLE;
    // s_set_strat x,viewopening_strat
    self->stratptr = viewopening_strat;
    // s_add_alvar W,x,al_worldx,#-600*2
    self->worldx = (int16)(self->worldx + (-600 * 2));
    // s_add_alvar W,x,al_worldy,#-1000*2
    self->worldy = (int16)(self->worldy + (-1000 * 2));
    // s_add_alvar W,x,al_worldz,#3500
    self->worldz = (int16)(self->worldz + 3500);
    // s_set_alvar B,x,al_roty,#deg180
    self->roty = DEG180;
    // s_set_alvar B,x,al_rotx,#deg5
    self->rotx = DEG5;
    // s_set_alvar B,x,al_sbyte1,#90
    self->sbyte1 = 90;
    // s_AND_var B,gameflags,#~gf_stratdone1
    g_gameflags &= (uint8)~GF_STRATDONE1;
    // s_and_var B,pshipflags3,#~psf3_enginesnd
    g_pshipflags3 &= (uint8)~PSF3_ENGINESND;
}

// viewopening_strat (GISTRATS.ASM:629-649)
// Multi-state camera that chases toward player position offsets.
static void viewopening_strat(Alien *self) {
    // s_set_var W,svar_word1,player_posx
    g_svar_word1 = g_player_posx;
    // s_set_var W,svar_word2,#-30
    g_svar_word2 = -30;
    // s_set_var W,svar_word3,player_posz
    g_svar_word3 = (int16)g_player_posz;

    // State machine (s_jmp_ifnotstate)
    switch (self->stratstate) {
    case 0:
        // s_add_var W,svar_word1,#-400
        g_svar_word1 = (int16)(g_svar_word1 + (-400));
        // s_add_var W,svar_word2,#-700
        g_svar_word2 = (int16)(g_svar_word2 + (-700));
        // s_add_var W,svar_word3,#-700
        g_svar_word3 = (int16)(g_svar_word3 + (-700));
        // s_achase_alvar W,x,al_worldx,svar_word1,5
        self->worldx = Strat_ChaseProportional(self->worldx, g_svar_word1, 5);
        // s_achase_alvar W,x,al_worldy,svar_word2,5
        self->worldy = Strat_ChaseProportional(self->worldy, g_svar_word2, 5);
        // s_achase_alvar W,x,al_worldz,svar_word3,5
        self->worldz = Strat_ChaseProportional(self->worldz, g_svar_word3, 5);
        // s_decbne_alvar B,x,al_sbyte1
        if (self->sbyte1 > 0) {
            self->sbyte1--;
            if (self->sbyte1 != 0) break;
        }
        // s_next_state x
        self->stratstate = 1;
        // s_set_alvar B,x,al_sbyte1,#80
        self->sbyte1 = 80;
        // s_or_var B,gameflags,#gf_stratdone2
        g_gameflags |= GF_STRATDONE2;
        // s_or_var B,pshipflags3,#psf3_enginesnd
        g_pshipflags3 |= PSF3_ENGINESND;
        break;
    default:
        // State 1 (GISTRATS.ASM:652-668)
        // s_decbne_alvar B,x,al_sbyte1,.ndone
        self->sbyte1--;
        if (self->sbyte1 == 0) {
            // s_or_var B,gameflags,#gf_stratdone1
            // Signals MAP1_1A's `mapif chkstratdone1,.fin` loop to finish.
            g_gameflags |= GF_STRATDONE1;
        }
        // .ndone: s_AND_var B,gameflags,#~gf_nozremove
        g_gameflags &= (uint8)~GF_NOZREMOVE;
        // s_add_var W,svar_word2,#20
        g_svar_word2 = (int16)(g_svar_word2 + 20);
        // s_add_var W,svar_word3,#-300
        g_svar_word3 = (int16)(g_svar_word3 - 300);
        // s_achase_alvar W,x,al_worldy,svar_word2,4
        self->worldy = Strat_ChaseProportional(self->worldy, g_svar_word2, 4);
        // s_achase_alvar W,x,al_worldx,svar_word1,3,.zoom
        self->worldx = Strat_ChaseProportional(self->worldx, g_svar_word1, 3);
        if (self->worldx == g_svar_word1) {
            // .zoom: s_add_alvar W,x,al_worldz,#10
            self->worldz = (int16)(self->worldz + 10);
        } else {
            // .nfrick: s_achase_alvar W,x,al_worldz,svar_word3,4
            self->worldz = Strat_ChaseProportional(self->worldz, g_svar_word3, 4);
        }
        break;
    }

    // .end (GISTRATS.ASM:679-683)
    // s_add_alvar W,x,al_worldz,#medpspeed
    self->worldz = (int16)(self->worldz + MED_PSPEED);
    // s_copy_alvar2var W,x,viewposx/viewposy/viewposz — drive the camera.
    // These are the final camera position vars (viewtype is FPOS during
    // the opening, so getview uses them verbatim).
    g_viewposx = self->worldx;
    g_viewposy = self->worldy;
    g_viewposz = self->worldz;
}

// --- playeropeningboost_strat (PISTRATS.ASM:108-112) ---
static void playeropeningboost_strat(Alien *self) {
    // s_achase_alvar B,x,al_rotz,#0,3
    {
        int8 diff = (int8)self->rotz;
        int8 step = diff >> 3;
        if (diff != 0 && step == 0) step = (diff > 0) ? 1 : -1;
        self->rotz = (uint8)((int8)self->rotz - step);
    }
    // s_add_Alvar W,x,al_worldz,#medpspeed+50
    self->worldz = (int16)(self->worldz + (MED_PSPEED + 50));
}

// --- playeropeningboost_init (PISTRATS.ASM:99-107) ---
static void playeropeningboost_init(Alien *self) {
    // s_set_strat x,playeropeningboost_strat
    self->stratptr = playeropeningboost_strat;
    // s_set_vartobeobj boostobj,x
    g_boostobj = (int16)(self - g_aliens);
    // boost_sprite — visual boost effect (SNES sprite overlay, not applicable in HD)
    // trigse $32
    Sound_PlaySE(0x32);
    // s_and_var B,pshipflags3,#~psf3_enginesnd
    g_pshipflags3 &= (uint8)~PSF3_ENGINESND;
}

// --- playerpening_strat (PISTRATS.ASM:74-97) ---
// Note: original ASM has typo "playerpening" (missing "o")
static void playerpening_strat(Alien *self) {
    // s_inc_var B,psvar_byte1
    g_psvar_byte1++;
    // s_jmp_varNE B,psvar_byte1,#pZrotfloattab_len,.pfzptrok
    if (g_psvar_byte1 >= PZROTFLOATTAB_LEN) {
        // s_set_var B,psvar_byte1,#0
        g_psvar_byte1 = 0;
    }
    // .pfzptrok: s_set_alvar2vartab B,B,B,x,al_rotz,psvar_byte1,pZrotfloattab
    self->rotz = (uint8)s_shipintro_rotz_float[g_psvar_byte1];

    // s_add_var B,psvar_byte2,#2
    g_psvar_byte2 = (uint8)(g_psvar_byte2 + 2);
    // s_jmp_varNE B,psvar_byte2,#viewfloattab_len,.nmaxy
    if (g_psvar_byte2 >= VIEWFLOATTAB_BYTELEN) {
        // s_set_var B,psvar_byte2,#0
        g_psvar_byte2 = 0;
    }
    // .nmaxy: s_set_alvar2vartab B,B,W,x,al_worldy,psvar_byte2,viewfloattab,1
    // Byte index / 2 = word index into viewfloattab
    self->worldy = s_shipintro_view_float[g_psvar_byte2 / 2];

    // s_add_alvar W,x,al_worldy,#-30
    self->worldy = (int16)(self->worldy + (-30));

    // s_jmpNOT_varAND B,gameflags,#gf_stratdone2,.nboost
    if ((g_gameflags & GF_STRATDONE2) != 0) {
        // s_beqdec_var B,psvar_byte3,playeropeningboost_init
        if (g_psvar_byte3 == 0) {
            playeropeningboost_init(self);
            return;
        }
        g_psvar_byte3--;
    }
    // .nboost

    // s_add_Alvar W,x,al_worldz,#medpspeed
    self->worldz = (int16)(self->worldz + MED_PSPEED);
    // jsl makeBG2black_l — zeros BG2 palette (SNES-specific, no-op in HD)
}

// --- playeropening_Istrat (PISTRATS.ASM:46-72) ---
void Strat_PlayerOpening_Init(Alien *self) {
    // s_playerctrl off
    g_pshipflags |= (PSF_NOCTRL | PSF_NOFIRE);

    // s_playerfly_mode Ltunnel
    g_viewCY = LTUNNEL_VIEWCY;
    g_minpmoveX = LTUNNEL_MINX;
    g_maxpmoveX = LTUNNEL_MAXX;
    g_minMmoveX = LTUNNEL_MMINX;
    g_maxMmoveX = LTUNNEL_MMAXX;
    g_maxMmoveY = LTUNNEL_MMAXY;
    g_minpWmoveY = LTUNNEL_MINY;
    g_maxpWmoveY = (int16)(LTUNNEL_MAXY + 5);
    g_minpmoveY = LTUNNEL_MINY;
    g_maxpmoveY = (int16)(LTUNNEL_MAXY + PLAYERB_YSTOP);
    g_playerflymode = (PFM_DIEFALL | PFM_SHADOWS);
    g_pmovelimitAND = PML_ALL;
    g_missboundflags = (MB_LEFT | MB_RIGHT | MB_BOTTOM);
    // Ltunnel_gameflagsON = 0 (no-op)
    g_gameflags &= (uint8)~GF_VIEWROT;    // ~Ltunnel_gameflagsOFF
    g_pstratflags &= (uint8)~PSTF_NOVIEWMOVE;
    g_pshipflags3 |= PSF3_ENGINESND;
    // Ltunnel_macro
    g_pshipflags2 &= (uint8)~PSF2_NOSPARK;
    g_pstratflags &= (uint8)~(PSTF_INSEQ | PSTF_NOTDIE);
    self->sflags |= ASF_SHADOW;
    g_pshipflags3 |= PSF3_INTUNNEL;

    // s_set_alptrs x,playerpening_strat,0,0
    self->stratptr = playerpening_strat;
    self->collstratptr = NULL;
    self->expstratptr = NULL;

    // stz gameframe
    g_gameframe = 0;

    // lda #2 / sta fadedir
    g_fadedir = 2;

    // s_set_var W,outvx,#-deg11*256
    g_outvx = (int16)(-(DEG11 * 256));

    // s_or_var B,gameflags,#gf_nozremove
    g_gameflags |= GF_NOZREMOVE;

    // s_set_var B,viewtype,#viewtype_toobj!viewtype_Fpos
    g_viewtype = (VIEWTYPE_TOOBJ | VIEWTYPE_FPOS);
    // viewtoobj defaults to playpt (MAIN.ASM) — aim the camera at the ship.
    g_viewtoobj = (int16)(self - g_aliens);

    // s_make_obj #nullshape,.bad  /  s_set_strat y,viewopening_Istrat  /  s_copy_pos y,x
    {
        Alien *cam = Strat_MakeObj(0);  // nullshape = 0
        if (cam) {
            cam->worldx = self->worldx;
            cam->worldy = self->worldy;
            cam->worldz = self->worldz;
            viewopening_Istrat(cam);
            // Seed the fixed camera position so the first rendered frame
            // (before the strat's first tick) already uses the fly-in cam.
            g_viewposx = cam->worldx;
            g_viewposy = cam->worldy;
            g_viewposz = cam->worldz;
        }
    }
    // .bad

    // s_AND_var B,gameflags,#~gf_stratdone2
    g_gameflags &= (uint8)~GF_STRATDONE2;

    // s_set_var B,psvar_byte3,#70
    g_psvar_byte3 = 70;

    // s_set_alvar W,x,al_shape,#Imyship_4
    self->shape = SHAPE_MYSHIP_4;

    // s_set_var B,psvar_byte1,#0
    g_psvar_byte1 = 0;
    // s_set_var B,psvar_byte2,#0
    g_psvar_byte2 = 0;
}
