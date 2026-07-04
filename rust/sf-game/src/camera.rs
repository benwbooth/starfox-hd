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

// libm FFI — must match the C build's float results exactly (same libm),
// like sf-path/src/interp.rs does.
mod cmath {
    extern "C" {
        pub fn sinf(x: f32) -> f32;
        pub fn cosf(x: f32) -> f32;
        pub fn atan2f(y: f32, x: f32) -> f32;
        pub fn sqrtf(x: f32) -> f32;
        pub fn lroundf(x: f32) -> core::ffi::c_long;
    }
}

/// C `FP16_FROM_INT` (src/types.h:44).
#[inline]
pub fn fp16_from_int(i: i32) -> i32 {
    ((i as u32) << 16) as i32
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
}

/// C `angle8_from_rad` (src/game/game.c:38).
fn angle8_from_rad(rad: f32) -> i16 {
    unsafe { cmath::lroundf(rad * (128.0f32 / 3.14159265f32)) as i16 }
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
    pub fn update(&mut self, vars: &GameVars, objs: &Objects) -> CameraSnapshot {
        // The player/view strategies (sf-strat) write the live camera-follow
        // state into the GameVars WRAM mirror; the C build shared these as
        // globals, but the parallel port split them so the camera read a
        // stale local copy and never followed the ship. Sync the working
        // copy from the live WRAM slots each tick.
        //
        // Addresses MUST match `sf_strat::common::sv`.
        const SV_PLAYER_TURNROT: u16 = 0x0510;
        const SV_PVIEWPOSX: u16 = 0x053C;
        const SV_PVIEWPOSY: u16 = 0x053E;
        const SV_PVIEWPOSZ: u16 = 0x0540;
        const SV_VIEWTYPE: u16 = 0x054C;
        const SV_VIEWTOOBJ: u16 = 0x054E;
        const SV_VIEWPOSX: u16 = 0x0550;
        const SV_VIEWPOSY: u16 = 0x0552;
        const SV_VIEWPOSZ: u16 = 0x0554;
        self.vars.pviewposx = vars.read_ext16(SV_PVIEWPOSX) as i16;
        self.vars.pviewposy = vars.read_ext16(SV_PVIEWPOSY) as i16;
        self.vars.pviewposz = vars.read_ext16(SV_PVIEWPOSZ) as i16;
        self.vars.viewposx = vars.read_ext16(SV_VIEWPOSX) as i16;
        self.vars.viewposy = vars.read_ext16(SV_VIEWPOSY) as i16;
        self.vars.viewposz = vars.read_ext16(SV_VIEWPOSZ) as i16;
        self.vars.viewtype = vars.read_ext16(SV_VIEWTYPE) as u8;
        self.vars.viewtoobj = vars.read_ext16(SV_VIEWTOOBJ) as i16;
        self.vars.player_turnrot = vars.read_ext16(SV_PLAYER_TURNROT) as i16;

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

            // View-float bob. The ROM only advances/applies this when the
            // player flymode has `pfm_wobble` set (GSTRATS.ASM:611 gates the
            // advance on `playerflymode & pfm_wobble`); normal flight and all
            // tunnel modes leave it clear, so `viewfloat_y` stays 0 and the
            // camera does NOT bob. Running it unconditionally produced a
            // continuous ~2s vertical sway (the reported "wobble").
            if vars.playerflymode & crate::vars::PFM_WOBBLE != 0
                && self.vars.viewfloatptr >= 0
                && (self.vars.viewfloatptr as usize) < VIEWFLOATTAB_LEN
            {
                self.vars.viewfloat_y = VIEWFLOATTAB[self.vars.viewfloatptr as usize];
                self.vars.viewfloatptr = (self.vars.viewfloatptr + 1) % VIEWFLOATTAB_LEN as i16;
            }

            // --- Step 3-4: -outdist offset along view pitch (game.c:75-92) ---
            let dist = if vars.viewdist > 0 { vars.viewdist } else { OUTVIEWDIST };
            let pitch_rad = player.rotx as f32 * (3.14159265f32 / 128.0f32);
            let (offset_y, offset_z) = unsafe {
                (
                    (cmath::sinf(pitch_rad) * dist as f32) as i16,
                    (-cmath::cosf(pitch_rad) * dist as f32) as i16,
                )
            };

            pos_x = base_x;
            pos_y = base_y.wrapping_sub(offset_y);
            pos_z = base_z.wrapping_add(offset_z);

            // Publish the final camera position (game.c:94-98).
            self.vars.viewposx = pos_x;
            self.vars.viewposy = pos_y;
            self.vars.viewposz = pos_z;
        }

        if self.vars.viewtype & VIEWTYPE_TOOBJ != 0 {
            // --- Look-at camera (game.c:101-122) ---
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

            let dx = (target.worldx as i32 - pos_x as i32) as f32;
            let dy = (target.worldy as i32 - pos_y as i32) as f32;
            let dz = (target.worldz as i32 - pos_z as i32) as f32;
            let hlen = unsafe { cmath::sqrtf(dx * dx + dz * dz) };

            rot_y = angle8_from_rad(unsafe { cmath::atan2f(dx, dz) });
            rot_x = angle8_from_rad(unsafe { cmath::atan2f(-dy, hlen) });
            rot_z = 0;
        } else {
            // --- Step 2: normal view rotation (ROM getview_l, GAME.ASM) ---
            // viewrotyw = outvy - player_turnrot.
            rot_x = player.rotx as i16;
            rot_y = (player.roty as i32 - ((self.vars.player_turnrot >> 8) as u8) as i32) as i16;
            // viewrotzw = outvz - plrotz, but the ROM GATES this on `dozrot`:
            //   lda dozrot; bne .ok; stz viewrotzw   (GAME.ASM:52-56)
            // dozrot is set from `levelinfo & if_zroton` + setzroton/off map
            // bytecodes (WORLD.ASM). So the camera rolls ONLY in levels/sections
            // flagged for it — NOT in normal flight (Corneria). $1776 = dozrot.
            const DOZROT: u16 = 0x1776;
            if vars.read_ext8(DOZROT) != 0 {
                // Onplanet outvz=0 (PSTRATS.ASM), so the roll = -plrotz ($050A).
                const SV_PLROTZ: u16 = 0x050A;
                let plrotz = vars.read_ext16(SV_PLROTZ) as i16;
                rot_z = (0i16.wrapping_sub(plrotz)) >> 8;
            } else {
                rot_z = 0;
            }
        }

        // --- Step 5: final camera position (game.c:131-138) ---
        self.camera_x = fp16_from_int(pos_x as i32);
        self.camera_y = fp16_from_int(pos_y as i32);
        self.camera_z = fp16_from_int(pos_z as i32);
        self.camera_rx = rot_x;
        self.camera_ry = rot_y;
        self.camera_rz = rot_z;

        // Camera cut detection (game.c:146-153).
        let mut snap = false;
        if self.vars.viewtype != self.last_viewtype {
            self.last_viewtype = self.vars.viewtype;
            snap = true;
        }

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

    #[test]
    fn no_player_keeps_init_transform() {
        let mut vars = GameVars::init();
        let objs = Objects::init();
        let mut cam = GameCamera::new();
        cam.init(&mut vars);
        let snap = cam.update(&vars, &objs);
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

        let snap = cam.update(&vars, &objs);
        assert_eq!(snap.x, 0);
        assert_eq!(snap.y, 0);
        assert_eq!(snap.z, fp16_from_int(-120));
        assert_eq!((snap.rx, snap.ry, snap.rz), (0, 0, 0));
        assert!(!snap.snap); // viewtype stayed VIEWTYPE_NORM (== static 0)
        // viewpos published back (game.c:94-98).
        assert_eq!(
            (cam.vars.viewposx, cam.vars.viewposy, cam.vars.viewposz),
            (0, 0, -120)
        );
        // Bob pointer advanced.
        assert_eq!(cam.vars.viewfloatptr, 1);

        // The bob update runs AFTER the base position is computed
        // (game.c:63-73): tick 2 still uses viewfloatY = tab[0] = 0; the
        // raised tab[1] = 1 lands on tick 3.
        let snap2 = cam.update(&vars, &objs);
        assert_eq!(snap2.y, 0);
        assert_eq!(cam.vars.viewfloat_y, 1);
        let snap3 = cam.update(&vars, &objs);
        assert_eq!(snap3.y, fp16_from_int(1));
    }

    #[test]
    fn viewtype_change_snaps() {
        let mut vars = GameVars::init();
        let mut objs = Objects::init();
        objs.alloc().unwrap();
        let mut cam = GameCamera::new();
        cam.init(&mut vars);

        // The camera now reads its inputs from the live GameVars WRAM slots
        // (written by the sf-strat view code), not from `cam.vars` directly.
        // Inject through the same slots the strat lane uses.
        vars.write_ext16(0x054C, VIEWTYPE_FPOS as u16); // sv::VIEWTYPE
        vars.write_ext16(0x0550, 10); // sv::VIEWPOSX
        vars.write_ext16(0x0552, 20); // sv::VIEWPOSY
        vars.write_ext16(0x0554, 30); // sv::VIEWPOSZ
        let snap = cam.update(&vars, &objs);
        assert!(snap.snap);
        assert_eq!(snap.x, fp16_from_int(10));
        assert_eq!(snap.y, fp16_from_int(20));
        assert_eq!(snap.z, fp16_from_int(30));

        let snap2 = cam.update(&vars, &objs);
        assert!(!snap2.snap);
    }
}
