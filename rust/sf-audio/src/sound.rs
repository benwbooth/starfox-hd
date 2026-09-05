//! Game-side sound layer (C oracle: `src/game/sound.c`, itself a literal
//! port of SOUND.ASM dosounds_l / playersnd / nearobjs / do_obstacles /
//! setport3_l).
//!
//! Game-state inputs arrive via plain data structs ([`SoundGameState`],
//! [`SoundPlayer`], [`SoundObj`]) and typed playback requests go through the
//! [`SoundBackend`] trait, so this module does not depend on sf-game.

use crate::catalog;

// ---------------------------------------------------------------------------
// Distance thresholds (C SOUND_* defines).
// ---------------------------------------------------------------------------
const SOUND_OBSDIST: i16 = 100;
const SOUND_CUTOFFSND: i16 = 3150;
const SOUND_DIST3SND: i16 = 1150;
const SOUND_DIST2SND: i16 = 650;
const SOUND_DIST1SND: i16 = 250;

const SOUND_SHIP1_CUTOFF: i16 = 11000;
const SOUND_SHIP1_DIST3: i16 = 10000;
const SOUND_SHIP1_DIST2: i16 = 10000;
const SOUND_SHIP1_DIST1: i16 = 5000;
const SOUND_SHIP1_MINDIST: i16 = 500;

const SOUND_PLAYACCEL1: i16 = 2;
const SOUND_PLAYACCEL2: i16 = 4;
const SOUND_PLAYACCEL3: i16 = 8;

const SOUND_SE_DOPRIGHT: u8 = 0x6D;
const SOUND_SE_DOPCENTRE: u8 = 0x6E;
const SOUND_SE_DOPLEFT: u8 = 0x6F;

// ---------------------------------------------------------------------------
// makesnd positional one-shot layer (C oracle: SOUND.ASM makesnd, 899-945,
// and the *sound_l family selectors 735-897).
//
// makesnd is the common tail of destbosssound_l / destenemysound_l /
// damenemysound_l / hitwallsound_l / missilesound_l / movewallsound_l /
// lasersound_l / enemybattrysound_l / ringlasersound_l / dooropensound_l /
// doorclosesound_l / enemyupsea_l / enemydownsea_l / separatemissile_l.
// Each loads L/C/R/mid/far SE ids into lsnd/csnd/rsnd/msnd/fsnd, then jmps
// makesnd, which:
//   * computes rangexz (xzdiffs_l XZ octagonal distance obj<->player);
//   * `rangexz < 2000`  -> near: L/C/R by the signed x-offset
//         dx = pviewposx - obj.worldx, split at +-170 (SOUND.ASM:920-937);
//   * `rangexz < dist3snd` (1150) -> mid (msnd) — UNREACHABLE, dead in ROM
//         because dist3snd < 2000 (the near threshold checked first);
//   * `rangexz < cutoffsnd` (3150) -> far (fsnd);
//   * else -> silence (no setport3 write).
// The selected id is queued through setport3_l (== play_se: same nosetport3
// / in-game-HP0 gate and 16-entry ring as one-shot trigse). This is a
// distance-attenuated ONE-SHOT layer, distinct from the per-frame looping
// nearobjs pass on port 2.
// ---------------------------------------------------------------------------

/// `rangexz` threshold below which makesnd uses the near L/C/R band
/// (SOUND.ASM:907 `cmp #2000`).
const SOUND_MAKESND_NEAR: u16 = 2000;
/// Signed x-offset (pviewposx - obj.worldx) that splits centre from the
/// left/right variants (SOUND.ASM:924/930 `cmp #170` / `cmp #-170`).
const SOUND_MAKESND_XSPLIT: i16 = 170;

/// One `*sound_l` family: the five SE ids loaded into lsnd/csnd/rsnd/msnd/fsnd
/// before the `jmp makesnd` tail. For the "near"-only families (destboss,
/// destenemy, damenemy, hitwall, missile, enemybattry, ringlaser) l==c==r.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PosSndFamily {
    /// `lsnd` — object far to the left (dx >= +170).
    pub l: u8,
    /// `csnd` — object roughly centred (|dx| <= 170).
    pub c: u8,
    /// `rsnd` — object far to the right (dx < -170).
    pub r: u8,
    /// `msnd` — mid band. Dead in ROM makesnd (dist3snd < near threshold);
    /// only reachable via the strats' own inline range checks.
    pub m: u8,
    /// `fsnd` — far band (2000..3150).
    pub f: u8,
}

impl PosSndFamily {
    /// Near-only family: l == c == r == `near`, plus `mid`/`far`
    /// (destboss/destenemy/damenemy/hitwall/missile/enemybattry).
    const fn near(near: u8, mid: u8, far: u8) -> Self {
        PosSndFamily {
            l: near,
            c: near,
            r: near,
            m: mid,
            f: far,
        }
    }
}

/// `destbosssound_l` — SOUND.ASM:735 ($1e/$1f/$20).
pub const POS_DESTBOSS: PosSndFamily = PosSndFamily::near(0x1e, 0x1f, 0x20);
/// `destenemysound_l` — SOUND.ASM:746 ($21/$22/$23).
pub const POS_DESTENEMY: PosSndFamily = PosSndFamily::near(0x21, 0x22, 0x23);
/// `damenemysound_l` — SOUND.ASM:757 ($24/$25/$26).
pub const POS_DAMENEMY: PosSndFamily = PosSndFamily::near(0x24, 0x25, 0x26);
/// `hitwallsound_l` — SOUND.ASM:768 ($27/$28/$29).
pub const POS_HITWALL: PosSndFamily = PosSndFamily::near(0x27, 0x28, 0x29);
/// `missilesound_l` — SOUND.ASM:779 ($3c/$3d/$3e).
pub const POS_MISSILE: PosSndFamily = PosSndFamily::near(0x3c, 0x3d, 0x3e);
/// `movewallsound_l` — SOUND.ASM:790 ($3f/$40/$41/$42/$43), distinct L/C/R.
pub const POS_MOVEWALL: PosSndFamily = PosSndFamily {
    l: 0x3f,
    c: 0x40,
    r: 0x41,
    m: 0x42,
    f: 0x43,
};
/// `lasersound_l` — SOUND.ASM:803 ($44/$45/$46/$47/$48), distinct L/C/R.
pub const POS_LASER: PosSndFamily = PosSndFamily {
    l: 0x44,
    c: 0x45,
    r: 0x46,
    m: 0x47,
    f: 0x48,
};
/// `enemybattrysound_l` — SOUND.ASM:816 ($49/$4a/$4b).
pub const POS_ENEMYBATTRY: PosSndFamily = PosSndFamily::near(0x49, 0x4a, 0x4b);
/// `ringlasersound_l` — SOUND.ASM:827 ($5c/$5d/$5e).
pub const POS_RINGLASER: PosSndFamily = PosSndFamily::near(0x5c, 0x5d, 0x5e);
/// `dooropensound_l` — SOUND.ASM:839 (near $54, mid/far $55).
pub const POS_DOOROPEN: PosSndFamily = PosSndFamily::near(0x54, 0x55, 0x55);
/// `doorclosesound_l` — SOUND.ASM:850 (near $52, mid/far $53).
pub const POS_DOORCLOSE: PosSndFamily = PosSndFamily::near(0x52, 0x53, 0x53);
/// `enemyupsea_l` — SOUND.ASM:861 ($68/$69/$6a/$6b/$6c), distinct L/C/R.
pub const POS_ENEMYUPSEA: PosSndFamily = PosSndFamily {
    l: 0x68,
    c: 0x69,
    r: 0x6a,
    m: 0x6b,
    f: 0x6c,
};
/// `enemydownsea_l` — SOUND.ASM:874 ($74/$75/$76/$77/$78), distinct L/C/R.
pub const POS_ENEMYDOWNSEA: PosSndFamily = PosSndFamily {
    l: 0x74,
    c: 0x75,
    r: 0x76,
    m: 0x77,
    f: 0x78,
};

/// Resolve SOUND.ASM `makesnd` to the exact one-shot effect without mutating
/// the sound queue. Differential runners use this same source-derived policy
/// to compare a typed positional command with the retail effect-ring write.
pub fn resolve_positional_effect(
    player_view_x: i16,
    player_worldx: i16,
    player_worldz: i16,
    obj_worldx: i16,
    obj_worldz: i16,
    family: &PosSndFamily,
) -> Option<u8> {
    let rangexz = xzdiffs_rangexz(player_worldx, player_worldz, obj_worldx, obj_worldz) as u16;

    if rangexz < SOUND_MAKESND_NEAR {
        let dx = player_view_x.wrapping_sub(obj_worldx);
        Some(if dx >= 0 {
            if dx < SOUND_MAKESND_XSPLIT {
                family.c
            } else {
                family.l
            }
        } else if dx >= -SOUND_MAKESND_XSPLIT {
            family.c
        } else {
            family.r
        })
    } else if rangexz < SOUND_DIST3SND as u16 {
        Some(family.m)
    } else if rangexz < SOUND_CUTOFFSND as u16 {
        Some(family.f)
    } else {
        None
    }
}

/// `separatemissile_l` — SOUND.ASM:887 ($49/$4a/$4b).
pub const POS_SEPARATEMISSILE: PosSndFamily = PosSndFamily::near(0x49, 0x4a, 0x4b);

/// ISTRATS def_shape indices used by SOUND.ASM's force-sound path
/// (C SOUND_SHAPE_SHIP_*).
pub const FORCESND_SHAPE_IDS: [u16; 5] = [21, 23, 107, 109, 110];

// D-pad bits of pad1 low byte (C sf_rtl.h PAD_TLEFT / PAD_TRIGHT).
const PAD_TLEFT: u8 = 1 << 5;
const PAD_TRIGHT: u8 = 1 << 4;

// levelfinished states that mute port 2 (C world.h LE_*).
const LE_BHOLE1: u8 = 11;
const LE_BHOLE2: u8 = 12;
const LE_BHOLE3: u8 = 13;
const LE_ENTERBHOLE: u8 = 15;
const LE_ENTERSPEC: u8 = 16;

// ---------------------------------------------------------------------------
// Map IDs (C src/map/levels.h MAP_ID_*), duplicated here so the crate does
// not depend on sf-map/sf-game.
// ---------------------------------------------------------------------------
pub const MAP_ID_1_1: u32 = 1;
pub const MAP_ID_1_2: u32 = 2;
pub const MAP_ID_1_3: u32 = 3;
pub const MAP_ID_1_4: u32 = 4;
pub const MAP_ID_1_5: u32 = 5;
pub const MAP_ID_1_6: u32 = 6;
pub const MAP_ID_2_1: u32 = 7;
pub const MAP_ID_2_2: u32 = 8;
pub const MAP_ID_2_3: u32 = 9;
pub const MAP_ID_2_4: u32 = 10;
pub const MAP_ID_2_5: u32 = 11;
pub const MAP_ID_2_6: u32 = 12;
pub const MAP_ID_3_1: u32 = 13;
pub const MAP_ID_3_2: u32 = 14;
pub const MAP_ID_3_3: u32 = 15;
pub const MAP_ID_3_4: u32 = 16;
pub const MAP_ID_3_5: u32 = 17;
pub const MAP_ID_3_6: u32 = 18;
pub const MAP_ID_3_7: u32 = 19;
pub const MAP_ID_BLACKHOLE: u32 = 20;
pub const MAP_ID_SPECIAL: u32 = 21;
pub const MAP_ID_FINAL: u32 = 22;
pub const MAP_ID_TRAINING: u32 = 29;

// ---------------------------------------------------------------------------
// Native playback backend. The game-side selection logic retains the source
// behavior, but publishes semantic channel changes instead of hardware-port
// traffic.
// ---------------------------------------------------------------------------
pub trait SoundBackend {
    fn set_engine_sound(&mut self, sound: u8);
    fn set_ambient_sound(&mut self, sound: u8);
    fn play_effect(&mut self, effect: u8);
    fn effect_consumed(&mut self, effect: u8) -> bool;
    fn clear_effect_acknowledgement(&mut self);
    fn start_music(&mut self, cue: u8);
    fn load_track(&mut self, track: u8);
    fn set_paused(&mut self, paused: bool);

    fn play_immediate(&mut self, effect: u8) {
        self.play_effect(effect);
    }
}

// ---------------------------------------------------------------------------
// Per-tick game state inputs (the globals sound.c reads).
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, Default)]
pub struct SoundGameState {
    /// `g_gameflags2 & GF2_INGAME`
    pub in_game: bool,
    /// `g_gameflags & GF_PLAYERDEAD`
    pub player_dead: bool,
    /// `g_pshipflags2 & PSF2_PLAYERHP0`
    pub player_hp0: bool,
    /// `g_pshipflags3 & PSF3_ENGINESND`
    pub engine_snd: bool,
    /// `g_levelfinished` (LE_*)
    pub level_finished: u8,
    /// `g_inatunnel` (0 = no, 1 = tunnel, 2 = variant)
    pub in_a_tunnel: u8,
    /// `g_game_mode & SPACE_MODE`
    pub space_mode: bool,
    /// `g_playersndflag`
    pub player_snd_flag: u8,
    /// `g_pad1` (only the low byte is inspected)
    pub pad1: u16,
    /// `g_pviewposx` — player view position X
    pub pviewposx: i16,
    /// `g_newmap` — current map id (drives the level BGM boot)
    pub new_map: u32,
    /// Current BGS background id (`g_currentbg`). The Corneria scramble
    /// starts in `bg_1_1i` (id 0) and switches to `bg_1_1c` (id 4), which
    /// carries the source's SND_10 -> SND_11 music-bank transition.
    pub current_bg: u16,
    /// `g_shapes_table[i]` for the five FORCESND_SHAPE_IDS indices
    /// (21, 23, 107, 109, 110), 0 when unmapped — feeds
    /// `sound_shape_matches_mapped`.
    pub mapped_forcesnd_shapes: [u16; 5],
}

/// The player alien fields sound.c reads (`sound_get_player` result).
#[derive(Clone, Copy, Debug, Default)]
pub struct SoundPlayer {
    /// `collflags & ACF_FIRSTFRAME`
    pub first_frame: bool,
    pub worldx: i16,
    pub worldz: i16,
}

/// One active-list alien as seen by nearobjs / do_obstacles.  `donesnd` is
/// written back by `update` (C sets ASF4_DONESND on the alien).
#[derive(Clone, Copy, Debug, Default)]
pub struct SoundObj {
    /// `sound_encode_alien`: alien index + 1; 0 = none.
    pub id: u16,
    pub shape: u16,
    /// `flags & AFEXP` (exploding)
    pub exploding: bool,
    pub snd1: u8,
    pub snd2: u8,
    pub worldx: i16,
    pub worldz: i16,
    /// `al->HP` (0xFF marks obstacles)
    pub hp: u8,
    /// `sflags3 & ASF3_REALOBJ`
    pub realobj: bool,
    /// `sflags4 & ASF4_DONESND`
    pub donesnd: bool,
}

// ---------------------------------------------------------------------------
// Sound state (the file-scope globals of sound.c).
// ---------------------------------------------------------------------------
pub struct Sound {
    // setport3_l SFX ring (g_sdport3 / g_sdspt3 / g_sdgpt3 / g_sdpck3).
    sdport3: [u8; 16],
    sdspt3: u8,
    sdgpt3: u8,
    sdpck3: u8,
    /// `g_pausesnd` — pause command that flushes the ring when set.
    pausesnd: u8,
    /// `g_nosetport3` — SFX queueing disabled (path scripts set this).
    nosetport3: bool,
    lastblock: u16,
    lastplayx: i16,
    port1bolox: u8,
    /// `g_tpa` — last engine value written to port 1.
    tpa: u8,
    // Level-entry BGM boot latch (C s_music_map / s_music_booted).
    music_map: u32,
    music_booted: bool,
    music_track: Option<u8>,
}

impl Default for Sound {
    fn default() -> Self {
        Self::new()
    }
}

const CORNERIA_INTRO_BG: u16 = 0;
const CORNERIA_GROUND_BG: u16 = 4;

/// BGS.ASM bg_* -> `bgm N` mapping after level-entry intro handling.
/// `bg_x_1c` uses `bgm 11` for all Corneria levels. Returns None for maps
/// with no auto boot.
pub fn sound_track_for_map(map_id: u32) -> Option<u8> {
    match map_id {
        MAP_ID_1_1 | MAP_ID_2_1 | MAP_ID_3_1 => Some(catalog::SND_11),
        MAP_ID_1_2 => Some(catalog::SND_12),
        MAP_ID_1_3 => Some(catalog::SND_13),
        MAP_ID_1_4 => Some(catalog::SND_14),
        MAP_ID_1_5 => Some(catalog::SND_15),
        MAP_ID_1_6 => Some(catalog::SND_16),
        MAP_ID_2_2 => Some(catalog::SND_22),
        MAP_ID_2_3 => Some(catalog::SND_23),
        MAP_ID_2_4 => Some(catalog::SND_24),
        MAP_ID_2_5 => Some(catalog::SND_25),
        MAP_ID_2_6 => Some(catalog::SND_26),
        MAP_ID_3_2 => Some(catalog::SND_32),
        MAP_ID_3_3 => Some(catalog::SND_33),
        MAP_ID_3_4 => Some(catalog::SND_34),
        MAP_ID_3_5 => Some(catalog::SND_35),
        MAP_ID_3_6 => Some(catalog::SND_36),
        MAP_ID_3_7 => Some(catalog::SND_37),
        MAP_ID_BLACKHOLE => Some(catalog::SND_BHOLE),
        MAP_ID_SPECIAL => Some(catalog::SND_SPECIAL),
        MAP_ID_TRAINING => Some(catalog::SND_TRAINING),
        // FINALMAP.ASM drives $12/$13 within the venom/andross bank.
        MAP_ID_FINAL => Some(catalog::SND_16),
        _ => None, // no auto boot
    }
}

#[inline]
fn is_corneria_scramble_map(map_id: u32) -> bool {
    matches!(map_id, MAP_ID_1_1 | MAP_ID_2_1 | MAP_ID_3_1)
}

/// C `sound_pan_from_angle`.
fn pan_from_angle(angle: u8) -> u8 {
    if angle <= 124 {
        0x40
    } else if angle <= 129 {
        0x80
    } else {
        0xC0
    }
}

/// XZ-plane octagonal distance (`xzdiffs_l`) — shared with strat/path.
#[inline]
fn xzdiffs_rangexz(px: i16, pz: i16, ox: i16, oz: i16) -> i16 {
    sf_core::aim_angle::xzdiffs(px.wrapping_sub(ox), pz.wrapping_sub(oz))
}

/// ROM `Yanglexy_l` / `anglexy_l` for nearobjs pan (SOUND.ASM:617).
#[inline]
fn angle_xz(src: &SoundPlayer, dst: &SoundObj) -> u8 {
    sf_core::aim_angle::yanglexy(
        dst.worldx.wrapping_sub(src.worldx),
        dst.worldz.wrapping_sub(src.worldz),
    )
}

impl Sound {
    pub fn new() -> Self {
        Sound {
            sdport3: [0; 16],
            sdspt3: 0,
            sdgpt3: 0,
            sdpck3: 0,
            pausesnd: 0,
            nosetport3: false,
            lastblock: 0,
            lastplayx: 0,
            port1bolox: 0,
            tpa: 0,
            music_map: 0xFFFF_FFFF,
            music_booted: false,
            music_track: None,
        }
    }

    /// C `Sound_Init`: reset the ring / port state and silence ports 1-2.
    /// (SPC upload/handshake stays out of this pass, like the C.)
    pub fn init(&mut self, backend: &mut dyn SoundBackend) {
        self.sdspt3 = 0;
        self.sdgpt3 = 0;
        self.sdpck3 = 0;
        self.pausesnd = 0;
        self.lastblock = 0;
        self.lastplayx = 0;
        self.port1bolox = 0;
        self.music_map = 0xFFFF_FFFF;
        self.music_booted = false;
        self.music_track = None;
        backend.set_engine_sound(0);
        backend.set_ambient_sound(0);
    }

    /// C `Sound_Update` (the dosounds_l tick, gameplay only).
    ///
    /// `objs` is the active list in list order; `donesnd` may be set on
    /// entries (write it back to the aliens' ASF4_DONESND).
    pub fn update(
        &mut self,
        state: &SoundGameState,
        player: Option<&SoundPlayer>,
        objs: &mut [SoundObj],
        backend: &mut dyn SoundBackend,
    ) {
        // BGS.ASM level-entry `bgm N` boot.
        self.level_music_tick(state, backend);

        // dosounds_l
        let mut mute_port2 = state.in_game && state.player_hp0;

        if !mute_port2
            && matches!(
                state.level_finished,
                LE_ENTERSPEC | LE_ENTERBHOLE | LE_BHOLE1 | LE_BHOLE2 | LE_BHOLE3
            )
        {
            mute_port2 = true;
        }

        if !mute_port2 {
            self.nearobjs(state, player, objs, backend);
            self.do_obstacles(state, player, objs);
        } else {
            backend.set_ambient_sound(0);
        }

        self.playersnd(state, player, backend);
        self.drain_port3_queue(backend);
    }

    /// C `Sound_PlaySE` (setport3_l ring queue write).
    pub fn play_se(&mut self, state: &SoundGameState, sound_id: u8) {
        if self.nosetport3 {
            return;
        }
        if state.in_game && state.player_hp0 {
            return;
        }
        self.sdport3[(self.sdspt3 & 0x0F) as usize] = sound_id;
        self.sdspt3 = self.sdspt3.wrapping_add(1) & 0x0F;
    }

    /// C `makesnd` (SOUND.ASM:899-945): distance-attenuated positional
    /// ONE-SHOT SE for an enemy weapon / door / wall / sea event, keyed to a
    /// `*sound_l` family. Call it from a spawn/impact site (e.g. an enemy
    /// laser fires `make_snd(.., &POS_LASER)`), passing the SOURCE object's
    /// world XZ and the player. It selects near L/C/R, far, or silence by the
    /// `xzdiffs` range and the signed x-offset, then queues the chosen id
    /// through the setport3 ring (same gate/ring as one-shot `play_se`).
    ///
    /// Returns the queued SE id (None = silence, past `cutoffsnd`), mostly for
    /// testing. The mid band (`family.m`) is unreachable here, matching ROM.
    pub fn make_snd(
        &mut self,
        state: &SoundGameState,
        player: &SoundPlayer,
        obj_worldx: i16,
        obj_worldz: i16,
        family: &PosSndFamily,
    ) -> Option<u8> {
        let id = resolve_positional_effect(
            state.pviewposx,
            player.worldx,
            player.worldz,
            obj_worldx,
            obj_worldz,
            family,
        )?;

        self.play_se(state, id);
        Some(id)
    }

    /// C `Sound_Play`: immediate effect command.
    pub fn play(&mut self, backend: &mut dyn SoundBackend, sound_id: u8) {
        backend.play_immediate(sound_id);
    }

    /// Source `startbgm`: start a driver cue from the currently loaded package.
    pub fn start_music_cue(&mut self, backend: &mut dyn SoundBackend, cue: u8) {
        backend.start_music(cue);
    }

    /// Source `bgm`: load a sound package and start its catalog entry cue.
    pub fn boot_music_track(&mut self, backend: &mut dyn SoundBackend, track: u8) {
        backend.load_track(track);
        backend.start_music(catalog::track_start_cue(track));
    }

    /// ROM `do_bgm_init` (SOUND.ASM:47) — `bootapu #snd_init`.
    pub fn do_bgm_init(&mut self, backend: &mut dyn SoundBackend) {
        self.boot_music_track(backend, catalog::SND_INIT);
    }

    /// ROM `do_bgm_continue` (SOUND.ASM:75) — `bootapu #snd_continue`.
    pub fn do_bgm_continue(&mut self, backend: &mut dyn SoundBackend) {
        self.boot_music_track(backend, catalog::SND_CONTINUE);
    }

    /// C `Sound_StopMusic`: startbgm 0 — tell the driver to stop the song.
    pub fn stop_music(&mut self, backend: &mut dyn SoundBackend) {
        backend.start_music(0);
    }

    /// `g_pausesnd` setter: next drain flushes the ring and forces this
    /// command onto port 3.
    pub fn set_pause_snd(&mut self, cmd: u8) {
        self.pausesnd = cmd;
    }

    /// `g_nosetport3` setter (path scripts disable SFX queueing).
    pub fn set_nosetport3(&mut self, disabled: bool) {
        self.nosetport3 = disabled;
    }

    /// Last engine value written to port 1 (`g_tpa`).
    pub fn tpa(&self) -> u8 {
        self.tpa
    }

    // -----------------------------------------------------------------------
    // Level-entry BGM boot (C `sound_level_music_tick`).  While the player
    // is dead (death anim / game over / continue), drop the boot latch so
    // the respawn reboots the level bank — mirroring the original's
    // initlevel -> BGS `bgm N` on restart.
    // -----------------------------------------------------------------------
    fn level_music_tick(&mut self, state: &SoundGameState, backend: &mut dyn SoundBackend) {
        if state.player_dead {
            self.music_booted = false;
            return;
        }

        let map_changed = self.music_map != state.new_map;
        if map_changed {
            self.music_map = state.new_map;
            self.music_booted = false;
        }

        if !self.music_booted {
            self.music_booted = true;

            let track = if is_corneria_scramble_map(state.new_map)
                && state.current_bg == CORNERIA_INTRO_BG
            {
                Some(catalog::SND_10)
            } else {
                sound_track_for_map(state.new_map)
            };

            if let Some(track) = track {
                self.music_track = Some(track);
                self.boot_music_track(backend, track);
            }
            return;
        }

        // BGS.ASM bg_1_1i_1 starts SND_10/cue $10, then bg_1_1c_1
        // switches to SND_11/cue $03.  This transition is observable from
        // the live background id; it must happen once, not once per tick.
        if is_corneria_scramble_map(state.new_map)
            && self.music_track == Some(catalog::SND_10)
            && state.current_bg == CORNERIA_GROUND_BG
        {
            self.music_track = Some(catalog::SND_11);
            self.boot_music_track(backend, catalog::SND_11);
        }
    }

    // -----------------------------------------------------------------------
    // SFX dispatch with consumption handshake (C `sound_drain_port3_queue`;
    // IRQ.ASM startmus .trig3, lines 1612-1644): only send the next queued
    // SFX once the SPC driver has echoed the previous one back on port 3.
    // -----------------------------------------------------------------------
    fn drain_port3_queue(&mut self, backend: &mut dyn SoundBackend) {
        if self.sdpck3 != 0 {
            if !backend.effect_consumed(self.sdpck3) {
                return; // .reject — SPC hasn't consumed it yet
            }
            self.sdpck3 = 0;
            backend.clear_effect_acknowledgement();
        }

        if self.pausesnd != 0 {
            // .pause — flush queue, force command
            backend.set_paused(self.pausesnd == 2);
            self.sdpck3 = self.pausesnd;
            self.sdspt3 = 0;
            self.sdgpt3 = 0;
            self.pausesnd = 0;
            return;
        }

        if self.sdgpt3 == self.sdspt3 {
            return;
        }
        let snd = self.sdport3[(self.sdgpt3 & 0x0F) as usize];
        backend.play_effect(snd);
        self.sdpck3 = snd;
        self.sdgpt3 = self.sdgpt3.wrapping_add(1) & 0x0F;
    }

    // -----------------------------------------------------------------------
    // playersnd (C `playersnd`): engine sound on port 1.
    // -----------------------------------------------------------------------
    fn playersnd(
        &mut self,
        state: &SoundGameState,
        player: Option<&SoundPlayer>,
        backend: &mut dyn SoundBackend,
    ) {
        let player = match player {
            Some(p) if !p.first_frame => p,
            _ => {
                backend.set_engine_sound(0);
                self.port1bolox = 0;
                return;
            }
        };

        if state.player_dead {
            backend.set_engine_sound(0);
            self.port1bolox = 0;
            return;
        }

        if !state.engine_snd {
            backend.set_engine_sound(0);
            self.port1bolox = 0;
            return;
        }

        if state.player_hp0 {
            backend.set_engine_sound(0x4B);
            self.port1bolox = 0x4B;
            return;
        }

        let mut tpa: u8 = if state.in_a_tunnel != 0 {
            if state.in_a_tunnel == 2 {
                0xC0
            } else {
                0x80
            }
        } else if state.space_mode {
            0x00
        } else {
            0xC0
        };

        tpa |= state.player_snd_flag;

        let contl0 = (state.pad1 & 0xFF) as u8;
        if contl0 & (PAD_TLEFT | PAD_TRIGHT) != 0 {
            let mut accel = player.worldx.wrapping_sub(self.lastplayx);
            self.lastplayx = player.worldx;
            if accel < 0 {
                accel = accel.wrapping_neg();
            }

            if accel < SOUND_PLAYACCEL1 {
                // tpa |= 0x00
            } else if accel < SOUND_PLAYACCEL2 {
                tpa |= 0x01;
            } else if accel < SOUND_PLAYACCEL3 {
                tpa |= 0x02;
            } else {
                tpa |= 0x03;
            }
        }

        self.tpa = tpa;
        backend.set_engine_sound(tpa);
        self.port1bolox = tpa;
    }

    // -----------------------------------------------------------------------
    // do_obstacles (C `do_obstacles`): one-shot doppler SEs for passing
    // obstacles (HP == $FF real objects).
    // -----------------------------------------------------------------------
    fn do_obstacles(
        &mut self,
        state: &SoundGameState,
        player: Option<&SoundPlayer>,
        objs: &mut [SoundObj],
    ) {
        let player = match player {
            Some(p) => *p,
            None => return,
        };

        for al in objs.iter_mut() {
            if al.hp != 0xFF {
                continue;
            }
            if !al.realobj {
                continue;
            }
            if al.donesnd {
                continue;
            }

            let dx = state.pviewposx.wrapping_sub(al.worldx);
            if dx >= 0 {
                if dx >= 300 {
                    continue;
                }
            } else if dx < -300 {
                continue;
            }

            let dz = al.worldz.wrapping_sub(player.worldz);
            if dz < 0 || dz >= SOUND_OBSDIST {
                continue;
            }

            al.donesnd = true;

            if dx >= 0 {
                if dx < 80 {
                    self.play_se(state, SOUND_SE_DOPCENTRE);
                } else {
                    self.play_se(state, SOUND_SE_DOPRIGHT);
                }
            } else if dx >= -80 {
                self.play_se(state, SOUND_SE_DOPCENTRE);
            } else {
                self.play_se(state, SOUND_SE_DOPLEFT);
            }
            return;
        }
    }

    // -----------------------------------------------------------------------
    // nearobjs (C `nearobjs`): nearest-object ambient sound.
    // -----------------------------------------------------------------------
    fn clear_ambient(&mut self, nearest_id: u16, backend: &mut dyn SoundBackend) {
        backend.set_ambient_sound(0);
        self.lastblock = nearest_id;
    }

    fn is_forcesnd_shape(state: &SoundGameState, shape: u16) -> bool {
        if FORCESND_SHAPE_IDS.contains(&shape) {
            return true;
        }
        // sound_shape_matches_mapped: g_shapes_table[idx] != 0 && equal.
        state
            .mapped_forcesnd_shapes
            .iter()
            .any(|&mapped| mapped != 0 && shape == mapped)
    }

    /// C `nearobjs_forcesnd`: dedicated protocol for the big capital ships.
    fn nearobjs_forcesnd(
        &mut self,
        state: &SoundGameState,
        al: &SoundObj,
        player: &SoundPlayer,
        backend: &mut dyn SoundBackend,
    ) {
        let mut snd = al.snd2;
        self.lastblock = al.id;

        let range = al.worldz.wrapping_sub(player.worldz);
        if range < 0 {
            backend.set_ambient_sound(0);
            return;
        }

        let dx = state.pviewposx.wrapping_sub(al.worldx);
        if dx >= 0 {
            snd |= if dx < 80 { 0x80 } else { 0x40 };
        } else {
            snd |= if dx >= -80 { 0x80 } else { 0xC0 };
        }

        if range < SOUND_SHIP1_MINDIST {
            backend.set_ambient_sound(0);
            return;
        }
        if range < SOUND_SHIP1_DIST1 {
            // snd |= 0x00
        } else if range < SOUND_SHIP1_DIST2 {
            snd |= 0x10;
        } else if range < SOUND_SHIP1_DIST3 {
            snd |= 0x20;
        } else if range < SOUND_SHIP1_CUTOFF {
            snd |= 0x30;
        } else {
            backend.set_ambient_sound(0);
            return;
        }

        backend.set_ambient_sound(snd);
    }

    fn nearobjs(
        &mut self,
        state: &SoundGameState,
        player: Option<&SoundPlayer>,
        objs: &[SoundObj],
        backend: &mut dyn SoundBackend,
    ) {
        let player = match player {
            Some(p) => *p,
            None => {
                self.clear_ambient(0, backend);
                return;
            }
        };

        let mut nearest_range: i16 = 0x7FFF;
        let mut nearest: Option<&SoundObj> = None;

        for al in objs {
            if Self::is_forcesnd_shape(state, al.shape) {
                self.nearobjs_forcesnd(state, al, &player, backend);
                return;
            }

            if al.exploding {
                continue;
            }
            if al.snd1 == 0 && al.snd2 == 0 {
                continue;
            }

            let mut range = al.worldz.wrapping_sub(player.worldz);
            if range < 0 {
                range = range.wrapping_neg();
            }

            if nearest_range < range {
                continue;
            }
            nearest = Some(al);
            nearest_range = range;
        }

        let nearest = match nearest {
            Some(n) => n,
            None => {
                self.clear_ambient(0, backend);
                return;
            }
        };

        if nearest.id != self.lastblock {
            self.clear_ambient(nearest.id, backend);
            return;
        }

        if nearest.snd1 != 0 {
            backend.set_ambient_sound(nearest.snd1);
            return;
        }

        let mut snd = nearest.snd2;
        let angle = angle_xz(&player, nearest);
        snd |= pan_from_angle(angle);

        if nearest_range < SOUND_DIST1SND {
            // snd |= 0x00
        } else if nearest_range < SOUND_DIST2SND {
            snd |= 0x10;
        } else if nearest_range < SOUND_DIST3SND {
            snd |= 0x20;
        } else if nearest_range >= SOUND_CUTOFFSND {
            self.clear_ambient(nearest.id, backend);
            return;
        } else {
            snd |= 0x30;
        }

        backend.set_ambient_sound(snd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recording fake backend for protocol tests.
    #[derive(Default)]
    struct FakeBackend {
        port_writes: Vec<(u8, u8)>,
        port3_read: u8,
        bgm: Vec<u8>,
        booted: Vec<u8>,
        paused: Vec<bool>,
    }

    impl SoundBackend for FakeBackend {
        fn set_engine_sound(&mut self, sound: u8) {
            self.port_writes.push((1, sound));
        }
        fn set_ambient_sound(&mut self, sound: u8) {
            self.port_writes.push((2, sound));
        }
        fn play_effect(&mut self, effect: u8) {
            self.port_writes.push((3, effect));
        }
        fn effect_consumed(&mut self, effect: u8) -> bool {
            self.port3_read == effect
        }
        fn clear_effect_acknowledgement(&mut self) {
            self.port_writes.push((3, 0));
        }
        fn start_music(&mut self, cue: u8) {
            self.bgm.push(cue);
        }
        fn load_track(&mut self, track_id: u8) {
            self.booted.push(track_id);
        }
        fn set_paused(&mut self, paused: bool) {
            self.paused.push(paused);
        }
    }

    fn last_write(b: &FakeBackend, port: u8) -> Option<u8> {
        b.port_writes
            .iter()
            .rev()
            .find(|w| w.0 == port)
            .map(|w| w.1)
    }

    #[test]
    fn sfx_ring_acks_before_next_send() {
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let st = SoundGameState::default();

        snd.play_se(&st, 0x41);
        snd.play_se(&st, 0x42);

        // First drain sends 0x41 and latches sdpck3.
        snd.drain_port3_queue(&mut be);
        assert_eq!(last_write(&be, 3), Some(0x41));

        // Driver hasn't echoed yet -> nothing new goes out (.reject).
        be.port_writes.clear();
        be.port3_read = 0;
        snd.drain_port3_queue(&mut be);
        assert!(be.port_writes.is_empty());

        // Echo arrives -> ack with 0, then 0x42 on the following drain.
        be.port3_read = 0x41;
        snd.drain_port3_queue(&mut be);
        assert_eq!(be.port_writes, vec![(3, 0), (3, 0x42)]);
    }

    #[test]
    fn pausesnd_flushes_ring() {
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let st = SoundGameState::default();

        snd.play_se(&st, 0x41);
        snd.play_se(&st, 0x42);
        snd.set_pause_snd(2);

        snd.drain_port3_queue(&mut be);
        // .pause branch wins: queue flushed, forced command sent.
        assert_eq!(be.paused, vec![true]);
        assert_eq!(snd.sdspt3, 0);
        assert_eq!(snd.sdgpt3, 0);
        assert_eq!(snd.sdpck3, 2);
        assert_eq!(snd.pausesnd, 0);
    }

    #[test]
    fn play_se_gated_by_hp0_and_nosetport3() {
        let mut snd = Sound::new();
        let st = SoundGameState {
            in_game: true,
            player_hp0: true,
            ..Default::default()
        };
        snd.play_se(&st, 0x41);
        assert_eq!(snd.sdspt3, 0, "in-game HP0 drops SEs");

        let st = SoundGameState::default();
        snd.set_nosetport3(true);
        snd.play_se(&st, 0x41);
        assert_eq!(snd.sdspt3, 0, "nosetport3 drops SEs");
    }

    #[test]
    fn track_for_map_matches_oracle_table() {
        assert_eq!(sound_track_for_map(MAP_ID_1_1), Some(catalog::SND_11));
        assert_eq!(sound_track_for_map(MAP_ID_2_1), Some(catalog::SND_11));
        assert_eq!(sound_track_for_map(MAP_ID_3_1), Some(catalog::SND_11));
        assert_eq!(sound_track_for_map(MAP_ID_1_4), Some(catalog::SND_14));
        assert_eq!(
            sound_track_for_map(MAP_ID_BLACKHOLE),
            Some(catalog::SND_BHOLE)
        );
        assert_eq!(sound_track_for_map(MAP_ID_FINAL), Some(catalog::SND_16));
        assert_eq!(
            sound_track_for_map(MAP_ID_TRAINING),
            Some(catalog::SND_TRAINING)
        );
        assert_eq!(sound_track_for_map(0), None);
        assert_eq!(sound_track_for_map(24), None); // title map: no auto boot
    }

    #[test]
    fn level_entry_boots_track_once_and_rearms_on_death() {
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let mut st = SoundGameState {
            new_map: MAP_ID_1_1,
            ..Default::default()
        };

        snd.update(&st, None, &mut [], &mut be);
        assert_eq!(be.booted, vec![catalog::SND_10]);
        assert_eq!(be.bgm, vec![catalog::track_start_cue(catalog::SND_10)]);

        // Same map again: latched, no reboot.
        snd.update(&st, None, &mut [], &mut be);
        assert_eq!(be.booted.len(), 1);

        // A checkpoint restart occurs after the intro, on bg_1_1c_1.
        st.current_bg = CORNERIA_GROUND_BG;
        // Death drops the latch; respawn reboots the level bank.
        st.player_dead = true;
        snd.update(&st, None, &mut [], &mut be);
        st.player_dead = false;
        snd.update(&st, None, &mut [], &mut be);
        assert_eq!(
            be.booted,
            vec![catalog::SND_10, catalog::SND_11],
            "checkpoint restart must not replay the scramble announcement"
        );
    }

    #[test]
    fn corneria_scramble_switches_from_intro_bank_at_ground_background() {
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let mut st = SoundGameState {
            new_map: MAP_ID_1_1,
            current_bg: CORNERIA_INTRO_BG,
            ..Default::default()
        };

        snd.update(&st, None, &mut [], &mut be);
        assert_eq!(be.booted, vec![catalog::SND_10]);

        st.current_bg = CORNERIA_GROUND_BG;
        snd.update(&st, None, &mut [], &mut be);
        assert_eq!(be.booted, vec![catalog::SND_10, catalog::SND_11]);
        assert_eq!(
            be.bgm,
            vec![
                catalog::track_start_cue(catalog::SND_10),
                catalog::track_start_cue(catalog::SND_11)
            ]
        );

        // The background remains ground for subsequent ticks; do not reboot.
        snd.update(&st, None, &mut [], &mut be);
        assert_eq!(be.booted, vec![catalog::SND_10, catalog::SND_11]);
    }

    #[test]
    fn corneria_blink_background_does_not_switch_music_bank() {
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let mut st = SoundGameState {
            new_map: MAP_ID_1_1,
            current_bg: CORNERIA_INTRO_BG,
            ..Default::default()
        };

        snd.update(&st, None, &mut [], &mut be);
        st.current_bg = 1; // bg_1_1a_1: blink-only, no BGM command
        snd.update(&st, None, &mut [], &mut be);
        assert_eq!(be.booted, vec![catalog::SND_10]);
    }

    #[test]
    fn music_cues_and_track_boots_are_explicit() {
        const GAMEPLAY_BOSS_CUE: u8 = 5;

        let mut snd = Sound::new();
        let mut be = FakeBackend::default();

        snd.start_music_cue(&mut be, GAMEPLAY_BOSS_CUE);
        assert_eq!(be.bgm, vec![GAMEPLAY_BOSS_CUE]);
        assert!(be.booted.is_empty());

        snd.start_music_cue(&mut be, catalog::MUSIC_ALL_CLEAR);
        assert_eq!(be.bgm, vec![GAMEPLAY_BOSS_CUE, catalog::MUSIC_ALL_CLEAR]);
        assert!(be.booted.is_empty());

        snd.boot_music_track(&mut be, catalog::SND_TITLE);
        assert_eq!(be.booted, vec![catalog::SND_TITLE]);
        assert_eq!(
            be.bgm,
            vec![
                GAMEPLAY_BOSS_CUE,
                catalog::MUSIC_ALL_CLEAR,
                catalog::track_start_cue(catalog::SND_TITLE),
            ]
        );

        snd.boot_music_track(&mut be, catalog::SND_MAP);
        assert_eq!(be.booted, vec![catalog::SND_TITLE, catalog::SND_MAP]);
        assert_eq!(
            be.bgm,
            vec![
                GAMEPLAY_BOSS_CUE,
                catalog::MUSIC_ALL_CLEAR,
                catalog::track_start_cue(catalog::SND_TITLE),
                catalog::track_start_cue(catalog::SND_MAP),
            ]
        );
    }

    #[test]
    fn do_bgm_init_and_continue_bootapu() {
        // SOUND.ASM:47/75 — thin bootapu wrappers over SND_INIT / SND_CONTINUE.
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        snd.do_bgm_init(&mut be);
        assert_eq!(be.booted, vec![catalog::SND_INIT]);
        assert_eq!(be.bgm, vec![catalog::track_start_cue(catalog::SND_INIT)]);
        snd.do_bgm_continue(&mut be);
        assert_eq!(be.booted, vec![catalog::SND_INIT, catalog::SND_CONTINUE]);
        assert_eq!(
            be.bgm,
            vec![
                catalog::track_start_cue(catalog::SND_INIT),
                catalog::track_start_cue(catalog::SND_CONTINUE),
            ]
        );
    }

    #[test]
    fn playersnd_engine_states() {
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let player = SoundPlayer::default();

        // Engine flag clear -> port1 0.
        let st = SoundGameState::default();
        snd.playersnd(&st, Some(&player), &mut be);
        assert_eq!(last_write(&be, 1), Some(0));

        // HP0 -> falling whine $4B.
        let st = SoundGameState {
            engine_snd: true,
            player_hp0: true,
            ..Default::default()
        };
        snd.playersnd(&st, Some(&player), &mut be);
        assert_eq!(last_write(&be, 1), Some(0x4B));

        // Space mode idle engine -> $00 base.
        let st = SoundGameState {
            engine_snd: true,
            space_mode: true,
            ..Default::default()
        };
        snd.playersnd(&st, Some(&player), &mut be);
        assert_eq!(last_write(&be, 1), Some(0x00));

        // Ground mode -> $C0 base; tunnel variant 2 -> $C0, tunnel 1 -> $80.
        let st = SoundGameState {
            engine_snd: true,
            ..Default::default()
        };
        snd.playersnd(&st, Some(&player), &mut be);
        assert_eq!(last_write(&be, 1), Some(0xC0));
        let st = SoundGameState {
            engine_snd: true,
            in_a_tunnel: 1,
            ..Default::default()
        };
        snd.playersnd(&st, Some(&player), &mut be);
        assert_eq!(last_write(&be, 1), Some(0x80));

        // Thruster held: accel bits from |worldx - lastplayx|.
        let st = SoundGameState {
            engine_snd: true,
            space_mode: true,
            pad1: (PAD_TLEFT as u16) | 0xFF00, // high byte ignored
            ..Default::default()
        };
        snd.lastplayx = 0;
        let player = SoundPlayer {
            worldx: 5, // accel 5 -> 0x02 band
            ..Default::default()
        };
        snd.playersnd(&st, Some(&player), &mut be);
        assert_eq!(last_write(&be, 1), Some(0x02));
        assert_eq!(snd.lastplayx, 5);
    }

    #[test]
    fn nearobjs_two_pass_lastblock_gate() {
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let st = SoundGameState::default();
        let player = SoundPlayer::default();
        let objs = [SoundObj {
            id: 7,
            snd1: 0x33,
            worldz: 100,
            ..Default::default()
        }];

        // First sighting: port2 0, lastblock latches the candidate.
        snd.nearobjs(&st, Some(&player), &objs, &mut be);
        assert_eq!(last_write(&be, 2), Some(0));
        assert_eq!(snd.lastblock, 7);

        // Second tick, same nearest: snd1 goes straight out.
        snd.nearobjs(&st, Some(&player), &objs, &mut be);
        assert_eq!(last_write(&be, 2), Some(0x33));
    }

    #[test]
    fn nearobjs_snd2_distance_and_pan() {
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let st = SoundGameState::default();
        let player = SoundPlayer::default();
        // Dead ahead (angle 0 -> pan 0x40), range 300 -> 0x10 band.
        let objs = [SoundObj {
            id: 3,
            snd2: 0x05,
            worldz: 300,
            ..Default::default()
        }];
        snd.lastblock = 3;
        snd.nearobjs(&st, Some(&player), &objs, &mut be);
        assert_eq!(last_write(&be, 2), Some(0x05 | 0x40 | 0x10));

        // Past the cutoff: silence + relatch.
        let objs = [SoundObj {
            id: 3,
            snd2: 0x05,
            worldz: 3200,
            ..Default::default()
        }];
        snd.nearobjs(&st, Some(&player), &objs, &mut be);
        assert_eq!(last_write(&be, 2), Some(0));
    }

    #[test]
    fn sound_aim_helpers_match_sf_core() {
        // makesnd rangexz + nearobjs pan angle share aim_angle with strat.
        assert_eq!(
            xzdiffs_rangexz(0, 0, 300, 400),
            sf_core::aim_angle::xzdiffs(-300, -400)
        );
        assert_eq!(
            xzdiffs_rangexz(0, 0, 300, 400),
            sf_core::aim_angle::xzdiffs(300, 400)
        );
        let player = SoundPlayer {
            worldx: 0,
            worldz: 0,
            ..Default::default()
        };
        let obj = SoundObj {
            worldx: 1000,
            worldz: 0,
            ..Default::default()
        };
        assert_eq!(
            angle_xz(&player, &obj),
            sf_core::aim_angle::yanglexy(1000, 0)
        );
        assert_eq!(angle_xz(&player, &obj), 64); // +X
                                                 // +X object → pan bit from angle 64.
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let st = SoundGameState::default();
        let objs = [SoundObj {
            id: 7,
            snd2: 0x01,
            worldx: 1000,
            worldz: 0,
            ..Default::default()
        }];
        snd.lastblock = 7;
        snd.nearobjs(&st, Some(&player), &objs, &mut be);
        let out = last_write(&be, 2).expect("port2");
        assert_eq!(out & 0x0F, 0x01);
        assert_ne!(out & 0xC0, 0, "pan bits set from yanglexy");
    }

    #[test]
    fn forcesnd_shape_takes_over_port2() {
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let st = SoundGameState::default();
        let player = SoundPlayer::default();
        // Capital ship shape 21 at range 6000 dead ahead:
        // snd2 | 0x80 (centre) | 0x10 (DIST1..DIST2 band).
        let objs = [SoundObj {
            id: 9,
            shape: 21,
            snd2: 0x04,
            worldz: 6000,
            ..Default::default()
        }];
        snd.nearobjs(&st, Some(&player), &objs, &mut be);
        assert_eq!(last_write(&be, 2), Some(0x04 | 0x80 | 0x10));
        assert_eq!(snd.lastblock, 9);

        // Behind the player: silence.
        let objs = [SoundObj {
            id: 9,
            shape: 21,
            snd2: 0x04,
            worldz: -10,
            ..Default::default()
        }];
        snd.nearobjs(&st, Some(&player), &objs, &mut be);
        assert_eq!(last_write(&be, 2), Some(0));
    }

    #[test]
    fn nearobjs_picks_nearest_of_several() {
        // The per-frame looping pass drives port 2 from the single NEAREST
        // sounding object (SOUND.ASM nearobjs ktpx/ktpy).
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let st = SoundGameState::default();
        let player = SoundPlayer::default();
        let objs = [
            SoundObj {
                id: 1,
                snd1: 0x11,
                worldz: 500,
                ..Default::default()
            },
            SoundObj {
                id: 2,
                snd1: 0x22,
                worldz: 100,
                ..Default::default()
            }, // nearest
            SoundObj {
                id: 3,
                snd1: 0x33,
                worldz: 800,
                ..Default::default()
            },
        ];
        // Pre-latch the nearest so its snd1 goes straight out (2nd-pass gate).
        snd.lastblock = 2;
        snd.nearobjs(&st, Some(&player), &objs, &mut be);
        assert_eq!(last_write(&be, 2), Some(0x22));
    }

    #[test]
    fn make_snd_near_lcr_selection() {
        // Near band (small range): L/C/R chosen by dx = pviewposx - obj.worldx.
        let mut snd = Sound::new();
        let player = SoundPlayer {
            worldx: 0,
            worldz: 0,
            ..Default::default()
        };
        // obj at (0,100): rangexz well under the 2000 near threshold.
        let ox = 0;
        let oz = 100;

        // dx = +200 (>= +170) -> lsnd.
        let st = SoundGameState {
            pviewposx: 200,
            ..Default::default()
        };
        assert_eq!(
            snd.make_snd(&st, &player, ox, oz, &POS_LASER),
            Some(POS_LASER.l)
        );
        assert_eq!(snd.sdport3[0], POS_LASER.l);

        // dx = +100 (< +170) -> csnd.
        let st = SoundGameState {
            pviewposx: 100,
            ..Default::default()
        };
        assert_eq!(
            snd.make_snd(&st, &player, ox, oz, &POS_LASER),
            Some(POS_LASER.c)
        );

        // dx = -200 (< -170) -> rsnd.
        let st = SoundGameState {
            pviewposx: -200,
            ..Default::default()
        };
        assert_eq!(
            snd.make_snd(&st, &player, ox, oz, &POS_LASER),
            Some(POS_LASER.r)
        );
    }

    #[test]
    fn make_snd_far_band_and_silence() {
        let mut snd = Sound::new();
        let st = SoundGameState::default();
        let player = SoundPlayer::default();

        // obj dead ahead at z=2500 -> rangexz ~2109, in [2000,3150): far.
        assert_eq!(
            snd.make_snd(&st, &player, 0, 2500, &POS_LASER),
            Some(POS_LASER.f)
        );

        // obj at z=4000 -> rangexz ~3375 >= cutoffsnd: silence, nothing queued.
        let spt = snd.sdspt3;
        assert_eq!(snd.make_snd(&st, &player, 0, 4000, &POS_LASER), None);
        assert_eq!(snd.sdspt3, spt, "silence queues no SE");
    }

    #[test]
    fn positional_effect_resolver_is_pure_and_uses_source_bands() {
        assert_eq!(
            resolve_positional_effect(0, 0, 0, 200, 0, &POS_LASER),
            Some(POS_LASER.r)
        );
        assert_eq!(
            resolve_positional_effect(0, 0, 0, 0, 2500, &POS_DOORCLOSE),
            Some(POS_DOORCLOSE.f)
        );
        assert_eq!(
            resolve_positional_effect(0, 0, 0, 0, 4000, &POS_DOORCLOSE),
            None
        );
    }

    #[test]
    fn make_snd_family_ids_match_oracle() {
        // Spot-check a couple of families against SOUND.ASM/SOUNDEQU.INC.
        let mut snd = Sound::new();
        let st = SoundGameState::default();
        let player = SoundPlayer::default();
        // Near, centred.
        assert_eq!(snd.make_snd(&st, &player, 0, 50, &POS_MISSILE), Some(0x3c));
        assert_eq!(snd.make_snd(&st, &player, 0, 50, &POS_DAMENEMY), Some(0x24));
        assert_eq!(
            snd.make_snd(&st, &player, 0, 50, &POS_ENEMYUPSEA),
            Some(0x69)
        );
        // Findings F1-F4 families: door + sea, near-centre ids.
        assert_eq!(snd.make_snd(&st, &player, 0, 50, &POS_DOOROPEN), Some(0x54));
        assert_eq!(
            snd.make_snd(&st, &player, 0, 50, &POS_DOORCLOSE),
            Some(0x52)
        );
        assert_eq!(
            snd.make_snd(&st, &player, 0, 50, &POS_ENEMYDOWNSEA),
            Some(0x75)
        );
        // Far.
        assert_eq!(
            snd.make_snd(&st, &player, 0, 2500, &POS_HITWALL),
            Some(0x29)
        );
        // F1/F2 door far bands collapse to the mid/far id.
        assert_eq!(
            snd.make_snd(&st, &player, 0, 2500, &POS_DOOROPEN),
            Some(0x55)
        );
        assert_eq!(
            snd.make_snd(&st, &player, 0, 2500, &POS_DOORCLOSE),
            Some(0x53)
        );
    }

    #[test]
    fn make_snd_respects_hp0_gate() {
        // makesnd routes through setport3_l: the in-game HP0 gate drops it,
        // same as one-shot trigse.
        let mut snd = Sound::new();
        let st = SoundGameState {
            in_game: true,
            player_hp0: true,
            ..Default::default()
        };
        let player = SoundPlayer::default();
        // Selection still returns the id, but the ring stays empty.
        assert_eq!(
            snd.make_snd(&st, &player, 0, 50, &POS_LASER),
            Some(POS_LASER.c)
        );
        assert_eq!(snd.sdspt3, 0, "HP0 gate drops the positional SE");
    }

    #[test]
    fn make_snd_and_oneshot_trigse_share_ring() {
        // Regression: one-shot trigse (play_se) still fires alongside the new
        // positional layer; both feed the same FIFO ring and drain in order.
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();
        let st = SoundGameState::default();
        let player = SoundPlayer::default();

        // Positional laser (near, centred) then a one-shot se_laser $35.
        assert_eq!(
            snd.make_snd(&st, &player, 0, 50, &POS_LASER),
            Some(POS_LASER.c)
        );
        snd.play_se(&st, 0x35);
        assert_eq!(snd.sdport3[0], POS_LASER.c);
        assert_eq!(snd.sdport3[1], 0x35);

        // Drain sends the positional id first (FIFO).
        snd.drain_port3_queue(&mut be);
        assert_eq!(last_write(&be, 3), Some(POS_LASER.c));
        // Ack, then the one-shot trigse goes out next.
        be.port3_read = POS_LASER.c;
        snd.drain_port3_queue(&mut be);
        assert_eq!(last_write(&be, 3), Some(0x35));
    }

    #[test]
    fn obstacles_fire_once_with_doppler_pan() {
        let mut snd = Sound::new();
        let st = SoundGameState {
            pviewposx: 0,
            ..Default::default()
        };
        let player = SoundPlayer::default();
        let mut objs = [SoundObj {
            id: 1,
            hp: 0xFF,
            realobj: true,
            worldx: -100, // dx = 100 -> right of centre
            worldz: 50,
            ..Default::default()
        }];

        snd.do_obstacles(&st, Some(&player), &mut objs);
        assert!(objs[0].donesnd);
        // Ring got the right-doppler SE.
        assert_eq!(snd.sdport3[0], SOUND_SE_DOPRIGHT);

        // Marked done: no double fire.
        let spt = snd.sdspt3;
        snd.do_obstacles(&st, Some(&player), &mut objs);
        assert_eq!(snd.sdspt3, spt);
    }
}
