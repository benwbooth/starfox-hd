//! Enemy strategies, back half (RIIR wave 3, enemy_b lane).
//!
//! C oracle: `src/strat/strat_enemy.c`, the boss-machine slices left to
//! this lane by the enemy_a partition (see enemy_a.rs header):
//! - lines 800-1974:  boss child-link machinery + homing projectiles
//!   (DUPLICATE-tracked: enemy_a claims 800-1296; ported here so this
//!   lane compiles standalone) and the full boss7 phase machine
//!   (parts, frame animation, kill chain, alldead sequence, exp).
//! - lines 2788-3542: bossA (turrets, cups, parent machine, exp).
//! - lines 7334-9135: spacepilon (mother + child pilons), bossF
//!   (King Joh: core C/C2/C3 chain, A/B halves, 6 turrets, exp parts),
//!   and the title-screen strategy.
//!
//! The middle of the file (boss1, zaco*/worm/items/clships/EXPSTRAT
//! explosion block, HitFlash/Explode) is owned by `crate::enemy_a`;
//! strat_common.c helpers live in `crate::common`. Shared items are
//! consumed through [`eb_compat`], which either re-exports the canonical
//! lane item or carries a documented DUPLICATE until consolidation.

#![allow(dead_code)]

use self::eb_compat::*;

// ============================================================
// Registration constants (table lane contract).
// C: src/strat/strat_table.c / strat_table.h — preserved verbatim.
// ============================================================

/// C `IS_BOSS7` (ISTRATS.ASM def_Istrat index; strat_table.c:66).
pub const IS_BOSS7: usize = 99;
/// C `IS_BOSSA` (strat_table.c:54).
pub const IS_BOSSA: usize = 85;
/// C `IS_BOSSF` (strat_table.c:55).
pub const IS_BOSSF: usize = 116;

/// C `STRAT_ADDR_SPACEPILON` (strat_table.h:12).
pub const STRAT_ADDR_SPACEPILON: u32 = 0x030004;
/// C `STRAT_ADDR_TIT` (strat_table.h:13).
pub const STRAT_ADDR_TIT: u32 = 0x050020;
/// C `STRAT_ADDR_BOSSF` (strat_table.h:16).
pub const STRAT_ADDR_BOSSF: u32 = 0x060010;

// ============================================================
// eb_compat — shared-helper facade for the enemy_b/bosses lane.
//
// Canonical homes: `crate::common` (strat_common.c) and `crate::enemy_a`
// (strat_enemy.c front half). Where the canonical item exists it is
// re-exported; where it has not landed yet (or is private to its lane)
// a documented DUPLICATE copy lives here. Consolidation after landing.
// ============================================================
pub(crate) mod eb_compat {
    pub use sf_game::alien::{
        Alien, StratId, ACF_COLLTYPE1, ACF_COLLTYPE2, ACF_COLLTYPE3, ACF_COLLTYPE4,
        ACF_COLLTYPE5, ACF_COLLTYPE6, ACF_FIRSTFRAME, ACF_WEAPON, AFEXP, ASF3_REALOBJ,
        ASF4_CSPECIAL, ASF4_PLAYEROBJ, ASF4_SFLAG8, ASF4_SPECIAL, ASF_COLLDISABLE,
        ASF_COLLIDE, ASF_HITFLASH, ASF_INVISIBLE, ASF_LCOLLIDE, ASF_NOHITAFFECT,
        ASF_PARTOBJ, ASF_SHADOW, ATGND, ATLASER, ATMISSILE, ATNUKED, ATZREMOVE,
        NUMBER_AL,
    };
    pub use sf_game::game::{Game, StrategyFn};
    pub use sf_game::vars::{
        COLLTYPE_ENEMY1, FRAMESPERAP, GF_BOSSDEAD, GF_NOZREMOVE, GF_PLAYERDEAD,
        GF_PLAYERDYING, GF_STAGEDONE, GF_STRATDONE1, GF_STRATDONE2, GF_VIEWROT,
        HARD_AP, HARD_HP,
    };

    // Canonical strat_common.c ports (crate::common).
    pub use crate::common::{
        projectile_strat_ids, sf_random, snes_cos, snes_sin, strat_add_to_pos,
        strat_angle_xz, strat_apply_velocity, strat_chase, strat_chase8,
        strat_chase_proportional, strat_count_down, strat_dist_xz, strat_gen_front_vecs,
        strat_gen_side_vecs, strat_gen_vecs_2d, strat_gen_vecs_3d, strat_init_obj_vars,
        strat_make_obj, strat_perc56, strat_perc62, strat_perc75, strat_perc87,
        strat_perc93, strat_projectile_on_collide, strat_projectile_tick,
        strat_remove_obj, strat_spawn_projectile, strat_speed_to, strat_trig_se, sv,
        StratRam,
    };

    // Canonical enemy_a front-half exports (landed).
    pub use crate::enemy_a::{
        bossflags, currentlevel, gasflags, set_bossflags, set_gasflags, strat_explode,
        strat_hit_flash, wm as ea_wm,
    };

    // ---- Flag constants missing from sf-game (C src/variables.h,
    // src/game/obj.h, src/strat/strat_enemy.h). Values verbatim.
    // (enemy_a re-declares some of these too; both copies cite C.) ----

    // al_flags bits (variables.h:69-75)
    pub const AF_INRNG_PL: u8 = 2;
    pub const AF_LEFT_PL: u8 = 4;
    pub const AF_FRONT_PL: u8 = 8;
    pub const AF_INVIEW_PL: u8 = 16;
    pub const AFHIT: u8 = 32;
    pub const AFONFIRE: u8 = 64;

    // al_sflags2 bits (obj.h:121-123)
    pub const ASF2_RELEXPLODE: u8 = 0x04;
    pub const ASF2_NOEXPSND: u8 = 0x08;
    pub const ASF2_SFLAG1: u8 = 0x10;

    // al_sflags3 bits (obj.h:109-114)
    pub const ASF3_SFLAG5: u8 = 0x01;
    pub const ASF3_SFLAG7: u8 = 0x04;
    pub const ASF3_CHILDOBJ: u8 = 0x10;
    pub const ASF3_MOTHEROBJ: u8 = 0x20;
    pub const ASF3_TEXTOBJ: u8 = 0x40;
    pub const ASF3_SSPRITE: u8 = 0x80;

    // al_sflags4 bits (obj.h:116-117)
    pub const ASF4_DONESND: u8 = 0x02;
    pub const ASF4_NOPOLYEXP: u8 = 0x04;

    // stratflags (variables.h:109)
    pub const SF_NOFIRING: u8 = 1;

    // bossflags (variables.h:114-118)
    pub const BF_FLAG1: u8 = 1;
    pub const BF_FLAG2: u8 = 2;
    pub const BF_FLAG3: u8 = 4;
    pub const BF_EASYMODE: u8 = 8;
    pub const BF_DYING: u8 = 16;

    // pshipflags bits (variables.h:123-130)
    pub const PSF_BODYCOLL: u8 = 1;
    pub const PSF_LWINGCOLL: u8 = 2;
    pub const PSF_RWINGCOLL: u8 = 4;
    pub const PSF_BRKLWING: u8 = 8;
    pub const PSF_BRKRWING: u8 = 16;
    pub const PSF_NOCTRL: u8 = 32;
    pub const PSF_NOFIRE: u8 = 64;
    pub const PSF_NOYCTRL: u8 = 128;

    // pshipflags2 bits (variables.h:135-142)
    pub const PSF2_DOUBLASER: u8 = 1;
    pub const PSF2_WIRESHIP: u8 = 2;
    pub const PSF2_NOSPARK: u8 = 4;
    pub const PSF2_TURN180: u8 = 8;
    pub const PSF2_FORCEBOOST: u8 = 16;
    pub const PSF2_BOOSTING: u8 = 32;
    pub const PSF2_BRAKING: u8 = 64;
    pub const PSF2_PLAYERHP0: u8 = 128;

    // pshipflags3 bits (variables.h:147-153)
    pub const PSF3_INTUNNEL: u8 = 1;
    pub const PSF3_ENGINESND: u8 = 2;
    pub const PSF3_FORCEBRAKE: u8 = 4;
    pub const PSF3_NOCOLLISIONS: u8 = 8;
    pub const PSF3_BEAMBALL: u8 = 16;
    pub const PSF3_NOVIEWCHANGE: u8 = 32;
    pub const PSF3_KEEPPSTRAT: u8 = 64;

    // pstratflags bits (variables.h:177-182)
    pub const PSTF_NOVDISTC: u8 = 1;
    pub const PSTF_FLAG1: u8 = 2;
    pub const PSTF_NOVIEWMOVE: u8 = 4;
    pub const PSTF_INSEQ: u8 = 8;
    pub const PSTF_FIRSTFRAMELCOL: u8 = 16;
    pub const PSTF_NOTDIE: u8 = 32;

    // playerflymode bits (variables.h:158-162)
    pub const PFM_DIEFALL: u8 = 1;
    pub const PFM_DIEYROT: u8 = 2;
    pub const PFM_WATER: u8 = 4;
    pub const PFM_SHADOWS: u8 = 8;
    pub const PFM_WOBBLE: u8 = 16;

    // splayerflymode values (variables.h:167-172)
    pub const SPFM_NORM: u8 = 0;
    pub const SPFM_CLOSE: u8 = 1;
    pub const SPFM_TOINSIDE: u8 = 2;
    pub const SPFM_INSIDE: u8 = 3;
    pub const SPFM_TONORM: u8 = 4;

    // Rotation units as u8 angles (variables.h:22-41; DEG360=256 wraps 0)
    pub const DEG180: u8 = 128;
    pub const DEG90: u8 = 64;
    pub const DEG45: u8 = 32;
    pub const DEG22: u8 = 16;
    pub const DEG11: u8 = 8;
    pub const DEG5: u8 = 4;
    pub const DEG270: u8 = 192;
    pub const DEG0: u8 = 0;
    pub const DEG120: u8 = 85;
    pub const DEG60: u8 = 42;
    pub const DEG240: u8 = 170;
    pub const DEG300: u8 = 212;
    pub const DEG135: u8 = 96;
    pub const DEG225: u8 = 160;
    pub const DEG315: u8 = 224;

    // Explosion sizes (variables.h:62-64)
    pub const EXPSIZE_SMALL: i16 = 64;
    pub const EXPSIZE_MEDIUM: i16 = 128;
    pub const EXPSIZE_LARGE: i16 = 256;

    // Collision types (strat_enemy.h:24-27) + hard AP variant
    pub const COLLTYPE_ENEMY2: u8 = 0x02;
    pub const COLLTYPE_ENEMYWEAP: u8 = 0x04;
    pub const COLLTYPE_ZENEMY: u8 = 0x08;
    pub const ROCKHARD_AP: u8 = 20;

    // Game modes (variables.h:51-52)
    pub const SPACE_MODE: u8 = 1;
    pub const WATER_MODE: u8 = 2;

    // ============================================================
    // WRAM cells for C globals GameVars does not carry as fields.
    //
    // ADDRESS ALIGNMENT (cross-lane): where `crate::common::sv` defines a
    // cell it is authoritative; where only `crate::enemy_a::wm` defines
    // one, that address is used so boss1 (enemy_a) and boss7+/bosses
    // (this lane) share state (BF_DYING etc.). The wm-map addresses
    // (0x0310..0x0317) are fixed by sf_map::consts::wm because level
    // bytecode pokes the same cells (bossG wave gating!).
    // Known divergences to consolidate (reported): enemy_a::wm re-defines
    // RNDVAL/PVIEWPOSZ/NUMPLASERS/LIVES/SPECWEPCNT at 0x1F0x addresses
    // that duplicate crate::common::sv cells.
    // ============================================================
    pub mod ebwm {
        /// C `g_gsvar_byte1` (= sf_map wm::GSVAR_BYTE1; level bytecode
        /// writes this cell via setvarb).
        pub const GSVAR_BYTE1: u16 = 0x0310;
        /// C `g_maptrigger` (= wm::MAPTRIGGER).
        pub const MAPTRIGGER: u16 = 0x0311;
        /// C `g_bossmaxhp` (= wm::BOSSMAXHP, 2 bytes; also mirrored to
        /// the typed `GameVars::bossmaxhp` field by [`super::set_bossmaxhp`]).
        pub const BOSSMAXHP: u16 = 0x0316;

        /// C `g_stratflags` (SF_*) — enemy_a::wm::STRATFLAGS.
        pub const STRATFLAGS: u16 = 0x1F05;
        /// C `g_playerscore` (u16) — enemy_a::wm::PLAYERSCORE.
        pub const PLAYERSCORE: u16 = 0x1F06;
        /// C `g_specials_dead` — enemy_a::wm::SPECIALS_DEAD.
        pub const SPECIALS_DEAD: u16 = 0x1F0B;
        /// C `g_bg2Xscroll` (i16) — new cell in the enemy_a 0x1F block.
        pub const BG2XSCROLL: u16 = 0x1F30;
    }

    // Generic WRAM accessors (C RAM8/RAM16 over g_ram, variables.h:14-16).
    #[inline]
    pub fn wm8(g: &Game, addr: u16) -> u8 {
        g.vars.read_ext8(addr)
    }
    #[inline]
    pub fn wm8_set(g: &mut Game, addr: u16, v: u8) {
        g.vars.write_ext8(addr, v);
    }
    #[inline]
    pub fn wm16(g: &Game, addr: u16) -> u16 {
        g.vars.read_ext16(addr)
    }
    #[inline]
    pub fn wm16_set(g: &mut Game, addr: u16, v: u16) {
        g.vars.write_ext16(addr, v);
    }
    #[inline]
    pub fn wm16s(g: &Game, addr: u16) -> i16 {
        g.vars.read_ext16(addr) as i16
    }
    #[inline]
    pub fn wm16s_set(g: &mut Game, addr: u16, v: i16) {
        g.vars.write_ext16(addr, v as u16);
    }

    /// C `g_bossmaxhp` accessors — single C global, split representation
    /// in Rust today (GameVars field + wm cell); writes keep both coherent.
    pub fn bossmaxhp(g: &Game) -> u16 {
        g.vars.bossmaxhp
    }
    pub fn set_bossmaxhp(g: &mut Game, v: u16) {
        g.vars.bossmaxhp = v;
        wm16_set(g, ebwm::BOSSMAXHP, v);
    }

    /// C `SfRtl_Random()` — canonical common-lane PRNG
    /// (crate::common::sf_random, state at sv::RNDVAL).
    #[inline]
    pub fn sfrtl_random(g: &mut Game) -> u16 {
        sf_random(&mut g.vars)
    }

    // ============================================================
    // Object helpers
    // ============================================================

    /// C `Obj_GetPlayer` (src/game/obj.c:125): slot 0 when active.
    #[inline]
    pub fn player_idx(g: &Game) -> Option<u16> {
        if g.objs.aliens[0].active {
            Some(0)
        } else {
            None
        }
    }
    /// Copy-out of the player alien (Alien is Copy).
    #[inline]
    pub fn player(g: &Game) -> Option<Alien> {
        player_idx(g).map(|i| g.objs.aliens[i as usize])
    }

    /// C `boss_child_from_index_raw` (strat_enemy.c:838): index+1 decode,
    /// no active check (C hands back pool pointers for freed slots too).
    #[inline]
    pub fn obj_from_ptr(ptr: u16) -> Option<u16> {
        if ptr == 0 || ptr as usize > NUMBER_AL {
            None
        } else {
            Some(ptr - 1)
        }
    }
    /// C `boss_obj_index_or_null` (strat_enemy.c:829): slot -> index+1.
    #[inline]
    pub fn obj_to_ptr(idx: u16) -> u16 {
        idx + 1
    }

    /// C `Obj_Free` for strategies that free other aliens directly.
    #[inline]
    pub fn obj_free(g: &mut Game, idx: u16) {
        g.objs.free(idx);
    }

    // ============================================================
    // Front-half statics (enemy_a lane) — DUPLICATEs so this lane
    // compiles standalone; consolidate to crate::enemy_a::* re-exports.
    // ============================================================

    /// C `strat_sin` (strat_enemy.c:28). DUPLICATE of enemy_a::strat_sin.
    #[inline]
    pub fn strat_sin(angle: u8) -> f32 {
        snes_sin(angle)
    }
    /// C `strat_cos` (strat_enemy.c:31).
    #[inline]
    pub fn strat_cos(angle: u8) -> f32 {
        snes_cos(angle)
    }

    /// C `achase_angle` (strat_enemy.c:41) — 8-bit wrapping proportional
    /// chase; true at target. DUPLICATE of enemy_a::achase_angle.
    pub fn achase_angle(current: &mut u8, target: u8, shift: u32) -> bool {
        if *current == target {
            return true;
        }
        let diff = current.wrapping_sub(target) as i8;
        let mut step = diff >> shift;
        if step == 0 {
            step = if diff > 0 { 1 } else { -1 };
        }
        *current = current.wrapping_sub(step as u8);
        *current == target
    }

    /// C `add_player_z` (strat_enemy.c:55). DUPLICATE of
    /// enemy_a::add_player_z.
    #[inline]
    pub fn add_player_z(g: &mut Game, idx: u16) {
        let v = g.vars.pviewvelz;
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(v);
    }

    /// C `set_hard_vars` (strat_enemy.c:65). DUPLICATE.
    #[inline]
    pub fn set_hard_vars(al: &mut Alien) {
        al.hp = HARD_HP;
        al.ap = HARD_AP;
    }

    /// C `strat_points_positive_z` (strat_enemy.c:302). DUPLICATE.
    pub fn points_positive_z(al: &Alien) -> bool {
        let signed_yaw = al.roty as i8;
        signed_yaw >= -(DEG45 as i8) && signed_yaw <= DEG45 as i8
    }

    /// C `strat_pitch_toward` (strat_enemy.c:4366). DUPLICATE of
    /// enemy_a::strat_pitch_toward.
    pub fn strat_pitch_toward(src: &Alien, dst: &Alien) -> u8 {
        let dy = (dst.worldy as i32 - src.worldy as i32) as f32;
        let dx = (dst.worldx as i32 - src.worldx as i32) as f32;
        let dz = (dst.worldz as i32 - src.worldz as i32) as f32;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist <= 1.0 {
            return src.rotx;
        }
        let pitch = dy.atan2(dist);
        ((pitch * (256.0 / (2.0 * 3.141_592_65_f32))) as i32) as u8
    }

    // ============================================================
    // Sound (hooks)
    // ============================================================
    /// C `Sound_PlaySE`.
    #[inline]
    pub fn play_se(g: &mut Game, id: u8) {
        g.hooks.play_se(id);
    }
    /// C `Sound_PlayMusic`.
    #[inline]
    pub fn play_music(g: &mut Game, id: u8) {
        g.hooks.play_music(id);
    }

    // ============================================================
    // Strategy-id lookup: Rust replacement for taking a C function's
    // address (same behavior as enemy_a::sid; both scan the one shared
    // registry, so ids stay consistent crate-wide).
    // ============================================================
    pub fn sid(g: &mut Game, f: StrategyFn) -> StratId {
        if let Some(pos) = g
            .world
            .strat_registry
            .iter()
            .position(|&r| r as usize == f as usize)
        {
            return StratId(pos as u16);
        }
        g.world.register_strategy(f)
    }
}

// ============================================================
// EB_PART_1_BEGIN (strat_enemy.c:800-1974 — boss machinery, hmissile1,
// homingflat, relelaserhome, boss7)
// EB_PART_1_END
// ============================================================
// EB_PART_2_BEGIN (strat_enemy.c:2788-3542 — bossA)
// EB_PART_2_END
// ============================================================
// EB_PART_3_BEGIN (strat_enemy.c:7334-9135 — spacepilon, bossF, title)
// EB_PART_3_END
// ============================================================

/// Table-lane registration entry (C: the enemy_b-owned rows of
/// `Strat_RegisterAll` + `Strat_RegisterAddressMap`, strat_table.c).
/// Completed as parts land.
pub fn register(_world: &mut sf_game::world::World) {
    // Filled in by the part assembly.
}
