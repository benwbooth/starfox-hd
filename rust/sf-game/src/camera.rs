//! Camera system — computes final camera position and rotation.
//!
//! C oracle: `src/game/game.c` (GAME.ASM getview_l -> C conversion:
//! `GameCamera_Init` game.c:29, `GameCamera_Update` game.c:42) plus the
//! camera-input globals in `src/game/game_vars.c` and the viewfloat bob
//! table (game_vars.c:334).
//!
//! The camera inputs `g_viewpos*` / `g_viewshake*` / `g_viewfloat*` /
//! `g_viewtype` / `g_viewtoobj` / `g_player_turnrot` / `g_pviewpos*` /
//! `g_pviewposzoff` are strat-lane variables not yet ported into
//! [`crate::vars::GameVars`]; they live in [`CameraVars`] with the C
//! `GameVars_Init` defaults (all zero, game_vars.c:372-413) until sf-strat
//! lands. `g_viewdist` IS in GameVars (the map VM's set_player_* callbacks
//! write it), so it is read from there.
//!
//! Instead of the C `Transform_SetCamera`/`Transform_SnapCamera` renderer
//! calls (game.c:141-152), each update returns a
//! [`crate::shell::CameraSnapshot`] with `snap` set on a viewtype change.

use crate::alien::NUMBER_AL;
use crate::obj::Objects;
use crate::shell::CameraSnapshot;
use crate::vars::{GameVars, OUTVIEWDIST};

// C VIEWTYPE_* (src/variables.h:225-227).
pub const VIEWTYPE_NORM: u8 = 0;
pub const VIEWTYPE_TOOBJ: u8 = 1;
pub const VIEWTYPE_FPOS: u8 = 2;

/// C `VIEWFLOATTAB_LEN` / `g_viewfloattab` (src/game/game_vars.c:334-342).
pub const VIEWFLOATTAB_LEN: usize = 40;
pub const VIEWFLOATTAB: [i16; VIEWFLOATTAB_LEN] = [
    0, //
    1, 2, 3, 4, 4, 5, 5, 6, 6, 6, //
    5, 5, 4, 4, 3, 2, 1, //
    0, //
    -1, -2, -3, -4, -4, -5, -5, -6, -6, -6, //
    -5, -5, -4, -4, -3, -2, -1, //
    0, 0, 0, 0, // padding to 40
];

/// C `FP16_FROM_INT` (src/types.h:44).
#[inline]
pub fn fp16_from_int(i: i32) -> i32 {
    ((i as u32) << 16) as i32
}

/// Pack a world-unit float into FP16.16 (truncates toward zero like a C cast).
#[inline]
pub fn fp16_from_float(v: f32) -> i32 {
    (v * 65536.0) as i32
}

/// Strat-lane camera inputs (see module docs). All C defaults are zero
/// (game_vars.c GameVars_Init).
///
/// TODO(sf-strat): once the strat lane ports the player/camera strategies,
/// these fields move into GameVars (or the strat context) and get written
/// by viewopening / playerfly / shake strategies:
/// - `g_pviewposx/y/z`, `g_pviewposzoff` (game_vars.c:67-74)
/// - `g_viewposx/y/z` (game_vars.c:70-72, VIEWTYPE_FPOS input)
/// - `g_viewshakeX/Y/Z` (game_vars.c:85-87)
/// - `g_viewfloatptr/X/Y` (game_vars.c:75-77)
/// - `g_viewtype` (game_vars.c:232, default VIEWTYPE_NORM)
/// - `g_viewtoobj` (game_vars.c:82)
/// - `g_player_turnrot` (game_vars.c:47)
#[derive(Debug, Clone, Copy, Default)]
pub struct CameraVars {
    pub pviewposx: i16,
    pub pviewposy: i16,
    pub pviewposz: i16,
    pub pviewposzoff: i16,
    pub viewposx: i16,
    pub viewposy: i16,
    pub viewposz: i16,
    pub viewshake_x: u8,
    pub viewshake_y: u8,
    pub viewshake_z: u8,
    pub viewfloatptr: i16,
    pub viewfloat_x: i16,
    pub viewfloat_y: i16,
    pub viewtype: u8,
    pub viewtoobj: i16,
    pub player_turnrot: i16,
}

/// Camera state (C `g_camera_x/y/z`, `g_camera_rx/ry/rz`, game.c:26-27,
/// plus the `s_last_viewtype` snap-detection static, game.c:148).
#[derive(Debug, Clone, Default)]
pub struct GameCamera {
    pub vars: CameraVars,
    /// C `g_camera_x/y/z` (FP16.16).
    pub camera_x: i32,
    pub camera_y: i32,
    pub camera_z: i32,
    /// C `g_camera_rx/ry/rz`.
    pub camera_rx: i16,
    pub camera_ry: i16,
    pub camera_rz: i16,
    /// C `s_last_viewtype` (game.c:148, static -> zero-initialized).
    last_viewtype: u8,
    /// Fractional pitch-offset stash (legacy float pull-back). Unused now that
    /// pull-back is ROM `rotate_16*` integer; kept so `Default` layout stays stable.
    #[allow(dead_code)]
    frac_off_y: f32,
    #[allow(dead_code)]
    frac_off_z: f32,
    frac_off_valid: bool,
}

/// Publish an 8-bit SNES angle as the high byte of a viewrot/outv* word.
#[inline]
fn angle8_to_viewword(a: u8) -> u16 {
    (a as u16) << 8
}

/// Camera snapshot angles are signed high-bytes of viewrot words (`outvx>>8`).
#[inline]
fn angle8_as_cam(a: u8) -> i16 {
    a as i8 as i16
}

impl GameCamera {
    pub fn new() -> Self {
        Self::default()
    }

    /// C `GameCamera_Init()` (src/game/game.c:29). Writes
    /// `GameVars::viewdist` (C `g_viewdist = OUTVIEWDIST`).
    pub fn init(&mut self, vars: &mut GameVars) {
        self.camera_x = fp16_from_int(0);
        self.camera_y = fp16_from_int(0);
        self.camera_z = fp16_from_int(-500);
        self.camera_rx = 0;
        self.camera_ry = 0;
        self.camera_rz = 0;
        vars.viewdist = OUTVIEWDIST;
    }

    /// C `GameCamera_Update()` (src/game/game.c:42). Returns the snapshot
    /// the C code hands to `Transform_SetCamera`, with `snap` mirroring the
    /// `Transform_SnapCamera` call site (game.c:146-153).
    ///
    /// Writes the published fixed-view position back to the shared strategy
    /// record so the next strategy tick can copy its depth into the
    /// background-scroll state. The original order is strategies, then
    /// camera, so parallax intentionally sees the previous frame's depth.
    pub fn update(&mut self, vars: &mut GameVars, objs: &Objects) -> CameraSnapshot {
        // The translated player/view strategies and camera share the typed
        // variables that were named globals in the original source.
        let strategy = &vars.strategy;
        self.vars.pviewposx = strategy.player_view_position[0];
        self.vars.pviewposy = strategy.player_view_position[1];
        self.vars.pviewposz = strategy.player_view_position[2];
        self.vars.viewposx = strategy.fixed_view_position[0];
        self.vars.viewposy = strategy.fixed_view_position[1];
        self.vars.viewposz = strategy.fixed_view_position[2];
        self.vars.viewtype = strategy.view_kind;
        self.vars.viewtoobj = strategy.view_target_object;
        self.vars.player_turnrot = strategy.player_turn_rotation;

        // `noxrot` forces the view pitch to zero before it is latched.
        let mut outvx = strategy.view_pitch;
        if vars.shared.no_pitch_rotation != 0 {
            outvx = 0;
        }
        let outvy = strategy.view_yaw;
        let outvz = strategy.view_roll;

        let player = match objs.player() {
            Some(p) if p.active => *p,
            _ => {
                // No player (e.g. title screen before any spawn) — keep the
                // current camera transform; the C early-return path never
                // reaches the snap detection (game.c:46-50).
                return CameraSnapshot {
                    x: self.camera_x,
                    y: self.camera_y,
                    z: self.camera_z,
                    rx: self.camera_rx,
                    ry: self.camera_ry,
                    rz: self.camera_rz,
                    snap: false,
                };
            }
        };

        let (pos_x, pos_y, pos_z): (i16, i16, i16);
        let (rot_x, rot_y, rot_z): (i16, i16, i16);
        self.frac_off_valid = false;

        if self.vars.viewtype & VIEWTYPE_FPOS != 0 {
            // --- Fixed-position camera (game.c:55-61) ---
            pos_x = self.vars.viewposx;
            pos_y = self.vars.viewposy;
            pos_z = self.vars.viewposz;
        } else {
            // --- Step 1: base position from pviewpos + shake + bob
            // (game.c:63-67) ---
            let base_x = self
                .vars
                .pviewposx
                .wrapping_add(self.vars.viewshake_x as i8 as i16)
                .wrapping_add(self.vars.viewfloat_x);
            let base_y = self
                .vars
                .pviewposy
                .wrapping_add(self.vars.viewshake_y as i8 as i16)
                .wrapping_add(self.vars.viewfloat_y);
            let base_z = self
                .vars
                .pviewposz
                .wrapping_add(self.vars.viewshake_z as i8 as i16)
                .wrapping_add(self.vars.pviewposzoff);

            // View-float bob. NOTE: the exact ROM gate is unconfirmed — the
            // `viewfloat` write site is not in the disassembly (GAME.ASM only
            // *consumes* viewfloatX/Y via `adc`), and there is no `pfm_wobble`
            // flag in the ROM. Running the bob unconditionally produced a
            // continuous ~2s vertical sway that the user reported as a bug, so
            // it is gated to PFM_WOBBLE (off in normal flight) pending a proper
            // oracle trace of where the ROM advances the float pointer.
            if vars.playerflymode & crate::vars::PFM_WOBBLE != 0
                && self.vars.viewfloatptr >= 0
                && (self.vars.viewfloatptr as usize) < VIEWFLOATTAB_LEN
            {
                self.vars.viewfloat_y = VIEWFLOATTAB[self.vars.viewfloatptr as usize];
                self.vars.viewfloatptr = (self.vars.viewfloatptr + 1) % VIEWFLOATTAB_LEN as i16;
            }

            // --- Step 3-4: pull-back (GAME.ASM:66-113) ---
            // ROM: rotate (0,0,-outdist) by X=nega(viewrotxw) then Y=nega(viewrotyw)
            // via crotmat16/wmatrotp16, add into viewpos. High bytes of outvx /
            // (outvy-turnrot) match the viewrot words latched above. Tables via
            // `rotate_16yz` (auto-nega) + `rotate_16xz` (explicit nega).
            //
            // The view strategy chases `view_distance` toward `viewdist` each
            // tick. Fall back to
            // viewdist/OUTVIEWDIST only when outdist is still 0 (pre-strat /
            // unit tests) so the camera is not glued to the ship at boot.
            let outdist = vars.strategy.view_distance;
            let dist = if outdist != 0 {
                outdist
            } else if vars.viewdist > 0 {
                vars.viewdist
            } else {
                OUTVIEWDIST
            };
            let pitch8 = (outvx >> 8) as u8;
            let yaw8 = (outvy.wrapping_sub(self.vars.player_turnrot) >> 8) as u8;
            let (big_y, big_z_x) = sf_core::snes_trig::rotate_16yz(pitch8, 0, dist.wrapping_neg());
            let (big_x, big_z) = sf_core::snes_trig::rotate_16xz(yaw8.wrapping_neg(), 0, big_z_x);

            pos_x = base_x.wrapping_add(big_x);
            pos_y = base_y.wrapping_add(big_y);
            pos_z = base_z.wrapping_add(big_z);

            // Publish the final camera position (game.c:94-98).
            self.vars.viewposx = pos_x;
            self.vars.viewposy = pos_y;
            self.vars.viewposz = pos_z;

            // Integer mulslog offsets — no float residual (ROM viewpos is i16).
            self.frac_off_valid = false;
        }

        if self.vars.viewtype & VIEWTYPE_TOOBJ != 0 {
            // --- Look-at camera (GAME.ASM:133-147) ---
            // X = viewblk (viewpos), Y = viewtoobj:
            //   viewrotXw/outvx = nega(Xanglexy_l)  // elev via xzdiffs_l
            //   viewrotYw/outvy = Yanglexy_l         // yaw atan2(dx,dz)
            //   viewrotZw       = outvz              // raw; not plrotz/dozrot
            let target = {
                let idx = self.vars.viewtoobj;
                let candidate = if idx >= 0 && (idx as usize) < NUMBER_AL {
                    let al = &objs.aliens[idx as usize];
                    if al.active {
                        Some(*al)
                    } else {
                        None
                    }
                } else {
                    None
                };
                candidate.unwrap_or(player)
            };

            let dx = target.worldx.wrapping_sub(pos_x);
            let dy = target.worldy.wrapping_sub(pos_y);
            let dz = target.worldz.wrapping_sub(pos_z);
            let pitch8 = sf_core::aim_angle::xanglexy(dy, dx, dz).wrapping_neg();
            let yaw8 = sf_core::aim_angle::yanglexy(dx, dz);
            rot_x = angle8_as_cam(pitch8);
            rot_y = angle8_as_cam(yaw8);
            rot_z = outvz >> 8;
            // ROM writes the look-at words back into outvx/outvy so the next
            // frame's pull-back (non-FPOS) uses last look-at pitch.
            vars.strategy.view_pitch = angle8_to_viewword(pitch8) as i16;
            vars.strategy.view_yaw = angle8_to_viewword(yaw8) as i16;
        } else {
            // --- Step 2: normal view rotation (GAME.ASM:42-47) ---
            // viewrotxw = outvx; viewrotyw = outvy - player_turnrot.
            // High bytes are SNES angle units. In normal on-rails flight both
            // accumulators sit near 0 (gf_viewrot leans them only in all-range
            // / U-turn arenas) — rail-shooter camera stays level while the
            // Arwing moves inside the view. Position still tracks pviewpos*.
            rot_x = outvx >> 8;
            rot_y = outvy.wrapping_sub(self.vars.player_turnrot) >> 8;

            // View roll subtracts the player's roll only while depth rotation
            // is enabled.
            if vars.shared.do_depth_rotation != 0 {
                rot_z = outvz.wrapping_sub(vars.strategy.player_rotation[2]) >> 8;
            } else {
                rot_z = 0;
            }
        }

        // --- Step 5: final camera position ---
        // Pull-back is already baked into pos_* (ROM i16 viewpos). FPOS and
        // the no-player path publish whole units.
        self.camera_x = fp16_from_int(pos_x as i32);
        self.camera_y = fp16_from_int(pos_y as i32);
        self.camera_z = fp16_from_int(pos_z as i32);
        self.frac_off_valid = false;
        self.camera_rx = rot_x;
        self.camera_ry = rot_y;
        self.camera_rz = rot_z;

        // DEBUG measurement (SF_DEBUG_CAM): compare camera pitch vs ROM.
        if std::env::var_os("SF_DEBUG_CAM").is_some() {
            eprintln!(
                "CAM vt={} toobj={} campos=({},{},{}) | plr pos=({},{},{}) vel={} hp={} sb1={} sf={:x} psf2={:x} gfl={:x}",
                self.vars.viewtype, self.vars.viewtoobj,
                pos_x, pos_y, pos_z,
                player.worldx, player.worldy, player.worldz, player.vel, player.hp,
                player.sbyte1, player.sflags, vars.pshipflags2, vars.gameflags,
            );
        }

        // Camera cut detection (game.c:146-153).
        let mut snap = false;
        if self.vars.viewtype != self.last_viewtype {
            self.last_viewtype = self.vars.viewtype;
            snap = true;
        }

        // Publish the fixed-view position to the shared strategy record.
        // Fixed-position mode already sourced from these fields; normal mode overwrites with the
        // computed pull-back camera. Strat `viewmove` / `playerdead` copy
        // viewposz → bgsscrollZ on the *next* tick (audit Minor #13).
        vars.strategy.fixed_view_position =
            [self.vars.viewposx, self.vars.viewposy, self.vars.viewposz];

        CameraSnapshot {
            x: self.camera_x,
            y: self.camera_y,
            z: self.camera_z,
            rx: rot_x,
            ry: rot_y,
            rz: rot_z,
            snap,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alien::ASF4_PLAYEROBJ;

    #[test]
    fn no_player_keeps_init_transform() {
        let mut vars = GameVars::init();
        let objs = Objects::init();
        let mut cam = GameCamera::new();
        cam.init(&mut vars);
        let snap = cam.update(&mut vars, &objs);
        assert_eq!(snap.x, 0);
        assert_eq!(snap.y, 0);
        assert_eq!(snap.z, fp16_from_int(-500));
        assert_eq!((snap.rx, snap.ry, snap.rz), (0, 0, 0));
        assert!(!snap.snap);
        assert_eq!(vars.viewdist, OUTVIEWDIST);
    }

    #[test]
    fn player_normal_path_matches_c() {
        // With all CameraVars at C defaults and a player with rotx=0:
        // pitch = 0 => offset_y = 0, offset_z = -viewdist. The bob table
        // starts at 0 so viewfloatY stays 0 for the first tick.
        let mut vars = GameVars::init();
        let mut objs = Objects::init();
        let idx = objs.alloc().unwrap();
        assert_eq!(idx, 0);
        let mut cam = GameCamera::new();
        cam.init(&mut vars); // viewdist = OUTVIEWDIST = 120

        let snap = cam.update(&mut vars, &objs);
        assert_eq!(snap.x, 0);
        assert_eq!(snap.y, 0);
        // ROM mulslog pull-back: X then Y rotate; yaw0 still applies cos=127 → z=-118.
        assert_eq!(snap.z, fp16_from_int(-118));
        assert_eq!((snap.rx, snap.ry, snap.rz), (0, 0, 0));
        assert!(!snap.snap); // viewtype stayed VIEWTYPE_NORM (== static 0)
                             // viewpos published back (game.c:94-98).
        assert_eq!(
            (cam.vars.viewposx, cam.vars.viewposy, cam.vars.viewposz),
            (0, 0, -118)
        );
        assert_eq!(vars.strategy.fixed_view_position, [0, 0, -118]);
        // The view-float bob is gated to PFM_WOBBLE, which is clear in normal
        // flight, so the float pointer does NOT advance and viewfloat_y stays 0
        // (no camera bob). See the gate note in `update`.
        assert_eq!(cam.vars.viewfloatptr, 0);
        let snap2 = cam.update(&mut vars, &objs);
        assert_eq!(snap2.y, 0);
        assert_eq!(cam.vars.viewfloat_y, 0);
        let snap3 = cam.update(&mut vars, &objs);
        assert_eq!(snap3.y, 0);
    }

    #[test]
    fn viewtype_change_snaps() {
        let mut vars = GameVars::init();
        let mut objs = Objects::init();
        objs.alloc().unwrap();
        let mut cam = GameCamera::new();
        cam.init(&mut vars);

        vars.strategy.view_kind = VIEWTYPE_FPOS;
        vars.strategy.fixed_view_position = [10, 20, 30];
        let snap = cam.update(&mut vars, &objs);
        assert!(snap.snap);
        assert_eq!(snap.x, fp16_from_int(10));
        assert_eq!(snap.y, fp16_from_int(20));
        assert_eq!(snap.z, fp16_from_int(30));

        let snap2 = cam.update(&mut vars, &objs);
        assert!(!snap2.snap);
    }

    /// Float-above-ground fix check: with ASF4_PLAYEROBJ set and nonzero
    /// `rotx`, pitch comes from `outvx>>8` (ROM `getview_l`), not `player.rotx`.
    /// Normal flight leaves outvx≈0 so the camera stays level while the ship
    /// pitches inside the view.
    #[test]
    fn pitch_follows_outvx_not_player_rotx() {
        use crate::alien::ASF4_PLAYEROBJ;
        let mut vars = GameVars::init();
        let mut objs = Objects::init();
        let idx = objs.alloc().unwrap();
        objs.aliens[idx as usize].sflags4 |= ASF4_PLAYEROBJ;
        objs.aliens[idx as usize].rotx = 16; // ~22.5° nose-down (ship only)
        vars.strategy.view_pitch = 0;
        let mut cam = GameCamera::new();
        cam.init(&mut vars);
        let snap = cam.update(&mut vars, &objs);
        assert_eq!(snap.rx, 0, "ROM viewrotxw = outvx>>8 ≈ 0 in normal flight");
        assert_ne!(
            snap.rx, 16,
            "must not chase player.rotx (float-ground regression)"
        );

        // View-lean: gf_viewrot writes outvx; camera pitch follows high byte.
        vars.strategy.view_pitch = 16 << 8;
        let snap2 = cam.update(&mut vars, &objs);
        assert_eq!(snap2.rx, 16);

        // noxrot gate (GAME.ASM:6-10) zeroes outvx before latch.
        vars.shared.no_pitch_rotation = 1;
        let snap3 = cam.update(&mut vars, &objs);
        assert_eq!(snap3.rx, 0, "noxrot forces pitch 0");
    }

    /// Yaw residual closed: player-obj view yaw is ROM `outvy - turnrot`, not
    /// `player.roty`. Normal flight outvy≈0 → level yaw; gf_viewrot leans work.
    #[test]
    fn yaw_follows_outvy_not_player_roty() {
        use crate::alien::ASF4_PLAYEROBJ;
        let mut vars = GameVars::init();
        let mut objs = Objects::init();
        let idx = objs.alloc().unwrap();
        objs.aliens[idx as usize].sflags4 |= ASF4_PLAYEROBJ;
        objs.aliens[idx as usize].roty = 40; // ship yaw — must not drive cam
        vars.strategy.view_yaw = 0;
        vars.strategy.player_turn_rotation = 0;
        let mut cam = GameCamera::new();
        cam.init(&mut vars);
        let snap = cam.update(&mut vars, &objs);
        assert_eq!(snap.ry, 0, "ROM viewrotyw = (outvy-turnrot)>>8 ≈ 0");

        // View-lean yaw: outvy = 4<<8 → rot_y = 4.
        vars.strategy.view_yaw = 4 << 8;
        let snap2 = cam.update(&mut vars, &objs);
        assert_eq!(snap2.ry, 4);

        // turnrot subtracts (GAME.ASM:44-47).
        vars.strategy.player_turn_rotation = 2 << 8;
        let snap3 = cam.update(&mut vars, &objs);
        assert_eq!(snap3.ry, 2);
    }

    #[test]
    fn pullback_uses_rotate16_sintab_not_float() {
        // outvx=0x0100 → pitch 1; outdist=120 → ROM rotate_16yz(1,0,-120).
        let mut vars = GameVars::init();
        let mut objs = Objects::init();
        let idx = objs.alloc().unwrap();
        objs.aliens[idx as usize].sflags4 |= ASF4_PLAYEROBJ;
        vars.strategy.view_pitch = 1 << 8;
        vars.strategy.view_distance = 120;
        let mut cam = GameCamera::new();
        cam.init(&mut vars);
        let snap = cam.update(&mut vars, &objs);
        let (big_y, z_x) = sf_core::snes_trig::rotate_16yz(1, 0, -120);
        let (big_x, big_z) = sf_core::snes_trig::rotate_16xz(0u8.wrapping_neg(), 0, z_x);
        assert_eq!(big_x, 0);
        assert_eq!(snap.y, fp16_from_int(big_y as i32));
        assert_eq!(snap.z, fp16_from_int(big_z as i32));
        assert_eq!(cam.vars.viewposy, big_y);
        assert_eq!(cam.vars.viewposz, big_z);
        // Float sin(1*π/128)*120 ≈ 2.94 — mulslog path is the integer −2.
        assert_eq!(big_y, -2);
        assert_eq!(big_z, -117);
    }

    #[test]
    fn pullback_applies_yaw_rotate16xz() {
        // Level pitch + 90° view yaw: pull-back swings to −X (ROM Y-rotate step).
        let mut vars = GameVars::init();
        let mut objs = Objects::init();
        let idx = objs.alloc().unwrap();
        objs.aliens[idx as usize].sflags4 |= ASF4_PLAYEROBJ;
        vars.strategy.view_pitch = 0;
        vars.strategy.view_yaw = 64 << 8;
        vars.strategy.player_turn_rotation = 0;
        vars.strategy.view_distance = 120;
        let mut cam = GameCamera::new();
        cam.init(&mut vars);
        let snap = cam.update(&mut vars, &objs);
        let (_, z_after_x) = sf_core::snes_trig::rotate_16yz(0, 0, -120);
        let (expect_x, expect_z) =
            sf_core::snes_trig::rotate_16xz((64u8).wrapping_neg(), 0, z_after_x);
        assert_eq!(snap.x, fp16_from_int(expect_x as i32));
        assert_eq!(snap.z, fp16_from_int(expect_z as i32));
        assert_eq!(snap.y, 0);
        assert!(
            expect_x < -100,
            "90° yaw pulls mostly sideways, got {expect_x}"
        );
    }

    /// Frame-centroid A/B: ROM pitch (`outvx>>8 ≈ 0`) keeps pull-back Y near
    /// zero vs the old chase-feel path (`player.rotx`) which lifted the camera
    /// by `sin(pitch)*outdist` — the visual "objects float" symptom.
    #[test]
    fn float_ground_rom_pitch_keeps_pullback_level() {
        use crate::alien::ASF4_PLAYEROBJ;
        const OUTDIST: i16 = 200;
        const SHIP_PITCH: u8 = 16; // ~22.5° — ship only; cam ignores it
        let mut vars = GameVars::init();
        let mut objs = Objects::init();
        let idx = objs.alloc().unwrap();
        objs.aliens[idx as usize].sflags4 |= ASF4_PLAYEROBJ;
        objs.aliens[idx as usize].rotx = SHIP_PITCH;
        objs.aliens[idx as usize].worldy = 0;
        vars.strategy.view_pitch = 0;
        vars.strategy.view_distance = OUTDIST;
        vars.strategy.player_view_position = [0; 3];
        vars.strategy.view_kind = VIEWTYPE_NORM;

        let mut cam = GameCamera::new();
        cam.init(&mut vars);
        let snap = cam.update(&mut vars, &objs);
        let cam_y = (snap.y as f32) / 65536.0;

        // Old chase-feel would pull back by sin(ship_pitch)*outdist (~70).
        let pitch_rad = (SHIP_PITCH as i8 as f32) * (3.14159265f32 / 128.0f32);
        let chase_offset_y = pitch_rad.sin() * OUTDIST as f32;
        assert!(
            chase_offset_y.abs() > 50.0,
            "sanity: chase-feel ΔY would be large"
        );
        assert!(
            cam_y.abs() < 1.0,
            "ROM outvx pitch keeps cam Y level, got {cam_y}"
        );
        assert_eq!(snap.rx, 0);
    }

    /// VIEWTYPE_TOOBJ look-at: ROM `nega(Xanglexy)` + `Yanglexy` + raw `outvz`
    /// (GAME.ASM:133-147), not float hypot/atan2. Also writes outvx/outvy.
    #[test]
    fn toobj_lookat_uses_aim_angle_and_outvz() {
        let mut vars = GameVars::init();
        let mut objs = Objects::init();
        let player_idx = objs.alloc().unwrap();
        objs.aliens[player_idx as usize].sflags4 |= ASF4_PLAYEROBJ;
        objs.aliens[player_idx as usize].worldx = 0;
        objs.aliens[player_idx as usize].worldy = 0;
        objs.aliens[player_idx as usize].worldz = 0;

        let tgt = objs.alloc().unwrap();
        objs.aliens[tgt as usize].worldx = 0;
        objs.aliens[tgt as usize].worldy = 500;
        objs.aliens[tgt as usize].worldz = 1000;

        // FPOS so look-at uses exact viewpos (no pull-back).
        vars.strategy.view_kind = VIEWTYPE_FPOS | VIEWTYPE_TOOBJ;
        vars.strategy.view_target_object = tgt as i16;
        vars.strategy.fixed_view_position = [0; 3];
        vars.strategy.view_roll = 24 << 8;

        let mut cam = GameCamera::new();
        cam.init(&mut vars);
        let snap = cam.update(&mut vars, &objs);

        let dx: i16 = 0;
        let dy: i16 = 500;
        let dz: i16 = 1000;
        let expect_pitch = sf_core::aim_angle::xanglexy(dy, dx, dz).wrapping_neg() as i8 as i16;
        let expect_yaw = sf_core::aim_angle::yanglexy(dx, dz) as i8 as i16;
        assert_eq!(snap.rx, expect_pitch);
        assert_eq!(snap.ry, expect_yaw);
        assert_eq!(snap.rz, 24, "toobj keeps the raw view roll");

        // xzdiffs elev differs from float hypot — fidelity marker.
        assert_ne!(
            sf_core::aim_angle::xzdiffs(dx, dz),
            1000,
            "sanity: xzdiffs != hypot for this sample"
        );

        let outvx = vars.strategy.view_pitch as u16;
        let outvy = vars.strategy.view_yaw as u16;
        assert_eq!(outvx, (expect_pitch as u8 as u16) << 8);
        assert_eq!(outvy, (expect_yaw as u8 as u16) << 8);
    }

    #[test]
    fn toobj_lookat_yaw_quarter_turn() {
        let mut vars = GameVars::init();
        let mut objs = Objects::init();
        let player_idx = objs.alloc().unwrap();
        objs.aliens[player_idx as usize].sflags4 |= ASF4_PLAYEROBJ;

        let tgt = objs.alloc().unwrap();
        objs.aliens[tgt as usize].worldx = 1000;
        objs.aliens[tgt as usize].worldy = 0;
        objs.aliens[tgt as usize].worldz = 0;

        vars.strategy.view_kind = VIEWTYPE_FPOS | VIEWTYPE_TOOBJ;
        vars.strategy.view_target_object = tgt as i16;
        vars.strategy.fixed_view_position = [0; 3];

        let mut cam = GameCamera::new();
        cam.init(&mut vars);
        let snap = cam.update(&mut vars, &objs);
        assert_eq!(snap.ry, 64, "Yanglexy(+x,0) = 90°");
        assert_eq!(snap.rx, 0);
    }
}
