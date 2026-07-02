//! Game-side sound layer (C oracle: `src/game/sound.c`, itself a literal
//! port of SOUND.ASM dosounds_l / playersnd / nearobjs / do_obstacles /
//! setport3_l).
//!
//! Game-state inputs arrive via plain data structs ([`SoundGameState`],
//! [`SoundPlayer`], [`SoundObj`]) and port I/O goes through the
//! [`SoundBackend`] trait, so this module does not depend on sf-game.

use crate::boot;

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
// Backend: APU port I/O + track boot (implemented over SpcPlayer by the app;
// C equivalents SpcPlayer_WritePort / SpcPlayer_ReadPort / SpcPlayer_StartBgm
// / SpcPlayer_LoadTrack+SpcBoot_TrackCommand).
// ---------------------------------------------------------------------------
pub trait SoundBackend {
    fn write_port(&mut self, port: u8, value: u8);
    fn read_port(&mut self, port: u8) -> u8;
    /// Queue a BGM start command (C `SpcPlayer_StartBgm`).
    fn start_bgm(&mut self, cmd: u8);
    /// Upload a sndtbl track row via the IPL boot (C `SpcPlayer_LoadTrack`).
    fn load_track(&mut self, track_id: u8);
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
}

impl Default for Sound {
    fn default() -> Self {
        Self::new()
    }
}

/// BGS.ASM bg_* -> `bgm N` mapping, ground/main variant per level
/// (C `sound_track_for_map`).  bg_x_1c uses `bgm 11` for all Corneria
/// levels; the tunnel intro bank (`bgm 10`, sound5) is skipped because the
/// port can't observe BG switches.  Returns None for maps with no auto boot.
pub fn sound_track_for_map(map_id: u32) -> Option<u8> {
    match map_id {
        MAP_ID_1_1 | MAP_ID_2_1 | MAP_ID_3_1 => Some(boot::SND_11), // BGS.ASM:106/127 `bgm 11`
        MAP_ID_1_2 => Some(boot::SND_12),
        MAP_ID_1_3 => Some(boot::SND_13),
        MAP_ID_1_4 => Some(boot::SND_14),
        MAP_ID_1_5 => Some(boot::SND_15),
        MAP_ID_1_6 => Some(boot::SND_16),
        MAP_ID_2_2 => Some(boot::SND_22),
        MAP_ID_2_3 => Some(boot::SND_23),
        MAP_ID_2_4 => Some(boot::SND_24),
        MAP_ID_2_5 => Some(boot::SND_25),
        MAP_ID_2_6 => Some(boot::SND_26),
        MAP_ID_3_2 => Some(boot::SND_32),
        MAP_ID_3_3 => Some(boot::SND_33),
        MAP_ID_3_4 => Some(boot::SND_34),
        MAP_ID_3_5 => Some(boot::SND_35),
        MAP_ID_3_6 => Some(boot::SND_36),
        MAP_ID_3_7 => Some(boot::SND_37),
        MAP_ID_BLACKHOLE => Some(boot::SND_BHOLE),
        MAP_ID_SPECIAL => Some(boot::SND_SPECIAL),
        MAP_ID_TRAINING => Some(boot::SND_TRAINING),
        // FINALMAP.ASM drives $12/$13 within the venom/andross bank.
        MAP_ID_FINAL => Some(boot::SND_16),
        _ => None, // no auto boot
    }
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

/// 0-255 angle from src to dst in the XZ plane.  Local copy of
/// `Strat_AngleXZ` (strat_common.c, decompiled from anglexy_l in
/// STRATROU.ASM) so this crate stays independent of the strat lane.
fn angle_xz(src: &SoundPlayer, dst: &SoundObj) -> u8 {
    let dx = (dst.worldx.wrapping_sub(src.worldx)) as f32;
    let dz = (dst.worldz.wrapping_sub(src.worldz)) as f32;
    let mut angle_rad = dx.atan2(dz);
    if angle_rad < 0.0 {
        angle_rad += 2.0 * std::f32::consts::PI;
    }
    (angle_rad * (256.0 / (2.0 * std::f32::consts::PI))) as u8
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
        backend.write_port(1, 0);
        backend.write_port(2, 0);
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
            backend.write_port(2, 0);
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

    /// C `Sound_Play`: immediate driver command (Audio_SendCommand ->
    /// SpcPlayer_SendCommand: cmd on port 0, 0 on port 1).
    pub fn play(&mut self, backend: &mut dyn SoundBackend, sound_id: u8) {
        backend.write_port(0, sound_id);
        backend.write_port(1, 0);
    }

    /// C `Sound_PlayMusic`.
    ///
    /// During gameplay (`in_gameplay`, C `g_game_state == GAME_STATE_PLAYING`)
    /// this matches the original exactly: map `setbgm` ops (WORLD.ASM
    /// setbgmdo) and strat `startbgm` macros write a RAW driver song command
    /// into bgm_music — 5 = boss, 7 = fanfare, $F0/$F1 = fades — played from
    /// the bank booted at level entry.  No sbootapu reboot.  Values >=
    /// SND_TRACK_COUNT are always raw commands.
    ///
    /// Outside gameplay the port's UI/map scripts pass sndtbl track ids
    /// (e.g. the title map's setbgm 2 -> SND_TITLE): full bootapu semantics.
    pub fn play_music(
        &mut self,
        backend: &mut dyn SoundBackend,
        music_id: u8,
        in_gameplay: bool,
    ) {
        if in_gameplay || music_id >= boot::SND_TRACK_COUNT {
            backend.start_bgm(music_id);
            return;
        }
        Self::boot_track(backend, music_id);
    }

    /// C `Sound_StopMusic`: startbgm 0 — tell the driver to stop the song.
    pub fn stop_music(&mut self, backend: &mut dyn SoundBackend) {
        backend.start_bgm(0);
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
    // bootapu: upload the sndtbl row and queue its start command (bgm_music)
    // (C `sound_boot_track`).
    // -----------------------------------------------------------------------
    fn boot_track(backend: &mut dyn SoundBackend, track_id: u8) {
        backend.load_track(track_id);
        backend.start_bgm(boot::track_command(track_id));
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

        if self.music_booted && self.music_map == state.new_map {
            return;
        }

        self.music_map = state.new_map;
        self.music_booted = true;

        if let Some(track) = sound_track_for_map(state.new_map) {
            Self::boot_track(backend, track);
        }
    }

    // -----------------------------------------------------------------------
    // SFX dispatch with consumption handshake (C `sound_drain_port3_queue`;
    // IRQ.ASM startmus .trig3, lines 1612-1644): only send the next queued
    // SFX once the SPC driver has echoed the previous one back on port 3.
    // -----------------------------------------------------------------------
    fn drain_port3_queue(&mut self, backend: &mut dyn SoundBackend) {
        if self.sdpck3 != 0 {
            if backend.read_port(3) != self.sdpck3 {
                return; // .reject — SPC hasn't consumed it yet
            }
            self.sdpck3 = 0;
            backend.write_port(3, 0);
        }

        if self.pausesnd != 0 {
            // .pause — flush queue, force command
            backend.write_port(3, self.pausesnd);
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
        backend.write_port(3, snd);
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
                backend.write_port(1, 0);
                self.port1bolox = 0;
                return;
            }
        };

        if state.player_dead {
            backend.write_port(1, 0);
            self.port1bolox = 0;
            return;
        }

        if !state.engine_snd {
            backend.write_port(1, 0);
            self.port1bolox = 0;
            return;
        }

        if state.player_hp0 {
            backend.write_port(1, 0x4B);
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
        backend.write_port(1, tpa);
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
    // nearobjs (C `nearobjs`): nearest-object sound on port 2.
    // -----------------------------------------------------------------------
    fn set_port2_nothing(&mut self, nearest_id: u16, backend: &mut dyn SoundBackend) {
        backend.write_port(2, 0);
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
            backend.write_port(2, 0);
            return;
        }

        let dx = state.pviewposx.wrapping_sub(al.worldx);
        if dx >= 0 {
            snd |= if dx < 80 { 0x80 } else { 0x40 };
        } else {
            snd |= if dx >= -80 { 0x80 } else { 0xC0 };
        }

        if range < SOUND_SHIP1_MINDIST {
            backend.write_port(2, 0);
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
            backend.write_port(2, 0);
            return;
        }

        backend.write_port(2, snd);
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
                self.set_port2_nothing(0, backend);
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
                self.set_port2_nothing(0, backend);
                return;
            }
        };

        if nearest.id != self.lastblock {
            self.set_port2_nothing(nearest.id, backend);
            return;
        }

        if nearest.snd1 != 0 {
            backend.write_port(2, nearest.snd1);
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
            self.set_port2_nothing(nearest.id, backend);
            return;
        } else {
            snd |= 0x30;
        }

        backend.write_port(2, snd);
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
    }

    impl SoundBackend for FakeBackend {
        fn write_port(&mut self, port: u8, value: u8) {
            self.port_writes.push((port, value));
        }
        fn read_port(&mut self, port: u8) -> u8 {
            assert_eq!(port, 3);
            self.port3_read
        }
        fn start_bgm(&mut self, cmd: u8) {
            self.bgm.push(cmd);
        }
        fn load_track(&mut self, track_id: u8) {
            self.booted.push(track_id);
        }
    }

    fn last_write(b: &FakeBackend, port: u8) -> Option<u8> {
        b.port_writes.iter().rev().find(|w| w.0 == port).map(|w| w.1)
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
        snd.set_pause_snd(0x77);

        snd.drain_port3_queue(&mut be);
        // .pause branch wins: queue flushed, forced command sent.
        assert_eq!(be.port_writes, vec![(3, 0x77)]);
        assert_eq!(snd.sdspt3, 0);
        assert_eq!(snd.sdgpt3, 0);
        assert_eq!(snd.sdpck3, 0x77);
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
        assert_eq!(sound_track_for_map(MAP_ID_1_1), Some(boot::SND_11));
        assert_eq!(sound_track_for_map(MAP_ID_2_1), Some(boot::SND_11));
        assert_eq!(sound_track_for_map(MAP_ID_3_1), Some(boot::SND_11));
        assert_eq!(sound_track_for_map(MAP_ID_1_4), Some(boot::SND_14));
        assert_eq!(sound_track_for_map(MAP_ID_BLACKHOLE), Some(boot::SND_BHOLE));
        assert_eq!(sound_track_for_map(MAP_ID_FINAL), Some(boot::SND_16));
        assert_eq!(sound_track_for_map(MAP_ID_TRAINING), Some(boot::SND_TRAINING));
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
        assert_eq!(be.booted, vec![boot::SND_11]);
        assert_eq!(be.bgm, vec![boot::track_command(boot::SND_11)]);

        // Same map again: latched, no reboot.
        snd.update(&st, None, &mut [], &mut be);
        assert_eq!(be.booted.len(), 1);

        // Death drops the latch; respawn reboots the level bank.
        st.player_dead = true;
        snd.update(&st, None, &mut [], &mut be);
        st.player_dead = false;
        snd.update(&st, None, &mut [], &mut be);
        assert_eq!(be.booted, vec![boot::SND_11, boot::SND_11]);
    }

    #[test]
    fn play_music_raw_vs_boot() {
        let mut snd = Sound::new();
        let mut be = FakeBackend::default();

        // In gameplay: raw driver command, no reboot (setbgmdo semantics).
        snd.play_music(&mut be, 5, true);
        assert_eq!(be.bgm, vec![5]);
        assert!(be.booted.is_empty());

        // >= SND_TRACK_COUNT is always a raw command (e.g. $F0 fade).
        snd.play_music(&mut be, 0xF0, false);
        assert_eq!(be.bgm, vec![5, 0xF0]);
        assert!(be.booted.is_empty());

        // Outside gameplay with a track id: full bootapu semantics.
        snd.play_music(&mut be, boot::SND_TITLE, false);
        assert_eq!(be.booted, vec![boot::SND_TITLE]);
        assert_eq!(be.bgm, vec![5, 0xF0, 0x12]);
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
